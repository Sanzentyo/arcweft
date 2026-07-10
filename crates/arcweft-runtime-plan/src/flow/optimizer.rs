//! Compiler-side optimization for lowered flow operations.

mod usage;

use self::usage::{count_flow_ops_pure_calls, local_is_unused_after_op, local_uses_in_op};
use super::{RuntimePlanLowerStats, record_projection::rewrite_known_record_projections_in_op};
use arcweft_core::plan::{FlowOp, RuntimePlan};
use arcweft_core::value::{RuntimeExpr, RuntimeSeq, RuntimeValue};
use std::sync::Arc;
pub(super) fn finalize_runtime_plan(
    mut plan: RuntimePlan,
    stats: &mut RuntimePlanLowerStats,
) -> RuntimePlan {
    for flow in &mut plan.flows {
        stats.optimized_flows += 1;
        optimize_flow_ops(&mut flow.ops, stats);
    }
    stats.pure_call_exprs = plan
        .flows
        .iter()
        .map(|flow| count_flow_ops_pure_calls(&flow.ops))
        .sum();
    plan
}

pub(super) fn optimize_flow_ops(ops: &mut Vec<FlowOp>, stats: &mut RuntimePlanLowerStats) {
    stats.optimized_op_slices += 1;
    for op in ops.iter_mut() {
        optimize_nested_flow_ops(op, stats);
    }
    optimize_known_record_projection_lets(ops);
    optimize_local_map_sum_lets(ops, stats);
}

fn optimize_flow_op_slice(ops: &mut [FlowOp], stats: &mut RuntimePlanLowerStats) {
    stats.optimized_op_slices += 1;
    for op in ops {
        optimize_nested_flow_ops(op, stats);
    }
}

fn optimize_nested_flow_ops(op: &mut FlowOp, stats: &mut RuntimePlanLowerStats) {
    match op {
        FlowOp::LetElse { else_ops, .. } => optimize_flow_ops(else_ops, stats),
        FlowOp::If {
            then_ops, else_ops, ..
        }
        | FlowOp::IfLet {
            then_ops, else_ops, ..
        } => {
            optimize_flow_ops(then_ops, stats);
            optimize_flow_ops(else_ops, stats);
        }
        FlowOp::Match { arms, .. } => {
            for arm in arms {
                optimize_flow_ops(&mut arm.ops, stats);
            }
        }
        FlowOp::Loop { body }
        | FlowOp::LetLoop { body, .. }
        | FlowOp::While { body, .. }
        | FlowOp::WhileLet { body, .. }
        | FlowOp::Thread { body, .. }
        | FlowOp::Scope(body)
        | FlowOp::LetScope { ops: body, .. }
        | FlowOp::For { body, .. } => optimize_flow_ops(body, stats),
        FlowOp::LoopNext { body }
        | FlowOp::WhileNext { body, .. }
        | FlowOp::WhileLetNext { body, .. }
        | FlowOp::ForNext { body, .. } => optimize_flow_op_slice(Arc::make_mut(body), stats),
        FlowOp::Bind(_)
        | FlowOp::Let { .. }
        | FlowOp::Dialogue { .. }
        | FlowOp::Choice { .. }
        | FlowOp::Await { .. }
        | FlowOp::AwaitMany { .. }
        | FlowOp::HostCall { .. }
        | FlowOp::Break(_)
        | FlowOp::Continue
        | FlowOp::Goto(_)
        | FlowOp::GotoExpr(_)
        | FlowOp::Return(_)
        | FlowOp::ReturnExpr(_)
        | FlowOp::Effect(_)
        | FlowOp::EvaluatedEffect(_)
        | FlowOp::RegisterCleanup { .. }
        | FlowOp::CancelCleanup { .. }
        | FlowOp::EnterScope
        | FlowOp::ExitScope
        | FlowOp::ExitScopeBind { .. }
        | FlowOp::Noop => {}
    }
}

