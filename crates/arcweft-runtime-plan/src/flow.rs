//! Flow-runtime lowering.

use crate::errors::{LinePlanLowerError, RuntimePlanLowerError};
use crate::expr::{lower_runtime_expr, lower_runtime_expr_strict_with_pure, runtime_call_effect};
use crate::host_request::lower_host_task_request;
use crate::labels::expr_label;
use crate::line_task::{lower_line_plan, lower_line_plan_statements};
use crate::pattern::lower_runtime_pattern;
use crate::pure::{PureHelperCandidate, lower_pure_helper_candidates};
use crate::source::lower_source_plan;
use crate::stream::lower_stream_function;
use arcweft_core::effect::LineEffectRequest;
use arcweft_core::line_task::{LineOutRequest, LineTaskGroup};
use arcweft_core::plan::{
    ChoiceRuntimeOption, EntryRuntimeId, FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec,
    RuntimeEntryTarget, RuntimeFlow, RuntimeLineId, RuntimeMatchArm, RuntimePlan,
    RuntimePureHelper, RuntimePureHelperId, RuntimePureInputType, RuntimeRouteBinding,
    RuntimeRouteBindingSource, RuntimeRouteSpec,
};
use arcweft_core::source::{SourceHandlerPlan, SourceOp, SourcePlan};
use arcweft_core::stream::{StreamMatchArm, StreamOp, StreamPlan};
use arcweft_core::task::{
    AWAIT_MANY_ITEM_BINDING, AwaitManyTarget, AwaitTarget, HostTaskArgTemplate,
    HostTaskRequestTemplate, NeedId, TaskId,
};
use arcweft_core::value::{RuntimeExpr, RuntimeValue};
use arcweft_lang_hir::model::{
    HirAwait, HirChoice, HirChoiceOption, HirDialogue, HirFlow, HirFlowItem, HirLoop, HirMatch,
    HirModule, HirScopeExpr, HirThread, HirTopLevelDecl,
};
use arcweft_lang_hir::syntax::ast::{
    choice::ChoiceAction,
    flow::{AwaitBranchKind, FlowItem, Stmt, StmtMatchArm, ThreadBlock},
    ids::{EntityRef, EntityRefSyntax},
    items::{EntryItem, EntryKind, FunctionKind},
    pattern::Pattern,
};
use arcweft_lang_hir::syntax::expr::Expr;
use std::{collections::BTreeMap, sync::Arc};

pub(crate) struct LoweredRuntimeFlows {
    pub(crate) flows: Vec<RuntimeFlow>,
    pub(crate) line_task_groups: Vec<LineTaskGroup>,
}

/// Lowers checked HIR flows to the Sans I/O core runtime program.
///
/// This pass is intentionally stricter than line-task-only lowering: it must
/// not silently skip flow syntax because the engine would otherwise execute a
/// different story than the source describes.
pub fn lower_runtime_plan(module: &HirModule) -> Result<RuntimePlan, Vec<RuntimePlanLowerError>> {
    let pure_candidates = lower_pure_helper_candidates(module).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| RuntimePlanLowerError::new(error.to_string()))
            .collect::<Vec<_>>()
    })?;
    let pure_helpers = runtime_pure_helpers(&pure_candidates);
    let pure_map = pure_helper_map(&pure_helpers);
    let lowered_flows = lower_runtime_flows(module, &pure_map)?;
    let entries = lower_runtime_entries(module);
    let entry = implicit_entry_flow(&entries, &lowered_flows.flows);
    let stream_plans = module
        .functions()
        .iter()
        .filter(|function| function.kind() == FunctionKind::Stream)
        .map(lower_stream_function)
        .collect::<Vec<_>>();
    let source_plans = module
        .declarations()
        .iter()
        .filter_map(|decl| match decl {
            HirTopLevelDecl::Source(source) => Some(lower_source_plan(source)),
            _ => None,
        })
        .collect::<Result<Vec<_>, _>>()?;
    RuntimePlan::new(entry, lowered_flows.flows, lowered_flows.line_task_groups)
        .map(|plan| {
            rewrite_runtime_plan_pure_calls(
                plan.with_entries(entries)
                    .with_generation_plans(stream_plans, source_plans)
                    .with_pure_helpers(pure_helpers),
                &pure_map,
            )
        })
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])
}

fn runtime_pure_helpers(candidates: &[PureHelperCandidate]) -> Vec<RuntimePureHelper> {
    candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| RuntimePureHelper {
            id: RuntimePureHelperId(index),
            name: candidate.name().to_owned(),
            input_names: candidate.input_names().to_vec(),
            input_types: vec![RuntimePureInputType::I64; candidate.input_names().len()],
            expr: candidate.expr().clone(),
            scalar_eval_supported: candidate.expr().supports_scalar_pure_eval(),
            origin: candidate.origin(),
        })
        .collect()
}

fn pure_helper_map(helpers: &[RuntimePureHelper]) -> BTreeMap<String, RuntimePureHelperId> {
    helpers
        .iter()
        .map(|helper| (helper.name.clone(), helper.id))
        .collect()
}

fn rewrite_runtime_plan_pure_calls(
    mut plan: RuntimePlan,
    helpers: &BTreeMap<String, RuntimePureHelperId>,
) -> RuntimePlan {
    for flow in &mut plan.flows {
        optimize_flow_ops(&mut flow.ops);
    }
    for source in &mut plan.source_plans {
        rewrite_source_plan_pure_calls(source, helpers);
    }
    for stream in &mut plan.stream_plans {
        rewrite_stream_plan_pure_calls(stream, helpers);
    }
    plan
}

fn optimize_flow_ops(ops: &mut Vec<FlowOp>) {
    for op in ops.iter_mut() {
        optimize_nested_flow_ops(op);
    }
    optimize_local_map_sum_lets(ops);
}

fn optimize_flow_op_slice(ops: &mut [FlowOp]) {
    for op in ops {
        optimize_nested_flow_ops(op);
    }
}

