use super::{
    Engine, FlowControlStackEntryKind, FlowCursor, FlowEvent, FlowExit, FlowFiberStatus,
    FlowRuntimeId, LineEffectRequest, RuntimeStepInput, RuntimeStepOutput,
    run_line_task_group_for_input,
};
use crate::pure::RuntimeCallBackend;

impl Engine {
    pub(super) fn step_line_only(
        &mut self,
        input: &RuntimeStepInput,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let Some(group) = self.plan.line_task_groups.get(self.fiber.line_cursor) else {
            self.finish(output);
            return;
        };
        self.merge_step_output(
            run_line_task_group_for_input(group, input),
            output,
            pure_backend,
        );
        self.fiber.line_cursor += 1;
        if self.fiber.line_cursor >= self.plan.line_task_groups.len() {
            self.finish(output);
        }
    }

    pub(super) fn apply_control_effects(&mut self, output: &mut RuntimeStepOutput) -> bool {
        let Some(control) = output.effects.line.iter().find_map(control_from_effect) else {
            return false;
        };
        match control {
            FlowControl::Goto(target) => self.goto(&FlowRuntimeId(target), output),
            FlowControl::Return(value) => self.return_value(value, output),
            FlowControl::Failed(message) => self.fiber.status = FlowFiberStatus::Failed(message),
        }
        true
    }

    pub(super) fn goto(&mut self, target: &FlowRuntimeId, output: &mut RuntimeStepOutput) {
        self.fiber.pending_ops.clear();
        self.unwind_control_stack();
        output.flow_events.push(FlowEvent::Goto {
            target: target.clone(),
        });
        if let Some(flow_index) = self.flow_index(target) {
            self.fiber.cursor = Some(FlowCursor {
                flow_index,
                op_index: 0,
            });
            self.fiber.status = FlowFiberStatus::Running;
        } else {
            self.fiber.cursor = None;
            self.fiber.status =
                FlowFiberStatus::Failed(format!("missing goto target {}", target.0));
        }
    }

    pub(super) fn return_value(&mut self, value: String, output: &mut RuntimeStepOutput) {
        self.fiber.pending_ops.clear();
        self.unwind_control_stack();
        output.flow_events.push(FlowEvent::Return {
            value: value.clone(),
        });
        self.fiber.status = FlowFiberStatus::Done(FlowExit::Return(value));
    }

    fn unwind_control_stack(&mut self) {
        while let Some(entry) = self.fiber.control_stack.pop() {
            if matches!(entry.kind, FlowControlStackEntryKind::Scope) {
                self.fiber.env.pop_scope();
            }
        }
    }

    pub(super) fn finish(&mut self, output: &mut RuntimeStepOutput) {
        output.flow_events.push(FlowEvent::Done);
        self.fiber.status = FlowFiberStatus::Done(FlowExit::Done);
    }
}

enum FlowControl {
    Goto(String),
    Return(String),
    Failed(String),
}

fn control_from_effect(effect: &LineEffectRequest) -> Option<FlowControl> {
    match effect {
        LineEffectRequest::Goto(target) => Some(FlowControl::Goto(target.clone())),
        LineEffectRequest::Return(value) => Some(FlowControl::Return(value.clone())),
        LineEffectRequest::Panic(message)
        | LineEffectRequest::Fail(message)
        | LineEffectRequest::Bail(message) => Some(FlowControl::Failed(message.clone())),
        _ => None,
    }
}
