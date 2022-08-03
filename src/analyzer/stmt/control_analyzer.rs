use std::collections::{HashMap, HashSet};

use crate::scope_context::control_action::ControlAction;
use hakana_reflection_info::codebase_info::CodebaseInfo;
use oxidized::aast;

use crate::typed_ast::TastInfo;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BreakContext {
    Switch,
    Loop,
}

pub(crate) fn get_control_actions(
    codebase: &CodebaseInfo,
    resolved_names: &HashMap<usize, String>,
    stmts: &Vec<aast::Stmt<(), ()>>,
    tast_info: Option<&TastInfo>,
    break_context: Vec<BreakContext>,
    return_is_exit: bool, // default true
) -> HashSet<ControlAction> {
    let mut control_actions = HashSet::new();

    if stmts.len() == 0 {
        control_actions.insert(ControlAction::None);
        return control_actions;
    }

    'outer: for stmt in stmts {
        match &stmt.1 {
            aast::Stmt_::Expr(_) => {
                let inner_expr = &stmt.1.as_expr().unwrap().2;

                if let aast::Expr_::Call(call_expr) = inner_expr {
                    if let Some(value) =
                        handle_call(call_expr, resolved_names, codebase, &control_actions)
                    {
                        return value;
                    }
                }
            }
            aast::Stmt_::Break => {
                if break_context.len() > 0 {
                    if let &BreakContext::Switch = break_context.last().unwrap() {
                        if !control_actions.contains(&ControlAction::LeaveSwitch) {
                            control_actions.insert(ControlAction::LeaveSwitch);
                        }
                    }

                    return control_actions;
                }

                if !control_actions.contains(&ControlAction::Break) {
                    control_actions.insert(ControlAction::Break);
                }

                return control_actions;
            }
            aast::Stmt_::Continue => {
                if !control_actions.contains(&ControlAction::Continue) {
                    control_actions.insert(ControlAction::Continue);
                }

                return control_actions;
            }
            aast::Stmt_::Throw(_) | aast::Stmt_::Return(_) => {
                if !return_is_exit && stmt.1.is_return() {
                    return control_return(control_actions);
                }

                return control_end(control_actions);
            }
            aast::Stmt_::If(_) => {
                let if_stmt = stmt.1.as_if().unwrap();

                let if_statement_actions = get_control_actions(
                    codebase,
                    resolved_names,
                    if_stmt.1,
                    tast_info,
                    break_context.clone(),
                    return_is_exit,
                );

                let mut all_leave = if_statement_actions
                    .iter()
                    .filter(|action| *action == &ControlAction::None)
                    .count()
                    == 0;

                let else_statement_actions = get_control_actions(
                    codebase,
                    resolved_names,
                    if_stmt.2,
                    tast_info,
                    break_context.clone(),
                    return_is_exit,
                );

                all_leave = all_leave
                    && else_statement_actions
                        .iter()
                        .filter(|action| *action == &ControlAction::None)
                        .count()
                        == 0;

                control_actions.extend(if_statement_actions);
                control_actions.extend(else_statement_actions);

                if all_leave {
                    return control_actions;
                }

                control_actions.retain(|action| *action != ControlAction::None);
            }
            aast::Stmt_::Do(_)
            | aast::Stmt_::While(_)
            | aast::Stmt_::Foreach(_)
            | aast::Stmt_::For(_) => {
                let loop_stmts = if stmt.1.is_do() {
                    stmt.1.as_do().unwrap().0
                } else if stmt.1.is_while() {
                    stmt.1.as_while().unwrap().1
                } else if stmt.1.is_for() {
                    stmt.1.as_for().unwrap().3
                } else {
                    stmt.1.as_foreach().unwrap().2
                };

                let mut loop_break_context = break_context.clone();
                loop_break_context.push(BreakContext::Loop);

                let loop_actions = get_control_actions(
                    codebase,
                    resolved_names,
                    loop_stmts,
                    tast_info,
                    loop_break_context,
                    return_is_exit,
                );

                control_actions.extend(loop_actions);

                control_actions = control_actions
                    .into_iter()
                    .filter(|action| action != &ControlAction::None)
                    .collect();

                // check for infinite loop behaviour
                if let Some(types) = tast_info {
                    if stmt.1.is_while() {
                        let stmt = stmt.1.as_while().unwrap();

                        if let Some(expr_type) = types.get_expr_type(&stmt.0 .1) {
                            if expr_type.is_always_truthy() {
                                //infinite while loop that only return don't have an exit path
                                let loop_only_ends = control_actions
                                    .iter()
                                    .filter(|action| {
                                        *action != &ControlAction::End
                                            && *action != &ControlAction::Return
                                    })
                                    .count()
                                    == 0;

                                if loop_only_ends {
                                    return control_actions;
                                }
                            }
                        }
                    }

                    if stmt.1.is_for() {
                        let stmt = stmt.1.as_for().unwrap();
                        let mut is_infinite_loop = true;

                        if let Some(for_cond) = stmt.1 {
                            if let Some(expr_type) = types.get_expr_type(&for_cond.1) {
                                if !expr_type.is_always_truthy() {
                                    is_infinite_loop = false
                                }
                            } else {
                                is_infinite_loop = false;
                            }
                        }

                        if is_infinite_loop {
                            let loop_only_ends = control_actions
                                .iter()
                                .filter(|action| {
                                    *action != &ControlAction::End
                                        && *action != &ControlAction::Return
                                })
                                .count()
                                == 0;

                            if loop_only_ends {
                                return control_actions;
                            }
                        }
                    }
                }
            }
            aast::Stmt_::Switch(_) => {
                let mut has_ended = false;
                let mut has_default_terminator = false;

                let switch_stmt = stmt.1.as_switch().unwrap();

                let mut cases = switch_stmt.1.clone();

                cases.reverse();

                let mut switch_break_context = break_context.clone();
                switch_break_context.push(BreakContext::Switch);

                let mut all_case_actions = Vec::new();

                for case in cases {
                    let inner_case_stmts = &case.1;

                    let case_actions = get_control_actions(
                        codebase,
                        resolved_names,
                        inner_case_stmts,
                        tast_info,
                        switch_break_context.clone(),
                        return_is_exit,
                    );

                    if case_actions.contains(&ControlAction::LeaveSwitch)
                        || case_actions.contains(&ControlAction::Break)
                        || case_actions.contains(&ControlAction::Continue)
                    {
                        continue 'outer;
                    }

                    let case_does_end = case_actions
                        .iter()
                        .filter(|action| {
                            *action != &ControlAction::End && *action != &ControlAction::Return
                        })
                        .count()
                        == 0;

                    if case_does_end {
                        has_ended = true;
                    }

                    all_case_actions.extend(case_actions);

                    if !case_does_end && !has_ended {
                        continue 'outer;
                    }
                }

                if let Some(default_case) = switch_stmt.2 {
                    let inner_case_stmts = &default_case.1;

                    let case_actions = get_control_actions(
                        codebase,
                        resolved_names,
                        inner_case_stmts,
                        tast_info,
                        switch_break_context.clone(),
                        return_is_exit,
                    );

                    if case_actions.contains(&ControlAction::LeaveSwitch)
                        || case_actions.contains(&ControlAction::Break)
                        || case_actions.contains(&ControlAction::Continue)
                    {
                        continue 'outer;
                    }

                    let case_does_end = case_actions
                        .iter()
                        .filter(|action| {
                            *action != &ControlAction::End && *action != &ControlAction::Return
                        })
                        .count()
                        == 0;

                    if case_does_end {
                        has_ended = true;
                    }

                    all_case_actions.extend(case_actions);

                    if !case_does_end && !has_ended {
                        continue 'outer;
                    }

                    has_default_terminator = true;
                }

                control_actions.extend(all_case_actions);

                if has_default_terminator
                    || if let Some(tast_info) = tast_info {
                        tast_info
                            .fully_matched_switch_offsets
                            .contains(&stmt.0.start_offset())
                    } else {
                        false
                    }
                {
                    return control_actions;
                }
            }
            aast::Stmt_::Try(_) => {
                let stmt = stmt.1.as_try().unwrap();

                let try_stmt_actions = get_control_actions(
                    codebase,
                    resolved_names,
                    stmt.0,
                    tast_info,
                    break_context.clone(),
                    return_is_exit,
                );

                let try_leaves = try_stmt_actions
                    .iter()
                    .filter(|action| *action == &ControlAction::None)
                    .count()
                    == 0;

                let mut all_catch_actions = Vec::new();

                if stmt.1.len() > 0 {
                    let mut all_catches_leave = try_leaves;

                    for catch in stmt.1 {
                        let catch_actions = get_control_actions(
                            codebase,
                            resolved_names,
                            &catch.2,
                            tast_info,
                            break_context.clone(),
                            return_is_exit,
                        );

                        if all_catches_leave {
                            all_catches_leave = catch_actions
                                .iter()
                                .filter(|action| *action == &ControlAction::None)
                                .count()
                                == 0;
                        }

                        if !all_catches_leave {
                            control_actions.extend(catch_actions);
                        } else {
                            all_catch_actions.extend(catch_actions);
                        }
                    }

                    let mut none_hashset = HashSet::new();
                    none_hashset.insert(ControlAction::None);

                    if all_catches_leave && try_stmt_actions != none_hashset {
                        control_actions.extend(try_stmt_actions);
                        control_actions.extend(all_catch_actions);

                        return control_actions;
                    }
                } else if try_leaves {
                    control_actions.extend(try_stmt_actions);

                    return control_actions;
                }

                if stmt.2.len() > 0 {
                    let finally_actions = get_control_actions(
                        codebase,
                        resolved_names,
                        stmt.2,
                        tast_info,
                        break_context.clone(),
                        return_is_exit,
                    );

                    if !finally_actions.contains(&ControlAction::None) {
                        control_actions.retain(|action| *action != ControlAction::None);
                        control_actions.extend(finally_actions);

                        return control_actions;
                    }
                }

                control_actions.extend(try_stmt_actions);

                control_actions.retain(|action| *action != ControlAction::None);
            }
            aast::Stmt_::Block(block_stmts) => {
                let block_actions = get_control_actions(
                    codebase,
                    resolved_names,
                    block_stmts,
                    tast_info,
                    break_context.clone(),
                    return_is_exit,
                );

                if !block_actions.contains(&ControlAction::None) {
                    control_actions.retain(|action| *action != ControlAction::None);
                    control_actions.extend(block_actions);

                    return control_actions;
                }

                control_actions.extend(block_actions);
            }
            aast::Stmt_::Fallthrough => {}
            aast::Stmt_::YieldBreak => {}
            aast::Stmt_::Awaitall(_) => {}
            aast::Stmt_::Using(_) => {}
            aast::Stmt_::Noop => {}
            aast::Stmt_::Markup(_) => {}
            aast::Stmt_::AssertEnv(_) => {}
        }
    }

    if !control_actions.contains(&ControlAction::None) {
        control_actions.insert(ControlAction::None);
    }

    control_actions
}

