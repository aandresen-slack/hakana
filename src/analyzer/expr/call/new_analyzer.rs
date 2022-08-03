use std::collections::HashMap;

use crate::expr::call_analyzer::{check_method_args, get_generic_param_for_offset};
use crate::expression_analyzer;
use crate::scope_analyzer::ScopeAnalyzer;
use crate::scope_context::ScopeContext;
use crate::statements_analyzer::StatementsAnalyzer;
use crate::typed_ast::TastInfo;
use function_context::method_identifier::MethodIdentifier;
use hakana_reflection_info::data_flow::graph::GraphKind;
use hakana_reflection_info::issue::{Issue, IssueKind};
use hakana_reflection_info::t_atomic::TAtomic;
use hakana_reflection_info::t_union::populate_union_type;
use hakana_reflector::typehint_resolver::get_type_from_hint;
use hakana_type::template::{self, TemplateBound, TemplateResult};
use hakana_type::{add_optional_union_type, get_mixed_any, get_named_object, wrap_atomic};
use indexmap::IndexMap;
use oxidized::pos::Pos;
use oxidized::{aast, ast_defs};

use super::atomic_method_call_analyzer::AtomicMethodCallAnalysisResult;

pub(crate) fn analyze(
    statements_analyzer: &StatementsAnalyzer,
    expr: (
        &aast::ClassId<(), ()>,
        &Vec<aast::Targ<()>>,
        &Vec<aast::Expr<(), ()>>,
        &Option<aast::Expr<(), ()>>,
    ),
    pos: &Pos,
    tast_info: &mut TastInfo,
    context: &mut ScopeContext,
    if_body_context: &mut Option<ScopeContext>,
) -> bool {
    //let method_id = None;

    let codebase = statements_analyzer.get_codebase();

    let mut can_extend = false;

    let lhs_type = match &expr.0 .2 {
        aast::ClassId_::CIexpr(lhs_expr) => {
            if let aast::Expr_::Id(id) = &lhs_expr.2 {
                let mut name_string = id.1.clone();
                match name_string.as_str() {
                    "self" => {
                        let self_name = &context.function_context.calling_class.clone().unwrap();

                        get_named_object(self_name.clone())
                    }
                    "parent" => {
                        let self_name = &context.function_context.calling_class.clone().unwrap();

                        let classlike_storage = codebase.classlike_infos.get(self_name).unwrap();

                        get_named_object(classlike_storage.direct_parent_class.clone().unwrap())
                    }
                    "static" => {
                        let self_name = &context.function_context.calling_class.clone().unwrap();

                        let classlike_storage = codebase.classlike_infos.get(self_name).unwrap();

                        if !classlike_storage.is_final {
                            can_extend = true;
                        }

                        wrap_atomic(TAtomic::TNamedObject {
                            name: self_name.clone(),
                            type_params: None,
                            is_this: !classlike_storage.is_final,
                            extra_types: None,
                            remapped_params: false,
                        })
                    }
                    _ => {
                        let resolved_names = statements_analyzer.get_file_analyzer().resolved_names;

                        if let Some(fq_name) = resolved_names.get(&id.0.start_offset()) {
                            name_string = fq_name.clone();
                        }

                        get_named_object(name_string)
                    }
                }
            } else {
                let was_inside_general_use = context.inside_general_use;
                context.inside_general_use = true;
                expression_analyzer::analyze(
                    statements_analyzer,
                    lhs_expr,
                    tast_info,
                    context,
                    if_body_context,
                );
                context.inside_general_use = was_inside_general_use;
                tast_info
                    .get_expr_type(&lhs_expr.1)
                    .cloned()
                    .unwrap_or(get_mixed_any())
            }
        }
        _ => {
            panic!("cannot get here")
        }
    };

    let mut result = AtomicMethodCallAnalysisResult::new();

    for (_, lhs_type_part) in &lhs_type.types {
        analyze_atomic(
            statements_analyzer,
            expr,
            pos,
            tast_info,
            context,
            if_body_context,
            lhs_type_part,
            can_extend,
            &mut result,
        );
    }

    tast_info.set_expr_type(&pos, result.return_type.clone().unwrap_or(get_mixed_any()));

    true
}

