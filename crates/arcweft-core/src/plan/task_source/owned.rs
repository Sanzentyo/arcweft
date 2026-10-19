//! An owning lease for an original program's actual completed Task slot.

use super::program::{RuntimeTaskExecutionRow, RuntimeTaskSourceBorrowError};
use super::{RuntimeTaskSourceProgram, RuntimeTaskSourceRef};
use crate::plan::{
    RuntimePlan, RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanRef, RuntimeTaskPlanSealLimits,
};
use crate::task::RuntimeProgramOwner;
use crate::task::semantic::TaskSemanticMeter;

/// Retains the genuine source namespace. Neither key assertions nor a metadata
/// launch coordinate can construct this handle. Cloning retains the same Arc.
#[derive(Clone)]
pub struct RuntimeTaskSource {
    program: RuntimeTaskSourceProgram,
    coordinate: RuntimeTaskPlanBuildCoordinate,
    execution: Option<RuntimeTaskExecutionRow>,
}

impl RuntimeTaskSource {
    pub(crate) fn from_borrowed(
        source: &RuntimeTaskSourceRef<'_>,
        program: &RuntimeTaskSourceProgram,
        owner: &RuntimeProgramOwner,
    ) -> Option<Self> {
        if !program.owns_source_for(source, owner) {
            return None;
        }
        let coordinate = source.owner.resolve(source.task.index().0)?;
        Some(Self {
            program: program.clone(),
            coordinate,
            execution: source
                .execution
                .as_ref()
                .map(super::program::RuntimeTaskExecutionRef::row),
        })
    }

    pub fn task_plan(&self) -> RuntimeTaskPlanRef<'_> {
        self.program
            .original()
            .task_plan(&self.coordinate)
            .expect("the original immutable program retains its issued slot")
    }

    pub(crate) fn borrow(
        &self,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeTaskSourceRef<'_>, RuntimeTaskSourceBorrowError> {
        self.program
            .reborrow_source(&self.coordinate, self.execution, limits, meter)
    }

    pub(crate) fn belongs_to(&self, program: &RuntimePlan) -> bool {
        std::ptr::eq(self.program.original(), program)
    }

    pub(crate) fn program_owner(&self) -> RuntimeProgramOwner {
        self.program.program_owner()
    }
}

impl PartialEq for RuntimeTaskSource {
    fn eq(&self, other: &Self) -> bool {
        self.program == other.program
            && self.coordinate == other.coordinate
            && self.execution == other.execution
    }
}
impl Eq for RuntimeTaskSource {}

impl std::fmt::Debug for RuntimeTaskSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeTaskSource")
            .field("key", &self.task_plan().key())
            .finish_non_exhaustive()
    }
}
