use crate::expression_analyzer;
use crate::function_analysis_data::FunctionAnalysisData;
use crate::stmt_analyzer::AnalysisError;
use crate::{scope_context::ScopeContext, statements_analyzer::StatementsAnalyzer};
use hakana_reflection_info::data_flow::graph::WholeProgramKind;
use hakana_reflection_info::{
    data_flow::{
        graph::GraphKind,
        node::DataFlowNode,
        path::{ArrayDataKind, PathKind},
    },
    t_atomic::TAtomic,
    t_union::TUnion,
};
use hakana_type::{get_mixed_any, get_nothing, wrap_atomic};
use oxidized::{aast, ast_defs::Pos};
use rustc_hash::FxHashSet;

use std::collections::BTreeMap;

pub(crate) fn analyze(
    statements_analyzer: &StatementsAnalyzer,
    tuple_fields: &[aast::Expr<(), ()>],
    pos: &Pos,
    analysis_data: &mut FunctionAnalysisData,
    context: &mut ScopeContext,
) -> Result<(), AnalysisError> {
    let mut parent_nodes = FxHashSet::default();

    let mut known_items = BTreeMap::new();
    for (i, value_expr) in tuple_fields.iter().enumerate() {
        // Now check types of the values
        expression_analyzer::analyze(
            statements_analyzer,
            value_expr,
            analysis_data,
            context,
            &mut None,
        )?;

        let value_item_type = analysis_data
            .get_expr_type(value_expr.pos())
            .cloned()
            .unwrap_or(get_mixed_any());

        if let Some(new_parent_node) = add_tuple_value_dataflow(
            statements_analyzer,
            &value_item_type,
            analysis_data,
            i,
            value_expr,
        ) {
            parent_nodes.insert(new_parent_node);
        }

        known_items.insert(i, (false, value_item_type));
    }

    let mut new_dict = wrap_atomic(TAtomic::TVec {
        known_count: Some(known_items.len()),
        known_items: if !known_items.is_empty() {
            Some(known_items)
        } else {
            None
        },
        type_param: Box::new(get_nothing()),
        non_empty: true,
    });

    new_dict.parent_nodes = parent_nodes;

    analysis_data.set_expr_type(pos, new_dict);

    Ok(())
}

fn add_tuple_value_dataflow(
    statements_analyzer: &StatementsAnalyzer,
    value_type: &TUnion,
    analysis_data: &mut FunctionAnalysisData,
    key_value: usize,
    value: &aast::Expr<(), ()>,
) -> Option<DataFlowNode> {
    if value_type.parent_nodes.is_empty()
        || (matches!(
            &analysis_data.data_flow_graph.kind,
            GraphKind::WholeProgram(WholeProgramKind::Taint)
        ) && value_type.has_taintable_value())
    {
        return None;
    }

    let node_name = format!("array[{}]", key_value);

    let new_parent_node =
        DataFlowNode::get_for_assignment(node_name, statements_analyzer.get_hpos(value.pos()));
    analysis_data
        .data_flow_graph
        .add_node(new_parent_node.clone());

    // TODO add taint event dispatches

    for parent_node in value_type.parent_nodes.iter() {
        analysis_data.data_flow_graph.add_path(
            parent_node,
            &new_parent_node,
            PathKind::ArrayAssignment(ArrayDataKind::ArrayValue, key_value.to_string()),
            None,
            None,
        );
    }

    Some(new_parent_node)
}
