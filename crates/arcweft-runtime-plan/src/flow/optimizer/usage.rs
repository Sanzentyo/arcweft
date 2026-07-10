//! Runtime-expression use counting for flow optimization.

use super::super::RuntimePlanLowerStats;
use arcweft_core::plan::{FlowOp, RuntimeMatchArm};
use arcweft_core::value::{RuntimeExpr, RuntimeExprMatchArm};
pub(super) fn local_is_unused_after_op(
    ops: &[FlowOp],
    op_index: usize,
    name: &str,
    stats: &mut RuntimePlanLowerStats,
) -> bool {
    stats.local_use_tail_scans += 1;
    ops.iter().skip(op_index + 1).all(|op| {
        stats.local_use_scan_ops += 1;
        count_flow_op_local_uses_by_name(op, name) == 0
    })
}

pub(super) fn local_uses_in_op(
    op: &FlowOp,
    name: &str,
    stats: &mut RuntimePlanLowerStats,
) -> usize {
    stats.local_use_scan_ops += 1;
    count_flow_op_local_uses_by_name(op, name)
}

pub(super) fn count_flow_ops_pure_calls(ops: &[FlowOp]) -> usize {
    ops.iter().map(count_flow_op_pure_calls).sum()
}

fn count_flow_op_pure_calls(op: &FlowOp) -> usize {
    match op {
        FlowOp::LetElse { expr, else_ops, .. } => {
            count_runtime_expr_pure_calls(expr) + count_flow_ops_pure_calls(else_ops)
        }
        FlowOp::If {
            condition,
            then_ops,
            else_ops,
        } => {
            count_runtime_expr_pure_calls(condition)
                + count_flow_ops_pure_calls(then_ops)
                + count_flow_ops_pure_calls(else_ops)
        }
        FlowOp::IfLet {
            expr,
            guard,
            then_ops,
            else_ops,
            ..
        } => {
            count_runtime_expr_pure_calls(expr)
                + guard.as_ref().map_or(0, count_runtime_expr_pure_calls)
                + count_flow_ops_pure_calls(then_ops)
                + count_flow_ops_pure_calls(else_ops)
        }
        FlowOp::Match { scrutinee, arms } => {
            count_runtime_expr_pure_calls(scrutinee)
                + arms
                    .iter()
                    .map(|arm| {
                        arm.guard.as_ref().map_or(0, count_runtime_expr_pure_calls)
                            + count_flow_ops_pure_calls(&arm.ops)
                    })
                    .sum::<usize>()
        }
        FlowOp::Loop { body }
        | FlowOp::LetLoop { body, .. }
        | FlowOp::Thread { body, .. }
        | FlowOp::Scope(body) => count_flow_ops_pure_calls(body),
        FlowOp::LoopNext { body } | FlowOp::ForNext { body, .. } => count_flow_ops_pure_calls(body),
        FlowOp::While { condition, body } => {
            count_runtime_expr_pure_calls(condition) + count_flow_ops_pure_calls(body)
        }
        FlowOp::WhileNext { condition, body } => {
            count_runtime_expr_pure_calls(condition) + count_flow_ops_pure_calls(body)
        }
        FlowOp::WhileLet {
            expr, guard, body, ..
        } => {
            count_runtime_expr_pure_calls(expr)
                + guard.as_ref().map_or(0, count_runtime_expr_pure_calls)
                + count_flow_ops_pure_calls(body)
        }
        FlowOp::WhileLetNext {
            expr, guard, body, ..
        } => {
            count_runtime_expr_pure_calls(expr)
                + guard.as_ref().map_or(0, count_runtime_expr_pure_calls)
                + count_flow_ops_pure_calls(body)
        }
        FlowOp::For { source, body, .. } => {
            count_runtime_expr_pure_calls(source) + count_flow_ops_pure_calls(body)
        }
        FlowOp::AwaitMany { target, .. } => count_runtime_expr_pure_calls(&target.source),
        FlowOp::HostCall { target, .. } => {
            target.args.iter().map(count_runtime_expr_pure_calls).sum()
        }
        FlowOp::EvaluatedEffect(effect) => effect
            .argument_exprs()
            .into_iter()
            .map(count_runtime_expr_pure_calls)
            .sum(),
        FlowOp::LetScope { ops, value, .. } => {
            count_flow_ops_pure_calls(ops) + count_runtime_expr_pure_calls(value)
        }
        FlowOp::Let { expr, .. }
        | FlowOp::Break(Some(expr))
        | FlowOp::GotoExpr(expr)
        | FlowOp::ReturnExpr(expr)
        | FlowOp::ExitScopeBind { expr, .. } => count_runtime_expr_pure_calls(expr),
        FlowOp::Bind(_)
        | FlowOp::Dialogue { .. }
        | FlowOp::Choice { .. }
        | FlowOp::Await { .. }
        | FlowOp::Effect(_)
        | FlowOp::RegisterCleanup { .. }
        | FlowOp::CancelCleanup { .. }
        | FlowOp::EnterScope
        | FlowOp::ExitScope
        | FlowOp::Break(None)
        | FlowOp::Continue
        | FlowOp::Goto(_)
        | FlowOp::Return(_)
        | FlowOp::Noop => 0,
    }
}

