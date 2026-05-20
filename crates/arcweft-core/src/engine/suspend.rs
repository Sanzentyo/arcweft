use super::{
    AwaitState, AwaitTarget, CancelScopeId, ChoiceRuntimeOption, ChoiceState, Engine, FlowEvent,
    FlowFiberStatus, LineEffectRequest, RuntimeDiagnostic, RuntimeStepInput, RuntimeStepOutput,
    TaskClass, TaskEvent, TaskEventKind, TaskKey, TaskPolicy, TaskPriority, TaskSource, TaskSpec,
};

impl Engine {
    pub(super) fn resume_suspended(
        &mut self,
        input: &RuntimeStepInput,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
    ) -> bool {
        match self.fiber.status.clone() {
            FlowFiberStatus::Waiting(state) => {
                self.resume_await_state(state, events, output);
                true
            }
            FlowFiberStatus::Choice(state) => {
                self.resume_choice_state(state, input, output);
                true
            }
            FlowFiberStatus::Running | FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_) => {
                false
            }
        }
    }

    pub(super) fn resume_await_state(
        &mut self,
        state: AwaitState,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
    ) {
        let Some(event) = events
            .iter()
            .find(|event| event.task_id == state.target.task)
            .cloned()
        else {
            self.fiber.status = FlowFiberStatus::Waiting(state);
            return;
        };
        match event.kind {
            TaskEventKind::Ready(value) => {
                output.flow_events.push(FlowEvent::AwaitReady {
                    need: state.target.need,
                    value,
                });
                self.fiber.cursor = Some(state.resume);
                self.fiber.status = FlowFiberStatus::Running;
            }
            TaskEventKind::Progress(progress) => {
                output.flow_events.push(FlowEvent::AwaitProgress {
                    need: state.target.need.clone(),
                    progress,
                });
                self.fiber.status = FlowFiberStatus::Waiting(state);
            }
            TaskEventKind::Err(error) => {
                self.fiber.status = FlowFiberStatus::Failed(error.clone());
                output.diagnostics.push(RuntimeDiagnostic {
                    message: format!("await task {} failed: {error}", state.target.task.0),
                });
            }
            TaskEventKind::Cancelled => {
                let message = format!("await task {} was cancelled", state.target.task.0);
                self.fiber.status = FlowFiberStatus::Failed(message.clone());
                output.diagnostics.push(RuntimeDiagnostic { message });
            }
        }
    }

    pub(super) fn resume_choice_state(
        &mut self,
        state: ChoiceState,
        input: &RuntimeStepInput,
        output: &mut RuntimeStepOutput,
    ) {
        let Some(option) = state
            .options
            .iter()
            .find(|option| input_selects_choice(input, option))
            .cloned()
        else {
            self.fiber.status = FlowFiberStatus::Choice(state);
            return;
        };
        let selected = option.id.clone().unwrap_or_else(|| option.label.clone());
        output.flow_events.push(FlowEvent::ChoiceSelected {
            id: state.id.clone(),
            option: selected,
        });
        output.effects.line.extend(option.effects.clone());
        if let Some(out) = option.out {
            output.effects.line.push(LineEffectRequest::Out(out));
        }
        if let Some(target) = option.target {
            self.goto(target, output);
        } else {
            self.fiber.cursor = Some(state.resume);
            self.fiber.status = FlowFiberStatus::Running;
        }
    }
}

fn input_selects_choice(input: &RuntimeStepInput, option: &ChoiceRuntimeOption) -> bool {
    input.input_events.iter().any(|event| {
        let Some(payload) = event.payload.as_deref() else {
            return false;
        };
        matches!(event.kind.as_str(), "choice" | "select")
            && (option.id.as_deref() == Some(payload) || option.label == payload)
    })
}

pub(super) fn await_task_spec(target: &AwaitTarget) -> TaskSpec {
    TaskSpec {
        id: target.task.clone(),
        key: TaskKey(target.task.0.clone()),
        class: TaskClass::Background,
        priority: TaskPriority(0),
        cancel_scope: CancelScopeId("flow".to_owned()),
        policy: TaskPolicy::JoinSameKey,
        source: TaskSource {
            label: format!("await {}", target.need.0),
        },
    }
}
