//! Structural ownership classification for executable runtime values.
//!
//! This module owns the generic classification used by structured execution,
//! AWBC, snapshots, and every affine opaque-handle leaf. New value containers
//! and affine leaf classes must extend the exhaustive traversals below rather
//! than introduce a side table.

use super::{
    RuntimeCallableValue, RuntimeHandleKind, RuntimeIterator, RuntimeOpaqueValueClass, RuntimeSeq,
    RuntimeValue,
};
use serde::{Deserialize, Serialize};
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
pub enum RuntimeAffineLineHandleError {
    #[error(transparent)]
    Path(#[from] RuntimeValuePathError),
    #[error(transparent)]
    Token(#[from] crate::runtime_id::RuntimeLineHandleTokenDecodeError),
    #[error(transparent)]
    RecordField(#[from] super::RuntimeRecordFieldIdError),
    #[error("runtime value structural ordinal exceeds the canonical u32 path coordinate")]
    StructuralOrdinalOverflow,
}

/// A bare value cannot transfer the ledger obligations of a live line handle.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeDetachedValueError {
    #[error(transparent)]
    HandleGraph(#[from] RuntimeAffineLineHandleError),
    #[error("line handle {token:?} at {path:?} requires its owning ledger")]
    LineHandleCustodyRequired {
        token: crate::runtime_id::RuntimeLineHandleToken,
        path: RuntimeValuePath,
    },
    #[error("Need {correlation:?} at {path:?} requires its retained producer context")]
    NeedProducerCustodyRequired {
        correlation: crate::task::TaskCorrelation,
        path: RuntimeValuePath,
    },
}

#[derive(Default)]
struct RuntimeValueResourceGraph {
    line_handles: Vec<RuntimeAffineLineHandle>,
    owned_need: Option<(crate::task::TaskCorrelation, RuntimeValuePath)>,
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
    /// Checks a transfer that carries a value without an execution-resource
    /// owner. External Need identities remain valid; issued line handles must
    /// travel with their ledger instead of becoming detached values.
    pub fn validate_detached_custody(&self) -> Result<(), RuntimeDetachedValueError> {
        let mut graph = RuntimeValueResourceGraph::default();
        self.collect_resources(&RuntimeValuePath::root(), &mut graph)?;
        if let Some(handle) = graph.line_handles.into_iter().next() {
            return Err(RuntimeDetachedValueError::LineHandleCustodyRequired {
                token: handle.token,
                path: handle.path,
            });
        }
        if let Some((correlation, path)) = graph.owned_need {
            return Err(RuntimeDetachedValueError::NeedProducerCustodyRequired {
                correlation,
                path,
            });
        }
        Ok(())
    }

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
            | Self::Color(_)
            | Self::Char(_)
            | Self::Duration(_)
            | Self::Progress(_)
            | Self::Range(_)
            | Self::EntityRef(_) => RuntimeValueOwnership::Unrestricted,
            Self::NeedHandle(_) => RuntimeValueOwnership::Affine,
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
            Self::Callable(callable) => callable.ownership(),
            Self::Variant { payload, .. } => payload
                .as_deref()
                .map_or(RuntimeValueOwnership::Unrestricted, RuntimeValue::ownership),
        }
    }

    pub(crate) fn affine_line_handles(
        &self,
    ) -> Result<Vec<RuntimeAffineLineHandle>, RuntimeAffineLineHandleError> {
        let mut handles = RuntimeValueResourceGraph::default();
        self.collect_resources(&RuntimeValuePath::root(), &mut handles)?;
        Ok(handles.line_handles)
    }

    fn collect_resources(
        &self,
        path: &RuntimeValuePath,
        handles: &mut RuntimeValueResourceGraph,
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
            | Self::Color(_)
            | Self::Char(_)
            | Self::Duration(_)
            | Self::Progress(_)
            | Self::Range(_)
            | Self::EntityRef(_)
            | Self::Seq(RuntimeSeq::Dense(_))
            | Self::Iterator(RuntimeIterator::Range(_)) => Ok(()),
            Self::NeedHandle(need) => {
                if handles.owned_need.is_none() && need.requires_producer_custody() {
                    handles.owned_need = Some((need.correlation(), path.clone()));
                }
                for (index, value) in need.request_values().enumerate() {
                    let index = u32::try_from(index)
                        .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
                    value.collect_resources(
                        &path.child(RuntimeValuePathSegment::NeedRequestArgument(index))?,
                        handles,
                    )?;
                }
                Ok(())
            }
            Self::Tuple(values) => collect_indexed_resources(
                values,
                path,
                RuntimeValuePathSegment::TupleElement,
                handles,
            ),
            Self::Seq(RuntimeSeq::Values(values)) => collect_indexed_resources_u64(
                values,
                path,
                RuntimeValuePathSegment::SequenceElement,
                handles,
            ),
            Self::Seq(RuntimeSeq::TupleColumns(columns)) => {
                for (index, column) in columns.columns().iter().enumerate() {
                    let index = u32::try_from(index)
                        .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
                    column.collect_resources(
                        &path.child(RuntimeValuePathSegment::TupleColumn(index))?,
                        handles,
                    )?;
                }
                Ok(())
            }
            Self::Seq(RuntimeSeq::RecordColumns(records)) => {
                for field in records.fields() {
                    field.values().collect_resources(
                        &path.child(RuntimeValuePathSegment::RecordColumn(field.field()))?,
                        handles,
                    )?;
                }
                Ok(())
            }
            Self::Record(fields) => {
                for field in fields {
                    field.value().collect_resources(
                        &path.child(RuntimeValuePathSegment::RecordField(field.field()))?,
                        handles,
                    )?;
                }
                Ok(())
            }
            Self::NominalRecord(record) => {
                for (index, value) in record.fields().iter().enumerate() {
                    let field = super::RuntimeRecordFieldId::try_from_zero_based_ordinal(index)?;
                    value.collect_resources(
                        &path.child(RuntimeValuePathSegment::NominalRecordField(field))?,
                        handles,
                    )?;
                }
                Ok(())
            }
            Self::Opaque(value) => match value.value_class() {
                RuntimeOpaqueValueClass::AffineHandle(kind) => {
                    handles.line_handles.push(RuntimeAffineLineHandle {
                        kind,
                        token: crate::runtime_id::RuntimeLineHandleToken::try_decode_payload(
                            value.payload(),
                        )?,
                        path: path.clone(),
                    });
                    Ok(())
                }
                RuntimeOpaqueValueClass::Plain => value.payload().collect_resources(
                    &path.child(RuntimeValuePathSegment::OpaquePayload)?,
                    handles,
                ),
            },
            Self::Iterator(RuntimeIterator::Values { items }) => {
                for (offset, value) in items.iter().enumerate() {
                    let offset = u64::try_from(offset)
                        .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
                    value.collect_resources(
                        &path.child(RuntimeValuePathSegment::IteratorRemainder(offset))?,
                        handles,
                    )?;
                }
                Ok(())
            }
            Self::Iterator(RuntimeIterator::Witness { state, .. }) => state.collect_resources(
                &path.child(RuntimeValuePathSegment::IteratorWitnessState)?,
                handles,
            ),
            Self::Variant { payload, .. } => match payload {
                Some(payload) => payload.collect_resources(
                    &path.child(RuntimeValuePathSegment::VariantPayload)?,
                    handles,
                ),
                None => Ok(()),
            },
            Self::Reduction(reduction) => {
                reduction.state().collect_resources(
                    &path.child(RuntimeValuePathSegment::ReductionState)?,
                    handles,
                )?;
                for (index, command) in reduction.commands().iter().enumerate() {
                    let index = u32::try_from(index)
                        .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
                    command.payload().0.collect_resources(
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
                    value.collect_resources(
                        &path.child(RuntimeValuePathSegment::AgentEmbeddedValue(index))?,
                        handles,
                    )?;
                }
                Ok(())
            }
            Self::Callable(callable) => collect_indexed_resources(
                callable.retained(),
                path,
                RuntimeValuePathSegment::CallableRetained,
                handles,
            ),
        }
    }
}

fn collect_indexed_resources(
    values: &[RuntimeValue],
    path: &RuntimeValuePath,
    segment: impl Fn(u32) -> RuntimeValuePathSegment,
    handles: &mut RuntimeValueResourceGraph,
) -> Result<(), RuntimeAffineLineHandleError> {
    for (index, value) in values.iter().enumerate() {
        let index = u32::try_from(index)
            .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
        value.collect_resources(&path.child(segment(index))?, handles)?;
    }
    Ok(())
}

fn collect_indexed_resources_u64(
    values: &[RuntimeValue],
    path: &RuntimeValuePath,
    segment: impl Fn(u64) -> RuntimeValuePathSegment,
    handles: &mut RuntimeValueResourceGraph,
) -> Result<(), RuntimeAffineLineHandleError> {
    for (index, value) in values.iter().enumerate() {
        let index = u64::try_from(index)
            .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
        value.collect_resources(&path.child(segment(index))?, handles)?;
    }
    Ok(())
}

impl RuntimeCallableValue {
    /// Computes ownership from the exact retained value set.
    #[must_use]
    pub fn ownership(&self) -> RuntimeValueOwnership {
        values_ownership(self.retained())
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

    fn collect_resources(
        &self,
        path: &RuntimeValuePath,
        handles: &mut RuntimeValueResourceGraph,
    ) -> Result<(), RuntimeAffineLineHandleError> {
        match self {
            Self::Values(values) => collect_indexed_resources_u64(
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
                    column.collect_resources(
                        &path.child(RuntimeValuePathSegment::TupleColumn(index))?,
                        handles,
                    )?;
                }
                Ok(())
            }
            Self::RecordColumns(records) => {
                for field in records.fields() {
                    field.values().collect_resources(
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
        RuntimeIterator::Values { items } => values_ownership(items),
        RuntimeIterator::Range(_) => RuntimeValueOwnership::Unrestricted,
        RuntimeIterator::Witness { state, .. } => state.ownership(),
    }
}

fn values_ownership<'a>(
    values: impl IntoIterator<Item = &'a RuntimeValue>,
) -> RuntimeValueOwnership {
    values
        .into_iter()
        .fold(RuntimeValueOwnership::Unrestricted, |ownership, value| {
            ownership.join(value.ownership())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::RuntimeVariantIdentity;
    use crate::value::TupleSeq;

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
                    type_instantiation: None,
                },
            )])
            .unwrap(),
            RuntimeValue::Bool(true),
        ]);

        assert_eq!(value.ownership(), RuntimeValueOwnership::Unrestricted);
    }

    #[test]
    fn need_handle_makes_its_entire_value_graph_affine() {
        let handle = RuntimeValue::NeedHandle(crate::tests::reusable_need("need.profile"));
        assert_eq!(handle.ownership(), RuntimeValueOwnership::Affine);
        assert_eq!(
            RuntimeValue::Tuple(vec![RuntimeValue::Bool(true), handle]).ownership(),
            RuntimeValueOwnership::Affine
        );
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
}
