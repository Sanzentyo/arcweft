//! Borrowed logical values over ordinary, dense, and columnar storage.
//!
//! These views own no runtime payload and are not another value carrier.
//! Consumers inspect the same logical fields without cloning a sequence row
//! before its work and scalar limits have been checked.

use crate::pattern::RuntimeVariantIdentity;
use crate::time::LogicalDuration;

use super::{
    DenseSeq, Progress, RecordSeqField, RuntimeAgentValue, RuntimeColor, RuntimeEntityReference,
    RuntimeInt, RuntimeNominalRecordValue, RuntimeOpaqueValue, RuntimeRecordFieldId,
    RuntimeRecordValue, RuntimeReductionValue, RuntimeSeq, RuntimeUInt, RuntimeValue,
};

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeScalarView<'a> {
    Unit,
    Bool(bool),
    Int(RuntimeInt),
    UInt(RuntimeUInt),
    F32(f32),
    F64(f64),
    String(&'a str),
    Color(RuntimeColor),
    Char(char),
    Duration(LogicalDuration),
    Progress(&'a Progress),
    EntityRef(&'a RuntimeEntityReference),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeValueView<'a> {
    Scalar(RuntimeScalarView<'a>),
    Tuple(RuntimeTupleView<'a>),
    Record(RuntimeRecordView<'a>),
    Sequence(&'a RuntimeSeq),
    NominalRecord(&'a RuntimeNominalRecordValue),
    Opaque(&'a RuntimeOpaqueValue),
    Reduction(&'a RuntimeReductionValue),
    Agent(&'a RuntimeAgentValue),
    Variant {
        owner: &'a RuntimeVariantIdentity,
        ordinal: u32,
        name: &'a str,
        payload: Option<&'a RuntimeValue>,
    },
    RuntimeOnly(&'a RuntimeValue),
}

impl RuntimeValueView<'_> {
    pub(crate) fn type_name(self) -> &'static str {
        match self {
            Self::Scalar(RuntimeScalarView::Unit) => "unit",
            Self::Scalar(RuntimeScalarView::Bool(_)) => "bool",
            Self::Scalar(RuntimeScalarView::Int(_)) => "signed integer",
            Self::Scalar(RuntimeScalarView::UInt(_)) => "unsigned integer",
            Self::Scalar(RuntimeScalarView::F32(_)) => "f32",
            Self::Scalar(RuntimeScalarView::F64(_)) => "f64",
            Self::Scalar(RuntimeScalarView::String(_)) => "string",
            Self::Scalar(RuntimeScalarView::Color(_)) => "color",
            Self::Scalar(RuntimeScalarView::Char(_)) => "char",
            Self::Scalar(RuntimeScalarView::Duration(_)) => "duration",
            Self::Scalar(RuntimeScalarView::Progress(_)) => "progress",
            Self::Scalar(RuntimeScalarView::EntityRef(_)) => "entity reference",
            Self::Tuple(_) => "tuple",
            Self::Record(_) => "record",
            Self::Sequence(_) => "sequence",
            Self::NominalRecord(_) => "nominal record",
            Self::Opaque(_) => "opaque value",
            Self::Reduction(_) => "Reduction value",
            Self::Agent(_) => "Agent value",
            Self::Variant { .. } => "variant",
            Self::RuntimeOnly(value) => match value {
                RuntimeValue::MatrixF32(_) => "f32 matrix",
                RuntimeValue::MatrixF64(_) => "f64 matrix",
                RuntimeValue::TensorF32(_) => "f32 tensor",
                RuntimeValue::TensorF64(_) => "f64 tensor",
                RuntimeValue::Range(_) => "range",
                RuntimeValue::Iterator(_) => "iterator",
                RuntimeValue::Callable(_) => "function",
                _ => unreachable!("runtime-only views are issued for runtime-only values"),
            },
        }
    }

    /// Tests one node of the closed `AgentValue` algebra. Container descendants
    /// must satisfy the same rule; the caller owns traversal and its allowance.
    pub(crate) fn is_agent_value_node(self) -> bool {
        use RuntimeScalarView as Scalar;
        match self {
            Self::Scalar(
                Scalar::Unit
                | Scalar::Bool(_)
                | Scalar::String(_)
                | Scalar::EntityRef(_)
                | Scalar::Int(RuntimeInt::I64(_))
                | Scalar::UInt(RuntimeUInt::U64(_)),
            )
            | Self::Sequence(_)
            | Self::Record(_) => true,
            Self::Scalar(Scalar::F64(value)) => value.is_finite(),
            Self::Scalar(_)
            | Self::Tuple(_)
            | Self::NominalRecord(_)
            | Self::Opaque(_)
            | Self::Reduction(_)
            | Self::Agent(_)
            | Self::Variant { .. }
            | Self::RuntimeOnly(_) => false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeTupleView<'a> {
    Values(&'a [RuntimeValue]),
    Columns {
        columns: &'a [RuntimeSeq],
        row: usize,
    },
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeRecordView<'a> {
    Values(&'a RuntimeRecordValue),
    Columns {
        fields: &'a [RecordSeqField],
        row: usize,
    },
}

impl RuntimeValue {
    pub(crate) fn view(&self) -> RuntimeValueView<'_> {
        use RuntimeScalarView as Scalar;
        use RuntimeValueView as View;
        match self {
            Self::Unit => View::Scalar(Scalar::Unit),
            Self::Bool(value) => View::Scalar(Scalar::Bool(*value)),
            Self::Int(value) => View::Scalar(Scalar::Int(*value)),
            Self::UInt(value) => View::Scalar(Scalar::UInt(*value)),
            Self::F32(value) => View::Scalar(Scalar::F32(*value)),
            Self::F64(value) => View::Scalar(Scalar::F64(*value)),
            Self::String(value) => View::Scalar(Scalar::String(value)),
            Self::Color(value) => View::Scalar(Scalar::Color(*value)),
            Self::Char(value) => View::Scalar(Scalar::Char(*value)),
            Self::Duration(value) => View::Scalar(Scalar::Duration(*value)),
            Self::Progress(value) => View::Scalar(Scalar::Progress(value)),
            Self::EntityRef(value) => View::Scalar(Scalar::EntityRef(value)),
            Self::Tuple(values) => View::Tuple(RuntimeTupleView::Values(values)),
            Self::Record(values) => View::Record(RuntimeRecordView::Values(values)),
            Self::Seq(values) => View::Sequence(values),
            Self::NominalRecord(value) => View::NominalRecord(value),
            Self::Opaque(value) => View::Opaque(value),
            Self::Reduction(value) => View::Reduction(value),
            Self::Agent(value) => View::Agent(value),
            Self::Variant {
                owner,
                ordinal,
                name,
                payload,
            } => View::Variant {
                owner,
                ordinal: *ordinal,
                name,
                payload: payload.as_deref(),
            },
            Self::MatrixF32(_)
            | Self::MatrixF64(_)
            | Self::TensorF32(_)
            | Self::TensorF64(_)
            | Self::Range(_)
            | Self::Iterator(_)
            | Self::Need(_)
            | Self::Callable(_) => View::RuntimeOnly(self),
        }
    }
}

impl RuntimeSeq {
    pub(crate) fn value_view(&self, index: usize) -> Option<RuntimeValueView<'_>> {
        if index >= self.len() {
            return None;
        }
        Some(match self {
            Self::Values(values) => values[index].view(),
            Self::Dense(values) => RuntimeValueView::Scalar(values.scalar_view(index)?),
            Self::TupleColumns(values) => RuntimeValueView::Tuple(RuntimeTupleView::Columns {
                columns: values.columns(),
                row: index,
            }),
            Self::RecordColumns(values) => RuntimeValueView::Record(RuntimeRecordView::Columns {
                fields: values.fields(),
                row: index,
            }),
        })
    }
}

impl DenseSeq {
    fn scalar_view(&self, index: usize) -> Option<RuntimeScalarView<'_>> {
        use RuntimeScalarView as Scalar;
        Some(match self {
            Self::Units(length) => {
                if index < *length {
                    Scalar::Unit
                } else {
                    return None;
                }
            }
            Self::I8(values) => Scalar::Int(RuntimeInt::I8(*values.as_slice().get(index)?)),
            Self::I16(values) => Scalar::Int(RuntimeInt::I16(*values.as_slice().get(index)?)),
            Self::I32(values) => Scalar::Int(RuntimeInt::I32(*values.as_slice().get(index)?)),
            Self::I64(values) => Scalar::Int(RuntimeInt::I64(*values.as_slice().get(index)?)),
            Self::I128(values) => Scalar::Int(RuntimeInt::I128(*values.as_slice().get(index)?)),
            Self::ISize(values) => {
                Scalar::Int(RuntimeInt::ISize(values.as_slice().get(index)?.get()))
            }
            Self::U8(values) | Self::Bytes(values) => {
                Scalar::UInt(RuntimeUInt::U8(*values.as_slice().get(index)?))
            }
            Self::U16(values) => Scalar::UInt(RuntimeUInt::U16(*values.as_slice().get(index)?)),
            Self::U32(values) => Scalar::UInt(RuntimeUInt::U32(*values.as_slice().get(index)?)),
            Self::U64(values) => Scalar::UInt(RuntimeUInt::U64(*values.as_slice().get(index)?)),
            Self::U128(values) => Scalar::UInt(RuntimeUInt::U128(*values.as_slice().get(index)?)),
            Self::USize(values) => {
                Scalar::UInt(RuntimeUInt::USize(values.as_slice().get(index)?.get()))
            }
            Self::F32(values) => Scalar::F32(*values.as_slice().get(index)?),
            Self::F64(values) => Scalar::F64(*values.as_slice().get(index)?),
            Self::Bool(values) => Scalar::Bool(*values.as_slice().get(index)?),
            Self::Chars(values) => Scalar::Char(*values.as_slice().get(index)?),
            Self::Durations(values) => Scalar::Duration(*values.as_slice().get(index)?),
            Self::Strings(values) => Scalar::String(values.as_slice().get(index)?),
            Self::EntityRefs(values) => Scalar::EntityRef(values.as_slice().get(index)?),
        })
    }
}

impl<'a> RuntimeTupleView<'a> {
    pub(crate) fn len(self) -> usize {
        match self {
            Self::Values(values) => values.len(),
            Self::Columns { columns, .. } => columns.len(),
        }
    }

    pub(crate) fn get(self, index: usize) -> Option<RuntimeValueView<'a>> {
        match self {
            Self::Values(values) => values.get(index).map(RuntimeValue::view),
            Self::Columns { columns, row } => columns.get(index)?.value_view(row),
        }
    }
}

impl<'a> RuntimeRecordView<'a> {
    pub(crate) fn len(self) -> usize {
        match self {
            Self::Values(values) => values.len(),
            Self::Columns { fields, .. } => fields.len(),
        }
    }

    pub(crate) fn get(
        self,
        index: usize,
    ) -> Option<(RuntimeRecordFieldId, &'a str, RuntimeValueView<'a>)> {
        match self {
            Self::Values(values) => values
                .get(index)
                .map(|field| (field.field(), field.name(), field.value().view())),
            Self::Columns { fields, row } => {
                let field = fields.get(index)?;
                Some((field.field(), field.name(), field.values().value_view(row)?))
            }
        }
    }
}
