//! One borrowed preparation pass for E0..E14 and its actual F/Q/C children.
//! Expected codec assertions never enter this pass; public sealing remains
//! behind the final task/View ownership and codec switch.

use super::{
    RuntimeTaskPlan, RuntimeTaskPlanSealError, RuntimeTaskSemanticBinding, UnsealedRuntimePlanImage,
};
use crate::line_task::semantic::LinePlanSemanticDigest;
use crate::plan::body_semantic::executable_rows::RuntimeExecutableSemanticRows;
use crate::plan::body_semantic::function::ProducerFunctionSemanticDigest;
use crate::plan::body_semantic::request::TaskRequestTemplateDigest;
use crate::plan::body_semantic::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::plan::{
    ControlEffectContractDigest, RuntimeExecutableSemanticDigest, RuntimeTaskPlanSealLimits,
    ViewTaskPlanAuthority,
};
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticEncodingError, TaskSemanticMeter};

/// Completed binding evidence is derived from the same source row. Line
/// retains its actual group proof; no raw digest can issue this evidence.
#[derive(Clone, Copy)]
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
    rows: PreparedRuntimePlanRows<'a>,
    meter: &'meter mut TaskSemanticMeter,
}

struct PreparedRuntimePlanRows<'a> {
    executable: RuntimeExecutableSemanticDigest,
    tasks: Box<[TaskSemanticChildren<'a>]>,
    control_proofs: Box<[ControlEffectContractDigest]>,
}

/// Candidate-local completion proof. Final publication consumes these keys
/// together with the same image's rows; this is never a second public table.
pub(super) struct CompletedRuntimeTaskPlanSemantics {
    executable: RuntimeExecutableSemanticDigest,
    control_proofs: Box<[ControlEffectContractDigest]>,
    rows: Box<
        [(
            crate::task::TaskPlanSemanticDigest,
            crate::plan::RuntimeTaskSemanticBinding,
        )],
    >,
    by_digest: std::collections::BTreeMap<
        crate::task::TaskPlanSemanticDigest,
        crate::plan::RuntimeTaskPlanIndex,
    >,
}

impl CompletedRuntimeTaskPlanSemantics {
    pub(super) const fn executable_bytes(&self) -> &[u8; 32] {
        self.executable.as_bytes()
    }

