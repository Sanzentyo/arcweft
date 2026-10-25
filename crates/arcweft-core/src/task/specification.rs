//! Final task/handle specification.
//! A start specification contains the complete existing producer authority and
//! never caller-supplied Need/task identities or a launch ordinal.

use super::identity::TaskCorrelation;
use super::{
    CancelScopeId, GenerationId, NeedProducerContractDigest, NeedProducerInstanceKey,
    NeedProducerSpec, RuntimeTypeSemanticDigest, TaskClass, TaskIdentityError, TaskLaunchOrdinal,
    TaskOutcomeContract, TaskPlanSemanticDigest, TaskPolicy, TaskPriority, TaskRequest,
};
use crate::entry::RuntimeValueDigest;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NeedProducerAssertionField {
    Plan,
    Contract,
    HandleType,
    Arguments,
    Invocation,
    Key,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum NeedProducerAssertionError {
    #[error("stored producer differs from its original source in {field:?}")]
    Mismatch { field: NeedProducerAssertionField },
    #[error(transparent)]
    Identity(#[from] TaskIdentityError),
}

/// Issued producer instance, distinct from its complete derivation input.
/// The private constructor commits every input through the existing sole key
/// encoder, then retains exactly the accepted instance contract fields.
/// Inert assertions retain the same fields and cannot use the live issuer.
///
/// ```compile_fail
/// use arcweft_core::task::{NeedProducerInstance, NeedProducerSpec};
/// fn untrusted(spec: &NeedProducerSpec<[u8; 32]>) {
///     let _ = NeedProducerInstance::<[u8; 32]>::try_from(spec);
/// }
/// ```
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct NeedProducerInstance<Plan = TaskPlanSemanticDigest> {
    key: NeedProducerInstanceKey,
    contract: NeedProducerContractDigest,
    plan: Plan,
    handle_type: super::TaskHandleType,
    arguments: RuntimeValueDigest,
    binding: Option<crate::program_types::RuntimeFunctionEffectInstantiation>,
}

impl TryFrom<&NeedProducerSpec> for NeedProducerInstance {
    type Error = TaskIdentityError;

    fn try_from(input: &NeedProducerSpec) -> Result<Self, Self::Error> {
        Ok(Self {
            key: input.instance_key()?,
            contract: input.contract(),
            plan: input.plan(),
            handle_type: input.handle_type(),
            arguments: input.arguments(),
            binding: input.binding().cloned(),
        })
    }
}

impl TryFrom<&NeedProducerSpec<crate::plan::RuntimeTaskSource>> for NeedProducerInstance {
    type Error = TaskIdentityError;

    fn try_from(
        input: &NeedProducerSpec<crate::plan::RuntimeTaskSource>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            key: input.instance_key()?,
            contract: input.contract(),
            plan: input.source().task_plan().key(),
            handle_type: input.handle_type(),
            arguments: input.arguments(),
            binding: input.binding().cloned(),
        })
    }
}

impl NeedProducerInstance {
    /// Called after the enclosing source/value batch is admitted. The issued
    /// key and immutable invocation metadata become original-source assertions.
    pub(crate) fn plan_assertion(&self) -> NeedProducerInstance<super::ExpectedTaskPlanKey> {
        NeedProducerInstance {
            key: self.key,
            contract: self.contract,
            plan: super::ExpectedTaskPlanKey::new(*self.plan.as_bytes()),
            handle_type: self.handle_type,
            arguments: self.arguments,
            binding: self.binding.clone(),
        }
    }
    pub const fn key(&self) -> NeedProducerInstanceKey {
        self.key
    }
    pub const fn contract(&self) -> NeedProducerContractDigest {
        self.contract
    }
    pub const fn plan(&self) -> TaskPlanSemanticDigest {
        self.plan
    }
    pub const fn handle_type(&self) -> super::TaskHandleType {
        self.handle_type
    }

    pub const fn payload_type(&self) -> RuntimeTypeSemanticDigest {
        self.handle_type.payload_type()
    }
    pub const fn arguments(&self) -> RuntimeValueDigest {
        self.arguments
    }
    pub(crate) fn binding(
        &self,
    ) -> Option<&crate::program_types::RuntimeFunctionEffectInstantiation> {
        self.binding.as_ref()
    }
}