fn optimize_local_map_sum_lets(ops: &mut Vec<FlowOp>, stats: &mut RuntimePlanLowerStats) {
    let original = std::mem::take(ops);
    let mut index = 0;
    while index < original.len() {
        if let Some(op) = fuse_sequence_map_sum_window(&original, index, stats) {
            stats.sequence_map_sum_fusions += 1;
            ops.push(op);
            index += 3;
            continue;
        }
        if let Some(op) = fuse_map_sum_window(&original, index, stats) {
            stats.map_sum_fusions += 1;
            ops.push(op);
            index += 2;
            continue;
        }
        if let Some(op) = inline_sequence_map_sum_source_window(&original, index, stats) {
            stats.sequence_source_inlines += 1;
            ops.push(op);
            index += 2;
            continue;
        }
        ops.push(original[index].clone());
        index += 1;
    }
}

fn optimize_known_record_projection_lets(ops: &mut [FlowOp]) {
    let mut env = Vec::<(String, Vec<String>)>::new();
    for op in ops {
        rewrite_known_record_projections_in_op(op, &env);
        if let Some(name) = runtime_pattern_binding_name_from_op(op).map(str::to_owned) {
            env.retain(|(candidate, _)| candidate != &name);
            if let FlowOp::Let { expr, .. } = op
                && let Some(fields) = record_projection_fields(expr)
            {
                env.push((name, fields));
            }
        }
    }
}

fn record_projection_fields(expr: &RuntimeExpr) -> Option<Vec<String>> {
    let RuntimeExpr::Value(RuntimeValue::Seq(RuntimeSeq::RecordColumns(records))) = expr else {
        return None;
    };
    Some(
        records
            .fields()
            .iter()
            .map(|field| field.name.clone())
            .collect(),
    )
}

fn fuse_sequence_map_sum_window(
    ops: &[FlowOp],
    index: usize,
    stats: &mut RuntimePlanLowerStats,
) -> Option<FlowOp> {
    let (sequence_name, source_expr) = sequence_let_binding(ops.get(index)?)?;
    let (_, map_expr) = map_let_binding(ops.get(index + 1)?)?;
    let (sum_pattern, sum_source) = local_sum_let_binding(ops.get(index + 2)?)?;
    let (map_source, _) = map_expr_source(map_expr)?;
    if map_source != sequence_name
        || sum_source != runtime_pattern_binding_name_from_op(ops.get(index + 1)?)?
    {
        return None;
    }
    if local_uses_in_op(ops.get(index + 1)?, sequence_name, stats) != 1 {
        return None;
    }
    if local_uses_in_op(ops.get(index + 2)?, sum_source, stats) != 1 {
        return None;
    }
    if !local_is_unused_after_op(ops, index + 1, sequence_name, stats) {
        return None;
    }
    if !local_is_unused_after_op(ops, index + 2, sum_source, stats) {
        return None;
    }
    let mut fused_map = map_expr.clone();
    replace_map_source(&mut fused_map, sequence_name, source_expr)?;
    Some(FlowOp::Let {
        pattern: sum_pattern.clone(),
        expr: RuntimeExpr::Sum {
            source: Box::new(fused_map),
        },
    })
}

fn fuse_map_sum_window(
    ops: &[FlowOp],
    index: usize,
    stats: &mut RuntimePlanLowerStats,
) -> Option<FlowOp> {
    let (sequence_name, map_expr) = map_let_binding(ops.get(index)?)?;
    let (sum_pattern, sum_source) = local_sum_let_binding(ops.get(index + 1)?)?;
    if sequence_name != sum_source
        || !local_is_unused_after_op(ops, index + 1, sequence_name, stats)
    {
        return None;
    }
    if local_uses_in_op(ops.get(index + 1)?, sequence_name, stats) != 1 {
        return None;
    }
    Some(FlowOp::Let {
        pattern: sum_pattern.clone(),
        expr: RuntimeExpr::Sum {
            source: Box::new(map_expr.clone()),
        },
    })
}