fn optimize_nested_flow_ops(op: &mut FlowOp) {
    match op {
        FlowOp::LetElse { else_ops, .. } => optimize_flow_ops(else_ops),
        FlowOp::If {
            then_ops, else_ops, ..
        }
        | FlowOp::IfLet {
            then_ops, else_ops, ..
        } => {
            optimize_flow_ops(then_ops);
            optimize_flow_ops(else_ops);
        }
        FlowOp::Match { arms, .. } => {
            for arm in arms {
                optimize_flow_ops(&mut arm.ops);
            }
        }
        FlowOp::Loop { body }
        | FlowOp::LetLoop { body, .. }
        | FlowOp::While { body, .. }
        | FlowOp::WhileLet { body, .. }
        | FlowOp::Thread { body, .. }
        | FlowOp::Scope(body)
        | FlowOp::LetScope { ops: body, .. }
        | FlowOp::For { body, .. } => optimize_flow_ops(body),
        FlowOp::LoopNext { body }
        | FlowOp::WhileNext { body, .. }
        | FlowOp::WhileLetNext { body, .. }
        | FlowOp::ForNext { body, .. } => optimize_flow_op_slice(Arc::make_mut(body)),
        FlowOp::Bind(_)
        | FlowOp::Let { .. }
        | FlowOp::Dialogue { .. }
        | FlowOp::Choice { .. }
        | FlowOp::Await { .. }
        | FlowOp::AwaitMany { .. }
        | FlowOp::Break(_)
        | FlowOp::Continue
        | FlowOp::Goto(_)
        | FlowOp::GotoExpr(_)
        | FlowOp::Return(_)
        | FlowOp::ReturnExpr(_)
        | FlowOp::Effect(_)
        | FlowOp::EnterScope
        | FlowOp::ExitScope
        | FlowOp::ExitScopeBind { .. }
        | FlowOp::Noop => {}
    }
}

fn optimize_local_map_sum_lets(ops: &mut Vec<FlowOp>) {
    let original = std::mem::take(ops);
    let suffix_uses = flow_op_suffix_local_uses(&original);
    let mut index = 0;
    while index < original.len() {
        if let Some(op) = fuse_sequence_map_sum_window(&original, &suffix_uses, index) {
            ops.push(op);
            index += 3;
            continue;
        }
        if let Some(op) = fuse_map_sum_window(&original, &suffix_uses, index) {
            ops.push(op);
            index += 2;
            continue;
        }
        if let Some(op) = inline_sequence_map_sum_source_window(&original, &suffix_uses, index) {
            ops.push(op);
            index += 2;
            continue;
        }
        ops.push(original[index].clone());
        index += 1;
    }
}

fn fuse_sequence_map_sum_window(
    ops: &[FlowOp],
    suffix_uses: &[BTreeMap<&str, usize>],
    index: usize,
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
    if local_uses_in_op(ops.get(index + 1)?, sequence_name) != 1 {
        return None;
    }
    if local_uses_in_op(ops.get(index + 2)?, sum_source) != 1 {
        return None;
    }
    if local_uses_after(suffix_uses, index + 1, sequence_name) != 0 {
        return None;
    }
    if local_uses_after(suffix_uses, index + 2, sum_source) != 0 {
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
    suffix_uses: &[BTreeMap<&str, usize>],
    index: usize,
) -> Option<FlowOp> {
    let (sequence_name, map_expr) = map_let_binding(ops.get(index)?)?;
    let (sum_pattern, sum_source) = local_sum_let_binding(ops.get(index + 1)?)?;
    if sequence_name != sum_source || local_uses_after(suffix_uses, index + 1, sequence_name) != 0 {
        return None;
    }
    if local_uses_in_op(ops.get(index + 1)?, sequence_name) != 1 {
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
    suffix_uses: &[BTreeMap<&str, usize>],
    index: usize,
) -> Option<FlowOp> {
    let (sequence_name, source_expr) = sequence_let_binding(ops.get(index)?)?;
    if local_uses_after(suffix_uses, index + 1, sequence_name) != 0 {
        return None;
    }
    if local_uses_in_op(ops.get(index + 1)?, sequence_name) != 1 {
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

fn flow_op_suffix_local_uses(ops: &[FlowOp]) -> Vec<BTreeMap<&str, usize>> {
    let mut suffix = vec![BTreeMap::new(); ops.len() + 1];
    for index in (0..ops.len()).rev() {
        suffix[index] = suffix[index + 1].clone();
        count_flow_op_local_uses(&ops[index], &mut suffix[index]);
    }
    suffix
}

fn local_uses_after(suffix_uses: &[BTreeMap<&str, usize>], op_index: usize, name: &str) -> usize {
    suffix_uses
        .get(op_index + 1)
        .and_then(|uses| uses.get(name))
        .copied()
        .unwrap_or_default()
}

fn local_uses_in_op(op: &FlowOp, name: &str) -> usize {
    let mut uses = BTreeMap::new();
    count_flow_op_local_uses(op, &mut uses);
    uses.get(name).copied().unwrap_or_default()
}

fn count_flow_ops_local_uses<'a>(ops: &'a [FlowOp], uses: &mut BTreeMap<&'a str, usize>) {
    for op in ops {
        count_flow_op_local_uses(op, uses);
    }
}

fn count_flow_op_local_uses<'a>(op: &'a FlowOp, uses: &mut BTreeMap<&'a str, usize>) {
    match op {
        FlowOp::LetElse { expr, else_ops, .. } => {
            count_runtime_expr_local_uses(expr, uses);
            count_flow_ops_local_uses(else_ops, uses);
        }
        FlowOp::If {
            condition,
            then_ops,
            else_ops,
        } => {
            count_runtime_expr_local_uses(condition, uses);
            count_flow_ops_local_uses(then_ops, uses);
            count_flow_ops_local_uses(else_ops, uses);
        }
        FlowOp::IfLet {
            expr,
            guard,
            then_ops,
            else_ops,
            ..
        } => {
            count_runtime_expr_local_uses(expr, uses);
            if let Some(guard) = guard {
                count_runtime_expr_local_uses(guard, uses);
            }
            count_flow_ops_local_uses(then_ops, uses);
            count_flow_ops_local_uses(else_ops, uses);
        }
        FlowOp::Match { scrutinee, arms } => {
            count_runtime_expr_local_uses(scrutinee, uses);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    count_runtime_expr_local_uses(guard, uses);
                }
                count_flow_ops_local_uses(&arm.ops, uses);
            }
        }
        FlowOp::Loop { body }
        | FlowOp::LetLoop { body, .. }
        | FlowOp::Thread { body, .. }
        | FlowOp::Scope(body) => count_flow_ops_local_uses(body, uses),
        FlowOp::LoopNext { body } | FlowOp::ForNext { body, .. } => {
            count_flow_ops_local_uses(body, uses);
        }
        FlowOp::While { condition, body } => {
            count_runtime_expr_local_uses(condition, uses);
            count_flow_ops_local_uses(body, uses);
        }
        FlowOp::WhileNext { condition, body } => {
            count_runtime_expr_local_uses(condition, uses);
            count_flow_ops_local_uses(body, uses);
        }
        FlowOp::WhileLet {
            expr, guard, body, ..
        } => {
            count_runtime_expr_local_uses(expr, uses);
            if let Some(guard) = guard {
                count_runtime_expr_local_uses(guard, uses);
            }
            count_flow_ops_local_uses(body, uses);
        }
        FlowOp::WhileLetNext {
            expr, guard, body, ..
        } => {
            count_runtime_expr_local_uses(expr, uses);
            if let Some(guard) = guard {
                count_runtime_expr_local_uses(guard, uses);
            }
            count_flow_ops_local_uses(body, uses);
        }
        FlowOp::For { source, body, .. } => {
            count_runtime_expr_local_uses(source, uses);
            count_flow_ops_local_uses(body, uses);
        }
        FlowOp::AwaitMany { target, .. } => count_runtime_expr_local_uses(&target.source, uses),
        FlowOp::LetScope { ops, value, .. } => {
            count_flow_ops_local_uses(ops, uses);
            count_runtime_expr_local_uses(value, uses);
        }
        FlowOp::Let { expr, .. }
        | FlowOp::Break(Some(expr))
        | FlowOp::GotoExpr(expr)
        | FlowOp::ReturnExpr(expr)
        | FlowOp::ExitScopeBind { expr, .. } => count_runtime_expr_local_uses(expr, uses),
        FlowOp::Bind(_)
        | FlowOp::Dialogue { .. }
        | FlowOp::Choice { .. }
        | FlowOp::Await { .. }
        | FlowOp::Effect(_)
        | FlowOp::EnterScope
        | FlowOp::ExitScope
        | FlowOp::Break(None)
        | FlowOp::Continue
        | FlowOp::Goto(_)
        | FlowOp::Return(_)
        | FlowOp::Noop => {}
    }
}