fn analyze_atomic(
    statements_analyzer: &StatementsAnalyzer,
    expr: (
        &aast::ClassId<(), ()>,
        &Vec<aast::Targ<()>>,
        &Vec<aast::Expr<(), ()>>,
        &Option<aast::Expr<(), ()>>,
    ),
    pos: &Pos,
    tast_info: &mut TastInfo,
    context: &mut ScopeContext,
    if_body_context: &mut Option<ScopeContext>,
    lhs_type_part: &TAtomic,
    can_extend: bool,
    result: &mut AtomicMethodCallAnalysisResult,
) {
    let mut from_static = false;
    let classlike_name = match &lhs_type_part {
        TAtomic::TNamedObject { name, is_this, .. } => {
            from_static = *is_this;
            // todo check class name and register usage
            name.clone()
        }
        TAtomic::TClassname { as_type, .. } | TAtomic::TTemplateParamClass { as_type, .. } => {
            let as_type = *as_type.clone();
            if let TAtomic::TNamedObject { name, .. } = as_type {
                // todo check class name and register usage
                name
            } else {
                tast_info.maybe_add_issue(Issue::new(
                    IssueKind::MixedMethodCall,
                    "Method called on unknown object".to_string(),
                    statements_analyzer.get_hpos(&pos),
                ));

                return;
            }
        }
        TAtomic::TLiteralClassname { name } => name.clone(),
        TAtomic::TTemplateParam { as_type, .. } => {
            let mut classlike_name = None;
            for (_, generic_param_type) in &as_type.types {
                if let TAtomic::TNamedObject { name, .. } = generic_param_type {
                    classlike_name = Some(name.clone());
                    break;
                } else {
                    return;
                }
            }

            if let Some(classlike_name) = classlike_name {
                classlike_name
            } else {
                // todo emit issue
                return;
            }
        }
        _ => {
            if lhs_type_part.is_mixed() {
                tast_info.maybe_add_issue(Issue::new(
                    IssueKind::MixedMethodCall,
                    "Method called on unknown object".to_string(),
                    statements_analyzer.get_hpos(&pos),
                ));
            }

            // todo handle nonobject call
            return;
        }
    };

    analyze_named_constructor(
        statements_analyzer,
        expr,
        pos,
        tast_info,
        context,
        if_body_context,
        classlike_name,
        from_static,
        can_extend,
        result,
    )
}