    pub(super) fn keys(
        &self,
    ) -> impl ExactSizeIterator<Item = crate::task::TaskPlanSemanticDigest> + '_ {
        self.rows.iter().map(|(key, _)| *key)
    }

    fn into_table(
        self,
        source_rows: Box<[RuntimeTaskPlan]>,
    ) -> Result<
        (
            crate::plan::RuntimeTaskPlanTable,
            Box<[ControlEffectContractDigest]>,
        ),
        RuntimeTaskPlanSealError,
    > {
        if source_rows.len() != self.rows.len() {
            return Err(RuntimeTaskPlanSealError::CompletionCount {
                source_rows: source_rows.len(),
                completed_rows: self.rows.len(),
            });
        }
        let rows = source_rows
            .into_vec()
            .into_iter()
            .zip(self.rows.into_vec())
            .map(|(source, (digest, binding))| {
                crate::plan::task_semantic::SealedRuntimeTaskPlanRow {
                    plan: crate::plan::RuntimeTaskPlan {
                        producer_function: source.producer_function,
                        family: source.family,
                        class: source.class,
                        request_template: source.request_template,
                        control_effect: source.control_effect,
                        binding,
                    },
                    digest,
                }
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok((
            crate::plan::RuntimeTaskPlanTable {
                rows,
                by_digest: self.by_digest,
            },
            self.control_proofs,
        ))
    }
}

impl PreparedRuntimePlanSemantic<'_, '_> {
    /// One completion path owns ordinary and capability-gated View keys, then
    /// expected assertions and global duplicate collection in that order.
    pub(super) fn complete_tasks(
        mut self,
        authority: Option<&dyn ViewTaskPlanAuthority>,
    ) -> Result<CompletedRuntimeTaskPlanSemantics, RuntimeTaskPlanSealError> {
        self.meter.status()?;
        let mut rows = Vec::with_capacity(self.rows.tasks.len());
        let mut unique = std::collections::BTreeMap::new();
        for ordinal in 0..self.rows.tasks.len() {
            let source_ordinal = self.meter.checked_ordinal(ordinal)?;
            self.meter.charge_work(1)?;
            let binding = self.rows.tasks[ordinal].binding;
            let key = if matches!(
                binding,
                TaskBindingSemantic::Marker(RuntimeTaskSemanticBinding::View)
            ) {
                let Some(authority) = authority else {
                    self.meter.reject_owner();
                    return Err(RuntimeTaskPlanSealError::MissingViewAuthority { ordinal });
                };
                // Transfer the checked Core prefix and its original meter to
                // the one-use request; the upper owner only supplies binding.
                let (core_prefix, core_prefix_bytes) = {
                    let prefix = self.task_prefix(ordinal)?;
                    let bytes = prefix.encoded_bytes();
                    (prefix.finish_hasher()?, bytes)
                };
                let coordinate = self
                    .image
                    .coordinate_owner
                    .resolve(source_ordinal)
                    .ok_or_else(|| {
                        self.meter.reject_owner();
                        RuntimeBodySemanticError::MissingRow {
                            table: "task coordinates",
                            ordinal,
                        }
                    })?;
                let invocation =
                    crate::plan::task_semantic::ViewTaskPlanSealInvocation::new(coordinate);
                let children = &self.rows.tasks[ordinal];
                let request = invocation.request(
                    children.row,
                    self.image.expected_key(ordinal),
                    self.rows.executable,
                    children.function,
                    children.request,
                    children.control,
                    core_prefix,
                    self.meter,
                );
                let completed = authority.task_plan_semantic_digest(request);
                // A budget rejection by completion is the original sticky
                // encoding failure, even if the upper owner maps its error.
                self.meter.status()?;
                let key = completed
                    .and_then(|key| {
                        invocation.verify(key, core_prefix_bytes)?;
                        Ok(key)
                    })
                    .map_err(|source| {
                        self.meter.reject_owner();
                        RuntimeTaskPlanSealError::ViewAuthority { ordinal, source }
                    })?;
                key
            } else {
                let mut transcript = self.task_prefix(ordinal)?;
                binding.write(&mut transcript);
                crate::task::TaskPlanSemanticDigest::from_semantic_transcript(transcript)?
            };
            if self
                .image
                .expected_key(ordinal)
                .is_some_and(|expected| expected.bytes() != key.as_bytes())
            {
                self.meter.reject_owner();
                return Err(RuntimeTaskPlanSealError::ExpectedKeyMismatch { ordinal });
            }
            let index = crate::plan::RuntimeTaskPlanIndex(source_ordinal);
            if let Some(first) = unique.insert(key, index) {
                self.meter.reject_owner();
                return Err(RuntimeTaskPlanSealError::DuplicateKey {
                    first: first.0 as usize,
                    ordinal,
                });
            }
            let completed_binding = self.rows.tasks[ordinal]
                .row
                .binding
                .try_complete_line(|group| {
                    if let TaskBindingSemantic::Line(proof) = binding {
                        Ok(
                            crate::plan::RuntimeLineTaskPlanBinding::from_original_group(
                                *group, proof,
                            ),
                        )
                    } else {
                        Err(TaskSemanticEncodingError::OwnerRejected)
                    }
                })
                .inspect_err(|_| self.meter.reject_owner())?;
            rows.push((key, completed_binding));
        }
        self.meter.status()?;
        Ok(CompletedRuntimeTaskPlanSemantics {
            executable: self.rows.executable,
            control_proofs: self.rows.control_proofs,
            rows: rows.into_boxed_slice(),
            by_digest: unique,
        })
    }

    pub(super) const fn executable_bytes(&self) -> &[u8; 32] {
        self.rows.executable.as_bytes()
    }

    /// Stages the exact core prefix under the caller's same sticky meter.
    /// Binding completion is deliberately private until actual View admission
    /// and common expected-key/duplicate publication are connected.
    pub(super) fn task_prefix(
        &mut self,
        ordinal: usize,
    ) -> Result<TaskSemanticEncoder<'_>, RuntimeTaskPlanSealError> {
        let meter = &mut *self.meter;
        meter.status()?;
        let children = self.rows.tasks.get(ordinal).ok_or_else(|| {
            meter.reject_owner();
            RuntimeBodySemanticError::MissingRow {
                table: "task plans",
                ordinal,
            }
        })?;
        let mut encoder =
            TaskSemanticEncoder::new(crate::task::TaskPlanSemanticDigest::DOMAIN, meter);
        encoder.tag(0);
        encoder.digest(self.rows.executable.as_bytes());
        children.write_base(&mut encoder);
        encoder.status()?;
        Ok(encoder)
    }
}

