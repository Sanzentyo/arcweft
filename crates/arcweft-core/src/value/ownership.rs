//! Structural ownership classification for executable runtime values.
//!
//! This module owns the generic classification used by structured execution,
//! AWBC, snapshots, and every affine opaque-handle leaf. New value containers
//! and affine leaf classes must extend the exhaustive traversals below rather
//! than introduce a side table.

use super::{
    RuntimeFunctionBody, RuntimeFunctionValue, RuntimeHandleKind, RuntimeIterator,
    RuntimeOpaqueValueClass, RuntimeSeq, RuntimeValue,
};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use thiserror::Error;

#[allow(dead_code, reason = "the canonical snapshot consumer lands in G1.2-D")]
mod binary;
mod path;
mod slot;

pub use path::{
    MAX_RUNTIME_VALUE_PATH_SEGMENTS, RuntimeValuePath, RuntimeValuePathError,
    RuntimeValuePathSegment,
};
pub use slot::RuntimeOwnedSlotId;

/// Whether a runtime value may be duplicated without transferring authority.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeValueOwnership {
    /// The complete transitive value graph may be copied.
    Unrestricted,
    /// The transitive graph contains at least one single-owner leaf.
    Affine,
}

/// One typed affine line handle with its canonical path in the containing
/// runtime value graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeAffineLineHandle {
    kind: RuntimeHandleKind,
    token: crate::runtime_id::RuntimeLineHandleToken,
    path: RuntimeValuePath,
}

impl RuntimeAffineLineHandle {
    pub(crate) const fn kind(&self) -> RuntimeHandleKind {
        self.kind
    }

    pub(crate) const fn token(&self) -> &crate::runtime_id::RuntimeLineHandleToken {
        &self.token
    }

