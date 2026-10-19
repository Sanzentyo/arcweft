//! A checked row binding belongs to one immutable original/executable pair.

use super::{
    RuntimeAwbcHostSourceError, RuntimeAwbcSelectedSourceError, RuntimeAwbcTemplateSourceError,
};
use super::{RuntimeTaskSourceProgram, RuntimeTaskSourceProgramData};
use crate::awbc::schema::{AwbcHostCallId, AwbcTaskPlanId};
use crate::plan::task_source::{RuntimeTaskSourceError, RuntimeTaskSourceRef};
use crate::plan::{RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanSealLimits};
use crate::task::semantic::TaskSemanticMeter;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::plan::task_source) enum RuntimeTaskExecutionRow {
    HostCall(AwbcHostCallId),
    Selected(AwbcTaskPlanId),
    Template(AwbcTaskPlanId),
}

/// No constructor is available outside this original-program owner. A Source
/// lookup alone does not certify an AWBC row or retain this capability.
pub(in crate::plan::task_source) struct RuntimeTaskExecutionRef<'program> {
    program: &'program RuntimeTaskSourceProgramData,
    row: RuntimeTaskExecutionRow,
}

impl RuntimeTaskExecutionRef<'_> {
    pub(in crate::plan::task_source) const fn row(&self) -> RuntimeTaskExecutionRow {
        self.row
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum RuntimeTaskSourceBorrowError {
    #[error(transparent)]
    Source(#[from] RuntimeTaskSourceError),
    #[error(transparent)]
    Host(#[from] RuntimeAwbcHostSourceError),
    #[error(transparent)]
    Selected(#[from] RuntimeAwbcSelectedSourceError),
    #[error(transparent)]
    Template(#[from] RuntimeAwbcTemplateSourceError),
}

impl RuntimeTaskSourceProgram {
    pub(super) fn bind_source<'program>(
        &'program self,
        mut source: RuntimeTaskSourceRef<'program>,
        row: RuntimeTaskExecutionRow,
    ) -> RuntimeTaskSourceRef<'program> {
        source.execution = Some(RuntimeTaskExecutionRef {
            program: self.data.as_ref(),
            row,
        });
        source
    }

    pub(super) fn owns_execution(&self, source: &RuntimeTaskSourceRef<'_>) -> bool {
        source
            .execution
            .as_ref()
            .is_some_and(|binding| std::ptr::eq(binding.program, self.data.as_ref()))
    }

    /// The immutable owning handle retains the code-local row it actually
    /// admitted. Reborrowing never falls back to an unbound Source lookup and
    /// applies the new caller's complete allowance to the same row again.
    pub(in crate::plan::task_source) fn reborrow_source(
        &self,
        coordinate: &RuntimeTaskPlanBuildCoordinate,
        execution: Option<RuntimeTaskExecutionRow>,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeTaskSourceRef<'_>, RuntimeTaskSourceBorrowError> {
        let result = (|| {
            let source = match execution {
                None => self.original().task_source(coordinate, limits, meter)?,
                Some(RuntimeTaskExecutionRow::HostCall(call)) => {
                    self.awbc_host_source(call, limits, meter)?
                }
                Some(RuntimeTaskExecutionRow::Template(task)) => {
                    self.awbc_template_source(task, limits, meter)?
                }
                Some(RuntimeTaskExecutionRow::Selected(task)) => {
                    let source = self.original().task_source(coordinate, limits, meter)?;
                    self.awbc_selected_source(task, source, limits, meter)?
                }
            };
            if self.original().task_plan(coordinate) != Some(source.task_plan()) {
                return Err(RuntimeTaskSourceError::ForeignRow.into());
            }
            Ok(source)
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }
}
