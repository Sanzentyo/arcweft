//! Original Program fanout scheduling. The controller remains the aggregate's
//! accepted task; each worker receives its own indexed Source admission.

use super::program_task::ProgramTaskError;
use super::*;
use crate::awbc::fiber::{FiberProgramFanoutState, validate_program_task_argument_refs};
use crate::awbc::schema::{AwbcProgramFanoutInput, AwbcProgramTaskKernel, AwbcTaskPlanExecution};
use crate::runtime_id::RuntimeFiberInstanceId;
use crate::task::{NeedProducerInstance, RuntimeTaskTemplateArguments, TaskRequest, TaskSpec};

impl AwbcProductStepExecutor {
    pub(super) fn fill_program_fanout_for_child(
        &mut self,
        child: &mut ProductChildFiber,
        output: &mut RuntimeStepOutput,
    ) -> Result<bool, ProductStepError> {
        if !matches!(child.owner, ProductChildFiberOwner::Program { .. }) {
            return Err(ProductStepError::Input(
                "Program Fanout requires its accepted Program launch".into(),
            ));
        }
        let Some(suspension) = child.fiber.suspension.as_mut() else {
            return Ok(false);
        };
        let reason = std::mem::replace(&mut suspension.reason, FiberSuspensionReason::BudgetYield);
        let FiberSuspensionReason::ProgramFanout(mut state) = reason else {
            suspension.reason = reason;
            return Ok(false);
        };
        let result = (|| {
            let mut changed = false;
            while (state.next_index as usize) < state.items.len()
                && state.in_flight.len() < state.limit as usize
                && self.remaining_new_task_requests > 0
            {
                self.start_program_fanout_worker(
                    &mut child.fiber,
                    &mut state,
                    child.runtime_generation,
                    output,
                )?;
                changed = true;
            }
            Ok(changed)
        })();
        child
            .fiber
            .suspension
            .as_mut()
            .expect("controller suspension remains owned")
            .reason = FiberSuspensionReason::ProgramFanout(state);
        result
    }

