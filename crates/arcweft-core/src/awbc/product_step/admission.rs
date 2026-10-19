//! One immutable executable and its admitted original Task Source.

use std::sync::Arc;

use crate::awbc::schema::AwbcProgram;
use crate::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext};
use crate::plan::task_source::RuntimeTaskSourceProgram;
use crate::plan::{RuntimeTaskSourceArtifactLimits, ViewTaskPlanAuthority};

use super::AwbcProductStepBuildError;

/// An actual AWBC program admitted with the upper authorities its original
/// Source requires. Clones retain the same immutable executable and Source;
/// invocation, rollback and restore do not reconstruct either owner.
#[derive(Clone, Debug, PartialEq)]
pub struct AwbcProductProgram {
    pub(super) program: Arc<AwbcProgram>,
    pub(super) task_source: Option<RuntimeTaskSourceProgram>,
}

impl AwbcProductProgram {
    pub fn admit(
        program: Arc<AwbcProgram>,
        verify: AwbcVerifyBudget,
        context: AwbcVerifyContext<'_>,
        source: RuntimeTaskSourceArtifactLimits,
        authority: Option<&dyn ViewTaskPlanAuthority>,
    ) -> Result<Self, AwbcProductStepBuildError> {
        let task_source = RuntimeTaskSourceProgram::prepare_awbc(
            Arc::clone(&program),
            verify,
            context,
            source,
            authority,
        )
        .map_err(|error| AwbcProductStepBuildError::InvalidProgram {
            message: error.to_string(),
        })?;
        Ok(Self {
            program,
            task_source,
        })
    }

    /// Retains the actual executable's type authority for host value admission.
    pub fn program_arc(&self) -> Arc<AwbcProgram> {
        Arc::clone(&self.program)
    }
}

impl std::ops::Deref for AwbcProductProgram {
    type Target = AwbcProgram;
    fn deref(&self) -> &Self::Target {
        &self.program
    }
}
