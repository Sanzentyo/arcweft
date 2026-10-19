//! Existing AWBC `HostCall` assertions resolve only through their original Source.

use super::types::{RuntimeAwbcSourceTypeError, RuntimeAwbcSourceTypeRelation};
use super::{RuntimeTaskSourceProgram, RuntimeTaskSourceProgramData};
use crate::awbc::schema::{AwbcHostCallId, AwbcHostCallMode};
use crate::plan::RuntimeTaskPlanSealLimits;
use crate::plan::body_semantic::flow::RuntimeBodyTaskSource;
use crate::plan::task_source::{RuntimeTaskSourceError, RuntimeTaskSourceRef};
use crate::step::RuntimeHostCallMode;
use crate::task::ExpectedTaskPlanKey;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeAwbcHostSourceField {
    RequestKind,
    ProducerContract,
    ProducerSite,
    PublicId,
    Capability,
    Operation,
    ManifestContract,
    ResultType,
    Mode,
    Deterministic,
    ArgumentCount,
    ArgumentName { index: usize },
    ArgumentSpread { index: usize },
    ArgumentType { index: usize },
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum RuntimeAwbcHostSourceError {
    #[error("HostCall Source admission requires the actual AWBC execution context")]
    Backend,
    #[error("AWBC HostCall {call:?} is absent from its actual executable")]
    MissingCall { call: AwbcHostCallId },
    #[error("AWBC HostCall {call:?} has {actual} argument roles; original Source allows {maximum}")]
    ArgumentRoles {
        call: AwbcHostCallId,
        actual: usize,
        maximum: u32,
    },
    #[error("AWBC HostCall {call:?} cannot resolve its original Source: {source}")]
    Source {
        call: AwbcHostCallId,
        #[source]
        source: RuntimeTaskSourceError,
    },
    #[error("AWBC HostCall {call:?} differs from its original Source in {field:?}")]
    Mismatch {
        call: AwbcHostCallId,
        field: RuntimeAwbcHostSourceField,
    },
    #[error(
        "AWBC HostCall {call:?} differs from its original executable type in {field:?}: {source}"
    )]
    Type {
        call: AwbcHostCallId,
        field: RuntimeAwbcHostSourceField,
        #[source]
        source: RuntimeAwbcSourceTypeError,
    },
    #[error("AWBC HostCall Source admission exhausted its original semantic allowance: {0:?}")]
    Encoding(TaskSemanticEncodingError),
}

impl From<TaskSemanticEncodingError> for RuntimeAwbcHostSourceError {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Encoding(error)
    }
}

impl RuntimeAwbcHostSourceError {
    fn type_relation(
        call: AwbcHostCallId,
        field: RuntimeAwbcHostSourceField,
        source: RuntimeAwbcSourceTypeError,
    ) -> Self {
        match source {
            RuntimeAwbcSourceTypeError::Encoding(error) => Self::Encoding(error),
            source => Self::Type {
                call,
                field,
                source,
            },
        }
    }
}

