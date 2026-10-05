//! Exact reusable-join or accepted-launch handle authority. Kept private with
//! the final specification until task, value, host and persistence consumers
//! switch together. Metadata Clone is never language-level Copy evidence.

use super::identity::{NeedId, TaskCorrelation};
use super::specification::{TaskHandle, TaskSpec};
use super::{TaskIdentityError, TaskLaunchOrdinal, TaskOutcomeContract, TaskPolicy};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RuntimeNeedHandle {
    correlation: TaskCorrelation,
    spec: Box<TaskSpec>,
    origin: RuntimeNeedHandleOrigin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeNeedHandleOrigin {
    ReusableJoin,
    AcceptedLaunch,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(super) enum RuntimeNeedHandleError {
    #[error("reusable Need handle requires JoinSameKey")]
    ReusableAlwaysStart,
    #[error("handle correlation does not match its task specification")]
    CorrelationMismatch,
    #[error("handle producer contract does not match its task specification")]
    ProducerContractMismatch,
    #[error("handle identity derivation failed: {0}")]
    Identity(#[from] TaskIdentityError),
}

/// Owned, inert in-memory snapshot. Persistence encoding and program-bound
/// RuntimeValue admission join the public carrier cut; this is not a second
/// wire reader. Moving into/out of it does not clone request argument values.
#[derive(Debug, PartialEq)]
pub(super) struct RuntimeNeedHandleSnapshot {
    correlation: TaskCorrelation,
    spec: Box<TaskSpec>,
    origin: RuntimeNeedHandleOrigin,
}

impl RuntimeNeedHandle {
    pub(super) fn try_reusable_join(spec: TaskSpec) -> Result<Self, RuntimeNeedHandleError> {
        if spec.policy != TaskPolicy::JoinSameKey {
            return Err(RuntimeNeedHandleError::ReusableAlwaysStart);
        }
        let correlation = spec.correlation(TaskLaunchOrdinal::JOIN)?;
        Ok(Self {
            correlation,
            spec: Box::new(spec),
            origin: RuntimeNeedHandleOrigin::ReusableJoin,
        })
    }

    /// Called with the actual TaskHost admission receipt, never an unlaunched
    /// producer descriptor. Exact derivation is checked before owning the row.
    pub(super) fn try_from_accepted_launch(
        spec: TaskSpec,
        handle: TaskHandle,
    ) -> Result<Self, RuntimeNeedHandleError> {
        Self::validate(&spec, handle.correlation)?;
        Ok(Self {
            correlation: handle.correlation,
            spec: Box::new(spec),
            origin: RuntimeNeedHandleOrigin::AcceptedLaunch,
        })
    }

    fn validate(
        spec: &TaskSpec,
        correlation: TaskCorrelation,
    ) -> Result<(), RuntimeNeedHandleError> {
        if correlation.producer_contract != spec.producer.contract() {
            return Err(RuntimeNeedHandleError::ProducerContractMismatch);
        }
        let expected = spec.correlation(correlation.launch_ordinal)?;
        if expected != correlation {
            return Err(RuntimeNeedHandleError::CorrelationMismatch);
        }
        Ok(())
    }

    pub(super) const fn correlation(&self) -> TaskCorrelation {
        self.correlation
    }

    pub(super) const fn need_id(&self) -> NeedId {
        self.correlation.need
    }

    pub(super) const fn outcome(&self) -> &TaskOutcomeContract {
        &self.spec.outcome
    }

    pub(super) const fn reusable_spec(&self) -> Option<&TaskSpec> {
        match self.origin {
            RuntimeNeedHandleOrigin::ReusableJoin => Some(&self.spec),
            RuntimeNeedHandleOrigin::AcceptedLaunch => None,
        }
    }

    pub(super) fn into_snapshot(self) -> RuntimeNeedHandleSnapshot {
        RuntimeNeedHandleSnapshot {
            correlation: self.correlation,
            spec: self.spec,
            origin: self.origin,
        }
    }

    pub(super) fn try_from_snapshot(
        snapshot: RuntimeNeedHandleSnapshot,
    ) -> Result<Self, RuntimeNeedHandleError> {
        match snapshot.origin {
            RuntimeNeedHandleOrigin::ReusableJoin => {
                if snapshot.spec.policy != TaskPolicy::JoinSameKey {
                    return Err(RuntimeNeedHandleError::ReusableAlwaysStart);
                }
                if snapshot.correlation.launch_ordinal != TaskLaunchOrdinal::JOIN {
                    return Err(RuntimeNeedHandleError::CorrelationMismatch);
                }
            }
            RuntimeNeedHandleOrigin::AcceptedLaunch => {}
        }
        Self::validate(&snapshot.spec, snapshot.correlation)?;
        Ok(Self {
            correlation: snapshot.correlation,
            spec: snapshot.spec,
            origin: snapshot.origin,
        })
    }
}

#[cfg(test)]
mod tests;
