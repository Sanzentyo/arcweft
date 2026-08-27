use super::dialogue::{DialogueLineTaskState, DialogueRuntimePhase};
use super::{
    AwaitManyInFlight, AwaitManyState, AwaitState, AwaitTarget, CancelScopeId, ChoiceState, Engine,
    FlowEvent, FlowExit, FlowFiberStatus, HostCallState, LineEffectRequest, RuntimeDiagnostic,
    RuntimeEvalError, RuntimePayload, RuntimeSeq, RuntimeStepInput, RuntimeStepOutput,
    RuntimeValue, TaskEvent, TaskEventKind, TaskKey, TaskPolicy, TaskPriority,
    TaskPublicationCursor, TaskSpec, runtime_sequence_values, runtime_value_into_sequence_values,
};
use crate::line_task::{
    cancel_live_line_task_group, finalize_live_line_task_close, finish_live_line_task_group,
    progress_live_line_task_group,
};
use crate::pure::RuntimeCallBackend;
use crate::step::RuntimeDiagnosticCategory;
use crate::task::{
    AssetRequest, AudioDecodeRequest, AwaitManyTarget, FileReadBytesRequest, FileReadTextRequest,
    FileWriteBytesRequest, FileWriteTextRequest, HostTaskRequest, HostTaskRequestTemplate,
    HttpFetchRequest, HttpRespondRequest, NeedId, ProcessRunRequest, RuntimeHostArgumentTemplate,
    ShaderRequest, SystemInfoKind, SystemInfoRequest, TaskId, TtsRequest, WasmCallRequest,
};
use crate::value::{DenseSeq, RuntimeFieldValue};

impl Engine {
    pub(super) fn resume_suspended(
        &mut self,
        input: &RuntimeStepInput,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        let status = std::mem::replace(&mut self.fiber.status, FlowFiberStatus::Running);
        match status {
            FlowFiberStatus::Dialogue(activation) => {
                let transaction = match self.begin_dialogue_activation_transaction(&activation) {
                    Ok(transaction) => transaction,
                    Err(error) => {
                        self.fail_eval(error, output);
                        return true;
                    }
                };
                self.resume_dialogue_state(transaction, output, pure_backend);
                true
            }
            FlowFiberStatus::Waiting(state) => {
                self.resume_await_state(*state, events, output);
                true
            }
            FlowFiberStatus::NeedWaiting(need) => {
                self.fiber.status = FlowFiberStatus::NeedWaiting(need);
                false
            }
            FlowFiberStatus::WaitingMany(state) => {
                self.resume_await_many_state(*state, events, output, pure_backend);
                true
            }
            FlowFiberStatus::HostCall(state) => {
                self.resume_host_call_state(state, input, output);
                true
            }
            FlowFiberStatus::Choice(state) => {
                self.resume_choice_state(state, input, output, pure_backend);
                true
            }
            status @ (FlowFiberStatus::Running
            | FlowFiberStatus::Done(_)
            | FlowFiberStatus::Failed(_)) => {
                self.fiber.status = status;
                false
            }
        }
    }