fn count_runtime_expr_pure_calls(expr: &RuntimeExpr) -> usize {
    match expr {
        RuntimeExpr::PureCall { args, .. } => {
            1 + args
                .iter()
                .map(count_runtime_expr_pure_calls)
                .sum::<usize>()
        }
        RuntimeExpr::Let { expr, body, .. } => {
            count_runtime_expr_pure_calls(expr) + count_runtime_expr_pure_calls(body)
        }
        RuntimeExpr::AssignField {
            target, expr, body, ..
        } => {
            count_runtime_expr_pure_calls(target)
                + count_runtime_expr_pure_calls(expr)
                + count_runtime_expr_pure_calls(body)
        }
        RuntimeExpr::Tuple(items) | RuntimeExpr::BracketSeq(items) => {
            items.iter().map(count_runtime_expr_pure_calls).sum()
        }
        RuntimeExpr::RepeatSeq { value, .. } => count_runtime_expr_pure_calls(value),
        RuntimeExpr::Range { start, end, .. } => {
            start.as_deref().map_or(0, count_runtime_expr_pure_calls)
                + end.as_deref().map_or(0, count_runtime_expr_pure_calls)
        }
        RuntimeExpr::Record(fields) => fields
            .iter()
            .map(|field| count_runtime_expr_pure_calls(&field.value))
            .sum(),
        RuntimeExpr::Variant { payload, .. } => {
            payload.as_deref().map_or(0, count_runtime_expr_pure_calls)
        }
        RuntimeExpr::Field { target, .. }
        | RuntimeExpr::ProjectTuple { target, .. }
        | RuntimeExpr::ProjectRecord { target, .. }
        | RuntimeExpr::SpreadArg(target) => count_runtime_expr_pure_calls(target),
        RuntimeExpr::Call { args, .. } => args.iter().map(count_runtime_expr_pure_calls).sum(),
        RuntimeExpr::Function { body, .. } => count_runtime_expr_pure_calls(body),
        RuntimeExpr::Apply { callee, args } => {
            count_runtime_expr_pure_calls(callee)
                + args
                    .iter()
                    .map(count_runtime_expr_pure_calls)
                    .sum::<usize>()
        }
        RuntimeExpr::MethodCall { receiver, args, .. }
        | RuntimeExpr::TraitCall { receiver, args, .. } => {
            count_runtime_expr_pure_calls(receiver)
                + args
                    .iter()
                    .map(count_runtime_expr_pure_calls)
                    .sum::<usize>()
        }
        RuntimeExpr::Map { source, body, .. } | RuntimeExpr::Filter { source, body, .. } => {
            count_runtime_expr_pure_calls(source) + count_runtime_expr_pure_calls(body)
        }
        RuntimeExpr::Sum { source } | RuntimeExpr::Unary { expr: source, .. } => {
            count_runtime_expr_pure_calls(source)
        }
        RuntimeExpr::Binary { lhs, rhs, .. } => {
            count_runtime_expr_pure_calls(lhs) + count_runtime_expr_pure_calls(rhs)
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            count_runtime_expr_pure_calls(condition)
                + count_runtime_expr_pure_calls(then_expr)
                + count_runtime_expr_pure_calls(else_expr)
        }
        RuntimeExpr::IfLet {
            expr,
            guard,
            then_expr,
            else_expr,
            ..
        } => {
            count_runtime_expr_pure_calls(expr)
                + guard.as_deref().map_or(0, count_runtime_expr_pure_calls)
                + count_runtime_expr_pure_calls(then_expr)
                + count_runtime_expr_pure_calls(else_expr)
        }
        RuntimeExpr::Match { scrutinee, arms } => {
            count_runtime_expr_pure_calls(scrutinee)
                + arms
                    .iter()
                    .map(|arm| {
                        arm.guard.as_ref().map_or(0, count_runtime_expr_pure_calls)
                            + count_runtime_expr_pure_calls(&arm.value)
                    })
                    .sum::<usize>()
        }
        RuntimeExpr::Value(_) | RuntimeExpr::Local(_) | RuntimeExpr::EntityRef(_) => 0,
    }
}

