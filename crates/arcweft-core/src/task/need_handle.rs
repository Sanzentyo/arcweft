//! Exact reusable-join or accepted-launch handle authority.
//! Metadata Clone is never language-level Copy evidence.

use super::identity::{NeedId, TaskCorrelation};
use super::specification::{TaskHandle, TaskSpec};
use super::{TaskIdentityError, TaskLaunchOrdinal, TaskOutcomeContract, TaskPolicy};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(transparent)]
pub struct RuntimeNeedHandle(Box<RuntimeNeedHandleData>);

// Keep recursive RuntimeValue/pattern nodes compact while retaining one owned
// allocation. Snapshot moves preserve this allocation and its argument custody.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeNeedHandleData {
    correlation: TaskCorrelation,
    spec: TaskSpec,
    origin: RuntimeNeedHandleOrigin,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
enum RuntimeNeedHandleOrigin {
    ReusableJoin,
    AcceptedLaunch,
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeNeedHandleSaveSnapshot {
    correlation: TaskCorrelation,
    spec: Box<super::TaskSpec<crate::value::AwbcRuntimeValueSnapshot>>,
    origin: RuntimeNeedHandleOrigin,
}

impl RuntimeNeedHandleSaveSnapshot {
    pub(crate) fn visit_descriptor(
        &self,
        visitor: &mut impl super::descriptor::TaskDescriptorVisitor,
        depth: usize,
    ) -> Result<(), crate::entry::RuntimeSchemaError> {
        self.origin
            .visit_descriptor(self.correlation, &self.spec, visitor, depth)
    }
    pub(crate) fn view(&self) -> crate::value::RuntimeNeedView<'_> {
        crate::value::RuntimeNeedView::Snapshot(self)
    }
    pub(crate) fn spec(&self) -> &super::TaskSpec<crate::value::AwbcRuntimeValueSnapshot> {
        &self.spec
    }

    pub(crate) fn validate_identity(&self) -> Result<(), RuntimeNeedHandleError> {
        let mut resources = crate::entry::schema::value_encoding::ValueResourceAdmission::snapshot(
            crate::entry::RuntimeSchemaLimits::engine_default(),
            crate::entry::schema::value_encoding::ValueAdmissionOrigin::InertSnapshot,
        );
        resources
            .validate_task_descriptor(&self.spec, 0)
            .map_err(|error| RuntimeNeedHandleError::Descriptor(Box::new(error)))?;
        self.origin.validate(
            self.correlation,
            self.spec.generation,
            &self.spec.producer,
            self.spec.policy,
            &self.spec.outcome,
        )
    }

    pub(crate) fn outcome(&self) -> &TaskOutcomeContract {
        &self.spec.outcome
    }
    pub(crate) const fn correlation(&self) -> TaskCorrelation {
        self.correlation
    }
    pub(crate) const fn requires_producer_custody(&self) -> bool {
        matches!(self.origin, RuntimeNeedHandleOrigin::AcceptedLaunch)
    }
    pub(crate) fn from_live(
        handle: &RuntimeNeedHandle,
        owner: Option<&super::RuntimeProgramOwner>,
    ) -> Result<Self, crate::value::AwbcRuntimeValueSnapshotError> {
        Ok(Self {
            correlation: handle.0.correlation,
            spec: Box::new(
                super::TaskSpec::<crate::value::AwbcRuntimeValueSnapshot>::from_live(
                    &handle.0.spec,
                    owner,
                )?,
            ),
            origin: handle.0.origin,
        })
    }

    pub(crate) fn into_live(
        self,
        owner: &super::RuntimeProgramOwner,
    ) -> Result<RuntimeNeedHandle, crate::value::AwbcRuntimeValueSnapshotError> {
        self.validate_identity().map_err(|error| {
            crate::value::AwbcRuntimeValueSnapshotError::Message {
                message: error.to_string(),
            }
        })?;
        let spec = self.spec.into_live(owner)?;
        RuntimeNeedHandle::try_from_snapshot(RuntimeNeedHandleSnapshot(Box::new(
            RuntimeNeedHandleData {
                correlation: self.correlation,
                spec,
                origin: self.origin,
            },
        )))
        .map_err(
            |error| crate::value::AwbcRuntimeValueSnapshotError::Message {
                message: error.to_string(),
            },
        )
    }

