//! A complete borrowed registry view, including its actual withdrawn owners.

use super::*;
use crate::value::{AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission};

/// This view does not copy a frame or issue a new activation. Every withdrawn
/// entry must be paired with the unique transaction from that reservation.
pub(crate) struct RuntimeDialogueRetainedContour<'a, F, T> {
    registry: &'a RuntimeDialogueActivationRegistry<F, T>,
    borrowed: BTreeMap<DialogueActivationId, &'a RuntimeDialogueActivationTransaction<F, T>>,
}

impl<F, T> RuntimeDialogueActivationRegistry<F, T> {
    pub(crate) fn retained_contour<'a>(
        &'a self,
        transactions: impl ExactSizeIterator<Item = &'a RuntimeDialogueActivationTransaction<F, T>>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<RuntimeDialogueRetainedContour<'a, F, T>, RuntimeDialogueRegistrySnapshotError>
    {
        let result = (|| {
            admission.validate_metadata_collection(self.entries.len())?;
            admission.validate_metadata_collection(transactions.len())?;
            let mut borrowed = BTreeMap::new();
            for transaction in transactions {
                admission.validate_metadata(&(transaction.activation, transaction.revision))?;
                if !matches!(self.entries.get(&transaction.activation),
                    Some(RuntimeDialogueRegistryEntry::InFlight { revision, reservation })
                        if *revision == transaction.revision && reservation == &transaction.reservation)
                    || borrowed
                        .insert(transaction.activation, transaction)
                        .is_some()
                {
                    return Err(LineRuntimeError::StaleActivationTransaction.into());
                }
            }
            for (activation, entry) in &self.entries {
                let revision = match entry {
                    RuntimeDialogueRegistryEntry::Active { revision, .. }
                    | RuntimeDialogueRegistryEntry::PublishedHandles { revision, .. } => revision,
                    RuntimeDialogueRegistryEntry::InFlight { revision, .. } => {
                        if !borrowed.contains_key(activation) {
                            return Err(RuntimeDialogueRegistrySnapshotError::Frame {
                                message: "dialogue transaction is in flight".to_owned(),
                            });
                        }
                        revision
                    }
                };
                admission.validate_metadata(&(activation, revision))?;
            }
            Ok(RuntimeDialogueRetainedContour {
                registry: self,
                borrowed,
            })
        })();
        result.inspect_err(|error: &RuntimeDialogueRegistrySnapshotError| {
            admission.retain_failure(AwbcRuntimeValueSnapshotError::Message {
                message: error.to_string(),
            });
        })
    }
}

impl<'a, F, T> RuntimeDialogueRetainedContour<'a, F, T> {
    pub(crate) fn try_visit_active<E>(
        &self,
        mut visit: impl FnMut(
            &'a DialogueActivationId,
            &'a F,
            &'a RuntimeDialogueActivationState<T>,
        ) -> Result<(), E>,
    ) -> Result<(), E> {
        for (activation, entry) in &self.registry.entries {
            match entry {
                RuntimeDialogueRegistryEntry::Active { frame, line, .. } => {
                    visit(activation, frame, line)?
                }
                RuntimeDialogueRegistryEntry::InFlight { .. } => {
                    let transaction = self.borrowed[activation];
                    visit(activation, &transaction.frame, &transaction.line)?;
                }
                RuntimeDialogueRegistryEntry::PublishedHandles { .. } => {}
            }
        }
        Ok(())
    }

    pub(crate) fn admit_retained(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), RuntimeDialogueRegistrySnapshotError>
    where
        T: super::super::handle::RuntimeDialogueTypeCoordinate,
    {
        for (activation, entry) in &self.registry.entries {
            match entry {
                RuntimeDialogueRegistryEntry::Active { line, .. } => {
                    line.admit_retained_resources(admission)?
                }
                RuntimeDialogueRegistryEntry::InFlight { .. } => self.borrowed[activation]
                    .line
                    .admit_retained_resources(admission)?,
                RuntimeDialogueRegistryEntry::PublishedHandles { handles, .. } => {
                    admission.validate_metadata(handles)?;
                    handles.restore_admit(activation)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
