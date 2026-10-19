//! Selected code assertions resolve only in their actual original namespace.
//! Metadata never selects or reconstructs Source, including after decoding.

use super::types::{RuntimeAwbcSourceTypeError, RuntimeAwbcSourceTypeRelation};
use super::{RuntimeTaskSourceProgram, RuntimeTaskSourceProgramData};
use crate::awbc::schema::{
    AwbcTaskPlanId, AwbcTaskPlanKind, AwbcTaskPolicy, AwbcTaskRequestProjection,
    AwbcTaskRestartPolicy,
};
use crate::plan::RuntimeTaskPlanSealLimits;
use crate::plan::body_semantic::flow::RuntimeBodyTaskSource;
use crate::plan::task_source::{RuntimeTaskSourceError, RuntimeTaskSourceRef};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use crate::task::{
    HostRestartPolicy, NeedProducerFamily, NeedProducerRequestProjection, TaskClass, TaskPolicy,
};

#[cfg(test)]
pub(in crate::plan::task_source) mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeAwbcSelectedSourceField {
    RequestKind,
    Family,
    Contract,
    Site,
    RequestProjection,
    AssetKind,
    ArgumentName { index: usize },
    Capability,
    Operation,
    ManifestContract,
    Class,
    Priority,
    CancelScope,
    Policy,
    Restart,
    Payload,
    ArgumentCount,
    ArgumentSpread { index: usize },
    ArgumentType { index: usize },
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum RuntimeAwbcSelectedSourceError {
    #[error("selected Source admission requires the actual AWBC execution context")]
    Backend,
    #[error("AWBC selected row {task:?} is absent from its actual executable")]
    MissingTask { task: AwbcTaskPlanId },
    #[error("AWBC selected row {task:?} cannot resolve its original Source: {source}")]
    Source {
        task: AwbcTaskPlanId,
        #[source]
        source: RuntimeTaskSourceError,
    },
    #[error("AWBC selected row {task:?} has {actual} argument roles; limit is {maximum}")]
    Roles {
        task: AwbcTaskPlanId,
        actual: usize,
        maximum: u32,
    },
    #[error("AWBC selected row {task:?} differs from its original Source in {field:?}")]
    Mismatch {
        task: AwbcTaskPlanId,
        field: RuntimeAwbcSelectedSourceField,
    },
    #[error("AWBC selected row {task:?} has another original type in {field:?}: {source}")]
    Type {
        task: AwbcTaskPlanId,
        field: RuntimeAwbcSelectedSourceField,
        #[source]
        source: RuntimeAwbcSourceTypeError,
    },
    #[error("AWBC selected Source admission exhausted its original allowance: {0:?}")]
    Encoding(TaskSemanticEncodingError),
}

impl From<TaskSemanticEncodingError> for RuntimeAwbcSelectedSourceError {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Encoding(error)
    }
}

