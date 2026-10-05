//! Final task/handle specification, private until the atomic public carrier cut.
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NeedProducerInstance {
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
    pub(super) const fn key(&self) -> NeedProducerInstanceKey {
        self.key
    }
    pub(super) const fn contract(&self) -> NeedProducerContractDigest {
        self.contract
    }
    pub(super) const fn plan(&self) -> TaskPlanSemanticDigest {
        self.plan
    }
    pub(super) const fn payload_type(&self) -> RuntimeTypeSemanticDigest {
        self.payload_type
    }
    pub(super) const fn arguments(&self) -> RuntimeValueDigest {
        self.arguments
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct TaskSpec {
    pub(super) generation: GenerationId,
    pub(super) producer: NeedProducerInstance,
    pub(super) class: TaskClass,
    pub(super) priority: TaskPriority,
    pub(super) cancel_scope: CancelScopeId,
    pub(super) policy: TaskPolicy,
    pub(super) outcome: TaskOutcomeContract,
    pub(super) request: HostTaskRequest,
    pub(super) debug_label: String,
}

impl TaskSpec {
    /// The launch journal supplies the ordinal at admission. This projection
    /// does not allocate or consume it, start work, or publish a handle.
    pub(super) fn correlation(
        &self,
        ordinal: TaskLaunchOrdinal,
    ) -> Result<TaskCorrelation, TaskIdentityError> {
        TaskCorrelation::try_for(self.generation, &self.producer, self.policy, ordinal)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct TaskHandle {
    pub(super) correlation: TaskCorrelation,
}