fn count_runtime_expr_local_uses<'a>(expr: &'a RuntimeExpr, uses: &mut BTreeMap<&'a str, usize>) {
    match expr {
        RuntimeExpr::Local(local) => {
            *uses.entry(local.as_str()).or_default() += 1;
        }
        RuntimeExpr::Let { expr, body, .. } => {
            count_runtime_expr_local_uses(expr, uses);
            count_runtime_expr_local_uses(body, uses);
        }
        RuntimeExpr::Tuple(items) | RuntimeExpr::BracketSeq(items) => {
            for item in items {
                count_runtime_expr_local_uses(item, uses);
            }
        }
        RuntimeExpr::RepeatSeq { value, .. } => count_runtime_expr_local_uses(value, uses),
        RuntimeExpr::Record(fields) => {
            for field in fields {
                count_runtime_expr_local_uses(&field.value, uses);
            }
        }
        RuntimeExpr::Variant { payload, .. } => {
            if let Some(payload) = payload {
                count_runtime_expr_local_uses(payload, uses);
            }
        }
        RuntimeExpr::Field { target, .. } | RuntimeExpr::SpreadArg(target) => {
            count_runtime_expr_local_uses(target, uses);
        }
        RuntimeExpr::Call { args, .. } | RuntimeExpr::PureCall { args, .. } => {
            for arg in args {
                count_runtime_expr_local_uses(arg, uses);
            }
        }
        RuntimeExpr::MethodCall { receiver, args, .. } => {
            count_runtime_expr_local_uses(receiver, uses);
            for arg in args {
                count_runtime_expr_local_uses(arg, uses);
            }
        }
        RuntimeExpr::Map { source, body, .. } => {
            count_runtime_expr_local_uses(source, uses);
            count_runtime_expr_local_uses(body, uses);
        }
        RuntimeExpr::Sum { source } | RuntimeExpr::Unary { expr: source, .. } => {
            count_runtime_expr_local_uses(source, uses);
        }
        RuntimeExpr::Binary { lhs, rhs, .. } => {
            count_runtime_expr_local_uses(lhs, uses);
            count_runtime_expr_local_uses(rhs, uses);
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            count_runtime_expr_local_uses(condition, uses);
            count_runtime_expr_local_uses(then_expr, uses);
            count_runtime_expr_local_uses(else_expr, uses);
        }
        RuntimeExpr::IfLet {
            expr,
            guard,
            then_expr,
            else_expr,
            ..
        } => {
            count_runtime_expr_local_uses(expr, uses);
            if let Some(guard) = guard {
                count_runtime_expr_local_uses(guard, uses);
            }
            count_runtime_expr_local_uses(then_expr, uses);
            count_runtime_expr_local_uses(else_expr, uses);
        }
        RuntimeExpr::Match { scrutinee, arms } => {
            count_runtime_expr_local_uses(scrutinee, uses);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    count_runtime_expr_local_uses(guard, uses);
                }
                count_runtime_expr_local_uses(&arm.value, uses);
            }
        }
        RuntimeExpr::Value(_) | RuntimeExpr::EntityRef(_) => {}
    }
}

