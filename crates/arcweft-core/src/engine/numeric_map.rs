//! Optional whole-span numeric execution of the admitted loop continuation.
//! Selection borrows the actual Core operations, callable transitions and all
//! materialization rows. Unsupported or under-budget spans remain ordinary ops.
use super::{Engine, FlowFiberStatus};
use crate::pattern::{
    RuntimeBuiltinVariantCaseIdentity, RuntimeBuiltinVariantIdentity, RuntimePattern,
    RuntimePatternKind,
};
use crate::plan::{FlowOp, RuntimePlan, RuntimePlanTypeProjection};
use crate::pure::{
    RuntimeCallBackend, RuntimeNumericMapBatchData, RuntimeNumericMapBatchResult,
    project_numeric_map,
};
use crate::runtime_id::{RuntimeLocalDeclarationId as Local, RuntimePlanTypeId as Ty};
use crate::step::{RuntimeStepMode, RuntimeStepOutput};
use crate::value::{
    RuntimeCallArgumentMode, RuntimeExpr, RuntimeExprKind, RuntimeIntrinsic, RuntimeIterator,
    RuntimeLocalReadMode, RuntimeMutablePlace, RuntimePlaceDisplacement,
    RuntimePlaceInitialization, RuntimeValue,
};
use std::sync::Arc;

