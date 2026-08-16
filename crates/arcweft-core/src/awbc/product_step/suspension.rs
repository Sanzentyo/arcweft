use super::{
    AudioCommandEnvelope, AudioDispatchId, AwbcEffectPlanId, AwbcFunctionId, AwbcHostCallId,
    AwbcHostCallMode, AwbcProductStepExecutor, AwbcResumePointId, AwbcTrapCode,
    FiberAwaitManyInFlight, FiberState, FiberSuspensionReason, FlowEvent, MappedEffect, NeedId,
    PendingHostCall, ProductStepError, RuntimeBinding, RuntimeDiagnostic,
    RuntimeDiagnosticCategory, RuntimeHostCallId, RuntimeHostCallMode, RuntimeHostCallRequest,
    RuntimeNeedState, RuntimePayload, RuntimeStepOutput, RuntimeStreamEvent, RuntimeValue,
    TaskEvent, TaskEventKind, TaskId, TaskKey, TaskSequence, VmObservation, content_request,
    resolved_runtime_need_state, runtime_sequence_values, runtime_value_label, stream_id_for,
    task_spec,
};
use crate::awbc::vm::cancel_fiber;
use crate::stream::StreamEventKind;
use crate::task::NamedHostArg;
use crate::value::runtime_value_into_sequence_values;
use arcweft_need::Need;

impl AwbcProductStepExecutor {
    pub(super) fn ensure_await_started(
        &mut self,
        task: &RuntimeValue,
        output: &mut RuntimeStepOutput,
    ) {
        let task_id = TaskId(runtime_value_label(task));
        let Some(plan) = self.task_plan_for_id(&task_id.0) else {
            self.record_error(
                ProductStepError::Host(format!("missing AWBC task plan for `{}`", task_id.0)),
                output,
            );
            return;
        };
        let need = self.task_need_id(plan);
        if self.started_tasks.insert(task_id.clone()) {
            match task_spec(&self.program, plan, &task_id, Vec::new()) {
                Ok((_, spec)) => output.requests.tasks.push(spec),
                Err(error) => self.record_error(error, output),
            }
        }
        if !output
            .flow_events
            .iter()
            .any(|event| matches!(event, FlowEvent::AwaitStarted { task, .. } if task == &task_id))
        {
            output.flow_events.push(FlowEvent::AwaitStarted {
                need,
                task: task_id,
            });
        }
    }

    pub(super) fn resume_await(
        &mut self,
        task: &RuntimeValue,
        binding: Option<crate::awbc::schema::AwbcPatternId>,
        resume: AwbcResumePointId,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let task_id = TaskId(runtime_value_label(task));
        let Some(event) = events.iter().find(|event| event.task_id == task_id) else {
            return false;
        };
        let need = self
            .task_plan_for_id(&task_id.0)
            .map_or_else(|| NeedId(task_id.0.clone()), |plan| self.task_need_id(plan));
        match &event.kind {
            TaskEventKind::Ready(value) => {
                let result = RuntimeValue::result_ok(value.value().clone());
                output.flow_events.push(FlowEvent::AwaitReady {
                    need,
                    value: RuntimePayload::from(result.clone()),
                });
                self.resume_await_value(binding, resume, &result, output)
            }
            TaskEventKind::Error(error) => {
                let result = RuntimeValue::result_err(error.value().clone());
                output.flow_events.push(FlowEvent::AwaitReady {
                    need,
                    value: RuntimePayload::from(result.clone()),
                });
                self.resume_await_value(binding, resume, &result, output)
            }
            TaskEventKind::Progress(progress) => {
                output.flow_events.push(FlowEvent::AwaitProgress {
                    need,
                    progress: progress.clone(),
                });
                true
            }
            TaskEventKind::Failed(error) => {
                self.fail_with_trap(
                    AwbcTrapCode::HostAbiMismatch,
                    format!("await task {} failed: {error}", task_id.0),
                    None,
                    output,
                );
                true
            }
            TaskEventKind::Cancelled => {
                let cancellation = cancel_fiber(&mut self.fiber);
                self.consume_observations(cancellation.observations, output);
                true
            }
        }
    }

