//! Explicit recursive AWBC session-save projection for live runtime values.
//!
//! `RuntimeValue` deliberately rejects generic serde for function values.  A
//! quiescent AWBC session is the sole exception, and it uses the typed DTOs in
//! this module rather than serializing or deserializing the live value graph.

use super::{
    AgentPredicateOperands, DenseSeq, RecordSeq, RuntimeAgentActionTarget,
    RuntimeAgentCaptureTarget, RuntimeAgentCompareOp, RuntimeAgentConstructionError,
    RuntimeAgentPath, RuntimeAgentPredicate, RuntimeAgentProbe, RuntimeAgentValue, RuntimeBinding,
    RuntimeCommand, RuntimeFunctionBody, RuntimeFunctionValue, RuntimeIterator,
    RuntimeNominalRecordValue, RuntimeOpaqueValue, RuntimePayload, RuntimeReductionValue,
    RuntimeSeq, RuntimeValue, TupleSeq,
};
use crate::awbc::schema::AwbcFunctionId;
use crate::entry::{RuntimeCommandConstructorId, RuntimeCommandTargetId};
use crate::pattern::RuntimeOpaqueTypeOwner;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AwbcRuntimeValueSnapshotError {
    #[error("{message}")]
    Message { message: String },
    #[error("invalid Agent predicate: {0}")]
    AgentPredicate(#[from] RuntimeAgentConstructionError),
}

impl AwbcRuntimeValueSnapshotError {
    fn new(message: impl Into<String>) -> Self {
        Self::Message {
            message: message.into(),
        }
    }
}

/// Typed recursive AWBC session-save representation of one live value.
///
/// The function variant is admitted only for AWBC closures.  Structured
/// closures retain an owning plan and are rejected at projection time.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcRuntimeValueSnapshot {
    Unit,
    Bool(bool),
    Int(super::RuntimeInt),
    UInt(super::RuntimeUInt),
    F32(f32),
    F64(f64),
    MatrixF32(crate::math::DenseMatrixF32),
    MatrixF64(crate::math::DenseMatrixF64),
    TensorF32(crate::math::DenseTensorF32),
    TensorF64(crate::math::DenseTensorF64),
    String(String),
    Char(char),
    Duration(crate::time::LogicalDuration),
    Progress {
        ratio: f32,
        label: Option<String>,
    },
    Range(super::RuntimeRange),
    Iterator(AwbcRuntimeIteratorSnapshot),
    EntityRef(String),
    Tuple(Vec<Self>),
    Seq(AwbcRuntimeSeqSnapshot),
    Record(Vec<AwbcRuntimeFieldSnapshot>),
    NominalRecord(AwbcRuntimeNominalRecordSnapshot),
    Opaque(AwbcRuntimeOpaqueSnapshot),
    Reduction(AwbcRuntimeReductionSnapshot),
    Agent(AwbcRuntimeAgentSnapshot),
    Function(AwbcRuntimeFunctionSnapshot),
    Variant {
        owner: super::RuntimeVariantIdentity,
        ordinal: u32,
        name: String,
        payload: Option<Box<Self>>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcRuntimeIteratorSnapshot {
    Values {
        items: Vec<AwbcRuntimeValueSnapshot>,
        index: u64,
    },
    Range(super::RuntimeRangeIterator),
    Witness {
        state: Box<AwbcRuntimeValueSnapshot>,
        next: crate::plan::RuntimeTraitMethodId,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcRuntimeSeqSnapshot {
    Values(Vec<AwbcRuntimeValueSnapshot>),
    Dense(DenseSeq),
    TupleColumns {
        len: u64,
        columns: Vec<AwbcRuntimeSeqSnapshot>,
    },
    RecordColumns {
        len: u64,
        fields: Vec<AwbcRuntimeRecordSeqFieldSnapshot>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcRuntimeRecordSeqFieldSnapshot {
    pub field: super::RuntimeRecordFieldId,
    pub name: String,
    pub values: AwbcRuntimeSeqSnapshot,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcRuntimeFieldSnapshot {
    pub field: super::RuntimeRecordFieldId,
    pub name: String,
    pub value: AwbcRuntimeValueSnapshot,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcRuntimeNominalRecordSnapshot {
    pub type_id: crate::entry::RuntimeNominalTypeId,
    pub layout: crate::entry::TypeLayoutHash,
    pub fields: Vec<AwbcRuntimeValueSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcRuntimeOpaqueSnapshot {
    pub producer: crate::pattern::RuntimeOpaqueTypeProducerId,
    pub semantic_identity: crate::pattern::RuntimeSemanticTypeId,
    pub value_class: super::RuntimeOpaqueValueClass,
    pub persistence: super::RuntimeOpaquePersistence,
    pub payload: Box<AwbcRuntimeValueSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcRuntimeReductionSnapshot {
    pub owner: RuntimeOpaqueTypeOwner,
    pub state: Box<AwbcRuntimeValueSnapshot>,
    pub commands: Vec<AwbcRuntimeCommandSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcRuntimeCommandSnapshot {
    pub constructor: RuntimeCommandConstructorId,
    pub target: RuntimeCommandTargetId,
    pub payload: Box<AwbcRuntimeValueSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcRuntimeAgentSnapshot {
    ActionTarget(RuntimeAgentActionTarget),
    CaptureTarget(RuntimeAgentCaptureTarget),
    DebugStatePath(RuntimeAgentPath),
    ObservationFieldPath(RuntimeAgentPath),
    Probe(RuntimeAgentProbe),
    Diagnostics,
    Predicate(AwbcRuntimeAgentPredicateSnapshot),
    ViewportPoint { x: u32, y: u32 },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcRuntimeAgentPredicateSnapshot {
    Compare {
        probe: RuntimeAgentProbe,
        op: RuntimeAgentCompareOp,
        value: Box<AwbcRuntimeValueSnapshot>,
    },
    Exists {
        probe: RuntimeAgentProbe,
    },
    ActionEnabled {
        target: RuntimeCommandTargetId,
    },
    DiagnosticsHasError,
    All {
        predicates: AgentPredicateOperands<AwbcRuntimeAgentPredicateSnapshot>,
    },
    Any {
        predicates: AgentPredicateOperands<AwbcRuntimeAgentPredicateSnapshot>,
    },
    Not {
        predicate: Box<AwbcRuntimeAgentPredicateSnapshot>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcRuntimeFunctionSnapshot {
    pub function: AwbcFunctionId,
    pub remaining_params: Vec<String>,
    pub captures: Vec<AwbcRuntimeBindingSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcRuntimeBindingSnapshot {
    pub name: String,
    pub value: Box<AwbcRuntimeValueSnapshot>,
}

impl AwbcRuntimeValueSnapshot {
    pub fn from_runtime_value(value: &RuntimeValue) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        Ok(match value {
            RuntimeValue::Unit => Self::Unit,
            RuntimeValue::Bool(value) => Self::Bool(*value),
            RuntimeValue::Int(value) => Self::Int(*value),
            RuntimeValue::UInt(value) => Self::UInt(*value),
            RuntimeValue::F32(value) => Self::F32(*value),
            RuntimeValue::F64(value) => Self::F64(*value),
            RuntimeValue::MatrixF32(value) => Self::MatrixF32(value.clone()),
            RuntimeValue::MatrixF64(value) => Self::MatrixF64(value.clone()),
            RuntimeValue::TensorF32(value) => Self::TensorF32(value.clone()),
            RuntimeValue::TensorF64(value) => Self::TensorF64(value.clone()),
            RuntimeValue::String(value) => Self::String(value.clone()),
            RuntimeValue::Char(value) => Self::Char(*value),
            RuntimeValue::Duration(value) => Self::Duration(*value),
            RuntimeValue::Progress(value) => Self::Progress {
                ratio: value.ratio(),
                label: value.label().map(str::to_owned),
            },
            RuntimeValue::Range(value) => Self::Range(value.clone()),
            RuntimeValue::Iterator(value) => Self::Iterator(Self::iterator_from_live(value)?),
            RuntimeValue::EntityRef(value) => Self::EntityRef(value.clone()),
            RuntimeValue::Tuple(values) => Self::Tuple(
                values
                    .iter()
                    .map(Self::from_runtime_value)
                    .collect::<Result<_, _>>()?,
            ),
            RuntimeValue::Seq(value) => Self::Seq(Self::sequence_from_live(value)?),
            RuntimeValue::Record(fields) => Self::Record(
                fields
                    .iter()
                    .map(|field| {
                        Ok(AwbcRuntimeFieldSnapshot {
                            field: field.field(),
                            name: field.name().to_owned(),
                            value: Self::from_runtime_value(field.value())?,
                        })
                    })
                    .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
            ),
            RuntimeValue::NominalRecord(value) => {
                Self::NominalRecord(Self::nominal_from_live(value)?)
            }
            RuntimeValue::Opaque(value) => Self::Opaque(Self::opaque_from_live(value)?),
            RuntimeValue::Reduction(value) => Self::Reduction(Self::reduction_from_live(value)?),
            RuntimeValue::Agent(value) => Self::Agent(Self::agent_from_live(value)?),
            RuntimeValue::Function(value) => Self::Function(Self::function_from_live(value)?),
            RuntimeValue::Variant {
                owner,
                ordinal,
                name,
                payload,
            } => Self::Variant {
                owner: owner.clone(),
                ordinal: *ordinal,
                name: name.clone(),
                payload: payload
                    .as_deref()
                    .map(Self::from_runtime_value)
                    .transpose()?
                    .map(Box::new),
            },
        })
    }

    pub fn into_runtime_value(self) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        Ok(match self {
            Self::Unit => RuntimeValue::Unit,
            Self::Bool(value) => RuntimeValue::Bool(value),
            Self::Int(value) => RuntimeValue::Int(value),
            Self::UInt(value) => RuntimeValue::UInt(value),
            Self::F32(value) => RuntimeValue::F32(value),
            Self::F64(value) => RuntimeValue::F64(value),
            Self::MatrixF32(value) => RuntimeValue::MatrixF32(value),
            Self::MatrixF64(value) => RuntimeValue::MatrixF64(value),
            Self::TensorF32(value) => RuntimeValue::TensorF32(value),
            Self::TensorF64(value) => RuntimeValue::TensorF64(value),
            Self::String(value) => RuntimeValue::String(value),
            Self::Char(value) => RuntimeValue::Char(value),
            Self::Duration(value) => RuntimeValue::Duration(value),
            Self::Progress { ratio, label } => {
                let progress = crate::value::Progress::new(ratio)
                    .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?;
                RuntimeValue::Progress(match label {
                    Some(label) => progress.with_label(label),
                    None => progress,
                })
            }
            Self::Range(value) => RuntimeValue::Range(value),
            Self::Iterator(value) => RuntimeValue::Iterator(Self::iterator_into_live(value)?),
            Self::EntityRef(value) => RuntimeValue::EntityRef(value),
            Self::Tuple(values) => RuntimeValue::Tuple(
                values
                    .into_iter()
                    .map(Self::into_runtime_value)
                    .collect::<Result<_, _>>()?,
            ),
            Self::Seq(value) => RuntimeValue::Seq(Self::sequence_into_live(value)?),
            Self::Record(fields) => RuntimeValue::Record(
                fields
                    .into_iter()
                    .enumerate()
                    .map(|(ordinal, field)| {
                        let expected =
                            super::RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)
                                .map_err(|error| {
                                    AwbcRuntimeValueSnapshotError::new(error.to_string())
                                })?;
                        if field.field != expected {
                            return Err(AwbcRuntimeValueSnapshotError::new(
                                "AWBC record field identity does not match its ordinal",
                            ));
                        }
                        Ok(super::RuntimeFieldValue::new_accepted(
                            field.field,
                            field.name,
                            field.value.into_runtime_value()?,
                        ))
                    })
                    .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
            ),
            Self::NominalRecord(value) => {
                RuntimeValue::NominalRecord(Self::nominal_into_live(value)?)
            }
            Self::Opaque(value) => Self::opaque_into_live(value)?,
            Self::Reduction(value) => RuntimeValue::Reduction(Self::reduction_into_live(value)?),
            Self::Agent(value) => RuntimeValue::Agent(Self::agent_into_live(value)?),
            Self::Function(value) => RuntimeValue::Function(Self::function_into_live(value)?),
            Self::Variant {
                owner,
                ordinal,
                name,
                payload,
            } => RuntimeValue::Variant {
                owner,
                ordinal,
                name,
                payload: payload
                    .map(|value| value.into_runtime_value().map(Box::new))
                    .transpose()?,
            },
        })
    }

    fn iterator_from_live(
        value: &RuntimeIterator,
    ) -> Result<AwbcRuntimeIteratorSnapshot, AwbcRuntimeValueSnapshotError> {
        Ok(match value {
            RuntimeIterator::Values { items, index } => AwbcRuntimeIteratorSnapshot::Values {
                items: items
                    .iter()
                    .map(Self::from_runtime_value)
                    .collect::<Result<_, _>>()?,
                index: u64::try_from(*index).map_err(|_| {
                    AwbcRuntimeValueSnapshotError::new(
                        "runtime iterator index does not fit the AWBC save field",
                    )
                })?,
            },
            RuntimeIterator::Range(value) => AwbcRuntimeIteratorSnapshot::Range(value.clone()),
            RuntimeIterator::Witness { state, next } => AwbcRuntimeIteratorSnapshot::Witness {
                state: Box::new(Self::from_runtime_value(state)?),
                next: *next,
            },
        })
    }

    fn iterator_into_live(
        value: AwbcRuntimeIteratorSnapshot,
    ) -> Result<RuntimeIterator, AwbcRuntimeValueSnapshotError> {
        Ok(match value {
            AwbcRuntimeIteratorSnapshot::Values { items, index } => RuntimeIterator::Values {
                items: items
                    .into_iter()
                    .map(Self::into_runtime_value)
                    .collect::<Result<_, _>>()?,
                index: usize::try_from(index).map_err(|_| {
                    AwbcRuntimeValueSnapshotError::new(
                        "AWBC iterator index does not fit this platform",
                    )
                })?,
            },
            AwbcRuntimeIteratorSnapshot::Range(value) => RuntimeIterator::Range(value),
            AwbcRuntimeIteratorSnapshot::Witness { state, next } => RuntimeIterator::Witness {
                state: Box::new(state.into_runtime_value()?),
                next,
            },
        })
    }

    fn sequence_from_live(
        value: &RuntimeSeq,
    ) -> Result<AwbcRuntimeSeqSnapshot, AwbcRuntimeValueSnapshotError> {
        Ok(match value {
            RuntimeSeq::Values(values) => AwbcRuntimeSeqSnapshot::Values(
                values
                    .iter()
                    .map(Self::from_runtime_value)
                    .collect::<Result<_, _>>()?,
            ),
            RuntimeSeq::Dense(value) => AwbcRuntimeSeqSnapshot::Dense(value.clone()),
            RuntimeSeq::TupleColumns(value) => AwbcRuntimeSeqSnapshot::TupleColumns {
                len: u64::try_from(value.len()).map_err(|_| {
                    AwbcRuntimeValueSnapshotError::new(
                        "tuple sequence length does not fit the AWBC save field",
                    )
                })?,
                columns: value
                    .columns()
                    .iter()
                    .map(Self::sequence_from_live)
                    .collect::<Result<_, _>>()?,
            },
            RuntimeSeq::RecordColumns(value) => AwbcRuntimeSeqSnapshot::RecordColumns {
                len: u64::try_from(value.len()).map_err(|_| {
                    AwbcRuntimeValueSnapshotError::new(
                        "record sequence length does not fit the AWBC save field",
                    )
                })?,
                fields: value
                    .fields()
                    .iter()
                    .map(|field| {
                        Ok(AwbcRuntimeRecordSeqFieldSnapshot {
                            field: field.field(),
                            name: field.name().to_owned(),
                            values: Self::sequence_from_live(field.values())?,
                        })
                    })
                    .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
            },
        })
    }

    fn sequence_into_live(
        value: AwbcRuntimeSeqSnapshot,
    ) -> Result<RuntimeSeq, AwbcRuntimeValueSnapshotError> {
        Ok(match value {
            AwbcRuntimeSeqSnapshot::Values(values) => RuntimeSeq::Values(
                values
                    .into_iter()
                    .map(Self::into_runtime_value)
                    .collect::<Result<_, _>>()?,
            ),
            AwbcRuntimeSeqSnapshot::Dense(value) => RuntimeSeq::Dense(value),
            AwbcRuntimeSeqSnapshot::TupleColumns { len, columns } => RuntimeSeq::TupleColumns(
                TupleSeq::new(
                    usize::try_from(len).map_err(|_| {
                        AwbcRuntimeValueSnapshotError::new(
                            "AWBC tuple sequence length does not fit this platform",
                        )
                    })?,
                    columns
                        .into_iter()
                        .map(Self::sequence_into_live)
                        .collect::<Result<_, _>>()?,
                )
                .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?,
            ),
            AwbcRuntimeSeqSnapshot::RecordColumns { len, fields } => RuntimeSeq::RecordColumns(
                RecordSeq::try_from_accepted_fields(
                    usize::try_from(len).map_err(|_| {
                        AwbcRuntimeValueSnapshotError::new(
                            "AWBC record sequence length does not fit this platform",
                        )
                    })?,
                    fields
                        .into_iter()
                        .enumerate()
                        .map(|(ordinal, field)| {
                            let expected = super::RuntimeRecordFieldId::try_from_zero_based_ordinal(
                                ordinal,
                            )
                            .map_err(|error| {
                                AwbcRuntimeValueSnapshotError::new(error.to_string())
                            })?;
                            if field.field != expected {
                                return Err(AwbcRuntimeValueSnapshotError::new(
                                    "AWBC record sequence field identity does not match its ordinal",
                                ));
                            }
                            Ok((field.name, Self::sequence_into_live(field.values)?))
                        })
                        .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
                )
                .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?,
            ),
        })
    }

    fn nominal_from_live(
        value: &RuntimeNominalRecordValue,
    ) -> Result<AwbcRuntimeNominalRecordSnapshot, AwbcRuntimeValueSnapshotError> {
        Ok(AwbcRuntimeNominalRecordSnapshot {
            type_id: value.type_id().clone(),
            layout: value.layout(),
            fields: value
                .fields()
                .iter()
                .map(Self::from_runtime_value)
                .collect::<Result<_, _>>()?,
        })
    }

    fn nominal_into_live(
        value: AwbcRuntimeNominalRecordSnapshot,
    ) -> Result<RuntimeNominalRecordValue, AwbcRuntimeValueSnapshotError> {
        Ok(RuntimeNominalRecordValue::new(
            value.type_id,
            value.layout,
            value
                .fields
                .into_iter()
                .map(Self::into_runtime_value)
                .collect::<Result<_, _>>()?,
        ))
    }

    fn opaque_from_live(
        value: &RuntimeOpaqueValue,
    ) -> Result<AwbcRuntimeOpaqueSnapshot, AwbcRuntimeValueSnapshotError> {
        Ok(AwbcRuntimeOpaqueSnapshot {
            producer: value.producer().clone(),
            semantic_identity: value.semantic_identity(),
            value_class: value.value_class(),
            persistence: value.persistence(),
            payload: Box::new(Self::from_runtime_value(value.payload())?),
        })
    }

    fn opaque_into_live(
        value: AwbcRuntimeOpaqueSnapshot,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        let owner = RuntimeOpaqueTypeOwner::exact_with(
            value.producer,
            value.semantic_identity,
            value.value_class,
            value.persistence,
        );
        owner
            .try_wrap((*value.payload).into_runtime_value()?)
            .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))
    }

    fn reduction_from_live(
        value: &RuntimeReductionValue,
    ) -> Result<AwbcRuntimeReductionSnapshot, AwbcRuntimeValueSnapshotError> {
        Ok(AwbcRuntimeReductionSnapshot {
            owner: value.owner().clone(),
            state: Box::new(Self::from_runtime_value(value.state())?),
            commands: value
                .commands()
                .iter()
                .map(|command| {
                    Ok(AwbcRuntimeCommandSnapshot {
                        constructor: command.constructor().clone(),
                        target: command.target().clone(),
                        payload: Box::new(Self::from_runtime_value(command.payload().value())?),
                    })
                })
                .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
        })
    }

    fn reduction_into_live(
        value: AwbcRuntimeReductionSnapshot,
    ) -> Result<RuntimeReductionValue, AwbcRuntimeValueSnapshotError> {
        let commands = value
            .commands
            .into_iter()
            .map(|command| {
                Ok(RuntimeCommand::new_accepted(
                    command.constructor,
                    command.target,
                    RuntimePayload::new((*command.payload).into_runtime_value()?),
                ))
            })
            .collect::<Result<Vec<_>, AwbcRuntimeValueSnapshotError>>()?;
        RuntimeReductionValue::try_from_admitted_parts(
            value.owner,
            (*value.state).into_runtime_value()?,
            commands,
        )
        .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))
    }

    fn agent_from_live(
        value: &RuntimeAgentValue,
    ) -> Result<AwbcRuntimeAgentSnapshot, AwbcRuntimeValueSnapshotError> {
        Ok(match value {
            RuntimeAgentValue::ActionTarget(value) => {
                AwbcRuntimeAgentSnapshot::ActionTarget(value.clone())
            }
            RuntimeAgentValue::CaptureTarget(value) => {
                AwbcRuntimeAgentSnapshot::CaptureTarget(value.clone())
            }
            RuntimeAgentValue::DebugStatePath(value) => {
                AwbcRuntimeAgentSnapshot::DebugStatePath(value.clone())
            }
            RuntimeAgentValue::ObservationFieldPath(value) => {
                AwbcRuntimeAgentSnapshot::ObservationFieldPath(value.clone())
            }
            RuntimeAgentValue::Probe(value) => AwbcRuntimeAgentSnapshot::Probe(value.clone()),
            RuntimeAgentValue::Diagnostics => AwbcRuntimeAgentSnapshot::Diagnostics,
            RuntimeAgentValue::Predicate(value) => {
                AwbcRuntimeAgentSnapshot::Predicate(Self::predicate_from_live(value)?)
            }
            RuntimeAgentValue::ViewportPoint { x, y } => {
                AwbcRuntimeAgentSnapshot::ViewportPoint { x: *x, y: *y }
            }
        })
    }

    fn agent_into_live(
        value: AwbcRuntimeAgentSnapshot,
    ) -> Result<RuntimeAgentValue, AwbcRuntimeValueSnapshotError> {
        Ok(match value {
            AwbcRuntimeAgentSnapshot::ActionTarget(value) => RuntimeAgentValue::ActionTarget(value),
            AwbcRuntimeAgentSnapshot::CaptureTarget(value) => {
                RuntimeAgentValue::CaptureTarget(value)
            }
            AwbcRuntimeAgentSnapshot::DebugStatePath(value) => {
                RuntimeAgentValue::DebugStatePath(value)
            }
            AwbcRuntimeAgentSnapshot::ObservationFieldPath(value) => {
                RuntimeAgentValue::ObservationFieldPath(value)
            }
            AwbcRuntimeAgentSnapshot::Probe(value) => RuntimeAgentValue::Probe(value),
            AwbcRuntimeAgentSnapshot::Diagnostics => RuntimeAgentValue::Diagnostics,
            AwbcRuntimeAgentSnapshot::Predicate(value) => {
                RuntimeAgentValue::Predicate(Self::predicate_into_live(value)?)
            }
            AwbcRuntimeAgentSnapshot::ViewportPoint { x, y } => {
                RuntimeAgentValue::ViewportPoint { x, y }
            }
        })
    }

    fn predicate_from_live(
        value: &RuntimeAgentPredicate,
    ) -> Result<AwbcRuntimeAgentPredicateSnapshot, AwbcRuntimeValueSnapshotError> {
        Ok(match value {
            RuntimeAgentPredicate::Compare { probe, op, value } => {
                AwbcRuntimeAgentPredicateSnapshot::Compare {
                    probe: probe.clone(),
                    op: *op,
                    value: Box::new(Self::from_runtime_value(value)?),
                }
            }
            RuntimeAgentPredicate::Exists { probe } => AwbcRuntimeAgentPredicateSnapshot::Exists {
                probe: probe.clone(),
            },
            RuntimeAgentPredicate::ActionEnabled { target } => {
                AwbcRuntimeAgentPredicateSnapshot::ActionEnabled {
                    target: target.clone(),
                }
            }
            RuntimeAgentPredicate::DiagnosticsHasError => {
                AwbcRuntimeAgentPredicateSnapshot::DiagnosticsHasError
            }
            RuntimeAgentPredicate::All { predicates } => AwbcRuntimeAgentPredicateSnapshot::All {
                predicates: AgentPredicateOperands::try_from(
                    predicates
                        .iter()
                        .map(Self::predicate_from_live)
                        .collect::<Result<Vec<_>, _>>()?,
                )
                .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?,
            },
            RuntimeAgentPredicate::Any { predicates } => AwbcRuntimeAgentPredicateSnapshot::Any {
                predicates: AgentPredicateOperands::try_from(
                    predicates
                        .iter()
                        .map(Self::predicate_from_live)
                        .collect::<Result<Vec<_>, _>>()?,
                )
                .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?,
            },
            RuntimeAgentPredicate::Not { predicate } => AwbcRuntimeAgentPredicateSnapshot::Not {
                predicate: Box::new(Self::predicate_from_live(predicate)?),
            },
        })
    }

    fn predicate_into_live(
        value: AwbcRuntimeAgentPredicateSnapshot,
    ) -> Result<RuntimeAgentPredicate, AwbcRuntimeValueSnapshotError> {
        Ok(match value {
            AwbcRuntimeAgentPredicateSnapshot::Compare { probe, op, value } => {
                RuntimeAgentPredicate::Compare {
                    probe,
                    op,
                    value: Box::new((*value).into_runtime_value()?),
                }
            }
            AwbcRuntimeAgentPredicateSnapshot::Exists { probe } => {
                RuntimeAgentPredicate::Exists { probe }
            }
            AwbcRuntimeAgentPredicateSnapshot::ActionEnabled { target } => {
                RuntimeAgentPredicate::ActionEnabled { target }
            }
            AwbcRuntimeAgentPredicateSnapshot::DiagnosticsHasError => {
                RuntimeAgentPredicate::DiagnosticsHasError
            }
            AwbcRuntimeAgentPredicateSnapshot::All { predicates } => {
                RuntimeAgentPredicate::try_all(
                    predicates
                        .into_iter()
                        .map(Self::predicate_into_live)
                        .collect::<Result<_, _>>()?,
                )?
            }
            AwbcRuntimeAgentPredicateSnapshot::Any { predicates } => {
                RuntimeAgentPredicate::try_any(
                    predicates
                        .into_iter()
                        .map(Self::predicate_into_live)
                        .collect::<Result<_, _>>()?,
                )?
            }
            AwbcRuntimeAgentPredicateSnapshot::Not { predicate } => RuntimeAgentPredicate::Not {
                predicate: Box::new(Self::predicate_into_live(*predicate)?),
            },
        })
    }

    fn function_from_live(
        value: &RuntimeFunctionValue,
    ) -> Result<AwbcRuntimeFunctionSnapshot, AwbcRuntimeValueSnapshotError> {
        let RuntimeFunctionBody::Awbc(value) = value.body() else {
            return Err(AwbcRuntimeValueSnapshotError::new(
                "structured runtime functions cannot cross the AWBC session-save boundary",
            ));
        };
        Ok(AwbcRuntimeFunctionSnapshot {
            function: value.function(),
            remaining_params: value.remaining_params().to_vec(),
            captures: value
                .captures()
                .iter()
                .map(|binding| {
                    Ok(AwbcRuntimeBindingSnapshot {
                        name: binding.name.clone(),
                        value: Box::new(Self::from_runtime_value(&binding.value)?),
                    })
                })
                .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
        })
    }

    fn function_into_live(
        value: AwbcRuntimeFunctionSnapshot,
    ) -> Result<RuntimeFunctionValue, AwbcRuntimeValueSnapshotError> {
        Ok(RuntimeFunctionValue::new_awbc(
            value.remaining_params,
            value.function,
            value
                .captures
                .into_iter()
                .map(|binding| {
                    Ok(RuntimeBinding {
                        name: binding.name,
                        value: (*binding.value).into_runtime_value()?,
                    })
                })
                .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
        ))
    }
}

// Keep the public serde boundary deliberately local to the DTO.  These impls
// make accidental use in non-JSON codecs behave exactly like a regular typed
// serializable value while never invoking `RuntimeValue` serde.
impl Serialize for AwbcRuntimeValueSnapshotError {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Err(serde::ser::Error::custom(self.to_string()))
    }
}

impl<'de> Deserialize<'de> for AwbcRuntimeValueSnapshotError {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Err(serde::de::Error::custom(
            "AWBC runtime-value snapshot errors are not wire values",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn awbc_snapshot_deserialize_rejects_empty_all_and_any_predicates() {
        for value in [
            serde_json::json!({ "All": { "predicates": [] } }),
            serde_json::json!({ "Any": { "predicates": [] } }),
        ] {
            assert!(serde_json::from_value::<AwbcRuntimeAgentPredicateSnapshot>(value).is_err());
        }
    }

    #[test]
    fn awbc_snapshot_deserialize_rejects_nested_empty_predicates() {
        let value = serde_json::json!({
            "All": {
                "predicates": [{
                    "Any": {
                        "predicates": [],
                    },
                }],
            },
        });
        assert!(serde_json::from_value::<AwbcRuntimeAgentPredicateSnapshot>(value).is_err());
    }
}
