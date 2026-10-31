//! Read-only admission of actual execution coordinates. The temporary index
//! relates physical owners; it cannot issue a bank, scope, activation or value.

use super::*;
use crate::runtime_id::{
    DialogueActivationId, RuntimeDialogueContentPlanId, RuntimeFiberInstanceId,
};
use crate::value::{AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission};

#[derive(Clone, Copy)]
pub(crate) enum PresentationExecutionRole {
    Root,
    Dialogue(DialogueActivationId),
    ProgramChild {
        parent: RuntimeFiberInstanceId,
    },
    /// A detached line child may retain its original Dialogue bank after the
    /// activation frame has closed.
    Independent,
}

pub(crate) struct PresentationExecutionAdmission<'a> {
    last_fiber: NonZeroU64,
    occurrences: BTreeMap<(RuntimePersistentFiberId, RuntimeDialogueContentPlanId), &'a u64>,
    fibers:
        BTreeMap<RuntimeFiberInstanceId, (RuntimePresentationBankId, PresentationExecutionRole)>,
}

impl<'a> PresentationExecutionAdmission<'a> {
    pub(crate) fn new(
        last_fiber: Option<NonZeroU64>,
        occurrences: impl ExactSizeIterator<
            Item = (
                RuntimePersistentFiberId,
                RuntimeDialogueContentPlanId,
                &'a u64,
            ),
        >,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        let result = (|| {
            let last_fiber = last_fiber.ok_or(RuntimePresentationError::InvalidState)?;
            admission.validate_metadata_collection(occurrences.len())?;
            let mut rows = BTreeMap::new();
            for (owner, content, next) in occurrences {
                admission.validate_metadata(&(owner, content, next))?;
                // Closed namespace cursors survive a compatible code change;
                // their content need not exist in the replacement artifact.
                if owner.get() == 0
                    || owner.get() > last_fiber.get()
                    || *next == 0
                    || rows.insert((owner, content), next).is_some()
                {
                    return Err(RuntimePresentationError::InvalidState.into());
                }
            }
            Ok(Self {
                last_fiber,
                occurrences: rows,
                fibers: BTreeMap::new(),
            })
        })();
        result.inspect_err(|error: &AwbcRuntimeValueSnapshotError| {
            admission.retain_failure(error.clone())
        })
    }

    fn validate_bank(
        &self,
        bank: RuntimePresentationBankId,
    ) -> Result<(), RuntimePresentationError> {
        if let RuntimePresentationBankId::Dialogue { activation } = bank {
            // An activation namespace survives a compatible code replacement.
            // Its issuing artifact is checked by the resource bank; rebasing
            // the executing code does not rename the activation or its bank.
            if self
                .occurrences
                .get(&(activation.owner_fiber(), activation.content()))
                .is_none_or(|next| activation.occurrence() >= **next)
            {
                return Err(RuntimePresentationError::Owner);
            }
        }
        Ok(())
    }

    pub(crate) fn admit_dialogue(
        &self,
        activation: DialogueActivationId,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        let result = admission.validate_metadata(&activation).and_then(|()| {
            self.validate_bank(RuntimePresentationBankId::Dialogue { activation })
                .map_err(Into::into)
        });
        result.inspect_err(|error| admission.retain_failure(error.clone()))
    }

    pub(crate) fn register_fiber(
        &mut self,
        instance: RuntimeFiberInstanceId,
        bank: RuntimePresentationBankId,
        role: PresentationExecutionRole,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        let result = (|| {
            admission.validate_metadata(&(instance, bank))?;
            self.validate_bank(bank)?;
            if instance.get() > self.last_fiber || self.fibers.contains_key(&instance) {
                return Err(RuntimePresentationError::InvalidState.into());
            }
            match role {
                PresentationExecutionRole::Root if bank != RuntimePresentationBankId::Root => {
                    return Err(RuntimePresentationError::Owner.into());
                }
                PresentationExecutionRole::Dialogue(activation)
                    if bank != (RuntimePresentationBankId::Dialogue { activation }) =>
                {
                    return Err(RuntimePresentationError::Owner.into());
                }
                _ => {}
            }
            self.fibers.insert(instance, (bank, role));
            Ok(())
        })();
        result.inspect_err(|error: &AwbcRuntimeValueSnapshotError| {
            admission.retain_failure(error.clone())
        })
    }

    pub(crate) fn finish(
        self,
        banks: impl Iterator<Item = RuntimePresentationBankId>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        let result = (|| {
            for (instance, (bank, role)) in &self.fibers {
                if let PresentationExecutionRole::ProgramChild { parent } = role {
                    if parent.get() >= instance.get()
                        || self
                            .fibers
                            .get(parent)
                            .is_none_or(|(parent_bank, _)| parent_bank != bank)
                    {
                        return Err(RuntimePresentationError::Owner.into());
                    }
                }
            }
            for bank in banks {
                self.validate_bank(bank)?;
            }
            Ok(())
        })();
        result.inspect_err(|error: &AwbcRuntimeValueSnapshotError| {
            admission.retain_failure(error.clone())
        })
    }
}

#[cfg(test)]
mod tests;