    pub(super) fn resume_need(
        &mut self,
        need: &NeedId,
        binding: Option<crate::awbc::schema::AwbcPatternId>,
        resume: AwbcResumePointId,
        states: &[RuntimeNeedState],
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let Some(state) = resolved_runtime_need_state(states, need) else {
            return false;
        };
        match state.state() {
            Need::NotStarted | Need::Pending(_) => false,
            Need::Ready(value) => {
                let result = RuntimeValue::result_ok(value.value().clone());
                self.resume_await_value(binding, resume, &result, output)
            }
            Need::Err(error) => {
                let result = RuntimeValue::result_err(error.value().clone());
                self.resume_await_value(binding, resume, &result, output)
            }
            Need::Cancelled => {
                let cancellation = cancel_fiber(&mut self.fiber);
                self.consume_observations(cancellation.observations, output);
                true
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

    pub(super) fn fill_await_many(&mut self, output: &mut RuntimeStepOutput) {
        let Some((plan_id, limit, argument_count)) =
            self.fiber
                .suspension
                .as_ref()
                .and_then(|suspension| match &suspension.reason {
                    FiberSuspensionReason::AwaitMany(state) => {
                        self.program.task_plans.get(state.plan.index()).map(|plan| {
                            (
                                state.plan,
                                plan.many
                                    .as_ref()
                                    .map_or(usize::MAX, |policy| policy.limit.max(1) as usize),
                                plan.arguments.len(),
                            )
                        })
                    }
                    _ => None,
                })
        else {
            return;
        };
        let base_task = self.task_public_id(plan_id);
        let base_need = self.task_need_id(plan_id).0;
        let Some(suspension) = self.fiber.suspension.as_mut() else {
            return;
        };
        let FiberSuspensionReason::AwaitMany(state) = &mut suspension.reason else {
            return;
        };
        if state.results.len() != state.items.len() {
            state.results = vec![None; state.items.len()];
        }
        while state.in_flight.len() < limit && (state.next_index as usize) < state.items.len() {
            let index = state.next_index as usize;
            let task = TaskId(format!("{base_task}.{index}"));
            let need = NeedId(format!("{base_need}.{index}"));
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
                Ok((_, mut spec)) => {
                    spec.key = TaskKey(spec.debug_label.clone());
                    output.flow_events.push(FlowEvent::AwaitStarted {
                        need: need.clone(),
                        task: task.clone(),
                    });
                    output.requests.tasks.push(spec);
                    state.in_flight.push(FiberAwaitManyInFlight {
                        index: index_u32,
                        task_id: task.0,
                        need_id: need.0,
                    });
                    state.next_index = state.next_index.saturating_add(1);
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
    }

    pub(super) fn resume_await_many(
        &mut self,
        mut state: crate::awbc::fiber::FiberAwaitManyState,
        resume: AwbcResumePointId,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
    ) -> bool {
        if state.results.len() != state.items.len() {
            state.results = vec![None; state.items.len()];
        }
        let mut progressed = false;
        for event in events {
            let Some(position) = state
                .in_flight
                .iter()
                .position(|in_flight| in_flight.task_id == event.task_id.0)
            else {
                continue;
            };
            match &event.kind {
                TaskEventKind::Ready(value) => {
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
                TaskEventKind::Error(error) => {
                    self.fail_with_trap(
                        AwbcTrapCode::HostAbiMismatch,
                        format!(
                            "await task {} at index {} returned error: {}",
                            event.task_id.0,
                            state.in_flight[position].index,
                            runtime_value_label(error.value())
                        ),
                        None,
                        output,
                    );
                    return true;
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
            output.flow_events.push(FlowEvent::AwaitReady {
                need: self.task_need_id(state.plan),
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
            args: positional,
            named_args,
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
        match &result.outcome {
            Ok(value) => {
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
                VmObservation::TaskStarted { plan, handle, args } => {
                    let task = TaskId(runtime_value_label(&handle));
                    if self.started_tasks.insert(task.clone()) {
                        match task_spec(&self.program, plan, &task, args) {
                            Ok((need, spec)) => {
                                output
                                    .flow_events
                                    .push(FlowEvent::AwaitStarted { need, task });
                                output.requests.tasks.push(spec);
                            }
                            Err(error) => self.record_error(error, output),
                        }
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
        match FiberState::for_function(
            &self.program,
            self.fiber.entry,
            function,
            self.next_generation,
            self.fiber.budget.quantum.max(1),
        ) {
            Ok(mut child) => {
                self.next_generation = self.next_generation.saturating_add(1);
                match child
                    .active_frame_mut()
                    .and_then(|frame| frame.bind_positional_arguments(&self.program, args))
                {
                    Ok(()) => self.child_fibers.push_back(super::ProductChildFiber {
                        owner,
                        fiber: child,
                    }),
                    Err(error) => {
                        self.record_error(ProductStepError::Type(error.to_string()), output);
                    }
                }
            }
            Err(error) => self.record_error(ProductStepError::Internal(error.to_string()), output),
        }
    }

    pub(super) fn bind_root_arguments(
        &mut self,
        bindings: &[RuntimeBinding],
    ) -> Result<(), crate::awbc::fiber::FiberStateError> {
        if self.entry_targets_active_function() {
            self.fiber.bind_entry_arguments(&self.program, bindings)
        } else {
            self.fiber.bind_function_arguments(&self.program, bindings)
        }
    }
}
