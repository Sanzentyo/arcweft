//! Original Program body, capture ABI and scheduling admission for AWBC tasks.

use super::types::{RuntimeAwbcSourceTypeError, RuntimeAwbcSourceTypeRelation};
use super::{RuntimeTaskExecutionRow, RuntimeTaskSourceProgram, RuntimeTaskSourceProgramData};
use crate::awbc::schema::{
    AwbcFunctionInputOwnership, AwbcFunctionKind, AwbcTaskPlanExecution, AwbcTaskPlanId,
    AwbcTaskPolicy,
};
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::task_source::{RuntimeTaskSourceError, RuntimeTaskSourceRef};
use crate::plan::{
    RuntimeFunctionInputSource, RuntimeFunctionInputTransfer, RuntimeFunctionSemanticRole,
    RuntimeGeneratedFunctionRole, RuntimeTaskPlanSealLimits,
};
use crate::task::TaskPolicy;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};

#[cfg(test)]
pub(crate) mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeAwbcProgramSourceField {
    Kind,
    Kernel,
    Definition,
    TypeContext,
    Effects,
    Class,
    Priority,
    CancelScope,
    Policy,
    Payload,
    HandleType,
    CaptureCount,
    Capture { index: usize },
    CaptureType { index: usize },
    CaptureMode { index: usize },
    Delivery,
    FanoutWorker,
    FanoutControls,
    FanoutInput { index: usize },
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum RuntimeAwbcProgramSourceError {
    #[error("Program task Source admission requires its actual AWBC execution context")]
    Backend,
    #[error("AWBC Program task {task:?} is absent from its executable")]
    MissingTask { task: AwbcTaskPlanId },
    #[error("AWBC Program task {task:?} differs from its original Source in {field:?}")]
    Mismatch {
        task: AwbcTaskPlanId,
        field: RuntimeAwbcProgramSourceField,
    },
    #[error("AWBC Program task {task:?} cannot resolve its original Source: {source}")]
    Source {
        task: AwbcTaskPlanId,
        #[source]
        source: RuntimeTaskSourceError,
    },
    #[error("AWBC Program task {task:?} has a different original type in {field:?}: {source}")]
    Type {
        task: AwbcTaskPlanId,
        field: RuntimeAwbcProgramSourceField,
        #[source]
        source: RuntimeAwbcSourceTypeError,
    },
    #[error("AWBC Program task {task:?} has {actual} capture roles; limit is {maximum}")]
    Roles {
        task: AwbcTaskPlanId,
        actual: usize,
        maximum: u32,
    },
    #[error("AWBC Program task Source admission exhausted its allowance: {0:?}")]
    Encoding(TaskSemanticEncodingError),
}

impl From<TaskSemanticEncodingError> for RuntimeAwbcProgramSourceError {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Encoding(error)
    }
}

impl RuntimeTaskSourceProgram {
    pub(crate) fn awbc_program_source(
        &self,
        task: AwbcTaskPlanId,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeTaskSourceRef<'_>, RuntimeAwbcProgramSourceError> {
        let result = self
            .admit_awbc_program_source(task, limits, meter)
            .map(|source| self.bind_source(source, RuntimeTaskExecutionRow::Program(task)));
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }

