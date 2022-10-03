use super::control_analyzer;
use crate::reconciler::reconciler;
use crate::scope_analyzer::ScopeAnalyzer;
use crate::scope_context::control_action::ControlAction;
use crate::scope_context::loop_scope::LoopScope;
use crate::scope_context::var_has_root;
use crate::scope_context::{if_scope::IfScope, ScopeContext};
use crate::{statements_analyzer::StatementsAnalyzer, typed_ast::TastInfo};
use hakana_reflection_info::codebase_info::CodebaseInfo;
use hakana_type::add_union_type;
use oxidized::aast;
use rustc_hash::{FxHashMap, FxHashSet};

pub(crate) fn analyze(
    statements_analyzer: &StatementsAnalyzer,
    stmt: (
        &aast::Expr<(), ()>,
        &aast::Block<(), ()>,
        &aast::Block<(), ()>,
    ),
    tast_info: &mut TastInfo,
    if_scope: &mut IfScope,
    mut cond_referenced_var_ids: FxHashSet<String>,
    if_context: &mut ScopeContext,
    outer_context: &mut ScopeContext,
    loop_scope: &mut Option<LoopScope>,
) -> bool {
    let codebase = statements_analyzer.get_codebase();

    let cond_object_id = (stmt.0.pos().start_offset(), stmt.0.pos().end_offset());

    let (reconcilable_if_types, active_if_types) = hakana_algebra::get_truths_from_formula(
        if_context.clauses.iter().map(|v| &**v).collect(),
        Some(cond_object_id),
        &mut cond_referenced_var_ids,
    );

    if !outer_context
        .clauses
        .iter()
        .filter(|clause| !clause.possibilities.is_empty())
        .next()
        .is_some()
    {
        let mut omit_keys =
            outer_context
                .clauses
                .iter()
                .fold(FxHashSet::default(), |mut acc, clause| {
                    acc.extend(clause.possibilities.keys().collect::<FxHashSet<_>>());
                    acc
                });

        let (dont_omit_keys, _) = hakana_algebra::get_truths_from_formula(
            outer_context.clauses.iter().map(|v| &**v).collect(),
            None,
            &mut FxHashSet::default(),
        );

        let dont_omit_keys = dont_omit_keys.keys().collect::<FxHashSet<_>>();

        omit_keys.retain(|k| !dont_omit_keys.contains(k));

        cond_referenced_var_ids.retain(|k| !omit_keys.contains(k));
    }

    // if the if has an || in the conditional, we cannot easily reason about it
    if !reconcilable_if_types.is_empty() {
        let mut changed_var_ids = FxHashSet::default();

        reconciler::reconcile_keyed_types(
            &reconcilable_if_types,
            active_if_types,
            if_context,
            &mut changed_var_ids,
            &cond_referenced_var_ids,
            statements_analyzer,
            tast_info,
            stmt.0.pos(),
            true,
            false,
            &FxHashMap::default(),
        );

        if !changed_var_ids.is_empty() {
            if_context.clauses =
                ScopeContext::remove_reconciled_clause_refs(&if_context.clauses, &changed_var_ids)
                    .0;

            for changed_var_id in &changed_var_ids {
                for (var_id, _) in if_context.vars_in_scope.clone() {
                    if var_has_root(&var_id, changed_var_id) {
                        if !changed_var_ids.contains(&var_id)
                            && !cond_referenced_var_ids.contains(&var_id)
                        {
                            if_context.vars_in_scope.remove(&var_id);
                        }
                    }
                }
            }
        }
    }

    if_context.reconciled_expression_clauses = Vec::new();

    let assigned_var_ids = if_context.assigned_var_ids.clone();
    let possibly_assigned_var_ids = if_context.possibly_assigned_var_ids.clone();

    if_context.assigned_var_ids.clear();
    if_context.possibly_assigned_var_ids.clear();

    if !statements_analyzer.analyze(stmt.1, tast_info, if_context, loop_scope) {
        return false;
    }

    let final_actions = control_analyzer::get_control_actions(
        codebase,
        statements_analyzer.get_file_analyzer().resolved_names,
        stmt.1,
        Some(tast_info),
        Vec::new(),
        true,
    );

    let has_ending_statements =
        final_actions.len() == 1 && final_actions.contains(&ControlAction::End);

    let has_leaving_statements = has_ending_statements
        || final_actions.len() > 0 && !final_actions.contains(&ControlAction::None);

    let has_break_statement =
        final_actions.len() == 1 && final_actions.contains(&ControlAction::Break);

    if_scope.if_actions = final_actions.clone();
    if_scope.final_actions = final_actions;

    let new_assigned_var_ids = if_context.assigned_var_ids.clone();
    let new_possibly_assigned_var_ids = if_context.possibly_assigned_var_ids.clone();

    if_context.assigned_var_ids.extend(assigned_var_ids.clone());
    if_context
        .possibly_assigned_var_ids
        .extend(possibly_assigned_var_ids.clone());

    if !has_leaving_statements {
        update_if_scope(
            codebase,
            if_scope,
            if_context,
            outer_context,
            &new_assigned_var_ids,
            &new_possibly_assigned_var_ids,
            if_scope.if_cond_changed_var_ids.clone(),
            true,
        );

        let mut reasonable_clauses = if_scope.reasonable_clauses.clone();

        if !reasonable_clauses.is_empty() {
            for (var_id, _) in new_assigned_var_ids {
                reasonable_clauses = ScopeContext::filter_clauses(
                    &var_id,
                    reasonable_clauses,
                    if let Some(t) = if_context.vars_in_scope.get(&var_id) {
                        Some((&t).clone())
                    } else {
                        None
                    },
                    Some(statements_analyzer),
                    tast_info,
                );
            }
        }

        if_scope.reasonable_clauses = reasonable_clauses;
    } else if !has_break_statement {
        if_scope.reasonable_clauses = Vec::new();
    }

    true
}

