//! Match candidates retain their value in the existing typed environment.
//! Guards and arms borrow the defining original Match operation.

use super::{
    Engine, FlowControlStackEntryKind, FlowOp, RuntimeCallBackend, RuntimeEvalError,
    RuntimeStepOutput, RuntimeValue,
};
use crate::engine::NativePendingWork;
use crate::engine::code_cursor::NativeSourceOperation;
use crate::engine::work::{NativeRuntimeContinuation, NativeSourceRangeRole};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeMatchGuardContinuation {
    source: NativeSourceOperation,
    next_arm: usize,
}

impl NativeMatchGuardContinuation {
    pub(in crate::engine) fn admit_retained(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        self.source.admit_retained(admission)?;
        admission.validate_metadata(&self.next_arm)?;
        let Some(crate::task::RuntimeProgramOwner::Plan(plan)) = admission.program_owner() else {
            unreachable!("an admitted original source operation has its Plan owner")
        };
        let valid = match self.source.operation(plan) {
            Some(FlowOp::Match { arms, .. }) => self
                .next_arm
                .checked_sub(1)
                .and_then(|index| arms.get(index))
                .is_some_and(|arm| arm.guard.is_some()),
            _ => false,
        };
        if !valid {
            return Err(crate::value::AwbcRuntimeValueSnapshotError::Message {
                message: "native Match guard differs from its original arm frontier".into(),
            });
        }
        Ok(())
    }
}

impl Engine {
    pub(super) fn select_match_candidate(
        &mut self,
        value: RuntimeValue,
        source: NativeSourceOperation,
    ) -> Result<(), RuntimeEvalError> {
        self.resume_match_candidate(value, source, 0)
    }

    fn resume_match_candidate(
        &mut self,
        value: RuntimeValue,
        source: NativeSourceOperation,
        start: usize,
    ) -> Result<(), RuntimeEvalError> {
        let plan = Arc::clone(&self.plan);
        let Some(FlowOp::Match { arms, .. }) = source.operation(&plan) else {
            return Err(RuntimeEvalError::NativeWorkAdmission {
                source: Box::new(crate::value::AwbcRuntimeValueSnapshotError::Message {
                    message: "native Match continuation lost its original declaration".into(),
                }),
            });
        };
        for (index, arm) in arms.iter().enumerate().skip(start) {
            if !crate::pattern::inspect_runtime_pattern_owned(
                &plan,
                &arm.pattern,
                &value,
                self.fiber.frame.env.function_instantiation(),
            )? {
                continue;
            }
            if let Some(guard) = &arm.guard {
                let bindings = crate::pattern::project_runtime_pattern_guard_bindings(
                    &plan,
                    &arm.pattern,
                    &value,
                    &guard.copy_locals,
                    self.fiber.frame.env.function_instantiation(),
                )?
                .expect("inspected guarded pattern remains matched");
                let body = self.original_body_range(
                    &source,
                    crate::plan::RuntimeFlowBodyRole::MatchGuard { arm: index },
                )?;
                let (token, next) =
                    self.prepare_native_scope(crate::scope::RuntimeScopeFrameKind::Control)?;
                let work = [
                    Some(NativePendingWork::source(
                        body,
                        NativeSourceRangeRole::NestedBody,
                    )),
                    Some(NativePendingWork::resume(
                        NativeRuntimeContinuation::ExitScheduledScope { token },
                    )),
                ];
                self.prepare_native_work(&work)?;
                self.next_scheduled_scope_sequence = next;
                self.push_scope_frame_with_origin(
                    crate::scope::RuntimeScopeIdentity::Anonymous,
                    crate::scope::RuntimeScopeFrameOrigin::Scheduled(token),
                );
                self.fiber.frame.env.set(guard.candidate, value);
                self.fiber.frame.env.bind_all(bindings);
                let Some(entry) = self.fiber.frame.control_stack.last_mut() else {
                    unreachable!("guard scope was just pushed")
                };
                let FlowControlStackEntryKind::Scope { match_guard, .. } = &mut entry.kind else {
                    unreachable!("guard scope owns its continuation")
                };
                *match_guard = Some(NativeMatchGuardContinuation {
                    source,
                    next_arm: index + 1,
                });
                self.prepend_admitted_native_work(work);
                return Ok(());
            }
            let bindings = crate::pattern::match_runtime_pattern_owned(
                &plan,
                &arm.pattern,
                value,
                self.fiber.frame.env.function_instantiation(),
            )?
            .expect("inspected owned pattern remains matched");
            return self.push_scoped_source_body(
                &source,
                crate::plan::RuntimeFlowBodyRole::MatchArm { arm: index },
                bindings,
            );
        }
        Err(RuntimeEvalError::PatternMismatch(
            "exhaustive Match has no selected arm".into(),
        ))
    }

    pub(super) fn complete_match_guard(
        &mut self,
        output: &mut RuntimeStepOutput,
        backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        let continuation =
            self.fiber
                .frame
                .control_stack
                .last_mut()
                .and_then(|entry| match &mut entry.kind {
                    FlowControlStackEntryKind::Scope { match_guard, .. } => match_guard.take(),
                    _ => None,
                });
        let Some(continuation) = continuation else {
            return false;
        };
        let plan = Arc::clone(&self.plan);
        let guard = match continuation.source.operation(&plan) {
            Some(FlowOp::Match { arms, .. }) => continuation
                .next_arm
                .checked_sub(1)
                .and_then(|index| arms.get(index))
                .and_then(|arm| arm.guard.as_ref()),
            _ => None,
        };
        let Some(guard) = guard else {
            self.fail_eval("native Match guard lost its original arm", output);
            return true;
        };
        let Some(condition) = &guard.condition else {
            self.fail_format_aware_eval(
                RuntimeEvalError::PatternMismatch("non-returning guard fell through".into()),
                output,
                backend,
            );
            return true;
        };
        let selected = match self.evaluate_bool_with_backend(condition, backend) {
            Ok(selected) => selected,
            Err(error) => {
                self.fail_format_aware_eval(error, output, backend);
                return true;
            }
        };
        let value = self
            .fiber
            .frame
            .env
            .take(guard.candidate)
            .ok_or(RuntimeEvalError::UninitializedLocal(guard.candidate));
        self.pop_scope_frame(output, backend);
        let result = value.and_then(|value| {
            if selected {
                let Some(FlowOp::Match { arms, .. }) = continuation.source.operation(&plan) else {
                    unreachable!("the immutable original Match was resolved before evaluation")
                };
                let arm = &arms[continuation.next_arm - 1];
                let bindings = crate::pattern::match_runtime_pattern_owned(
                    &plan,
                    &arm.pattern,
                    value,
                    self.fiber.frame.env.function_instantiation(),
                )?
                .expect("inspected owned pattern remains matched");
                self.push_scoped_source_body(
                    &continuation.source,
                    crate::plan::RuntimeFlowBodyRole::MatchArm {
                        arm: continuation.next_arm - 1,
                    },
                    bindings,
                )
            } else {
                self.resume_match_candidate(value, continuation.source, continuation.next_arm)
            }
        });
        if let Err(error) = result {
            self.fail_format_aware_eval(error, output, backend);
        }
        true
    }
}
