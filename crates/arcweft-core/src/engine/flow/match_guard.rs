//! Match candidates retain their value in the existing typed environment.
//! Guard calls use the same function frames, budgets and unwind as any flow op.

use super::{
    Engine, FlowControlStackEntryKind, FlowOp, RuntimeCallBackend, RuntimeEvalError,
    RuntimeScopeIdentity, RuntimeStepOutput, RuntimeValue,
};
use crate::plan::RuntimeMatchArm;
use crate::runtime_id::RuntimeLocalDeclarationId;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeMatchGuardContinuation {
    candidate: RuntimeLocalDeclarationId,
    arms: Arc<[RuntimeMatchArm]>,
    next_arm: usize,
}

impl Engine {
    pub(super) fn select_match_candidate(
        &mut self,
        value: RuntimeValue,
        arms: Vec<RuntimeMatchArm>,
    ) -> Result<(), RuntimeEvalError> {
        self.resume_match_candidate(value, Arc::from(arms), 0)
    }

    fn resume_match_candidate(
        &mut self,
        value: RuntimeValue,
        arms: Arc<[RuntimeMatchArm]>,
        start: usize,
    ) -> Result<(), RuntimeEvalError> {
        for (index, arm) in arms.iter().enumerate().skip(start) {
            if !crate::pattern::inspect_runtime_pattern_owned(
                &self.plan,
                &arm.pattern,
                &value,
                self.fiber.env.function_instantiation(),
            )? {
                continue;
            }
            if let Some(guard) = &arm.guard {
                let bindings = crate::pattern::project_runtime_pattern_guard_bindings(
                    &self.plan,
                    &arm.pattern,
                    &value,
                    &guard.copy_locals,
                    self.fiber.env.function_instantiation(),
                )?
                .expect("inspected guarded pattern remains matched");
                self.push_scope_frame(RuntimeScopeIdentity::Anonymous);
                self.fiber.env.set(guard.candidate, value);
                self.fiber.env.bind_all(bindings);
                let Some(entry) = self.fiber.control_stack.last_mut() else {
                    unreachable!("guard scope was just pushed")
                };
                let FlowControlStackEntryKind::Scope { match_guard, .. } = &mut entry.kind else {
                    unreachable!("guard scope owns its continuation")
                };
                *match_guard = Some(NativeMatchGuardContinuation {
                    candidate: guard.candidate,
                    arms: Arc::clone(&arms),
                    next_arm: index + 1,
                });
                self.fiber.pending_ops.push_front(FlowOp::ExitScope);
                for op in guard.ops.iter().rev() {
                    self.fiber.pending_ops.push_front(op.clone());
                }
                return Ok(());
            }
            let bindings = crate::pattern::match_runtime_pattern_owned(
                &self.plan,
                &arm.pattern,
                value,
                self.fiber.env.function_instantiation(),
            )?
            .expect("inspected owned pattern remains matched");
            self.push_scoped_ops_with_bindings(bindings, arm.ops.clone());
            return Ok(());
        }
        Err(RuntimeEvalError::PatternMismatch(
            "exhaustive Match has no selected arm".to_owned(),
        ))
    }

    /// Only normal scope completion evaluates a condition; Return/Break/error
    /// unwinds this scope through the ordinary cleanup path without selecting.
    pub(super) fn complete_match_guard(
        &mut self,
        output: &mut RuntimeStepOutput,
        backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        let continuation =
            self.fiber
                .control_stack
                .last_mut()
                .and_then(|entry| match &mut entry.kind {
                    FlowControlStackEntryKind::Scope { match_guard, .. } => match_guard.take(),
                    _ => None,
                });
        let Some(continuation) = continuation else {
            return false;
        };
        let arm = &continuation.arms[continuation.next_arm - 1];
        let guard = arm.guard.as_ref().expect("continuation has a guarded arm");
        let Some(condition) = &guard.condition else {
            self.fail_format_aware_eval(
                RuntimeEvalError::PatternMismatch("non-returning guard fell through".to_owned()),
                output,
                backend,
            );
            return true;
        };
        let selected = match self.evaluate_bool_with_backend(condition, backend) {
            Ok(selected) => selected,
            Err(error) => {
                // Failure leaves the candidate under the environment's custody.
                self.fail_format_aware_eval(error, output, backend);
                return true;
            }
        };
        let value = self
            .fiber
            .env
            .take(continuation.candidate)
            .ok_or(RuntimeEvalError::UninitializedLocal(continuation.candidate));
        self.pop_scope_frame(output, backend);
        let result = value.and_then(|value| {
            if selected {
                let bindings = crate::pattern::match_runtime_pattern_owned(
                    &self.plan,
                    &arm.pattern,
                    value,
                    self.fiber.env.function_instantiation(),
                )?
                .expect("inspected owned pattern remains matched");
                self.push_scoped_ops_with_bindings(bindings, arm.ops.clone());
                Ok(())
            } else {
                self.resume_match_candidate(
                    value,
                    Arc::clone(&continuation.arms),
                    continuation.next_arm,
                )
            }
        });
        if let Err(error) = result {
            self.fail_format_aware_eval(error, output, backend);
        }
        true
    }
}