pub(crate) fn update_if_scope(
    codebase: &CodebaseInfo,
    if_scope: &mut IfScope,
    if_context: &ScopeContext,
    outer_context: &ScopeContext,
    assigned_var_ids: &FxHashMap<String, usize>,
    possibly_assigned_var_ids: &FxHashSet<String>,
    newly_reconciled_var_ids: FxHashSet<String>,
    update_new_vars: bool,
) {
    let redefined_vars = if_context.get_redefined_vars(
        &outer_context.vars_in_scope,
        false,
        &mut if_scope.removed_var_ids,
    );

    if let Some(ref mut new_vars) = if_scope.new_vars {
        for (new_var_id, new_type) in new_vars.clone() {
            if let Some(if_var_type) = if_context.vars_in_scope.get(&new_var_id) {
                new_vars.insert(
                    new_var_id,
                    hakana_type::add_union_type(new_type, if_var_type, codebase, false),
                );
            } else {
                new_vars.remove(&new_var_id);
            }
        }
    } else {
        if update_new_vars {
            if_scope.new_vars = Some(
                if_context
                    .vars_in_scope
                    .iter()
                    .filter(|(k, _)| !outer_context.vars_in_scope.contains_key(*k))
                    .map(|(k, v)| (k.clone(), (**v).clone()))
                    .collect(),
            );
        }
    }

    let mut possibly_redefined_vars = redefined_vars.clone();

    for (var_id, _) in possibly_redefined_vars.clone() {
        if !possibly_assigned_var_ids.contains(&var_id)
            && newly_reconciled_var_ids.contains(&var_id)
        {
            possibly_redefined_vars.remove(&var_id);
        }
    }

    if let Some(ref mut scope_assigned_var_ids) = if_scope.assigned_var_ids {
        *scope_assigned_var_ids = assigned_var_ids
            .clone()
            .into_iter()
            .filter(|(k, _)| scope_assigned_var_ids.contains_key(k))
            .collect::<FxHashMap<_, _>>();
    } else {
        if_scope.assigned_var_ids = Some(assigned_var_ids.clone());
    }

    if_scope
        .possibly_assigned_var_ids
        .extend(possibly_assigned_var_ids.clone());

    if let Some(ref mut scope_redefined_vars) = if_scope.redefined_vars {
        for (redefined_var_id, scope_redefined_type) in scope_redefined_vars.clone() {
            if let Some(redefined_var_type) = redefined_vars.get(&redefined_var_id) {
                scope_redefined_vars.insert(
                    redefined_var_id.clone(),
                    hakana_type::combine_union_types(
                        redefined_var_type,
                        &scope_redefined_type,
                        codebase,
                        false,
                    ),
                );

                if let Some(outer_context_type) = outer_context.vars_in_scope.get(&redefined_var_id)
                {
                    if scope_redefined_type == **outer_context_type {
                        scope_redefined_vars.remove(&redefined_var_id);
                    }
                }
            } else {
                scope_redefined_vars.remove(&redefined_var_id);
            }
        }

        let mut new_scoped_possibly_redefined_vars = FxHashMap::default();

        for (var_id, possibly_redefined_type) in possibly_redefined_vars {
            if let Some(existing_type) = if_scope.possibly_redefined_vars.get(&var_id) {
                new_scoped_possibly_redefined_vars.insert(
                    var_id.clone(),
                    add_union_type(
                        possibly_redefined_type,
                        &existing_type,
                        codebase,
                        false,
                    ),
                );
            } else {
                new_scoped_possibly_redefined_vars.insert(var_id, possibly_redefined_type);
            }
        }

        if_scope
            .possibly_redefined_vars
            .extend(new_scoped_possibly_redefined_vars);
    } else {
        if_scope.redefined_vars = Some(redefined_vars);
        if_scope.possibly_redefined_vars = possibly_redefined_vars;
    }
}
