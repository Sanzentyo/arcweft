//! Owned, bounded Action messages shared by Native and Product execution.
//! Declaration and payload schemas stay in the selected original program.

use std::collections::VecDeque;

use crate::pattern::RuntimeSemanticTypeId;
use crate::task::RuntimeProgramOwner;
use crate::value::{
    AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimePayload,
    RuntimeRetainedValueAdmission, RuntimeStoredValue,
};

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq)]
struct ActionMessage<P> {
    payload_type: RuntimeSemanticTypeId,
    declaration: crate::entry::RuntimeNominalDeclarationId,
    payload: P,
}

/// The mailbox owns values and original address claims. Declaration schemas
/// remain in the selected program; this is not a second channel catalog.
/// Global insertion order also preserves FIFO order within each channel.
#[derive(Debug, PartialEq)]
pub(crate) struct RuntimeActionMailbox<P = RuntimePayload> {
    messages: VecDeque<ActionMessage<P>>,
}

impl<P> Default for RuntimeActionMailbox<P> {
    fn default() -> Self {
        Self {
            messages: VecDeque::new(),
        }
    }
}

impl Clone for RuntimeActionMailbox<AwbcRuntimeValueSnapshot> {
    fn clone(&self) -> Self {
        Self {
            messages: self
                .messages
                .iter()
                .map(|message| ActionMessage {
                    payload_type: message.payload_type,
                    declaration: message.declaration.clone(),
                    payload: message.payload.clone(),
                })
                .collect(),
        }
    }
}

impl<P: RuntimeStoredValue> RuntimeActionMailbox<P> {
    pub(crate) fn visit_value_roots<E>(
        &self,
        mut visit: impl FnMut(&P) -> Result<(), E>,
    ) -> Result<(), E> {
        for message in &self.messages {
            visit(&message.payload)?;
        }
        Ok(())
    }
    /// One allowance covers all queued roots before any snapshot copying,
    /// restore allocation, delivery, or candidate publication.
    pub(crate) fn admit_retained(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata_collection(self.messages.len())?;
        self.admit_messages(admission)
    }

    fn admit_messages(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        for message in &self.messages {
            Self::validate_declaration(message.payload_type, &message.declaration, admission)?;
            admission.validate_owned_typed_borrowed(
                message.payload.borrowed_value(),
                message.payload_type,
            )?;
        }
        Ok(())
    }

    /// Runs before evaluating an emission operand or copying a channel capture.
    pub(crate) fn validate_destination(
        payload_type: RuntimeSemanticTypeId,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata(&payload_type)?;
        let result = admission
            .program_owner()
            .ok_or(AwbcRuntimeValueSnapshotError::ProgramOwnerRequired {
                expected: payload_type,
            })
            .and_then(|owner| {
                owner
                    .types()
                    .action_declaration(payload_type)
                    .map(|_| ())
                    .map_err(Into::into)
            });
        if let Err(error) = &result {
            admission.retain_failure(error.clone());
        }
        result
    }

    /// A retained address is a claim against the selected program, never an
    /// alternate declaration catalog. Semantic IDs alone cannot authorize a
    /// replacement that substituted an original Action declaration.
    fn validate_declaration(
        payload_type: RuntimeSemanticTypeId,
        declaration: &crate::entry::RuntimeNominalDeclarationId,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        Self::validate_destination(payload_type, admission)?;
        admission.validate_metadata(declaration)?;
        let owner = admission
            .program_owner()
            .expect("destination admitted its owner");
        if owner.types().action_declaration(payload_type)? != declaration {
            let error: AwbcRuntimeValueSnapshotError =
                crate::program_types::RuntimeProgramTypeError::ActionDeclarationMismatch {
                    semantic_type: payload_type,
                }
                .into();
            admission.retain_failure(error.clone());
            return Err(error);
        }
        Ok(())
    }
}

/// Failed emission retains the candidate payload and leaves the mailbox intact.
#[derive(Debug, thiserror::Error)]
#[error("{error}")]
pub(crate) struct RuntimeActionEmitError {
    #[source]
    pub(crate) error: AwbcRuntimeValueSnapshotError,
    pub(crate) payload: RuntimePayload,
}

/// Exclusive queue reservation after borrowed value and capacity preflight.
/// The execution owner transfers the very operand it admitted; it cannot run
/// another queue operation while this reservation is held.
pub(crate) struct PreparedActionEmission<'queue> {
    mailbox: &'queue mut RuntimeActionMailbox,
    payload_type: RuntimeSemanticTypeId,
    declaration: crate::entry::RuntimeNominalDeclarationId,
}

impl PreparedActionEmission<'_> {
    pub(crate) fn commit(self, payload: RuntimePayload) {
        self.mailbox.messages.push_back(ActionMessage {
            payload_type: self.payload_type,
            declaration: self.declaration,
            payload,
        });
    }
}

impl RuntimeActionMailbox {
    pub(crate) fn peek(&self, payload_type: RuntimeSemanticTypeId) -> Option<&RuntimePayload> {
        self.messages
            .iter()
            .find(|message| message.payload_type == payload_type)
            .map(|message| &message.payload)
    }

    pub(crate) fn has_message(&self, payload_type: RuntimeSemanticTypeId) -> bool {
        self.messages
            .iter()
            .any(|message| message.payload_type == payload_type)
    }

