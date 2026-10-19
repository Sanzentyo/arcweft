//! A completed Task and its actual original function/request source.
//! This boundary borrows the immutable program and never accepts a key assertion.

use super::body_semantic::function::OriginalRequestEndpoint;
use super::body_semantic::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use super::{
    RuntimePlan, RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanCoordinateOwner,
    RuntimeTaskPlanRef, RuntimeTaskPlanSealLimits, RuntimeTaskRequestSource,
};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};

#[derive(Debug, thiserror::Error)]
pub(crate) enum RuntimeTaskSourceError {
    #[error("Task source coordinate does not belong to this completed program")]
    ForeignCoordinate,
    #[error("completed Task source has no request endpoint {request:?}")]
    MissingEndpoint { request: RuntimeTaskRequestSource },
    #[error("completed Task request endpoint names another original Task slot")]
    EndpointCoordinateMismatch,
    #[error("original Task function admission failed: {0}")]
    Function(#[from] RuntimeBodySemanticError),
}

/// The row is already sealed. Request metadata borrows that same immutable
/// program. No digest is recomputed and no second Task table exists.
pub(crate) struct RuntimeTaskSourceRef<'program> {
    task: RuntimeTaskPlanRef<'program>,
    request: OriginalRequestEndpoint<'program>,
}

impl RuntimePlan {
    pub(crate) fn task_source(
        &self,
        coordinate: &RuntimeTaskPlanBuildCoordinate,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeTaskSourceRef<'_>, RuntimeTaskSourceError> {
        meter
            .constrain(limits.max_semantic_work, limits.max_transcript_bytes)
            .map_err(RuntimeBodySemanticError::from)?;
        let result = (|| {
            let task = self
                .task_plan(coordinate)
                .ok_or(RuntimeTaskSourceError::ForeignCoordinate)?;
            let owner = RuntimeTaskPlanCoordinateOwner::from_completed(coordinate, task)
                .ok_or(RuntimeTaskSourceError::ForeignCoordinate)?;
            let request = RuntimeBodySemanticContext::new(&self.inventory)
                .original_request_endpoint(
                    task.definition().producer_function(),
                    task.definition().request_template().source(),
                    &owner,
                    meter,
                    limits,
                )?
                .ok_or(RuntimeTaskSourceError::MissingEndpoint {
                    request: task.definition().request_template().source(),
                })?;
            if request
                .task_source()
                .is_some_and(|source| source.coordinate() != coordinate)
            {
                return Err(RuntimeTaskSourceError::EndpointCoordinateMismatch);
            }
            Ok(RuntimeTaskSourceRef { task, request })
        })();
        result.map_err(|error| {
            let cause = meter.status().err();
            meter.reject_owner();
            match cause {
                Some(cause) if cause != TaskSemanticEncodingError::OwnerRejected => {
                    RuntimeTaskSourceError::Function(cause.into())
                }
                _ => error,
            }
        })
    }
}

impl RuntimeTaskSourceRef<'_> {
    pub(crate) const fn task_plan(&self) -> RuntimeTaskPlanRef<'_> {
        self.task
    }

    pub(crate) const fn request_endpoint(&self) -> OriginalRequestEndpoint<'_> {
        self.request
    }
}

#[cfg(test)]
mod tests;
