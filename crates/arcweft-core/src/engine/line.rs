use super::defer::{
    NativeDeferBoundary, NativeGotoOrigin, NativeLexicalExit, NativeLexicalExitAction,
};
use super::{
    Engine, FlowControlStackEntryKind, FlowEvent, FlowExit, FlowFiberStatus, LineEffectRequest,
    RuntimeStepInput, RuntimeStepOutput,
};
use crate::plan::{FlowOp, FlowRuntimeId};
use crate::pure::RuntimeCallBackend;

impl Engine {
    pub(super) fn step_line_only(
        &mut self,
        input: &RuntimeStepInput,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let _ = input;
        self.finish(output, pure_backend);
    }

    pub(super) fn apply_control_effects(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        let Some(control) = output.effects.line.iter().find_map(control_from_effect) else {
            return false;
        };
        match control {
            FlowControl::Goto(target) => self.goto_from_label(&target, None, output, pure_backend),
            FlowControl::Return(value) => self.return_value(value, output, pure_backend),
            FlowControl::Failed(message) => {
                self.fiber.frame.status = FlowFiberStatus::Failed(message)
            }
        }
        true
    }

    pub(super) fn goto_from_label(
        &mut self,
        label: &str,
        source: Option<NativeGotoOrigin>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        match self.plan.resolve_flow_target_value(label) {
            Ok(target) => self.goto(&target, source, output, pure_backend),
            Err(error) => {
                self.fiber.frame.status =
                    FlowFiberStatus::Failed(format!("invalid goto target {label}: {error}"));
            }
        }
    }

    pub(super) fn goto(
        &mut self,
        target: &FlowRuntimeId,
        mut source: Option<NativeGotoOrigin>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        if self.flow_index(target).is_none() {
            self.fiber.frame.status =
                FlowFiberStatus::Failed(format!("missing goto target {target}"));
            return;
        }
        let plan = std::sync::Arc::clone(&self.plan);
        let declaration = match plan.validate_flow_invocation(target, &[]) {
            Ok(declaration) => declaration,
            Err(error) => {
                self.fiber.frame.status = FlowFiberStatus::Failed(error.to_string());
                return;
            }
        };
        let pending_ops = match self.prepare_function_root_work(declaration.site()) {
            Ok(work) => work,
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        if source.is_some()
            && self.fiber.owner.owns_flow_defers()
            && let Some(scope) = self.fiber.frame.defer_target(NativeDeferBoundary::Flow)
        {
            self.fiber.frame.lexical_exit = Some(NativeLexicalExit {
                target: scope,
                action: NativeLexicalExitAction::Goto {
                    source: source.take().expect("the authored goto retains its source"),
                    destination: target.clone(),
                },
            });
            return;
        }
        self.fiber.frame.await_observer = None;
        if let Err(error) = self.unwind_control_stack(output, pure_backend) {
            self.fail_eval(error, output);
            return;
        }
        if let Some(FlowOp::Effect(effect @ LineEffectRequest::Goto(_))) =
            source.as_ref().and_then(|source| match source {
                NativeGotoOrigin::Operation(source) => source.operation(&plan),
                NativeGotoOrigin::Choice { .. } => None,
            })
        {
            self.emit_line_effect(effect.clone(), output, pure_backend);
        }
        output.flow_events.push(FlowEvent::Goto {
            target: target.clone(),
        });
        self.fiber.frame.bind_root_function(
            declaration.site(),
            declaration.type_instantiation.clone(),
            ([], crate::value::RuntimeInputOrder::canonical(0)),
        );
        self.fiber.frame.pending_ops = pending_ops;
        self.fiber.frame.status = FlowFiberStatus::Running;
    }

    pub(super) fn return_value(
        &mut self,
        value: String,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let Some(value) = self.return_typed_value(
            crate::value::RuntimeValue::String(value),
            output,
            pure_backend,
        ) else {
            return;
        };
        let crate::value::RuntimeValue::String(value) = value else {
            unreachable!("the unhandled flow return retains its original string")
        };
        self.fiber.frame.pending_ops.clear();
        self.fiber.frame.await_observer = None;
        if let Err(error) = self.unwind_control_stack(output, pure_backend) {
            self.fail_eval(error, output);
            return;
        }
        output.flow_events.push(FlowEvent::Return {
            value: value.clone(),
        });
        self.fiber.frame.status = FlowFiberStatus::Done(FlowExit::Return(value));
    }

    pub(super) fn unwind_control_stack(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), crate::value::RuntimeEvalError> {
        while !self.fiber.frame.control_stack.is_empty() {
            self.unwind_control_entry(output, pure_backend)?;
        }
        self.drain_root_cleanups(output, pure_backend)?;
        self.fiber.frame.pending_ops.clear();
        self.fiber.frame.env.close_root_flow_registry();
        Ok(())
    }

    pub(super) fn unwind_control_entry(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), crate::value::RuntimeEvalError> {
        let Some(entry) = self.fiber.frame.control_stack.last() else {
            return Ok(());
        };
        let owns_scope = match &entry.kind {
            FlowControlStackEntryKind::Scope { defers, .. } => {
                if defers.has_work() {
                    return Err(crate::scope::RuntimeScopeExitError::DeferredWorkPending.into());
                }
                true
            }
            FlowControlStackEntryKind::FunctionCall(frame) => frame.function_scope,
            FlowControlStackEntryKind::FormatAttempt(frame) => frame.active.is_some(),
            _ => false,
        };
        if owns_scope {
            self.pop_runtime_scope()?;
        }
        let entry = self
            .fiber
            .frame
            .control_stack
            .pop()
            .expect("inspected control owner");
        match entry.kind {
            FlowControlStackEntryKind::Scope {
                origin, cleanups, ..
            } => {
                self.cancel_scheduled_scope_close(origin);
                self.emit_scope_cleanups(cleanups, output, pure_backend);
            }
            FlowControlStackEntryKind::FunctionCall(frame) => {
                self.fiber.frame.pending_ops = frame.caller_pending_ops;
            }
            FlowControlStackEntryKind::FormatAttempt(frame) => {
                if let Some(active) = frame.active {
                    self.fiber.frame.pending_ops = active.caller_pending_ops;
                }
            }
            _ => {}
        }
        Ok(())
    }
    pub(super) fn finish(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        if self.defer_function_fallthrough(output) {
            return;
        }
        if matches!(&self.fiber.launch, super::FlowFiberLaunch::Program(_))
            && self.fiber.frame.invocation_result.is_none()
        {
            self.complete_program_task_return(
                crate::value::RuntimeValue::Unit,
                false,
                output,
                pure_backend,
            );
            return;
        }
        self.fiber.frame.pending_ops.clear();
        if let Err(error) = self.unwind_control_stack(output, pure_backend) {
            self.fail_eval(error, output);
            return;
        }
        output.flow_events.push(FlowEvent::Done);
        self.fiber.frame.status = FlowFiberStatus::Done(FlowExit::Done);
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
