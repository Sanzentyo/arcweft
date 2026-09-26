use super::{
    AudioCommandEnvelope, AudioDispatchId, AwbcAwaitObserverResume, AwbcEffectPlanId,
    AwbcFunctionId, AwbcHostCallId, AwbcHostCallMode, AwbcProductStepExecutor, AwbcProgram,
    AwbcResumePointId, AwbcTrapCode, FiberAwaitManyInFlight, FiberAwaitTarget, FiberState,
    FiberSuspensionReason, FlowEvent, MappedEffect, NeedId, PendingHostCall, ProductStepError,
    RuntimeDiagnostic, RuntimeDiagnosticCategory, RuntimeHostCallId, RuntimeHostCallMode,
    RuntimeHostCallRequest, RuntimeNeedState, RuntimePayload, RuntimeStepOutput,
    RuntimeStreamEvent, RuntimeValue, TaskEvent, TaskEventKind, TaskId, TaskSequence,
    VmObservation, content_request, resolved_runtime_need_state, runtime_sequence_values,
    runtime_value_label, stream_id_for, task_spec,
};
use crate::awbc::vm::cancel_fiber;
use crate::stream::StreamEventKind;
use crate::task::{NamedHostArg, NeedProducerRuntimeArgument};
use crate::value::runtime_value_into_sequence_values;
use arcweft_need::Need;
use std::cmp::Ordering;

struct NeedProducerStartOutput {
    spec: crate::task::TaskSpec,
}

impl AwbcProductStepExecutor {
    pub(super) fn emit_pending_need_reensure(&mut self, output: &mut RuntimeStepOutput) -> bool {
        let pending = self
            .need_producers
            .pending_reensure()
            .map(|launch| launch.need().clone())
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return true;
        }