    fn admit_awbc_program_source(
        &self,
        task: AwbcTaskPlanId,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeTaskSourceRef<'_>, RuntimeAwbcProgramSourceError> {
        use RuntimeAwbcProgramSourceError as Error;
        use RuntimeAwbcProgramSourceField as Field;
        meter.constrain(limits.max_semantic_work, limits.max_transcript_bytes)?;
        meter.charge_work(1)?;
        let RuntimeTaskSourceProgramData::Awbc {
            executable,
            original,
        } = self.data.as_ref()
        else {
            return Err(Error::Backend);
        };
        let row = executable
            .task_plans
            .get(task.index())
            .ok_or(Error::MissingTask { task })?;
        let mismatch = |field| Error::Mismatch { task, field };
        let AwbcTaskPlanExecution::Program(program) = &row.execution else {
            return Err(mismatch(Field::Kind));
        };
        let source = original
            .task_source_by_expected_key(&program.source, limits, meter)
            .map_err(|source| Error::Source { task, source })?;
        let Some(template) = source.request_endpoint().program_template() else {
            return Err(mismatch(Field::Kind));
        };
        let producer = template.producer();
        let delivery = template.delivery();
        let captures = template.captures();
        let kernel = executable
            .functions
            .get(program.kernel.index())
            .ok_or_else(|| mismatch(Field::Kernel))?;
        let signature = executable
            .signatures
            .get(kernel.signature.index())
            .ok_or_else(|| mismatch(Field::Kernel))?;
        // Count every source/code role before traversing types or copying a lease.
        for actual in [
            captures.len(),
            signature.params.len(),
            kernel.input_ownership.len(),
            program.capture_modes.len(),
        ] {
            meter.charge_work(1)?;
            if actual > limits.max_function_roles as usize {
                return Err(Error::Roles {
                    task,
                    actual,
                    maximum: limits.max_function_roles,
                });
            }
        }
        let check = |field, equal, meter: &mut TaskSemanticMeter| -> Result<(), Error> {
            meter.charge_work(1)?;
            if equal { Ok(()) } else { Err(mismatch(field)) }
        };
        check(Field::Delivery, program.delivery == delivery, meter)?;
        check(
            Field::HandleType,
            row.handle_type.map_payload(|_| ()) == producer.handle_type.map_payload(|_| ()),
            meter,
        )?;
        check(
            Field::CaptureCount,
            captures.len() == program.capture_modes.len(),
            meter,
        )?;
        for (index, (capture, mode)) in captures.iter().zip(&program.capture_modes).enumerate() {
            check(Field::CaptureMode { index }, capture.mode == *mode, meter)?;
        }
        match (template.kernel(), &program.body) {
            (
                crate::plan::RuntimeProgramTaskKernelRef::Flow(_),
                crate::awbc::schema::AwbcProgramTaskKernel::Flow,
            ) => {}
            (
                crate::plan::RuntimeProgramTaskKernelRef::Fanout(fanout),
                crate::awbc::schema::AwbcProgramTaskKernel::Fanout {
                    worker,
                    source: source_position,
                    limit,
                    inputs,
                },
            ) => {
                meter.charge_work(1)?;
                if inputs.len() > limits.max_function_roles as usize {
                    return Err(Error::Roles {
                        task,
                        actual: inputs.len(),
                        maximum: limits.max_function_roles,
                    });
                }
                check(
                    Field::FanoutControls,
                    captures
                        .get(*source_position as usize)
                        .is_some_and(|capture| capture.local == fanout.source)
                        && captures
                            .get(*limit as usize)
                            .is_some_and(|capture| capture.local == fanout.limit),
                    meter,
                )?;
                let worker_row = executable
                    .task_plans
                    .get(worker.index())
                    .ok_or_else(|| mismatch(Field::FanoutWorker))?;
                let AwbcTaskPlanExecution::Program(worker_body) = &worker_row.execution else {
                    return Err(mismatch(Field::FanoutWorker));
                };
                let worker_source = original
                    .task_source(&fanout.worker.producer.plan, limits, meter)
                    .map_err(|source| Error::Source { task, source })?;
                check(
                    Field::FanoutWorker,
                    worker_body.source == worker_source.task_plan().key().into(),
                    meter,
                )?;
                check(
                    Field::CaptureCount,
                    inputs.len() == fanout.worker.captures.len(),
                    meter,
                )?;
                for (index, (input, capture)) in
                    inputs.iter().zip(&fanout.worker.captures).enumerate()
                {
                    let same = match input {
                        crate::awbc::schema::AwbcProgramFanoutInput::Item => {
                            capture.local == fanout.item
                        }
                        crate::awbc::schema::AwbcProgramFanoutInput::Capture { position } => {
                            captures.get(*position as usize).is_some_and(|parent| {
                                parent.local == capture.local && capture.local != fanout.item
                            })
                        }
                    };
                    check(Field::FanoutInput { index }, same, meter)?;
                }
            }
            _ => return Err(mismatch(Field::Kind)),
        }
        check(
            Field::Kernel,
            kernel.kind == AwbcFunctionKind::ProgramTask,
            meter,
        )?;
        check(
            Field::Definition,
            kernel.definition
                == producer
                    .function
                    .generated_child(RuntimeGeneratedFunctionRole::TaskBody)
                && kernel.semantic_role == RuntimeFunctionSemanticRole::Ordinary,
            meter,
        )?;
        check(
            Field::Class,
            crate::task::TaskClass::from(row.class) == producer.class,
            meter,
        )?;
        check(Field::Priority, row.priority == producer.priority.0, meter)?;
        let scope = executable
            .strings
            .get(row.cancel_scope.index())
            .ok_or_else(|| mismatch(Field::CancelScope))?;
        let bytes = meter.checked_count_sum(scope.len(), producer.cancel_scope.0.len())?;
        meter.charge_work((bytes as u64).div_ceil(32))?;
        check(Field::CancelScope, scope == &producer.cancel_scope.0, meter)?;
        let policy = match row.policy {
            AwbcTaskPolicy::JoinSameKey => TaskPolicy::JoinSameKey,
            AwbcTaskPolicy::AlwaysStart => TaskPolicy::AlwaysStart,
        };
        check(Field::Policy, policy == producer.policy, meter)?;
        check(
            Field::Payload,
            signature.result == Some(row.payload_type()),
            meter,
        )?;
        let enclosing = original
            .function_sites()
            .get(source.request_endpoint().function_id())
            .ok_or_else(|| mismatch(Field::TypeContext))?;
        let context = match (enclosing.function_type(), kernel.type_context) {
            (None, None) => None,
            (
                Some(original),
                Some(crate::awbc::schema::AwbcFunctionTypeContext::ProgramBody(actual)),
            ) => Some((original, actual)),
            _ => return Err(mismatch(Field::TypeContext)),
        };
        let effects = executable
            .effect_sets
            .get(signature.effects.index())
            .ok_or_else(|| mismatch(Field::Effects))?;
        check(
            Field::Effects,
            effects.effects.len() == enclosing.invocation_effects().len(),
            meter,
        )?;
        meter.charge_work(effects.effects.len() as u64)?;
        for (actual, expected) in effects
            .effects
            .iter()
            .zip(enclosing.invocation_effects().iter())
        {
            let actual = executable
                .strings
                .get(actual.index())
                .ok_or_else(|| mismatch(Field::Effects))?;
            let bytes = meter.checked_count_sum(actual.len(), expected.as_str().len())?;
            meter.charge_work((bytes as u64).div_ceil(32))?;
            check(Field::Effects, actual == expected.as_str(), meter)?;
        }
        check(
            Field::CaptureCount,
            captures.len() == signature.params.len()
                && captures.len() == kernel.input_ownership.len(),
            meter,
        )?;
        let payload = original
            .type_table()
            .id_for_semantic(RuntimeSemanticTypeId::from_bytes(
                *producer.handle_type.payload_type().as_bytes(),
            ))
            .ok_or_else(|| mismatch(Field::Payload))?;
        // Extracting an original body retains external bindings. It does not
        // fabricate a language Copy or Move operation from frame ingress.
        meter.charge_work(captures.len() as u64)?;
        for (index, (capture, actual)) in captures.iter().zip(&kernel.input_ownership).enumerate() {
            let local = &capture.local;
            let declaration = original
                .local_declarations()
                .get(*local)
                .ok_or_else(|| mismatch(Field::Capture { index }))?;
            let expected = AwbcFunctionInputOwnership::owned(
                declaration.origin(),
                RuntimeFunctionInputSource::Capture {
                    position: u32::try_from(index)
                        .map_err(|_| TaskSemanticEncodingError::ArithmeticOverflow)?,
                },
                RuntimeFunctionInputTransfer::ExternalBinding,
            );
            if actual != &expected {
                return Err(mismatch(Field::Capture { index }));
            }
        }
        let mut types = RuntimeAwbcSourceTypeRelation::new(original, executable, limits, meter);
        if let Some((original, actual)) = context {
            types
                .admit(original, actual)
                .map_err(|source| match source {
                    RuntimeAwbcSourceTypeError::Encoding(error) => Error::Encoding(error),
                    source => Error::Type {
                        task,
                        field: Field::TypeContext,
                        source,
                    },
                })?;
        }
        types
            .admit(payload, row.payload_type())
            .map_err(|source| match source {
                RuntimeAwbcSourceTypeError::Encoding(error) => Error::Encoding(error),
                source => Error::Type {
                    task,
                    field: Field::Payload,
                    source,
                },
            })?;
        for (index, (capture, ty)) in captures.iter().zip(&signature.params).enumerate() {
            let local = &capture.local;
            let declaration = original
                .local_declarations()
                .get(*local)
                .ok_or_else(|| mismatch(Field::Capture { index }))?;
            types
                .admit(declaration.ty(), *ty)
                .map_err(|source| match source {
                    RuntimeAwbcSourceTypeError::Encoding(error) => Error::Encoding(error),
                    source => Error::Type {
                        task,
                        field: Field::CaptureType { index },
                        source,
                    },
                })?;
        }
        Ok(source)
    }
}
