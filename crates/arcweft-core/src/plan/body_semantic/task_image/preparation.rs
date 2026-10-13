//! One borrowed preparation pass for E0..E14 and its actual F/Q/C children.
//! Expected codec assertions never enter this pass; public sealing remains
//! behind the final task/View ownership and codec switch.

use super::{
    RuntimeTaskPlan, RuntimeTaskPlanImageError, RuntimeTaskSemanticBinding,
    UnsealedRuntimePlanImage,
};
use crate::line_task::semantic::LinePlanSemanticDigest;
use crate::plan::body_semantic::executable_rows::RuntimeExecutableSemanticRows;
use crate::plan::body_semantic::flow::RuntimeBodyTaskSource;
use crate::plan::body_semantic::function::ProducerFunctionSemanticDigest;
use crate::plan::body_semantic::request::TaskRequestTemplateDigest;
use crate::plan::body_semantic::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::plan::{
    ControlEffectContractDigest, RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanSealLimits,
};
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticMeter};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeExecutableSemanticDigest([u8; 32]);

/// Completed binding evidence is derived from the same source row. Line
/// retains its actual group proof; no raw digest can issue this evidence.
enum TaskBindingSemantic<'a> {
    Marker(&'a RuntimeTaskSemanticBinding),
    Line(LinePlanSemanticDigest),
}

impl TaskBindingSemantic<'_> {
    fn write(&self, encoder: &mut TaskSemanticEncoder<'_>) {
        match self {
            Self::Marker(binding) => {
                encoder.tag(binding.semantic_tag());
                if let RuntimeTaskSemanticBinding::Timeout { contract } = binding {
                    encoder.digest(contract.as_bytes());
                }
            }
            Self::Line(digest) => {
                encoder.tag(5);
                encoder.digest(digest.as_bytes());
            }
        }
    }
}

/// Child proofs borrow the sole candidate definitions, rather than copying a
/// parallel task inventory or supplying completed keys to executable bodies.
struct TaskSemanticChildren<'a> {
    row: &'a RuntimeTaskPlan,
    function: ProducerFunctionSemanticDigest,
    request: TaskRequestTemplateDigest,
    control: ControlEffectContractDigest,
    binding: TaskBindingSemantic<'a>,
}

impl TaskSemanticChildren<'_> {
    fn write_base(&self, encoder: &mut TaskSemanticEncoder<'_>) {
        encoder.digest(self.function.as_bytes());
        encoder.tag(self.row.family.semantic_tag());
        encoder.tag(self.row.class.semantic_tag());
        encoder.digest(self.request.as_bytes());
        encoder.digest(self.control.as_bytes());
    }
}

/// Private completed executable evidence; it does not mint a public task key,
/// a plan/table, a View capability, or a live runtime value.
pub(super) struct PreparedRuntimePlanSemantic<'a, 'meter> {
    image: &'a UnsealedRuntimePlanImage,
    executable: RuntimeExecutableSemanticDigest,
    tasks: Box<[TaskSemanticChildren<'a>]>,
    meter: &'meter mut TaskSemanticMeter,
}

impl PreparedRuntimePlanSemantic<'_, '_> {
    pub(super) const fn executable_bytes(&self) -> &[u8; 32] {
        &self.executable.0
    }

    /// Stages the exact core prefix under the caller's same sticky meter.
    /// Binding completion is deliberately private until actual View admission
    /// and common expected-key/duplicate publication are connected.
    pub(super) fn task_prefix(
        &mut self,
        ordinal: usize,
    ) -> Result<TaskSemanticEncoder<'_>, RuntimeTaskPlanImageError> {
        let meter = &mut *self.meter;
        meter.status()?;
        let children = self.tasks.get(ordinal).ok_or_else(|| {
            meter.reject_owner();
            RuntimeBodySemanticError::MissingRow {
                table: "task plans",
                ordinal,
            }
        })?;
        let mut encoder = TaskSemanticEncoder::new(b"arcweft.task.plan-semantic.v1\0", meter);
        encoder.tag(0);
        encoder.digest(&self.executable.0);
        children.write_base(&mut encoder);
        encoder.status()?;
        Ok(encoder)
    }
}

