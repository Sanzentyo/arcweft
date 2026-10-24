//! Original Task Source and the exact executable used for value admission.
//! Pair construction alone does not certify AWBC row/source assertions.

use super::{
    RuntimeTaskSourceArtifactError, RuntimeTaskSourceArtifactLimits, RuntimeTaskSourceRef,
};
use crate::awbc::schema::AwbcProgram;
use crate::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext, AwbcVerifyError};
use crate::plan::{RuntimePlan, ViewTaskPlanAuthority};
use crate::task::RuntimeProgramOwner;
use std::sync::Arc;

mod execution;
pub(super) use execution::{
    RuntimeTaskExecutionRef, RuntimeTaskExecutionRow, RuntimeTaskSourceBorrowError,
};
mod host;
#[cfg(test)]
pub(super) use host::tests::executable as host_executable_fixture;
pub(crate) use host::{RuntimeAwbcHostSourceError, RuntimeAwbcHostSourceField};
mod program_task;
#[cfg(test)]
pub(crate) use program_task::tests::executable as program_task_executable_fixture;
mod selected;
pub(crate) use program_task::{RuntimeAwbcProgramSourceError, RuntimeAwbcProgramSourceField};
mod template;
pub(crate) use selected::{RuntimeAwbcSelectedSourceError, RuntimeAwbcSelectedSourceField};
#[cfg(test)]
pub(super) use template::tests::{
    executable as template_executable_fixture, original as template_original_fixture,
};
mod types;
pub(crate) use template::{RuntimeAwbcTemplateSourceError, RuntimeAwbcTemplateSourceField};

#[cfg(test)]
mod tests;

/// Retains complete admitted original Source and its actual execution lease.
/// Neither decoded assertions nor request metadata construct this context.
#[derive(Clone)]
pub(crate) struct RuntimeTaskSourceProgram {
    data: Arc<RuntimeTaskSourceProgramData>,
}

enum RuntimeTaskSourceProgramData {
    Native(Arc<RuntimePlan>),
    Awbc {
        executable: Arc<AwbcProgram>,
        original: Arc<RuntimePlan>,
    },
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum RuntimeTaskSourceProgramError {
    #[error("AWBC has no original Task Source")]
    MissingOriginalSource,
    #[error(transparent)]
    Awbc(#[from] AwbcVerifyError),
    #[error(transparent)]
    Source(#[from] RuntimeTaskSourceArtifactError),
}

impl RuntimeTaskSourceProgram {
    pub(crate) fn native(program: Arc<RuntimePlan>) -> Self {
        Self {
            data: Arc::new(RuntimeTaskSourceProgramData::Native(program)),
        }
    }

    /// Verifies this actual immutable code lease and admits only the original
    /// Source it carries. Code task-row assertions still require their own
    /// original-row admission before producer publication.
    pub(crate) fn admit_awbc(
        executable: Arc<AwbcProgram>,
        verify: AwbcVerifyBudget,
        context: AwbcVerifyContext<'_>,
        source: RuntimeTaskSourceArtifactLimits,
        authority: Option<&dyn ViewTaskPlanAuthority>,
    ) -> Result<Self, RuntimeTaskSourceProgramError> {
        Self::prepare_awbc(executable, verify, context, source, authority)?
            .ok_or(RuntimeTaskSourceProgramError::MissingOriginalSource)
    }

    /// The unadmitted code schema permits Source-free autonomous executables.
    /// Absence grants no selected/original authority and has no fallback reader.
    pub(crate) fn prepare_awbc(
        executable: Arc<AwbcProgram>,
        verify: AwbcVerifyBudget,
        context: AwbcVerifyContext<'_>,
        source: RuntimeTaskSourceArtifactLimits,
        authority: Option<&dyn ViewTaskPlanAuthority>,
    ) -> Result<Option<Self>, RuntimeTaskSourceProgramError> {
        executable.verify(verify, context)?;
        let Some(original) = executable.original_source.as_ref() else {
            return Ok(None);
        };
        let original = original.admit(source, authority)?;
        Ok(Some(Self {
            data: Arc::new(RuntimeTaskSourceProgramData::Awbc {
                executable,
                original: Arc::new(original),
            }),
        }))
    }

    pub(crate) fn original(&self) -> &RuntimePlan {
        match self.data.as_ref() {
            RuntimeTaskSourceProgramData::Native(program) => program,
            RuntimeTaskSourceProgramData::Awbc { original, .. } => original,
        }
    }

    /// Owns the actual backend's types, never a Native surrogate for AWBC.
    pub(crate) fn program_owner(&self) -> RuntimeProgramOwner {
        match self.data.as_ref() {
            RuntimeTaskSourceProgramData::Native(program) => {
                RuntimeProgramOwner::Plan(Arc::clone(program))
            }
            RuntimeTaskSourceProgramData::Awbc { executable, .. } => {
                RuntimeProgramOwner::Awbc(Arc::clone(executable))
            }
        }
    }

    /// Checks both authorities before a value is projected or a lease copied.
    pub(crate) fn owns_source_for(
        &self,
        source: &RuntimeTaskSourceRef<'_>,
        owner: &RuntimeProgramOwner,
    ) -> bool {
        let same_executable = match (self.data.as_ref(), owner) {
            (RuntimeTaskSourceProgramData::Native(program), RuntimeProgramOwner::Plan(owner)) => {
                Arc::ptr_eq(program, owner) && source.execution.is_none()
            }
            (
                RuntimeTaskSourceProgramData::Awbc { executable, .. },
                RuntimeProgramOwner::Awbc(owner),
            ) => Arc::ptr_eq(executable, owner) && self.owns_execution(source),
            _ => false,
        };
        same_executable
            && source.task_plan().belongs_to(self.original())
            && source.request_endpoint().belongs_to(self.original())
    }
}

impl PartialEq for RuntimeTaskSourceProgram {
    fn eq(&self, other: &Self) -> bool {
        match (self.data.as_ref(), other.data.as_ref()) {
            (
                RuntimeTaskSourceProgramData::Native(left),
                RuntimeTaskSourceProgramData::Native(right),
            ) => Arc::ptr_eq(left, right),
            _ => Arc::ptr_eq(&self.data, &other.data),
        }
    }
}

impl Eq for RuntimeTaskSourceProgram {}

impl std::fmt::Debug for RuntimeTaskSourceProgram {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeTaskSourceProgram")
            .field("executable", &self.original().executable_semantic_digest())
            .finish_non_exhaustive()
    }
}
