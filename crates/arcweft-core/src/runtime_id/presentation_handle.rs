//! Exact execution identity for a general presentation resource lease.

use super::{ExecutionInstanceId, RuntimePersistentFiberId, RuntimePresentationBankId};
use crate::effect::RuntimeArtifactFingerprint;
use crate::value::{RuntimeScalarView, RuntimeUInt, RuntimeValue, RuntimeValueView};
use arcweft_interaction_model::presentation::PresentationResourceKind;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

/// Inert identity of a presentation lease, distinct from a dialogue activation.
/// A token grants no resource access without its original execution ledger.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePresentationHandleToken {
    artifact: RuntimeArtifactFingerprint,
    resource: PresentationResourceKind,
    execution: ExecutionInstanceId,
    fiber: RuntimePersistentFiberId,
    issuance: NonZeroU64,
    bank: RuntimePresentationBankId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimePresentationHandleTokenDecodeError {
    #[error("presentation handle token must contain its six canonical fields")]
    Shape,
    #[error(transparent)]
    Bank(#[from] super::RuntimePresentationBankIdDecodeError),
    #[error("presentation handle token contains an unknown resource kind")]
    Resource,
    #[error("presentation handle token contains an invalid artifact fingerprint")]
    Artifact,
    #[error("presentation handle token execution identity must be nonzero")]
    Execution,
    #[error("presentation handle token issuance must be nonzero")]
    Issuance,
}

impl RuntimePresentationHandleToken {
    pub(crate) const fn from_issued(
        artifact: RuntimeArtifactFingerprint,
        resource: PresentationResourceKind,
        execution: ExecutionInstanceId,
        fiber: RuntimePersistentFiberId,
        issuance: NonZeroU64,
        bank: RuntimePresentationBankId,
    ) -> Self {
        Self {
            artifact,
            resource,
            execution,
            fiber,
            issuance,
            bank,
        }
    }

    pub const fn artifact(&self) -> &RuntimeArtifactFingerprint {
        &self.artifact
    }

    pub const fn resource(&self) -> PresentationResourceKind {
        self.resource
    }
    pub const fn execution(&self) -> ExecutionInstanceId {
        self.execution
    }
    pub const fn fiber(&self) -> RuntimePersistentFiberId {
        self.fiber
    }
    pub const fn issuance(&self) -> NonZeroU64 {
        self.issuance
    }

    pub const fn bank(&self) -> RuntimePresentationBankId {
        self.bank
    }

    /// The canonical value envelope is independent of Native/AWBC storage.
    pub fn encode_payload(&self) -> RuntimeValue {
        RuntimeValue::Tuple(vec![
            RuntimeValue::Seq(crate::value::RuntimeSeq::dense_bytes(
                self.artifact.as_bytes().to_vec(),
            )),
            RuntimeValue::UInt(RuntimeUInt::U8(self.resource.semantic_tag())),
            RuntimeValue::u64(self.execution.get().get()),
            RuntimeValue::u64(self.fiber.get()),
            RuntimeValue::u64(self.issuance.get()),
            self.bank.encode_payload(),
        ])
    }

    /// Reads inert identity metadata; this does not issue or activate a lease.
    pub fn try_decode_payload(
        value: &RuntimeValue,
    ) -> Result<Self, RuntimePresentationHandleTokenDecodeError> {
        Self::try_decode_view(value.view())
    }

    pub(crate) fn try_decode_view(
        value: RuntimeValueView<'_>,
    ) -> Result<Self, RuntimePresentationHandleTokenDecodeError> {
        use RuntimePresentationHandleTokenDecodeError as Error;
        let RuntimeValueView::Tuple(fields) = value else {
            return Err(Error::Shape);
        };
        if fields.len() != 6 {
            return Err(Error::Shape);
        }
        let Some(RuntimeValueView::Sequence(artifact)) = fields.get(0) else {
            return Err(Error::Shape);
        };
        let artifact = artifact
            .byte_storage()
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(Error::Artifact)?;
        let artifact =
            RuntimeArtifactFingerprint::try_from_bytes(artifact).map_err(|_| Error::Artifact)?;
        let Some(RuntimeValueView::Scalar(RuntimeScalarView::UInt(RuntimeUInt::U8(resource)))) =
            fields.get(1)
        else {
            return Err(Error::Shape);
        };
        let resource =
            PresentationResourceKind::from_semantic_tag(resource).ok_or(Error::Resource)?;
        let scalar = |index| match fields.get(index) {
            Some(RuntimeValueView::Scalar(RuntimeScalarView::UInt(RuntimeUInt::U64(value)))) => {
                Ok(value)
            }
            _ => Err(Error::Shape),
        };
        let execution = NonZeroU64::new(scalar(2)?)
            .map(ExecutionInstanceId::from_allocated)
            .ok_or(Error::Execution)?;
        let fiber = RuntimePersistentFiberId::from_allocated(scalar(3)?);
        let issuance = NonZeroU64::new(scalar(4)?).ok_or(Error::Issuance)?;
        let bank = RuntimePresentationBankId::try_decode_view(fields.get(5).ok_or(Error::Shape)?)?;
        if !bank.matches_artifact(artifact) {
            return Err(Error::Artifact);
        }
        Ok(Self::from_issued(
            artifact, resource, execution, fiber, issuance, bank,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token() -> RuntimePresentationHandleToken {
        RuntimePresentationHandleToken::from_issued(
            RuntimeArtifactFingerprint::try_from_bytes([7; 32]).unwrap(),
            PresentationResourceKind::Image,
            ExecutionInstanceId::from_allocated(NonZeroU64::new(3).unwrap()),
            RuntimePersistentFiberId::from_allocated(0),
            NonZeroU64::new(1).unwrap(),
            RuntimePresentationBankId::Root,
        )
    }

    #[test]
    fn presentation_token_roundtrip_retains_exact_execution_and_fiber_identity() {
        let token = token();
        assert_eq!(
            RuntimePresentationHandleToken::try_decode_payload(&token.encode_payload()).unwrap(),
            token
        );
        let encoded = serde_json::to_string(&token).unwrap();
        assert_eq!(
            serde_json::from_str::<RuntimePresentationHandleToken>(&encoded).unwrap(),
            token
        );
        assert_eq!(token.execution().get().get(), 3);
        assert_eq!(token.fiber().get(), 0);
        assert_eq!(token.issuance().get(), 1);
        let mut identities = std::collections::BTreeSet::new();
        for resource in PresentationResourceKind::ALL {
            let selected = RuntimePresentationHandleToken::from_issued(
                token.artifact().clone(),
                resource,
                token.execution(),
                token.fiber(),
                token.issuance(),
                token.bank(),
            );
            assert_eq!(
                RuntimePresentationHandleToken::try_decode_payload(&selected.encode_payload())
                    .unwrap(),
                selected
            );
            assert!(identities.insert(selected));
        }
        assert_ne!(
            token,
            RuntimePresentationHandleToken::from_issued(
                token.artifact().clone(),
                token.resource(),
                ExecutionInstanceId::from_allocated(NonZeroU64::new(4).unwrap()),
                token.fiber(),
                token.issuance(),
                token.bank(),
            )
        );
    }

    #[test]
    fn presentation_token_decode_rejects_shape_width_zero_and_extra_fields() {
        use RuntimePresentationHandleTokenDecodeError as Error;
        let token = token();
        for (index, replacement, expected) in [
            (
                0,
                RuntimeValue::Seq(crate::value::RuntimeSeq::dense_bytes(vec![7; 31])),
                Error::Artifact,
            ),
            (1, RuntimeValue::UInt(RuntimeUInt::U8(8)), Error::Resource),
            (1, RuntimeValue::u32(0), Error::Shape),
            (2, RuntimeValue::u64(0), Error::Execution),
            (2, RuntimeValue::u32(3), Error::Shape),
            (3, RuntimeValue::u32(0), Error::Shape),
            (4, RuntimeValue::u64(0), Error::Issuance),
        ] {
            let RuntimeValue::Tuple(mut fields) = token.encode_payload() else {
                unreachable!()
            };
            fields[index] = replacement;
            assert_eq!(
                RuntimePresentationHandleToken::try_decode_payload(&RuntimeValue::Tuple(fields)),
                Err(expected)
            );
        }
        let RuntimeValue::Tuple(mut fields) = token.encode_payload() else {
            unreachable!()
        };
        fields.push(RuntimeValue::Unit);
        assert_eq!(
            RuntimePresentationHandleToken::try_decode_payload(&RuntimeValue::Tuple(fields)),
            Err(Error::Shape)
        );
        let mut encoded = serde_json::to_value(&token).unwrap();
        encoded
            .as_object_mut()
            .unwrap()
            .insert("dialogue".into(), serde_json::json!(1));
        assert!(serde_json::from_value::<RuntimePresentationHandleToken>(encoded).is_err());
    }

    #[test]
    fn presentation_bank_is_part_of_exact_token_identity_and_closed_payload() {
        use super::super::{DialogueActivationId, RuntimeDialogueContentPlanId};
        use std::num::NonZeroU32;
        let root = token();
        let bank = RuntimePresentationBankId::Dialogue {
            activation: DialogueActivationId::new(
                *root.artifact(),
                RuntimePersistentFiberId::from_allocated(11),
                RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::new(3).unwrap()),
                4,
            ),
        };
        let dialogue = RuntimePresentationHandleToken::from_issued(
            *root.artifact(),
            root.resource(),
            root.execution(),
            root.fiber(),
            root.issuance(),
            bank,
        );
        assert_ne!(root, dialogue, "bank identity cannot alias a Root lease");
        assert_eq!(
            RuntimePresentationHandleToken::try_decode_payload(&dialogue.encode_payload()).unwrap(),
            dialogue
        );
        assert_eq!(
            serde_json::from_slice::<RuntimePresentationHandleToken>(
                &serde_json::to_vec(&dialogue).unwrap()
            )
            .unwrap(),
            dialogue
        );
        let RuntimeValue::Tuple(mut envelope) = dialogue.encode_payload() else {
            unreachable!()
        };
        let RuntimeValue::Tuple(fields) = &mut envelope[5] else {
            unreachable!()
        };
        fields[3] = RuntimeValue::u32(0);
        assert_eq!(
            RuntimePresentationHandleToken::try_decode_payload(&RuntimeValue::Tuple(envelope)),
            Err(RuntimePresentationHandleTokenDecodeError::Bank(
                super::super::RuntimePresentationBankIdDecodeError::Content
            ))
        );
        let RuntimeValue::Tuple(mut envelope) = dialogue.encode_payload() else {
            unreachable!()
        };
        let RuntimeValue::Tuple(fields) = &mut envelope[5] else {
            unreachable!()
        };
        fields[1] = RuntimeValue::Seq(crate::value::RuntimeSeq::dense_bytes(vec![8; 32]));
        assert_eq!(
            RuntimePresentationHandleToken::try_decode_payload(&RuntimeValue::Tuple(envelope)),
            Err(RuntimePresentationHandleTokenDecodeError::Artifact)
        );
        let RuntimeValue::Tuple(mut envelope) = root.encode_payload() else {
            unreachable!()
        };
        envelope.pop();
        assert_eq!(
            RuntimePresentationHandleToken::try_decode_payload(&RuntimeValue::Tuple(envelope)),
            Err(RuntimePresentationHandleTokenDecodeError::Shape),
            "the replaced five-field reader is not retained"
        );
    }
}
