use rustc_hash::FxHashMap;

use function_context::method_identifier::MethodIdentifier;
use hakana_reflection_info::classlike_info::ClassLikeInfo;
use hakana_reflection_info::data_flow::graph::GraphKind;
use hakana_reflection_info::data_flow::node::{DataFlowNode, NodeKind};
use hakana_reflection_info::data_flow::path::PathKind;
use hakana_reflection_info::t_atomic::TAtomic;
use hakana_reflection_info::t_union::TUnion;
use hakana_type::{get_mixed_any, get_nothing, get_string, template, type_expander};
use oxidized::ast_defs::Pos;

use crate::scope_analyzer::ScopeAnalyzer;
use crate::scope_context::ScopeContext;
use crate::statements_analyzer::StatementsAnalyzer;
use crate::typed_ast::TastInfo;
use hakana_reflection_info::functionlike_info::FunctionLikeInfo;
use hakana_type::template::{TemplateBound, TemplateResult};

pub(crate) fn fetch(
    statements_analyzer: &StatementsAnalyzer,
    tast_info: &mut TastInfo,
    context: &ScopeContext,
    method_id: &MethodIdentifier,
    declaring_method_id: &MethodIdentifier,
    lhs_type_part: &TAtomic,
    functionlike_storage: &FunctionLikeInfo,
    classlike_storage: &ClassLikeInfo,
    template_result: &TemplateResult,
    call_pos: &Pos,
) -> TUnion {
    let mut return_type_candidate =
        functionlike_storage
            .return_type
            .clone()
            .unwrap_or(if method_id.0 == "__toString" {
                get_string()
            } else {
                get_mixed_any()
            });

    let codebase = statements_analyzer.get_codebase();

    let method_storage = &functionlike_storage.method_info.as_ref().unwrap();

    let mut template_result = template_result.clone();

    if !functionlike_storage.template_types.is_empty() {
        for (template_name, _) in &functionlike_storage.template_types {
            template_result
                .lower_bounds
                .entry(template_name.clone())
                .or_insert(FxHashMap::from_iter([(
                    format!("fn-{}", method_id.to_string()),
                    vec![TemplateBound::new(get_nothing(), 1, None, None)],
                )]));
        }
    }

    if !template_result.lower_bounds.is_empty() {
        type_expander::expand_union(
            codebase,
            &mut return_type_candidate,
            Some(&method_id.0),
            &type_expander::StaticClassType::None,
            classlike_storage.direct_parent_class.as_ref(),
            &mut tast_info.data_flow_graph,
            true,
            false,
            method_storage.is_final,
            true,
            true,
        );

        return_type_candidate = template::inferred_type_replacer::replace(
            &return_type_candidate,
            &template_result,
            Some(codebase),
        );
    }

    type_expander::expand_union(
        codebase,
        &mut return_type_candidate,
        Some(&method_id.0),
        &if let TAtomic::TNamedObject { .. } | TAtomic::TTemplateParam { .. } = lhs_type_part {
            type_expander::StaticClassType::Object(lhs_type_part)
        } else if let TAtomic::TClassname { as_type } = lhs_type_part {
            type_expander::StaticClassType::Object(as_type)
        } else {
            type_expander::StaticClassType::None
        },
        classlike_storage.direct_parent_class.as_ref(),
        &mut tast_info.data_flow_graph,
        true,
        false,
        method_storage.is_final,
        true,
        true,
    );

    add_dataflow(
        statements_analyzer,
        return_type_candidate,
        context,
        method_id,
        declaring_method_id,
        functionlike_storage,
        tast_info,
        call_pos,
    )
}

fn add_dataflow(
    statements_analyzer: &StatementsAnalyzer,
    mut return_type_candidate: TUnion,
    context: &ScopeContext,
    method_id: &MethodIdentifier,
    declaring_method_id: &MethodIdentifier,
    functionlike_storage: &FunctionLikeInfo,
    tast_info: &mut TastInfo,
    call_pos: &Pos,
) -> TUnion {
    // todo dispatch AddRemoveTaintsEvent

    let added_taints = None;
    let removed_taints = None;

    let ref mut data_flow_graph = tast_info.data_flow_graph;

    if data_flow_graph.kind == GraphKind::Taint {
        if !context.allow_taints {
            return return_type_candidate;
        }
    }

    let mut method_call_node = DataFlowNode::get_for_method_return(
        NodeKind::Default,
        method_id.to_string(),
        if method_id == declaring_method_id {
            functionlike_storage.return_type_location.clone()
        } else {
            None
        },
        if functionlike_storage.specialize_call && data_flow_graph.kind == GraphKind::Taint {
            Some(statements_analyzer.get_hpos(call_pos))
        } else {
            None
        },
    );

    if data_flow_graph.kind == GraphKind::Taint {
        if method_id != declaring_method_id {
            let declaring_method_call_node = DataFlowNode::get_for_method_return(
                NodeKind::Default,
                declaring_method_id.to_string(),
                functionlike_storage.return_type_location.clone(),
                None,
            );

            data_flow_graph.add_node(declaring_method_call_node.clone());
            data_flow_graph.add_path(
                &declaring_method_call_node,
                &method_call_node,
                PathKind::Default,
                added_taints,
                removed_taints,
            );
        }

        if !functionlike_storage.taint_source_types.is_empty() {
            method_call_node.taints = Some(functionlike_storage.taint_source_types.clone());
            data_flow_graph.add_source(method_call_node.clone());
        } else {
            data_flow_graph.add_node(method_call_node.clone());
        }
    } else {
        data_flow_graph.add_node(method_call_node.clone());
    }

    return_type_candidate.parent_nodes =
        FxHashMap::from_iter([(method_call_node.id.clone(), method_call_node.clone())]);

    if let GraphKind::Taint = data_flow_graph.kind {
        // todo taint using flows and taint sources
    }

    return_type_candidate
}