fn lower_runtime_entries(module: &HirModule) -> Vec<RuntimeEntrySpec> {
    module
        .declarations()
        .iter()
        .filter_map(|decl| match decl {
            HirTopLevelDecl::Entry(entry) => Some(RuntimeEntrySpec {
                id: EntryRuntimeId(entry.id().body().to_owned()),
                kind: lower_entry_kind(entry.kind()),
                target: lower_entry_target(entry.items()),
            }),
            _ => None,
        })
        .collect()
}

fn lower_entry_kind(kind: &EntryKind) -> RuntimeEntryKind {
    match kind {
        EntryKind::Game => RuntimeEntryKind::Game,
        EntryKind::Cli => RuntimeEntryKind::Cli,
        EntryKind::Server => RuntimeEntryKind::Server,
        EntryKind::Activity => RuntimeEntryKind::Activity,
        EntryKind::Test => RuntimeEntryKind::Test,
        EntryKind::Bench => RuntimeEntryKind::Bench,
        EntryKind::Custom(value) => RuntimeEntryKind::Custom(value.clone()),
    }
}

fn lower_entry_target(items: &[EntryItem]) -> RuntimeEntryTarget {
    let routes = items
        .iter()
        .filter_map(|item| match item {
            EntryItem::Route {
                method,
                path,
                target,
                bindings,
            } => Some(RuntimeRouteSpec {
                method: method.clone(),
                path: path.clone(),
                target: flow_runtime_id(target),
                bindings: bindings
                    .iter()
                    .map(|binding| RuntimeRouteBinding {
                        name: binding.name().to_owned(),
                        source: match binding.source() {
                            arcweft_lang_hir::syntax::ast::items::EntryRouteBindingSource::PathParam(name) => {
                                RuntimeRouteBindingSource::PathParam(name.clone())
                            }
                        },
                    })
                    .collect(),
            }),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !routes.is_empty() {
        return RuntimeEntryTarget::Routes(routes);
    }
    items
        .iter()
        .find_map(|item| match item {
            EntryItem::Start(target) | EntryItem::Run(target) => {
                Some(RuntimeEntryTarget::Flow(flow_runtime_id(target)))
            }
            EntryItem::Route { .. } | EntryItem::Option { .. } | EntryItem::Raw(_) => None,
        })
        .unwrap_or_else(|| RuntimeEntryTarget::Routes(Vec::new()))
}

fn implicit_entry_flow(
    entries: &[RuntimeEntrySpec],
    flows: &[RuntimeFlow],
) -> Option<FlowRuntimeId> {
    if entries.len() == 1
        && let Some(flow) = match &entries[0].target {
            RuntimeEntryTarget::Flow(flow) => Some(flow),
            RuntimeEntryTarget::Routes(routes) => routes.first().map(|route| &route.target),
        }
    {
        return Some(flow.clone());
    }
    if entries.is_empty() {
        return flows.first().map(|flow| flow.id.clone());
    }
    None
}

/// Lowers HIR flow bodies into executable Sans I/O flow operations.
pub(crate) fn lower_runtime_flows(
    module: &HirModule,
    pure_helpers: &BTreeMap<String, RuntimePureHelperId>,
) -> Result<LoweredRuntimeFlows, Vec<RuntimePlanLowerError>> {
    let mut lowerer = FlowRuntimeLowerer {
        line_task_groups: Vec::new(),
        errors: Vec::new(),
        pure_helpers,
    };
    let flows = module
        .flows()
        .iter()
        .enumerate()
        .map(|(index, flow)| lowerer.lower_flow(index, flow))
        .collect::<Vec<_>>();
    if !module.top_level_items().is_empty() {
        lowerer.errors.push(RuntimePlanLowerError::new(
            "top-level flow items are not executable by the flow runtime yet",
        ));
    }
    if lowerer.errors.is_empty() {
        Ok(LoweredRuntimeFlows {
            flows,
            line_task_groups: lowerer.line_task_groups,
        })
    } else {
        Err(lowerer.errors)
    }
}

struct FlowRuntimeLowerer<'a> {
    line_task_groups: Vec<LineTaskGroup>,
    errors: Vec<RuntimePlanLowerError>,
    pure_helpers: &'a BTreeMap<String, RuntimePureHelperId>,
}

impl FlowRuntimeLowerer<'_> {
    fn lower_runtime_expr(&mut self, expr: &Expr) -> RuntimeExpr {
        match lower_runtime_expr_strict_with_pure(expr, self.pure_helpers) {
            Ok(expr) => expr,
            Err(message) => {
                self.errors.push(RuntimePlanLowerError::new(message));
                RuntimeExpr::Value(RuntimeValue::Unit)
            }
        }
    }

    fn lower_optional_runtime_expr(&mut self, expr: Option<&Expr>) -> Option<RuntimeExpr> {
        expr.map(|expr| self.lower_runtime_expr(expr))
    }

    fn lower_flow(&mut self, index: usize, flow: &HirFlow) -> RuntimeFlow {
        let id = flow.id().map_or_else(
            || FlowRuntimeId(format!("flow.{}", flow.name().unwrap_or("anonymous"))),
            flow_runtime_id,
        );
        let ops = self.lower_flow_items(&id, flow.body(), index);
        RuntimeFlow { id, ops }
    }

    fn lower_flow_items(
        &mut self,
        flow_id: &FlowRuntimeId,
        items: &[HirFlowItem],
        flow_index: usize,
    ) -> Vec<FlowOp> {
        let mut ops = Vec::new();
        for item in items {
            match item {
                HirFlowItem::Dialogue(dialogue) => {
                    ops.push(self.lower_runtime_dialogue(flow_id, flow_index, dialogue));
                }
                HirFlowItem::Choice(choice) | HirFlowItem::LetChoice { choice, .. } => {
                    ops.push(self.lower_choice(choice));
                }
                HirFlowItem::Await(await_with) => {
                    ops.push(self.lower_await(None, await_with));
                }
                HirFlowItem::LetAwait {
                    pattern,
                    await_with,
                    ..
                } => {
                    ops.push(self.lower_await(Some(pattern), await_with));
                }
                HirFlowItem::Stmt(stmt) => {
                    ops.extend(self.lower_flow_stmt(stmt));
                }
                HirFlowItem::Thread(thread) => {
                    ops.extend(self.lower_hir_thread(thread, flow_id, flow_index));
                }
                HirFlowItem::Scope(scope) => {
                    ops.push(FlowOp::Scope(self.lower_flow_items(
                        flow_id,
                        scope.body(),
                        flow_index,
                    )));
                }
                HirFlowItem::LetScope { pattern, scope } => {
                    ops.push(self.lower_scope_expr(pattern, scope));
                }
                HirFlowItem::If(block) => {
                    ops.push(FlowOp::If {
                        condition: self.lower_runtime_expr(block.condition()),
                        then_ops: self.lower_flow_items(flow_id, block.body(), flow_index),
                        else_ops: self.lower_flow_items(flow_id, block.else_body(), flow_index),
                    });
                }
                HirFlowItem::IfLet(block) => {
                    ops.push(FlowOp::IfLet {
                        pattern: lower_runtime_pattern(block.pattern()),
                        expr: self.lower_runtime_expr(block.expr()),
                        guard: self.lower_optional_runtime_expr(block.guard()),
                        then_ops: self.lower_flow_items(flow_id, block.body(), flow_index),
                        else_ops: self.lower_flow_items(flow_id, block.else_body(), flow_index),
                    });
                }
                HirFlowItem::Match(block) => {
                    ops.push(self.lower_match_block(flow_id, block, flow_index));
                }
                HirFlowItem::Loop(block) => {
                    ops.push(FlowOp::Loop {
                        body: self.lower_flow_items(flow_id, block.body(), flow_index),
                    });
                }
                HirFlowItem::LetLoop { pattern, block } => {
                    ops.push(self.lower_loop_expr(flow_id, pattern, block, flow_index));
                }
                HirFlowItem::While(block) => {
                    ops.push(FlowOp::While {
                        condition: self.lower_runtime_expr(block.condition()),
                        body: self.lower_flow_items(flow_id, block.body(), flow_index),
                    });
                }
                HirFlowItem::WhileLet(block) => {
                    ops.push(FlowOp::WhileLet {
                        pattern: lower_runtime_pattern(block.pattern()),
                        expr: self.lower_runtime_expr(block.expr()),
                        guard: self.lower_optional_runtime_expr(block.guard()),
                        body: self.lower_flow_items(flow_id, block.body(), flow_index),
                    });
                }
                HirFlowItem::For(block) => {
                    ops.push(FlowOp::For {
                        pattern: lower_runtime_pattern(block.pattern()),
                        source: self.lower_runtime_expr(block.source()),
                        body: self.lower_flow_items(flow_id, block.body(), flow_index),
                    });
                }
                other => {
                    self.errors.push(RuntimePlanLowerError::new(format!(
                        "unsupported flow item for runtime lowering: {other:?}"
                    )));
                }
            }
        }
        ops
    }

    fn lower_scope_expr(&mut self, pattern: &Pattern, scope: &HirScopeExpr) -> FlowOp {
        FlowOp::LetScope {
            pattern: lower_runtime_pattern(pattern),
            ops: self.lower_flow_stmt_list(scope.statements()),
            value: scope
                .value()
                .map_or(RuntimeExpr::Value(RuntimeValue::Unit), |value| {
                    self.lower_runtime_expr(value)
                }),
        }
    }

    fn lower_match_block(
        &mut self,
        flow_id: &FlowRuntimeId,
        block: &HirMatch,
        flow_index: usize,
    ) -> FlowOp {
        FlowOp::Match {
            scrutinee: self.lower_runtime_expr(block.expr()),
            arms: block
                .arms()
                .iter()
                .map(|arm| RuntimeMatchArm {
                    pattern: lower_runtime_pattern(arm.pattern()),
                    guard: self.lower_optional_runtime_expr(arm.guard()),
                    ops: self.lower_flow_items(flow_id, arm.body(), flow_index),
                })
                .collect(),
        }
    }

    fn lower_loop_expr(
        &mut self,
        flow_id: &FlowRuntimeId,
        pattern: &Pattern,
        block: &HirLoop,
        flow_index: usize,
    ) -> FlowOp {
        FlowOp::LetLoop {
            pattern: lower_runtime_pattern(pattern),
            body: self.lower_flow_items(flow_id, block.body(), flow_index),
        }
    }

    fn lower_runtime_dialogue(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        dialogue: &HirDialogue,
    ) -> FlowOp {
        let group = if let Some(plan) = dialogue.plan() {
            match lower_line_plan(plan) {
                Ok(group) => group,
                Err(errors) => {
                    self.push_line_errors(errors);
                    LineTaskGroup::default()
                }
            }
        } else {
            LineTaskGroup::default()
        };
        let task_group = self.line_task_groups.len();
        self.line_task_groups.push(group);
        let line = dialogue.id().map_or_else(
            || RuntimeLineId(format!("{}.line.{task_group}", flow_id.0)),
            |id| RuntimeLineId(id.body().to_owned()),
        );
        let _ = flow_index;
        FlowOp::Dialogue { line, task_group }
    }

    fn lower_choice(&mut self, choice: &HirChoice) -> FlowOp {
        FlowOp::Choice {
            id: choice.id().map(|id| id.body().to_owned()),
            options: choice
                .options()
                .iter()
                .map(|option| self.lower_choice_option(option))
                .collect(),
        }
    }

    fn lower_choice_option(&mut self, option: &HirChoiceOption) -> ChoiceRuntimeOption {
        let mut effects = Vec::new();
        let mut out = None;
        let mut target = option.target().map(flow_runtime_id);
        match option.action() {
            ChoiceAction::Goto(target_ref) => {
                if let EntityRefSyntax::Absolute(target_ref) = target_ref {
                    target = Some(flow_runtime_id(target_ref));
                }
            }
            ChoiceAction::Out(expr) => {
                out = Some(LineOutRequest {
                    label: None,
                    value: expr_label(expr),
                });
            }
            ChoiceAction::SelectBlock(statements) => {
                effects.extend(self.lower_flow_statements(statements));
            }
            ChoiceAction::None => {}
        }
        ChoiceRuntimeOption {
            id: option.id().map(|id| id.body().to_owned()),
            label: option.label().to_owned(),
            target,
            out,
            effects,
        }
    }

    fn lower_await(&mut self, binding: Option<&Pattern>, await_with: &HirAwait) -> FlowOp {
        let label = expr_label(await_with.expr());
        let task_name = sanitize_task_id_part(&label);
        let pending = await_with
            .branches()
            .iter()
            .filter(|branch| branch.kind() == AwaitBranchKind::Pending)
            .flat_map(|branch| self.lower_pending_flow_items(branch.body()))
            .collect();
        match self.lower_await_many_target(await_with.expr(), &task_name) {
            Ok(Some(target)) => {
                return FlowOp::AwaitMany {
                    binding: binding.map(lower_runtime_pattern),
                    target,
                    pending,
                };
            }
            Ok(None) => {}
            Err(message) => {
                self.errors.push(RuntimePlanLowerError::new(message));
                return FlowOp::Noop;
            }
        }
        FlowOp::Await {
            binding: binding.map(lower_runtime_pattern),
            target: AwaitTarget::new(
                NeedId(format!("need.await.{task_name}")),
                TaskId(format!("task.await.{task_name}")),
                lower_host_task_request(await_with.expr()),
            ),
            pending,
        }
    }

    fn lower_await_many_target(
        &mut self,
        expr: &Expr,
        task_name: &str,
    ) -> Result<Option<AwaitManyTarget>, String> {
        let Expr::MethodCall {
            receiver: parallel_receiver,
            method,
            args: parallel_args,
        } = expr
        else {
            return Ok(None);
        };
        if method_name(method) != "parallel" {
            return Ok(None);
        }
        let Expr::MethodCall {
            receiver: source,
            method: traverse_method,
            args: traverse_args,
        } = parallel_receiver.as_ref()
        else {
            return Err(
                "parallel await requires `source.traverse(call).parallel(limit = N)`".to_owned(),
            );
        };
        if method_name(traverse_method) != "traverse" {
            return Err(
                "parallel await requires `traverse(...)` before `.parallel(...)`".to_owned(),
            );
        }
        let callee = traverse_callee(traverse_args)?;
        let (capability, operation) = split_capability_operation(&expr_label(callee))?;
        let limit = parallel_limit(parallel_args)?;
        Ok(Some(AwaitManyTarget::new(
            NeedId(format!("need.await_many.{task_name}")),
            TaskId(format!("task.await_many.{task_name}")),
            self.lower_runtime_expr(source),
            AWAIT_MANY_ITEM_BINDING,
            limit,
            HostTaskRequestTemplate::new(
                capability,
                operation,
                [HostTaskArgTemplate::positional(RuntimeExpr::Local(
                    AWAIT_MANY_ITEM_BINDING.to_owned(),
                ))],
            ),
        )))
    }

    fn lower_pending_flow_items(&mut self, items: &[HirFlowItem]) -> Vec<LineEffectRequest> {
        items
            .iter()
            .flat_map(|item| match item {
                HirFlowItem::Stmt(stmt) => self.lower_flow_statements(std::slice::from_ref(stmt)),
                other => {
                    self.errors.push(RuntimePlanLowerError::new(format!(
                        "unsupported await pending item for runtime lowering: {other:?}"
                    )));
                    Vec::new()
                }
            })
            .collect()
    }

    fn lower_flow_stmt(&mut self, stmt: &Stmt) -> Vec<FlowOp> {
        match stmt {
            Stmt::Let { pattern, expr, .. } => vec![FlowOp::Let {
                pattern: lower_runtime_pattern(pattern),
                expr: self.lower_runtime_expr(expr),
            }],
            Stmt::LetScope { pattern, scope } => vec![FlowOp::LetScope {
                pattern: lower_runtime_pattern(pattern),
                ops: self.lower_flow_stmt_list(scope.statements()),
                value: scope
                    .value()
                    .map_or(RuntimeExpr::Value(RuntimeValue::Unit), |value| {
                        self.lower_runtime_expr(value)
                    }),
            }],
            Stmt::LetLoop { pattern, block } => vec![FlowOp::LetLoop {
                pattern: lower_runtime_pattern(pattern),
                body: self.lower_syntax_flow_items(block.body()),
            }],
            Stmt::LetElse {
                pattern,
                expr,
                else_body,
                ..
            } => vec![FlowOp::LetElse {
                pattern: lower_runtime_pattern(pattern),
                expr: self.lower_runtime_expr(expr),
                else_ops: self.lower_flow_stmt_list(else_body),
            }],
            Stmt::Goto(expr) => vec![FlowOp::GotoExpr(self.lower_runtime_expr(expr))],
            Stmt::Return(expr) => vec![FlowOp::ReturnExpr(
                lower_runtime_expr_strict_with_pure(expr, self.pure_helpers)
                    .unwrap_or_else(|_| lower_runtime_expr(expr)),
            )],
            Stmt::Expr(expr) => vec![FlowOp::Effect(runtime_call_effect(expr))],
            Stmt::Out { label, expr } => {
                vec![FlowOp::Effect(LineEffectRequest::Out(LineOutRequest {
                    label: label.clone(),
                    value: expr_label(expr),
                }))]
            }
            Stmt::If { condition, body } => vec![FlowOp::If {
                condition: self.lower_runtime_expr(condition),
                then_ops: self.lower_flow_stmt_list(body),
                else_ops: Vec::new(),
            }],
            Stmt::Loop { body } => vec![FlowOp::Loop {
                body: self.lower_flow_stmt_list(body),
            }],
            Stmt::While { condition, body } => vec![FlowOp::While {
                condition: self.lower_runtime_expr(condition),
                body: self.lower_flow_stmt_list(body),
            }],
            Stmt::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => vec![FlowOp::WhileLet {
                pattern: lower_runtime_pattern(pattern),
                expr: self.lower_runtime_expr(expr),
                guard: self.lower_optional_runtime_expr(guard.as_ref()),
                body: self.lower_flow_stmt_list(body),
            }],
            Stmt::For {
                pattern,
                source,
                body,
            } => vec![FlowOp::For {
                pattern: lower_runtime_pattern(pattern),
                source: self.lower_runtime_expr(source),
                body: self.lower_flow_stmt_list(body),
            }],
            Stmt::Thread(thread) => self.lower_thread_stmt(thread),
            Stmt::Match { expr, arms } => vec![FlowOp::Match {
                scrutinee: self.lower_runtime_expr(expr),
                arms: self.lower_stmt_match_arms(arms),
            }],
            Stmt::Break { expr, .. } => {
                vec![FlowOp::Break(
                    self.lower_optional_runtime_expr(expr.as_ref()),
                )]
            }
            Stmt::Continue { .. } => vec![FlowOp::Continue],
            other => {
                self.errors.push(RuntimePlanLowerError::new(format!(
                    "unsupported flow statement for runtime lowering: {other:?}"
                )));
                Vec::new()
            }
        }
    }

    fn lower_stmt_match_arms(&mut self, arms: &[StmtMatchArm]) -> Vec<RuntimeMatchArm> {
        arms.iter()
            .map(|arm| RuntimeMatchArm {
                pattern: lower_runtime_pattern(arm.pattern()),
                guard: self.lower_optional_runtime_expr(arm.guard()),
                ops: self.lower_flow_stmt_list(arm.body()),
            })
            .collect()
    }

    fn lower_thread_stmt(&mut self, thread: &ThreadBlock) -> Vec<FlowOp> {
        if thread.is_detached() {
            self.errors.push(RuntimePlanLowerError::new(
                "detached flow thread runtime lowering requires a checked detach contract"
                    .to_owned(),
            ));
            Vec::new()
        } else {
            vec![FlowOp::Thread {
                name: thread.name().map(str::to_owned),
                body: self.lower_syntax_flow_items(thread.body()),
            }]
        }
    }

    fn lower_hir_thread(
        &mut self,
        thread: &HirThread,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
    ) -> Vec<FlowOp> {
        if thread.is_detached() {
            self.errors.push(RuntimePlanLowerError::new(
                "detached flow thread runtime lowering requires a checked detach contract"
                    .to_owned(),
            ));
            Vec::new()
        } else {
            vec![FlowOp::Thread {
                name: thread.name().map(str::to_owned),
                body: self.lower_flow_items(flow_id, thread.body(), flow_index),
            }]
        }
    }

    fn lower_flow_stmt_list(&mut self, statements: &[Stmt]) -> Vec<FlowOp> {
        statements
            .iter()
            .flat_map(|statement| self.lower_flow_stmt(statement))
            .collect()
    }

    fn lower_syntax_flow_items(&mut self, items: &[FlowItem]) -> Vec<FlowOp> {
        items
            .iter()
            .flat_map(|item| match item {
                FlowItem::Stmt(statement) => self.lower_flow_stmt(statement),
                other => {
                    self.errors.push(RuntimePlanLowerError::new(format!(
                        "unsupported nested flow item for runtime lowering: {other:?}"
                    )));
                    Vec::new()
                }
            })
            .collect()
    }

    fn lower_flow_statements(&mut self, statements: &[Stmt]) -> Vec<LineEffectRequest> {
        let (effects, errors) = lower_line_plan_statements(statements);
        self.push_line_errors(errors);
        effects
    }

    fn push_line_errors(&mut self, errors: Vec<LinePlanLowerError>) {
        self.errors.extend(
            errors
                .into_iter()
                .map(|error| RuntimePlanLowerError::new(error.message().to_owned())),
        );
    }
}

