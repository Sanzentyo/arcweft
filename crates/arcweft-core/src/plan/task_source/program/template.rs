//! Existing template assertions borrow their exact original producer and ABI.
//! This static join does not certify executable request bodies or caller-local
//! register mappings; the public cutover must retain those execution bindings.

use super::types::{RuntimeAwbcSourceTypeError, RuntimeAwbcSourceTypeRelation};
use super::{RuntimeTaskSourceProgram, RuntimeTaskSourceProgramData};
use crate::awbc::schema::{
    AwbcFunctionId, AwbcTaskPlan, AwbcTaskPlanId, AwbcTaskPlanKind, AwbcTaskPolicy,
    AwbcTaskRequestProjection,
};
use crate::plan::body_semantic::flow::RuntimeBodyTaskSource;
use crate::plan::task_source::{RuntimeTaskSourceError, RuntimeTaskSourceRef};
use crate::plan::{
    FlowOp, RuntimeFunctionCaptureMode, RuntimeGeneratedFunctionRole, RuntimeTaskPlanSealLimits,
};
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use crate::task::{ExpectedTaskPlanKey, NeedProducerFamily, TaskClass, TaskPolicy};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeAwbcTemplateSourceField {
    RequestKind,
    Family,
    Contract,
    Site,
    RequestProjection,
    Capability,
    Operation,
    Class,
    Priority,
    CancelScope,
    Policy,
    Payload,
    ArgumentCount,
    ArgumentName { index: usize },
    ArgumentSpread { index: usize },
    ArgumentType { index: usize },
    RequestDefinition,
    CaptureCount,
    Capture { index: usize },
    CaptureType { index: usize },
    ChildKey,
    Limit,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum RuntimeAwbcTemplateSourceError {
    #[error("template Source admission requires the actual AWBC execution context")]
    Backend,
    #[error("AWBC template {task:?} is absent from its actual executable")]
    MissingTask { task: AwbcTaskPlanId },
    #[error("AWBC template {task:?} has {actual} roles in {field:?}; limit is {maximum}")]
    Roles {
        task: AwbcTaskPlanId,
        field: RuntimeAwbcTemplateSourceField,
        actual: usize,
        maximum: u32,
    },
    #[error("AWBC template {task:?} cannot resolve its original Source: {source}")]
    Source {
        task: AwbcTaskPlanId,
        #[source]
        source: RuntimeTaskSourceError,
    },
    #[error("AWBC template {task:?} differs from its original Source in {field:?}")]
    Mismatch {
        task: AwbcTaskPlanId,
        field: RuntimeAwbcTemplateSourceField,
    },
    #[error("AWBC template {task:?} has a different original type in {field:?}: {source}")]
    Type {
        task: AwbcTaskPlanId,
        field: RuntimeAwbcTemplateSourceField,
        #[source]
        source: RuntimeAwbcSourceTypeError,
    },
    #[error("AWBC template Source admission exhausted its original allowance: {0:?}")]
    Encoding(TaskSemanticEncodingError),
}

impl From<TaskSemanticEncodingError> for RuntimeAwbcTemplateSourceError {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Encoding(error)
    }
}

impl RuntimeAwbcTemplateSourceError {
    fn type_relation(
        task: AwbcTaskPlanId,
        field: RuntimeAwbcTemplateSourceField,
        source: RuntimeAwbcSourceTypeError,
    ) -> Self {
        match source {
            RuntimeAwbcSourceTypeError::Encoding(error) => Self::Encoding(error),
            source => Self::Type {
                task,
                field,
                source,
            },
        }
    }
}

impl RuntimeTaskSourceProgram {
    /// Resolves only an already asserted original row. The temporary relation
    /// spans the base and child without copying either definition or any lease.
    pub(crate) fn awbc_template_source(
        &self,
        task: AwbcTaskPlanId,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeTaskSourceRef<'_>, RuntimeAwbcTemplateSourceError> {
        let result = self
            .admit_awbc_template_source(task, limits, meter)
            .map(|source| self.bind_source(source, super::RuntimeTaskExecutionRow::Template(task)));
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }

