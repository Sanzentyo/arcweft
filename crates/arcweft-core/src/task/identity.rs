//! Final fixed task correlation identities, kept private until the atomic
//! task/Need carrier cut. These owners contain the version-one transcripts;
//! neither legacy String identifiers nor compatibility readers enter them.

use super::specification::NeedProducerInstance;
use super::{
    GenerationId, NeedProducerContractDigest, NeedProducerInstanceKey, TaskLaunchOrdinal,
    TaskPolicy,
};
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskIdentityKind {
    NeedProducerInstance,
    Need,
    TaskKey,
    Task,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum TaskIdentityError {
    #[error("fixed task identity {kind:?} must not be all zero")]
    Zero { kind: TaskIdentityKind },
    #[error("task policy tag {0} is not assigned in version 1")]
    UnknownPolicy(u8),
    #[error("JoinSameKey requires launch ordinal zero")]
    NonZeroJoinOrdinal,
    #[error("AlwaysStart requires a nonzero launch ordinal")]
    ZeroAlwaysStartOrdinal,
}

macro_rules! fixed_identity {
    ($name:ident, $kind:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[repr(transparent)]
        pub(super) struct $name([u8; 32]);

        impl $name {
            pub(super) fn try_from_bytes(bytes: [u8; 32]) -> Result<Self, TaskIdentityError> {
                if bytes == [0; 32] {
                    Err(TaskIdentityError::Zero {
                        kind: TaskIdentityKind::$kind,
                    })
                } else {
                    Ok(Self(bytes))
                }
            }

            pub(super) const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let bytes = <[u8; 32]>::deserialize(deserializer)?;
                Self::try_from_bytes(bytes).map_err(serde::de::Error::custom)
            }
        }
    };
}

fixed_identity!(NeedId, Need);
fixed_identity!(TaskKey, TaskKey);
fixed_identity!(TaskId, Task);

impl NeedId {
    pub(super) fn try_for(
        producer: NeedProducerInstanceKey,
        policy: TaskPolicy,
        ordinal: TaskLaunchOrdinal,
    ) -> Result<Self, TaskIdentityError> {
        TaskLaunchOrdinal::try_for_policy(policy, ordinal.get())?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.need.id.v1\0");
        hasher.update(producer.as_bytes());
        hasher.update(&[policy.semantic_tag()]);
        hasher.update(&ordinal.get().to_le_bytes());
        Self::try_from_bytes(*hasher.finalize().as_bytes())
    }
}

impl TaskKey {
    pub(super) fn try_for(
        generation: GenerationId,
        producer: NeedProducerInstanceKey,
        policy: TaskPolicy,
    ) -> Result<Self, TaskIdentityError> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.task.key.v1\0");
        hasher.update(&generation.get().to_le_bytes());
        hasher.update(producer.as_bytes());
        hasher.update(&[policy.semantic_tag()]);
        Self::try_from_bytes(*hasher.finalize().as_bytes())
    }
}

impl TaskId {
    pub(super) fn try_for(
        task_key: TaskKey,
        ordinal: TaskLaunchOrdinal,
    ) -> Result<Self, TaskIdentityError> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.task.id.v1\0");
        hasher.update(task_key.as_bytes());
        hasher.update(&ordinal.get().to_le_bytes());
        Self::try_from_bytes(*hasher.finalize().as_bytes())
    }
}

/// One complete correlation row derived from the producer and launch authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct TaskCorrelation {
    pub(super) generation: GenerationId,
    pub(super) producer: NeedProducerInstanceKey,
    pub(super) producer_contract: NeedProducerContractDigest,
    pub(super) need: NeedId,
    pub(super) task_key: TaskKey,
    pub(super) task_id: TaskId,
    pub(super) launch_ordinal: TaskLaunchOrdinal,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(super) enum TaskCorrelationError {
    #[error("task correlation identity derivation failed: {0}")]
    Identity(#[from] TaskIdentityError),
    #[error("task correlation differs from its complete producer/launch authority")]
    Mismatch,
}

impl TaskCorrelation {
    pub(super) fn try_for(
        generation: GenerationId,
        spec: &NeedProducerInstance,
        policy: TaskPolicy,
        ordinal: TaskLaunchOrdinal,
    ) -> Result<Self, TaskIdentityError> {
        let producer = spec.key();
        let need = NeedId::try_for(producer, policy, ordinal)?;
        let task_key = TaskKey::try_for(generation, producer, policy)?;
        let task_id = TaskId::try_for(task_key, ordinal)?;
        Ok(Self {
            generation,
            producer,
            producer_contract: spec.contract(),
            need,
            task_key,
            task_id,
            launch_ordinal: ordinal,
        })
    }

    pub(super) fn validate(
        &self,
        generation: GenerationId,
        spec: &NeedProducerInstance,
        policy: TaskPolicy,
        ordinal: TaskLaunchOrdinal,
    ) -> Result<(), TaskCorrelationError> {
        let expected = Self::try_for(generation, spec, policy, ordinal)?;
        if *self == expected {
            Ok(())
        } else {
            Err(TaskCorrelationError::Mismatch)
        }
    }
}

#[cfg(test)]
mod tests;