impl RuntimeAwbcSelectedSourceError {
    fn type_relation(
        task: AwbcTaskPlanId,
        field: RuntimeAwbcSelectedSourceField,
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
    /// Resolves the decoded assertion in this executable's actual original
    /// table. No caller Source, site lookup or reconstructed request selects
    /// a row. Its complete ABI precedes the borrowed execution capability.
    pub(crate) fn awbc_selected_source<'program>(
        &'program self,
        task: AwbcTaskPlanId,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeTaskSourceRef<'program>, RuntimeAwbcSelectedSourceError> {
        let result = self
            .admit_awbc_selected_source(task, limits, meter)
            .map(|source| self.bind_source(source, super::RuntimeTaskExecutionRow::Selected(task)));
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one ordered borrowed selected request/code assertion boundary without rebuilding a request or issuing a digest"
    )]
    fn admit_awbc_selected_source<'program>(
        &'program self,
        task: AwbcTaskPlanId,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeTaskSourceRef<'program>, RuntimeAwbcSelectedSourceError> {
        use RuntimeAwbcSelectedSourceField as Field;
        meter.constrain(limits.max_semantic_work, limits.max_transcript_bytes)?;
        meter.charge_work(1)?;
        let RuntimeTaskSourceProgramData::Awbc {
            executable,
            original,
        } = self.data.as_ref()
        else {
            return Err(RuntimeAwbcSelectedSourceError::Backend);
        };
        let row = executable
            .task_plans
            .get(task.index())
            .ok_or(RuntimeAwbcSelectedSourceError::MissingTask { task })?;
        let roles = |actual: usize, meter: &mut TaskSemanticMeter| {
            meter.charge_work(1)?;
            if actual > limits.max_request_roles as usize {
                Err(RuntimeAwbcSelectedSourceError::Roles {
                    task,
                    actual,
                    maximum: limits.max_request_roles,
                })
            } else {
                Ok(())
            }
        };
        roles(row.arguments.len(), meter)?;
        let AwbcTaskPlanKind::NeedProducer {
            plan,
            contract,
            site,
            restart,
        } = &row.kind
        else {
            return Err(RuntimeAwbcSelectedSourceError::Mismatch {
                task,
                field: Field::RequestKind,
            });
        };
        let source = original
            .task_source_by_expected_key(plan, limits, meter)
            .map_err(|source| RuntimeAwbcSelectedSourceError::Source { task, source })?;
        let Some(RuntimeBodyTaskSource::Start(target)) = source.request_endpoint().task_source()
        else {
            return Err(RuntimeAwbcSelectedSourceError::Mismatch {
                task,
                field: Field::RequestKind,
            });
        };
        let metadata = target.plan();
        roles(target.arguments().len(), meter)?;
        let check = |field, equal, meter: &mut TaskSemanticMeter| {
            meter.charge_work(1)?;
            if equal {
                Ok(())
            } else {
                Err(RuntimeAwbcSelectedSourceError::Mismatch { task, field })
            }
        };
        let text = |field, actual: &str, expected: &str, meter: &mut TaskSemanticMeter| {
            let bytes = meter.checked_count_sum(actual.len(), expected.len())?;
            meter.charge_work((bytes as u64).div_ceil(32))?;
            check(field, actual == expected, meter)
        };
        check(
            Field::Family,
            source.task_plan().definition().family() == NeedProducerFamily::SelectedCallable,
            meter,
        )?;
        check(Field::Contract, *contract == metadata.contract(), meter)?;
        check(Field::Site, *site == metadata.site(), meter)?;
        match (&row.request, metadata.request()) {
            (
                AwbcTaskRequestProjection::AssetLoad {
                    kind,
                    argument_name,
                },
                NeedProducerRequestProjection::AssetLoad {
                    kind: expected,
                    argument_name: expected_name,
                },
            ) => {
                check(Field::AssetKind, kind == expected, meter)?;
                text(
                    Field::ArgumentName { index: 0 },
                    &executable.strings[argument_name.index()],
                    expected_name,
                    meter,
                )?;
            }
            (
                AwbcTaskRequestProjection::ExternCapability {
                    capability,
                    operation,
                    contract,
                },
                NeedProducerRequestProjection::ExternCapability {
                    capability: expected_capability,
                    operation: expected_operation,
                    contract: expected_contract,
                    ..
                },
            ) => {
                text(
                    Field::Capability,
                    &executable.strings[capability.index()],
                    &expected_capability.0,
                    meter,
                )?;
                text(
                    Field::Operation,
                    &executable.strings[operation.index()],
                    expected_operation,
                    meter,
                )?;
                check(
                    Field::ManifestContract,
                    contract == expected_contract,
                    meter,
                )?;
            }
            _ => {
                return Err(RuntimeAwbcSelectedSourceError::Mismatch {
                    task,
                    field: Field::RequestProjection,
                });
            }
        }
        check(
            Field::Class,
            &TaskClass::from(row.class) == source.task_plan().definition().class(),
            meter,
        )?;
        check(
            Field::Priority,
            row.priority == metadata.priority().0,
            meter,
        )?;
        text(
            Field::CancelScope,
            &executable.strings[row.cancel_scope.index()],
            &metadata.cancel_scope().0,
            meter,
        )?;
        check(
            Field::Policy,
            matches!(
                (row.policy, metadata.policy()),
                (AwbcTaskPolicy::JoinSameKey, TaskPolicy::JoinSameKey)
                    | (AwbcTaskPolicy::AlwaysStart, TaskPolicy::AlwaysStart)
            ),
            meter,
        )?;
        check(
            Field::Restart,
            matches!(
                (*restart, metadata.restart()),
                (
                    AwbcTaskRestartPolicy::MustBeQuiescent,
                    HostRestartPolicy::MustBeQuiescent
                ) | (
                    AwbcTaskRestartPolicy::Restartable,
                    HostRestartPolicy::Restartable
                )
            ),
            meter,
        )?;
        let args = target.arguments();
        let signature = &executable.signatures[row.signature.index()];
        check(
            Field::ArgumentCount,
            args.len() == row.arguments.len()
                && signature.params.len() == args.len()
                && signature.result.is_none(),
            meter,
        )?;
        for (index, (expected, actual)) in args.iter().zip(&row.arguments).enumerate() {
            let actual_name = actual
                .name
                .map(|name| executable.strings[name.index()].as_str());
            if let (Some(actual), Some(expected)) = (actual_name, expected.name()) {
                text(Field::ArgumentName { index }, actual, expected, meter)?;
            } else {
                check(
                    Field::ArgumentName { index },
                    actual_name == expected.name(),
                    meter,
                )?;
            }
            check(
                Field::ArgumentSpread { index },
                actual.spread == expected.is_spread(),
                meter,
            )?;
        }
        let payload = original
            .type_table()
            .id_for_semantic(metadata.payload_type())
            .expect("the common seal admitted the original selected payload");
        let mut types = RuntimeAwbcSourceTypeRelation::new(original, executable, limits, meter);
        types.admit(payload, row.payload_type).map_err(|source| {
            RuntimeAwbcSelectedSourceError::type_relation(task, Field::Payload, source)
        })?;
        for (index, (expected, actual)) in args.iter().zip(&signature.params).enumerate() {
            types
                .admit(expected.value().ty(), *actual)
                .map_err(|source| {
                    RuntimeAwbcSelectedSourceError::type_relation(
                        task,
                        Field::ArgumentType { index },
                        source,
                    )
                })?;
        }
        Ok(source)
    }
}