    fn resume_dialogue_state(
        &mut self,
        mut transaction: super::dialogue::DialogueActivationTransaction,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let line_outcomes = transaction.take_line_outcomes();
        if let Err(error) = self.consume_dialogue_host_outcomes(&mut transaction, &line_outcomes) {
            if transaction.frame().failure.is_some() {
                output.diagnostics.push(RuntimeDiagnostic::new(format!(
                    "dialogue cleanup after primary failure also failed: {error}"
                )));
                self.resume_dialogue_failure_close(transaction, output);
            } else {
                self.begin_dialogue_failure(transaction, error, output);
            }
            return;
        }
        if transaction.frame().failure.is_some() {
            self.resume_dialogue_failure_close(transaction, output);
            return;
        }
        if transaction.frame().phase == DialogueRuntimePhase::Closing {
            self.resume_dialogue_successful_close(transaction, output);
            return;
        }
        if transaction.frame().phase == DialogueRuntimePhase::Publishing {
            self.resume_dialogue_publication(transaction, output);
            return;
        }
        if transaction.frame().phase == DialogueRuntimePhase::Activating {
            match self.resume_dialogue_activation(&mut transaction, &line_outcomes, pure_backend) {
                Ok(start) => self.commit_and_suspend_dialogue(transaction, output, start),
                Err(error) => self.begin_dialogue_failure(transaction, error, output),
            }
            return;
        }
        let content_id = transaction.frame().content;
        let Some(content) = self.plan.dialogue_content().get(content_id).cloned() else {
            self.begin_dialogue_failure(
                transaction,
                crate::line_task::LineRuntimeError::UnknownContentPlan.into(),
                output,
            );
            return;
        };
        let elapsed = transaction.frame().elapsed;
        let due_tokens = match transaction.line_mut().arm_due_schedules(elapsed) {
            Ok(due) => due,
            Err(error) => {
                self.begin_dialogue_failure(transaction, error.into(), output);
                return;
            }
        };
        let content_events = transaction.take_content_events();
        let advance = transaction.take_advance();
        let mut frame = transaction.frame().clone();
        let mut start = None;
        if let DialogueLineTaskState::Live(line_task) = &mut frame.line_task {
            let Some(group) = self
                .plan
                .line_task_groups()
                .get(frame.task_group.index())
                .cloned()
            else {
                *transaction.frame_mut() = frame;
                self.begin_dialogue_failure(
                    transaction,
                    crate::line_task::LineRuntimeError::UnknownTaskGroup.into(),
                    output,
                );
                return;
            };
            let accepted_content =
                match line_task.accept_content_event_kinds(&content_events, |event| match event {
                    crate::step::RuntimeDialogueContentEventKind::Mark(mark) => content
                        .marks()
                        .get(mark.index())
                        .is_some_and(|candidate| candidate.id() == mark),
                    crate::step::RuntimeDialogueContentEventKind::Effect(effect) => {
                        content.effect_site_count().contains(effect)
                    }
                }) {
                    Ok(events) => events,
                    Err(error) => {
                        *transaction.frame_mut() = frame;
                        self.begin_dialogue_failure(transaction, error.into(), output);
                        return;
                    }
                };
            let marks = accepted_content.marks();
            if let Some(cancelled) = cancel_live_line_task_group(&group, &marks, line_task) {
                let event = marks.iter().copied().find_map(|mark| {
                    group
                        .cancel_rules()
                        .iter()
                        .any(|rule| rule.trigger() == mark)
                        .then(|| {
                            content
                                .marks()
                                .iter()
                                .find(|candidate| candidate.id() == mark)
                                .map(|candidate| FlowEvent::LineCancelled {
                                    trigger: candidate.label().to_owned(),
                                })
                        })
                        .flatten()
                });
                start = Some(super::dialogue::DialogueLineTaskStart {
                    event,
                    request_cancellation: true,
                    group,
                    activation: cancelled,
                    captures: frame.captures.clone(),
                });
                *transaction.frame_mut() = frame;
                self.commit_and_suspend_dialogue(transaction, output, start);
                return;
            }
            if line_task.is_closing() {
                let activation = finalize_live_line_task_close(&group, line_task);
                if line_task.is_closed() {
                    frame.line_task = DialogueLineTaskState::Closed;
                    frame.phase = DialogueRuntimePhase::Closing;
                }
                start = Some(super::dialogue::DialogueLineTaskStart {
                    event: None,
                    request_cancellation: false,
                    group,
                    activation,
                    captures: frame.captures.clone(),
                });
                *transaction.frame_mut() = frame;
                self.commit_and_suspend_dialogue(transaction, output, start);
                return;
            }
            for token in due_tokens {
                if let Err(error) = line_task.mark_scheduled_ready(token) {
                    *transaction.frame_mut() = frame;
                    self.begin_dialogue_failure(transaction, error.into(), output);
                    return;
                }
            }
            let activation = match progress_live_line_task_group(
                &group,
                frame.elapsed,
                accepted_content.ready(),
                line_task,
            ) {
                Ok(activation) => activation,
                Err(error) => {
                    *transaction.frame_mut() = frame;
                    self.begin_dialogue_failure(transaction, error.into(), output);
                    return;
                }
            };
            start = Some(super::dialogue::DialogueLineTaskStart {
                event: None,
                request_cancellation: false,
                group,
                activation,
                captures: frame.captures.clone(),
            });
        } else if !content_events.is_empty() {
            let event = content_events[0];
            *transaction.frame_mut() = frame;
            self.begin_dialogue_failure(
                transaction,
                crate::line_task::LineRuntimeError::ContentEventOutsideLiveLineTask { event }
                    .into(),
                output,
            );
            return;
        }
        if advance {
            if let DialogueLineTaskState::Live(line_task) = &mut frame.line_task
                && let Some(group) = self
                    .plan
                    .line_task_groups()
                    .get(frame.task_group.index())
                    .cloned()
            {
                let cleanup = finish_live_line_task_group(&group, line_task);
                start = Some(super::dialogue::DialogueLineTaskStart {
                    event: None,
                    request_cancellation: true,
                    group,
                    activation: cleanup,
                    captures: frame.captures.clone(),
                });
            }
            if matches!(frame.line_task, DialogueLineTaskState::Closed)
                || matches!(
                    &frame.line_task,
                    DialogueLineTaskState::Live(line_task) if line_task.is_closed()
                )
            {
                frame.line_task = DialogueLineTaskState::Closed;
                frame.phase = DialogueRuntimePhase::Closing;
            }
        }
        *transaction.frame_mut() = frame;
        self.commit_and_suspend_dialogue(transaction, output, start);
    }

