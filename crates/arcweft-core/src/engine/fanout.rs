//! Bounded Program fanout executes the worker owned by its original Source.

use super::{AwaitManyInFlight, Engine, FlowFiberStatus, WaitingManyStatus};
use crate::engine::code_cursor::NativeSourceOperation;
use crate::plan::{FlowOp, RuntimeFlowBodyRole, RuntimeProgramTaskBody};
use crate::task::{NeedProducerInstance, RuntimeNeedProducerState, RuntimeTaskTemplateArguments};
use crate::value::{
    RuntimePayload, RuntimeRetainedValueAdmission, RuntimeStoredValue, RuntimeValue,
};
use std::sync::Arc;

#[cfg(test)]
mod tests;

/// Native continuation state owns unstarted items and completed results.
/// Worker code and capture declarations remain in the original executable.
#[derive(Debug, PartialEq)]
pub struct ProgramFanoutState<
    H = crate::task::RuntimeNeedHandle,
    V = RuntimeValue,
    P = RuntimePayload,
> {
    pub(super) source: NativeSourceOperation,
    pub(super) context: crate::value::RuntimeScopeTypeContext,
    pub(super) base: H,
    pub(super) limit: u32,
    pub(super) items: Vec<Option<V>>,
    pub(super) next_index: usize,
    pub(super) in_flight: Vec<AwaitManyInFlight<H>>,
    pub(super) results: Vec<Option<P>>,
}

pub(super) type ProgramFanoutRollbackImage = ProgramFanoutState<
    crate::task::RuntimeNeedHandleSaveSnapshot,
    crate::value::AwbcRuntimeValueSnapshot,
    crate::value::AwbcRuntimeValueSnapshot,
>;

impl Clone for ProgramFanoutRollbackImage {
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            context: self.context.clone(),
            base: self.base.clone(),
            limit: self.limit,
            items: self.items.clone(),
            next_index: self.next_index,
            in_flight: self.in_flight.clone(),
            results: self.results.clone(),
        }
    }
}

impl ProgramFanoutState {
    pub(super) fn snapshot_with_admitted_values(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<ProgramFanoutRollbackImage, String> {
        let value = |value: &RuntimeValue| {
            crate::value::AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value, owner)
                .map_err(|error| error.to_string())
        };
        Ok(ProgramFanoutRollbackImage {
            source: self.source.clone(),
            context: self.context.clone(),
            base: crate::task::RuntimeNeedHandleSaveSnapshot::from_live_admitted(
                &self.base,
                Some(owner),
            )
            .map_err(|error| error.to_string())?,
            limit: self.limit,
            next_index: self.next_index,
            items: self
                .items
                .iter()
                .map(|item| item.as_ref().map(value).transpose())
                .collect::<Result<_, _>>()?,
            in_flight: self
                .in_flight
                .iter()
                .map(|flight| {
                    Ok(AwaitManyInFlight {
                        index: flight.index,
                        observed_through: flight.observed_through,
                        handle: crate::task::RuntimeNeedHandleSaveSnapshot::from_live_admitted(
                            &flight.handle,
                            Some(owner),
                        )
                        .map_err(|error| error.to_string())?,
                    })
                })
                .collect::<Result<_, String>>()?,
            results: self
                .results
                .iter()
                .map(|result| {
                    result
                        .as_ref()
                        .map(|payload| value(payload.value()))
                        .transpose()
                })
                .collect::<Result<_, _>>()?,
        })
    }