    fn admit_awbc_template_source(
        &self,
        task: AwbcTaskPlanId,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeTaskSourceRef<'_>, RuntimeAwbcTemplateSourceError> {
        use RuntimeAwbcTemplateSourceField as Field;
        meter.constrain(limits.max_semantic_work, limits.max_transcript_bytes)?;
        meter.charge_work(1)?;
        let RuntimeTaskSourceProgramData::Awbc {
            executable,
            original,
        } = self.data.as_ref()
        else {
            return Err(RuntimeAwbcTemplateSourceError::Backend);
        };
        let row = executable
            .task_plans
            .get(task.index())
            .ok_or(RuntimeAwbcTemplateSourceError::MissingTask { task })?;
        let mut admission = TemplateAdmission {
            program: self,
            limits,
            meter,
            types: Vec::new(),
        };
        let source = admission.row(task, row)?;
        if let AwbcTaskPlanKind::AwaitMany { child, limit, .. } = &row.kind {
            let Some(FlowOp::AwaitMany { target, .. }) = source.request_endpoint().operation()
            else {
                return Err(RuntimeAwbcTemplateSourceError::Mismatch {
                    task,
                    field: Field::RequestKind,
                });
            };
            admission.check(task, Field::Limit, *limit == target.limit)?;
            let child_source = admission.row(*child, &executable.task_plans[child.index()])?;
            let expected = original
                .task_plan(&target.child.plan)
                .expect("the original common seal admitted its child coordinate");
            admission.check(task, Field::ChildKey, child_source.task_plan() == expected)?;
        }
        // Defer the complete graph walk until every metadata/role bound is
        // checked. These pairs borrow existing rows, not a second type catalog.
        let mut relation =
            RuntimeAwbcSourceTypeRelation::new(original, executable, limits, admission.meter);
        for (task, field, original, executable) in admission.types {
            relation.admit(original, executable).map_err(|source| {
                RuntimeAwbcTemplateSourceError::type_relation(task, field, source)
            })?;
        }
        Ok(source)
    }
}

/// Temporary ordered join state. It never owns a Source or an executable lease.
struct TemplateAdmission<'program, 'work> {
    program: &'program RuntimeTaskSourceProgram,
    limits: RuntimeTaskPlanSealLimits,
    meter: &'work mut TaskSemanticMeter,
    types: Vec<(
        AwbcTaskPlanId,
        RuntimeAwbcTemplateSourceField,
        crate::runtime_id::RuntimePlanTypeId,
        crate::awbc::schema::AwbcTypeId,
    )>,
}

