//! Count-only task-image preflight, before semantic children or memo allocation.

mod children;

use super::{RuntimeTaskPlanImageError, UnsealedRuntimePlanImage};
use crate::plan::RuntimeTaskPlanSealLimits;
use crate::plan::body_semantic::executable_rows::RuntimeExecutableSemanticRows;
use crate::plan::body_semantic::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::task::semantic::TaskSemanticMeter;

impl UnsealedRuntimePlanImage {
    /// All eight preflight fields run before structural resolution or child
    /// transcript completion. Success owns the same reachable C pass; it
    /// exposes no completed executable/task proof.
    pub(in crate::plan::body_semantic) fn preflight<'a>(
        &'a self,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<
        crate::plan::control_effect::semantic::RuntimeControlEffectSemanticPass<'a>,
        RuntimeTaskPlanImageError,
    > {
        use crate::plan::control_effect::semantic::RuntimeControlEffectPreflight;
        (|| {
            self.preflight_rows(limits, meter)?;
            self.preflight_children(limits, meter)?;
            let control = RuntimeControlEffectPreflight::new(
                self.inventory.control_effect_contracts(),
                self.inventory.type_table(),
                self.task_plans.iter().map(|row| row.control_effect),
                meter,
            )?;
            control.check_children(limits, meter)?;
            self.preflight_roles(limits, meter)?;
            control.check_effects(limits, meter)?;
            let views = self
                .task_plans
                .iter()
                .filter(|row| matches!(row.binding, super::RuntimeTaskSemanticBinding::View))
                .count();
            let views = meter.checked_count_sum(views, 0)?;
            if views > limits.max_view_bindings as usize {
                meter.reject_owner();
                return Err(RuntimeTaskPlanImageError::ViewRows {
                    actual: views,
                    maximum: limits.max_view_bindings,
                });
            }
            let known = self
                .known_transcript_bytes(meter)?
                .checked_add(control.known_transcript_bytes(meter)?)
                .ok_or_else(|| {
                    meter.reject_owner();
                    crate::task::semantic::TaskSemanticEncodingError::ArithmeticOverflow
                })?;
            meter.preflight_bytes(known)?;
            control.finish(limits, meter).map_err(Into::into)
        })()
        .inspect_err(|_| meter.reject_owner())
    }

    /// Known complete Q and C shapes plus conservative fixed E/F/task bytes.
    /// Unresolved dynamic owner strings/paths add bytes later under the same
    /// meter. Assertions, wire layout and cache/index bytes never enter this.
    fn known_transcript_bytes(
        &self,
        meter: &mut TaskSemanticMeter,
    ) -> Result<u64, RuntimeTaskPlanImageError> {
        use crate::task::semantic::TaskSemanticEncodingError::ArithmeticOverflow;
        let mut bytes = b"arcweft.runtime-plan.executable-semantic.v1\0".len() as u64 + 2 + 15 * 5;
        let mut add = |count: u64| {
            bytes = bytes.checked_add(count).ok_or_else(|| {
                meter.reject_owner();
                ArithmeticOverflow
            })?;
            Ok::<(), RuntimeTaskPlanImageError>(())
        };
        for count in RuntimeExecutableSemanticRows::table_counts(&self.inventory) {
            let count = u64::try_from(count).map_err(|_| ArithmeticOverflow)?;
            let row_min = 37 + b"arcweft.runtime-plan.executable-row.v1\0".len() as u64 + 2;
            add(count.checked_mul(row_min).ok_or(ArithmeticOverflow)?)?;
        }
        let mut functions = std::collections::BTreeSet::new();
        for row in &self.task_plans {
            let binding = u64::from(matches!(
                row.binding,
                super::RuntimeTaskSemanticBinding::Timeout { .. }
                    | super::RuntimeTaskSemanticBinding::Line { .. }
            )) * 32;
            add(108 + binding)?; // inline E14 ordinal/kind/base/binding
            add(row.request_template.known_transcript_bytes()?)?;
            add(b"arcweft.task.plan-semantic.v1\0".len() as u64 + 132 + binding)?;
            if functions.insert(row.producer_function) {
                add(b"arcweft.runtime-plan.producer-function-semantic.v1\0".len() as u64 + 109)?;
            }
        }
        Ok(bytes)
    }

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