struct LoopProjection<'a> {
    iterator: Local,
    callable: Local,
    result: &'a RuntimeMutablePlace,
    item_ty: Ty,
    output_ty: Ty,
}
#[derive(Clone, Copy)]
enum SumTarget<'a> {
    Bind(&'a RuntimePattern),
    Return,
}
enum OwnedSumTarget {
    Bind(RuntimePattern),
    Return,
}
struct SumProjection<'a> {
    skipped_ops: usize,
    aliases: Vec<Local>,
    target: SumTarget<'a>,
    elided: Vec<Local>,
}
struct ArrayProjection<'a> {
    callable: Local,
    items: Vec<Local>,
    results: Vec<&'a RuntimePattern>,
    item_ty: Ty,
    output_ty: Ty,
    aggregate: Option<Local>,
}
enum CommitSource {
    Loop {
        iterator: Local,
        accumulator: RuntimeMutablePlace,
    },
    Array {
        items: Vec<Local>,
        results: Vec<RuntimePattern>,
    },
}
struct Commit {
    source: CommitSource,
    consumed_ops: usize,
    sum: Option<OwnedSumTarget>,
    cost: usize,
}
fn local(expression: &RuntimeExpr, mode: RuntimeLocalReadMode) -> Option<Local> {
    let RuntimeExprKind::Local(read) = expression.kind() else {
        return None;
    };
    (read.mode() == mode && read.fields().is_empty()).then_some(read.local())
}
fn binder(pattern: &RuntimePattern) -> Option<Local> {
    match pattern.kind() {
        RuntimePatternKind::Bind { binding, .. } | RuntimePatternKind::Typed { binding } => {
            Some(binding.local())
        }
        _ => None,
    }
}
fn project_loop<'a>(plan: &RuntimePlan, op: &'a FlowOp) -> Option<LoopProjection<'a>> {
    let FlowOp::WhileLet {
        pattern,
        expr,
        guard: None,
        body,
    } = op
    else {
        return None;
    };
    let RuntimeExprKind::Call { callee, args } = expr.kind() else {
        return None;
    };
    if callee.as_intrinsic() != Some(RuntimeIntrinsic::CoreIterNext) {
        return None;
    }
    let [argument] = args.as_slice() else {
        return None;
    };
    if argument.mode() != RuntimeCallArgumentMode::Value || argument.abi_position() != 0 {
        return None;
    }
    let iterator = local(argument.value(), RuntimeLocalReadMode::Move)?;
    let RuntimePatternKind::Tuple(patterns) = pattern.kind() else {
        return None;
    };
    let [next, option] = patterns.as_ref() else {
        return None;
    };
    let next_iterator = binder(next)?;
    let RuntimePlanTypeProjection::Option {
        item: item_ty,
        some_payload,
    } = plan.type_table().get(option.ty())?.projection()
    else {
        return None;
    };
    let RuntimePatternKind::Variant {
        ordinal,
        payload: Some(payload),
    } = option.kind()
    else {
        return None;
    };
    if RuntimeBuiltinVariantIdentity::Option
        .cases()
        .get(*ordinal as usize)?
        .identity()
        != RuntimeBuiltinVariantCaseIdentity::OptionSome
        || payload.ty() != *some_payload
    {
        return None;
    }
    let RuntimePatternKind::Tuple(items) = payload.kind() else {
        return None;
    };
    let [item] = items.as_ref() else { return None };
    if item.ty() != *item_ty || next.ty() != argument.value().ty() || pattern.ty() != expr.ty() {
        return None;
    }
    let item_local = binder(item)?;
    let [
        FlowOp::Assign { place, value },
        FlowOp::ApplyGroup {
            callee,
            args,
            result,
        },
        FlowOp::Let {
            pattern: discard,
            expr: push,
        },
    ] = body.as_slice()
    else {
        return None;
    };
    if place.place().local() != iterator
        || !place.place().fields().is_empty()
        || !matches!(place.displacement(), RuntimePlaceDisplacement::Reachable {
            initialization: RuntimePlaceInitialization::Uninitialized, fields
        } if fields.is_empty())
        || local(value, RuntimeLocalReadMode::Move) != Some(next_iterator)
        || value.ty() != next.ty()
    {
        return None;
    }
    let callable = local(callee, RuntimeLocalReadMode::Copy)?;
    let [argument] = args.as_slice() else {
        return None;
    };
    if argument.mode() != RuntimeCallArgumentMode::Value
        || argument.abi_position() != 0
        || local(argument.value(), RuntimeLocalReadMode::Move) != Some(item_local)
        || argument.value().ty() != *item_ty
    {
        return None;
    }
    let mapped = binder(result)?;
    let RuntimeExprKind::SequencePush {
        place: accumulator,
        value,
    } = push.kind()
    else {
        return None;
    };
    if !accumulator.fields().is_empty()
        || local(value, RuntimeLocalReadMode::Move) != Some(mapped)
        || value.ty() != result.ty()
        || discard.ty() != push.ty()
        || !matches!(discard.kind(), RuntimePatternKind::Discard)
        || !matches!(
            plan.type_table().get(discard.ty())?.projection(),
            RuntimePlanTypeProjection::Unit
        )
    {
        return None;
    }
    let accumulator_ty = plan.local_declarations().get(accumulator.local())?.ty();
    let RuntimePlanTypeProjection::Sequence { item: output, .. } =
        plan.type_table().get(accumulator_ty)?.projection()
    else {
        return None;
    };
    if *output != result.ty() {
        return None;
    }
    let unique = [
        iterator,
        next_iterator,
        item_local,
        callable,
        mapped,
        accumulator.local(),
    ]
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    if unique.len() != 6 {
        return None;
    }
    Some(LoopProjection {
        iterator,
        callable,
        result: accumulator,
        item_ty: *item_ty,
        output_ty: *output,
    })
}
fn project_array<'a>(plan: &RuntimePlan, ops: &[&'a FlowOp]) -> Option<ArrayProjection<'a>> {
    let mut callable = None;
    let mut callable_ty = None;
    let mut item_ty = None;
    let mut output_ty = None;
    let mut items = Vec::new();
    let mut results = Vec::new();
    let mut distinct = std::collections::BTreeSet::new();
    for op in ops {
        let FlowOp::ApplyGroup {
            callee,
            args,
            result,
        } = op
        else {
            break;
        };
        let owner = local(callee, RuntimeLocalReadMode::Copy)?;
        let [argument] = args.as_slice() else {
            return None;
        };
        if argument.mode() != RuntimeCallArgumentMode::Value || argument.abi_position() != 0 {
            return None;
        }
        let item = local(argument.value(), RuntimeLocalReadMode::Move)?;
        let result_local = binder(result)?;
        if callable.is_some_and(|first| first != owner)
            || callable_ty.is_some_and(|first| first != callee.ty())
            || item_ty.is_some_and(|first| first != argument.value().ty())
            || output_ty.is_some_and(|first| first != result.ty())
            || owner == item
            || owner == result_local
            || !distinct.insert(item)
            || !distinct.insert(result_local)
            || plan.local_declarations().get(item)?.ty() != argument.value().ty()
            || plan.local_declarations().get(result_local)?.ty() != result.ty()
        {
            return None;
        }
        callable = Some(owner);
        callable_ty = Some(callee.ty());
        item_ty = Some(argument.value().ty());
        output_ty = Some(result.ty());
        items.push(item);
        results.push(result);
    }
    let count = items.len();
    if count == 0 {
        return None;
    }
    let (aggregate, value) = match *ops.get(count)? {
        FlowOp::Let { pattern, expr } if pattern.ty() == expr.ty() => (binder(pattern), expr),
        FlowOp::ReturnExpr(expr) => (None, expr),
        _ => return None,
    };
    let RuntimeExprKind::BracketSeq(values) = value.kind() else {
        return None;
    };
    let RuntimePlanTypeProjection::Array { item, length } =
        plan.type_table().get(value.ty())?.projection()
    else {
        return None;
    };
    if length
        .constant()
        .and_then(|length| usize::try_from(length).ok())
        != Some(count)
        || Some(*item) != output_ty
        || values.len() != count
        || aggregate.is_some_and(|local| distinct.contains(&local) || Some(local) == callable)
    {
        return None;
    }
    for (value, result) in values.iter().zip(&results) {
        if value.ty() != *item || local(value, RuntimeLocalReadMode::Move) != binder(result) {
            return None;
        }
    }
    Some(ArrayProjection {
        callable: callable?,
        items,
        results,
        item_ty: item_ty?,
        output_ty: output_ty?,
        aggregate,
    })
}
fn project_sum<'a>(
    plan: &RuntimePlan,
    ops: &[&'a FlowOp],
    accumulator: Local,
) -> Option<SumProjection<'a>> {
    let mut source = accumulator;
    let mut aliases = Vec::new();
    let sequence_ty = plan.local_declarations().get(source)?.ty();
    for (index, op) in ops.iter().enumerate() {
        let (pattern, expr) = match *op {
            FlowOp::Let { pattern, expr } if pattern.ty() == expr.ty() => (Some(pattern), expr),
            FlowOp::ReturnExpr(expr) => (None, expr),
            _ => return None,
        };
        if let RuntimeExprKind::Sum { source: sum_source } = expr.kind() {
            if ![RuntimeLocalReadMode::Move, RuntimeLocalReadMode::Copy]
                .into_iter()
                .any(|mode| local(sum_source, mode) == Some(source))
                || sum_source.ty() != sequence_ty
                || pattern.is_some_and(|pattern| binder(pattern).is_none())
                || !plan
                    .value_matches_type(expr.ty(), &RuntimeValue::i64(0))
                    .ok()?
            {
                return None;
            }
            let elided = std::iter::once(accumulator)
                .chain(aliases.iter().copied())
                .collect();
            return Some(SumProjection {
                skipped_ops: index + 1,
                aliases,
                target: pattern.map_or(SumTarget::Return, SumTarget::Bind),
                elided,
            });
        }
        let pattern = pattern?;
        if ![RuntimeLocalReadMode::Move, RuntimeLocalReadMode::Copy]
            .into_iter()
            .any(|mode| local(expr, mode) == Some(source))
            || expr.ty() != sequence_ty
            || pattern.ty() != sequence_ty
        {
            return None;
        }
        let next = binder(pattern)?;
        if next == accumulator || aliases.contains(&next) {
            return None;
        }
        aliases.push(next);
        source = next;
    }
    None
}
// Copy receivers are elided only when no admitted remaining control, input,
// capture or write can observe the omitted numeric locals. This borrows the
// existing exhaustive value-root/owned-body and expression free-read owners;
// unsupported hidden/table control edges conservatively decline.
fn operations_avoid_locals(plan: &RuntimePlan, ops: &[&FlowOp], elided: &[Local]) -> bool {
    use crate::value::RuntimeExpressionNode;
    let expression_avoids = |expr: &RuntimeExpr| {
        expr.evaluation_free_local_reads(plan)
            .is_ok_and(|reads| reads.iter().all(|(local, _)| !elided.contains(local)))
    };
    let mut pending = ops.to_vec();
    while let Some(op) = pending.pop() {
        match op {
            FlowOp::Assign { place, .. } if elided.contains(&place.place().local()) => {
                return false;
            }
            FlowOp::ProjectCall { site } => {
                let Some(site) = plan.project_call_sites().get(*site) else {
                    return false;
                };
                let call = site.plan();
                if !expression_avoids(call.callee())
                    || call
                        .operands()
                        .iter()
                        .any(|operand| !expression_avoids(operand.value()))
                    || site
                        .result()
                        .binding_declarations()
                        .any(|binding| elided.contains(&binding.local()))
                {
                    return false;
                }
            }
            FlowOp::Let { .. }
            | FlowOp::LetElse { .. }
            | FlowOp::Assign { .. }
            | FlowOp::ApplyGroup { .. }
            | FlowOp::HostCall { .. }
            | FlowOp::If { .. }
            | FlowOp::IfLet { .. }
            | FlowOp::Match { .. }
            | FlowOp::Scope { .. }
            | FlowOp::LetScope { .. }
            | FlowOp::ExitScopeBind { .. }
            | FlowOp::Return(_)
            | FlowOp::ReturnExpr(_)
            | FlowOp::EnterScope { .. }
            | FlowOp::ExitScope
            | FlowOp::EnterScheduledScope { .. }
            | FlowOp::ExitScheduledScope { .. }
            | FlowOp::Noop => {}
            _ => return false,
        }
        if op
            .try_visit_value_roots(&mut |_, node| -> Result<(), ()> {
                match node {
                    RuntimeExpressionNode::Expression(expr) if !expression_avoids(expr) => Err(()),
                    RuntimeExpressionNode::Pattern(pattern)
                        if pattern
                            .binding_declarations()
                            .any(|binding| elided.contains(&binding.local())) =>
                    {
                        Err(())
                    }
                    _ => Ok(()),
                }
            })
            .is_err()
        {
            return false;
        }
        for (_, body) in op.owned_bodies() {
            pending.extend(body.iter());
        }
    }
    true
}
impl Engine {
    fn numeric_elision_has_no_later_observer(
        &self,
        plan: &RuntimePlan,
        ops: &[&FlowOp],
        elided: &[Local],
    ) -> bool {
        use super::{FlowControlStackEntryKind as Control, FunctionReturnContinuation as Return};
        let cursor_avoids = |cursor: &super::FlowCursor| {
            self.flow_at_cursor(cursor)
                .and_then(|flow| flow.body().ops().get(cursor.op_index..))
                .is_some_and(|ops| {
                    operations_avoid_locals(plan, &ops.iter().collect::<Vec<_>>(), elided)
                })
        };
        if !operations_avoid_locals(plan, ops, elided)
            || !self.fiber.root_cleanups.is_empty()
            || (!self.fiber.pending_ops.is_empty()
                && self
                    .fiber
                    .cursor
                    .as_ref()
                    .is_some_and(|cursor| !cursor_avoids(cursor)))
        {
            return false;
        }
        self.fiber
            .control_stack
            .iter()
            .all(|frame| match &frame.kind {
                Control::Scope {
                    cleanups,
                    match_guard,
                    ..
                } => cleanups.is_empty() && match_guard.is_none(),
                Control::FunctionCall(frame) => {
                    let continuation_avoids = match &frame.continuation {
                        Return::Program { .. } | Return::Function { .. } => true,
                        Return::Bind { result } => result
                            .binding_declarations()
                            .all(|binding| !elided.contains(&binding.local())),
                        Return::CallableDefault { .. } => false,
                    };
                    continuation_avoids
                        && operations_avoid_locals(
                            plan,
                            &frame.caller_pending_ops.iter().collect::<Vec<_>>(),
                            elided,
                        )
                        && frame
                            .resume
                            .as_ref()
                            .is_none_or(|cursor| cursor_avoids(cursor))
                }
                Control::Loop { .. }
                | Control::While { .. }
                | Control::WhileLet { .. }
                | Control::FormatAttempt(_) => false,
            })
    }

