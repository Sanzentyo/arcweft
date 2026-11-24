use super::DialogueRuntimePhase;
use super::dialogue::DialogueLineTaskState;
use super::{
    AwaitItemType, AwaitManyInFlight, AwaitManyState, AwaitState, CancelScopeId, ChoiceState,
    Engine, FlowEvent, FlowExit, FlowFiberStatus, HostCallState, LineEffectRequest,
    RuntimeDiagnostic, RuntimeEvalError, RuntimePayload, RuntimeStepInput, RuntimeStepOutput,
    RuntimeValue, TaskEvent, TaskEventKind, TaskKey, TaskPolicy, TaskPriority, TaskSpec,
    WaitingManyStatus, runtime_value_into_sequence_values,
};
use crate::line_task::{
    cancel_live_line_task_group, finalize_live_line_task_close, finish_live_line_task_group,
    progress_live_line_task_group,
};
use crate::plan::RuntimeTaskPlanSealLimits;
use crate::plan::task_source::RuntimeTaskSourceProgram;
use crate::pure::RuntimeCallBackend;
use crate::step::RuntimeDiagnosticCategory;
use crate::task::{
    AwaitManyTarget, NeedId, NeedProducerRuntimeArgument, NeedProducerTaskDisposition,
    RuntimeHostArgumentTemplate, RuntimeNeedProducerState, RuntimeNeedPublication,
    RuntimeTaskTemplateArguments, TaskId,
};
use crate::value::{RuntimeRetainedValueAdmission, RuntimeValueSource};
use arcweft_need::Need;
use std::collections::VecDeque;
use std::sync::Arc;

impl Engine {
    /// Retain Host fanout publications before the scheduler selects a fiber.
    /// A parent operation may consume this step's budget before its waiting
    /// child runs. The existing Need queue owns that child's value meanwhile.
    pub(super) fn latch_await_many_task_event(
        &mut self,
        event: TaskEvent,
        output: &mut RuntimeStepOutput,
    ) -> Option<TaskEvent> {
        let handle = std::iter::once(&self.fiber)
            .chain(&self.child_fibers)
            .find_map(|fiber| {
                let FlowFiberStatus::WaitingMany(WaitingManyStatus::Native(state)) =
                    &fiber.frame.status
                else {
                    return None;
                };
                state.in_flight.iter().find_map(|flight| {
                    (flight.handle.correlation() == event.correlation)
                        .then(|| flight.handle.clone())
                })
            });
        let Some(handle) = handle else {
            return Some(event);
        };
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            crate::task::RuntimeProgramOwner::Plan(Arc::clone(&self.plan)),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        if let Err(error) = event.admit_retained(Some(handle.spec()), &mut admission) {
            self.fail_eval(error, output);
            return None;
        }
        let correlation = event.correlation;
        let cursor = event.cursor;
        let publication = match event.kind {
            TaskEventKind::Ready(value) => RuntimeNeedPublication::State {
                correlation,
                cursor,
                state: Need::Ready(crate::task::RuntimeNeedOutcome::Value(value)),
            },
            TaskEventKind::Progress(progress) => RuntimeNeedPublication::State {
                correlation,
                cursor,
                state: Need::Pending(progress),
            },
            TaskEventKind::Cancelled => RuntimeNeedPublication::State {
                correlation,
                cursor,
                state: Need::Cancelled,
            },
            TaskEventKind::InfrastructureFailure(failure) => {
                RuntimeNeedPublication::InfrastructureFailure {
                    correlation,
                    cursor,
                    failure,
                }
            }
            TaskEventKind::ProgramTerminal(_) => {
                unreachable!("Host fanout publication was admitted before moving it")
            }
        };
        self.enqueue_need_publication(publication, output);
        None
    }

    pub(super) fn resume_suspended(
        &mut self,
        input: &mut RuntimeStepInput,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        let status = std::mem::replace(&mut self.fiber.frame.status, FlowFiberStatus::Running);
        match status {
            FlowFiberStatus::ActionWaiting(super::ActionWaitingStatus::Native(state)) => {
                if !self.actions.has_message(state.payload) {
                    self.fiber.frame.status =
                        FlowFiberStatus::ActionWaiting(super::ActionWaitingStatus::Native(state));
                    return false;
                }
                if let Err(error) = self.execute_action_receive(&state.source) {
                    self.fail_eval(error, output);
                }
                true
            }
            FlowFiberStatus::ActionWaiting(super::ActionWaitingStatus::Observed { .. }) => false,
            FlowFiberStatus::Dialogue(activation) => {
                let transaction =
                    match self.begin_dialogue_activation_transaction(activation.activation()) {
                        Ok(transaction) => transaction,
                        Err(error) => {
                            self.fail_eval(error, output);
                            return true;
                        }
                    };
                self.resume_dialogue_state(transaction, input, events, output, pure_backend);
                true
            }
            FlowFiberStatus::NeedWaiting(state) => {
                self.resume_await_state(*state, output);
                true
            }
            FlowFiberStatus::WaitingMany(WaitingManyStatus::Native(state)) => {
                self.resume_await_many_state(*state, output, pure_backend);
                true
            }
            FlowFiberStatus::WaitingMany(WaitingManyStatus::Program(state)) => {
                self.resume_program_fanout_state(*state, output, pure_backend);
                true
            }
            waiting @ FlowFiberStatus::WaitingMany(WaitingManyStatus::Observed(_)) => {
                self.fiber.frame.status = waiting;
                false
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
            | FlowFiberStatus::DurationWaiting(_)
            | FlowFiberStatus::Done(_)
            | FlowFiberStatus::Failed(_)) => {
                self.fiber.frame.status = status;
                false
            }
        }
    }