pub(crate) fn sanitize_task_id_part(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn flow_runtime_id(id: &EntityRef) -> FlowRuntimeId {
    FlowRuntimeId(id.body().to_owned())
}

fn method_name(method: &str) -> &str {
    method.split_once('<').map_or(method, |(name, _)| name)
}

fn traverse_callee(args: &[arcweft_lang_hir::syntax::expr::CallArg]) -> Result<&Expr, String> {
    let [arg] = args else {
        return Err("traverse(...) requires exactly one positional task function".to_owned());
    };
    if arg.name().is_some() || arg.is_spread() {
        return Err("traverse(...) task function must be a positional argument".to_owned());
    }
    Ok(arg.value())
}

fn split_capability_operation(name: &str) -> Result<(String, String), String> {
    name.rsplit_once('.').map_or_else(
        || {
            Err(format!(
                "traverse task function `{name}` must be capability-qualified"
            ))
        },
        |(capability, operation)| Ok((capability.to_owned(), operation.to_owned())),
    )
}

fn parallel_limit(args: &[arcweft_lang_hir::syntax::expr::CallArg]) -> Result<usize, String> {
    let [arg] = args else {
        return Err("parallel(...) requires exactly `limit = N`".to_owned());
    };
    if arg.name() != Some("limit") || arg.is_spread() {
        return Err("parallel(...) requires a named `limit = N` argument".to_owned());
    }
    let Expr::Literal(arcweft_lang_hir::syntax::expr::Literal::Int { value, .. }) = arg.value()
    else {
        return Err("parallel limit must be an integer literal".to_owned());
    };
    usize::try_from(*value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| "parallel limit must be greater than zero".to_owned())
}

fn rewrite_source_plan_pure_calls(
    source: &mut SourcePlan,
    helpers: &BTreeMap<String, RuntimePureHelperId>,
) {
    rewrite_expr_pure_calls(&mut source.from, helpers);
    for handler in &mut source.handlers {
        let ops = match handler {
            SourceHandlerPlan::Item { ops, .. }
            | SourceHandlerPlan::Error { ops, .. }
            | SourceHandlerPlan::Progress { ops, .. }
            | SourceHandlerPlan::Disconnected { ops }
            | SourceHandlerPlan::PermissionRevoked { ops }
            | SourceHandlerPlan::End { ops } => ops,
        };
        rewrite_source_ops_pure_calls(ops, helpers);
    }
}

fn rewrite_source_ops_pure_calls(
    ops: &mut [SourceOp],
    helpers: &BTreeMap<String, RuntimePureHelperId>,
) {
    for op in ops {
        if let SourceOp::Yield(expr) = op {
            rewrite_expr_pure_calls(expr, helpers);
        }
    }
}

fn rewrite_stream_plan_pure_calls(
    stream: &mut StreamPlan,
    helpers: &BTreeMap<String, RuntimePureHelperId>,
) {
    rewrite_stream_ops_pure_calls(&mut stream.ops, helpers);
}

fn rewrite_stream_ops_pure_calls(
    ops: &mut [StreamOp],
    helpers: &BTreeMap<String, RuntimePureHelperId>,
) {
    for op in ops {
        match op {
            StreamOp::Let { expr, .. }
            | StreamOp::Yield { expr }
            | StreamOp::Close { source: expr } => rewrite_expr_pure_calls(expr, helpers),
            StreamOp::ForNext { source, body, .. } => {
                rewrite_expr_pure_calls(source, helpers);
                rewrite_stream_ops_pure_calls(body, helpers);
            }
            StreamOp::If {
                condition,
                then_ops,
                else_ops,
            } => {
                rewrite_expr_pure_calls(condition, helpers);
                rewrite_stream_ops_pure_calls(then_ops, helpers);
                rewrite_stream_ops_pure_calls(else_ops, helpers);
            }
            StreamOp::Match { scrutinee, arms } => {
                rewrite_expr_pure_calls(scrutinee, helpers);
                for arm in arms {
                    rewrite_stream_match_arm_pure_calls(arm, helpers);
                }
            }
            StreamOp::Return | StreamOp::Noop => {}
        }
    }
}

fn rewrite_stream_match_arm_pure_calls(
    arm: &mut StreamMatchArm,
    helpers: &BTreeMap<String, RuntimePureHelperId>,
) {
    if let Some(guard) = &mut arm.guard {
        rewrite_expr_pure_calls(guard, helpers);
    }
    rewrite_stream_ops_pure_calls(&mut arm.ops, helpers);
}

fn rewrite_expr_pure_calls(
    expr: &mut RuntimeExpr,
    helpers: &BTreeMap<String, RuntimePureHelperId>,
) {
    match expr {
        RuntimeExpr::Call { callee, args } => {
            for arg in args.iter_mut() {
                rewrite_expr_pure_calls(arg, helpers);
            }
            if let Some(helper) = helpers.get(callee).copied() {
                *expr = RuntimeExpr::PureCall {
                    helper,
                    args: std::mem::take(args),
                };
            }
        }
        RuntimeExpr::PureCall { args, .. }
        | RuntimeExpr::Tuple(args)
        | RuntimeExpr::BracketSeq(args) => {
            for arg in args {
                rewrite_expr_pure_calls(arg, helpers);
            }
        }
        RuntimeExpr::RepeatSeq { value, .. } => rewrite_expr_pure_calls(value, helpers),
        RuntimeExpr::Let {
            expr: value, body, ..
        } => {
            rewrite_expr_pure_calls(value, helpers);
            rewrite_expr_pure_calls(body, helpers);
        }
        RuntimeExpr::Record(fields) => {
            for field in fields {
                rewrite_expr_pure_calls(&mut field.value, helpers);
            }
        }
        RuntimeExpr::Variant { payload, .. } => {
            if let Some(payload) = payload {
                rewrite_expr_pure_calls(payload, helpers);
            }
        }
        RuntimeExpr::Field { target, .. } | RuntimeExpr::SpreadArg(target) => {
            rewrite_expr_pure_calls(target, helpers);
        }
        RuntimeExpr::MethodCall { receiver, args, .. } => {
            rewrite_expr_pure_calls(receiver, helpers);
            for arg in args {
                rewrite_expr_pure_calls(arg, helpers);
            }
        }
        RuntimeExpr::Map { source, body, .. } => {
            rewrite_expr_pure_calls(source, helpers);
            rewrite_expr_pure_calls(body, helpers);
        }
        RuntimeExpr::Sum { source } => rewrite_expr_pure_calls(source, helpers),
        RuntimeExpr::Unary { expr, .. } => rewrite_expr_pure_calls(expr, helpers),
        RuntimeExpr::Binary { lhs, rhs, .. } => {
            rewrite_expr_pure_calls(lhs, helpers);
            rewrite_expr_pure_calls(rhs, helpers);
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            rewrite_expr_pure_calls(condition, helpers);
            rewrite_expr_pure_calls(then_expr, helpers);
            rewrite_expr_pure_calls(else_expr, helpers);
        }
        RuntimeExpr::IfLet {
            expr,
            guard,
            then_expr,
            else_expr,
            ..
        } => {
            rewrite_expr_pure_calls(expr, helpers);
            if let Some(guard) = guard {
                rewrite_expr_pure_calls(guard, helpers);
            }
            rewrite_expr_pure_calls(then_expr, helpers);
            rewrite_expr_pure_calls(else_expr, helpers);
        }
        RuntimeExpr::Match { scrutinee, arms } => {
            rewrite_expr_pure_calls(scrutinee, helpers);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    rewrite_expr_pure_calls(guard, helpers);
                }
                rewrite_expr_pure_calls(&mut arm.value, helpers);
            }
        }
        RuntimeExpr::Value(_) | RuntimeExpr::Local(_) | RuntimeExpr::EntityRef(_) => {}
    }
}