fn handle_call(
    call_expr: &Box<(
        aast::Expr<(), ()>,
        Vec<aast::Targ<()>>,
        Vec<(oxidized::ast_defs::ParamKind, aast::Expr<(), ()>)>,
        Option<aast::Expr<(), ()>>,
    )>,
    resolved_names: &HashMap<usize, String>,
    codebase: &CodebaseInfo,
    control_actions: &HashSet<ControlAction>,
) -> Option<HashSet<ControlAction>> {
    match &call_expr.0 .2 {
        aast::Expr_::Id(id) => {
            if id.1.eq("exit") || id.1.eq("die") {
                return Some(control_end(control_actions.clone()));
            }

            if let Some(resolved_name) = resolved_names.get(&id.0.start_offset()) {
                if let Some(functionlike_storage) = codebase.functionlike_infos.get(resolved_name) {
                    if let Some(return_type) = &functionlike_storage.return_type {
                        if return_type.is_nothing() {
                            return Some(control_end(control_actions.clone()));
                        }
                    }
                }
            }
        }
        aast::Expr_::ClassConst(boxed) => {
            match &boxed.0 .2 {
                aast::ClassId_::CIexpr(lhs_expr) => {
                    if let aast::Expr_::Id(id) = &lhs_expr.2 {
                        let mut name_string = &id.1;

                        match name_string.as_str() {
                            "self" | "parent" | "static" => {
                                // do nothing
                            }
                            _ => {
                                if let Some(fq_name) = resolved_names.get(&id.0.start_offset()) {
                                    name_string = fq_name;
                                }

                                if let Some(classlike_storage) =
                                    codebase.classlike_infos.get(name_string)
                                {
                                    if let Some(functionlike_storage) =
                                        classlike_storage.methods.get(&boxed.1 .1)
                                    {
                                        if let Some(return_type) = &functionlike_storage.return_type
                                        {
                                            if return_type.is_nothing() {
                                                return Some(control_end(control_actions.clone()));
                                            }
                                        }
                                    }
                                }
                            }
                        };
                    }
                }
                _ => {}
            }
        }
        _ => (),
    }
    None
}

#[inline]
fn control_end(mut control_actions: HashSet<ControlAction>) -> HashSet<ControlAction> {
    control_actions.insert(ControlAction::End);

    control_actions
}

#[inline]
fn control_return(mut control_actions: HashSet<ControlAction>) -> HashSet<ControlAction> {
    control_actions.insert(ControlAction::Return);

    control_actions
}
