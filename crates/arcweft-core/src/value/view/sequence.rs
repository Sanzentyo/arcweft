//! Physical storage metadata and logical rows share one borrowed sequence.

mod metadata;

use super::super::{RuntimeSeq, awbc_save::AwbcRuntimeSeqSnapshot};
use super::{RuntimeRecordView, RuntimeTupleView, RuntimeValueView};
use crate::entry::{
    RuntimeSchemaError,
    schema::value_budget::{ValidationWork, ValueBudget},
};

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeSequenceView<'a> {
    Bytes(&'a [u8]),
    DialogueBindings(&'a [super::super::RuntimeDialogueContentBinding]),
    DialogueEffects(&'a [super::super::RuntimeDialogueContentEffectBinding]),
    Live(&'a RuntimeSeq),
    Snapshot(&'a AwbcRuntimeSeqSnapshot),
}

impl<'a> RuntimeSequenceView<'a> {
    pub(crate) fn storage_label(self) -> &'static str {
        use super::super::DenseSeqKind as Kind;
        let dense = match self {
            Self::Bytes(_) => Kind::Bytes,
            Self::Live(RuntimeSeq::Dense(value))
            | Self::Snapshot(AwbcRuntimeSeqSnapshot::Dense(value)) => value.kind(),
            Self::Live(RuntimeSeq::Values(_))
            | Self::Snapshot(AwbcRuntimeSeqSnapshot::Values(_))
            | Self::DialogueBindings(_)
            | Self::DialogueEffects(_) => return "values",
            Self::Live(RuntimeSeq::TupleColumns(_))
            | Self::Snapshot(AwbcRuntimeSeqSnapshot::TupleColumns { .. }) => return "tuple_columns",
            Self::Live(RuntimeSeq::RecordColumns(_))
            | Self::Snapshot(AwbcRuntimeSeqSnapshot::RecordColumns { .. }) => {
                return "record_columns";
            }
        };
        match dense {
            Kind::Units => "units",
            Kind::I8 => "i8",
            Kind::I16 => "i16",
            Kind::I32 => "i32",
            Kind::I64 => "i64",
            Kind::I128 => "i128",
            Kind::ISize => "isize",
            Kind::U8 => "u8",
            Kind::U16 => "u16",
            Kind::U32 => "u32",
            Kind::U64 => "u64",
            Kind::U128 => "u128",
            Kind::USize => "usize",
            Kind::F32 => "f32",
            Kind::F64 => "f64",
            Kind::Bool => "bool",
            Kind::Bytes => "bytes",
            Kind::Chars => "chars",
            Kind::Durations => "durations",
            Kind::Strings => "strings",
            Kind::EntityRefs => "entity_refs",
        }
    }
    pub(crate) fn all(self, mut predicate: impl FnMut(RuntimeValueView<'a>) -> bool) -> bool {
        self.len().is_ok_and(|length| {
            (0..length).all(|index| self.value_view(index).is_some_and(&mut predicate))
        })
    }
    pub(crate) fn dense_bytes(self) -> Option<&'a [u8]> {
        match self {
            Self::Bytes(value) => Some(value),
            Self::Live(v) if v.dense_kind() == Some(super::super::DenseSeqKind::Bytes) => {
                v.as_bytes()
            }
            Self::Snapshot(AwbcRuntimeSeqSnapshot::Dense(super::super::DenseSeq::Bytes(v))) => {
                Some(v.as_slice())
            }
            _ => None,
        }
    }
    pub(crate) fn byte_storage(self) -> Option<&'a [u8]> {
        match self {
            Self::Bytes(value) => Some(value),
            Self::Live(value) => value.as_bytes(),
            Self::Snapshot(AwbcRuntimeSeqSnapshot::Dense(value)) => value.as_bytes(),
            _ => None,
        }
    }
    pub(crate) fn len(self) -> Result<usize, RuntimeSchemaError> {
        match self {
            Self::Bytes(value) => Ok(value.len()),
            Self::DialogueBindings(value) => Ok(value.len()),
            Self::DialogueEffects(value) => Ok(value.len()),
            Self::Live(v) => Ok(v.len()),
            Self::Snapshot(AwbcRuntimeSeqSnapshot::Values(v)) => Ok(v.len()),
            Self::Snapshot(AwbcRuntimeSeqSnapshot::Dense(v)) => Ok(v.len()),
            Self::Snapshot(
                AwbcRuntimeSeqSnapshot::TupleColumns { len, .. }
                | AwbcRuntimeSeqSnapshot::RecordColumns { len, .. },
            ) => usize::try_from(*len).map_err(|_| RuntimeSchemaError::Encoding {
                message: "snapshot sequence length exceeds the selected target width".to_owned(),
            }),
        }
    }
    pub(crate) fn value_view(self, index: usize) -> Option<RuntimeValueView<'a>> {
        if index >= self.len().ok()? {
            return None;
        }
        Some(match self {
            Self::Bytes(value) => RuntimeValueView::Scalar(super::RuntimeScalarView::UInt(
                super::super::RuntimeUInt::U8(*value.get(index)?),
            )),
            Self::DialogueBindings(value) => {
                super::super::opaque::RuntimeDialogueTupleView::Binding(value.get(index)?).view()
            }
            Self::DialogueEffects(value) => {
                super::super::opaque::RuntimeDialogueTupleView::Effect(value.get(index)?).view()
            }
            Self::Live(v) => v.value_view(index)?,
            Self::Snapshot(AwbcRuntimeSeqSnapshot::Values(v)) => v.get(index)?.view(),
            Self::Snapshot(AwbcRuntimeSeqSnapshot::Dense(v)) => {
                RuntimeValueView::Scalar(v.scalar_view(index)?)
            }
            Self::Snapshot(AwbcRuntimeSeqSnapshot::TupleColumns { columns, .. }) => {
                RuntimeValueView::Tuple(RuntimeTupleView::SnapshotColumns {
                    columns,
                    row: index,
                })
            }
            Self::Snapshot(AwbcRuntimeSeqSnapshot::RecordColumns { fields, .. }) => {
                RuntimeValueView::Record(RuntimeRecordView::SnapshotColumns { fields, row: index })
            }
        })
    }
    fn column_length_error() -> RuntimeSchemaError {
        RuntimeSchemaError::Encoding {
            message: "snapshot column length does not match its row inventory".to_owned(),
        }
    }
    fn record_error(error: super::super::RuntimeRecordAdmissionError) -> RuntimeSchemaError {
        RuntimeSchemaError::Encoding {
            message: error.to_string(),
        }
    }
    pub(super) const fn live_value(self) -> Option<&'a RuntimeSeq> {
        match self {
            Self::Live(v) => Some(v),
            Self::Snapshot(_)
            | Self::Bytes(_)
            | Self::DialogueBindings(_)
            | Self::DialogueEffects(_) => None,
        }
    }
}