    pub(crate) fn emit(
        &mut self,
        payload_type: RuntimeSemanticTypeId,
        payload: RuntimePayload,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), RuntimeActionEmitError> {
        match self.prepare_emit(payload_type, payload.borrowed_value(), admission) {
            Ok(prepared) => {
                prepared.commit(payload);
                Ok(())
            }
            Err(error) => Err(RuntimeActionEmitError { error, payload }),
        }
    }

    pub(crate) fn prepare_emit(
        &mut self,
        payload_type: RuntimeSemanticTypeId,
        payload: crate::value::RuntimeBorrowedValue<'_>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<PreparedActionEmission<'_>, AwbcRuntimeValueSnapshotError> {
        let declaration = self.preflight_emission_header(payload_type, admission)?;
        admission.validate_owned_typed_borrowed(payload, payload_type)?;
        Ok(PreparedActionEmission {
            mailbox: self,
            payload_type,
            declaration,
        })
    }

    /// Bounds the destination, retained group and prospective capacity before
    /// the Native owner evaluates an operand. The later borrowed-value
    /// preparation repeats this header check in the same original allowance.
    pub(crate) fn preflight_emission_header(
        &mut self,
        payload_type: RuntimeSemanticTypeId,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<crate::entry::RuntimeNominalDeclarationId, AwbcRuntimeValueSnapshotError> {
        let result = (|| {
            Self::validate_destination(payload_type, admission)?;
            let count = self
                .messages
                .len()
                .checked_add(1)
                .ok_or(AwbcRuntimeValueSnapshotError::RetainedCollectionLengthOverflow)?;
            admission.validate_metadata_collection(count)?;
            self.admit_messages(admission)?;
            self.messages.try_reserve(1).map_err(|_| {
                AwbcRuntimeValueSnapshotError::RetainedCollectionAllocation { count }
            })?;
            let owner = admission
                .program_owner()
                .expect("destination admitted its owner")
                .clone();
            let declaration = owner.types().action_declaration(payload_type)?;
            admission.validate_metadata(declaration)?;
            Ok::<_, AwbcRuntimeValueSnapshotError>(declaration.clone())
        })();
        let declaration = result.map_err(|error| {
            admission.retain_failure(error.clone());
            error
        })?;
        Ok(declaration)
    }

    /// Validates the complete retained group before moving the first matching
    /// value. An empty observation creates neither a Host request nor a value.
    pub(crate) fn receive(
        &mut self,
        payload_type: RuntimeSemanticTypeId,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<Option<RuntimePayload>, AwbcRuntimeValueSnapshotError> {
        Self::validate_destination(payload_type, admission)?;
        self.admit_retained(admission)?;
        let Some(index) = self
            .messages
            .iter()
            .position(|message| message.payload_type == payload_type)
        else {
            return Ok(None);
        };
        Ok(Some(
            self.messages
                .remove(index)
                .expect("the admitted queue retains the selected position")
                .payload,
        ))
    }

    /// The enclosing execution owner has already admitted every retained root.
    pub(crate) fn snapshot_admitted(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> Result<RuntimeActionMailbox<AwbcRuntimeValueSnapshot>, AwbcRuntimeValueSnapshotError> {
        Ok(RuntimeActionMailbox {
            messages: self
                .messages
                .iter()
                .map(|message| {
                    Ok(ActionMessage {
                        payload_type: message.payload_type,
                        declaration: message.declaration.clone(),
                        payload: AwbcRuntimeValueSnapshot::from_admitted_runtime_value(
                            message.payload.value(),
                            owner,
                        )?,
                    })
                })
                .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
        })
    }
}

impl RuntimeActionMailbox<AwbcRuntimeValueSnapshot> {
    /// Inert transport has no authority to activate a payload.
    pub(crate) fn from_rows(
        rows: Vec<(
            RuntimeSemanticTypeId,
            crate::entry::RuntimeNominalDeclarationId,
            AwbcRuntimeValueSnapshot,
        )>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata_collection(rows.len())?;
        for (payload_type, declaration, payload) in &rows {
            Self::validate_declaration(*payload_type, declaration, admission)?;
            admission.validate_owned_typed_borrowed(payload.borrowed_value(), *payload_type)?;
        }
        Ok(Self {
            messages: rows
                .into_iter()
                .map(|(payload_type, declaration, payload)| ActionMessage {
                    payload_type,
                    declaration,
                    payload,
                })
                .collect(),
        })
    }

    /// Only called after the enclosing image's complete retained preflight.
    pub(crate) fn snapshot_rows_admitted(
        &self,
    ) -> Vec<(
        RuntimeSemanticTypeId,
        crate::entry::RuntimeNominalDeclarationId,
        AwbcRuntimeValueSnapshot,
    )> {
        self.messages
            .iter()
            .map(|message| {
                (
                    message.payload_type,
                    message.declaration.clone(),
                    message.payload.clone(),
                )
            })
            .collect()
    }

    /// Consumes only after the enclosing candidate's complete shared preflight.
    pub(crate) fn into_live_admitted<'a>(
        self,
        context: impl Into<crate::value::RuntimeValueRestoreContext<'a>>,
    ) -> Result<RuntimeActionMailbox, AwbcRuntimeValueSnapshotError> {
        let context = context.into();
        Ok(RuntimeActionMailbox {
            messages: self
                .messages
                .into_iter()
                .map(|message| {
                    Ok(ActionMessage {
                        payload_type: message.payload_type,
                        declaration: message.declaration,
                        payload: RuntimePayload::new(
                            message.payload.into_admitted_runtime_value(context)?,
                        ),
                    })
                })
                .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
        })
    }
}
