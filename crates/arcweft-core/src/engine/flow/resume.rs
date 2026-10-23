//! Runtime continuation dispatch borrows every authored header from its source.

use super::{Engine, FlowFiberStatus, FlowOp, RuntimeCallBackend, RuntimeStepOutput};
use crate::engine::work::NativeRuntimeContinuation;
use crate::scope::RuntimeScopeFrameOrigin;
use std::sync::Arc;

impl Engine {
    pub(super) fn execute_native_resume(
        &mut self,
        resume: NativeRuntimeContinuation<
            crate::value::RuntimeValue,
            crate::value::RuntimeIterator,
        >,
        output: &mut RuntimeStepOutput,
        backend: &mut impl RuntimeCallBackend,
    ) {
        use NativeRuntimeContinuation as Resume;
        let plan = Arc::clone(&self.plan);
        match resume {
            Resume::Bind(bindings) => self.fiber.frame.env.bind_all(bindings),
            Resume::EnterTaskScope => {
                self.push_scope_frame(crate::scope::RuntimeScopeIdentity::Anonymous);
            }
            Resume::ExitTaskScope => {
                if let Err(error) = self.exit_emitted_scope(output, backend) {
                    self.fail_eval(error, output);
                }
            }
            Resume::LoopNext { source } => {
                if !matches!(source.operation(&plan), Some(FlowOp::Loop { .. })) {
                    self.fail_eval("native loop continuation lost its original header", output);
                    return;
                }
                if let Err(error) = self.push_loop_iteration(&source) {
                    self.fail_format_aware_eval(error, output, backend);
                }
            }
            Resume::WhileNext { source } => {
                let Some(FlowOp::While { condition, .. }) = source.operation(&plan) else {
                    self.fail_eval("native while continuation lost its original header", output);
                    return;
                };
                match self.evaluate_bool_with_backend(condition, backend) {
                    Ok(true) => {
                        if let Err(error) = self.push_while_iteration(&source) {
                            self.fail_format_aware_eval(error, output, backend);
                        }
                    }
                    Ok(false) => {
                        self.pop_loop_frame(output, backend);
                    }
                    Err(error) => self.fail_format_aware_eval(error, output, backend),
                }
            }
            Resume::WhileLetNext { source } => {
                let Some(FlowOp::WhileLet {
                    pattern,
                    expr,
                    guard,
                    ..
                }) = source.operation(&plan)
                else {
                    self.fail_eval(
                        "native while-let continuation lost its original header",
                        output,
                    );
                    return;
                };
                match self.evaluate_if_let_with_backend(pattern, expr, guard.as_ref(), backend) {
                    Ok(Some(bindings)) => {
                        if let Err(error) = self.push_while_let_iteration(&source, bindings) {
                            self.fail_format_aware_eval(error, output, backend);
                        }
                    }
                    Ok(None) => {
                        self.pop_loop_frame(output, backend);
                    }
                    Err(error) => self.fail_format_aware_eval(error, output, backend),
                }
            }
            Resume::ForNext { source, iterator } => {
                self.push_for_next(&source, iterator, output, backend);
            }
            Resume::CompleteFormatOperand { source } => {
                let Some(FlowOp::FormatOperandAttempt {
                    attempt,
                    parameter,
                    value,
                    ..
                }) = source.operation(&plan)
                else {
                    self.fail_eval(
                        "native formatter completion lost its original operand",
                        output,
                    );
                    return;
                };
                if let Err(error) =
                    self.complete_format_operand_attempt(*attempt, *parameter, value, backend)
                {
                    self.fail_format_aware_eval(error, output, backend);
                }
            }
            Resume::EnterScheduledScope { identity, token } => {
                if !token.belongs_to(self.fiber.execution, self.fiber.persistent_id)
                    || token.ordinal().get() >= self.next_scheduled_scope_sequence
                    || (token.kind() == crate::scope::RuntimeScopeFrameKind::Control
                        && !matches!(&identity, crate::scope::RuntimeScopeIdentity::Anonymous))
                {
                    self.fail_eval(
                        crate::scope::RuntimeScopeExitError::ScheduledTargetMismatch,
                        output,
                    );
                    return;
                }
                self.push_scope_frame_with_origin(
                    identity,
                    RuntimeScopeFrameOrigin::Scheduled(token),
                );
            }
            Resume::ExitScheduledScope { token } => {
                if crate::scope::RuntimeScopeExitTarget::Frame(Some(token))
                    .resolve(self.active_scope_targets())
                    .is_err()
                {
                    self.fail_eval(
                        crate::scope::RuntimeScopeExitError::ScheduledTargetMismatch,
                        output,
                    );
                    return;
                }
                if !self.complete_match_guard(output, backend) {
                    self.pop_scope_frame(output, backend);
                }
            }
            Resume::CompleteAwaitObserver => {
                let Some(state) = self.fiber.frame.await_observer.take() else {
                    self.fail_eval(
                        "Await observer completed without an active Await context",
                        output,
                    );
                    return;
                };
                self.fiber.frame.status = FlowFiberStatus::NeedWaiting(state);
            }
        }
    }
}