    pub(super) fn resume_await_state(
        &mut self,
        mut state: AwaitState,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
    ) {
        let event = state.queued.pop_front().or_else(|| {
            events
                .iter()
                .find(|event| {
                    event.task_id == state.target.task
                        && state
                            .observed_through
                            .is_none_or(|cursor| TaskPublicationCursor::from_event(event) > cursor)
                })
                .cloned()
        });
        let Some(event) = event else {
            self.fiber.status = FlowFiberStatus::Waiting(Box::new(state));
            return;
        };
        let event_cursor = TaskPublicationCursor::from_event(&event);
        state.observed_through = Some(
            state
                .observed_through
                .map_or(event_cursor, |cursor| cursor.max(event_cursor)),
        );
        self.task_publications.insert(
            state.target.task.clone(),
            state.observed_through.unwrap_or(event_cursor),
        );
        match event.kind {
            TaskEventKind::Ready(value) => {
                if !state.target.outcome.payload().accepts_value(value.value()) {
                    let message = format!(
                        "await task {} published a payload outside its checked outcome contract",
                        state.target.task.0
                    );
                    self.fiber.status = FlowFiberStatus::Failed(message.clone());
                    output.diagnostics.push(RuntimeDiagnostic::categorized(
                        RuntimeDiagnosticCategory::Host,
                        message,
                    ));
                    return;
                }
                if let Some(binding) = &state.binding {
                    match self.try_bind_pattern(binding, value.value()) {
                        Ok(true) => {}
                        Ok(false) => {
                            self.fiber.status = FlowFiberStatus::Failed(
                                "await result did not match binding pattern".to_owned(),
                            );
                            output.diagnostics.push(RuntimeDiagnostic::new(
                                "await result did not match binding pattern".to_owned(),
                            ));
                            return;
                        }
                        Err(error) => {
                            self.fail_eval(error, output);
                            return;
                        }
                    }
                }
                output.flow_events.push(FlowEvent::AwaitReady {
                    need: state.target.need,
                    value,
                });
                self.fiber.cursor = state.resume;
                self.fiber.status = FlowFiberStatus::Running;
            }
            TaskEventKind::Progress(progress) => {
                output.flow_events.push(FlowEvent::AwaitProgress {
                    need: state.target.need.clone(),
                    progress: progress.clone(),
                });
                self.start_await_pending_observer(state, progress, output);
            }
            TaskEventKind::Failed(error) => {
                let message = format!("await task {} failed: {error}", state.target.task.0);
                self.fiber.status = FlowFiberStatus::Failed(message.clone());
                output.diagnostics.push(RuntimeDiagnostic::categorized(
                    RuntimeDiagnosticCategory::Host,
                    message,
                ));
            }
            TaskEventKind::Cancelled => {
                self.fiber.status = FlowFiberStatus::Done(FlowExit::Done);
            }
        }
    }

    fn start_await_pending_observer(
        &mut self,
        state: AwaitState,
        progress: arcweft_need::Progress,
        output: &mut RuntimeStepOutput,
    ) {
        let value = RuntimeValue::Progress(progress);
        for observer in &state.observers {
            match crate::pattern::match_runtime_pattern(&self.plan, &observer.pattern, &value) {
                Ok(Some(bindings)) => {
                    let ops = observer.ops.clone();
                    self.fiber.await_observer = Some(Box::new(state));
                    self.push_await_observer_ops(bindings, &ops);
                    self.fiber.status = FlowFiberStatus::Running;
                    return;
                }
                Ok(None) => {}
                Err(error) => {
                    self.fail_eval(error, output);
                    return;
                }
            }
        }
        self.fiber.status = FlowFiberStatus::Waiting(Box::new(state));
    }

    pub(super) fn latch_active_await_observer_events(&mut self, events: &[TaskEvent]) {
        let Some(state) = self.fiber.await_observer.as_mut() else {
            return;
        };
        for event in events {
            if event.task_id != state.target.task
                || state
                    .observed_through
                    .is_some_and(|cursor| TaskPublicationCursor::from_event(event) <= cursor)
            {
                continue;
            }
            state.observed_through = Some(TaskPublicationCursor::from_event(event));
            self.task_publications.insert(
                state.target.task.clone(),
                TaskPublicationCursor::from_event(event),
            );
            state.queued.push_back(event.clone());
            if matches!(
                event.kind,
                TaskEventKind::Ready(_) | TaskEventKind::Failed(_) | TaskEventKind::Cancelled
            ) {
                break;
            }
        }
    }

    pub(super) fn resume_host_call_state(
        &mut self,
        state: HostCallState,
        input: &RuntimeStepInput,
        output: &mut RuntimeStepOutput,
    ) {
        let Some(result) = input
            .host_call_results
            .iter()
            .find(|result| result.id == state.id)
        else {
            self.fiber.status = FlowFiberStatus::HostCall(state);
            return;
        };
        match &result.outcome {
            Ok(value) => {
                if let Some(binding) = &state.binding {
                    match self.try_bind_pattern(binding, value.value()) {
                        Ok(true) => {}
                        Ok(false) => {
                            self.fiber.status = FlowFiberStatus::Failed(
                                "host-call result did not match binding pattern".to_owned(),
                            );
                            output.diagnostics.push(RuntimeDiagnostic::categorized(
                                RuntimeDiagnosticCategory::Host,
                                "host-call result did not match binding pattern".to_owned(),
                            ));
                            return;
                        }
                        Err(error) => {
                            self.fail_eval(error, output);
                            return;
                        }
                    }
                }
                self.fiber.cursor = state.resume;
                self.fiber.status = FlowFiberStatus::Running;
            }
            Err(error) => {
                let category = match error.kind {
                    crate::step::RuntimeHostCallErrorKind::UnsupportedCapability => {
                        RuntimeDiagnosticCategory::Capability
                    }
                    crate::step::RuntimeHostCallErrorKind::Rejected
                    | crate::step::RuntimeHostCallErrorKind::Failed => {
                        RuntimeDiagnosticCategory::Host
                    }
                };
                self.fiber.status = FlowFiberStatus::Failed(error.message.clone());
                output.diagnostics.push(RuntimeDiagnostic::categorized(
                    category,
                    error.message.clone(),
                ));
            }
        }
    }

