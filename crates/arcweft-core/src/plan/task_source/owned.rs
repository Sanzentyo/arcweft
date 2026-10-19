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

/// Inert locator for an already completed Source and its exact execution row.
/// Decoding this value grants no Source lease or producer authority.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeTaskSourceAssertion {
    pub(in crate::plan::task_source) expected: crate::task::ExpectedTaskPlanKey,
    pub(in crate::plan::task_source) execution: RuntimeTaskSourceExecutionAssertion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(in crate::plan::task_source) enum RuntimeTaskSourceExecutionAssertion {
    Native,
    Awbc(RuntimeTaskExecutionRow),
}

impl RuntimeTaskSource {
    pub(crate) fn assertion(&self) -> RuntimeTaskSourceAssertion {
        RuntimeTaskSourceAssertion {
            expected: self.task_plan().key().into(),
            execution: match self.execution {
                Some(row) => RuntimeTaskSourceExecutionAssertion::Awbc(row),
                None => RuntimeTaskSourceExecutionAssertion::Native,
            },
        }
    }
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

    pub(crate) fn program(&self) -> &RuntimeTaskSourceProgram {
        &self.program
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