    pub(super) fn from_image_with_admitted_values(
        image: ProgramFanoutRollbackImage,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, String> {
        let value = |value: crate::value::AwbcRuntimeValueSnapshot| {
            value
                .into_admitted_runtime_value(owner)
                .map_err(|error| error.to_string())
        };
        Ok(Self {
            source: image.source,
            context: image.context,
            base: image
                .base
                .into_live_admitted(owner)
                .map_err(|error| error.to_string())?,
            limit: image.limit,
            next_index: image.next_index,
            items: image
                .items
                .into_iter()
                .map(|item| item.map(value).transpose())
                .collect::<Result<_, _>>()?,
            in_flight: image
                .in_flight
                .into_iter()
                .map(|flight| {
                    Ok(AwaitManyInFlight {
                        index: flight.index,
                        observed_through: flight.observed_through,
                        handle: flight
                            .handle
                            .into_live_admitted(owner)
                            .map_err(|error| error.to_string())?,
                    })
                })
                .collect::<Result<_, String>>()?,
            results: image
                .results
                .into_iter()
                .map(|result| {
                    result
                        .map(|item| value(item).map(RuntimePayload::new))
                        .transpose()
                })
                .collect::<Result<_, _>>()?,
        })
    }
}

fn original_fanout<'a>(
    source: &NativeSourceOperation,
    plan: &'a crate::plan::RuntimePlan,
) -> Result<
    (
        &'a crate::task::NeedProducerDefinition,
        &'a crate::plan::RuntimeProgramFanout,
    ),
    String,
> {
    let Some(FlowOp::ProgramTask { template, .. }) = source.operation(plan) else {
        return Err("Program fanout lost its original ProgramTask operation".into());
    };
    let RuntimeProgramTaskBody::Fanout(fanout) = &template.body else {
        return Err("Program fanout requires its original Fanout kernel".into());
    };
    Ok((&template.producer, fanout))
}

