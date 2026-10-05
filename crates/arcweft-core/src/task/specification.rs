//! Final task/handle specification.
//! A start specification contains the complete existing producer authority and
//! never caller-supplied Need/task identities or a launch ordinal.

use super::identity::TaskCorrelation;
use super::{
    CancelScopeId, GenerationId, HostTaskRequest, NeedProducerContractDigest,
    NeedProducerInstanceKey, NeedProducerSpec, RuntimeTypeSemanticDigest, TaskClass,
    TaskIdentityError, TaskLaunchOrdinal, TaskOutcomeContract, TaskPlanSemanticDigest, TaskPolicy,
    TaskPriority,
};
use crate::entry::RuntimeValueDigest;

/// Issued producer instance, distinct from its complete derivation input.
/// The private constructor commits every input through the existing sole key
/// encoder, then retains exactly the accepted instance contract fields.
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct NeedProducerInstance {
    key: NeedProducerInstanceKey,
    contract: NeedProducerContractDigest,
    plan: TaskPlanSemanticDigest,
    payload_type: RuntimeTypeSemanticDigest,
    arguments: RuntimeValueDigest,
}

impl TryFrom<&NeedProducerSpec> for NeedProducerInstance {
    type Error = TaskIdentityError;

    fn try_from(input: &NeedProducerSpec) -> Result<Self, Self::Error> {
        Ok(Self {
            key: input.instance_key()?,
            contract: input.contract(),
            plan: input.plan(),
            payload_type: input.payload_type(),
            arguments: input.arguments(),
        })
    }
}

impl NeedProducerInstance {
    pub const fn key(&self) -> NeedProducerInstanceKey {
        self.key
    }
    pub const fn contract(&self) -> NeedProducerContractDigest {
        self.contract
    }
    pub const fn plan(&self) -> TaskPlanSemanticDigest {
        self.plan
    }
    pub const fn payload_type(&self) -> RuntimeTypeSemanticDigest {
        self.payload_type
    }
    pub const fn arguments(&self) -> RuntimeValueDigest {
        self.arguments
    }
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSpec {
    pub generation: GenerationId,
    pub producer: NeedProducerInstance,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub policy: TaskPolicy,
    pub outcome: TaskOutcomeContract,
    pub request: HostTaskRequest,
    pub debug_label: String,
}

impl TaskSpec {
    pub(crate) fn validate_outcome(&self) -> Result<(), super::TaskEnsureError> {
        if self.outcome.payload_semantic_identity().as_bytes()
            != self.producer.payload_type().as_bytes()
        {
            return Err(super::TaskEnsureError::OutcomeContractMismatch);
        }
        Ok(())
    }

    /// Full join contract. Diagnostic text never changes accepted work.
    pub fn same_join_contract(&self, other: &Self) -> bool {
        self.generation == other.generation
            && self.producer == other.producer
            && self.class == other.class
            && self.priority == other.priority
            && self.cancel_scope == other.cancel_scope
            && self.policy == other.policy
            && self.outcome == other.outcome
            && self.request == other.request
    }
    /// The launch journal supplies the ordinal at admission. This projection
    /// does not allocate or consume it, start work, or publish a handle.
    pub fn correlation(
        &self,
        ordinal: TaskLaunchOrdinal,
    ) -> Result<TaskCorrelation, TaskIdentityError> {
        TaskCorrelation::try_for(self.generation, &self.producer, self.policy, ordinal)
    }
}

#[derive(
    Clone, Copy, Debug, serde::Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize,
)]
#[serde(deny_unknown_fields)]
pub struct TaskHandle {
    pub correlation: TaskCorrelation,
}
