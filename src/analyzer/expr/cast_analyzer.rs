use std::rc::Rc;

use rustc_hash::FxHashMap;

use crate::scope_context::ScopeContext;
use crate::statements_analyzer::StatementsAnalyzer;

use crate::expression_analyzer;
use crate::function_analysis_data::FunctionAnalysisData;
use crate::stmt_analyzer::AnalysisError;
use hakana_reflection_info::data_flow::graph::GraphKind;
use hakana_reflector::typehint_resolver::get_type_from_hint;
use hakana_type::get_mixed_any;
use oxidized::aast;

pub(crate) fn analyze(
    statements_analyzer: &StatementsAnalyzer,
    expr_pos: &aast::Pos,
    hint: &aast::Hint,
    inner_expr: &aast::Expr<(), ()>,
    analysis_data: &mut FunctionAnalysisData,
    context: &mut ScopeContext,
    if_body_context: &mut Option<ScopeContext>,
) -> Result<(), AnalysisError> {
    expression_analyzer::analyze(
        statements_analyzer,
        inner_expr,
        analysis_data,
        context,
        if_body_context,
    )?;

    let expr_type = analysis_data
        .get_rc_expr_type(inner_expr.pos())
        .cloned()
        .unwrap_or(Rc::new(get_mixed_any()));

    let mut hint_type = get_type_from_hint(
        &hint.1,
        None,
        statements_analyzer.get_type_resolution_context(),
        &FxHashMap::default(),
    )
    .unwrap();

    // todo emit issues about redundant casts

    if hint_type.has_taintable_value()
        || analysis_data.data_flow_graph.kind == GraphKind::FunctionBody
    {
        hint_type.parent_nodes = expr_type.parent_nodes.clone();
    }

    analysis_data.set_expr_type(expr_pos, hint_type);

    analysis_data.expr_effects.insert(
        (expr_pos.start_offset() as u32, expr_pos.end_offset() as u32),
        *analysis_data
            .expr_effects
            .get(&(
                inner_expr.pos().start_offset() as u32,
                inner_expr.pos().end_offset() as u32,
            ))
            .unwrap_or(&0),
    );

    Ok(())
}
