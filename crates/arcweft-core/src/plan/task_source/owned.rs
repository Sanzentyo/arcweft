//! An owning lease for an original program's actual completed Task slot.

use super::{RuntimeTaskSourceError, RuntimeTaskSourceRef};
use crate::plan::{
    RuntimePlan, RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanRef, RuntimeTaskPlanSealLimits,
};
use crate::task::RuntimeProgramOwner;
use crate::task::semantic::TaskSemanticMeter;
use std::sync::Arc;

/// Retains the genuine source namespace. Neither key assertions nor a metadata
/// launch coordinate can construct this handle. Cloning retains the same Arc.
#[derive(Clone)]
pub(crate) struct RuntimeTaskSource {
    program: Arc<RuntimePlan>,
    coordinate: RuntimeTaskPlanBuildCoordinate,
}

impl RuntimeTaskSource {
    pub(crate) fn from_borrowed(
        source: &RuntimeTaskSourceRef<'_>,
        owner: &RuntimeProgramOwner,
    ) -> Option<Self> {
        let RuntimeProgramOwner::Plan(program) = owner else {
            return None;
        };
        if !source.task.belongs_to(program) || !source.request.belongs_to(program) {
            return None;
        }
        let coordinate = source.owner.resolve(source.task.index().0)?;
        Some(Self {
            program: Arc::clone(program),
            coordinate,
        })
    }

    pub(crate) fn task_plan(&self) -> RuntimeTaskPlanRef<'_> {
        self.program
            .task_plan(&self.coordinate)
            .expect("the original immutable program retains its issued slot")
    }

    pub(crate) fn borrow(
        &self,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeTaskSourceRef<'_>, RuntimeTaskSourceError> {
        self.program.task_source(&self.coordinate, limits, meter)
    }

    pub(crate) fn belongs_to(&self, program: &RuntimePlan) -> bool {
        std::ptr::eq(self.program.as_ref(), program)
    }
}

impl PartialEq for RuntimeTaskSource {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.program, &other.program) && self.coordinate == other.coordinate
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