    pub(super) fn try_step_numeric_map_span(
        &mut self,
        remaining_ops: usize,
        mode: RuntimeStepMode,
        output: &mut RuntimeStepOutput,
        backend: &mut impl RuntimeCallBackend,
    ) -> Option<usize> {
        if mode != RuntimeStepMode::Drain
            || self.has_executor_work()
            || self.has_joined_work()
            || !matches!(self.fiber.status, FlowFiberStatus::Running)
            || self.fiber.await_observer.is_some()
        {
            return None;
        }
        let owner = Arc::clone(&self.plan);
        let ops = if self.fiber.pending_ops.is_empty() {
            let cursor = self.fiber.cursor.as_ref()?;
            self.flow_at_cursor(cursor)?
                .body()
                .ops()
                .get(cursor.op_index..)?
                .iter()
                .collect::<Vec<_>>()
        } else {
            self.fiber.pending_ops.iter().collect::<Vec<_>>()
        };
        let (callable, item_ty, output_ty, items, source, calls, loop_base, ordinary_row_ops, sum) =
            if let Some(selected) = project_loop(&owner, ops.first().copied()?) {
                let RuntimeValue::Iterator(RuntimeIterator::Values { items }) =
                    self.fiber.env.get(selected.iterator)?
                else {
                    return None;
                };
                let RuntimeValue::Seq(sequence) =
                    self.fiber.env.inspect_place(selected.result).ok()?
                else {
                    return None;
                };
                if !sequence.is_empty()
                    || items.len().checked_mul(7)?.checked_add(1)? > remaining_ops
                {
                    return None;
                }
                let sum = project_sum(&owner, &ops[1..], selected.result.local());
                (
                    selected.callable,
                    selected.item_ty,
                    selected.output_ty,
                    items.iter().collect::<Vec<_>>(),
                    CommitSource::Loop {
                        iterator: selected.iterator,
                        accumulator: selected.result.clone(),
                    },
                    1,
                    1_usize,
                    7_usize,
                    sum,
                )
            } else {
                let selected = project_array(&owner, &ops)?;
                if selected.items.len() > remaining_ops {
                    return None;
                }
                if selected
                    .results
                    .iter()
                    .filter_map(|result| binder(result))
                    .any(|local| self.fiber.env.get(local).is_some())
                {
                    return None;
                }
                let items = selected
                    .items
                    .iter()
                    .map(|local| self.fiber.env.get(*local))
                    .collect::<Option<Vec<_>>>()?;
                let calls = items.len();
                let sum = selected
                    .aggregate
                    .filter(|local| self.fiber.env.get(*local).is_none())
                    .and_then(|aggregate| project_sum(&owner, &ops[calls + 1..], aggregate))
                    .map(|mut sum| {
                        sum.skipped_ops += 1;
                        sum
                    });
                (
                    selected.callable,
                    selected.item_ty,
                    selected.output_ty,
                    items,
                    CommitSource::Array {
                        items: selected.items,
                        results: selected.results.into_iter().cloned().collect(),
                    },
                    calls,
                    0_usize,
                    1_usize,
                    sum,
                )
            };
        if items.is_empty()
            || items
                .iter()
                .any(|value| !owner.value_matches_type(item_ty, value).unwrap_or(false))
        {
            return None;
        }
        let RuntimeValue::Callable(mapping) = self.fiber.env.get(callable)? else {
            return None;
        };
        let projection = project_numeric_map(&owner, mapping).ok()??;
        if projection.function.result_type() != output_ty {
            return None;
        }
        let rows = match &source {
            CommitSource::Loop { iterator, .. } => {
                let RuntimeValue::Iterator(RuntimeIterator::Values { items }) =
                    self.fiber.env.get(*iterator)?
                else {
                    return None;
                };
                items.len()
            }
            CommitSource::Array { items, .. } => items.len(),
        };
        let sum = sum.filter(|sum| {
            let mut elided = sum.elided.clone();
            if let CommitSource::Array { results, .. } = &source {
                elided.extend(results.iter().filter_map(binder));
            }
            projection.supports_total_sum()
                && sum
                    .aliases
                    .iter()
                    .all(|local| self.fiber.env.get(*local).is_none())
                && match sum.target {
                    SumTarget::Bind(pattern) => {
                        binder(pattern).is_some_and(|local| self.fiber.env.get(local).is_none())
                    }
                    SumTarget::Return => true,
                }
                && self.numeric_elision_has_no_later_observer(
                    &owner,
                    &ops[calls + sum.skipped_ops..],
                    &elided,
                )
        });
        let continuation_ops = sum.as_ref().map_or(0, |sum| sum.skipped_ops);
        // The WhileLet owner schedules EnterScope, Bind, its three body ops,
        // ExitScope and WhileLetNext per row, plus the initial WhileLet. The
        // Array owner has one already-admitted ApplyGroup per bound item. Both
        // retain the forwarding body's actual scheduled control operations.
        // Physical scalar completion and interpreted function frames carry the
        // same original scheduled body cost. This owner supplies the exact
        // total subset/cost across every ABI; the batch never invents a cheaper
        // scalar-only execution model or promotes before its whole span fits.
        let target_ops = projection.function.exact_scalar_completion_control_ops()?;
        let per_row = ordinary_row_ops
            .checked_add(projection.forwarding_ops)?
            .checked_add(target_ops)?;
        let cost = loop_base
            .checked_add(rows.checked_mul(per_row)?)?
            .checked_add(continuation_ops)?;
        if cost > remaining_ops {
            return None;
        }
        // Materialize the borrowed numeric packet only after its complete
        // original schedule fits this caller budget. Backend selection or
        // promotion still follows the actual representation check.
        let flat = projection.flat_inputs(items.into_iter())?;
        backend.prepare_total_numeric_batch(projection.function, rows)?;
        let commit = Commit {
            source,
            consumed_ops: calls + continuation_ops,
            sum: sum.map(|sum| match sum.target {
                SumTarget::Bind(pattern) => OwnedSumTarget::Bind(pattern.clone()),
                SumTarget::Return => OwnedSumTarget::Return,
            }),
            cost,
        };
        let function = projection.function;
        let executed =
            self.with_main_flow_transaction(
                output,
                backend,
                move |candidate, staged, backend, _| {
                    let result = flat.execute(function, rows, commit.sum.is_some(), backend);
                    match result {
                        Ok(value) => {
                            // The physical projection removes only closed numeric
                            // Copy members. Each original item owner still moves once
                            // at the same committed continuation boundary.
                            let mut sum_result = None;
                            match (&commit.source, value) {
                                (
                                    CommitSource::Loop {
                                        iterator,
                                        accumulator,
                                    },
                                    RuntimeNumericMapBatchResult::Values(values),
                                ) => {
                                    let _iterator =
                                        candidate.fiber.env.take(*iterator).expect(
                                            "projected iterator remains owned until commit",
                                        );
                                    for value in values {
                                        if let Err(error) = candidate
                                            .fiber
                                            .env
                                            .push_sequence_item(accumulator, value)
                                        {
                                            candidate.fail_eval(error, staged);
                                            return commit.cost;
                                        }
                                    }
                                }
                                (
                                    CommitSource::Loop {
                                        iterator,
                                        accumulator,
                                    },
                                    RuntimeNumericMapBatchResult::Sum(value),
                                ) => {
                                    let _iterator =
                                        candidate.fiber.env.take(*iterator).expect(
                                            "projected iterator remains owned until commit",
                                        );
                                    let _sequence =
                                        candidate.fiber.env.take(accumulator.local()).expect(
                                            "projected Sum consumes its admitted accumulator",
                                        );
                                    sum_result = Some(RuntimeValue::i64(value));
                                }
                                (
                                    CommitSource::Array { items, results },
                                    RuntimeNumericMapBatchResult::Values(values),
                                ) => {
                                    for (local, (pattern, value)) in
                                        items.iter().zip(results.iter().zip(values))
                                    {
                                        let _item = candidate.fiber.env.take(*local).expect(
                                            "projected Array item remains owned until commit",
                                        );
                                        candidate.bind_value(pattern, value, staged);
                                    }
                                }
                                (
                                    CommitSource::Array { items, .. },
                                    RuntimeNumericMapBatchResult::Sum(value),
                                ) => {
                                    for local in items {
                                        let _item = candidate.fiber.env.take(*local).expect(
                                            "projected Array item remains owned until commit",
                                        );
                                    }
                                    sum_result = Some(RuntimeValue::i64(value));
                                }
                            }
                            if candidate.fiber.pending_ops.is_empty() {
                                if let Some(cursor) = candidate.fiber.cursor.as_mut() {
                                    cursor.op_index += commit.consumed_ops;
                                }
                            } else {
                                for _ in 0..commit.consumed_ops {
                                    candidate.fiber.pending_ops.pop_front();
                                }
                            }
                            candidate.run_child_next = true;
                            if let Some(value) = sum_result {
                                match commit
                                    .sum
                                    .as_ref()
                                    .expect("fused Sum keeps its actual result continuation")
                                {
                                    OwnedSumTarget::Bind(pattern) => {
                                        candidate.bind_value(pattern, value, staged)
                                    }
                                    OwnedSumTarget::Return => {
                                        // This is the original ReturnExpr boundary,
                                        // after its exact span has been consumed. Its
                                        // existing function/scope/cleanup owner still
                                        // validates and publishes the returned value.
                                        let label = crate::value::runtime_value_label(&value);
                                        if !candidate
                                            .return_function_call_value(value, staged, backend)
                                        {
                                            candidate.return_value(label, staged, backend);
                                        }
                                    }
                                }
                            }
                        }
                        Err(error) => candidate.fail_eval(error, staged),
                    }
                    commit.cost
                },
            );
        Some(executed)
    }
}