    pub(super) fn resume_choice_state(
        &mut self,
        mut state: ChoiceState,
        input: &RuntimeStepInput,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let Some((requested_choice, selection)) = input_choice_selection(input) else {
            self.fiber.status = FlowFiberStatus::Choice(state);
            return;
        };
        if requested_choice.is_some() && requested_choice != state.id.as_deref() {
            output.diagnostics.push(RuntimeDiagnostic::categorized(
                RuntimeDiagnosticCategory::Input,
                format!(
                    "stale choice selection for `{}` while waiting on `{}`",
                    requested_choice.unwrap_or_default(),
                    state.id.as_deref().unwrap_or("-")
                ),
            ));
            self.fiber.status = FlowFiberStatus::Choice(state);
            return;
        }
        let Some(position) = state.options.iter().position(|option| {
            option.id.as_deref() == Some(selection) || option.label == selection
        }) else {
            output.diagnostics.push(RuntimeDiagnostic::categorized(
                RuntimeDiagnosticCategory::Input,
                format!(
                    "invalid option `{selection}` for choice `{}`",
                    state.id.as_deref().unwrap_or("-")
                ),
            ));
            self.fiber.status = FlowFiberStatus::Choice(state);
            return;
        };
        let option = state.options.remove(position);
        let selected = option.id.clone().unwrap_or_else(|| option.label.clone());
        output.flow_events.push(FlowEvent::ChoiceSelected {
            id: state.id,
            option: selected,
        });
        self.emit_line_effects(option.effects, output, pure_backend);
        if let Some(out) = option.out {
            self.emit_line_effect(LineEffectRequest::Out(out), output, pure_backend);
        }
        if let Some(target) = option.target {
            self.goto(&target, output, pure_backend);
        } else {
            self.fiber.cursor = state.resume;
            self.fiber.status = FlowFiberStatus::Running;
        }
    }

