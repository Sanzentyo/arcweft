//! Borrowed admission of Native dialogue frame roots in the session allowance.

use super::{DialogueActivationFrame, DialogueOrigin, PendingLineOperation};
use crate::value::{
    AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission, RuntimeStoredValue,
    RuntimeValueView,
};

impl<V, O, C, E, R> DialogueActivationFrame<V, O, C, E, R>
where
    V: RuntimeStoredValue,
    O: RuntimeStoredValue,
    C: RuntimeStoredValue,
{
    pub(super) fn admit_frame_roots(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
        admit_execution: impl FnOnce(
            &E,
            &mut RuntimeRetainedValueAdmission,
        ) -> Result<(), AwbcRuntimeValueSnapshotError>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata(&(
            self.parent,
            &self.line,
            self.content,
            self.task_group.index(),
            self.elapsed,
            &self.voice,
            self.activation_pc,
            self.exiting_for_result,
        ))?;
        if let Some(message) = &self.failure {
            admission.validate_metadata_string(message)?;
            if self.phase != crate::engine::DialogueRuntimePhase::Closing {
                return Err(invalid_shape(
                    "dialogue failure has no closing phase",
                    admission,
                ));
            }
        }
        match &self.origin {
            DialogueOrigin::Operation(source) => {
                source.admit_retained(admission)?;
                let Some(crate::task::RuntimeProgramOwner::Plan(plan)) = admission.program_owner()
                else {
                    return Err(invalid_shape(
                        "dialogue origin requires its original Plan",
                        admission,
                    ));
                };
                let content = plan.dialogue_content().get(self.content);
                let valid = self.result_target(plan).is_ok_and(|result| {
                    plan.line_task_groups()
                        .get(self.task_group.index())
                        .is_some_and(|group| group.result_type() == result.ty())
                }) && content.is_some_and(|content| {
                    content.line() == &self.line
                        && content.line_task_group() == Some(self.task_group)
                });
                if !valid {
                    return Err(invalid_shape(
                        "dialogue origin differs from its line or result contract",
                        admission,
                    ));
                }
            }
            #[cfg(test)]
            DialogueOrigin::Fixture(result) => {
                admission.validate_metadata(&result.ty().get().get())?
            }
        }
        if let Some(target) = &self.target {
            if !matches!(target.borrowed_view(), RuntimeValueView::Opaque(_)) {
                return Err(invalid_shape(
                    "dialogue rollback target is not opaque",
                    admission,
                ));
            }
            admission.validate_borrowed(target.borrowed_value())?;
        }
        admission.validate_metadata_collection(self.task_inputs.len())?;
        for input in &self.task_inputs {
            admission.validate_metadata(&input.local)?;
            admission.validate_borrowed(input.value.borrowed_value())?;
        }
        admit_execution(&self.execution, admission)?;
        admission.validate_metadata_collection(self.values.len())?;
        for binding in &self.values {
            admission.validate_metadata(&(binding.slot, binding.role))?;
            admission.validate_borrowed(binding.value.borrowed_value())?;
        }
        admission.validate_metadata_collection(self.effect_callbacks.len())?;
        for binding in &self.effect_callbacks {
            admission.validate_metadata(&binding.site())?;
            let callback = binding.callback();
            if !matches!(callback.borrowed_view(), RuntimeValueView::Callable(_)) {
                return Err(invalid_shape(
                    "dialogue rollback callback is not callable",
                    admission,
                ));
            }
            admission.validate_borrowed(callback.borrowed_value())?;
        }
        admission.validate_metadata_collection(self.scopes.len())?;
        for scope in &self.scopes {
            scope.admit_roots(
                admission,
                crate::engine::defer::NativeDeferScopeOwner::Activation,
            )?;
        }
        if let Some(pending) = &self.pending_scope_result {
            if !matches!(
                self.phase,
                crate::engine::DialogueRuntimePhase::Activating
                    | crate::engine::DialogueRuntimePhase::Closing
            ) || self.exiting_for_result
                || self.pending_line_operation.is_some()
                || self.pending_host_call.is_some()
            {
                return Err(invalid_shape(
                    "dialogue scope result has no active scope exit",
                    admission,
                ));
            }
            pending
                .admit(
                    self.task_group,
                    self.activation_pc,
                    self.scopes.len(),
                    binding,
                    admission,
                )
                .inspect_err(|error| admission.retain_failure(error.clone()))?;
        }
        if let Some(pending) = &self.pending_line_operation {
            let Some(crate::task::RuntimeProgramOwner::Plan(plan)) =
                admission.program_owner().cloned()
            else {
                return Err(invalid_shape(
                    "dialogue command has no original Plan",
                    admission,
                ));
            };
            let (pattern, _) = pending
                .declaration(self.activation_operation(&plan))
                .ok_or_else(|| {
                    invalid_shape(
                        "dialogue command has no original activation operation",
                        admission,
                    )
                })?;
            let expected = pattern
                .map(|pattern| {
                    plan.type_table()
                        .get(pattern.ty())
                        .map(|ty| ty.semantic_identity())
                        .ok_or_else(|| {
                            invalid_shape("dialogue command has no original result type", admission)
                        })
                })
                .transpose()?;
            pending.admit_roots(expected, binding, admission)?;
        }
        if let Some(pending) = &self.pending_host_call {
            let Some(crate::task::RuntimeProgramOwner::Plan(plan)) = admission.program_owner()
            else {
                return Err(invalid_shape(
                    "dialogue host call has no original Plan",
                    admission,
                ));
            };
            if self.pending_line_operation.is_some()
                || self.exiting_for_result
                || !matches!(
                    self.phase,
                    crate::engine::DialogueRuntimePhase::Activating
                        | crate::engine::DialogueRuntimePhase::Closing
                )
                || self.activation_source(plan).as_ref() != Some(&pending.operation)
            {
                return Err(invalid_shape(
                    "dialogue host call differs from its activation operation",
                    admission,
                ));
            }
            pending.admit_retained(admission)?;
        }
        Ok(())
    }
}

