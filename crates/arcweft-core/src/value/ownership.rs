//! Structural ownership classification for executable runtime values.
//!
//! This module owns the generic classification used by structured execution,
//! AWBC, snapshots, and future affine leaves.  No affine authority can be
//! constructed in this cut; adding such a leaf must extend the exhaustive
//! traversals below rather than introduce a side table.

use super::{RuntimeFunctionBody, RuntimeFunctionValue, RuntimeIterator, RuntimeSeq, RuntimeValue};
use serde::{Deserialize, Serialize};

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
    /// The current graph has no constructible affine leaf, so every accepted
    /// value is unrestricted.  The exhaustive recursive traversal is already
    /// the sole authority and will make a future affine variant a compile-time
    /// obligation here.
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
            Self::Opaque(value) => value.payload().ownership(),
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
                    owner: RuntimeVariantIdentity::Result,
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
