//! A closed total source Map can execute as its owning control operation.
//! The original ordinary body remains authoritative and is checked in full.

use super::*;
use crate::pattern::RuntimePatternRest;
use crate::plan::RuntimeStandardMapControl;
use crate::value::RuntimeStandardMapFamily as Family;

fn project_body<'a>(
    plan: &RuntimePlan,
    control: &'a RuntimeStandardMapControl,
) -> Option<&'a RuntimePattern> {
    match control.family() {
        Family::Array => {
            let [FlowOp::Let { pattern, expr }, rest @ ..] = control.mapping_body() else {
                return None;
            };
            let RuntimePatternKind::Sequence {
                items,
                rest: RuntimePatternRest::Exact,
            } = pattern.kind()
            else {
                return None;
            };
            if local(expr, RuntimeLocalReadMode::Move) != Some(control.source())
                || expr.ty() != pattern.ty()
            {
                return None;
            }
            let selected = project_array(plan, &rest.iter().collect::<Vec<_>>())?;
            if rest.len() != selected.items.len().checked_add(1)?
                || items.len() != selected.items.len()
                || selected.callable != control.mapping()
                || selected.aggregate != Some(control.result())
                || items.iter().zip(&selected.items).any(|(pattern, local)| {
                    binder(pattern) != Some(*local) || pattern.ty() != selected.item_ty
                })
            {
                return None;
            }
            let FlowOp::Let { pattern, .. } = rest.last()? else {
                return None;
            };
            Some(pattern)
        }
        Family::Vec | Family::Seq | Family::Slice => {
            let [
                FlowOp::Let {
                    pattern,
                    expr: empty,
                },
                FlowOp::Let {
                    pattern: iterator,
                    expr: into,
                },
                gate,
            ] = control.mapping_body()
            else {
                return None;
            };
            let RuntimeExprKind::BracketSeq(items) = empty.kind() else {
                return None;
            };
            let RuntimeExprKind::Call { callee, args } = into.kind() else {
                return None;
            };
            let [argument] = args.as_slice() else {
                return None;
            };
            let intrinsic = match control.family() {
                Family::Vec => RuntimeIntrinsic::CoreVecIntoIter,
                Family::Seq => RuntimeIntrinsic::CoreSeqIntoIter,
                Family::Slice => RuntimeIntrinsic::CoreSliceIntoIter,
                _ => unreachable!("selected sequence family"),
            };
            let selected = project_loop(plan, gate)?;
            if !items.is_empty()
                || binder(pattern) != Some(control.result())
                || empty.ty() != pattern.ty()
                || selected.result.local() != control.result()
                || selected.callable != control.mapping()
                || binder(iterator) != Some(selected.iterator)
                || iterator.ty() != into.ty()
                || callee.as_intrinsic() != Some(intrinsic)
                || argument.mode() != RuntimeCallArgumentMode::Value
                || argument.abi_position() != 0
                || local(argument.value(), RuntimeLocalReadMode::Move) != Some(control.source())
            {
                return None;
            }
            Some(pattern)
        }
        Family::Option | Family::Result => None,
    }
}