fn count_flow_ops_local_uses_by_name(ops: &[FlowOp], name: &str) -> usize {
    ops.iter()
        .map(|op| count_flow_op_local_uses_by_name(op, name))
        .sum()
}

fn count_flow_op_local_uses_by_name(op: &FlowOp, name: &str) -> usize {
    match op {
        FlowOp::LetElse { expr, else_ops, .. } => {
            count_runtime_expr_local_uses_by_name(expr, name)
                + count_flow_ops_local_uses_by_name(else_ops, name)
        }
        FlowOp::If {
            condition,
            then_ops,
            else_ops,
        } => {
            count_runtime_expr_local_uses_by_name(condition, name)
                + count_flow_ops_local_uses_by_name(then_ops, name)
                + count_flow_ops_local_uses_by_name(else_ops, name)
        }
        FlowOp::IfLet {
            expr,
            guard,
            then_ops,
            else_ops,
            ..
        } => {
            count_runtime_expr_local_uses_by_name(expr, name)
                + count_optional_runtime_expr_local_uses_by_name(guard.as_ref(), name)
                + count_flow_ops_local_uses_by_name(then_ops, name)
                + count_flow_ops_local_uses_by_name(else_ops, name)
        }
        FlowOp::Match { scrutinee, arms } => {
            count_runtime_expr_local_uses_by_name(scrutinee, name)
                + count_match_arms_local_uses_by_name(arms, name)
        }
        FlowOp::Loop { body }
        | FlowOp::LetLoop { body, .. }
        | FlowOp::Thread { body, .. }
        | FlowOp::Scope(body) => count_flow_ops_local_uses_by_name(body, name),
        FlowOp::LoopNext { body } | FlowOp::ForNext { body, .. } => {
            count_flow_ops_local_uses_by_name(body, name)
        }
        FlowOp::While { condition, body } => {
            count_runtime_expr_local_uses_by_name(condition, name)
                + count_flow_ops_local_uses_by_name(body, name)
        }
        FlowOp::WhileNext { condition, body } => {
            count_runtime_expr_local_uses_by_name(condition, name)
                + count_flow_ops_local_uses_by_name(body, name)
        }
        FlowOp::WhileLet {
            expr, guard, body, ..
        } => {
            count_runtime_expr_local_uses_by_name(expr, name)
                + count_optional_runtime_expr_local_uses_by_name(guard.as_ref(), name)
                + count_flow_ops_local_uses_by_name(body, name)
        }
        FlowOp::WhileLetNext {
            expr, guard, body, ..
        } => {
            count_runtime_expr_local_uses_by_name(expr, name)
                + count_optional_runtime_expr_local_uses_by_name(guard.as_ref(), name)
                + count_flow_ops_local_uses_by_name(body, name)
        }
        FlowOp::For { source, body, .. } => {
            count_runtime_expr_local_uses_by_name(source, name)
                + count_flow_ops_local_uses_by_name(body, name)
        }
        FlowOp::AwaitMany { target, .. } => {
            count_runtime_expr_local_uses_by_name(&target.source, name)
        }
        FlowOp::HostCall { target, .. } => target
            .args
            .iter()
            .map(|arg| count_runtime_expr_local_uses_by_name(arg, name))
            .sum(),
        FlowOp::EvaluatedEffect(effect) => effect
            .argument_exprs()
            .into_iter()
            .map(|arg| count_runtime_expr_local_uses_by_name(arg, name))
            .sum(),
        FlowOp::LetScope { ops, value, .. } => {
            count_flow_ops_local_uses_by_name(ops, name)
                + count_runtime_expr_local_uses_by_name(value, name)
        }
        FlowOp::Let { expr, .. }
        | FlowOp::Break(Some(expr))
        | FlowOp::GotoExpr(expr)
        | FlowOp::ReturnExpr(expr)
        | FlowOp::ExitScopeBind { expr, .. } => count_runtime_expr_local_uses_by_name(expr, name),
        FlowOp::Bind(_)
        | FlowOp::Dialogue { .. }
        | FlowOp::Choice { .. }
        | FlowOp::Await { .. }
        | FlowOp::Effect(_)
        | FlowOp::RegisterCleanup { .. }
        | FlowOp::CancelCleanup { .. }
        | FlowOp::EnterScope
        | FlowOp::ExitScope
        | FlowOp::Break(None)
        | FlowOp::Continue
        | FlowOp::Goto(_)
        | FlowOp::Return(_)
        | FlowOp::Noop => 0,
    }
}

