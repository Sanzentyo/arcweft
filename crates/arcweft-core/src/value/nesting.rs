//! Shared runtime-value nesting validation.

use super::{RuntimeIterator, RuntimeSeq, RuntimeValue};
use thiserror::Error;

/// Maximum nesting accepted by the runtime value, AWBC, and persistence
/// boundaries.
pub const MAX_RUNTIME_VALUE_NESTING_DEPTH: usize = 64;

/// Runtime value nesting validation failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeValueNestingError {
    #[error("runtime value nesting exceeds {maximum} levels")]
    Exceeded { maximum: usize },
}

impl RuntimeValue {
    /// Validates this value's recursive nesting without applying
    /// domain-specific structured-value limits.
    pub fn validate_nesting_depth(&self, maximum: usize) -> Result<(), RuntimeValueNestingError> {
        validate_value(self, 0, maximum)
    }
}

fn validate_value(
    value: &RuntimeValue,
    depth: usize,
    maximum: usize,
) -> Result<(), RuntimeValueNestingError> {
    ensure_depth(depth, maximum)?;
    match value {
        RuntimeValue::Tuple(values) => validate_values(values, depth + 1, maximum),
        RuntimeValue::Seq(sequence) => validate_sequence(sequence, depth + 1, maximum),
        RuntimeValue::Record(fields) => fields
            .iter()
            .try_for_each(|field| validate_value(field.value(), depth + 1, maximum)),
        RuntimeValue::NominalRecord(record) => validate_values(record.fields(), depth + 1, maximum),
        RuntimeValue::Opaque(value) => validate_value(value.payload(), depth + 1, maximum),
        RuntimeValue::Function(function) => function
            .captures
            .iter()
            .try_for_each(|capture| validate_value(&capture.value, depth + 1, maximum)),
        RuntimeValue::Iterator(RuntimeIterator::Values { items, .. }) => {
            validate_values(items, depth + 1, maximum)
        }
        RuntimeValue::Iterator(RuntimeIterator::Witness { state, .. }) => {
            validate_value(state, depth + 1, maximum)
        }
        RuntimeValue::Variant {
            payload: Some(payload),
            ..
        } => validate_value(payload, depth + 1, maximum),
        RuntimeValue::Unit
        | RuntimeValue::Bool(_)
        | RuntimeValue::Int(_)
        | RuntimeValue::UInt(_)
        | RuntimeValue::F32(_)
        | RuntimeValue::F64(_)
        | RuntimeValue::MatrixF32(_)
        | RuntimeValue::MatrixF64(_)
        | RuntimeValue::TensorF32(_)
        | RuntimeValue::TensorF64(_)
        | RuntimeValue::String(_)
        | RuntimeValue::Char(_)
        | RuntimeValue::Duration(_)
        | RuntimeValue::Range(_)
        | RuntimeValue::Iterator(RuntimeIterator::Range(_))
        | RuntimeValue::EntityRef(_)
        | RuntimeValue::Variant { payload: None, .. } => Ok(()),
    }
}

fn validate_values(
    values: &[RuntimeValue],
    depth: usize,
    maximum: usize,
) -> Result<(), RuntimeValueNestingError> {
    values
        .iter()
        .try_for_each(|value| validate_value(value, depth, maximum))
}

/// Validates values represented by one sequence storage strategy at their
/// logical item depth. Columnar tuple/record rows add one logical container
/// level before their field values.
fn validate_sequence(
    sequence: &RuntimeSeq,
    item_depth: usize,
    maximum: usize,
) -> Result<(), RuntimeValueNestingError> {
    match sequence {
        RuntimeSeq::Values(values) => validate_values(values, item_depth, maximum),
        RuntimeSeq::Dense(values) => {
            if values.is_empty() {
                Ok(())
            } else {
                ensure_depth(item_depth, maximum)
            }
        }
        RuntimeSeq::TupleColumns(rows) => {
            if rows.is_empty() {
                return Ok(());
            }
            ensure_depth(item_depth, maximum)?;
            rows.columns()
                .iter()
                .try_for_each(|column| validate_sequence(column, item_depth + 1, maximum))
        }
        RuntimeSeq::RecordColumns(rows) => {
            if rows.is_empty() {
                return Ok(());
            }
            ensure_depth(item_depth, maximum)?;
            rows.fields()
                .iter()
                .try_for_each(|field| validate_sequence(field.values(), item_depth + 1, maximum))
        }
    }
}

fn ensure_depth(depth: usize, maximum: usize) -> Result<(), RuntimeValueNestingError> {
    if depth > maximum {
        Err(RuntimeValueNestingError::Exceeded { maximum })
    } else {
        Ok(())
    }
}