impl Engine {
    pub(in crate::engine) fn try_step_numeric_map_control(
        &mut self,
        remaining_ops: usize,
        mode: RuntimeStepMode,
        output: &mut RuntimeStepOutput,
        backend: &mut impl RuntimeCallBackend,
    ) -> Option<usize> {
        if remaining_ops == 0
            || mode != RuntimeStepMode::Drain
            || self.has_executor_work()
            || self.has_joined_work()
            || !matches!(self.fiber.frame.status, FlowFiberStatus::Running)
            || self.fiber.frame.await_observer.is_some()
        {
            return None;
        }
        let owner = Arc::clone(&self.plan);
        let current = if self.fiber.frame.pending_ops.is_empty() {
            self.original_operation_remainder(&owner)?.first()?
        } else {
            self.fiber.frame.pending_ops.front()?
        };
        let FlowOp::StandardMap(control) = current else {
            return None;
        };
        let ops = if self.fiber.frame.pending_ops.is_empty() {
            self.original_operation_remainder(&owner)?
                .iter()
                .collect::<Vec<_>>()
        } else {
            self.fiber.frame.pending_ops.iter().collect::<Vec<_>>()
        };
        let result_pattern = project_body(&owner, control)?;
        let value = self.fiber.frame.env.get(control.mapping())?;
        control.admit_mapping(value).ok()?;
        let RuntimeValue::Callable(mapping) = value else {
            return None;
        };
        // Captured or partially bound callbacks preserve their original
        // per-application continuation and its operation budget.
        if !mapping.retained().is_empty() {
            return None;
        }
        let projection = project_numeric_map(&owner, mapping).ok()??;
        if !projection.function.supports_total_numeric_completion() {
            return None;
        }
        let source = self.fiber.frame.env.get(control.source())?;
        let source_type = owner.local_declarations().get(control.source())?.ty();
        if !source.ownership().permits_copy()
            || !owner.value_matches_type(source_type, source).ok()?
        {
            return None;
        }
        let RuntimeValue::Seq(items) = source else {
            return None;
        };
        if items.is_empty() || self.fiber.frame.env.get(control.result()).is_some() {
            return None;
        }
        let result_type = owner.local_declarations().get(control.result())?.ty();
        let output_type = match owner.type_table().get(result_type)?.projection() {
            RuntimePlanTypeProjection::Array { item, .. }
            | RuntimePlanTypeProjection::Sequence { item, .. } => *item,
            _ => return None,
        };
        if projection.function.result_type() != output_type {
            return None;
        }
        let consuming = control.consuming_body().unwrap_or_default();
        let consuming_ops = consuming.iter().collect::<Vec<_>>();
        let sum = project_numeric_sum(&owner, &consuming_ops, control.result()).filter(|sum| {
            sum.aliases
                .iter()
                .all(|local| self.fiber.frame.env.get(*local).is_none())
                && match sum.target {
                    SumTarget::Bind(pattern) => binder(pattern)
                        .is_some_and(|local| self.fiber.frame.env.get(local).is_none()),
                    SumTarget::Return => true,
                }
                && self.numeric_elision_has_no_later_observer(&owner, &ops[1..], &sum.elided)
        });
        let admission = projection.admit_sequence(items, sum.is_some())?;
        let sum = sum.filter(|_| admission.is_sum());
        let consumed = if sum.is_some() {
            control.completion_operation_cost()
        } else {
            1
        };
        if consumed > remaining_ops {
            return None;
        }
        let batch = admission.materialize();
        backend.prepare_total_numeric_batch(batch.function(), batch.rows())?;
        let source = control.source();
        let result_pattern = result_pattern.clone();
        let continuation = if sum.is_none() {
            consuming.to_vec()
        } else {
            Vec::new()
        };
        let sum = sum.map(|sum| match sum.target {
            SumTarget::Bind(pattern) => OwnedSumTarget::Bind(pattern.clone()),
            SumTarget::Return => OwnedSumTarget::Return,
        });
        Some(self.with_main_flow_transaction(
            output,
            backend,
            move |candidate, staged, backend, _| {
                match batch.execute(backend) {
                    Ok(result) => {
                        let _source = candidate
                            .fiber
                            .frame
                            .env
                            .take(source)
                            .expect("admitted Map source is consumed once at commit");
                        candidate.consume_projected_ops(1);
                        candidate.run_child_next = true;
                        match result {
                            RuntimeNumericBatchResult::Values(values) => {
                                candidate.bind_value(
                                    &result_pattern,
                                    crate::value::runtime_sequence_from_literal_values(values),
                                    staged,
                                );
                                candidate.push_ops(continuation);
                            }
                            RuntimeNumericBatchResult::Sum(value) => {
                                let value = RuntimeValue::i64(value);
                                match sum
                                    .as_ref()
                                    .expect("fused Map retains its original consuming Sum")
                                {
                                    OwnedSumTarget::Bind(pattern) => {
                                        candidate.bind_value(pattern, value, staged);
                                    }
                                    OwnedSumTarget::Return => {
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
                    }
                    Err(error) => candidate.fail_eval(error, staged),
                }
                consumed
            },
        ))
    }
}
