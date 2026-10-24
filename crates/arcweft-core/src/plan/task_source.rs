//! A completed Task and its actual original function/request source.
//! This boundary borrows the immutable program. Assertions only resolve rows
//! already issued by its original namespace; they never grant Source authority.

mod artifact;
mod owned;
mod program;
#[cfg(test)]
pub(crate) use program::program_task_executable_fixture;
mod program_body;
pub use artifact::{
    RuntimeTaskSourceArtifact, RuntimeTaskSourceArtifactError, RuntimeTaskSourceArtifactLimits,
};
pub(crate) use program::{
    RuntimeAwbcHostSourceError, RuntimeAwbcHostSourceField, RuntimeAwbcProgramSourceError,
    RuntimeAwbcSelectedSourceError, RuntimeAwbcSelectedSourceField, RuntimeAwbcTemplateSourceError,
    RuntimeAwbcTemplateSourceField, RuntimeTaskSourceProgram, RuntimeTaskSourceProgramError,
};
pub use program_body::{RuntimeProgramTaskBodyRef, RuntimeTaskSourceReadContext};
#[cfg(test)]
mod host_tests;
#[cfg(test)]
mod producer_tests;
#[cfg(test)]
mod template_tests;
pub use owned::{RuntimeTaskSource, RuntimeTaskSourceAssertion};

use super::body_semantic::function::OriginalRequestEndpoint;
use super::body_semantic::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use super::{
    RuntimePlan, RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanCoordinateOwner,
    RuntimeTaskPlanRef, RuntimeTaskPlanSealLimits, RuntimeTaskRequestSource,
};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};

#[derive(Debug, thiserror::Error)]
pub enum RuntimeTaskSourceError {
    #[error("Task source coordinate does not belong to this completed program")]
    ForeignCoordinate,
    #[error("Task source row does not belong to this completed program")]
    ForeignRow,
    #[error("Task Source execution assertion belongs to another backend")]
    ForeignExecution,
    #[error("Task source expected key does not resolve in this completed program")]
    MissingExpectedKey,
    #[error("Task source is not an original Thread body")]
    NotProgramBody,
    #[error("completed Task source has no request endpoint {request:?}")]
    MissingEndpoint { request: RuntimeTaskRequestSource },
    #[error("original Task function admission failed: {0}")]
    Function(#[from] RuntimeBodySemanticError),
}

/// The row is already sealed. Request metadata borrows that same immutable
/// program and may be shared. The authenticated row retains plan identity and
/// class independently of a launch coordinate inside that metadata.
/// No digest is recomputed and no second Task table exists.
pub(crate) struct RuntimeTaskSourceRef<'program> {
    task: RuntimeTaskPlanRef<'program>,
    request: OriginalRequestEndpoint<'program>,
    owner: &'program RuntimeTaskPlanCoordinateOwner,
    execution: Option<program::RuntimeTaskExecutionRef<'program>>,
}

impl RuntimePlan {
    pub(crate) fn task_source(
        &self,
        coordinate: &RuntimeTaskPlanBuildCoordinate,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeTaskSourceRef<'_>, RuntimeTaskSourceError> {
        self.task_source_for_row(
            self.task_plan(coordinate)
                .ok_or(RuntimeTaskSourceError::ForeignCoordinate),
            limits,
            meter,
        )
    }

    /// Assertion bytes select only a row already issued by this exact plan.
    /// Its original namespace is retained by the common seal, never recovered
    /// from request metadata, another program or a caller-supplied coordinate.
    pub(crate) fn task_source_by_expected_key(
        &self,
        expected: &crate::task::ExpectedTaskPlanKey,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeTaskSourceRef<'_>, RuntimeTaskSourceError> {
        self.task_source_for_row(
            self.task_plans()
                .by_expected_key(expected)
                .ok_or(RuntimeTaskSourceError::MissingExpectedKey),
            limits,
            meter,
        )
    }

    fn task_source_for_row<'program>(
        &'program self,
        task: Result<RuntimeTaskPlanRef<'program>, RuntimeTaskSourceError>,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeTaskSourceRef<'program>, RuntimeTaskSourceError> {
        meter
            .constrain(limits.max_semantic_work, limits.max_transcript_bytes)
            .map_err(RuntimeBodySemanticError::from)?;
        let result = (|| {
            let task = task?;
            if !task.belongs_to(self) {
                return Err(RuntimeTaskSourceError::ForeignRow);
            }
            let owner = &self.task_coordinates;
            let request = RuntimeBodySemanticContext::new(&self.inventory)
                .original_request_endpoint(
                    task.definition().producer_function(),
                    task.definition().request_template().source(),
                    owner,
                    meter,
                    limits,
                )?
                .ok_or(RuntimeTaskSourceError::MissingEndpoint {
                    request: task.definition().request_template().source(),
                })?;
            Ok(RuntimeTaskSourceRef {
                task,
                request,
                owner,
                execution: None,
            })
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