impl NeedProducerInstance<super::ExpectedTaskPlanKey> {
    /// The caller supplies the original producer binding, never a spec rebuilt
    /// from assertion fields. No assertion bytes are converted into a key.
    pub(crate) fn admit_original_spec(
        self,
        original: &NeedProducerSpec,
    ) -> Result<NeedProducerInstance, NeedProducerAssertionError> {
        let mismatch = if self.plan.bytes() != original.plan().as_bytes() {
            Some(NeedProducerAssertionField::Plan)
        } else if self.contract != original.contract() {
            Some(NeedProducerAssertionField::Contract)
        } else if self.handle_type != original.handle_type() {
            Some(NeedProducerAssertionField::HandleType)
        } else if self.arguments != original.arguments() {
            Some(NeedProducerAssertionField::Arguments)
        } else if self.binding.as_ref() != original.binding() {
            Some(NeedProducerAssertionField::Invocation)
        } else {
            None
        };
        if let Some(field) = mismatch {
            return Err(NeedProducerAssertionError::Mismatch { field });
        }
        let canonical = NeedProducerInstance::try_from(original)?;
        if self.key != canonical.key {
            return Err(NeedProducerAssertionError::Mismatch {
                field: NeedProducerAssertionField::Key,
            });
        }
        Ok(canonical)
    }
}

/// The plan role is distinct from payload storage. Decoded plan assertions
/// cannot derive live correlations before original-program admission.
///
/// ```compile_fail
/// use arcweft_core::task::{TaskLaunchOrdinal, TaskSpec};
/// use arcweft_core::value::RuntimePayload;
/// fn untrusted(spec: &TaskSpec<RuntimePayload, [u8; 32]>) {
///     let _ = spec.correlation(TaskLaunchOrdinal::JOIN);
/// }
/// ```
#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSpec<
    P = crate::value::RuntimePayload,
    Plan = TaskPlanSemanticDigest,
    S = crate::plan::RuntimeTaskSource,
> {
    pub generation: GenerationId,
    pub producer: NeedProducerInstance<Plan>,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub policy: TaskPolicy,
    pub outcome: TaskOutcomeContract,
    pub request: TaskRequest<P, S>,
    pub debug_label: String,
}

impl<P, S> TaskSpec<P, TaskPlanSemanticDigest, S> {
    /// Changes only the Source storage role after enclosing batch admission.
    /// Payload custody and the complete issued producer contract move intact.
    pub(crate) fn map_source_admitted<T, E>(
        self,
        source: impl FnOnce(S) -> Result<T, E>,
    ) -> Result<TaskSpec<P, TaskPlanSemanticDigest, T>, E> {
        Ok(TaskSpec {
            generation: self.generation,
            producer: self.producer,
            class: self.class,
            priority: self.priority,
            cancel_scope: self.cancel_scope,
            policy: self.policy,
            outcome: self.outcome,
            request: self.request.into_mapped_admitted(Ok, source)?,
            debug_label: self.debug_label,
        })
    }
}

impl<P, S: super::TaskExecutionSource> TaskSpec<P, TaskPlanSemanticDigest, S> {
    pub(crate) fn validate_outcome(&self) -> Result<(), super::TaskEnsureError> {
        self.request
            .validate_handle_type(self.producer.handle_type())?;
        if !self.request.matches_plan(self.producer.plan()) {
            return Err(super::TaskEnsureError::ProgramSourceMismatch);
        }
        if matches!(&self.request, TaskRequest::Program(_))
            && !matches!(&self.outcome, TaskOutcomeContract::Program { .. })
        {
            return Err(super::TaskEnsureError::OutcomeContractMismatch);
        }
        self.outcome.validate_producer(&self.producer)
    }

    pub(crate) fn validate_handle(&self, handle: TaskHandle) -> Result<(), super::TaskEnsureError> {
        self.validate_outcome()?;
        let expected = self.correlation(handle.correlation.launch_ordinal)?;
        if expected != handle.correlation {
            return Err(super::TaskEnsureError::TaskIdSpecificationConflict {
                task_id: handle.correlation.task_id,
            });
        }
        Ok(())
    }

    /// Full join contract. Diagnostic text never changes accepted work.
    pub fn same_join_contract(&self, other: &Self) -> bool
    where
        P: PartialEq,
        S: PartialEq,
    {
        self.same_execution_contract(other) && self.request == other.request
    }

    /// A Program receipt has no runtime request payload. Its complete contract
    /// can therefore be checked across live and inert value storage roles.
    pub(crate) fn same_program_contract<Q>(
        &self,
        other: &TaskSpec<Q, TaskPlanSemanticDigest, S>,
    ) -> bool
    where
        S: PartialEq,
    {
        self.same_execution_contract(other)
            && matches!((&self.request, &other.request),
                (TaskRequest::Program(left), TaskRequest::Program(right)) if left == right)
    }

    fn same_execution_contract<Q>(&self, other: &TaskSpec<Q, TaskPlanSemanticDigest, S>) -> bool {
        self.generation == other.generation
            && self.producer == other.producer
            && self.class == other.class
            && self.priority == other.priority
            && self.cancel_scope == other.cancel_scope
            && self.policy == other.policy
            && self.outcome == other.outcome
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