fn count_match_arms_local_uses_by_name(arms: &[RuntimeMatchArm], name: &str) -> usize {
    arms.iter()
        .map(|arm| {
            count_optional_runtime_expr_local_uses_by_name(arm.guard.as_ref(), name)
                + count_flow_ops_local_uses_by_name(&arm.ops, name)
        })
        .sum()
}

fn count_runtime_expr_local_uses_by_name(expr: &RuntimeExpr, name: &str) -> usize {
    match expr {
        RuntimeExpr::Local(local) => usize::from(local == name),
        RuntimeExpr::Let { expr, body, .. } => {
            count_runtime_expr_local_uses_by_name(expr, name)
                + count_runtime_expr_local_uses_by_name(body, name)
        }
        RuntimeExpr::AssignField {
            target, expr, body, ..
        } => {
            count_runtime_expr_local_uses_by_name(target, name)
                + count_runtime_expr_local_uses_by_name(expr, name)
                + count_runtime_expr_local_uses_by_name(body, name)
        }
        RuntimeExpr::Tuple(items) | RuntimeExpr::BracketSeq(items) => items
            .iter()
            .map(|item| count_runtime_expr_local_uses_by_name(item, name))
            .sum(),
        RuntimeExpr::RepeatSeq { value, .. } => count_runtime_expr_local_uses_by_name(value, name),
        RuntimeExpr::Range { start, end, .. } => {
            count_optional_runtime_expr_local_uses_by_name(start.as_deref(), name)
                + count_optional_runtime_expr_local_uses_by_name(end.as_deref(), name)
        }
        RuntimeExpr::Record(fields) => fields
            .iter()
            .map(|field| count_runtime_expr_local_uses_by_name(&field.value, name))
            .sum(),
        RuntimeExpr::Variant { payload, .. } => payload.as_deref().map_or(0, |payload| {
            count_runtime_expr_local_uses_by_name(payload, name)
        }),
        RuntimeExpr::Field { target, .. }
        | RuntimeExpr::ProjectTuple { target, .. }
        | RuntimeExpr::ProjectRecord { target, .. }
        | RuntimeExpr::SpreadArg(target) => count_runtime_expr_local_uses_by_name(target, name),
        RuntimeExpr::Call { args, .. } | RuntimeExpr::PureCall { args, .. } => args
            .iter()
            .map(|arg| count_runtime_expr_local_uses_by_name(arg, name))
            .sum(),
        RuntimeExpr::Function { params, body } => {
            count_runtime_function_local_uses_by_name(params, body, name)
        }
        RuntimeExpr::Apply { callee, args } => {
            count_runtime_apply_local_uses_by_name(callee, args, name)
        }
        RuntimeExpr::MethodCall { receiver, args, .. }
        | RuntimeExpr::TraitCall { receiver, args, .. } => {
            count_runtime_expr_local_uses_by_name(receiver, name)
                + args
                    .iter()
                    .map(|arg| count_runtime_expr_local_uses_by_name(arg, name))
                    .sum::<usize>()
        }
        RuntimeExpr::Map {
            source,
            param,
            body,
        }
        | RuntimeExpr::Filter {
            source,
            param,
            body,
        } => count_runtime_scoped_body_local_uses_by_name(source, param, body, name),
        RuntimeExpr::Sum { source } | RuntimeExpr::Unary { expr: source, .. } => {
            count_runtime_expr_local_uses_by_name(source, name)
        }
        RuntimeExpr::Binary { lhs, rhs, .. } => {
            count_runtime_expr_local_uses_by_name(lhs, name)
                + count_runtime_expr_local_uses_by_name(rhs, name)
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            count_runtime_expr_local_uses_by_name(condition, name)
                + count_runtime_expr_local_uses_by_name(then_expr, name)
                + count_runtime_expr_local_uses_by_name(else_expr, name)
        }
        RuntimeExpr::IfLet {
            expr,
            guard,
            then_expr,
            else_expr,
            ..
        } => count_runtime_if_let_local_uses_by_name(
            expr,
            guard.as_deref(),
            then_expr,
            else_expr,
            name,
        ),
        RuntimeExpr::Match { scrutinee, arms } => {
            count_runtime_match_local_uses_by_name(scrutinee, arms, name)
        }
        RuntimeExpr::Value(_) | RuntimeExpr::EntityRef(_) => 0,
    }
}