impl UnsealedRuntimePlanImage {
    /// The sole consuming completion boundary. No plan or table is exposed
    /// while source definitions, View capabilities or assertions can still fail.
    pub(in crate::plan) fn seal(
        mut self,
        limits: RuntimeTaskPlanSealLimits,
        authority: Option<&dyn ViewTaskPlanAuthority>,
    ) -> Result<SealedRuntimePlanImage, RuntimeTaskPlanSealError> {
        self.coordinate_owner.ensure_unsealed()?;
        let mut meter = self
            .preparation_meter
            .take()
            .expect("every image owns its original preparation meter");
        meter.constrain(limits.max_semantic_work, limits.max_transcript_bytes)?;
        let completed = self
            .prepare_semantics(limits, &mut meter)?
            .complete_tasks(authority)?;
        meter.status()?;
        let executable = completed.executable;
        let (task_plans, control_proofs) = completed.into_table(self.task_plans)?;
        let mut inventory = self.inventory;
        inventory.control_effect_contracts = inventory
            .control_effect_contracts
            .complete(control_proofs)?;
        let semantics =
            std::sync::Arc::new(crate::plan::task_semantic::RuntimeTaskProgramSemantics {
                executable,
                task_plans,
            });
        self.coordinate_owner
            .publish(std::sync::Arc::clone(&semantics))?;
        Ok(SealedRuntimePlanImage {
            inventory,
            semantics,
            coordinate_owner: self.coordinate_owner,
        })
    }

    pub(super) fn prepare_semantics<'image, 'meter>(
        &'image self,
        limits: RuntimeTaskPlanSealLimits,
        meter: &'meter mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimePlanSemantic<'image, 'meter>, RuntimeTaskPlanSealError> {
        let rows = self
            .prepare_complete(limits, meter)
            .inspect_err(|_| meter.reject_owner())?;
        Ok(PreparedRuntimePlanSemantic {
            image: self,
            rows,
            meter,
        })
    }

    fn prepare_complete(
        &self,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimePlanRows<'_>, RuntimeTaskPlanSealError> {
        let mut control = self.preflight(limits, meter)?;
        for (ordinal, row) in self.task_plans.iter().enumerate() {
            row.family
                .validate_runtime_task_binding(&row.binding)
                .map_err(|source| RuntimeTaskPlanSealError::TaskDefinition { ordinal, source })?;
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
            let function = executable.prepare_function(row.producer_function, meter)?;
            let endpoint = function
                .request_endpoint(row.request_template.source())
                .ok_or(RuntimeBodySemanticError::InvalidRequestSource)?;
            let request =
                context.request_template_digest(meter, endpoint, &row.request_template, limits)?;
            let function = function.digest();
            let control = control.complete(row.control_effect, meter)?;
            let binding = match &row.binding {
                RuntimeTaskSemanticBinding::Line { plan: group } => {
                    TaskBindingSemantic::Line(executable.prepare_line(*group, meter)?.digest())
                }
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
        let mut control_proofs =
            Vec::with_capacity(self.inventory.control_effect_contracts().len());
        for index in 0..self.inventory.control_effect_contracts().len() {
            let id = crate::plan::RuntimeControlEffectContractId::for_index(index)
                .ok_or(TaskSemanticEncodingError::ArithmeticOverflow)?;
            control_proofs.push(control.complete(id, meter)?);
        }
        let mut encoder =
            TaskSemanticEncoder::new(b"arcweft.runtime-plan.executable-semantic.v1\0", meter);
        encoder.tag(0);
        encoder.tag(15);
        executable.write_tables(&mut encoder)?;
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
        let executable = RuntimeExecutableSemanticDigest::from_semantic_transcript(encoder)?;
        if self
            .expected_executable_key
            .is_some_and(|expected| executable.as_bytes() != &expected)
        {
            return Err(RuntimeTaskPlanSealError::ExpectedExecutableKeyMismatch);
        }
        Ok(PreparedRuntimePlanRows {
            executable,
            tasks: tasks.into_boxed_slice(),
            control_proofs: control_proofs.into_boxed_slice(),
        })
    }
}

pub(in crate::plan) struct SealedRuntimePlanImage {
    pub(super) inventory: crate::plan::RuntimePlanInventory,
    semantics: std::sync::Arc<crate::plan::task_semantic::RuntimeTaskProgramSemantics>,
    coordinate_owner: crate::plan::RuntimeTaskPlanCoordinateOwner,
}

impl std::ops::Deref for SealedRuntimePlanImage {
    type Target = crate::plan::task_semantic::RuntimeTaskProgramSemantics;
    fn deref(&self) -> &Self::Target {
        &self.semantics
    }
}

impl SealedRuntimePlanImage {
    pub(in crate::plan) fn into_plan(self) -> crate::plan::RuntimePlan {
        crate::plan::RuntimePlan {
            artifact: None,
            inventory: self.inventory,
            semantics: self.semantics,
            task_coordinates: self.coordinate_owner,
        }
    }
}

#[cfg(test)]
pub(super) mod tests;