    pub(crate) const fn path(&self) -> &RuntimeValuePath {
        &self.path
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum RuntimeAffineLineHandleError {
    #[error(transparent)]
    Path(#[from] RuntimeValuePathError),
    #[error(transparent)]
    Token(#[from] crate::runtime_id::RuntimeLineHandleTokenDecodeError),
    #[error(transparent)]
    RecordField(#[from] super::RuntimeRecordFieldIdError),
    #[error("runtime value structural ordinal exceeds the canonical u32 path coordinate")]
    StructuralOrdinalOverflow,
}

impl RuntimeValueOwnership {
    /// Joins two transitive ownership classifications.
    #[must_use]
    pub const fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unrestricted, Self::Unrestricted) => Self::Unrestricted,
            _ => Self::Affine,
        }
    }

    /// Returns whether language-level copying is permitted.
    #[must_use]
    pub const fn permits_copy(self) -> bool {
        matches!(self, Self::Unrestricted)
    }
}

impl RuntimeValue {
    /// Computes ownership from the complete executable value graph.
    ///
    /// The exhaustive recursive traversal is the sole authority and makes a
    /// future value variant a compile-time obligation here.
    #[must_use]
    pub fn ownership(&self) -> RuntimeValueOwnership {
        match self {
            Self::Unit
            | Self::Bool(_)
            | Self::Int(_)
            | Self::UInt(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::MatrixF32(_)
            | Self::MatrixF64(_)
            | Self::TensorF32(_)
            | Self::TensorF64(_)
            | Self::String(_)
            | Self::Char(_)
            | Self::Duration(_)
            | Self::Progress(_)
            | Self::Range(_)
            | Self::EntityRef(_) => RuntimeValueOwnership::Unrestricted,
            Self::Iterator(iterator) => iterator_ownership(iterator),
            Self::Tuple(values) => values_ownership(values),
            Self::Seq(sequence) => sequence.ownership(),
            Self::Record(fields) => fields
                .iter()
                .fold(RuntimeValueOwnership::Unrestricted, |ownership, field| {
                    ownership.join(field.value().ownership())
                }),
            Self::NominalRecord(record) => values_ownership(record.fields()),
            Self::Opaque(value) => match value.value_class() {
                RuntimeOpaqueValueClass::Plain => value.payload().ownership(),
                RuntimeOpaqueValueClass::AffineHandle(_) => {
                    RuntimeValueOwnership::Affine.join(value.payload().ownership())
                }
            },
            Self::Reduction(value) => value
                .commands()
                .iter()
                .fold(value.state().ownership(), |ownership, command| {
                    ownership.join(command.payload().0.ownership())
                }),
            Self::Agent(value) => value.ownership(),
            Self::Function(function) => function.ownership(),
            Self::Variant { payload, .. } => payload
                .as_deref()
                .map_or(RuntimeValueOwnership::Unrestricted, RuntimeValue::ownership),
        }
    }

    pub(crate) fn affine_line_handles(
        &self,
    ) -> Result<Vec<RuntimeAffineLineHandle>, RuntimeAffineLineHandleError> {
        let mut handles = Vec::new();
        self.collect_affine_line_handles(&RuntimeValuePath::root(), &mut handles)?;
        Ok(handles)
    }

    fn collect_affine_line_handles(
        &self,
        path: &RuntimeValuePath,
        handles: &mut Vec<RuntimeAffineLineHandle>,
    ) -> Result<(), RuntimeAffineLineHandleError> {
        match self {
            Self::Unit
            | Self::Bool(_)
            | Self::Int(_)
            | Self::UInt(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::MatrixF32(_)
            | Self::MatrixF64(_)
            | Self::TensorF32(_)
            | Self::TensorF64(_)
            | Self::String(_)
            | Self::Char(_)
            | Self::Duration(_)
            | Self::Progress(_)
            | Self::Range(_)
            | Self::EntityRef(_) => Ok(()),
            Self::Tuple(values) => collect_indexed_line_handles(
                values,
                path,
                RuntimeValuePathSegment::TupleElement,
                handles,
            ),
            Self::Seq(RuntimeSeq::Values(values)) => collect_indexed_line_handles_u64(
                values,
                path,
                RuntimeValuePathSegment::SequenceElement,
                handles,
            ),
            Self::Seq(RuntimeSeq::Dense(_)) => Ok(()),
            Self::Seq(RuntimeSeq::TupleColumns(columns)) => {
                for (index, column) in columns.columns().iter().enumerate() {
                    let index = u32::try_from(index)
                        .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
                    column.collect_affine_line_handles(
                        &path.child(RuntimeValuePathSegment::TupleColumn(index))?,
                        handles,
                    )?;
                }
                Ok(())
            }
            Self::Seq(RuntimeSeq::RecordColumns(records)) => {
                for field in records.fields() {
                    field.values().collect_affine_line_handles(
                        &path.child(RuntimeValuePathSegment::RecordColumn(field.field()))?,
                        handles,
                    )?;
                }
                Ok(())
            }
            Self::Record(fields) => {
                for field in fields {
                    field.value().collect_affine_line_handles(
                        &path.child(RuntimeValuePathSegment::RecordField(field.field()))?,
                        handles,
                    )?;
                }
                Ok(())
            }
            Self::NominalRecord(record) => {
                for (index, value) in record.fields().iter().enumerate() {
                    let field = super::RuntimeRecordFieldId::try_from_zero_based_ordinal(index)?;
                    value.collect_affine_line_handles(
                        &path.child(RuntimeValuePathSegment::NominalRecordField(field))?,
                        handles,
                    )?;
                }
                Ok(())
            }
            Self::Opaque(value) => match value.value_class() {
                RuntimeOpaqueValueClass::AffineHandle(kind) => {
                    handles.push(RuntimeAffineLineHandle {
                        kind,
                        token: crate::runtime_id::RuntimeLineHandleToken::try_decode_payload(
                            value.payload(),
                        )?,
                        path: path.clone(),
                    });
                    Ok(())
                }
                RuntimeOpaqueValueClass::Plain => value.payload().collect_affine_line_handles(
                    &path.child(RuntimeValuePathSegment::OpaquePayload)?,
                    handles,
                ),
            },
            Self::Iterator(RuntimeIterator::Values { items, index }) => {
                for (offset, value) in items.iter().enumerate().skip(*index) {
                    let offset = u64::try_from(offset)
                        .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
                    value.collect_affine_line_handles(
                        &path.child(RuntimeValuePathSegment::IteratorRemainder(offset))?,
                        handles,
                    )?;
                }
                Ok(())
            }
            Self::Iterator(RuntimeIterator::Range(_)) => Ok(()),
            Self::Iterator(RuntimeIterator::Witness { state, .. }) => state
                .collect_affine_line_handles(
                    &path.child(RuntimeValuePathSegment::IteratorWitnessState)?,
                    handles,
                ),
            Self::Variant { payload, .. } => match payload {
                Some(payload) => payload.collect_affine_line_handles(
                    &path.child(RuntimeValuePathSegment::VariantPayload)?,
                    handles,
                ),
                None => Ok(()),
            },
            Self::Reduction(reduction) => {
                reduction.state().collect_affine_line_handles(
                    &path.child(RuntimeValuePathSegment::ReductionState)?,
                    handles,
                )?;
                for (index, command) in reduction.commands().iter().enumerate() {
                    let index = u32::try_from(index)
                        .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
                    command.payload().0.collect_affine_line_handles(
                        &path.child(RuntimeValuePathSegment::ReductionCommandPayload(index))?,
                        handles,
                    )?;
                }
                Ok(())
            }
            Self::Agent(agent) => {
                for (index, (_, value)) in agent
                    .nested_runtime_values_with_depth()
                    .into_iter()
                    .enumerate()
                {
                    let index = u32::try_from(index)
                        .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
                    value.collect_affine_line_handles(
                        &path.child(RuntimeValuePathSegment::AgentEmbeddedValue(index))?,
                        handles,
                    )?;
                }
                Ok(())
            }
            Self::Function(function) => function.collect_affine_line_handles(path, handles),
        }
    }
}

fn collect_indexed_line_handles(
    values: &[RuntimeValue],
    path: &RuntimeValuePath,
    segment: impl Fn(u32) -> RuntimeValuePathSegment,
    handles: &mut Vec<RuntimeAffineLineHandle>,
) -> Result<(), RuntimeAffineLineHandleError> {
    for (index, value) in values.iter().enumerate() {
        let index = u32::try_from(index)
            .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
        value.collect_affine_line_handles(&path.child(segment(index))?, handles)?;
    }
    Ok(())
}

fn collect_indexed_line_handles_u64(
    values: &[RuntimeValue],
    path: &RuntimeValuePath,
    segment: impl Fn(u64) -> RuntimeValuePathSegment,
    handles: &mut Vec<RuntimeAffineLineHandle>,
) -> Result<(), RuntimeAffineLineHandleError> {
    for (index, value) in values.iter().enumerate() {
        let index = u64::try_from(index)
            .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
        value.collect_affine_line_handles(&path.child(segment(index))?, handles)?;
    }
    Ok(())
}

impl RuntimeFunctionValue {
    /// Computes ownership from the exact captured value set.
    #[must_use]
    pub fn ownership(&self) -> RuntimeValueOwnership {
        match self.body() {
            RuntimeFunctionBody::Structured(closure) => closure
                .capture_values()
                .iter()
                .chain(closure.bound_args())
                .fold(RuntimeValueOwnership::Unrestricted, |ownership, value| {
                    ownership.join(value.ownership())
                }),
            RuntimeFunctionBody::Awbc(closure) => closure
                .captures()
                .iter()
                .fold(RuntimeValueOwnership::Unrestricted, |ownership, capture| {
                    ownership.join(capture.value.ownership())
                }),
        }
    }

    fn collect_affine_line_handles(
        &self,
        path: &RuntimeValuePath,
        handles: &mut Vec<RuntimeAffineLineHandle>,
    ) -> Result<(), RuntimeAffineLineHandleError> {
        let values: Box<dyn Iterator<Item = &RuntimeValue> + '_> = match self.body() {
            RuntimeFunctionBody::Structured(closure) => {
                Box::new(closure.capture_values().iter().chain(closure.bound_args()))
            }
            RuntimeFunctionBody::Awbc(closure) => {
                Box::new(closure.captures().iter().map(|capture| &capture.value))
            }
        };
        for (index, value) in values.enumerate() {
            let ordinal = u32::try_from(index)
                .ok()
                .and_then(|index| index.checked_add(1))
                .and_then(NonZeroU32::new)
                .map(crate::runtime_id::RuntimeCaptureSlotId::from_accepted_ordinal)
                .ok_or(RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
            value.collect_affine_line_handles(
                &path.child(RuntimeValuePathSegment::FunctionCapture(ordinal))?,
                handles,
            )?;
        }
        Ok(())
    }
}

impl RuntimeSeq {
    /// Computes ownership in canonical logical storage order.
    #[must_use]
    pub fn ownership(&self) -> RuntimeValueOwnership {
        match self {
            Self::Values(values) => values_ownership(values),
            Self::Dense(_) => RuntimeValueOwnership::Unrestricted,
            Self::TupleColumns(columns) => columns
                .columns()
                .iter()
                .fold(RuntimeValueOwnership::Unrestricted, |ownership, column| {
                    ownership.join(column.ownership())
                }),
            Self::RecordColumns(records) => records
                .fields()
                .iter()
                .fold(RuntimeValueOwnership::Unrestricted, |ownership, field| {
                    ownership.join(field.values().ownership())
                }),
        }
    }

    fn collect_affine_line_handles(
        &self,
        path: &RuntimeValuePath,
        handles: &mut Vec<RuntimeAffineLineHandle>,
    ) -> Result<(), RuntimeAffineLineHandleError> {
        match self {
            Self::Values(values) => collect_indexed_line_handles_u64(
                values,
                path,
                RuntimeValuePathSegment::SequenceElement,
                handles,
            ),
            Self::Dense(_) => Ok(()),
            Self::TupleColumns(columns) => {
                for (index, column) in columns.columns().iter().enumerate() {
                    let index = u32::try_from(index)
                        .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
                    column.collect_affine_line_handles(
                        &path.child(RuntimeValuePathSegment::TupleColumn(index))?,
                        handles,
                    )?;
                }
                Ok(())
            }
            Self::RecordColumns(records) => {
                for field in records.fields() {
                    field.values().collect_affine_line_handles(
                        &path.child(RuntimeValuePathSegment::RecordColumn(field.field()))?,
                        handles,
                    )?;
                }
                Ok(())
            }
        }
    }
}

fn iterator_ownership(iterator: &RuntimeIterator) -> RuntimeValueOwnership {
    match iterator {
        RuntimeIterator::Values { items, index } => {
            values_ownership(items.get(*index..).unwrap_or_default())
        }
        RuntimeIterator::Range(_) => RuntimeValueOwnership::Unrestricted,
        RuntimeIterator::Witness { state, .. } => state.ownership(),
    }
}

fn values_ownership(values: &[RuntimeValue]) -> RuntimeValueOwnership {
    values
        .iter()
        .fold(RuntimeValueOwnership::Unrestricted, |ownership, value| {
            ownership.join(value.ownership())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::awbc::schema::AwbcFunctionId;
    use crate::pattern::RuntimeVariantIdentity;
    use crate::value::{RuntimeBinding, RuntimeFunctionValue, TupleSeq};

    #[test]
    fn join_is_affine_if_either_side_is_affine() {
        assert_eq!(
            RuntimeValueOwnership::Unrestricted.join(RuntimeValueOwnership::Unrestricted),
            RuntimeValueOwnership::Unrestricted
        );
        assert_eq!(
            RuntimeValueOwnership::Unrestricted.join(RuntimeValueOwnership::Affine),
            RuntimeValueOwnership::Affine
        );
        assert_eq!(
            RuntimeValueOwnership::Affine.join(RuntimeValueOwnership::Unrestricted),
            RuntimeValueOwnership::Affine
        );
        assert_eq!(
            RuntimeValueOwnership::Affine.join(RuntimeValueOwnership::Affine),
            RuntimeValueOwnership::Affine
        );
        assert!(RuntimeValueOwnership::Unrestricted.permits_copy());
        assert!(!RuntimeValueOwnership::Affine.permits_copy());
    }

    #[test]
    fn current_nested_value_graph_is_recursively_unrestricted() {
        let value = RuntimeValue::Tuple(vec![
            RuntimeValue::try_record(vec![(
                "payload".to_owned(),
                RuntimeValue::Variant {
                    owner: RuntimeVariantIdentity::Builtin(
                        crate::pattern::RuntimeBuiltinVariantIdentity::Result,
                    ),
                    ordinal: 0,
                    name: "Ok".to_owned(),
                    payload: Some(Box::new(RuntimeValue::Seq(RuntimeSeq::values(vec![
                        RuntimeValue::String("value".to_owned()),
                    ])))),
                },
            )])
            .unwrap(),
            RuntimeValue::Bool(true),
        ]);

        assert_eq!(value.ownership(), RuntimeValueOwnership::Unrestricted);
    }

    #[test]
    fn columnar_sequence_ownership_uses_stored_column_order() {
        let tuple = RuntimeSeq::TupleColumns(
            TupleSeq::new(
                1,
                vec![
                    RuntimeSeq::values(vec![RuntimeValue::u32(1)]),
                    RuntimeSeq::values(vec![RuntimeValue::String("x".to_owned())]),
                ],
            )
            .unwrap(),
        );
        let record = RuntimeSeq::record_columns(1, vec![("field".to_owned(), tuple)]).unwrap();

        assert_eq!(record.ownership(), RuntimeValueOwnership::Unrestricted);
    }

    #[test]
    fn ownership_wire_names_are_stable() {
        assert_eq!(
            serde_json::to_string(&RuntimeValueOwnership::Unrestricted).unwrap(),
            "\"unrestricted\""
        );
        assert_eq!(
            serde_json::to_string(&RuntimeValueOwnership::Affine).unwrap(),
            "\"affine\""
        );
    }

    #[test]
    fn function_ownership_is_derived_from_exact_captures() {
        let function = RuntimeFunctionValue::new_awbc(
            Vec::new(),
            AwbcFunctionId(0),
            vec![RuntimeBinding {
                name: "captured".to_owned(),
                value: RuntimeValue::Tuple(vec![RuntimeValue::Bool(true)]),
            }],
        );

        assert_eq!(function.ownership(), RuntimeValueOwnership::Unrestricted);
        assert_eq!(
            RuntimeValue::Function(function).ownership(),
            RuntimeValueOwnership::Unrestricted
        );
    }
}
