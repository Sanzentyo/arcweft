//! Borrowed admission of Native dialogue frame roots in the session allowance.

use super::{DialogueActivationFrame, DialogueActivationScope, PendingLineOperation};
use crate::value::{
    AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission, RuntimeStoredValue,
    RuntimeValueView,
};

impl<V, O, C, E> DialogueActivationFrame<V, O, C, E>
where
    V: RuntimeStoredValue,
    O: RuntimeStoredValue,
    C: RuntimeStoredValue,
{
    pub(super) fn admit_frame_roots(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
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
            self.result_target.ty().get().get(),
            &self.voice,
            self.activation_pc,
            self.exiting_for_result,
        ))?;
        admission.validate_metadata_collection(self.captures.len())?;
        for local in &self.captures {
            admission.validate_metadata(local)?;
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
            scope.admit_roots(admission)?;
        }
        if let Some(pending) = &self.pending_line_operation {
            pending.admit_roots(admission)?;
        }
        if let Some(pending) = &self.pending_host_call {
            admission.validate_metadata(&(
                &pending.id,
                pending.result.get().get(),
                pending
                    .binding
                    .as_ref()
                    .map(|pattern| pattern.ty().get().get()),
            ))?;
        }
        Ok(())
    }
}

impl<V: RuntimeStoredValue> DialogueActivationScope<V> {
    fn admit_roots(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata(&(self.exit, self.inflight))?;
        admission.validate_metadata_collection(self.deferred.len())?;
        for registration in &self.deferred {
            registration.admit_retained(admission)?;
        }
        Ok(())
    }
}

impl<V: RuntimeStoredValue> PendingLineOperation<V> {
    fn admit_roots(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        match self {
            Self::AcquireActor {
                command,
                binding,
                value,
                token,
            }
            | Self::ActorLook {
                command,
                binding,
                value,
                token,
            } => {
                admission.validate_metadata(&(
                    command,
                    binding.as_ref().map(|pattern| pattern.ty().get().get()),
                    token,
                ))?;
                admission.validate_borrowed(value.borrowed_value())
            }
            Self::StartVoice {
                command,
                binding,
                site,
            } => admission.validate_metadata(&(
                command,
                binding.as_ref().map(|pattern| pattern.ty().get().get()),
                site.index(),
            )),
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
