//! Count-only task-image preflight, before semantic children or memo allocation.

use super::{RuntimeTaskPlanImageError, UnsealedRuntimePlanImage};
use crate::plan::RuntimeTaskPlanSealLimits;
use crate::plan::body_semantic::executable_rows::RuntimeExecutableSemanticRows;
use crate::plan::body_semantic::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::task::semantic::TaskSemanticMeter;

impl UnsealedRuntimePlanImage {
    /// First two global limit fields include the inline task table. This method
    /// neither resolves references nor creates intermediate semantic proofs.
    pub(super) fn preflight_rows(
        &self,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), RuntimeTaskPlanImageError> {
        let tasks = meter.checked_count_sum(self.task_plans.len(), 0)?;
        if tasks > limits.max_task_plan_rows as usize {
            meter.reject_owner();
            return Err(RuntimeTaskPlanImageError::TaskRows {
                actual: tasks,
                maximum: limits.max_task_plan_rows,
            });
        }
        let mut actual = tasks;
        for count in RuntimeExecutableSemanticRows::table_counts(&self.inventory) {
            actual = meter.checked_count_sum(actual, count)?;
        }
        if actual > limits.max_executable_rows as usize {
            meter.reject_owner();
            return Err(RuntimeTaskPlanImageError::Body(
                RuntimeBodySemanticError::ExecutableRows {
                    actual,
                    maximum: limits.max_executable_rows,
                },
            ));
        }
        Ok(())
    }

    /// Count function roles across the actual inventory, then static request
    /// roles across the source task rows. Called after the common child-count
    /// stage; bad type/endpoint references are semantic completion errors.
    pub(super) fn preflight_roles(
        &self,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), RuntimeTaskPlanImageError> {
        for function in self.inventory.function_sites().iter() {
            let endpoints = RuntimeBodySemanticContext::endpoint_count(
                function.body(),
                function.inputs().len(),
                limits.max_function_roles,
            )
            .inspect_err(|_| meter.reject_owner())?;
            let actual = meter.checked_count_sum(function.inputs().len(), endpoints)?;
            if actual > limits.max_function_roles as usize {
                meter.reject_owner();
                return Err(RuntimeTaskPlanImageError::Body(
                    RuntimeBodySemanticError::FunctionRoles {
                        actual,
                        maximum: limits.max_function_roles,
                    },
                ));
            }
        }
        for task in &self.task_plans {
            let actual = task.request_template.role_count(meter)?;
            if actual > limits.max_request_roles as usize {
                meter.reject_owner();
                return Err(RuntimeTaskPlanImageError::Body(
                    RuntimeBodySemanticError::RequestRoles {
                        actual,
                        maximum: limits.max_request_roles,
                    },
                ));
            }
        }
        Ok(())
    }
}