fn count_runtime_function_local_uses_by_name(
    params: &[String],
    body: &RuntimeExpr,
    name: &str,
) -> usize {
    if params.iter().any(|param| param == name) {
        0
    } else {
        count_runtime_expr_local_uses_by_name(body, name)
    }
}

fn count_runtime_apply_local_uses_by_name(
    callee: &RuntimeExpr,
    args: &[RuntimeExpr],
    name: &str,
) -> usize {
    count_runtime_expr_local_uses_by_name(callee, name)
        + args
            .iter()
            .map(|arg| count_runtime_expr_local_uses_by_name(arg, name))
            .sum::<usize>()
}

fn count_runtime_scoped_body_local_uses_by_name(
    source: &RuntimeExpr,
    param: &str,
    body: &RuntimeExpr,
    name: &str,
) -> usize {
    count_runtime_expr_local_uses_by_name(source, name)
        + if param == name {
            0
        } else {
            count_runtime_expr_local_uses_by_name(body, name)
        }
}

fn count_runtime_if_let_local_uses_by_name(
    expr: &RuntimeExpr,
    guard: Option<&RuntimeExpr>,
    then_expr: &RuntimeExpr,
    else_expr: &RuntimeExpr,
    name: &str,
) -> usize {
    count_runtime_expr_local_uses_by_name(expr, name)
        + count_optional_runtime_expr_local_uses_by_name(guard, name)
        + count_runtime_expr_local_uses_by_name(then_expr, name)
        + count_runtime_expr_local_uses_by_name(else_expr, name)
}

fn count_runtime_match_local_uses_by_name(
    scrutinee: &RuntimeExpr,
    arms: &[RuntimeExprMatchArm],
    name: &str,
) -> usize {
    count_runtime_expr_local_uses_by_name(scrutinee, name)
        + arms
            .iter()
            .map(|arm| {
                count_optional_runtime_expr_local_uses_by_name(arm.guard.as_ref(), name)
                    + count_runtime_expr_local_uses_by_name(&arm.value, name)
            })
            .sum::<usize>()
}

fn count_optional_runtime_expr_local_uses_by_name(expr: Option<&RuntimeExpr>, name: &str) -> usize {
    expr.map_or(0, |expr| count_runtime_expr_local_uses_by_name(expr, name))
}