    pub(super) fn await_task_spec(
        &mut self,
        target: &AwaitTarget,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Option<TaskSpec> {
        let request = match self.evaluate_host_task_request(target, pure_backend) {
            Ok(request) => request,
            Err(error) => {
                self.fail_eval(error, output);
                return None;
            }
        };
        Some(
            TaskSpec::new(
                target.task.clone(),
                TaskKey(target.task.0.clone()),
                request.task_class(),
                TaskPriority(0),
                CancelScopeId("flow".to_owned()),
                TaskPolicy::JoinSameKey,
                request,
            )
            .with_outcome(target.outcome.clone()),
        )
    }

    pub(super) fn start_await_many_state(
        &mut self,
        binding: Option<crate::pattern::RuntimePattern>,
        target: AwaitManyTarget,
        resume: Option<crate::engine::FlowCursor>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let items = match self.evaluate_expr_with_backend(&target.source, pure_backend) {
            Ok(value) => match runtime_value_into_sequence_values(value) {
                Ok(items) => items,
                Err(value) => {
                    self.fail_eval(
                        RuntimeEvalError::ExpectedBracketSeq(super::runtime_value_label(&value)),
                        output,
                    );
                    return;
                }
            },
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        let mut state = AwaitManyState {
            binding,
            target,
            resume,
            next_index: 0,
            in_flight: Vec::new(),
            results: vec![None; items.len()],
            items,
        };
        if self.fill_await_many_queue(&mut state, output, pure_backend) {
            self.commit_await_many_state(state, output);
        }
    }

    pub(super) fn resume_await_many_state(
        &mut self,
        mut state: AwaitManyState,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        for event in events {
            let Some(position) = state
                .in_flight
                .iter()
                .position(|task| task.task == event.task_id)
            else {
                continue;
            };
            match &event.kind {
                TaskEventKind::Ready(value) => {
                    if !state.target.outcome.payload().accepts_value(value.value()) {
                        let in_flight = &state.in_flight[position];
                        let message = format!(
                            "await task {} at index {} published a payload outside its checked outcome contract",
                            in_flight.task.0, in_flight.index
                        );
                        self.fiber.status = FlowFiberStatus::Failed(message.clone());
                        output.diagnostics.push(RuntimeDiagnostic::categorized(
                            RuntimeDiagnosticCategory::Host,
                            message,
                        ));
                        return;
                    }
                    let in_flight = state.in_flight.remove(position);
                    state.results[in_flight.index] = Some(value.clone());
                    output.flow_events.push(FlowEvent::AwaitReady {
                        need: in_flight.need,
                        value: value.clone(),
                    });
                }
                TaskEventKind::Progress(progress) => {
                    let in_flight = &state.in_flight[position];
                    output.flow_events.push(FlowEvent::AwaitProgress {
                        need: in_flight.need.clone(),
                        progress: progress.clone(),
                    });
                }
                TaskEventKind::Failed(error) => {
                    let in_flight = &state.in_flight[position];
                    let message = format!(
                        "await task {} at index {} failed: {error}",
                        in_flight.task.0, in_flight.index
                    );
                    self.fiber.status = FlowFiberStatus::Failed(message.clone());
                    output.diagnostics.push(RuntimeDiagnostic::categorized(
                        RuntimeDiagnosticCategory::Host,
                        message,
                    ));
                    return;
                }
                TaskEventKind::Cancelled => {
                    self.fiber.status = FlowFiberStatus::Done(FlowExit::Done);
                    return;
                }
            }
        }
        if self.fill_await_many_queue(&mut state, output, pure_backend) {
            self.commit_await_many_state(state, output);
        }
    }

    fn fill_await_many_queue(
        &mut self,
        state: &mut AwaitManyState,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        while state.in_flight.len() < state.target.limit && state.next_index < state.items.len() {
            let index = state.next_index;
            let item = state.items[index].clone();
            let need = indexed_need_id(&state.target.need, index);
            let task = indexed_task_id(&state.target.task, index);
            let Some(spec) =
                self.await_many_task_spec(&state.target, index, &item, &task, output, pure_backend)
            else {
                return false;
            };
            output.flow_events.push(FlowEvent::AwaitStarted {
                need: need.clone(),
                task: task.clone(),
            });
            output.requests.tasks.push(spec);
            state
                .in_flight
                .push(AwaitManyInFlight { index, task, need });
            state.next_index += 1;
        }
        true
    }

    fn commit_await_many_state(&mut self, state: AwaitManyState, output: &mut RuntimeStepOutput) {
        if !state.in_flight.is_empty() || state.results.iter().any(Option::is_none) {
            self.fiber.status = FlowFiberStatus::WaitingMany(Box::new(state));
            return;
        }
        let values = state
            .results
            .iter()
            .filter_map(|value| value.as_ref().map(|value| value.value().clone()))
            .collect::<Vec<_>>();
        let ready_value = runtime_sequence_values(values);
        if let Some(binding) = &state.binding {
            match self.try_bind_pattern(binding, &ready_value) {
                Ok(true) => {}
                Ok(false) => {
                    self.fiber.status = FlowFiberStatus::Failed(
                        "await result did not match binding pattern".to_owned(),
                    );
                    output.diagnostics.push(RuntimeDiagnostic::new(
                        "await result did not match binding pattern".to_owned(),
                    ));
                    return;
                }
                Err(error) => {
                    self.fail_eval(error, output);
                    return;
                }
            }
        }
        output.flow_events.push(FlowEvent::AwaitReady {
            need: state.target.need,
            value: RuntimePayload::new(ready_value),
        });
        self.fiber.cursor = state.resume;
        self.fiber.status = FlowFiberStatus::Running;
    }

    fn await_many_task_spec(
        &mut self,
        target: &AwaitManyTarget,
        index: usize,
        item: &RuntimeValue,
        task: &TaskId,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Option<TaskSpec> {
        let request = match self.with_temp_binding_ref(target.item_binding, item, |this| {
            this.evaluate_host_task_request_template(&target.request, pure_backend)
        }) {
            Ok(request) => request,
            Err(error) => {
                self.fail_eval(format!("await many item {index}: {error}"), output);
                return None;
            }
        };
        Some(
            TaskSpec::new(
                task.clone(),
                TaskKey(request.debug_label()),
                request.task_class(),
                TaskPriority(0),
                CancelScopeId("flow".to_owned()),
                TaskPolicy::JoinSameKey,
                request,
            )
            .with_outcome(target.outcome.clone()),
        )
    }

    fn evaluate_host_task_request(
        &mut self,
        target: &AwaitTarget,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<HostTaskRequest, String> {
        self.evaluate_host_task_request_template(&target.request, pure_backend)
    }

    fn evaluate_host_task_request_template(
        &mut self,
        template: &HostTaskRequestTemplate,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<HostTaskRequest, String> {
        let args = self.evaluate_host_task_args(&template.args, pure_backend)?;
        let call = EvaluatedHostCall {
            capability: template.capability.0.as_str(),
            operation: template.operation.as_str(),
            args: &args,
        };
        lower_evaluated_host_request(&call)
    }

    fn evaluate_host_task_args(
        &mut self,
        args: &[RuntimeHostArgumentTemplate],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Vec<EvaluatedHostArg>, String> {
        let mut evaluated = Vec::new();
        for arg in args {
            let value = self
                .evaluate_expr_with_backend(arg.value(), pure_backend)
                .map_err(|error| error.to_string())?;
            if arg.is_spread() {
                for value in spread_host_arg_values(value)? {
                    evaluated.push(EvaluatedHostArg { name: None, value });
                }
            } else {
                evaluated.push(EvaluatedHostArg {
                    name: arg.name().map(str::to_owned),
                    value,
                });
            }
        }
        Ok(evaluated)
    }
}

struct EvaluatedHostArg {
    name: Option<String>,
    value: RuntimeValue,
}

struct EvaluatedHostCall<'a> {
    capability: &'a str,
    operation: &'a str,
    args: &'a [EvaluatedHostArg],
}

fn spread_host_arg_values(value: RuntimeValue) -> Result<Vec<RuntimeValue>, String> {
    match runtime_value_into_sequence_values(value) {
        Ok(items) => Ok(items),
        Err(value) => Err(format!(
            "spread host argument requires a tuple or bracket sequence, found {}",
            super::runtime_value_label(&value)
        )),
    }
}

fn lower_evaluated_host_request(call: &EvaluatedHostCall<'_>) -> Result<HostTaskRequest, String> {
    match (call.capability, call.operation) {
        ("file" | "fs", "read_text") => Ok(HostTaskRequest::FileReadText(FileReadTextRequest {
            path: positional_string(call.args, 0)?,
        })),
        ("file" | "fs", "read_bytes") => Ok(HostTaskRequest::FileReadBytes(FileReadBytesRequest {
            path: positional_string(call.args, 0)?,
        })),
        ("file" | "fs", "write_text") => Ok(HostTaskRequest::FileWriteText(FileWriteTextRequest {
            path: positional_string(call.args, 0)?,
            text: positional_string(call.args, 1)?,
        })),
        ("file" | "fs", "write_bytes") => {
            Ok(HostTaskRequest::FileWriteBytes(FileWriteBytesRequest {
                path: positional_string(call.args, 0)?,
                bytes: positional_bytes(call.args, 1)?,
            }))
        }
        ("file" | "fs", "write") => lower_file_write_request(call.args),
        ("http", "fetch") => Ok(HostTaskRequest::HttpFetch(HttpFetchRequest {
            url: positional_string(call.args, 0)?,
            method: named_string(call.args, "method").unwrap_or_else(|| "GET".to_owned()),
            headers: named_headers(call.args, "headers").unwrap_or_default(),
            body: named_payload(call.args, "body"),
        })),
        ("http", "respond") => Ok(HostTaskRequest::HttpRespond(HttpRespondRequest {
            request_id: positional_string(call.args, 0)?,
            status: named_u16(call.args, "status").unwrap_or(200),
            headers: named_headers(call.args, "headers").unwrap_or_default(),
            body: named_payload(call.args, "body"),
        })),
        ("process", "run") => Ok(HostTaskRequest::ProcessRun(ProcessRunRequest {
            program: positional_string(call.args, 0)?,
            args: named_string_seq(call.args, "args").unwrap_or_default(),
            env: named_headers(call.args, "env").unwrap_or_default(),
        })),
        ("asset", "load") => Ok(HostTaskRequest::AssetLoad(AssetRequest {
            id: positional_string(call.args, 0)?,
            kind: named_string(call.args, "kind").unwrap_or_else(|| "asset".to_owned()),
        })),
        ("asset", kind) => Ok(HostTaskRequest::AssetLoad(AssetRequest {
            id: positional_string(call.args, 0)?,
            kind: kind.to_owned(),
        })),
        ("voice", "load") => Ok(HostTaskRequest::AssetLoad(AssetRequest {
            id: positional_string(call.args, 0)?,
            kind: "voice".to_owned(),
        })),
        ("shader", "compile") => Ok(HostTaskRequest::ShaderCompile(ShaderRequest {
            id: positional_string(call.args, 0)?,
            entry: named_string(call.args, "entry"),
        })),
        ("audio", "decode") => Ok(HostTaskRequest::AudioDecode(AudioDecodeRequest {
            id: positional_string(call.args, 0)?,
        })),
        ("tts", "synthesize" | "synthesis") => Ok(HostTaskRequest::TtsSynthesis(TtsRequest {
            voice: named_string(call.args, "voice"),
            text: named_string(call.args, "text")
                .map_or_else(|| positional_string(call.args, 0), Ok)?,
        })),
        ("wasm", "call") => Ok(HostTaskRequest::WasmCall(WasmCallRequest {
            module: positional_string(call.args, 0)?,
            function: positional_string(call.args, 1)?,
            args: positional_after(call.args, 2)
                .into_iter()
                .map(|value| RuntimePayload::new(value.clone()))
                .collect(),
        })),
        ("system" | "runtime", "core_count") => Ok(system_info_request(SystemInfoKind::CoreCount)),
        ("system" | "runtime", "thread_count") => {
            Ok(system_info_request(SystemInfoKind::ThreadCount))
        }
        ("system" | "runtime", "available_parallelism") => {
            Ok(system_info_request(SystemInfoKind::AvailableParallelism))
        }
        _ => Ok(HostTaskRequest::custom_with_named_args(
            call.capability,
            call.operation,
            call.args
                .iter()
                .filter(|arg| arg.name.is_none())
                .map(|arg| RuntimePayload::new(arg.value.clone())),
            call.args.iter().filter_map(|arg| {
                Some((arg.name.clone()?, RuntimePayload::new(arg.value.clone())))
            }),
        )),
    }
}

fn system_info_request(kind: SystemInfoKind) -> HostTaskRequest {
    HostTaskRequest::SystemInfo(SystemInfoRequest { kind })
}

fn indexed_need_id(base: &NeedId, index: usize) -> NeedId {
    NeedId(format!("{}.{}", base.0, index))
}

fn indexed_task_id(base: &TaskId, index: usize) -> TaskId {
    TaskId(format!("{}.{}", base.0, index))
}

fn lower_file_write_request(args: &[EvaluatedHostArg]) -> Result<HostTaskRequest, String> {
    let path = positional_string(args, 0)?;
    let body =
        positional_arg(args, 1).ok_or_else(|| "missing positional argument #1".to_owned())?;
    match body {
        RuntimeValue::String(text) => Ok(HostTaskRequest::FileWriteText(FileWriteTextRequest {
            path,
            text: text.clone(),
        })),
        RuntimeValue::Seq(_) => Ok(HostTaskRequest::FileWriteBytes(FileWriteBytesRequest {
            path,
            bytes: runtime_value_to_bytes(body)?,
        })),
        value => Err(format!(
            "fs.write body must be String or byte sequence, found {}",
            super::runtime_value_label(value)
        )),
    }
}

fn positional_after(args: &[EvaluatedHostArg], count: usize) -> Vec<&RuntimeValue> {
    args.iter()
        .filter(|arg| arg.name.is_none())
        .skip(count)
        .map(|arg| &arg.value)
        .collect()
}

fn positional_string(args: &[EvaluatedHostArg], index: usize) -> Result<String, String> {
    positional_arg(args, index)
        .map(runtime_value_to_string)
        .ok_or_else(|| format!("missing positional task argument {index}"))
}

fn positional_bytes(args: &[EvaluatedHostArg], index: usize) -> Result<Vec<u8>, String> {
    let Some(value) = positional_arg(args, index) else {
        return Err(format!("missing positional task argument {index}"));
    };
    runtime_value_to_bytes(value)
}

fn positional_arg(args: &[EvaluatedHostArg], index: usize) -> Option<&RuntimeValue> {
    args.iter()
        .filter(|arg| arg.name.is_none())
        .nth(index)
        .map(|arg| &arg.value)
}

fn named_arg<'a>(args: &'a [EvaluatedHostArg], name: &str) -> Option<&'a RuntimeValue> {
    args.iter()
        .find(|arg| arg.name.as_deref() == Some(name))
        .map(|arg| &arg.value)
}

fn named_string(args: &[EvaluatedHostArg], name: &str) -> Option<String> {
    named_arg(args, name).map(runtime_value_to_string)
}

fn named_payload(args: &[EvaluatedHostArg], name: &str) -> Option<RuntimePayload> {
    named_arg(args, name).cloned().map(RuntimePayload::new)
}

fn named_u16(args: &[EvaluatedHostArg], name: &str) -> Option<u16> {
    named_arg(args, name).and_then(|value| match value {
        RuntimeValue::Int(value) => value
            .try_into_i64()
            .and_then(|value| u16::try_from(value).ok()),
        RuntimeValue::UInt(value) => value
            .try_into_i64()
            .and_then(|value| u16::try_from(value).ok()),
        value => runtime_value_to_string(value).parse().ok(),
    })
}

fn named_string_seq(args: &[EvaluatedHostArg], name: &str) -> Option<Vec<String>> {
    named_arg(args, name).map(|value| match value {
        RuntimeValue::Seq(RuntimeSeq::Values(items)) | RuntimeValue::Tuple(items) => {
            items.iter().map(runtime_value_to_string).collect()
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(items)) => match items {
            DenseSeq::Units(len) => (0..*len).map(|_| "()".to_owned()).collect(),
            DenseSeq::I8(items) => items.as_slice().iter().map(i8::to_string).collect(),
            DenseSeq::I16(items) => items.as_slice().iter().map(i16::to_string).collect(),
            DenseSeq::I32(items) => items.as_slice().iter().map(i32::to_string).collect(),
            DenseSeq::I64(items) => items.as_slice().iter().map(i64::to_string).collect(),
            DenseSeq::ISize(items) => items
                .as_slice()
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            DenseSeq::I128(items) => items.as_slice().iter().map(i128::to_string).collect(),
            DenseSeq::U8(items) | DenseSeq::Bytes(items) => {
                items.as_slice().iter().map(u8::to_string).collect()
            }
            DenseSeq::U16(items) => items.as_slice().iter().map(u16::to_string).collect(),
            DenseSeq::U32(items) => items.as_slice().iter().map(u32::to_string).collect(),
            DenseSeq::U64(items) => items.as_slice().iter().map(u64::to_string).collect(),
            DenseSeq::USize(items) => items
                .as_slice()
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            DenseSeq::U128(items) => items.as_slice().iter().map(u128::to_string).collect(),
            DenseSeq::F32(items) => items
                .as_slice()
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            DenseSeq::F64(items) => items
                .as_slice()
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            DenseSeq::Bool(items) => items.as_slice().iter().map(bool::to_string).collect(),
            DenseSeq::Chars(items) => items.as_slice().iter().map(char::to_string).collect(),
            DenseSeq::Durations(items) => items
                .as_slice()
                .iter()
                .map(|value| format!("{}ns", value.as_nanos()))
                .collect(),
            DenseSeq::Strings(items) => items.as_slice().to_vec(),
            DenseSeq::EntityRefs(items) => items
                .as_slice()
                .iter()
                .map(|value| value.runtime_label())
                .collect(),
        },
        value => vec![runtime_value_to_string(value)],
    })
}

fn named_headers(args: &[EvaluatedHostArg], name: &str) -> Option<Vec<(String, String)>> {
    named_arg(args, name).and_then(runtime_value_to_headers)
}

fn runtime_value_to_headers(value: &RuntimeValue) -> Option<Vec<(String, String)>> {
    match value {
        RuntimeValue::Record(fields) => fields
            .iter()
            .map(|field| {
                Some((
                    field.name().to_owned(),
                    runtime_value_to_string(field.value()),
                ))
            })
            .collect(),
        RuntimeValue::Seq(RuntimeSeq::Values(items)) | RuntimeValue::Tuple(items) => {
            items.iter().map(runtime_value_to_header_pair).collect()
        }
        _ => None,
    }
}

fn runtime_value_to_header_pair(value: &RuntimeValue) -> Option<(String, String)> {
    match value {
        RuntimeValue::Tuple(items) if items.len() == 2 => Some((
            runtime_value_to_string(&items[0]),
            runtime_value_to_string(&items[1]),
        )),
        RuntimeValue::Record(fields) => {
            let key = record_field(fields, "key")
                .or_else(|| record_field(fields, "name"))
                .map(runtime_value_to_string)?;
            let value = record_field(fields, "value").map(runtime_value_to_string)?;
            Some((key, value))
        }
        _ => None,
    }
}

fn record_field<'a>(fields: &'a [RuntimeFieldValue], name: &str) -> Option<&'a RuntimeValue> {
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(RuntimeFieldValue::value)
}

fn runtime_value_to_bytes(value: &RuntimeValue) -> Result<Vec<u8>, String> {
    match value {
        RuntimeValue::Seq(RuntimeSeq::Values(items)) => items
            .iter()
            .map(|item| match item {
                RuntimeValue::Int(value) => value
                    .try_into_i64()
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or_else(|| format!("byte value `{value}` is outside u8 range")),
                RuntimeValue::UInt(value) => value
                    .try_into_i64()
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or_else(|| format!("byte value `{value}` is outside u8 range")),
                value => Err(format!(
                    "byte payload item must be Int, found {}",
                    super::runtime_value_label(value)
                )),
            })
            .collect(),
        RuntimeValue::Seq(RuntimeSeq::Dense(items)) => match items {
            DenseSeq::Bytes(items) | DenseSeq::U8(items) => Ok(items.as_slice().to_vec()),
            DenseSeq::I64(items) => items
                .as_slice()
                .iter()
                .map(|value| {
                    u8::try_from(*value)
                        .map_err(|_| format!("byte value `{value}` is outside u8 range"))
                })
                .collect(),
            DenseSeq::Units(_)
            | DenseSeq::I8(_)
            | DenseSeq::I16(_)
            | DenseSeq::I32(_)
            | DenseSeq::I128(_)
            | DenseSeq::ISize(_)
            | DenseSeq::U16(_)
            | DenseSeq::U32(_)
            | DenseSeq::U64(_)
            | DenseSeq::U128(_)
            | DenseSeq::USize(_)
            | DenseSeq::F32(_)
            | DenseSeq::F64(_)
            | DenseSeq::Bool(_)
            | DenseSeq::Chars(_)
            | DenseSeq::Durations(_)
            | DenseSeq::Strings(_)
            | DenseSeq::EntityRefs(_) => Err(format!(
                "byte payload item must be Int, found {}",
                super::runtime_value_label(value)
            )),
        },
        _ => Err("byte payload must be a bracket sequence".to_owned()),
    }
}

fn runtime_value_to_string(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::String(value) => value.clone(),
        RuntimeValue::EntityRef(value) => value.runtime_label(),
        RuntimeValue::Char(value) => value.to_string(),
        RuntimeValue::Int(value) => value.to_string(),
        RuntimeValue::UInt(value) => value.to_string(),
        RuntimeValue::F32(value) => value.to_string(),
        RuntimeValue::F64(value) => value.to_string(),
        RuntimeValue::Bool(value) => value.to_string(),
        RuntimeValue::Duration(value) => format!("{}ns", value.as_nanos()),
        RuntimeValue::Progress(value) => value
            .label()
            .map_or_else(|| value.ratio().to_string(), str::to_owned),
        RuntimeValue::Unit
        | RuntimeValue::Range(_)
        | RuntimeValue::Iterator(_)
        | RuntimeValue::MatrixF32(_)
        | RuntimeValue::MatrixF64(_)
        | RuntimeValue::TensorF32(_)
        | RuntimeValue::TensorF64(_)
        | RuntimeValue::Tuple(_)
        | RuntimeValue::Seq(_)
        | RuntimeValue::Record(_)
        | RuntimeValue::NominalRecord(_)
        | RuntimeValue::Opaque(_)
        | RuntimeValue::Agent(_)
        | RuntimeValue::Reduction(_)
        | RuntimeValue::Function(_)
        | RuntimeValue::Variant { .. } => super::runtime_value_label(value),
    }
}

fn input_choice_selection(input: &RuntimeStepInput) -> Option<(Option<&str>, &str)> {
    input.input_events.iter().find_map(|event| {
        let trigger = crate::step::input_event_trigger_name(event)?;
        let selection = crate::step::input_event_text_payload(event)?;
        match trigger {
            "choice" | "select" => Some((None, selection)),
            trigger => trigger
                .strip_prefix("choice:")
                .or_else(|| trigger.strip_prefix("select:"))
                .map(|choice| (Some(choice), selection)),
        }
    })
}