    fn start_program_fanout_worker(
        &mut self,
        parent: &mut FiberState,
        state: &mut FiberProgramFanoutState,
        generation: GenerationId,
        output: &mut RuntimeStepOutput,
    ) -> Result<(), ProgramTaskError> {
        let admitted = self.program.clone();
        let original = admitted
            .original_task_source()
            .ok_or(ProgramTaskError::MissingSource)?;
        let AwbcTaskPlanExecution::Program(body) =
            &admitted.task_plans[state.plan.index()].execution
        else {
            return Err(ProgramTaskError::Parent);
        };
        let AwbcProgramTaskKernel::Fanout { worker, inputs, .. } = &body.body else {
            return Err(ProgramTaskError::Parent);
        };
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            original.program_owner(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        admission.validate_metadata_collection(
            self.child_fibers
                .len()
                .checked_add(1)
                .ok_or(ProgramTaskError::Capacity)?,
        )?;
        admission.validate_metadata_collection(
            state
                .in_flight
                .len()
                .checked_add(1)
                .ok_or(ProgramTaskError::Capacity)?,
        )?;
        admission.validate_metadata_collection(inputs.len())?;
        self.child_fibers
            .try_reserve(1)
            .map_err(|_| ProgramTaskError::Capacity)?;
        state
            .in_flight
            .try_reserve(1)
            .map_err(|_| ProgramTaskError::Capacity)?;
        output
            .requests
            .tasks
            .try_reserve(1)
            .map_err(|_| ProgramTaskError::Capacity)?;
        let limits = crate::plan::RuntimeTaskPlanSealLimits::default();
        let source = original.awbc_program_source(
            *worker,
            limits,
            &mut admission.task_source_meter(limits)?,
        )?;
        let template = source
            .request_endpoint()
            .program_template()
            .ok_or(ProgramTaskError::Parent)?;
        let producer = template.producer();
        let index = state.next_index;
        let frame = parent.active_frame()?;
        let item = state
            .items
            .get(index as usize)
            .and_then(Option::as_ref)
            .ok_or(ProgramTaskError::Parent)?;
        let values = inputs
            .iter()
            .map(|input| match input {
                AwbcProgramFanoutInput::Item => Ok(item),
                AwbcProgramFanoutInput::Capture { position } => {
                    frame.register(crate::awbc::schema::AwbcRegisterId(*position))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let checked = source.template_producer_spec(
            original,
            RuntimeTaskTemplateArguments::FanoutProgramWorker {
                captures: &values,
                index,
            },
            frame.type_instantiation.as_deref(),
            &mut admission,
        )?;
        admission.validate_metadata_string(&producer.debug_label)?;
        admission.validate_metadata(&checked.source().assertion())?;
        let binding = validate_program_task_argument_refs(
            &admitted,
            *worker,
            &values,
            frame.type_instantiation.as_deref(),
            &admission.work(),
        )?;
        let next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(ProgramTaskError::Capacity)?;
        let mut next_fiber_instance = self.next_fiber_instance;
        let instance = RuntimeFiberInstanceId::from_allocated(
            next_fiber_instance.take_next(crate::runtime_id::RuntimeIdNamespace::FiberInstance)?,
        );
        let child = binding.prepare_owned_program_task_fiber(
            &admitted,
            *worker,
            instance,
            self.next_generation,
            parent.budget.quantum,
        )?;
        let last = index as usize + 1 == state.items.len();
        let mut copies = Vec::with_capacity(inputs.len());
        for (input, value) in inputs.iter().zip(&values) {
            copies.push(
                if !last && matches!(input, AwbcProgramFanoutInput::Capture { .. }) {
                    Some(
                        value
                            .live_view()
                            .copy_unrestricted()
                            .ok_or(ProgramTaskError::Parent)?,
                    )
                } else {
                    None
                },
            );
        }
        let mut arguments = Vec::with_capacity(inputs.len());
        let spec = TaskSpec {
            generation,
            producer: NeedProducerInstance::try_from(&checked)?,
            class: producer.class.clone(),
            priority: producer.priority,
            cancel_scope: producer.cancel_scope.clone(),
            policy: producer.policy,
            outcome: producer.outcome(),
            request: TaskRequest::Program(checked.source().clone()),
            debug_label: producer.debug_label.clone(),
        };
        let (receipt, launch, handle) =
            self.need_producers
                .ensure_program_task_with_binding(spec, |receipt| {
                    Ok::<_, ProgramTaskError>(crate::task::RuntimeNeedHandle::try_from(
                        receipt.clone(),
                    )?)
                })?;
        for (position, input) in inputs.iter().enumerate() {
            arguments.push(match input {
                AwbcProgramFanoutInput::Item => state.items[index as usize]
                    .take()
                    .expect("unique future item was admitted before launch"),
                AwbcProgramFanoutInput::Capture { position: source } if last => parent
                    .active_frame_mut()
                    .expect("exclusive original parent frame remains active")
                    .take_register(crate::awbc::schema::AwbcRegisterId(*source))
                    .expect("all distinct final capture moves were admitted"),
                AwbcProgramFanoutInput::Capture { .. } => copies[position]
                    .take()
                    .expect("all earlier unrestricted capture copies were prepared"),
            });
        }
        let deliver = launch.delivery() == crate::task::ProgramTaskDelivery::HostMetadata;
        self.child_fibers.push_back(ProductChildFiber {
            owner: ProductChildFiberOwner::Program {
                launch,
                parent: parent.instance,
            },
            fiber: child.bind(arguments),
            runtime_generation: generation,
            pending_host_call: None,
        });
        state
            .in_flight
            .push(crate::awbc::fiber::FiberAwaitManyInFlight { index, handle });
        state.next_index += 1;
        self.next_generation = next_generation;
        self.next_fiber_instance = next_fiber_instance;
        self.remaining_new_task_requests -= 1;
        if deliver {
            output.requests.tasks.push(receipt);
        }
        Ok(())
    }

    pub(super) fn resume_program_fanout_for_child(
        &mut self,
        child: &mut ProductChildFiber,
        resume: AwbcResumePointId,
        output: &mut RuntimeStepOutput,
        journal: &mut DeferredChildResumeJournal,
    ) -> Result<bool, ProductStepError> {
        if !matches!(child.owner, ProductChildFiberOwner::Program { .. }) {
            return Err(ProductStepError::Input(
                "Program Fanout requires its accepted Program launch".into(),
            ));
        }
        let Some(suspension) = child.fiber.suspension.as_mut() else {
            return Ok(false);
        };
        let FiberSuspensionReason::ProgramFanout(mut state) =
            std::mem::replace(&mut suspension.reason, FiberSuspensionReason::BudgetYield)
        else {
            return Err(ProductStepError::Internal(
                "Program Fanout suspension changed during dispatch".into(),
            ));
        };
        let mut cancelled = false;
        let result = (|| {
            let mut changed = false;
            let mut position = 0;
            while position < state.in_flight.len() {
                let handle = &state.in_flight[position].handle;
                let launch = self
                    .need_producers
                    .launch_for_handle(handle)
                    .ok_or_else(|| {
                        ProductStepError::Input(
                            "Program Fanout worker lost its accepted launch".into(),
                        )
                    })?;
                match launch.state() {
                    crate::task::RuntimeNeedProducerState::Ready(_) => {
                        let proof = self
                            .need_producers
                            .inspect_ready_take_for_correlation(&handle.correlation())
                            .map_err(|error| ProductStepError::Input(error.to_string()))?;
                        let flight = state.in_flight.remove(position);
                        let value = self.need_producers.take_ready_for_need_prepared(proof);
                        state.results[flight.index as usize] = Some(value.into_value());
                        journal.record_owner_value_transfer();
                        changed = true;
                    }
                    crate::task::RuntimeNeedProducerState::InfrastructureFailure(error) => {
                        return Err(ProductStepError::Input(error.to_string()));
                    }
                    crate::task::RuntimeNeedProducerState::Cancelled => {
                        cancelled = true;
                        return Ok(true);
                    }
                    crate::task::RuntimeNeedProducerState::ReadyTransferred => {
                        return Err(ProductStepError::Input(
                            "Program Fanout worker Ready was already transferred".into(),
                        ));
                    }
                    crate::task::RuntimeNeedProducerState::NotStarted
                    | crate::task::RuntimeNeedProducerState::Pending(_) => position += 1,
                }
            }
            if state.next_index as usize == state.items.len() && state.in_flight.is_empty() {
                self.finish_program_fanout(&mut child.fiber, &mut state, resume)?;
                journal.record_owner_value_transfer();
                return Ok(true);
            }
            Ok(changed)
        })();
        if child.fiber.status == FiberStatus::Suspended {
            child
                .fiber
                .suspension
                .as_mut()
                .expect("pending Program controller retains its suspension")
                .reason = FiberSuspensionReason::ProgramFanout(state);
        }
        let changed = result?;
        if cancelled {
            // Restore the original suspension owner before admitting unwind.
            // Its ordinary Program retirement joins already attached workers.
            let cancellation = crate::awbc::vm::cancel_fiber(&self.program, &mut child.fiber)
                .map_err(|error| ProductStepError::Internal(error.to_string()))?;
            journal.record_drop_policy(crate::effect::RuntimeDropPolicy::Cancel);
            journal.record_deferred_observations(cancellation.observations);
            return Ok(true);
        }
        let started = if child.fiber.status == FiberStatus::Suspended {
            self.fill_program_fanout_for_child(child, output)?
        } else {
            false
        };
        if started {
            journal.record_owner_value_transfer();
        }
        Ok(changed || started)
    }

    fn finish_program_fanout(
        &mut self,
        fiber: &mut FiberState,
        state: &mut FiberProgramFanoutState,
        resume: AwbcResumePointId,
    ) -> Result<(), ProductStepError> {
        let admitted = self.program.clone();
        let AwbcTaskPlanExecution::Program(body) =
            &admitted.task_plans[state.plan.index()].execution
        else {
            return Err(ProductStepError::Internal(
                "Fanout controller plan changed kind".into(),
            ));
        };
        let AwbcProgramTaskKernel::Fanout { worker, .. } = body.body else {
            return Err(ProductStepError::Internal(
                "Fanout controller lost its original worker".into(),
            ));
        };
        let binding = fiber.active_frame()?.type_instantiation.clone();
        let mut admission =
            admitted.retained_value_admission(crate::entry::RuntimeSchemaLimits::engine_default());
        let aggregate = admission
            .prepare_fanout(
                admitted.runtime_types[admitted.task_plans[state.plan.index()]
                    .payload_type()
                    .index()]
                .semantic_identity(),
                admitted.runtime_types[admitted.task_plans[worker.index()].payload_type().index()]
                    .semantic_identity(),
                binding.as_deref(),
                &mut state.results,
            )
            .map_err(|error| ProductStepError::Input(error.to_string()))?;
        let prepared = aggregate.with_view(|value| {
            let resume = fiber.validate_resume_at(&admitted, resume)?;
            let binding = state
                .binding
                .map(|pattern| {
                    crate::awbc::vm::prepare_pattern_binding_view(&admitted, fiber, pattern, value)
                        .map_err(|error| ProductStepError::Input(error.to_string()))
                })
                .transpose()?;
            if let Some(binding) = &binding {
                let frame = fiber.active_frame()?;
                if binding
                    .registers()
                    .iter()
                    .any(|register| !frame.registers[register.index()].is_vacant())
                {
                    return Err(ProductStepError::Input(
                        "Program Fanout result destination is occupied".into(),
                    ));
                }
            }
            Ok::<_, ProductStepError>((resume, binding))
        });
        let (resume, binding) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                aggregate.abort();
                return Err(error);
            }
        };
        let value = aggregate.commit();
        if let Some(binding) = binding {
            crate::awbc::vm::bind_pattern_owned_prepared(&admitted, fiber, binding, value);
        }
        fiber.resume_at_prepared(resume);
        Ok(())
    }
}
