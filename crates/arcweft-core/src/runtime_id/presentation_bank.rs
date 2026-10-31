//! Inert namespace identity of one Root or dialogue-owned presentation bank.

use super::{DialogueActivationId, RuntimeDialogueContentPlanId, RuntimePersistentFiberId};
use crate::effect::RuntimeArtifactFingerprint;
use crate::value::{RuntimeScalarView, RuntimeUInt, RuntimeValue, RuntimeValueView};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

/// Bank selection is part of resource identity, rather than a host-side lookup
/// inferred from the currently displayed line. Decoding grants no custody.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", deny_unknown_fields, rename_all = "snake_case")]
pub enum RuntimePresentationBankId {
    Root,
    Dialogue { activation: DialogueActivationId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimePresentationBankIdDecodeError {
    #[error("presentation bank identity has an invalid closed shape")]
    Shape,
    #[error("presentation bank identity has an invalid artifact fingerprint")]
    Artifact,
    #[error("presentation bank content identity must be nonzero")]
    Content,
}

impl RuntimePresentationBankId {
    pub(crate) fn matches_artifact(self, artifact: RuntimeArtifactFingerprint) -> bool {
        match self {
            Self::Root => true,
            Self::Dialogue { activation } => activation.artifact() == artifact,
        }
    }

    pub(crate) fn encode_payload(self) -> RuntimeValue {
        match self {
            Self::Root => RuntimeValue::Tuple(vec![RuntimeValue::UInt(RuntimeUInt::U8(0))]),
            Self::Dialogue { activation } => RuntimeValue::Tuple(vec![
                RuntimeValue::UInt(RuntimeUInt::U8(1)),
                RuntimeValue::Seq(crate::value::RuntimeSeq::dense_bytes(
                    activation.artifact().as_bytes().to_vec(),
                )),
                RuntimeValue::u64(activation.owner_fiber().get()),
                RuntimeValue::u32(activation.content().get().get()),
                RuntimeValue::u64(activation.occurrence()),
            ]),
        }
    }

    pub(crate) fn try_decode_view(
        value: RuntimeValueView<'_>,
    ) -> Result<Self, RuntimePresentationBankIdDecodeError> {
        use RuntimePresentationBankIdDecodeError as Error;
        let RuntimeValueView::Tuple(fields) = value else {
            return Err(Error::Shape);
        };
        match (fields.len(), fields.get(0)) {
            (1, Some(RuntimeValueView::Scalar(RuntimeScalarView::UInt(RuntimeUInt::U8(0))))) => {
                Ok(Self::Root)
            }
            (5, Some(RuntimeValueView::Scalar(RuntimeScalarView::UInt(RuntimeUInt::U8(1))))) => {
                let Some(RuntimeValueView::Sequence(artifact)) = fields.get(1) else {
                    return Err(Error::Shape);
                };
                let bytes = artifact
                    .byte_storage()
                    .and_then(|bytes| bytes.try_into().ok())
                    .ok_or(Error::Artifact)?;
                let artifact = RuntimeArtifactFingerprint::try_from_bytes(bytes)
                    .map_err(|_| Error::Artifact)?;
                let Some(RuntimeValueView::Scalar(RuntimeScalarView::UInt(RuntimeUInt::U64(
                    fiber,
                )))) = fields.get(2)
                else {
                    return Err(Error::Shape);
                };
                let Some(RuntimeValueView::Scalar(RuntimeScalarView::UInt(RuntimeUInt::U32(
                    content,
                )))) = fields.get(3)
                else {
                    return Err(Error::Shape);
                };
                let content = NonZeroU32::new(content)
                    .map(RuntimeDialogueContentPlanId::from_accepted_ordinal)
                    .ok_or(Error::Content)?;
                let Some(RuntimeValueView::Scalar(RuntimeScalarView::UInt(RuntimeUInt::U64(
                    occurrence,
                )))) = fields.get(4)
                else {
                    return Err(Error::Shape);
                };
                Ok(Self::Dialogue {
                    activation: DialogueActivationId::new(
                        artifact,
                        RuntimePersistentFiberId::from_allocated(fiber),
                        content,
                        occurrence,
                    ),
                })
            }
            _ => Err(Error::Shape),
        }
    }
}