    fn resume_dialogue_state(
        &mut self,
        mut transaction: super::dialogue::DialogueActivationTransaction,
        input: &mut RuntimeStepInput,
        events: &[TaskEvent],
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
            let (transaction, step) = self.resume_dialogue_activation(
                transaction,
                &line_outcomes,
                input,
                events,
                pure_backend,
            );
            match step {
                Ok(step) => self.commit_and_suspend_dialogue(transaction, output, step),
                Err(error) => self.begin_dialogue_failure(transaction, error, output),
            }
            return;
        }
        let content_id = transaction.frame().content;
        let plan = std::sync::Arc::clone(&self.plan);
        let Some(content) = plan.dialogue_content().get(content_id) else {
            self.begin_dialogue_failure(
                transaction,
                crate::line_task::LineRuntimeError::UnknownContentPlan.into(),
                output,
            );
            return;
        };
        let content_events = transaction.take_content_events();
        let advance = transaction.take_advance();
        let input_actions = transaction.take_input_actions();
        let (frame, line) = transaction.parts_mut();
        let mut start = None;
        let mut callback_sites = Vec::new();
        let mut task_inputs = None;
        if let DialogueLineTaskState::Live(line_task) = &mut frame.line_task {
            let Some(group) = plan.line_task_groups().get(frame.task_group.index()) else {
                self.begin_dialogue_failure(
                    transaction,
                    crate::line_task::LineRuntimeError::UnknownTaskGroup.into(),
                    output,
                );
                return;
            };
            if frame
                .task_inputs
                .iter()
                .any(|binding| !binding.value.ownership().permits_copy())
            {
                self.begin_dialogue_failure(
                    transaction,
                    crate::line_task::LineRuntimeError::AffineGroupCapture.into(),
                    output,
                );
                return;
            }
            task_inputs = Some(frame.task_inputs.to_vec().into_boxed_slice());
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
                        self.begin_dialogue_failure(transaction, error.into(), output);
                        return;
                    }
                };
            let requested_effects = content_events
                .iter()
                .filter_map(|event| match event {
                    crate::step::RuntimeDialogueContentEventKind::Mark(_) => None,
                    crate::step::RuntimeDialogueContentEventKind::Effect(site) => Some(*site),
                })
                .collect::<Vec<_>>();
            callback_sites = requested_effects;
            let input_actions = match line_task.accept_input_action_events(&input_actions) {
                Ok(actions) => actions,
                Err(error) => {
                    self.begin_dialogue_failure(transaction, error.into(), output);
                    return;
                }
            };
            let ready = accepted_content.ready().with_input_actions(&input_actions);
            if let Some((action, cancelled)) = cancel_live_line_task_group(group, ready, line_task)
            {
                let event = Some(FlowEvent::LineCancelled {
                    trigger: action.as_str().to_owned(),
                });
                start = Some(super::dialogue::DialogueLineTaskStart {
                    event,
                    request_cancellation: true,
                    group: frame.task_group,
                    activation: cancelled,
                    captures: task_inputs.take().expect("live task inputs were checked"),
                    callback_sites: std::mem::take(&mut callback_sites),
                });
                self.commit_and_suspend_dialogue(
                    transaction,
                    output,
                    start.map_or(
                        super::dialogue::DialogueActivationStep::Continue,
                        super::dialogue::DialogueActivationStep::Reveal,
                    ),
                );
                return;
            }
            if line_task.is_closing() || line_task.is_closed() {
                let activation = finalize_live_line_task_close(group, line_task);
                if let Some(exit) = line_task.closed_exit() {
                    frame.line_task = DialogueLineTaskState::Closed(exit);
                    frame.phase = DialogueRuntimePhase::Closing;
                }
                start = Some(super::dialogue::DialogueLineTaskStart {
                    event: None,
                    request_cancellation: false,
                    group: frame.task_group,
                    activation,
                    captures: task_inputs.take().expect("live task inputs were checked"),
                    callback_sites: std::mem::take(&mut callback_sites),
                });
                self.commit_and_suspend_dialogue(
                    transaction,
                    output,
                    start.map_or(
                        super::dialogue::DialogueActivationStep::Continue,
                        super::dialogue::DialogueActivationStep::Reveal,
                    ),
                );
                return;
            }
            if let Err(error) = line.advance_schedules(frame.elapsed, line_task) {
                self.begin_dialogue_failure(transaction, error.into(), output);
                return;
            }
            let activation =
                match progress_live_line_task_group(group, frame.elapsed, ready, line_task) {
                    Ok(activation) => activation,
                    Err(error) => {
                        self.begin_dialogue_failure(transaction, error.into(), output);
                        return;
                    }
                };
            start = Some(super::dialogue::DialogueLineTaskStart {
                event: None,
                request_cancellation: false,
                group: frame.task_group,
                activation,
                captures: Box::new([]),
                callback_sites: Vec::new(),
            });
        } else if !content_events.is_empty() {
            let event = content_events[0];
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
                && let Some(group) = plan.line_task_groups().get(frame.task_group.index())
            {
                let cleanup = finish_live_line_task_group(group, line_task);
                start = Some(super::dialogue::DialogueLineTaskStart {
                    event: None,
                    request_cancellation: true,
                    group: frame.task_group,
                    activation: cleanup,
                    captures: Box::new([]),
                    callback_sites: Vec::new(),
                });
            }
            if let Some(exit) = frame.line_task.closed_exit() {
                frame.line_task = DialogueLineTaskState::Closed(exit);
                frame.phase = DialogueRuntimePhase::Closing;
            }
        }
        if let Some(start) = &mut start {
            start.captures = task_inputs.take().expect("live task inputs were checked");
            start.callback_sites = callback_sites;
        }
        self.commit_and_suspend_dialogue(
            transaction,
            output,
            start.map_or(
                super::dialogue::DialogueActivationStep::Continue,
                super::dialogue::DialogueActivationStep::Reveal,
            ),
        );
    }

    pub(super) fn resume_await_state(
        &mut self,
        mut state: AwaitState,
        output: &mut RuntimeStepOutput,
    ) {
        let plan = Arc::clone(&self.plan);
        let super::AwaitSource::Native(source) = &state.source else {
            self.fail_eval(
                "native Await cannot execute an AWBC status observation",
                output,
            );
            return;
        };
        let Some(crate::plan::FlowOp::Await { binding, .. }) = source.operation.operation(&plan)
        else {
            self.fail_eval("native Await lost its original operation", output);
            return;
        };
        if self
            .need_producers
            .launch_for_handle(&state.handle)
            .is_some_and(|launch| {
                launch.execution_kind() == crate::task::TaskExecutionKind::Program
                    && launch.state().is_terminal()
            })
        {
            self.resume_local_await_state(state, output);
            return;
        }
        let publication = match state.queued.pop_front() {
            Some(publication) => Some(publication),
            None => {
                match self.next_need_publication_for_await(&state.handle, state.observed_through) {
                    Ok(publication) => publication,
                    Err(error) => {
                        self.fail_eval(error, output);
                        return;
                    }
                }
            }
        };
        let Some(publication) = publication else {
            self.fiber.frame.status = FlowFiberStatus::NeedWaiting(Box::new(state));
            return;
        };
        if publication.correlation() != state.handle.correlation() {
            self.fail_eval(
                "Await publication differs from its retained handle correlation",
                output,
            );
            return;
        }
        let publication_cursor = publication.cursor();
        if let Some(queued_cursor) = state.queued.back().map(RuntimeNeedPublication::cursor) {
            match state.observed_through {
                None => state.observed_through = Some(queued_cursor),
                Some(cursor) => match cursor.compare_same_source(queued_cursor) {
                    Some(std::cmp::Ordering::Less) => state.observed_through = Some(queued_cursor),
                    Some(_) => {}
                    None => {
                        self.fail_eval("Await publication changed cursor authority", output);
                        return;
                    }
                },
            }
        }
        match state.observed_through {
            None => state.observed_through = Some(publication_cursor),
            Some(cursor) => match cursor.compare_same_source(publication_cursor) {
                Some(std::cmp::Ordering::Less) => state.observed_through = Some(publication_cursor),
                Some(_) => {}
                None => {
                    self.fail_eval("Await publication changed cursor authority", output);
                    return;
                }
            },
        }
        match publication {
            RuntimeNeedPublication::InfrastructureFailure {
                correlation,
                failure,
                ..
            } => {
                let message = format!("await Need {} failed: {failure}", correlation.need);
                self.fiber.frame.status = FlowFiberStatus::Failed(message.clone());
                output.diagnostics.push(RuntimeDiagnostic::categorized(
                    RuntimeDiagnosticCategory::Host,
                    message,
                ));
            }
            RuntimeNeedPublication::State {
                state: need_state, ..
            } => match need_state {
                Need::NotStarted => {
                    self.fiber.frame.status = FlowFiberStatus::NeedWaiting(Box::new(state));
                }
                Need::Pending(progress) => {
                    output.flow_events.push(FlowEvent::AwaitProgress {
                        need: state.handle.need_id(),
                        progress: progress.clone(),
                    });
                    self.start_await_pending_observer(state, progress, output);
                }
                Need::Ready(crate::task::RuntimeNeedOutcome::InfrastructureFailure(failure)) => {
                    self.fail_eval(failure, output);
                }
                Need::Ready(crate::task::RuntimeNeedOutcome::Value(value)) => {
                    let type_matches = match state.item_type() {
                        AwaitItemType::Plan(item_type) => self
                            .plan
                            .validate_live_value(
                                item_type,
                                value.value(),
                                crate::entry::RuntimeSchemaLimits::engine_default(),
                            )
                            .is_ok(),
                        AwaitItemType::Awbc(_) => false,
                    };
                    if !type_matches {
                        let message = format!(
                            "await Need {} published a payload outside its checked item type",
                            state.handle.need_id()
                        );
                        self.fiber.frame.status = FlowFiberStatus::Failed(message.clone());
                        output.diagnostics.push(RuntimeDiagnostic::categorized(
                            RuntimeDiagnosticCategory::Host,
                            message,
                        ));
                        return;
                    }
                    if let Some(binding) = binding {
                        match crate::pattern::inspect_runtime_pattern_owned(
                            &self.plan,
                            binding,
                            value.value(),
                            state.source_context.instantiation(),
                        ) {
                            Ok(true) => {}
                            Ok(false) => {
                                self.fiber.frame.status = FlowFiberStatus::Failed(
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
                    let observed = value
                        .value()
                        .ownership()
                        .permits_copy()
                        .then(|| value.clone());
                    if let Some(binding) = binding {
                        let bindings = crate::pattern::match_runtime_pattern_owned(
                            &self.plan,
                            binding,
                            value.into_value(),
                            state.source_context.instantiation(),
                        )
                        .expect("checked external Need binding remains valid")
                        .expect("checked external Need binding matches");
                        self.fiber.frame.env.bind_all(bindings);
                    }
                    output.flow_events.push(FlowEvent::AwaitReady {
                        need: state.handle.need_id(),
                        value: observed,
                    });
                    self.fiber.frame.status = FlowFiberStatus::Running;
                }
                Need::Cancelled => {
                    self.fiber.frame.status = FlowFiberStatus::Done(FlowExit::Done);
                }
            },
            RuntimeNeedPublication::Producer { .. } => self.resume_local_await_state(state, output),
        }
    }

    fn resume_local_await_state(&mut self, state: AwaitState, output: &mut RuntimeStepOutput) {
        let plan = Arc::clone(&self.plan);
        let super::AwaitSource::Native(source) = &state.source else {
            self.fail_eval("local Native Await requires its original operation", output);
            return;
        };
        let Some(crate::plan::FlowOp::Await { binding, .. }) = source.operation.operation(&plan)
        else {
            self.fail_eval("local Native Await lost its original operation", output);
            return;
        };
        let Some(launch) = self.need_producers.launch_for_handle(&state.handle) else {
            self.fail_eval("local Need publication has no producer launch", output);
            return;
        };
        match launch.state() {
            RuntimeNeedProducerState::NotStarted => {
                self.fiber.frame.status = FlowFiberStatus::NeedWaiting(Box::new(state));
            }
            RuntimeNeedProducerState::InfrastructureFailure(failure) => {
                self.fail_eval(failure.clone(), output);
            }
            RuntimeNeedProducerState::Pending(progress) => {
                let progress = progress.clone();
                output.flow_events.push(FlowEvent::AwaitProgress {
                    need: state.handle.need_id(),
                    progress: progress.clone(),
                });
                self.start_await_pending_observer(state, progress, output);
            }
            RuntimeNeedProducerState::Ready(value) => {
                let type_matches = match state.item_type() {
                    AwaitItemType::Plan(item_type) => self
                        .plan
                        .validate_live_value(
                            item_type,
                            value.value(),
                            crate::entry::RuntimeSchemaLimits::engine_default(),
                        )
                        .is_ok(),
                    AwaitItemType::Awbc(_) => false,
                };
                if !type_matches {
                    self.fail_eval(
                        "local Need Ready differs from its checked item type",
                        output,
                    );
                    return;
                }
                if let Some(binding) = binding {
                    match crate::pattern::inspect_runtime_pattern_owned(
                        &self.plan,
                        binding,
                        value.value(),
                        state.source_context.instantiation(),
                    ) {
                        Ok(true) => {}
                        Ok(false) => {
                            self.fail_eval("await result did not match binding pattern", output);
                            return;
                        }
                        Err(error) => {
                            self.fail_eval(error, output);
                            return;
                        }
                    }
                }
                let observed = value
                    .value()
                    .ownership()
                    .permits_copy()
                    .then(|| value.clone());
                let value = self
                    .need_producers
                    .take_ready_for_correlation(&state.handle.correlation())
                    .expect("checked producer Ready remains available");
                if let Some(binding) = binding {
                    let bindings = crate::pattern::match_runtime_pattern_owned(
                        &self.plan,
                        binding,
                        value.into_value(),
                        state.source_context.instantiation(),
                    )
                    .expect("checked await binding remains valid")
                    .expect("checked await binding matches");
                    self.fiber.frame.env.bind_all(bindings);
                }
                output.flow_events.push(FlowEvent::AwaitReady {
                    need: state.handle.need_id(),
                    value: observed,
                });
                self.fiber.frame.status = FlowFiberStatus::Running;
            }
            RuntimeNeedProducerState::ReadyTransferred => {
                self.fail_eval("local Need Ready was already consumed", output);
            }
            RuntimeNeedProducerState::Cancelled => {
                self.fiber.frame.status = FlowFiberStatus::Done(FlowExit::Done);
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
        let plan = Arc::clone(&self.plan);
        let super::AwaitSource::Native(source) = &state.source else {
            self.fail_eval(
                "native Await observer requires its original Plan operation",
                output,
            );
            return;
        };
        let Some(crate::plan::FlowOp::Await { observers, .. }) = source.operation.operation(&plan)
        else {
            self.fail_eval("native Await observer lost its original operation", output);
            return;
        };
        for (ordinal, observer) in observers.iter().enumerate() {
            match crate::pattern::inspect_runtime_pattern_owned(
                &plan,
                &observer.pattern,
                &value,
                state.source_context.instantiation(),
            ) {
                Ok(true) => {
                    let bindings = crate::pattern::match_runtime_pattern_owned(
                        &plan,
                        &observer.pattern,
                        value,
                        state.source_context.instantiation(),
                    )
                    .expect("checked Pending observer remains valid")
                    .expect("checked Pending observer matches");
                    if let Err(error) =
                        self.push_await_observer_ops(bindings, &source.operation, ordinal)
                    {
                        self.fail_eval(error, output);
                        return;
                    }
                    self.fiber.frame.await_observer = Some(Box::new(state));
                    self.fiber.frame.status = FlowFiberStatus::Running;
                    return;
                }
                Ok(false) => {}
                Err(error) => {
                    self.fail_eval(error, output);
                    return;
                }
            }
        }
        self.fiber.frame.status = FlowFiberStatus::NeedWaiting(Box::new(state));
    }

    pub(super) fn latch_active_await_observer_publications(&mut self) -> Result<(), String> {
        let Some(state) = self.fiber.frame.await_observer.as_ref() else {
            return Ok(());
        };
        let queued_cursor = state.queued.back().map(RuntimeNeedPublication::cursor);
        let queued_through = match (state.observed_through, queued_cursor) {
            (None, cursor) | (cursor, None) => cursor,
            (Some(observed), Some(queued)) => match observed.compare_same_source(queued) {
                Some(std::cmp::Ordering::Less) => Some(queued),
                Some(_) => Some(observed),
                None => Some(observed),
            },
        };
        let correlation = state.handle.correlation();
        let Some(queue) = self.need_publications.get_mut(&correlation) else {
            return Ok(());
        };
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            crate::task::RuntimeProgramOwner::Plan(Arc::clone(&self.plan)),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        admission
            .validate_metadata_collection(queue.len())
            .map_err(|error| error.to_string())?;
        // Borrow the complete selection first. A later rejection must leave
        // earlier affine Ready owners and the observer cursor untouched.
        let mut through = queued_through;
        while let Some(index) = next_need_publication_index(queue, through) {
            let publication = &queue[index];
            publication
                .admit_for_await((&state.handle).into(), &mut admission)
                .map_err(|error| error.to_string())?;
            through = Some(publication.cursor());
        }
        let mut through = queued_through;
        let mut additions = Vec::new();
        while let Some(publication) = take_admitted_need_publication(queue, through) {
            through = Some(publication.cursor());
            additions.push(publication);
        }
        let state = self
            .fiber
            .frame
            .await_observer
            .as_mut()
            .expect("await observer remains active while publications are staged");
        for publication in additions {
            let cursor = publication.cursor();
            state.observed_through = Some(match state.observed_through {
                None => cursor,
                Some(previous) => match previous.compare_same_source(cursor) {
                    Some(std::cmp::Ordering::Less) => cursor,
                    Some(_) => previous,
                    None => previous,
                },
            });
            state.queued.push_back(publication);
        }
        Ok(())
    }

    pub(super) fn await_has_pending_work(&self, state: &AwaitState) -> bool {
        !state.queued.is_empty()
            || self
                .need_producers
                .launch_for_handle(&state.handle)
                .is_some_and(|launch| {
                    launch.execution_kind() == crate::task::TaskExecutionKind::Program
                        && launch.state().is_terminal()
                })
            || self
                .need_publications
                .get(&state.handle.correlation())
                .is_some_and(|queue| {
                    next_need_publication_index(queue, state.observed_through).is_some()
                })
    }

    /// Selects a publication for one observer. An affine Ready moves out of
    /// the shared queue; only recursively unrestricted publications may be
    /// copied for another observer of the same Need.
    fn next_need_publication_for_await(
        &mut self,
        handle: &crate::task::RuntimeNeedHandle,
        through: Option<crate::task::TaskPublicationCursor>,
    ) -> Result<Option<RuntimeNeedPublication>, String> {
        let Some(queue) = self.need_publications.get_mut(&handle.correlation()) else {
            return Ok(None);
        };
        let Some(index) = next_need_publication_index(queue, through) else {
            return Ok(None);
        };
        queue[index]
            .admit_for_await(
                handle.into(),
                &mut crate::value::RuntimeRetainedValueAdmission::for_program(
                    crate::task::RuntimeProgramOwner::Plan(Arc::clone(&self.plan)),
                    crate::entry::RuntimeSchemaLimits::engine_default(),
                ),
            )
            .map_err(|error| error.to_string())?;
        Ok(take_admitted_need_publication(queue, through))
    }

    pub(super) fn reserve_new_task_request(&mut self) -> bool {
        if self.task_request_quota_remaining == 0 {
            return false;
        }
        self.task_request_quota_remaining -= 1;
        true
    }

    pub(super) fn fail_task_request_quota(
        &mut self,
        message: impl Into<String>,
        output: &mut RuntimeStepOutput,
    ) {
        let message = message.into();
        self.fiber.frame.status = FlowFiberStatus::Failed(message.clone());
        output.diagnostics.push(RuntimeDiagnostic::categorized(
            RuntimeDiagnosticCategory::Budget,
            message,
        ));
    }

    /// Deliver replies before spending the source-operation budget. The call
    /// instruction was charged when it issued the request; reply admission and
    /// binding belong to the suspended frame, including with a zero-op budget.
    pub(super) fn latch_host_call_results(
        &mut self,
        input: &mut RuntimeStepInput,
        output: &mut RuntimeStepOutput,
    ) {
        self.latch_active_host_call_result(input, output);
        for index in 0..self.child_fibers.len() {
            if matches!(&self.child_fibers[index].frame.status,
                FlowFiberStatus::HostCall(state) if state.result_index(input).is_some())
            {
                std::mem::swap(&mut self.fiber, &mut self.child_fibers[index]);
                self.latch_active_host_call_result(input, output);
                std::mem::swap(&mut self.fiber, &mut self.child_fibers[index]);
            }
        }
    }

    fn latch_active_host_call_result(
        &mut self,
        input: &mut RuntimeStepInput,
        output: &mut RuntimeStepOutput,
    ) {
        if self.fiber.owner.requests_scope_close()
            || !matches!(&self.fiber.frame.status,
                FlowFiberStatus::HostCall(state) if state.result_index(input).is_some())
        {
            return;
        }
        let FlowFiberStatus::HostCall(state) =
            std::mem::replace(&mut self.fiber.frame.status, FlowFiberStatus::Running)
        else {
            unreachable!("the inspected frame still owns its pending Host call")
        };
        self.resume_host_call_state(state, input, output);
    }

    pub(super) fn resume_host_call_state(
        &mut self,
        state: HostCallState,
        input: &mut RuntimeStepInput,
        output: &mut RuntimeStepOutput,
    ) {
        let Some(index) = state.result_index(input) else {
            self.fiber.frame.status = FlowFiberStatus::HostCall(state);
            return;
        };
        let plan = Arc::clone(&self.plan);
        let Some((binding, target)) = state
            .continuation()
            .and_then(|state| state.declaration(&plan))
        else {
            let message = super::host_call::invalid_host_call().to_string();
            self.fiber.frame.status = FlowFiberStatus::Failed(message.clone());
            output.diagnostics.push(RuntimeDiagnostic::categorized(
                RuntimeDiagnosticCategory::Host,
                message,
            ));
            return;
        };
        let result = input.host_call_results.remove(index);
        match result.outcome {
            Ok(value) => {
                if let Err(error) = plan.validate_live_value(
                    target.result,
                    value.value(),
                    crate::entry::RuntimeSchemaLimits::engine_default(),
                ) {
                    self.fiber.frame.status = FlowFiberStatus::Failed(error.to_string());
                    output.diagnostics.push(RuntimeDiagnostic::categorized(
                        RuntimeDiagnosticCategory::Host,
                        error.to_string(),
                    ));
                    return;
                }
                if let Some(binding) = binding {
                    match self.try_bind_pattern_owned(binding, value.into_value()) {
                        Ok(None) => {}
                        Ok(Some(_value)) => {
                            self.fiber.frame.status = FlowFiberStatus::Failed(
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
                self.fiber.frame.status = FlowFiberStatus::Running;
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
                self.fiber.frame.status = FlowFiberStatus::Failed(error.message.clone());
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
        let admitted = self.native_work_admission().and_then(|mut admission| {
            state.admit_native(&mut admission).map_err(|source| {
                RuntimeEvalError::NativeWorkAdmission {
                    source: Box::new(source),
                }
            })
        });
        if let Err(error) = admitted {
            self.fail_eval(error, output);
            return;
        }
        if state.selected().is_none() {
            let Some((requested_choice, selection)) = input_choice_selection(input) else {
                self.fiber.frame.status = FlowFiberStatus::Choice(state);
                return;
            };
            if requested_choice.is_some() && requested_choice != state.id() {
                output.diagnostics.push(RuntimeDiagnostic::categorized(
                    RuntimeDiagnosticCategory::Input,
                    format!(
                        "stale choice selection for `{}` while waiting on `{}`",
                        requested_choice.unwrap_or_default(),
                        state.id().unwrap_or("-")
                    ),
                ));
                self.fiber.frame.status = FlowFiberStatus::Choice(state);
                return;
            }
            let Some(position) = state.options().iter().position(|option| {
                option.id.as_deref() == Some(selection) || option.label == selection
            }) else {
                output.diagnostics.push(RuntimeDiagnostic::categorized(
                    RuntimeDiagnosticCategory::Input,
                    format!(
                        "invalid option `{selection}` for choice `{}`",
                        state.id().unwrap_or("-")
                    ),
                ));
                self.fiber.frame.status = FlowFiberStatus::Choice(state);
                return;
            };
            state.select(position);
        }
        if state.selected_has_target() && self.has_joined_work_for(self.fiber.id) {
            self.fiber.frame.status = FlowFiberStatus::Choice(state);
            return;
        }
        let position = state.selected().expect("accepted original Choice ordinal");
        let option = &state.options()[position];
        let selected = option.id.clone().unwrap_or_else(|| option.label.clone());
        output.flow_events.push(FlowEvent::ChoiceSelected {
            id: state.id().map(str::to_owned),
            option: selected,
        });
        self.emit_line_effects(option.effects.clone(), output, pure_backend);
        if let Some(out) = &option.out {
            self.emit_line_effect(LineEffectRequest::Out(out.clone()), output, pure_backend);
        }
        if let Some(target) = &option.target {
            self.goto(target, state.selected_goto(), output, pure_backend);
        } else {
            self.fiber.frame.status = FlowFiberStatus::Running;
        }
    }

    pub(super) fn start_need_await(
        &mut self,
        source: super::code_cursor::NativeSourceOperation,
        next_op_index: Option<usize>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let plan = Arc::clone(&self.plan);
        let Some(crate::plan::FlowOp::Await { target, .. }) = source.operation(&plan) else {
            self.fail_eval("native Await requires its original operation", output);
            return;
        };
        let admitted = self.native_work_admission().and_then(|mut admission| {
            source.admit_retained(&mut admission).map_err(|source| {
                RuntimeEvalError::NativeWorkAdmission {
                    source: Box::new(source),
                }
            })
        });
        if let Err(error) = admitted {
            self.fail_eval(error, output);
            return;
        }
        let value = match self.evaluate_expr_with_backend(target.source(), pure_backend) {
            Ok(value) => value,
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        let need = match value {
            RuntimeValue::NeedHandle(need) => need,
            value => {
                self.fail_eval(
                    format!(
                        "Await source produced {}, expected Need<T> or ThreadHandle<T>",
                        super::runtime_value_label(&value)
                    ),
                    output,
                );
                return;
            }
        };
        let item_type = match self
            .plan
            .type_table()
            .get(target.source().ty())
            .map(|declaration| declaration.projection())
        {
            Some(
                crate::plan::RuntimePlanTypeProjection::Need(item)
                | crate::plan::RuntimePlanTypeProjection::ThreadHandle(item),
            ) => *item,
            _ => {
                self.fail_eval("Await source type is not a checked awaitable", output);
                return;
            }
        };
        let correlation = need.correlation();
        if self.need_producers.launch_for_handle(&need).is_none() {
            self.fail_eval(
                "Await handle does not belong to its complete accepted producer context",
                output,
            );
            return;
        }
        self.advance_if_needed(next_op_index);
        let task = Some(correlation.task_id);
        output.flow_events.push(FlowEvent::AwaitStarted {
            need: correlation.need,
            task,
        });
        let queued = VecDeque::new();
        let state = AwaitState {
            source_context: self.fiber.frame.env.source_function_context(),
            source: super::AwaitSource::Native(super::NativeAwaitSource {
                operation: source,
                item_type,
            }),
            handle: need,
            observed_through: None,
            queued,
        };
        self.resume_await_state(state, output);
    }

    pub(super) fn start_need_producer(
        &mut self,
        binding: crate::pattern::RuntimePattern,
        target: crate::plan::RuntimeNeedProducerStartTarget,
        next_op_index: Option<usize>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let source_program =
            crate::plan::task_source::RuntimeTaskSourceProgram::native(Arc::clone(&self.plan));
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            source_program.program_owner(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        let limits = crate::plan::RuntimeTaskPlanSealLimits::default();
        let mut meter = match admission.task_source_meter(limits) {
            Ok(meter) => meter,
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        let source =
            match source_program
                .original()
                .task_source(target.task_plan(), limits, &mut meter)
            {
                Ok(source) => source,
                Err(error) => {
                    self.fail_eval(error, output);
                    return;
                }
            };
        let prepared = match source.prepare_selected_producer(&source_program, &mut admission) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        for (index, argument) in prepared.arguments().iter().enumerate() {
            let expression = argument.value();
            let Some(expected) = prepared.argument_types().get(index).copied() else {
                self.fail_eval("Need producer argument type row is incomplete", output);
                return;
            };
            let actual = self
                .plan
                .type_table()
                .get(expression.ty())
                .map(|declaration| declaration.semantic_identity());
            if actual != Some(expected) {
                self.fail_eval(
                    format!("Need producer argument {index} differs from its checked source type"),
                    output,
                );
                return;
            }
        }
        let mut arguments = Vec::with_capacity(prepared.arguments().len());
        for (index, argument) in prepared.arguments().iter().enumerate() {
            let expression = argument.value();
            let value = match self.evaluate_expr_with_backend(expression, pure_backend) {
                Ok(value) => value,
                Err(error) => {
                    self.fail_eval(error, output);
                    return;
                }
            };
            arguments.push(NeedProducerRuntimeArgument {
                name: prepared.argument_name(index).map(str::to_owned),
                value,
            });
        }
        let start = match prepared.start(
            self.generation,
            arguments,
            self.fiber.frame.env.function_instantiation(),
        ) {
            Ok(start) => start,
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        let proof = match self
            .need_producers
            .inspect_start_visit(self.fiber.persistent_id, start)
        {
            Ok(proof) => proof,
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        let ensure = proof.admission().disposition() == NeedProducerTaskDisposition::Ensure;
        if ensure && self.task_request_quota_remaining == 0 {
            self.fail_task_request_quota(
                "Need producer start exceeds the remaining task request quota",
                output,
            );
            return;
        }
        let bound = self
            .need_producers
            .commit_start_visit_with(proof, |handle| {
                crate::pattern::match_runtime_pattern_owned(
                    &self.plan,
                    &binding,
                    RuntimeValue::NeedHandle(handle),
                    self.fiber.frame.env.function_instantiation(),
                )
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "Need producer result did not match its checked binding".to_owned())
            });
        let (admission, bindings) = match bound {
            Ok(bound) => bound,
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        if ensure {
            self.task_request_quota_remaining -= 1;
        }
        let task_spec = (admission.disposition() == NeedProducerTaskDisposition::Ensure)
            .then(|| self.need_producers.submission_for_admission(&admission));
        self.fiber.frame.env.bind_all(bindings);
        self.advance_if_needed(next_op_index);
        if let Some(task_spec) = task_spec {
            output.requests.tasks.push(task_spec);
        }
    }

    pub(super) fn start_await_many_state(
        &mut self,
        operation: super::code_cursor::NativeSourceOperation,
        next_op_index: Option<usize>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let plan = Arc::clone(&self.plan);
        let Some((binding, target, pending)) = operation.await_many(&plan) else {
            self.fail_eval("native AwaitMany requires its original operation", output);
            return;
        };
        self.emit_line_effects(pending.iter().cloned(), output, pure_backend);
        let source_value = match self.evaluate_expr_with_backend(&target.source, pure_backend) {
            Ok(value) => value,
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        let program = RuntimeTaskSourceProgram::native(Arc::clone(&self.plan));
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            program.program_owner(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        let limits = RuntimeTaskPlanSealLimits::default();
        let prepared = (|| {
            let mut meter = admission
                .task_source_meter(limits)
                .map_err(|error| error.to_string())?;
            let source = program
                .original()
                .task_source(&target.base.definition.plan, limits, &mut meter)
                .map_err(|error| error.to_string())?;
            admission
                .validate_metadata_collection(target.base.request.captures().len())
                .map_err(|error| error.to_string())?;
            admission
                .validate_metadata_collection(target.child.request.captures().len())
                .map_err(|error| error.to_string())?;
            let mut values = Vec::new();
            target.try_visit_captures(|local| {
                admission
                    .validate_metadata_collection(
                        values
                            .len()
                            .checked_add(1)
                            .ok_or("AwaitMany capture count overflow")?,
                    )
                    .map_err(|error| error.to_string())?;
                values.push(
                    self.fiber
                        .frame
                        .env
                        .get(local)
                        .ok_or("AwaitMany capture is unavailable")?,
                );
                Ok::<_, String>(())
            })?;
            let checked = source
                .template_producer_spec(
                    &program,
                    RuntimeTaskTemplateArguments::AwaitManyBase {
                        captures: &values,
                        source: RuntimeValueSource::Live(&source_value),
                    },
                    self.fiber.frame.env.function_instantiation(),
                    &mut admission,
                )
                .map_err(|error| error.to_string())?;
            let captured = values.into_iter().cloned().collect::<Vec<_>>();
            Ok::<_, String>((source, checked, captured))
        })();
        let (source, checked, captured) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        let items = match runtime_value_into_sequence_values(source_value) {
            Ok(items) => items,
            Err(value) => {
                self.fail_eval(
                    RuntimeEvalError::ExpectedBracketSeq(super::runtime_value_label(&value)),
                    output,
                );
                return;
            }
        };
        let arguments =
            match self.evaluate_host_task_arguments(&target.base.request.args, pure_backend) {
                Ok(request) => request,
                Err(error) => {
                    self.fail_eval(error, output);
                    return;
                }
            };
        let spec =
            match source.template_task_spec(&checked, self.generation, arguments, &mut admission) {
                Ok(spec) => spec,
                Err(error) => {
                    self.fail_eval(error, output);
                    return;
                }
            };
        let submission = match self.need_producers.ensure_task(spec.clone()) {
            Ok(submission) => submission,
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        let base =
            crate::task::RuntimeNeedHandle::try_from_accepted_launch(spec, submission.handle())
                .expect("aggregate base uses its actual accepted journal receipt");
        let mut state = AwaitManyState {
            source: operation,
            source_context: self.fiber.frame.env.source_function_context(),
            base,
            captured,
            next_index: 0,
            in_flight: Vec::new(),
            results: vec![None; items.len()],
            items,
        };
        if self.fill_await_many_queue(&mut state, target, output, pure_backend) {
            self.advance_if_needed(next_op_index);
            self.commit_await_many_state(state, target, binding, output);
        }
    }

    pub(super) fn resume_await_many_state(
        &mut self,
        mut state: AwaitManyState,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let plan = Arc::clone(&self.plan);
        let Some((binding, target, _)) = state.source.await_many(&plan) else {
            self.fail_eval("native AwaitMany requires its original operation", output);
            return;
        };
        let mut position = 0;
        while position < state.in_flight.len() {
            let in_flight = &state.in_flight[position];
            let publication = match self
                .next_need_publication_for_await(&in_flight.handle, in_flight.observed_through)
            {
                Ok(Some(publication)) => publication,
                Ok(None) => {
                    position += 1;
                    continue;
                }
                Err(error) => {
                    self.fiber.frame.status =
                        FlowFiberStatus::WaitingMany(WaitingManyStatus::Native(Box::new(state)));
                    self.fail_eval(error, output);
                    return;
                }
            };
            state.in_flight[position].observed_through = Some(publication.cursor());
            match publication {
                RuntimeNeedPublication::State {
                    state: Need::Ready(crate::task::RuntimeNeedOutcome::Value(value)),
                    ..
                } => {
                    let in_flight = state.in_flight.remove(position);
                    let observed = value
                        .value()
                        .ownership()
                        .permits_copy()
                        .then(|| value.clone());
                    state.results[in_flight.index] = Some(value);
                    output.flow_events.push(FlowEvent::AwaitReady {
                        need: in_flight.handle.need_id(),
                        value: observed,
                    });
                }
                RuntimeNeedPublication::State {
                    state: Need::Pending(progress),
                    ..
                } => {
                    let in_flight = &state.in_flight[position];
                    output.flow_events.push(FlowEvent::AwaitProgress {
                        need: in_flight.handle.need_id(),
                        progress,
                    });
                }
                RuntimeNeedPublication::InfrastructureFailure { failure: error, .. }
                | RuntimeNeedPublication::State {
                    state:
                        Need::Ready(crate::task::RuntimeNeedOutcome::InfrastructureFailure(error)),
                    ..
                } => {
                    let in_flight = &state.in_flight[position];
                    let message = format!(
                        "await task {} at index {} failed: {error}",
                        in_flight.handle.correlation().task_id,
                        in_flight.index
                    );
                    self.fiber.frame.status = FlowFiberStatus::Failed(message.clone());
                    output.diagnostics.push(RuntimeDiagnostic::categorized(
                        RuntimeDiagnosticCategory::Host,
                        message,
                    ));
                    return;
                }
                RuntimeNeedPublication::State {
                    state: Need::Cancelled,
                    ..
                } => {
                    self.fiber.frame.status = FlowFiberStatus::Done(FlowExit::Done);
                    return;
                }
                RuntimeNeedPublication::State {
                    state: Need::NotStarted,
                    ..
                } => {}
                RuntimeNeedPublication::Producer { .. } => {
                    self.fail_eval(
                        "AwaitMany Host result has a local producer publication",
                        output,
                    );
                    return;
                }
            }
        }
        if self.fill_await_many_queue(&mut state, target, output, pure_backend) {
            self.commit_await_many_state(state, target, binding, output);
        }
    }

    fn fill_await_many_queue(
        &mut self,
        state: &mut AwaitManyState,
        target: &AwaitManyTarget,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        while state.in_flight.len() < target.limit as usize && state.next_index < state.items.len()
        {
            if self.task_request_quota_remaining == 0 {
                if state.in_flight.is_empty() {
                    self.fail_task_request_quota(
                        "AwaitMany requires another task request after the task quota was exhausted",
                        output,
                    );
                    return false;
                }
                return true;
            }
            let index = state.next_index;
            let item = &state.items[index];
            if !item.ownership().permits_copy() {
                self.fail_eval(
                    "AwaitMany host request item requires its checked Copy input",
                    output,
                );
                return false;
            }
            let Some(spec) = self.await_many_task_spec(
                target,
                &state.captured,
                index,
                item,
                output,
                pure_backend,
            ) else {
                return false;
            };
            if !self.reserve_new_task_request() {
                if state.in_flight.is_empty() {
                    self.fail_task_request_quota(
                        "AwaitMany requires another task request after the task quota was exhausted",
                        output,
                    );
                    return false;
                }
                return true;
            }
            let submission = match self.need_producers.ensure_task(spec.clone()) {
                Ok(submission) => submission,
                Err(error) => {
                    self.fail_eval(error, output);
                    return false;
                }
            };
            let handle =
                crate::task::RuntimeNeedHandle::try_from_accepted_launch(spec, submission.handle())
                    .expect("child uses its actual accepted journal receipt");
            let correlation = handle.correlation();
            output.flow_events.push(FlowEvent::AwaitStarted {
                need: correlation.need,
                task: Some(correlation.task_id),
            });
            output.requests.tasks.push(submission);
            state.in_flight.push(AwaitManyInFlight {
                index,
                handle,
                observed_through: None,
            });
            state.next_index += 1;
        }
        true
    }

    fn commit_await_many_state(
        &mut self,
        mut state: AwaitManyState,
        target: &AwaitManyTarget,
        pattern: Option<&crate::pattern::RuntimePattern>,
        output: &mut RuntimeStepOutput,
    ) {
        if !state.in_flight.is_empty() || state.results.iter().any(Option::is_none) {
            self.fiber.frame.status =
                FlowFiberStatus::WaitingMany(WaitingManyStatus::Native(Box::new(state)));
            return;
        }
        let program = RuntimeTaskSourceProgram::native(Arc::clone(&self.plan));
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            program.program_owner(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        let aggregate = match admission.prepare_fanout(
            target.base.outcome().payload_semantic_identity(),
            target.child.outcome().payload_semantic_identity(),
            state.source_context.instantiation(),
            &mut state.results,
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.fiber.frame.status =
                    FlowFiberStatus::WaitingMany(WaitingManyStatus::Native(Box::new(state)));
                output
                    .diagnostics
                    .push(RuntimeDiagnostic::new(error.to_string()));
                return;
            }
        };
        let binding = if let Some(pattern) = pattern {
            let prepared = aggregate.with_view(|value| {
                admission.admit_composite(
                    crate::entry::schema::value_encoding::ValueAdmissionOrigin::Live,
                    |_, resources| {
                        let work = resources.work();
                        let result = crate::pattern::prepare_runtime_pattern_owned_view(
                            &self.plan,
                            pattern,
                            value,
                            self.fiber.frame.env.function_instantiation(),
                            &work,
                        );
                        if let Some(error) = work.failure() {
                            return Err(error.into());
                        }
                        result.map_err(|error| {
                            crate::value::AwbcRuntimeValueSnapshotError::Message {
                                message: error.to_string(),
                            }
                        })
                    },
                )
            });
            match prepared {
                Ok(Some(prepared)) => Some(prepared),
                refused => {
                    let message = match refused {
                        Err(error) => error.to_string(),
                        Ok(None) => "await result did not match binding pattern".to_owned(),
                        Ok(Some(_)) => unreachable!("accepted pattern handled above"),
                    };
                    aggregate.abort();
                    self.fiber.frame.status =
                        FlowFiberStatus::WaitingMany(WaitingManyStatus::Native(Box::new(state)));
                    output.diagnostics.push(RuntimeDiagnostic::new(message));
                    return;
                }
            }
        } else {
            None
        };
        let ready_value = aggregate.commit();
        let observed = ready_value
            .ownership()
            .permits_copy()
            .then(|| RuntimePayload::new(ready_value.clone()));
        if let Some(binding) = binding {
            self.fiber.frame.env.bind_all(binding.commit(ready_value));
        }
        output.flow_events.push(FlowEvent::AwaitReady {
            need: state.base.need_id(),
            value: observed,
        });
        self.fiber.frame.status = FlowFiberStatus::Running;
    }

    fn await_many_task_spec(
        &mut self,
        target: &AwaitManyTarget,
        captured: &[RuntimeValue],
        index: usize,
        item: &RuntimeValue,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Option<TaskSpec> {
        let program = RuntimeTaskSourceProgram::native(Arc::clone(&self.plan));
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            program.program_owner(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        let prepared = (|| {
            let limits = RuntimeTaskPlanSealLimits::default();
            let mut meter = admission
                .task_source_meter(limits)
                .map_err(|error| error.to_string())?;
            let source = program
                .original()
                .task_source(&target.child.definition.plan, limits, &mut meter)
                .map_err(|error| error.to_string())?;
            let checked = source
                .template_producer_spec(
                    &program,
                    RuntimeTaskTemplateArguments::AwaitManyChild {
                        captures: captured,
                        index: u32::try_from(index).map_err(|error| error.to_string())?,
                        item: RuntimeValueSource::Live(item),
                    },
                    self.fiber.frame.env.function_instantiation(),
                    &mut admission,
                )
                .map_err(|error| error.to_string())?;
            Ok::<_, String>((source, checked))
        })();
        let (source, checked) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.fail_eval(error, output);
                return None;
            }
        };
        let bindings = target
            .captures()
            .zip(captured)
            .map(|(local, value)| crate::value::RuntimeLocalBinding {
                local,
                value: value.clone(),
            })
            .collect::<Vec<_>>();
        let arguments = self.with_temp_bindings(bindings, |this| {
            this.with_temp_binding_ref(target.item_binding, item, |this| {
                this.evaluate_host_task_arguments(&target.child.request.args, pure_backend)
            })
        });
        let arguments = match arguments {
            Ok(arguments) => arguments,
            Err(error) => {
                self.fail_eval(format!("await many item {index}: {error}"), output);
                return None;
            }
        };
        match source.template_task_spec(&checked, self.generation, arguments, &mut admission) {
            Ok(spec) => Some(spec),
            Err(error) => {
                self.fail_eval(error, output);
                None
            }
        }
    }
    fn evaluate_host_task_arguments(
        &mut self,
        arguments: &[RuntimeHostArgumentTemplate],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
        arguments
            .iter()
            .map(|argument| self.evaluate_expr_with_backend(argument.value(), pure_backend))
            .collect()
    }
}

fn next_need_publication_index(
    queue: &VecDeque<RuntimeNeedPublication>,
    through: Option<crate::task::TaskPublicationCursor>,
) -> Option<usize> {
    queue.iter().position(|publication| {
        through.is_none_or(|cursor| {
            cursor.compare_same_source(publication.cursor()) == Some(std::cmp::Ordering::Less)
        })
    })
}

fn take_admitted_need_publication(
    queue: &mut VecDeque<RuntimeNeedPublication>,
    through: Option<crate::task::TaskPublicationCursor>,
) -> Option<RuntimeNeedPublication> {
    let index = next_need_publication_index(queue, through)?;
    queue[index]
        .copy_for_observer_admitted()
        .or_else(|| queue.remove(index))
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