impl RuntimeTaskSourceProgram {
    /// Checks the existing code-row assertion and its entire operand/result ABI
    /// against the actual original endpoint. This borrows both immutable owners;
    /// it neither reconstructs a request nor copies a lease or issues a key.
    pub(crate) fn awbc_host_source(
        &self,
        call: AwbcHostCallId,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeTaskSourceRef<'_>, RuntimeAwbcHostSourceError> {
        let result = self.admit_awbc_host_source(call, limits, meter);
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one ordered assertion boundary preserves original-key and field-error precedence under the caller's meter"
    )]
    fn admit_awbc_host_source(
        &self,
        call: AwbcHostCallId,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeTaskSourceRef<'_>, RuntimeAwbcHostSourceError> {
        use RuntimeAwbcHostSourceField as Field;
        meter.constrain(limits.max_semantic_work, limits.max_transcript_bytes)?;
        meter.charge_work(1)?;
        let RuntimeTaskSourceProgramData::Awbc {
            executable,
            original,
        } = self.data.as_ref()
        else {
            return Err(RuntimeAwbcHostSourceError::Backend);
        };
        let row = executable
            .host_calls
            .get(call.index())
            .ok_or(RuntimeAwbcHostSourceError::MissingCall { call })?;
        let actual = meter.checked_count_sum(row.arguments.len(), 0)?;
        if actual > limits.max_request_roles as usize {
            return Err(RuntimeAwbcHostSourceError::ArgumentRoles {
                call,
                actual,
                maximum: limits.max_request_roles,
            });
        }
        // The decoded row's digest is assertion data. Only this original plan's
        // already sealed table can supply the actual completed key and endpoint.
        let source = original
            .task_source_by_expected_key(
                &ExpectedTaskPlanKey::new(*row.producer.plan.as_bytes()),
                limits,
                meter,
            )
            .map_err(|source| RuntimeAwbcHostSourceError::Source { call, source })?;
        let Some(RuntimeBodyTaskSource::Host(host)) = source.request_endpoint().task_source()
        else {
            return Err(RuntimeAwbcHostSourceError::Mismatch {
                call,
                field: Field::RequestKind,
            });
        };
        let actual = meter.checked_count_sum(host.args.len(), 0)?;
        if actual > limits.max_request_roles as usize {
            return Err(RuntimeAwbcHostSourceError::ArgumentRoles {
                call,
                actual,
                maximum: limits.max_request_roles,
            });
        }
        let check = |field, equal, meter: &mut TaskSemanticMeter| {
            meter.charge_work(1)?;
            if equal {
                Ok(())
            } else {
                Err(RuntimeAwbcHostSourceError::Mismatch { call, field })
            }
        };
        check(
            Field::ProducerContract,
            row.producer.contract == host.producer.contract,
            meter,
        )?;
        check(
            Field::ProducerSite,
            row.producer.site == host.producer.site,
            meter,
        )?;
        for (field, actual, expected) in [
            (Field::PublicId, row.public_id, host.public_id.as_str()),
            (Field::Capability, row.capability, host.capability.as_str()),
            (Field::Operation, row.operation, host.operation.as_str()),
        ] {
            let actual = executable.strings[actual.index()].as_str();
            let bytes = meter.checked_count_sum(actual.len(), expected.len())?;
            meter.charge_work((bytes as u64).div_ceil(32))?;
            check(field, actual == expected, meter)?;
        }
        check(
            Field::ManifestContract,
            row.contract == host.contract,
            meter,
        )?;
        let signature = &executable.signatures[row.signature.index()];
        let expected_result = original
            .type_table()
            .get(host.result)
            .expect("the common seal admitted the original host result")
            .semantic_identity();
        check(
            Field::ResultType,
            signature.result.is_some_and(|ty| {
                executable.runtime_types[ty.index()].semantic_identity() == expected_result
            }),
            meter,
        )?;
        let mode = match host.mode {
            RuntimeHostCallMode::Immediate => AwbcHostCallMode::Immediate,
            RuntimeHostCallMode::Suspend => AwbcHostCallMode::Suspend,
        };
        check(Field::Mode, row.mode == mode, meter)?;
        check(
            Field::Deterministic,
            row.deterministic == host.deterministic,
            meter,
        )?;
        check(
            Field::ArgumentCount,
            row.arguments.len() == host.args.len() && signature.params.len() == host.args.len(),
            meter,
        )?;
        for (index, ((actual, ty), expected)) in row
            .arguments
            .iter()
            .zip(&signature.params)
            .zip(&host.args)
            .enumerate()
        {
            let actual_name = actual
                .name
                .map(|name| executable.strings[name.index()].as_str());
            let expected_name = expected.name();
            if let (Some(actual), Some(expected)) = (actual_name, expected_name) {
                let bytes = meter.checked_count_sum(actual.len(), expected.len())?;
                meter.charge_work((bytes as u64).div_ceil(32))?;
            }
            check(
                Field::ArgumentName { index },
                actual_name == expected_name,
                meter,
            )?;
            check(
                Field::ArgumentSpread { index },
                actual.spread == expected.is_spread(),
                meter,
            )?;
            let original_type = original
                .type_table()
                .get(expected.value().ty())
                .expect("the common seal admitted every original operand type")
                .semantic_identity();
            check(
                Field::ArgumentType { index },
                executable.runtime_types[ty.index()].semantic_identity() == original_type,
                meter,
            )?;
        }
        let mut types = RuntimeAwbcSourceTypeRelation::new(original, executable, limits, meter);
        types
            .admit(
                host.result,
                signature
                    .result
                    .expect("the original result identity was checked"),
            )
            .map_err(|source| {
                RuntimeAwbcHostSourceError::type_relation(call, Field::ResultType, source)
            })?;
        for (index, (expected, actual)) in host.args.iter().zip(&signature.params).enumerate() {
            types
                .admit(expected.value().ty(), *actual)
                .map_err(|source| {
                    RuntimeAwbcHostSourceError::type_relation(
                        call,
                        Field::ArgumentType { index },
                        source,
                    )
                })?;
        }
        Ok(source)
    }
}