impl Engine {
    pub(super) fn start_program_fanout_state(
        &mut self,
        source: NativeSourceOperation,
        output: &mut crate::step::RuntimeStepOutput,
        backend: &mut impl super::RuntimeCallBackend,
    ) {
        let prepare = (|| {
            let (producer, fanout) = original_fanout(&source, &self.plan)?;
            let super::FlowFiberLaunch::Program(launch) = &self.fiber.launch else {
                return Err("Program fanout requires an accepted Program launch".into());
            };
            if self.plan.task_plan(&producer.plan).map(|row| row.key())
                != Some(launch.source().task_plan().key())
            {
                return Err("Program fanout belongs to a different accepted launch".into());
            }
            let limit = match self.fiber.frame.env.get(fanout.limit) {
                Some(RuntimeValue::UInt(crate::value::RuntimeUInt::U32(limit))) if *limit != 0 => {
                    *limit
                }
                _ => return Err("Program fanout limit must be a positive u32".into()),
            };
            let Some(RuntimeValue::Seq(items)) = self.fiber.frame.env.get(fanout.source) else {
                return Err("Program fanout source must be its owned collection".into());
            };
            let count = items.len();
            u32::try_from(count).map_err(|_| "Program fanout source exceeds u32 positions")?;
            let mut admission = RuntimeRetainedValueAdmission::for_program(
                launch.source().program_owner(),
                crate::entry::RuntimeSchemaLimits::engine_default(),
            );
            admission
                .validate_metadata_collection(count)
                .map_err(|error| error.to_string())?;
            admission
                .validate_metadata_collection(count)
                .map_err(|error| error.to_string())?;
            admission
                .validate_metadata_collection((limit as usize).min(count))
                .map_err(|error| error.to_string())?;
            for capture in &fanout.worker.captures {
                if capture.local == fanout.item {
                    continue;
                }
                let value = self.fiber.frame.env.get(capture.local).ok_or_else(|| {
                    format!("Program fanout capture {} is uninitialized", capture.local)
                })?;
                if count > 1 {
                    admission
                        .validate_borrowed(value.borrowed_value())
                        .map_err(|error| error.to_string())?;
                    if !value.ownership().permits_copy() {
                        return Err(format!(
                            "Program fanout repeats an affine capture {}",
                            capture.local
                        ));
                    }
                }
            }
            let base = crate::task::RuntimeNeedHandle::try_from_accepted_launch(
                launch.spec().clone(),
                launch.handle(),
            )
            .map_err(|error| error.to_string())?;
            Ok::<_, String>((fanout.source, limit, count, base))
        })();
        let (local, limit, count, base) = match prepare {
            Ok(prepared) => prepared,
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        let Some(RuntimeValue::Seq(items)) = self.fiber.frame.env.take(local) else {
            unreachable!("the admitted collection remains in its exclusive parent frame")
        };
        let state = ProgramFanoutState {
            source,
            context: self.fiber.frame.env.source_function_context(),
            base,
            limit,
            items: items.into_values().into_iter().map(Some).collect(),
            next_index: 0,
            in_flight: Vec::with_capacity((limit as usize).min(count)),
            results: (0..count).map(|_| None).collect(),
        };
        self.resume_program_fanout_state(state, output, backend);
    }

    pub(super) fn resume_program_fanout_state(
        &mut self,
        mut state: ProgramFanoutState,
        output: &mut crate::step::RuntimeStepOutput,
        backend: &mut impl super::RuntimeCallBackend,
    ) {
        let mut position = 0;
        while position < state.in_flight.len() {
            let flight = &state.in_flight[position];
            let Some(launch) = self.need_producers.launch_for_handle(&flight.handle) else {
                self.fail_eval("Program fanout child lost its accepted launch", output);
                return;
            };
            match launch.state() {
                RuntimeNeedProducerState::Ready(_) => {
                    let flight = state.in_flight.remove(position);
                    let value = self
                        .need_producers
                        .take_ready_for_correlation(&flight.handle.correlation())
                        .expect("the inspected local Ready remains owned by its accepted launch");
                    state.results[flight.index] = Some(value);
                }
                RuntimeNeedProducerState::InfrastructureFailure(failure) => {
                    self.fail_eval(failure.clone(), output);
                    return;
                }
                RuntimeNeedProducerState::Cancelled => {
                    self.fiber.frame.status = FlowFiberStatus::Done(super::FlowExit::Done);
                    return;
                }
                RuntimeNeedProducerState::ReadyTransferred => {
                    self.fail_eval(
                        "Program fanout child result was already transferred",
                        output,
                    );
                    return;
                }
                RuntimeNeedProducerState::NotStarted | RuntimeNeedProducerState::Pending(_) => {
                    position += 1
                }
            }
        }
        while state.next_index < state.items.len() && state.in_flight.len() < state.limit as usize {
            if self.task_request_quota_remaining == 0 {
                break;
            }
            if let Err(error) = self.start_program_fanout_worker(&mut state, output) {
                self.fiber.frame.status =
                    FlowFiberStatus::WaitingMany(WaitingManyStatus::Program(Box::new(state)));
                self.fail_eval(error, output);
                return;
            }
        }
        if !state.in_flight.is_empty() || state.next_index < state.items.len() {
            self.fiber.frame.status =
                FlowFiberStatus::WaitingMany(WaitingManyStatus::Program(Box::new(state)));
            return;
        }
        let plan = Arc::clone(&self.plan);
        let (producer, fanout) = original_fanout(&state.source, &plan)
            .expect("the suspended state retains its admitted original fanout");
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            crate::task::RuntimeProgramOwner::Plan(plan.clone()),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        let aggregate = match admission.prepare_fanout(
            producer.outcome().payload_semantic_identity(),
            fanout.worker.producer.outcome().payload_semantic_identity(),
            state.context.instantiation(),
            &mut state.results,
        ) {
            Ok(aggregate) => aggregate,
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        let super::FlowFiberLaunch::Program(launch) = &self.fiber.launch else {
            unreachable!("the fanout executes only under its admitted launch")
        };
        if let Err(error) = aggregate.with_view(|view| {
            launch.admit_result_view(
                view.inspect(),
                crate::entry::schema::value_encoding::ValueAdmissionOrigin::Live,
                &mut admission,
            )
        }) {
            aggregate.abort();
            self.fiber.frame.status =
                FlowFiberStatus::WaitingMany(WaitingManyStatus::Program(Box::new(state)));
            self.fail_eval(error, output);
            return;
        }
        self.complete_admitted_program_task_return(aggregate.commit(), true, output, backend);
    }

    fn start_program_fanout_worker(
        &mut self,
        state: &mut ProgramFanoutState,
        output: &mut crate::step::RuntimeStepOutput,
    ) -> Result<(), String> {
        let plan = Arc::clone(&self.plan);
        let (_, fanout) = original_fanout(&state.source, &plan)?;
        let producer = &fanout.worker.producer;
        let index = state.next_index;
        let item = state.items[index]
            .as_ref()
            .ok_or("Program fanout item was already moved")?;
        let program = crate::plan::task_source::RuntimeTaskSourceProgram::native(plan.clone());
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            program.program_owner(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        let limits = crate::plan::RuntimeTaskPlanSealLimits::default();
        let mut meter = admission
            .task_source_meter(limits)
            .map_err(|error| error.to_string())?;
        let source = program
            .original()
            .task_source(&producer.plan, limits, &mut meter)
            .map_err(|error| error.to_string())?;
        admission
            .validate_metadata_collection(fanout.worker.captures.len())
            .map_err(|error| error.to_string())?;
        let values = fanout
            .worker
            .captures
            .iter()
            .map(|capture| {
                if capture.local == fanout.item {
                    Ok(item)
                } else {
                    self.fiber
                        .frame
                        .env
                        .get(capture.local)
                        .ok_or("Program fanout capture is uninitialized")
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let checked = source
            .template_producer_spec(
                &program,
                RuntimeTaskTemplateArguments::FanoutProgramWorker {
                    captures: &values,
                    index: index as u32,
                },
                state.context.instantiation(),
                &mut admission,
            )
            .map_err(|error| error.to_string())?;
        admission
            .validate_metadata_string(&producer.debug_label)
            .map_err(|error| error.to_string())?;
        admission
            .validate_metadata(&checked.source().assertion())
            .map_err(|error| error.to_string())?;
        let spec = crate::task::TaskSpec {
            generation: self.generation,
            producer: NeedProducerInstance::try_from(&checked)
                .map_err(|error| error.to_string())?,
            class: producer.class.clone(),
            priority: producer.priority,
            cancel_scope: producer.cancel_scope.clone(),
            policy: producer.policy,
            outcome: producer.outcome(),
            request: crate::task::TaskRequest::Program(checked.source().clone()),
            debug_label: producer.debug_label.clone(),
        };
        let body = self
            .original_body_range(&state.source, RuntimeFlowBodyRole::Body)
            .map_err(|error| error.to_string())?;
        let pending = self
            .prepare_task_root_work(body, false)
            .map_err(|error| error.to_string())?;
        let (mut child, next_id) = self
            .prepare_child_fiber(pending, &[])
            .map_err(|error| error.to_string())?;
        let last = index + 1 == state.items.len();
        let transfers: Vec<_> = fanout
            .worker
            .captures
            .iter()
            .filter(|capture| capture.local != fanout.item)
            .map(|capture| crate::plan::RuntimeProgramTaskCapture {
                local: capture.local,
                mode: if last {
                    crate::value::RuntimeLocalReadMode::Move
                } else {
                    crate::value::RuntimeLocalReadMode::Copy
                },
            })
            .collect();
        self.child_fibers
            .try_reserve(1)
            .map_err(|error| error.to_string())?;
        let captured = self
            .fiber
            .frame
            .env
            .prepare_task_captures(&transfers)
            .map_err(|error| error.to_string())?;
        let (receipt, launch, handle) = self
            .need_producers
            .ensure_program_task_with_binding(spec, |receipt| {
                Ok::<_, super::thread::ProgramTaskPreparationError>(
                    crate::task::RuntimeNeedHandle::try_from(receipt.clone())?,
                )
            })
            .map_err(|error| error.to_string())?;
        child.frame.env = captured.commit();
        child
            .frame
            .env
            .bind_all([crate::value::RuntimeLocalBinding {
                local: fanout.item,
                value: state.items[index]
                    .take()
                    .expect("the admitted item remains owned until launch commit"),
            }]);
        let deliver = launch.delivery() == crate::task::ProgramTaskDelivery::HostMetadata;
        child.launch = super::FlowFiberLaunch::Program(launch);
        child.frame.status = FlowFiberStatus::Running;
        self.child_fibers.push_back(child);
        self.next_fiber_id = next_id;
        self.run_child_next = true;
        self.task_request_quota_remaining -= 1;
        state.in_flight.push(AwaitManyInFlight {
            index,
            handle,
            observed_through: None,
        });
        state.next_index += 1;
        if deliver {
            output.requests.tasks.push(receipt);
        }
        Ok(())
    }
}