    pub(crate) fn request_values(
        &self,
    ) -> impl Iterator<Item = &crate::value::AwbcRuntimeValueSnapshot> {
        self.spec.request.payloads()
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeNeedHandleError {
    #[error("Need descriptor violates its resource contract: {0}")]
    Descriptor(Box<crate::entry::RuntimeSchemaError>),
    #[error("reusable Need handle requires JoinSameKey")]
    ReusableAlwaysStart,
    #[error("handle correlation does not match its task specification")]
    CorrelationMismatch,
    #[error("handle producer contract does not match its task specification")]
    ProducerContractMismatch,
    #[error("handle identity derivation failed: {0}")]
    Identity(#[from] TaskIdentityError),
}

impl RuntimeNeedHandleOrigin {
    fn visit_descriptor<P>(
        self,
        correlation: TaskCorrelation,
        spec: &super::TaskSpec<P>,
        visitor: &mut impl super::descriptor::TaskDescriptorVisitor,
        depth: usize,
    ) -> Result<(), crate::entry::RuntimeSchemaError> {
        visitor.node(depth)?;
        let next = depth + 1;
        for bytes in [
            correlation.task_id.as_bytes(),
            correlation.task_key.as_bytes(),
            correlation.need.as_bytes(),
            correlation.producer_contract.as_bytes(),
        ] {
            visitor.fixed(bytes, next)?;
        }
        visitor.fixed(&correlation.generation.get().to_le_bytes(), next)?;
        visitor.fixed(&correlation.launch_ordinal.get().to_le_bytes(), next)?;
        visitor.fixed(
            &[match self {
                Self::ReusableJoin => 0,
                Self::AcceptedLaunch => 1,
            }],
            next,
        )?;
        spec.visit_descriptor(visitor, next)
    }
    fn validate(
        self,
        correlation: TaskCorrelation,
        generation: super::GenerationId,
        producer: &super::NeedProducerInstance,
        policy: TaskPolicy,
        outcome: &TaskOutcomeContract,
    ) -> Result<(), RuntimeNeedHandleError> {
        if self == Self::ReusableJoin {
            if policy != TaskPolicy::JoinSameKey {
                return Err(RuntimeNeedHandleError::ReusableAlwaysStart);
            }
            if correlation.launch_ordinal != TaskLaunchOrdinal::JOIN {
                return Err(RuntimeNeedHandleError::CorrelationMismatch);
            }
        }
        outcome
            .validate_producer(producer)
            .map_err(|_| RuntimeNeedHandleError::CorrelationMismatch)?;
        if correlation.producer_contract != producer.contract() {
            return Err(RuntimeNeedHandleError::ProducerContractMismatch);
        }
        if TaskCorrelation::try_for(generation, producer, policy, correlation.launch_ordinal)?
            != correlation
        {
            return Err(RuntimeNeedHandleError::CorrelationMismatch);
        }
        Ok(())
    }
}

/// Owned, inert in-memory snapshot. Persistence encoding and program-bound
/// RuntimeValue admission join the public carrier cut; this is not a second
/// wire reader. Moving into/out of it does not clone request argument values.
#[derive(Debug, serde::Deserialize, PartialEq)]
#[serde(transparent)]
pub(super) struct RuntimeNeedHandleSnapshot(Box<RuntimeNeedHandleData>);

impl RuntimeNeedHandle {
    pub(crate) fn visit_descriptor(
        &self,
        visitor: &mut impl super::descriptor::TaskDescriptorVisitor,
        depth: usize,
    ) -> Result<(), crate::entry::RuntimeSchemaError> {
        self.0
            .origin
            .visit_descriptor(self.0.correlation, &self.0.spec, visitor, depth)
    }
    pub(crate) fn view(&self) -> crate::value::RuntimeNeedView<'_> {
        crate::value::RuntimeNeedView::Live(self)
    }
    /// Called with the actual TaskHost admission receipt, never an unlaunched
    /// producer descriptor. Exact derivation is checked before owning the row.
    pub(crate) fn try_from_accepted_launch(
        spec: TaskSpec,
        handle: TaskHandle,
    ) -> Result<Self, RuntimeNeedHandleError> {
        spec.validate_need_descriptor()?;
        RuntimeNeedHandleOrigin::AcceptedLaunch.validate(
            handle.correlation,
            spec.generation,
            &spec.producer,
            spec.policy,
            &spec.outcome,
        )?;
        Ok(Self(Box::new(RuntimeNeedHandleData {
            correlation: handle.correlation,
            spec,
            origin: RuntimeNeedHandleOrigin::AcceptedLaunch,
        })))
    }

    pub const fn correlation(&self) -> TaskCorrelation {
        self.0.correlation
    }

    pub const fn need_id(&self) -> NeedId {
        self.0.correlation.need
    }

    pub const fn outcome(&self) -> &TaskOutcomeContract {
        &self.0.spec.outcome
    }

    pub(crate) const fn spec(&self) -> &TaskSpec {
        &self.0.spec
    }

    pub(crate) fn request_values(&self) -> impl Iterator<Item = &crate::value::RuntimeValue> {
        self.0.spec.request.runtime_values()
    }

    pub const fn reusable_spec(&self) -> Option<&TaskSpec> {
        match self.0.origin {
            RuntimeNeedHandleOrigin::ReusableJoin => Some(&self.0.spec),
            RuntimeNeedHandleOrigin::AcceptedLaunch => None,
        }
    }

    pub(crate) const fn requires_producer_custody(&self) -> bool {
        matches!(self.0.origin, RuntimeNeedHandleOrigin::AcceptedLaunch)
    }

    pub(crate) fn matches_spec(&self, spec: &TaskSpec) -> bool {
        self.0.spec.same_join_contract(spec)
    }

    pub(super) fn into_snapshot(self) -> RuntimeNeedHandleSnapshot {
        RuntimeNeedHandleSnapshot(self.0)
    }

    pub(super) fn try_from_snapshot(
        snapshot: RuntimeNeedHandleSnapshot,
    ) -> Result<Self, RuntimeNeedHandleError> {
        snapshot.0.spec.validate_need_descriptor()?;
        snapshot.0.origin.validate(
            snapshot.0.correlation,
            snapshot.0.spec.generation,
            &snapshot.0.spec.producer,
            snapshot.0.spec.policy,
            &snapshot.0.spec.outcome,
        )?;
        Ok(Self(snapshot.0))
    }
}

/// A reusable Join descriptor does not represent an accepted live launch.
/// Its complete specification must be admitted by the execution owner's journal
/// before starting work; AlwaysStart requires an actual TaskSubmission receipt.
impl TryFrom<TaskSpec> for RuntimeNeedHandle {
    type Error = RuntimeNeedHandleError;

    fn try_from(spec: TaskSpec) -> Result<Self, Self::Error> {
        spec.validate_need_descriptor()?;
        spec.validate_outcome()
            .map_err(|_| RuntimeNeedHandleError::CorrelationMismatch)?;
        if spec.policy != TaskPolicy::JoinSameKey {
            return Err(RuntimeNeedHandleError::ReusableAlwaysStart);
        }
        let correlation = spec.correlation(TaskLaunchOrdinal::JOIN)?;
        Ok(Self(Box::new(RuntimeNeedHandleData {
            correlation,
            spec,
            origin: RuntimeNeedHandleOrigin::ReusableJoin,
        })))
    }
}

impl TryFrom<super::TaskSubmission> for RuntimeNeedHandle {
    type Error = RuntimeNeedHandleError;

    fn try_from(submission: super::TaskSubmission) -> Result<Self, Self::Error> {
        let (spec, handle) = submission.into_parts();
        Self::try_from_accepted_launch(spec, handle)
    }
}

impl<'de> serde::Deserialize<'de> for RuntimeNeedHandle {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let _ = serde::de::IgnoredAny::deserialize(deserializer)?;
        Err(serde::de::Error::custom(
            "live Need handles require program-bound snapshot admission",
        ))
    }
}

#[cfg(test)]
mod tests;