fn analyze_named_constructor(
    statements_analyzer: &StatementsAnalyzer,
    expr: (
        &aast::ClassId<(), ()>,
        &Vec<aast::Targ<()>>,
        &Vec<aast::Expr<(), ()>>,
        &Option<aast::Expr<(), ()>>,
    ),
    pos: &Pos,
    tast_info: &mut TastInfo,
    context: &mut ScopeContext,
    if_body_context: &mut Option<ScopeContext>,
    classlike_name: String,
    from_static: bool,
    can_extend: bool,
    result: &mut AtomicMethodCallAnalysisResult,
) {
    let codebase = statements_analyzer.get_codebase();
    let storage = if let Some(storage) = codebase.classlike_infos.get(&classlike_name) {
        storage
    } else {
        return;
    };

    if from_static {
        // todo check for unsafe instantiation
    }

    if storage.is_abstract && !can_extend {
        // todo complain about abstract instantiation
    }

    if storage.is_deprecated
        && classlike_name
            != context
                .function_context
                .calling_class
                .clone()
                .unwrap_or("".to_string())
    {
        // todo complain about deprecated class
    }

    let mut generic_type_params = None;

    let method_name = "__construct".to_string();

    if codebase.method_exists(&classlike_name, &method_name) {
        tast_info.symbol_references.add_reference_to_class_member(
            &context.function_context,
            (classlike_name.clone(), format!("{}()", method_name.clone())),
        );

        let method_id = MethodIdentifier(classlike_name.clone(), method_name);
        let mut template_result = TemplateResult::new(IndexMap::new(), IndexMap::new());

        let declaring_method_id = codebase.get_declaring_method_id(&method_id);
        let method_storage = codebase.get_method(&declaring_method_id).unwrap();

        if !check_method_args(
            statements_analyzer,
            tast_info,
            &method_id,
            method_storage,
            (
                expr.1,
                &expr
                    .2
                    .iter()
                    .map(|arg_expr| (ast_defs::ParamKind::Pnormal, arg_expr.clone()))
                    .collect::<Vec<_>>(),
                expr.3,
            ),
            &mut template_result,
            context,
            if_body_context,
            pos,
        ) {
            return;
        }

        // todo check method visibility

        // todo check purity

        if !storage.template_types.is_empty() {
            for (i, type_arg) in expr.1.iter().enumerate() {
                let mut param_type = get_type_from_hint(
                    &type_arg.1 .1,
                    context.function_context.calling_class.as_ref(),
                    &statements_analyzer.get_type_resolution_context(),
                    statements_analyzer.get_file_analyzer().resolved_names,
                );

                populate_union_type(&mut param_type, &statements_analyzer.get_codebase().symbols);

                if let Some((template_name, map)) = template_result.template_types.get_index(i) {
                    template_result.lower_bounds.insert(
                        template_name.clone(),
                        map.iter()
                            .map(|(entity, _)| {
                                (
                                    entity.clone(),
                                    vec![TemplateBound::new(param_type.clone(), 0, None, None)],
                                )
                            })
                            .collect::<HashMap<_, _>>(),
                    );
                }
            }

            let mut v = vec![];
            for (template_name, base_type_map) in storage.template_types.iter() {
                let mut generic_param_type = if let Some(template_bounds) =
                    if let Some(result_map) = template_result.lower_bounds.get(template_name) {
                        result_map.get(&classlike_name)
                    } else {
                        None
                    } {
                    template::standin_type_replacer::get_most_specific_type_from_bounds(
                        template_bounds,
                        Some(codebase),
                    )
                } else if !storage.template_extended_params.is_empty()
                    && !template_result.lower_bounds.is_empty()
                {
                    let found_generic_params = template_result
                        .lower_bounds
                        .iter()
                        .map(
                            |(key, type_map)|
                            (
                                key.clone(),
                                type_map.iter().map(
                                    |(map_key, bounds)|
                                    (map_key.clone(), template::standin_type_replacer::get_most_specific_type_from_bounds(bounds, Some(codebase)))
                                ).collect::<HashMap<_, _>>()
                            ))
                        .collect::<HashMap<_, _>>();

                    get_generic_param_for_offset(
                        &classlike_name,
                        template_name,
                        &storage.template_extended_params,
                        &found_generic_params,
                    )
                } else {
                    base_type_map.iter().next().unwrap().1.clone()
                };

                generic_param_type.had_template = true;

                v.push(generic_param_type);
            }

            generic_type_params = Some(v);
        }
    } else {
        tast_info
            .symbol_references
            .add_reference_to_symbol(&context.function_context, classlike_name.clone());

        if !expr.2.is_empty() {
            // todo complain about too many arguments
        }

        generic_type_params = if !storage.template_types.is_empty() {
            Some(
                storage
                    .template_types
                    .iter()
                    .map(|(_, map)| map.iter().next().unwrap().1.clone())
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        };
    }

    let result_type = wrap_atomic(TAtomic::TNamedObject {
        name: classlike_name,
        type_params: generic_type_params,
        is_this: from_static,
        extra_types: None,
        remapped_params: false,
    });

    if tast_info.data_flow_graph.kind == GraphKind::Taint {
        // track taints in new calls
    }

    result.return_type = Some(add_optional_union_type(
        result_type,
        result.return_type.as_ref(),
        Some(codebase),
    ));
}