        let mut candidate = self.need_producers.clone();
        let mut requests = Vec::new();
        for need in &pending {
            match candidate.mark_task_ensured(need) {
                Ok(Some(spec)) => requests.push(spec),
                Ok(None) => continue,
                Err(error) => {
                    self.fail_with_error(ProductStepError::Internal(error.to_string()), output);
                    return false;
                }
            }
        }
        if requests.is_empty() {
            self.fail_with_error(
                ProductStepError::Internal(
                    "Restartable Need re-registration produced no task specification".to_owned(),
                ),
                output,
            );
            return false;
        }
        self.need_producers = candidate;
        output.requests.tasks.extend(requests);
        true
    }

    pub(super) fn latch_task_events(
        &mut self,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
    ) {
        for event in events {
            let local_launch = self
                .need_producers
                .launches()
                .find(|launch| launch.task() == &event.task_id);
            if let Some(launch) = local_launch {
                if launch.generation() != event.generation {
                    self.fail_with_trap(
                        AwbcTrapCode::InternalInvariant,
                        "task event generation differs from the accepted Need producer launch"
                            .to_owned(),
                        None,
                        output,
                    );
                    continue;
                }
                if let TaskEventKind::Ready(value) = &event.kind {
                    let item_type =
                        self.awbc_type_id_for_semantic_identity(launch.plan().payload_type());
                    if item_type.is_none_or(|item_type| {
                        !crate::awbc::fiber::runtime_value_matches_type(
                            &self.program,
                            value.value(),
                            item_type,
                            0,
                        )
                    }) {
                        self.fail_with_trap(
                            AwbcTrapCode::HostAbiMismatch,
                            "Need producer Ready payload is outside its checked item type"
                                .to_owned(),
                            None,
                            output,
                        );
                        continue;
                    }
                }
                if let Err(error) = self.need_producers.publish_task_event(event) {
                    self.fail_with_trap(
                        AwbcTrapCode::InternalInvariant,
                        error.to_string(),
                        None,
                        output,
                    );
                }
                continue;
            }
            let cursor = crate::task::TaskPublicationCursor::from_event(event);
            if let Some(observed) = self.task_publications.get(&event.task_id) {
                match cursor.compare_same_source(*observed) {
                    Some(Ordering::Greater) => {}
                    Some(Ordering::Equal | Ordering::Less) => continue,
                    None => {
                        self.fail_with_trap(
                            AwbcTrapCode::InternalInvariant,
                            "task publication cursor changed source for one task".to_owned(),
                            None,
                            output,
                        );
                        continue;
                    }
                }
            }
            self.task_publications.insert(event.task_id.clone(), cursor);
            self.queued_task_events.push_back(event.clone());
        }
    }

    pub(super) fn resume_need(
        &mut self,
        need: &NeedId,
        item_type: crate::awbc::schema::AwbcTypeId,
        binding: Option<crate::awbc::schema::AwbcPatternId>,
        observer: Option<AwbcAwaitObserverResume>,
        resume: AwbcResumePointId,
        states: &[RuntimeNeedState],
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let local_launch = self
            .need_producers
            .launches()
            .find(|launch| launch.need() == need);
        let (cursor, state) = if let Some(launch) = local_launch {
            if states.iter().any(|state| state.need() == need) {
                self.fail_with_trap(
                    AwbcTrapCode::InternalInvariant,
                    "external Need state attempts to publish a Product-owned Need".to_owned(),
                    None,
                    output,
                );
                return true;
            }
            if let Some(fault) = launch.task_fault() {
                self.fail_with_trap(
                    AwbcTrapCode::HostAbiMismatch,
                    format!("Need producer task failed: {fault}"),
                    None,
                    output,
                );
                return true;
            }
            let Some(cursor) = launch.publication() else {
                return false;
            };
            (cursor, launch.state().clone())
        } else {
            let Some(state) = resolved_runtime_need_state(states, need) else {
                return false;
            };
            (
                crate::task::TaskPublicationCursor::from_need_state(state),
                state.state().clone(),
            )
        };
        let waiter = crate::runtime_id::RuntimePersistentFiberId::from_allocated(
            self.fiber.instance.get().get(),
        );
        let publication_key = (waiter, need.clone());
        if let Some(observed) = self.need_publications.get(&publication_key) {
            match cursor.compare_same_source(*observed) {
                Some(Ordering::Greater) => {}
                Some(Ordering::Equal | Ordering::Less) => return false,
                None => {
                    self.fail_with_trap(
                        AwbcTrapCode::InternalInvariant,
                        "Need publication cursor changed source for one Await".to_owned(),
                        None,
                        output,
                    );
                    return true;
                }
            }
        }
        self.need_publications.insert(publication_key, cursor);
        match &state {
            Need::NotStarted => false,
            Need::Pending(progress) => {
                output.flow_events.push(FlowEvent::AwaitProgress {
                    need: need.clone(),
                    progress: progress.clone(),
                });
                observer.is_some_and(|observer| {
                    self.resume_await_progress(observer, progress.clone(), output)
                })
            }
            Need::Ready(value) => {
                if !crate::awbc::fiber::runtime_value_matches_type(
                    &self.program,
                    value.value(),
                    item_type,
                    0,
                ) {
                    self.fail_with_trap(
                        AwbcTrapCode::HostAbiMismatch,
                        format!(
                            "Need {} published a Ready payload outside its checked item type",
                            need.0
                        ),
                        None,
                        output,
                    );
                    return true;
                }
                output.flow_events.push(FlowEvent::AwaitReady {
                    need: need.clone(),
                    value: value.clone(),
                });
                self.resume_await_value(binding, resume, value.value(), output)
            }
            Need::Cancelled => {
                let cancellation = cancel_fiber(&mut self.fiber);
                self.consume_observations(cancellation.observations, output);
                true
            }
        }
    }

    fn resume_await_progress(
        &mut self,
        observer: AwbcAwaitObserverResume,
        progress: arcweft_need::Progress,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let value = RuntimeValue::Progress(progress);
        if let Ok(frame) = self.fiber.active_frame_mut()
            && let Err(error) = frame.set_register(observer.destination, value)
        {
            self.fail_with_trap(AwbcTrapCode::TypeMismatch, error.to_string(), None, output);
            return true;
        }
        match self
            .fiber
            .resume_await_observer_at(&self.program, observer.resume)
        {
            Ok(()) => true,
            Err(error) => {
                self.fail_with_trap(
                    AwbcTrapCode::InternalInvariant,
                    error.to_string(),
                    None,
                    output,
                );
                false
            }
        }
    }

    fn resume_await_value(
        &mut self,
        binding: Option<crate::awbc::schema::AwbcPatternId>,
        resume: AwbcResumePointId,
        value: &RuntimeValue,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        if let Some(pattern) = binding
            && let Err(error) =
                crate::awbc::vm::bind_pattern(&self.program, &mut self.fiber, pattern, value)
        {
            self.fail_with_trap(
                AwbcTrapCode::PatternMismatch,
                error.to_string(),
                None,
                output,
            );
            return true;
        }
        self.resume_at(resume, output)
    }

    fn task_payload_accepts(
        &self,
        plan: crate::awbc::schema::AwbcTaskPlanId,
        value: &RuntimeValue,
    ) -> bool {
        self.program
            .task_plans
            .get(plan.index())
            .is_some_and(|record| {
                self.program
                    .validate_live_value(
                        record.payload_type,
                        value,
                        crate::entry::RuntimeSchemaLimits::engine_default(),
                    )
                    .is_ok()
            })
    }

    pub(super) fn fill_await_many(&mut self, output: &mut RuntimeStepOutput) {
        let Some((plan_id, limit, argument_count)) =
            self.fiber
                .suspension
                .as_ref()
                .and_then(|suspension| match &suspension.reason {
                    FiberSuspensionReason::AwaitMany(state) => self
                        .program
                        .task_plans
                        .get(state.plan.index())
                        .and_then(|plan| match &plan.kind {
                            crate::awbc::schema::AwbcTaskPlanKind::AwaitMany { limit, .. } => {
                                Some((state.plan, *limit as usize, plan.arguments.len()))
                            }
                            crate::awbc::schema::AwbcTaskPlanKind::NeedProducer { .. } => None,
                        }),
                    _ => None,
                })
        else {
            return;
        };
        let Some((base_task, base_need_id)) = self.task_plan_ids(plan_id) else {
            self.fail_with_trap(
                AwbcTrapCode::InternalInvariant,
                "AwaitMany suspension references a non-AwaitMany plan".to_owned(),
                None,
                output,
            );
            return;
        };
        let needs_invocation = self
            .fiber
            .suspension
            .as_ref()
            .and_then(|suspension| match &suspension.reason {
                FiberSuspensionReason::AwaitMany(state) => Some(state.invocation.is_none()),
                _ => None,
            })
            .unwrap_or(false);
        if needs_invocation {
            let invocation = match self
                .fiber
                .take_await_many_invocation(self.runtime_generation)
            {
                Ok(invocation) => invocation,
                Err(error) => {
                    self.fail_with_trap(
                        AwbcTrapCode::InternalInvariant,
                        error.to_string(),
                        None,
                        output,
                    );
                    return;
                }
            };
            if let Some(suspension) = self.fiber.suspension.as_mut()
                && let FiberSuspensionReason::AwaitMany(state) = &mut suspension.reason
            {
                state.invocation = Some(invocation);
            }
        }
        let Some(suspension) = self.fiber.suspension.as_mut() else {
            return;
        };
        let FiberSuspensionReason::AwaitMany(state) = &mut suspension.reason else {
            return;
        };
        if state.results.len() != state.items.len() {
            state.results = vec![None; state.items.len()];
        }
        let Some(invocation) = state.invocation else {
            self.fail_with_trap(
                AwbcTrapCode::InternalInvariant,
                "AwaitMany fan-out has no accepted occurrence identity".to_owned(),
                None,
                output,
            );
            return;
        };
        let mut quota_exhausted = false;
        while state.in_flight.len() < limit && (state.next_index as usize) < state.items.len() {
            if self.remaining_new_task_requests == 0 {
                quota_exhausted = true;
                break;
            }
            let index = state.next_index as usize;
            let task = match invocation.task_id(&TaskId(base_task.clone()), index) {
                Ok(task) => task,
                Err(error) => {
                    self.fail_with_trap(
                        AwbcTrapCode::InternalInvariant,
                        error.to_string(),
                        None,
                        output,
                    );
                    return;
                }
            };
            let need = match invocation.need_id(&base_need_id, index) {
                Ok(need) => need,
                Err(error) => {
                    self.fail_with_trap(
                        AwbcTrapCode::InternalInvariant,
                        error.to_string(),
                        None,
                        output,
                    );
                    return;
                }
            };
            let args = match argument_count {
                0 => Vec::new(),
                1 => vec![state.items[index].clone()],
                count => {
                    output.diagnostics.push(RuntimeDiagnostic::categorized(
                        RuntimeDiagnosticCategory::Input,
                        format!(
                            "await-many task `{base_task}` expects {count} arguments; item expansion supports zero or one"
                        ),
                    ));
                    return;
                }
            };
            let Ok(index_u32) = u32::try_from(index) else {
                output.diagnostics.push(RuntimeDiagnostic::categorized(
                    RuntimeDiagnosticCategory::Input,
                    format!("await-many task index {index} exceeds compact index range"),
                ));
                return;
            };
            match task_spec(&self.program, plan_id, &task, args) {
                Ok((_, spec)) => {
                    output.flow_events.push(FlowEvent::AwaitStarted {
                        need: need.clone(),
                        task: Some(task.clone()),
                    });
                    output.requests.tasks.push(spec);
                    state.in_flight.push(FiberAwaitManyInFlight {
                        index: index_u32,
                        task_id: task.0,
                        need_id: need.0,
                    });
                    let Some(next_index) = state.next_index.checked_add(1) else {
                        self.fail_with_trap(
                            AwbcTrapCode::InternalInvariant,
                            "AwaitMany item cursor overflowed".to_owned(),
                            None,
                            output,
                        );
                        return;
                    };
                    state.next_index = next_index;
                    self.remaining_new_task_requests -= 1;
                }
                Err(error) => {
                    output.diagnostics.push(RuntimeDiagnostic::categorized(
                        error.category(),
                        error.to_string(),
                    ));
                    return;
                }
            }
        }
        if quota_exhausted && state.in_flight.is_empty() {
            let message = "AwaitMany fan-out exceeds this step's task request quota".to_owned();
            output.diagnostics.push(RuntimeDiagnostic::categorized(
                RuntimeDiagnosticCategory::Budget,
                message.clone(),
            ));
            self.fail_with_trap(AwbcTrapCode::InternalInvariant, message, None, output);
        }
    }

    pub(super) fn resume_await_many(
        &mut self,
        mut state: crate::awbc::fiber::FiberAwaitManyState,
        resume: AwbcResumePointId,
        _events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
    ) -> bool {
        if state.results.len() != state.items.len() {
            state.results = vec![None; state.items.len()];
        }
        let in_flight_tasks = state
            .in_flight
            .iter()
            .map(|in_flight| in_flight.task_id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let events = self.take_await_many_task_events(&in_flight_tasks);
        let mut progressed = false;
        for event in &events {
            let Some(position) = state
                .in_flight
                .iter()
                .position(|in_flight| in_flight.task_id == event.task_id.0)
            else {
                continue;
            };
            match &event.kind {
                TaskEventKind::Ready(value) => {
                    if !self.task_payload_accepts(state.plan, value.value()) {
                        self.fail_with_trap(
                            AwbcTrapCode::HostAbiMismatch,
                            format!(
                                "await task {} published a payload outside its checked outcome contract",
                                event.task_id.0
                            ),
                            None,
                            output,
                        );
                        return true;
                    }
                    let in_flight = state.in_flight.remove(position);
                    state.results[in_flight.index as usize] = Some(value.value().clone());
                    output.flow_events.push(FlowEvent::AwaitReady {
                        need: NeedId(in_flight.need_id),
                        value: value.clone(),
                    });
                    progressed = true;
                }
                TaskEventKind::Progress(progress) => {
                    output.flow_events.push(FlowEvent::AwaitProgress {
                        need: NeedId(state.in_flight[position].need_id.clone()),
                        progress: progress.clone(),
                    });
                    progressed = true;
                }
                TaskEventKind::Failed(error) => {
                    self.fail_with_trap(
                        AwbcTrapCode::HostAbiMismatch,
                        format!(
                            "await task {} at index {} failed: {}",
                            event.task_id.0, state.in_flight[position].index, error
                        ),
                        None,
                        output,
                    );
                    return true;
                }
                TaskEventKind::Cancelled => {
                    let cancellation = cancel_fiber(&mut self.fiber);
                    self.consume_observations(cancellation.observations, output);
                    return true;
                }
            }
        }
        if state.in_flight.is_empty()
            && state.next_index as usize >= state.items.len()
            && state.results.iter().all(Option::is_some)
        {
            let values = state
                .results
                .iter()
                .filter_map(Clone::clone)
                .collect::<Vec<_>>();
            let value = runtime_sequence_values(values);
            if let Some(pattern) = state.binding
                && let Err(error) =
                    crate::awbc::vm::bind_pattern(&self.program, &mut self.fiber, pattern, &value)
            {
                self.fail_with_trap(
                    AwbcTrapCode::PatternMismatch,
                    error.to_string(),
                    None,
                    output,
                );
                return true;
            }
            let Some((_, need)) = self.task_plan_ids(state.plan) else {
                self.fail_with_trap(
                    AwbcTrapCode::InternalInvariant,
                    "AwaitMany completion references a non-AwaitMany plan".to_owned(),
                    None,
                    output,
                );
                return true;
            };
            output.flow_events.push(FlowEvent::AwaitReady {
                need,
                value: RuntimePayload::from(value),
            });
            return self.resume_at(resume, output);
        }
        if let Some(suspension) = self.fiber.suspension.as_mut() {
            suspension.reason = FiberSuspensionReason::AwaitMany(state);
        }
        self.fill_await_many(output);
        progressed || !output.requests.tasks.is_empty()
    }

    pub(super) fn emit_host_call(
        &mut self,
        call: AwbcHostCallId,
        args: &[RuntimeValue],
        output: &mut RuntimeStepOutput,
    ) {
        if self
            .pending_host_call
            .as_ref()
            .is_some_and(|pending| pending.call == call)
        {
            return;
        }
        let Some(record) = self.program.host_calls.get(call.index()) else {
            self.record_error(
                ProductStepError::Internal(format!("missing AWBC host call {}", call.0)),
                output,
            );
            return;
        };
        let Some(signature) = self.program.signatures.get(record.signature.index()) else {
            self.record_error(
                ProductStepError::Internal(format!("AWBC host call {} has no signature", call.0)),
                output,
            );
            return;
        };
        let result =
            match signature.result {
                Some(result) => self.program.runtime_types.get(result.index()),
                None => self.program.runtime_types.iter().find(|ty| {
                    matches!(ty.shape(), crate::awbc::schema::AwbcRuntimeTypeShape::Unit)
                }),
            };
        let Some(result) = result else {
            self.record_error(
                ProductStepError::Internal(format!(
                    "AWBC host call {} result type is absent from the selected program",
                    call.0
                )),
                output,
            );
            return;
        };
        let public_id = self
            .program
            .strings
            .get(record.public_id.index())
            .cloned()
            .unwrap_or_else(|| format!("awbc.host_call.{}", call.0));
        let sequence = self.next_host_call_sequence;
        self.next_host_call_sequence = self.next_host_call_sequence.saturating_add(1);
        let id = RuntimeHostCallId(if sequence == 0 {
            public_id.clone()
        } else {
            format!("{public_id}.{sequence}")
        });
        let mut positional = Vec::new();
        let mut named_args = Vec::new();
        for (descriptor, value) in record.arguments.iter().zip(args) {
            if descriptor.spread {
                let Ok(values) = runtime_value_into_sequence_values(value.clone()) else {
                    self.record_error(
                        ProductStepError::Host(format!(
                            "spread host argument requires a tuple or bracket sequence, found {}",
                            runtime_value_label(value)
                        )),
                        output,
                    );
                    return;
                };
                positional.extend(values.into_iter().map(RuntimePayload::from));
            } else if let Some(name) = descriptor.name {
                let name = self
                    .program
                    .strings
                    .get(name.index())
                    .cloned()
                    .unwrap_or_else(|| format!("argument.{}", name.0));
                named_args.push(NamedHostArg {
                    name,
                    value: RuntimePayload::from(value.clone()),
                });
            } else {
                positional.push(RuntimePayload::from(value.clone()));
            }
        }
        output.requests.host_calls.push(RuntimeHostCallRequest {
            id: id.clone(),
            public_id,
            capability: self
                .program
                .strings
                .get(record.capability.index())
                .cloned()
                .unwrap_or_else(|| "host".to_owned()),
            operation: self
                .program
                .strings
                .get(record.operation.index())
                .cloned()
                .unwrap_or_else(|| "call".to_owned()),
            contract: record.contract,
            args: positional,
            named_args,
            result: result.semantic_identity(),
            mode: match record.mode {
                AwbcHostCallMode::Immediate => RuntimeHostCallMode::Immediate,
                AwbcHostCallMode::Suspend => RuntimeHostCallMode::Suspend,
            },
            deterministic: record.deterministic,
        });
        self.pending_host_call = Some(PendingHostCall { call, id });
    }

    pub(super) fn resume_host_call(
        &mut self,
        call: AwbcHostCallId,
        destination: Option<crate::awbc::schema::AwbcRegisterId>,
        resume: AwbcResumePointId,
        results: &[crate::step::RuntimeHostCallResult],
        output: &mut RuntimeStepOutput,
    ) -> bool {
        if self.pending_host_call.is_none() {
            let args = self
                .fiber
                .suspension
                .as_ref()
                .and_then(|suspension| match &suspension.reason {
                    FiberSuspensionReason::HostCall { args, .. } => Some(args.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            self.emit_host_call(call, &args, output);
        }
        let Some(pending) = self.pending_host_call.clone() else {
            return false;
        };
        let Some(result) = results.iter().find(|result| result.id == pending.id) else {
            return false;
        };
        if pending.call != call {
            self.pending_host_call = None;
            self.fail_with_trap(
                AwbcTrapCode::HostAbiMismatch,
                "pending host call does not match the selected continuation".to_owned(),
                None,
                output,
            );
            return true;
        }
        match &result.outcome {
            Ok(value) => {
                let checked = self
                    .program
                    .host_calls
                    .get(call.index())
                    .and_then(|record| self.program.signatures.get(record.signature.index()))
                    .is_some_and(|signature| match signature.result {
                        Some(ty) => self
                            .program
                            .validate_live_value(
                                ty,
                                value.value(),
                                crate::entry::RuntimeSchemaLimits::engine_default(),
                            )
                            .is_ok(),
                        None => value.value() == &crate::value::RuntimeValue::Unit,
                    });
                if !checked {
                    self.pending_host_call = None;
                    self.fail_with_trap(
                        AwbcTrapCode::HostAbiMismatch,
                        "host-call result does not satisfy the selected program type".to_owned(),
                        None,
                        output,
                    );
                    return true;
                }
                if let Some(destination) = destination
                    && let Ok(frame) = self.fiber.active_frame_mut()
                    && let Err(error) = frame.set_register(destination, value.value().clone())
                {
                    self.fail_with_error(ProductStepError::Internal(error.to_string()), output);
                    return true;
                }
                self.pending_host_call = None;
                self.resume_at(resume, output)
            }
            Err(error) => {
                self.pending_host_call = None;
                self.fail_with_trap(
                    match error.kind {
                        crate::step::RuntimeHostCallErrorKind::UnsupportedCapability => {
                            AwbcTrapCode::CapabilityDenied
                        }
                        crate::step::RuntimeHostCallErrorKind::Rejected
                        | crate::step::RuntimeHostCallErrorKind::Failed => {
                            AwbcTrapCode::HostAbiMismatch
                        }
                    },
                    error.message.clone(),
                    None,
                    output,
                );
                true
            }
        }
    }

    pub(super) fn initialize_deferred_child_suspension(
        &mut self,
        child: &mut super::ProductChildFiber,
        output: &mut RuntimeStepOutput,
    ) -> Result<(), ProductStepError> {
        let Some(suspension) = child.fiber.suspension.clone() else {
            return Ok(());
        };
        if let FiberSuspensionReason::Await {
            target: FiberAwaitTarget::Need { id, .. },
            ..
        } = &suspension.reason
        {
            let task = self
                .need_producers
                .launches()
                .find(|launch| launch.need() == id)
                .map(|launch| launch.task().clone());
            output.flow_events.push(FlowEvent::AwaitStarted {
                need: id.clone(),
                task,
            });
        }
        match suspension.reason {
            FiberSuspensionReason::Await {
                target: FiberAwaitTarget::Need { .. },
                ..
            }
            | FiberSuspensionReason::BudgetYield => {}
            FiberSuspensionReason::AwaitMany(_) => {
                let diagnostics = output.diagnostics.len();
                self.fill_await_many_for_fiber(&mut child.fiber, output);
                if output.diagnostics.len() > diagnostics {
                    return Err(ProductStepError::Internal(
                        output.diagnostics.last().map_or_else(
                            || "deferred await-many request failed".to_owned(),
                            |d| d.message.clone(),
                        ),
                    ));
                }
            }
            FiberSuspensionReason::HostCall { call, args, .. } => {
                if child.pending_host_call.is_none() {
                    let parent_pending = self.pending_host_call.take();
                    self.emit_host_call(call, &args, output);
                    child.pending_host_call = self.pending_host_call.take();
                    self.pending_host_call = parent_pending;
                    if child.pending_host_call.is_none() {
                        return Err(ProductStepError::Internal(format!(
                            "deferred child host call {} did not produce a pending request",
                            call.0
                        )));
                    }
                }
            }
            FiberSuspensionReason::Dialogue { .. } | FiberSuspensionReason::Choice { .. } => {
                return Err(ProductStepError::Internal(
                    "deferred child suspended on a product-owned dialogue or choice boundary"
                        .to_owned(),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn resume_deferred_child_suspension(
        &mut self,
        child: &mut super::ProductChildFiber,
        need_states: &[RuntimeNeedState],
        host_results: &[crate::step::RuntimeHostCallResult],
        output: &mut RuntimeStepOutput,
    ) -> Result<bool, ProductStepError> {
        let Some(suspension) = child.fiber.suspension.clone() else {
            return Ok(false);
        };
        let Some(resume) = suspension.declared_resume() else {
            return match suspension.reason {
                FiberSuspensionReason::BudgetYield => {
                    child
                        .fiber
                        .resume_budget_yield(&self.program)
                        .map_err(|error| ProductStepError::Internal(error.to_string()))?;
                    child.fiber.replenish_budget();
                    Ok(true)
                }
                _ => Err(ProductStepError::Internal(
                    "deferred child suspension has no declared resume point".to_owned(),
                )),
            };
        };
        match suspension.reason {
            FiberSuspensionReason::Await {
                target:
                    FiberAwaitTarget::Need {
                        id: need,
                        item_type,
                        ..
                    },
                binding,
                observer,
            } => Ok(self.resume_deferred_await_need(
                &mut child.fiber,
                &need,
                item_type,
                binding,
                observer,
                resume,
                need_states,
                output,
            )),
            FiberSuspensionReason::AwaitMany(state) => {
                Ok(self.resume_deferred_await_many(&mut child.fiber, state, resume, output))
            }
            FiberSuspensionReason::HostCall {
                call, destination, ..
            } => Ok(self.resume_deferred_host_call(
                child,
                call,
                destination,
                resume,
                host_results,
                output,
            )),
            FiberSuspensionReason::Dialogue { .. } | FiberSuspensionReason::Choice { .. } => {
                Err(ProductStepError::Internal(
                    "deferred child suspended on a product-owned dialogue or choice boundary"
                        .to_owned(),
                ))
            }
            FiberSuspensionReason::BudgetYield => Ok(false),
        }
    }

    fn resume_deferred_await_need(
        &mut self,
        fiber: &mut FiberState,
        need: &NeedId,
        item_type: crate::awbc::schema::AwbcTypeId,
        binding: Option<crate::awbc::schema::AwbcPatternId>,
        observer: Option<AwbcAwaitObserverResume>,
        resume: AwbcResumePointId,
        states: &[RuntimeNeedState],
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let local_launch = self
            .need_producers
            .launches()
            .find(|launch| launch.need() == need);
        let (cursor, state) = if let Some(launch) = local_launch {
            if states.iter().any(|state| state.need() == need) {
                mark_child_trapped(
                    fiber,
                    AwbcTrapCode::InternalInvariant,
                    "external Need state attempts to publish a Product-owned Need".to_owned(),
                );
                return true;
            }
            if let Some(fault) = launch.task_fault() {
                mark_child_trapped(
                    fiber,
                    AwbcTrapCode::HostAbiMismatch,
                    format!("Need producer task failed: {fault}"),
                );
                return true;
            }
            let Some(cursor) = launch.publication() else {
                return false;
            };
            (cursor, launch.state().clone())
        } else {
            let Some(state) = resolved_runtime_need_state(states, need) else {
                return false;
            };
            (
                crate::task::TaskPublicationCursor::from_need_state(state),
                state.state().clone(),
            )
        };
        let waiter =
            crate::runtime_id::RuntimePersistentFiberId::from_allocated(fiber.instance.get().get());
        let publication_key = (waiter, need.clone());
        if let Some(observed) = self.need_publications.get(&publication_key) {
            match cursor.compare_same_source(*observed) {
                Some(Ordering::Greater) => {}
                Some(Ordering::Equal | Ordering::Less) => return false,
                None => {
                    mark_child_trapped(
                        fiber,
                        AwbcTrapCode::InternalInvariant,
                        "Need publication cursor changed source for one Await".to_owned(),
                    );
                    return true;
                }
            }
        }
        self.need_publications.insert(publication_key, cursor);
        match &state {
            Need::NotStarted => false,
            Need::Pending(progress) => {
                output.flow_events.push(FlowEvent::AwaitProgress {
                    need: need.clone(),
                    progress: progress.clone(),
                });
                observer.is_some_and(|observer| {
                    resume_deferred_await_progress(
                        &self.program,
                        fiber,
                        observer,
                        progress.clone(),
                        output,
                    )
                })
            }
            Need::Ready(value) => {
                if !crate::awbc::fiber::runtime_value_matches_type(
                    &self.program,
                    value.value(),
                    item_type,
                    0,
                ) {
                    mark_child_trapped(
                        fiber,
                        AwbcTrapCode::HostAbiMismatch,
                        format!(
                            "Need {} published a Ready payload outside its checked item type",
                            need.0
                        ),
                    );
                    return true;
                }
                output.flow_events.push(FlowEvent::AwaitReady {
                    need: need.clone(),
                    value: value.clone(),
                });
                resume_deferred_await_value(
                    &self.program,
                    fiber,
                    binding,
                    resume,
                    value.value(),
                    output,
                )
            }
            Need::Cancelled => {
                let cancellation = cancel_fiber(fiber);
                self.consume_observations(cancellation.observations, output);
                true
            }
        }
    }

    fn resume_deferred_host_call(
        &mut self,
        child: &mut super::ProductChildFiber,
        call: AwbcHostCallId,
        destination: Option<crate::awbc::schema::AwbcRegisterId>,
        resume: AwbcResumePointId,
        results: &[crate::step::RuntimeHostCallResult],
        output: &mut RuntimeStepOutput,
    ) -> bool {
        if child.pending_host_call.is_none() {
            let args = child
                .fiber
                .suspension
                .as_ref()
                .and_then(|suspension| match &suspension.reason {
                    FiberSuspensionReason::HostCall { args, .. } => Some(args.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            let parent_pending = self.pending_host_call.take();
            self.emit_host_call(call, &args, output);
            child.pending_host_call = self.pending_host_call.take();
            self.pending_host_call = parent_pending;
        }
        let Some(pending) = child.pending_host_call.clone() else {
            return false;
        };
        let Some(result) = results.iter().find(|result| result.id == pending.id) else {
            return false;
        };
        if pending.call != call {
            child.pending_host_call = None;
            mark_child_trapped(
                &mut child.fiber,
                AwbcTrapCode::HostAbiMismatch,
                "pending deferred child host call does not match its continuation".to_owned(),
            );
            return true;
        }
        match &result.outcome {
            Ok(value) => {
                let checked = self
                    .program
                    .host_calls
                    .get(call.index())
                    .and_then(|record| self.program.signatures.get(record.signature.index()))
                    .is_some_and(|signature| match signature.result {
                        Some(ty) => self
                            .program
                            .validate_live_value(
                                ty,
                                value.value(),
                                crate::entry::RuntimeSchemaLimits::engine_default(),
                            )
                            .is_ok(),
                        None => value.value() == &RuntimeValue::Unit,
                    });
                if !checked {
                    child.pending_host_call = None;
                    mark_child_trapped(
                        &mut child.fiber,
                        AwbcTrapCode::HostAbiMismatch,
                        "deferred child host-call result violates its checked type".to_owned(),
                    );
                    return true;
                }
                if let Some(destination) = destination
                    && let Ok(frame) = child.fiber.active_frame_mut()
                    && let Err(error) = frame.set_register(destination, value.value().clone())
                {
                    child.pending_host_call = None;
                    mark_child_trapped(
                        &mut child.fiber,
                        AwbcTrapCode::TypeMismatch,
                        error.to_string(),
                    );
                    return true;
                }
                child.pending_host_call = None;
                if let Err(error) = child.fiber.resume_at(&self.program, resume) {
                    mark_child_trapped(
                        &mut child.fiber,
                        AwbcTrapCode::InternalInvariant,
                        error.to_string(),
                    );
                }
                true
            }
            Err(error) => {
                child.pending_host_call = None;
                mark_child_trapped(
                    &mut child.fiber,
                    match error.kind {
                        crate::step::RuntimeHostCallErrorKind::UnsupportedCapability => {
                            AwbcTrapCode::CapabilityDenied
                        }
                        crate::step::RuntimeHostCallErrorKind::Rejected
                        | crate::step::RuntimeHostCallErrorKind::Failed => {
                            AwbcTrapCode::HostAbiMismatch
                        }
                    },
                    error.message.clone(),
                );
                true
            }
        }
    }

    fn fill_await_many_for_fiber(
        &mut self,
        fiber: &mut FiberState,
        output: &mut RuntimeStepOutput,
    ) {
        let Some((plan_id, limit, argument_count)) =
            fiber
                .suspension
                .as_ref()
                .and_then(|suspension| match &suspension.reason {
                    FiberSuspensionReason::AwaitMany(state) => self
                        .program
                        .task_plans
                        .get(state.plan.index())
                        .and_then(|plan| match &plan.kind {
                            crate::awbc::schema::AwbcTaskPlanKind::AwaitMany { limit, .. } => {
                                Some((state.plan, *limit as usize, plan.arguments.len()))
                            }
                            crate::awbc::schema::AwbcTaskPlanKind::NeedProducer { .. } => None,
                        }),
                    _ => None,
                })
        else {
            return;
        };
        let Some((base_task, base_need_id)) = self.task_plan_ids(plan_id) else {
            output.diagnostics.push(RuntimeDiagnostic::categorized(
                RuntimeDiagnosticCategory::Internal,
                "AwaitMany child suspension references a non-AwaitMany plan",
            ));
            return;
        };
        let needs_invocation = fiber
            .suspension
            .as_ref()
            .and_then(|suspension| match &suspension.reason {
                FiberSuspensionReason::AwaitMany(state) => Some(state.invocation.is_none()),
                _ => None,
            })
            .unwrap_or(false);
        if needs_invocation {
            let invocation = match fiber.take_await_many_invocation(self.runtime_generation) {
                Ok(invocation) => invocation,
                Err(error) => {
                    mark_child_trapped(fiber, AwbcTrapCode::InternalInvariant, error.to_string());
                    return;
                }
            };
            if let Some(suspension) = fiber.suspension.as_mut()
                && let FiberSuspensionReason::AwaitMany(state) = &mut suspension.reason
            {
                state.invocation = Some(invocation);
            }
        }
        let Some(suspension) = fiber.suspension.as_mut() else {
            return;
        };
        let FiberSuspensionReason::AwaitMany(state) = &mut suspension.reason else {
            return;
        };
        if state.results.len() != state.items.len() {
            state.results = vec![None; state.items.len()];
        }
        let Some(invocation) = state.invocation else {
            mark_child_trapped(
                fiber,
                AwbcTrapCode::InternalInvariant,
                "AwaitMany fan-out has no accepted occurrence identity".to_owned(),
            );
            return;
        };
        let mut quota_exhausted = false;
        while state.in_flight.len() < limit && (state.next_index as usize) < state.items.len() {
            if self.remaining_new_task_requests == 0 {
                quota_exhausted = true;
                break;
            }
            let index = state.next_index as usize;
            let task = match invocation.task_id(&TaskId(base_task.clone()), index) {
                Ok(task) => task,
                Err(error) => {
                    mark_child_trapped(fiber, AwbcTrapCode::InternalInvariant, error.to_string());
                    return;
                }
            };
            let need = match invocation.need_id(&base_need_id, index) {
                Ok(need) => need,
                Err(error) => {
                    mark_child_trapped(fiber, AwbcTrapCode::InternalInvariant, error.to_string());
                    return;
                }
            };
            let args = match argument_count {
                0 => Vec::new(),
                1 => vec![state.items[index].clone()],
                count => {
                    output.diagnostics.push(RuntimeDiagnostic::categorized(
                        RuntimeDiagnosticCategory::Input,
                        format!(
                            "await-many task `{base_task}` expects {count} arguments; item expansion supports zero or one"
                        ),
                    ));
                    return;
                }
            };
            let Ok(index_u32) = u32::try_from(index) else {
                output.diagnostics.push(RuntimeDiagnostic::categorized(
                    RuntimeDiagnosticCategory::Input,
                    format!("await-many task index {index} exceeds compact index range"),
                ));
                return;
            };
            match task_spec(&self.program, plan_id, &task, args) {
                Ok((_, spec)) => {
                    output.flow_events.push(FlowEvent::AwaitStarted {
                        need: need.clone(),
                        task: Some(task.clone()),
                    });
                    output.requests.tasks.push(spec);
                    state.in_flight.push(FiberAwaitManyInFlight {
                        index: index_u32,
                        task_id: task.0,
                        need_id: need.0,
                    });
                    let Some(next_index) = state.next_index.checked_add(1) else {
                        mark_child_trapped(
                            fiber,
                            AwbcTrapCode::InternalInvariant,
                            "AwaitMany item cursor overflowed".to_owned(),
                        );
                        return;
                    };
                    state.next_index = next_index;
                    self.remaining_new_task_requests -= 1;
                }
                Err(error) => {
                    output.diagnostics.push(RuntimeDiagnostic::categorized(
                        error.category(),
                        error.to_string(),
                    ));
                    return;
                }
            }
        }
        if quota_exhausted && state.in_flight.is_empty() {
            let message = "AwaitMany fan-out exceeds this step's task request quota".to_owned();
            output.diagnostics.push(RuntimeDiagnostic::categorized(
                RuntimeDiagnosticCategory::Budget,
                message.clone(),
            ));
            mark_child_trapped(fiber, AwbcTrapCode::InternalInvariant, message);
        }
    }

    fn resume_deferred_await_many(
        &mut self,
        fiber: &mut FiberState,
        mut state: crate::awbc::fiber::FiberAwaitManyState,
        resume: AwbcResumePointId,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        if state.results.len() != state.items.len() {
            state.results = vec![None; state.items.len()];
        }
        let in_flight_tasks = state
            .in_flight
            .iter()
            .map(|in_flight| in_flight.task_id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let events = self.take_await_many_task_events(&in_flight_tasks);
        let mut progressed = false;
        for event in &events {
            let Some(position) = state
                .in_flight
                .iter()
                .position(|in_flight| in_flight.task_id == event.task_id.0)
            else {
                continue;
            };
            match &event.kind {
                TaskEventKind::Ready(value) => {
                    if !self.task_payload_accepts(state.plan, value.value()) {
                        mark_child_trapped(
                            fiber,
                            AwbcTrapCode::HostAbiMismatch,
                            format!(
                                "await task {} published a payload outside its checked outcome contract",
                                event.task_id.0
                            ),
                        );
                        return true;
                    }
                    let in_flight = state.in_flight.remove(position);
                    state.results[in_flight.index as usize] = Some(value.value().clone());
                    output.flow_events.push(FlowEvent::AwaitReady {
                        need: NeedId(in_flight.need_id),
                        value: value.clone(),
                    });
                    progressed = true;
                }
                TaskEventKind::Progress(progress) => {
                    output.flow_events.push(FlowEvent::AwaitProgress {
                        need: NeedId(state.in_flight[position].need_id.clone()),
                        progress: progress.clone(),
                    });
                    progressed = true;
                }
                TaskEventKind::Failed(error) => {
                    mark_child_trapped(
                        fiber,
                        AwbcTrapCode::HostAbiMismatch,
                        format!(
                            "await task {} at index {} failed: {}",
                            event.task_id.0, state.in_flight[position].index, error
                        ),
                    );
                    return true;
                }
                TaskEventKind::Cancelled => {
                    let cancellation = cancel_fiber(fiber);
                    self.consume_observations(cancellation.observations, output);
                    return true;
                }
            }
        }
        if state.in_flight.is_empty()
            && state.next_index as usize >= state.items.len()
            && state.results.iter().all(Option::is_some)
        {
            let values = state
                .results
                .iter()
                .filter_map(Clone::clone)
                .collect::<Vec<_>>();
            let value = runtime_sequence_values(values);
            if let Some(pattern) = state.binding
                && let Err(error) =
                    crate::awbc::vm::bind_pattern(&self.program, fiber, pattern, &value)
            {
                mark_child_trapped(fiber, AwbcTrapCode::PatternMismatch, error.to_string());
                return true;
            }
            let Some((_, need)) = self.task_plan_ids(state.plan) else {
                mark_child_trapped(
                    fiber,
                    AwbcTrapCode::InternalInvariant,
                    "AwaitMany completion references a non-AwaitMany plan".to_owned(),
                );
                return true;
            };
            output.flow_events.push(FlowEvent::AwaitReady {
                need,
                value: RuntimePayload::from(value),
            });
            if let Err(error) = fiber.resume_at(&self.program, resume) {
                mark_child_trapped(fiber, AwbcTrapCode::InternalInvariant, error.to_string());
            }
            return true;
        }
        if let Some(suspension) = fiber.suspension.as_mut() {
            suspension.reason = FiberSuspensionReason::AwaitMany(state);
        }
        let request_count = output.requests.tasks.len();
        let diagnostic_count = output.diagnostics.len();
        self.fill_await_many_for_fiber(fiber, output);
        if output.diagnostics.len() > diagnostic_count {
            mark_child_trapped(
                fiber,
                AwbcTrapCode::InternalInvariant,
                output.diagnostics.last().map_or_else(
                    || "deferred await-many request failed".to_owned(),
                    |d| d.message.clone(),
                ),
            );
            return true;
        }
        progressed || output.requests.tasks.len() > request_count
    }

    fn take_await_many_task_events(
        &mut self,
        in_flight_tasks: &std::collections::BTreeSet<String>,
    ) -> Vec<TaskEvent> {
        let mut events = Vec::new();
        let mut index = 0;
        while index < self.queued_task_events.len() {
            if in_flight_tasks.contains(&self.queued_task_events[index].task_id.0) {
                if let Some(event) = self.queued_task_events.remove(index) {
                    events.push(event);
                }
            } else {
                index += 1;
            }
        }
        events
    }

    pub(super) fn consume_observations(
        &mut self,
        observations: Vec<VmObservation>,
        output: &mut RuntimeStepOutput,
    ) {
        for observation in observations {
            match observation {
                VmObservation::Instruction { .. } => {}
                VmObservation::Effect { effect, args } => self.emit_effect(effect, &args, output),
                VmObservation::EnsureContent(content) => {
                    if self.emitted_content.insert(content) {
                        match content_request(&self.program, content) {
                            Ok(request) => output.requests.ensure_content.push(request),
                            Err(error) => self.record_error(error, output),
                        }
                    }
                }
                VmObservation::NeedProducerStarted {
                    cursor,
                    fiber,
                    dst,
                    plan,
                    args,
                } => {
                    if self.fiber.instance.get().get() == fiber.get() {
                        self.commit_need_producer_started(cursor, fiber, dst, plan, args, output);
                    } else {
                        self.commit_child_need_producer_started(
                            cursor, fiber, dst, plan, args, output,
                        );
                    }
                }
                VmObservation::Goto(target) => match self.flow_identity_for_function(target) {
                    Ok(target) => output.flow_events.push(FlowEvent::Goto { target }),
                    Err(error) => self.record_error(error, output),
                },
                VmObservation::FiberSpawned { function, args, .. } => {
                    self.spawn_child(function, &args, output);
                }
                VmObservation::StreamYield { stream, value } => {
                    let sequence = self.stream_sequences.entry(stream).or_default();
                    output.effects.stream_events.push(RuntimeStreamEvent {
                        stream: stream_id_for(&self.program, stream),
                        sequence: TaskSequence(*sequence),
                        kind: StreamEventKind::Item(RuntimePayload::from(value.clone())),
                    });
                    *sequence = sequence.saturating_add(1);
                    if let Some(state) = self
                        .facade_fiber
                        .stream_states
                        .get_mut(&stream_id_for(&self.program, stream))
                    {
                        state.push_item(RuntimePayload::from(value));
                    }
                }
                VmObservation::StreamClose(stream) => {
                    let stream_id = stream_id_for(&self.program, stream);
                    if let Some(state) = self.facade_fiber.stream_states.get_mut(&stream_id)
                        && let Some(sequence) = state.close_with_sequence()
                    {
                        output.effects.stream_events.push(RuntimeStreamEvent {
                            stream: stream_id,
                            sequence,
                            kind: StreamEventKind::End,
                        });
                    }
                }
                VmObservation::LineOperation { .. }
                | VmObservation::DialogueResult { .. }
                | VmObservation::LineDeferRegistration { .. }
                | VmObservation::ScopedDeferRegistration { .. }
                | VmObservation::ScopedDeferUnwind { .. }
                | VmObservation::ScopedDeferFailure(_)
                | VmObservation::Drop { .. } => self.fail_with_error(
                    crate::line_task::LineRuntimeError::InvalidActivationOperation.into(),
                    output,
                ),
                VmObservation::Trap(trap) => self.record_trap(&trap, output),
            }
        }
    }

    pub(super) fn emit_effect(
        &mut self,
        effect: AwbcEffectPlanId,
        args: &[RuntimeValue],
        output: &mut RuntimeStepOutput,
    ) {
        let Some(plan) = self.program.effect_plans.get(effect.index()) else {
            self.record_error(
                ProductStepError::Internal(format!("missing AWBC effect plan {}", effect.0)),
                output,
            );
            return;
        };
        match plan.kind.map_product_effect(&self.program, effect, args) {
            MappedEffect::Omitted => {}
            MappedEffect::Line(effect) => output.effects.line.push(effect),
            MappedEffect::Audio(command) => output.requests.audio.push(AudioCommandEnvelope::new(
                self.next_audio_dispatch(),
                command,
            )),
            MappedEffect::Unsupported(diagnostic) => output.diagnostics.push(diagnostic),
        }
    }

    pub(super) fn next_audio_dispatch(&mut self) -> AudioDispatchId {
        let dispatch = AudioDispatchId::new(0, self.next_audio_sequence);
        self.next_audio_sequence = self.next_audio_sequence.saturating_add(1);
        dispatch
    }

    pub(super) fn spawn_child(
        &mut self,
        function: AwbcFunctionId,
        args: &[RuntimeValue],
        output: &mut RuntimeStepOutput,
    ) {
        self.spawn_owned_child(
            super::ProductChildFiberOwner::Independent,
            function,
            args,
            output,
        );
    }

    pub(super) fn spawn_owned_child(
        &mut self,
        owner: super::ProductChildFiberOwner,
        function: AwbcFunctionId,
        args: &[RuntimeValue],
        output: &mut RuntimeStepOutput,
    ) {
        let Some(next_generation) = self.next_generation.checked_add(1) else {
            self.record_error(ProductStepError::ChildGenerationOverflow, output);
            return;
        };
        let mut next_fiber_instance = self.next_fiber_instance;
        let fiber_instance = match next_fiber_instance
            .take_next(crate::runtime_id::RuntimeIdNamespace::FiberInstance)
        {
            Ok(instance) => crate::runtime_id::RuntimeFiberInstanceId::from_allocated(instance),
            Err(error) => {
                self.record_error(ProductStepError::RuntimeIdentity(error), output);
                return;
            }
        };
        match FiberState::for_function_with_instance(
            &self.program,
            self.fiber.entry,
            function,
            fiber_instance,
            self.next_generation,
            self.fiber.budget.quantum.max(1),
        ) {
            Ok(mut child) => {
                match child
                    .active_frame_mut()
                    .and_then(|frame| frame.bind_positional_arguments(&self.program, args))
                {
                    Ok(()) => {
                        self.next_generation = next_generation;
                        self.next_fiber_instance = next_fiber_instance;
                        self.child_fibers.push_back(super::ProductChildFiber {
                            owner,
                            fiber: child,
                            runtime_generation: self.runtime_generation,
                            pending_host_call: None,
                        });
                    }
                    Err(error) => {
                        self.record_error(ProductStepError::Type(error.to_string()), output);
                    }
                }
            }
            Err(error) => self.record_error(ProductStepError::Internal(error.to_string()), output),
        }
    }

    fn commit_need_producer_started(
        &mut self,
        cursor: crate::awbc::fiber::FiberCursor,
        fiber: crate::runtime_id::RuntimePersistentFiberId,
        dst: crate::awbc::schema::AwbcRegisterId,
        plan_id: crate::awbc::schema::AwbcTaskPlanId,
        values: Vec<RuntimeValue>,
        output: &mut RuntimeStepOutput,
    ) {
        let mut candidate_fiber = self.fiber.clone();
        let mut candidate_registry = self.need_producers.clone();
        let started = self.stage_need_producer_started(
            &mut candidate_fiber,
            &mut candidate_registry,
            self.runtime_generation,
            cursor,
            fiber,
            dst,
            plan_id,
            values,
        );
        let started = match started {
            Ok(started) => started,
            Err(error) => {
                self.fail_with_error(error, output);
                return;
            }
        };
        if started.is_some() && self.remaining_new_task_requests == 0 {
            self.fail_with_budget_error(
                "Need producer start exceeds this step's task request quota".to_owned(),
                output,
            );
            return;
        }
        self.fiber = candidate_fiber;
        self.need_producers = candidate_registry;
        if let Some(started) = started {
            self.remaining_new_task_requests -= 1;
            output.requests.tasks.push(started.spec);
        }
    }

    fn commit_child_need_producer_started(
        &mut self,
        cursor: crate::awbc::fiber::FiberCursor,
        persistent_fiber: crate::runtime_id::RuntimePersistentFiberId,
        dst: crate::awbc::schema::AwbcRegisterId,
        plan_id: crate::awbc::schema::AwbcTaskPlanId,
        values: Vec<RuntimeValue>,
        output: &mut RuntimeStepOutput,
    ) {
        let Some(index) = self
            .child_fibers
            .iter()
            .position(|child| child.fiber.instance.get().get() == persistent_fiber.get())
        else {
            self.fail_with_error(
                ProductStepError::Internal(
                    "Need producer observation references an absent Product child fiber".to_owned(),
                ),
                output,
            );
            return;
        };
        let mut candidate_fiber = self.child_fibers[index].fiber.clone();
        let mut candidate_registry = self.need_producers.clone();
        let started = self.stage_need_producer_started(
            &mut candidate_fiber,
            &mut candidate_registry,
            self.runtime_generation,
            cursor,
            persistent_fiber,
            dst,
            plan_id,
            values,
        );
        let started = match started {
            Ok(started) => started,
            Err(error) => {
                self.fail_with_error(error, output);
                return;
            }
        };
        if started.is_some() && self.remaining_new_task_requests == 0 {
            self.fail_with_budget_error(
                "Need producer start exceeds this step's task request quota".to_owned(),
                output,
            );
            return;
        }
        self.child_fibers[index].fiber = candidate_fiber;
        self.need_producers = candidate_registry;
        if let Some(started) = started {
            self.remaining_new_task_requests -= 1;
            output.requests.tasks.push(started.spec);
        }
    }

    fn fail_with_budget_error(&mut self, message: String, output: &mut RuntimeStepOutput) {
        output.diagnostics.push(RuntimeDiagnostic::categorized(
            RuntimeDiagnosticCategory::Budget,
            message.clone(),
        ));
        self.fail_with_trap(AwbcTrapCode::InternalInvariant, message, None, output);
    }

    fn stage_need_producer_started(
        &self,
        fiber: &mut FiberState,
        registry: &mut crate::task::NeedProducerRegistry,
        generation: crate::task::GenerationId,
        cursor: crate::awbc::fiber::FiberCursor,
        persistent_fiber: crate::runtime_id::RuntimePersistentFiberId,
        dst: crate::awbc::schema::AwbcRegisterId,
        plan_id: crate::awbc::schema::AwbcTaskPlanId,
        values: Vec<RuntimeValue>,
    ) -> Result<Option<NeedProducerStartOutput>, ProductStepError> {
        if fiber.cursor != cursor || fiber.instance.get().get() != persistent_fiber.get() {
            return Err(ProductStepError::Internal(
                "Need producer observation does not belong to the selected Product fiber cursor"
                    .to_owned(),
            ));
        }
        let row = self
            .program
            .task_plans
            .get(plan_id.index())
            .ok_or_else(|| {
                ProductStepError::Internal(format!(
                    "Need producer observation references absent task plan {}",
                    plan_id.0
                ))
            })?;
        let plan = row
            .need_producer_plan(&self.program)
            .map_err(ProductStepError::Internal)?;
        let signature = self
            .program
            .signatures
            .get(row.signature.index())
            .ok_or_else(|| {
                ProductStepError::Internal("Need producer signature is absent".to_owned())
            })?;
        if values.len() != row.arguments.len() || values.len() != signature.params.len() {
            return Err(ProductStepError::Internal(
                "Need producer runtime arguments disagree with its verified signature".to_owned(),
            ));
        }
        let mut arguments = Vec::with_capacity(values.len());
        for ((descriptor, value), expected) in row
            .arguments
            .iter()
            .zip(values)
            .zip(signature.params.iter().copied())
        {
            if descriptor.spread
                || !crate::awbc::fiber::runtime_value_matches_type(
                    &self.program,
                    &value,
                    expected,
                    0,
                )
            {
                return Err(ProductStepError::Type(
                    "Need producer argument violates its checked source type".to_owned(),
                ));
            }
            let name = descriptor
                .name
                .map(|name| {
                    self.program
                        .strings
                        .get(name.index())
                        .cloned()
                        .ok_or_else(|| {
                            ProductStepError::Internal(
                                "Need producer argument name is absent".to_owned(),
                            )
                        })
                })
                .transpose()?;
            arguments.push(NeedProducerRuntimeArgument { name, value });
        }
        let invocation = registry
            .begin_invocation(generation, persistent_fiber, plan.site())
            .map_err(|error| ProductStepError::Internal(error.to_string()))?;
        let admission = registry
            .admit_start(invocation, plan, arguments)
            .map_err(|error| ProductStepError::Host(error.to_string()))?;
        let launch = admission.launch();
        let started = (admission.disposition() == crate::task::NeedProducerTaskDisposition::Ensure)
            .then(|| NeedProducerStartOutput {
                spec: launch.task_spec().clone(),
            });
        fiber
            .active_frame_mut()
            .and_then(|frame| frame.set_register(dst, RuntimeValue::Need(launch.need().clone())))
            .and_then(|()| fiber.commit_yielded_instruction(cursor))
            .map_err(|error| ProductStepError::Internal(error.to_string()))?;
        Ok(started)
    }
}

fn mark_child_trapped(fiber: &mut FiberState, code: AwbcTrapCode, message: String) {
    fiber.mark_trapped(crate::awbc::fiber::FiberTrap {
        code,
        message: Some(message),
        source_map: None,
    });
}

fn resume_deferred_await_value(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    binding: Option<crate::awbc::schema::AwbcPatternId>,
    resume: AwbcResumePointId,
    value: &RuntimeValue,
    _output: &mut RuntimeStepOutput,
) -> bool {
    if let Some(pattern) = binding
        && let Err(error) = crate::awbc::vm::bind_pattern(program, fiber, pattern, value)
    {
        mark_child_trapped(fiber, AwbcTrapCode::PatternMismatch, error.to_string());
        return true;
    }
    if let Err(error) = fiber.resume_at(program, resume) {
        mark_child_trapped(fiber, AwbcTrapCode::InternalInvariant, error.to_string());
    }
    true
}

fn resume_deferred_await_progress(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    observer: AwbcAwaitObserverResume,
    progress: arcweft_need::Progress,
    _output: &mut RuntimeStepOutput,
) -> bool {
    let value = RuntimeValue::Progress(progress);
    match fiber.active_frame_mut() {
        Ok(frame) => {
            if let Err(error) = frame.set_register(observer.destination, value) {
                mark_child_trapped(fiber, AwbcTrapCode::TypeMismatch, error.to_string());
                return true;
            }
        }
        Err(error) => {
            mark_child_trapped(fiber, AwbcTrapCode::InternalInvariant, error.to_string());
            return true;
        }
    }
    if let Err(error) = fiber.resume_await_observer_at(program, observer.resume) {
        mark_child_trapped(fiber, AwbcTrapCode::InternalInvariant, error.to_string());
    }
    true
}
