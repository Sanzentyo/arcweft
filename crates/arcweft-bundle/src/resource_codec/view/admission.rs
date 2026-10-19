//! Joins an actual executable with the original Source of its accepted View product.

use std::sync::Arc;

use arcweft_core::awbc::{
    product_step::{AwbcProductProgram, AwbcProductStepBuildError},
    schema::AwbcProgram,
    verify::{AwbcVerifyBudget, AwbcVerifyContext},
};
use arcweft_core::plan::RuntimeTaskSourceArtifactLimits;
use arcweft_core::task::{ViewTaskPlanAuthority, ViewTaskPlanValidationError};
use thiserror::Error;

use super::{ValidatedViewProduct, ViewProductValidationLimits};

/// Failure to admit one executable against the actual accepted View product.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ViewProgramAdmissionError {
    #[error("linked View task assertions require their original executable Source")]
    MissingOriginalSource,
    #[error(transparent)]
    TaskBinding(#[from] ViewTaskPlanValidationError),
    #[error(transparent)]
    Executable(#[from] AwbcProductStepBuildError),
}

impl ValidatedViewProduct {
    /// Admits code and its complete original Source through this exact product.
    /// A genuinely Source-free executable grants no original Task authority.
    pub fn admit_awbc_program(
        &self,
        awbc: Arc<AwbcProgram>,
    ) -> Result<AwbcProductProgram, ViewProgramAdmissionError> {
        let authority = if awbc.original_source.is_some() {
            self.program()
                .map(|program| {
                    program.task_source_authority(ViewProductValidationLimits::default())
                })
                .transpose()?
        } else {
            if self
                .program()
                .is_some_and(|program| !program.resource().task_plan_bindings.is_empty())
            {
                return Err(ViewProgramAdmissionError::MissingOriginalSource);
            }
            None
        };
        Ok(AwbcProductProgram::admit(
            awbc,
            AwbcVerifyBudget::default(),
            AwbcVerifyContext {
                require_entrypoint: false,
                ..Default::default()
            },
            RuntimeTaskSourceArtifactLimits::default(),
            authority
                .as_ref()
                .map(|authority| authority as &dyn ViewTaskPlanAuthority),
        )?)
    }
}