impl<V, O, C, E, R> DialogueActivationFrame<V, O, C, E, R> {
    pub(super) fn admit_pending_line_command<L>(
        &self,
        activation: &crate::runtime_id::DialogueActivationId,
        line: &crate::line_task::RuntimeDialogueActivationState<
            crate::runtime_id::RuntimePlanTypeId,
            L,
        >,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        let Some(pending) = &self.pending_line_operation else {
            return Ok(());
        };
        let command = pending.command();
        let matches =
            if let Some(crate::task::RuntimeProgramOwner::Plan(plan)) = admission.program_owner() {
                pending
                    .declaration(self.activation_operation(plan))
                    .is_some_and(|(_, operation)| {
                        command.activation() == activation
                            && line
                                .issued_command(command)
                                .is_some_and(|issued| pending.matches_issued(operation, issued))
                    })
            } else {
                false
            };
        if matches {
            Ok(())
        } else {
            Err(invalid_shape(
                "dialogue pending command differs from its issued activation command",
                admission,
            ))
        }
    }
}

impl<V: RuntimeStoredValue> PendingLineOperation<V> {
    fn admit_roots(
        &self,
        expected: Option<crate::pattern::RuntimeSemanticTypeId>,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata(self.command())?;
        match self {
            Self::AcquireActor { value, token, .. } | Self::ActorLook { value, token, .. } => {
                admission.validate_metadata(token)?;
                match expected {
                    Some(ty) => admission.validate_owned_typed_borrowed_in_invocation(
                        value.borrowed_value(),
                        ty,
                        binding,
                    )?,
                    None => admission.validate_borrowed(value.borrowed_value())?,
                }
                let matches = matches!(value.borrowed_view(), RuntimeValueView::Opaque(_))
                    && value.borrowed_view().line_handle_projection().is_ok_and(
                        |handles| matches!(handles.as_slice(), [handle] if handle.token() == token),
                    );
                if !matches {
                    return Err(invalid_shape(
                        "dialogue command value differs from its retained handle",
                        admission,
                    ));
                }
                Ok(())
            }
            Self::StartVoice { .. } => Ok(()),
        }
    }
}

fn invalid_shape(
    message: &str,
    admission: &mut RuntimeRetainedValueAdmission,
) -> AwbcRuntimeValueSnapshotError {
    let error = AwbcRuntimeValueSnapshotError::Message {
        message: message.to_owned(),
    };
    admission.retain_failure(error.clone());
    error
}