impl UnsealedRuntimePlanImage {
    pub(super) fn prepare_semantics<'image, 'meter>(
        &'image self,
        limits: RuntimeTaskPlanSealLimits,
        meter: &'meter mut TaskSemanticMeter,
        task_reference: &mut impl FnMut(
            RuntimeBodyTaskSource<'_>,
        ) -> Result<
            RuntimeTaskPlanBuildCoordinate,
            RuntimeBodySemanticError,
        >,
    ) -> Result<PreparedRuntimePlanSemantic<'image, 'meter>, RuntimeTaskPlanImageError> {
        let (executable, tasks) = self
            .prepare_complete(limits, meter, task_reference)
            .inspect_err(|_| meter.reject_owner())?;
        Ok(PreparedRuntimePlanSemantic {
            image: self,
            executable,
            tasks,
            meter,
        })
    }

    fn prepare_complete(
        &self,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
        task_reference: &mut impl FnMut(
            RuntimeBodyTaskSource<'_>,
        ) -> Result<
            RuntimeTaskPlanBuildCoordinate,
            RuntimeBodySemanticError,
        >,
    ) -> Result<
        (
            RuntimeExecutableSemanticDigest,
            Box<[TaskSemanticChildren<'_>]>,
        ),
        RuntimeTaskPlanImageError,
    > {
        let mut control = self.preflight(limits, meter)?;
        for (ordinal, row) in self.task_plans.iter().enumerate() {
            if !row.family.accepts_runtime_task_binding(&row.binding) {
                return Err(RuntimeTaskPlanImageError::FamilyBinding {
                    ordinal,
                    family: row.family,
                    binding: row.binding.semantic_tag(),
                });
            }
        }
        let context = RuntimeBodySemanticContext::new(&self.inventory);
        let mut executable = RuntimeExecutableSemanticRows::new(
            &self.inventory,
            &self.coordinate_owner,
            limits,
            meter,
        )?;
        let mut tasks = Vec::with_capacity(self.task_plans.len());
        for row in &self.task_plans {
            meter.charge_work(1)?;
            let function =
                executable.prepare_function(row.producer_function, meter, task_reference)?;
            let endpoint = function
                .endpoint(row.request_template.endpoint())
                .ok_or(RuntimeBodySemanticError::InvalidHostRequestEndpoint)?;
            let request =
                context.request_template_digest(meter, endpoint, &row.request_template, limits)?;
            let function = function.digest();
            let control = control.complete(row.control_effect, meter)?;
            let binding = match &row.binding {
                RuntimeTaskSemanticBinding::Line { group } => TaskBindingSemantic::Line(
                    executable
                        .prepare_line(*group, meter, task_reference)?
                        .digest(),
                ),
                binding => TaskBindingSemantic::Marker(binding),
            };
            tasks.push(TaskSemanticChildren {
                row,
                function,
                request,
                control,
                binding,
            });
        }
        let mut encoder =
            TaskSemanticEncoder::new(b"arcweft.runtime-plan.executable-semantic.v1\0", meter);
        encoder.tag(0);
        encoder.tag(15);
        executable.write_tables(&mut encoder, task_reference)?;
        encoder.enter_role();
        encoder.tag(14);
        encoder.count(tasks.len());
        for (ordinal, children) in tasks.iter().enumerate() {
            encoder.enter_element();
            encoder.count(ordinal);
            encoder.tag(0);
            encoder.count(ordinal);
            children.write_base(&mut encoder);
            children.binding.write(&mut encoder);
        }
        let executable = RuntimeExecutableSemanticDigest(*encoder.finish()?.as_bytes());
        Ok((executable, tasks.into_boxed_slice()))
    }
}

#[cfg(test)]
mod tests;
