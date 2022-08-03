use function_context::FunctionLikeIdentifier;
use hakana_reflection_info::codebase_info::CodebaseInfo;
use hakana_reflection_info::data_flow::graph::GraphKind;
use hakana_reflection_info::data_flow::node::{DataFlowNode, NodeKind};
use hakana_reflection_info::data_flow::path::{PathExpressionKind, PathKind};
use hakana_reflection_info::functionlike_info::FunctionLikeInfo;
use hakana_reflection_info::t_atomic::TAtomic;
use hakana_reflection_info::t_union::TUnion;
use hakana_reflection_info::taint::TaintType;
use hakana_type::type_comparator::type_comparison_result::TypeComparisonResult;
use hakana_type::type_comparator::union_type_comparator;
use hakana_type::type_expander::StaticClassType;
use hakana_type::{
    get_bool, get_float, get_int, get_mixed, get_mixed_any, get_mixed_vec, get_nothing, get_object,
    get_string, template, type_expander, wrap_atomic,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use crate::expr::variable_fetch_analyzer;
use crate::scope_analyzer::ScopeAnalyzer;
use crate::scope_context::ScopeContext;
use crate::statements_analyzer::StatementsAnalyzer;
use crate::typed_ast::TastInfo;

use hakana_type::template::{TemplateBound, TemplateResult};
use oxidized::pos::Pos;
use oxidized::{aast, ast_defs};

pub(crate) fn fetch(
    statements_analyzer: &StatementsAnalyzer,
    expr: (
        (&Pos, &ast_defs::Id_),
        &Vec<aast::Targ<()>>,
        &Vec<(ast_defs::ParamKind, aast::Expr<(), ()>)>,
        &Option<aast::Expr<(), ()>>,
    ),
    pos: &Pos,
    functionlike_id: &FunctionLikeIdentifier,
    function_storage: &FunctionLikeInfo,
    mut template_result: TemplateResult,
    tast_info: &mut TastInfo,
    context: &mut ScopeContext,
) -> TUnion {
    let codebase = statements_analyzer.get_codebase();
    let mut stmt_type = None;

    match functionlike_id {
        FunctionLikeIdentifier::Function(name) => {
            if let Some(t) = handle_special_functions(
                statements_analyzer,
                name,
                expr.2,
                pos,
                codebase,
                tast_info,
            ) {
                stmt_type = Some(t);
            }
        }
        _ => {}
    }

    // todo support custom return type providers for functions

    let stmt_type = if let Some(stmt_type) = stmt_type {
        stmt_type
    } else {
        if let Some(function_return_type) = &function_storage.return_type {
            if !function_storage.template_types.is_empty() {
                for (template_name, _) in &function_storage.template_types {
                    match functionlike_id {
                        FunctionLikeIdentifier::Function(function_name) => {
                            // these functions take advantage of a bug/feature in the official Hack typechecker
                            // that treats all unmatched templates as a new type environment variable that can
                            // be used anywhere for any purpose until it sees clashing type constraints.
                            //
                            // For our internal usage we treat this unmatched param as "any", not "nothing"
                            if function_name == "cache_get" || function_name == "cache_get_unscoped"
                            {
                                template_result.lower_bounds.insert(
                                    template_name.clone(),
                                    HashMap::from([(
                                        format!("fn-{}", functionlike_id.to_string()),
                                        vec![TemplateBound::new(get_mixed_any(), 1, None, None)],
                                    )]),
                                );
                                continue;
                            }
                        }
                        FunctionLikeIdentifier::Method(_, _) => panic!(),
                    };

                    template_result
                        .lower_bounds
                        .entry(template_name.clone())
                        .or_insert(HashMap::from([(
                            format!("fn-{}", functionlike_id.to_string()),
                            vec![TemplateBound::new(get_nothing(), 1, None, None)],
                        )]));
                }
            }

            let mut function_return_type = function_return_type.clone();

            if !template_result.lower_bounds.is_empty()
                && !function_storage.template_types.is_empty()
            {
                type_expander::expand_union(
                    codebase,
                    &mut function_return_type,
                    None,
                    &StaticClassType::None,
                    None,
                    &mut tast_info.data_flow_graph,
                    true,
                    false,
                    false,
                    false,
                    false,
                );

                function_return_type = template::inferred_type_replacer::replace(
                    &function_return_type,
                    &template_result,
                    Some(codebase),
                );
            }

            type_expander::expand_union(
                codebase,
                &mut function_return_type,
                None,
                &StaticClassType::None,
                None,
                &mut tast_info.data_flow_graph,
                true,
                false,
                false,
                true,
                false,
            );

            // todo dispatch AfterFunctionCallAnalysisEvent

            function_return_type
        } else {
            get_mixed_any()
        }
    };

    return add_dataflow(
        statements_analyzer,
        expr,
        pos,
        functionlike_id,
        function_storage,
        stmt_type,
        &template_result,
        tast_info,
        context,
    );
}

fn handle_special_functions(
    statements_analyzer: &StatementsAnalyzer,
    name: &String,
    args: &Vec<(ast_defs::ParamKind, aast::Expr<(), ()>)>,
    pos: &Pos,
    codebase: &CodebaseInfo,
    tast_info: &mut TastInfo,
) -> Option<TUnion> {
    match name.as_str() {
        "HH\\global_get" => {
            if let Some((_, arg_expr)) = args.get(0) {
                if let Some(expr_type) = tast_info.get_expr_type(arg_expr.pos()) {
                    if let Some(value) = expr_type.get_single_literal_string_value() {
                        Some(variable_fetch_analyzer::get_type_for_superglobal(
                            statements_analyzer,
                            value,
                            pos,
                            tast_info,
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
        // bool
        "hash_equals" | "in_array" => Some(get_bool()),
        // int
        "mb_strlen" => Some(get_int()),
        // string
        "utf8_encode" | "sha1" | "dirname" | "hash_hmac" | "vsprintf" | "trim" | "ltrim"
        | "rtrim" | "strpad" | "str_repeat" | "md5" | "basename" | "strtolower" | "strtoupper" => {
            Some(get_string())
        }
        // falsable strings
        "json_encode" | "file_get_contents" | "hex2bin" | "realpath" | "date" | "base64_decode" => {
            let mut false_or_string = TUnion::new(vec![TAtomic::TString, TAtomic::TFalse]);
            false_or_string.ignore_falsable_issues = true;
            Some(false_or_string)
        }
        // falsable ints
        "strtotime" | "mktime" => {
            let mut false_or_int = TUnion::new(vec![TAtomic::TInt, TAtomic::TFalse]);
            false_or_int.ignore_falsable_issues = true;
            Some(false_or_int)
        }
        "preg_split" => {
            if let Some((_, arg_expr)) = args.get(3) {
                if let Some(expr_type) = tast_info.get_expr_type(arg_expr.pos()) {
                    return if let Some(value) = expr_type.get_single_literal_int_value() {
                        match value {
                            0 | 2 => {
                                let mut false_or_string_vec = TUnion::new(vec![
                                    TAtomic::TVec {
                                        known_items: None,
                                        type_param: get_string(),
                                        known_count: None,
                                        non_empty: true,
                                    },
                                    TAtomic::TFalse,
                                ]);
                                false_or_string_vec.ignore_falsable_issues = true;
                                Some(false_or_string_vec)
                            }
                            1 | 3 => {
                                let mut false_or_string_vec = TUnion::new(vec![
                                    TAtomic::TVec {
                                        known_items: None,
                                        type_param: get_string(),
                                        known_count: None,
                                        non_empty: false,
                                    },
                                    TAtomic::TFalse,
                                ]);
                                false_or_string_vec.ignore_falsable_issues = true;
                                Some(false_or_string_vec)
                            }
                            _ => {
                                let mut false_or_string_vec = TUnion::new(vec![
                                    TAtomic::TVec {
                                        known_items: None,
                                        type_param: wrap_atomic(TAtomic::TVec {
                                            known_items: Some(BTreeMap::from([
                                                (0, (false, get_string())),
                                                (1, (false, get_int())),
                                            ])),
                                            type_param: get_nothing(),
                                            known_count: None,
                                            non_empty: true,
                                        }),
                                        known_count: None,
                                        non_empty: false,
                                    },
                                    TAtomic::TFalse,
                                ]);
                                false_or_string_vec.ignore_falsable_issues = true;
                                Some(false_or_string_vec)
                            }
                        }
                    } else {
                        let mut false_or_string_vec = TUnion::new(vec![
                            TAtomic::TVec {
                                known_items: None,
                                type_param: get_mixed(),
                                known_count: None,
                                non_empty: true,
                            },
                            TAtomic::TFalse,
                        ]);
                        false_or_string_vec.ignore_falsable_issues = true;
                        Some(false_or_string_vec)
                    };
                }
            } else {
                let mut false_or_string_vec = TUnion::new(vec![
                    TAtomic::TVec {
                        known_items: None,
                        type_param: get_string(),
                        known_count: None,
                        non_empty: true,
                    },
                    TAtomic::TFalse,
                ]);
                false_or_string_vec.ignore_falsable_issues = true;
                return Some(false_or_string_vec);
            }

            None
        }
        "debug_backtrace" => Some(wrap_atomic(TAtomic::TVec {
            known_items: None,
            type_param: wrap_atomic(TAtomic::TDict {
                known_items: Some(BTreeMap::from([
                    ("file".to_string(), (false, Arc::new(get_string()))),
                    ("line".to_string(), (false, Arc::new(get_int()))),
                    ("function".to_string(), (false, Arc::new(get_string()))),
                    ("class".to_string(), (true, Arc::new(get_string()))),
                    ("object".to_string(), (true, Arc::new(get_object()))),
                    ("type".to_string(), (true, Arc::new(get_string()))),
                    ("args".to_string(), (true, Arc::new(get_mixed_vec()))),
                ])),
                enum_items: None,
                key_param: get_nothing(),
                value_param: get_nothing(),
                non_empty: true,
                shape_name: None,
            }),
            known_count: None,
            non_empty: true,
        })),
        "str_replace" => {
            // returns string if the second arg is a string
            if let Some((_, arg_expr)) = args.get(1) {
                if let Some(expr_type) = tast_info.get_expr_type(arg_expr.pos()) {
                    if union_type_comparator::is_contained_by(
                        codebase,
                        expr_type,
                        &get_string(),
                        false,
                        false,
                        false,
                        &mut TypeComparisonResult::new(),
                    ) {
                        Some(get_string())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
        "preg_replace" => {
            // returns string if the third arg is a string
            if let Some((_, arg_expr)) = args.get(2) {
                if let Some(expr_type) = tast_info.get_expr_type(arg_expr.pos()) {
                    if union_type_comparator::is_contained_by(
                        codebase,
                        expr_type,
                        &get_string(),
                        false,
                        false,
                        false,
                        &mut TypeComparisonResult::new(),
                    ) {
                        Some(get_string())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
        "microtime" => {
            if let Some((_, arg_expr)) = args.get(0) {
                if let Some(expr_type) = tast_info.get_expr_type(arg_expr.pos()) {
                    if expr_type.is_always_truthy() {
                        Some(get_float())
                    } else if expr_type.is_always_falsy() {
                        Some(get_string())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

fn add_dataflow(
    statements_analyzer: &StatementsAnalyzer,
    expr: (
        (&Pos, &ast_defs::Id_),
        &Vec<aast::Targ<()>>,
        &Vec<(ast_defs::ParamKind, aast::Expr<(), ()>)>,
        &Option<aast::Expr<(), ()>>,
    ),
    pos: &Pos,
    functionlike_id: &FunctionLikeIdentifier,
    functionlike_storage: &FunctionLikeInfo,
    stmt_type: TUnion,
    _template_result: &TemplateResult,
    tast_info: &mut TastInfo,
    context: &mut ScopeContext,
) -> TUnion {
    let _codebase = statements_analyzer.get_codebase();

    // todo dispatch AddRemoveTaintsEvent

    //let added_taints = Vec::new();
    //let removed_taints = Vec::new();

    let ref mut data_flow_graph = tast_info.data_flow_graph;

    if data_flow_graph.kind == GraphKind::Taint {
        if !context.allow_taints {
            return stmt_type;
        }
    }

    let mut function_call_node = DataFlowNode::get_for_method_return(
        NodeKind::Default,
        functionlike_id.to_string(),
        if let Some(return_pos) = &functionlike_storage.return_type_location {
            Some(return_pos.clone())
        } else {
            functionlike_storage.name_location.clone()
        },
        if functionlike_storage.specialize_call {
            Some(statements_analyzer.get_hpos(pos))
        } else {
            None
        },
    );

    let mut stmt_type = stmt_type;

    // todo conditionally remove taints

    if data_flow_graph.kind == GraphKind::Taint {
        if !functionlike_storage.return_source_params.is_empty() {
            // todo dispatch AddRemoveTaintEvent
            // and also handle simple preg_replace calls
        }

        let (param_offsets, _variadic_path) = get_special_argument_nodes(functionlike_id);
        let added_removed_taints = get_special_added_removed_taints(functionlike_id);

        for (param_offset, path_kind) in param_offsets {
            let argument_node = get_node_for_argument(
                statements_analyzer,
                expr,
                pos,
                &functionlike_storage.name,
                functionlike_storage,
                param_offset,
            );

            let (added_taints, removed_taints) =
                if let Some(added_removed_taints) = added_removed_taints.get(&param_offset) {
                    added_removed_taints.clone()
                } else {
                    (HashSet::new(), HashSet::new())
                };

            data_flow_graph.add_path(
                &argument_node,
                &function_call_node,
                path_kind,
                added_taints,
                removed_taints,
            );
            data_flow_graph.add_node(argument_node);
        }

        if !functionlike_storage.taint_source_types.is_empty() {
            function_call_node.taints = Some(functionlike_storage.taint_source_types.clone());
            data_flow_graph.add_source(function_call_node.clone());
        } else {
            data_flow_graph.add_node(function_call_node.clone());
        }
    } else {
        data_flow_graph.add_node(function_call_node.clone());
    }

    stmt_type
        .parent_nodes
        .insert(function_call_node.id.clone(), function_call_node);

    stmt_type
}

fn get_special_argument_nodes(
    functionlike_id: &FunctionLikeIdentifier,
) -> (Vec<(usize, PathKind)>, Option<PathKind>) {
    match functionlike_id {
        FunctionLikeIdentifier::Function(function_name) => match function_name.as_str() {
            "var_export"
            | "print_r"
            | "highlight_string"
            | "strtolower"
            | "strtoupper"
            | "trim"
            | "ltrim"
            | "rtrim"
            | "HH\\Lib\\Str\\trim"
            | "HH\\Lib\\Str\\trim_left"
            | "HH\\Lib\\Str\\trim_right"
            | "strip_tags"
            | "stripslashes"
            | "stripcslashes"
            | "htmlentities"
            | "htmlentitydecode"
            | "htmlspecialchars"
            | "htmlspecialchars_decode"
            | "str_repeat"
            | "str_rot13"
            | "str_shuffle"
            | "strstr"
            | "stristr"
            | "strchr"
            | "strpbrk"
            | "strrchr"
            | "strrev"
            | "substr"
            | "preg_quote"
            | "wordwrap"
            | "realpath"
            | "strval"
            | "strgetcsv"
            | "addcslashes"
            | "addslashes"
            | "ucfirst"
            | "ucwords"
            | "lcfirst"
            | "nl2br"
            | "quoted_printable_decode"
            | "quoted_printable_encode"
            | "quote_meta"
            | "chop"
            | "convert_uudecode"
            | "convert_uuencode"
            | "json_encode"
            | "json_decode"
            | "base64_encode"
            | "base64_decode" => (vec![(0, PathKind::Default)], None),
            "var_dump" | "printf" => (vec![(0, PathKind::Default)], Some(PathKind::Default)),
            "sscanf" | "substr_replace" => {
                (vec![(0, PathKind::Default), (1, PathKind::Default)], None)
            }
            "str_replace" | "str_ireplace" | "preg_filter" | "preg_replace" => {
                (vec![(1, PathKind::Default), (2, PathKind::Default)], None)
            }
            "str_pad" | "chunk_split" => {
                (vec![(0, PathKind::Default), (2, PathKind::Default)], None)
            }
            "implode" | "join" => (
                vec![
                    (0, PathKind::Default),
                    (
                        1,
                        PathKind::UnknownExpressionFetch(PathExpressionKind::ArrayValue),
                    ),
                ],
                None,
            ),
            "explode" | "preg_split" => (
                vec![(
                    1,
                    PathKind::UnknownExpressionAssignment(PathExpressionKind::ArrayValue),
                )],
                None,
            ),
            "str_split" => (
                vec![(
                    0,
                    PathKind::UnknownExpressionAssignment(PathExpressionKind::ArrayValue),
                )],
                None,
            ),
            "HH\\Lib\\Vec\\sort" => (vec![(0, PathKind::Default)], None),
            "HH\\Lib\\Str\\join" => (
                vec![
                    (
                        0,
                        PathKind::UnknownExpressionFetch(PathExpressionKind::ArrayValue),
                    ),
                    (1, PathKind::Default),
                ],
                None,
            ),
            "HH\\Lib\\Vec\\map" | "HH\\Lib\\Dict\\map" | "HH\\Lib\\Keyset\\map" => (
                vec![(
                    1,
                    PathKind::UnknownExpressionAssignment(PathExpressionKind::ArrayValue),
                )],
                None,
            ),
            "HH\\Lib\\Vec\\filter" | "HH\\Lib\\Dict\\filter" | "HH\\Lib\\Keyset\\filter" => {
                (vec![(0, PathKind::Default)], None)
            }
            _ => (vec![], None),
        },
        FunctionLikeIdentifier::Method(_, _) => panic!(),
    }
}

fn get_special_added_removed_taints(
    functionlike_id: &FunctionLikeIdentifier,
) -> HashMap<usize, (HashSet<TaintType>, HashSet<TaintType>)> {
    match functionlike_id {
        FunctionLikeIdentifier::Function(function_name) => match function_name.as_str() {
            "html_entity_decode" | "htmlspecialchars_decode" => {
                HashMap::from([(0, (HashSet::from([TaintType::HtmlTag]), HashSet::new()))])
            }
            "htmlentities" | "htmlspecialchars" | "strip_tags" => {
                HashMap::from([(0, (HashSet::new(), HashSet::from([TaintType::HtmlTag])))])
            }
            _ => HashMap::new(),
        },
        FunctionLikeIdentifier::Method(_, _) => panic!(),
    }
}

fn get_node_for_argument(
    statements_analyzer: &StatementsAnalyzer,
    expr: (
        (&Pos, &ast_defs::Id_),
        &Vec<aast::Targ<()>>,
        &Vec<(ast_defs::ParamKind, aast::Expr<(), ()>)>,
        &Option<aast::Expr<(), ()>>,
    ),
    pos: &Pos,
    function_name: &String,
    function_storage: &FunctionLikeInfo,
    argument_offset: usize,
) -> DataFlowNode {
    DataFlowNode::get_for_method_argument(
        NodeKind::Default,
        function_name.clone(),
        argument_offset,
        if let Some(arg) = expr.2.get(argument_offset) {
            Some(statements_analyzer.get_hpos(arg.1.pos()))
        } else {
            None
        },
        if function_storage.specialize_call {
            Some(statements_analyzer.get_hpos(pos))
        } else {
            None
        },
    )
}