fn inline_sequence_map_sum_source_window(
    ops: &[FlowOp],
    index: usize,
    stats: &mut RuntimePlanLowerStats,
) -> Option<FlowOp> {
    let (sequence_name, source_expr) = sequence_let_binding(ops.get(index)?)?;
    if !local_is_unused_after_op(ops, index + 1, sequence_name, stats) {
        return None;
    }
    if local_uses_in_op(ops.get(index + 1)?, sequence_name, stats) != 1 {
        return None;
    }
    let mut next = ops.get(index + 1)?.clone();
    replace_map_sum_source(&mut next, sequence_name, source_expr).then_some(next)
}

fn sequence_let_binding(op: &FlowOp) -> Option<(&str, &RuntimeExpr)> {
    let FlowOp::Let { pattern, expr } = op else {
        return None;
    };
    is_runtime_sequence_expr(expr)
        .then(|| runtime_pattern_binding_name(pattern).map(|name| (name, expr)))?
}

fn is_runtime_sequence_expr(expr: &RuntimeExpr) -> bool {
    matches!(
        expr,
        RuntimeExpr::Value(RuntimeValue::Seq(_) | RuntimeValue::Tuple(_))
            | RuntimeExpr::RepeatSeq { .. }
            | RuntimeExpr::BracketSeq(_)
            | RuntimeExpr::Tuple(_)
    )
}

fn replace_map_sum_source(op: &mut FlowOp, sequence_name: &str, source_expr: &RuntimeExpr) -> bool {
    let FlowOp::Let { expr, .. } = op else {
        return false;
    };
    let RuntimeExpr::Sum { source } = expr else {
        return false;
    };
    let RuntimeExpr::Map { source, .. } = source.as_mut() else {
        return false;
    };
    let RuntimeExpr::Local(name) = source.as_ref() else {
        return false;
    };
    if name != sequence_name {
        return false;
    }
    **source = source_expr.clone();
    true
}

fn map_expr_source(expr: &RuntimeExpr) -> Option<(&str, &RuntimeExpr)> {
    let RuntimeExpr::Map { source, .. } = expr else {
        return None;
    };
    let RuntimeExpr::Local(name) = source.as_ref() else {
        return None;
    };
    Some((name.as_str(), source.as_ref()))
}

fn replace_map_source(
    expr: &mut RuntimeExpr,
    sequence_name: &str,
    source_expr: &RuntimeExpr,
) -> Option<()> {
    let RuntimeExpr::Map { source, .. } = expr else {
        return None;
    };
    let RuntimeExpr::Local(name) = source.as_ref() else {
        return None;
    };
    (name == sequence_name).then(|| {
        **source = source_expr.clone();
    })
}

fn map_let_binding(op: &FlowOp) -> Option<(&str, &RuntimeExpr)> {
    let FlowOp::Let { pattern, expr } = op else {
        return None;
    };
    let RuntimeExpr::Map { .. } = expr else {
        return None;
    };
    runtime_pattern_binding_name(pattern).map(|name| (name, expr))
}

fn local_sum_let_binding(op: &FlowOp) -> Option<(&arcweft_core::pattern::RuntimePattern, &str)> {
    let FlowOp::Let { pattern, expr } = op else {
        return None;
    };
    let RuntimeExpr::Sum { source } = expr else {
        return None;
    };
    match source.as_ref() {
        RuntimeExpr::Local(name) => Some((pattern, name.as_str())),
        _ => None,
    }
}

fn runtime_pattern_binding_name(pattern: &arcweft_core::pattern::RuntimePattern) -> Option<&str> {
    match pattern {
        arcweft_core::pattern::RuntimePattern::Ident(name)
        | arcweft_core::pattern::RuntimePattern::MutIdent(name)
        | arcweft_core::pattern::RuntimePattern::Typed { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

fn runtime_pattern_binding_name_from_op(op: &FlowOp) -> Option<&str> {
    let FlowOp::Let { pattern, .. } = op else {
        return None;
    };
    runtime_pattern_binding_name(pattern)
}