impl<'program> TemplateAdmission<'program, '_> {
    fn check(
        &mut self,
        task: AwbcTaskPlanId,
        field: RuntimeAwbcTemplateSourceField,
        equal: bool,
    ) -> Result<(), RuntimeAwbcTemplateSourceError> {
        self.meter.charge_work(1)?;
        if equal {
            Ok(())
        } else {
            Err(RuntimeAwbcTemplateSourceError::Mismatch { task, field })
        }
    }

    fn roles(
        &mut self,
        task: AwbcTaskPlanId,
        field: RuntimeAwbcTemplateSourceField,
        actual: usize,
        maximum: u32,
    ) -> Result<(), RuntimeAwbcTemplateSourceError> {
        self.meter.charge_work(1)?;
        if actual <= maximum as usize {
            Ok(())
        } else {
            Err(RuntimeAwbcTemplateSourceError::Roles {
                task,
                field,
                actual,
                maximum,
            })
        }
    }

    fn text(
        &mut self,
        task: AwbcTaskPlanId,
        field: RuntimeAwbcTemplateSourceField,
        actual: &str,
        expected: &str,
    ) -> Result<(), RuntimeAwbcTemplateSourceError> {
        let bytes = self.meter.checked_count_sum(actual.len(), expected.len())?;
        self.meter.charge_work((bytes as u64).div_ceil(32))?;
        self.check(task, field, actual == expected)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one original template assertion boundary preserves key, field, capture and type error order"
    )]
    fn row(
        &mut self,
        task: AwbcTaskPlanId,
        row: &'program AwbcTaskPlan,
    ) -> Result<RuntimeTaskSourceRef<'program>, RuntimeAwbcTemplateSourceError> {
        use RuntimeAwbcTemplateSourceField as Field;
        let RuntimeTaskSourceProgramData::Awbc {
            executable,
            original,
        } = self.program.data.as_ref()
        else {
            return Err(RuntimeAwbcTemplateSourceError::Backend);
        };
        let (family, contract, site, plan, request_function) = match &row.kind {
            AwbcTaskPlanKind::AwaitMany {
                contract,
                site,
                plan,
                request_function,
                ..
            } => (
                NeedProducerFamily::AwaitManyBase,
                *contract,
                *site,
                *plan,
                *request_function,
            ),
            AwbcTaskPlanKind::Template {
                family,
                contract,
                site,
                plan,
                request_function,
            } => (*family, *contract, *site, *plan, *request_function),
            AwbcTaskPlanKind::NeedProducer { .. } => {
                return Err(RuntimeAwbcTemplateSourceError::Mismatch {
                    task,
                    field: Field::RequestKind,
                });
            }
        };
        self.roles(
            task,
            Field::ArgumentCount,
            row.arguments.len(),
            self.limits.max_request_roles,
        )?;
        let kernel = &executable.functions[request_function.index()];
        let kernel_signature = &executable.signatures[kernel.signature.index()];
        self.roles(
            task,
            Field::CaptureCount,
            kernel_signature.params.len(),
            self.limits.max_function_roles,
        )?;
        let source = original
            .task_source_by_expected_key(
                &ExpectedTaskPlanKey::new(*plan.as_bytes()),
                self.limits,
                self.meter,
            )
            .map_err(|source| RuntimeAwbcTemplateSourceError::Source { task, source })?;
        let Some(RuntimeBodyTaskSource::Producer(producer)) =
            source.request_endpoint().task_source()
        else {
            return Err(RuntimeAwbcTemplateSourceError::Mismatch {
                task,
                field: Field::RequestKind,
            });
        };
        let original_kind = source.request_endpoint().kind();
        self.check(
            task,
            Field::RequestKind,
            match &row.kind {
                AwbcTaskPlanKind::AwaitMany { .. } => {
                    original_kind
                        == crate::plan::body_semantic::function::EndpointKind::AwaitManyBase
                }
                AwbcTaskPlanKind::Template { .. } => {
                    original_kind
                        != crate::plan::body_semantic::function::EndpointKind::AwaitManyBase
                }
                AwbcTaskPlanKind::NeedProducer { .. } => false,
            },
        )?;
        self.check(
            task,
            Field::Family,
            family == producer.family && family == source.task_plan().definition().family(),
        )?;
        self.check(task, Field::Contract, contract == producer.contract)?;
        self.check(task, Field::Site, site == producer.producer_site)?;
        let AwbcTaskRequestProjection::CustomCapability {
            capability,
            operation,
        } = row.request
        else {
            return Err(RuntimeAwbcTemplateSourceError::Mismatch {
                task,
                field: Field::RequestProjection,
            });
        };
        self.text(
            task,
            Field::Capability,
            &executable.strings[capability.index()],
            &producer.request.capability.0,
        )?;
        self.text(
            task,
            Field::Operation,
            &executable.strings[operation.index()],
            &producer.request.operation,
        )?;
        // Shared request metadata does not own the actual completed row's class.
        self.check(
            task,
            Field::Class,
            &TaskClass::from(row.class) == source.task_plan().definition().class(),
        )?;
        self.check(task, Field::Priority, row.priority == producer.priority.0)?;
        self.text(
            task,
            Field::CancelScope,
            &executable.strings[row.cancel_scope.index()],
            &producer.cancel_scope.0,
        )?;
        self.check(
            task,
            Field::Policy,
            matches!(
                (row.policy, producer.policy),
                (AwbcTaskPolicy::JoinSameKey, TaskPolicy::JoinSameKey)
                    | (AwbcTaskPolicy::AlwaysStart, TaskPolicy::AlwaysStart)
            ),
        )?;
        let payload =
            crate::pattern::RuntimeSemanticTypeId::from_bytes(*producer.payload_type.as_bytes());
        let payload = original
            .type_table()
            .id_for_semantic(payload)
            .expect("the original common seal admitted the producer payload");
        self.types
            .push((task, Field::Payload, payload, row.payload_type));
        let args = &producer.request.args;
        self.roles(
            task,
            Field::ArgumentCount,
            args.len(),
            self.limits.max_request_roles,
        )?;
        let signature = &executable.signatures[row.signature.index()];
        self.check(
            task,
            Field::ArgumentCount,
            args.len() == row.arguments.len()
                && signature.params.len() == args.len()
                && signature.result.is_none(),
        )?;
        for (index, ((expected, actual), ty)) in args
            .iter()
            .zip(&row.arguments)
            .zip(&signature.params)
            .enumerate()
        {
            let actual_name = actual
                .name
                .map(|name| executable.strings[name.index()].as_str());
            if let (Some(actual), Some(expected)) = (actual_name, expected.name()) {
                self.text(task, Field::ArgumentName { index }, actual, expected)?;
            } else {
                self.check(
                    task,
                    Field::ArgumentName { index },
                    actual_name == expected.name(),
                )?;
            }
            self.check(
                task,
                Field::ArgumentSpread { index },
                actual.spread == expected.is_spread(),
            )?;
            self.types.push((
                task,
                Field::ArgumentType { index },
                expected.value().ty(),
                *ty,
            ));
        }
        self.check(
            task,
            Field::RequestDefinition,
            kernel.definition
                == producer
                    .definition
                    .generated_child(RuntimeGeneratedFunctionRole::TaskRequest)
                && kernel.semantic_role == crate::plan::RuntimeFunctionSemanticRole::Ordinary,
        )?;
        self.captures(task, request_function, &source)?;
        Ok(source)
    }

    fn captures(
        &mut self,
        task: AwbcTaskPlanId,
        function: AwbcFunctionId,
        source: &RuntimeTaskSourceRef<'program>,
    ) -> Result<(), RuntimeAwbcTemplateSourceError> {
        use RuntimeAwbcTemplateSourceField as Field;
        let mut inputs: Vec<RuntimeLocalDeclarationId> = Vec::new();
        match source.request_endpoint().operation() {
            Some(FlowOp::Thread { captures, .. }) => {
                self.roles(
                    task,
                    Field::CaptureCount,
                    captures.len(),
                    self.limits.max_function_roles,
                )?;
                self.meter.charge_work(captures.len() as u64)?;
                inputs.extend_from_slice(captures);
            }
            Some(FlowOp::AwaitMany { target, .. }) => {
                self.roles(
                    task,
                    Field::CaptureCount,
                    target.base.request.captures.len(),
                    self.limits.max_function_roles,
                )?;
                self.roles(
                    task,
                    Field::CaptureCount,
                    target.child.request.captures.len(),
                    self.limits.max_function_roles,
                )?;
                let count = self.meter.checked_count_sum(
                    target.base.request.captures.len(),
                    target.child.request.captures.len(),
                )?;
                self.meter.charge_work(count as u64)?;
                let mut seen = std::collections::BTreeSet::new();
                for local in target
                    .base
                    .request
                    .captures
                    .iter()
                    .chain(&target.child.request.captures)
                {
                    if !seen.contains(local) {
                        let next = self.meter.checked_count_sum(inputs.len(), 1)?;
                        self.roles(
                            task,
                            Field::CaptureCount,
                            next,
                            self.limits.max_function_roles,
                        )?;
                        seen.insert(*local);
                        inputs.push(*local);
                    }
                }
                if source.request_endpoint().kind()
                    == crate::plan::body_semantic::function::EndpointKind::AwaitManyChild
                {
                    let next = self.meter.checked_count_sum(inputs.len(), 1)?;
                    self.roles(
                        task,
                        Field::CaptureCount,
                        next,
                        self.limits.max_function_roles,
                    )?;
                    inputs.push(target.item_binding);
                }
            }
            _ => {
                return Err(RuntimeAwbcTemplateSourceError::Mismatch {
                    task,
                    field: Field::RequestKind,
                });
            }
        }
        let RuntimeTaskSourceProgramData::Awbc {
            executable,
            original,
        } = self.program.data.as_ref()
        else {
            return Err(RuntimeAwbcTemplateSourceError::Backend);
        };
        let kernel = &executable.functions[function.index()];
        let signature = &executable.signatures[kernel.signature.index()];
        self.check(
            task,
            Field::CaptureCount,
            inputs.len() == signature.params.len() && inputs.len() == kernel.input_ownership.len(),
        )?;
        for (index, ((local, ty), actual)) in inputs
            .iter()
            .zip(&signature.params)
            .zip(&kernel.input_ownership)
            .enumerate()
        {
            let expected = original
                .local_declarations()
                .get(*local)
                .expect("the original common seal admitted its captured local");
            let expected_ownership = crate::awbc::schema::AwbcFunctionInputOwnership::capture(
                expected.origin(),
                u32::try_from(index).map_err(|_| TaskSemanticEncodingError::ArithmeticOverflow)?,
                RuntimeFunctionCaptureMode::Copy,
            );
            self.check(
                task,
                Field::Capture { index },
                actual == &expected_ownership,
            )?;
            self.types
                .push((task, Field::CaptureType { index }, expected.ty(), *ty));
        }
        Ok(())
    }
}
