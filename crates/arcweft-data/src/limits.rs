use crate::error::{DataError, Result};
use crate::value::Value;

/// Decode resource limits applied to all externally supplied data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeLimits {
    pub max_depth: usize,
    pub max_sequence_len: usize,
    pub max_map_len: usize,
    pub max_string_len: usize,
    pub max_bytes_len: usize,
    pub allow_trailing_data: bool,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_depth: 64,
            max_sequence_len: 1_000_000,
            max_map_len: 200_000,
            max_string_len: 16 * 1024 * 1024,
            max_bytes_len: 64 * 1024 * 1024,
            allow_trailing_data: false,
        }
    }
}

impl DecodeLimits {
    pub fn validate(&self, value: &Value) -> Result<()> {
        validate_value(value, self, 0)
    }
}

fn validate_value(value: &Value, limits: &DecodeLimits, depth: usize) -> Result<()> {
    if depth > limits.max_depth {
        return Err(DataError::limit(format!(
            "maximum nesting depth {} exceeded",
            limits.max_depth
        )));
    }
    match value {
        Value::String(value) if value.len() > limits.max_string_len => {
            Err(DataError::limit(format!(
                "string length {} exceeds {}",
                value.len(),
                limits.max_string_len
            )))
        }
        Value::Bytes(bytes) if bytes.as_slice().len() > limits.max_bytes_len => {
            Err(DataError::limit(format!(
                "bytes length {} exceeds {}",
                bytes.as_slice().len(),
                limits.max_bytes_len
            )))
        }
        Value::Seq(values) => {
            if values.len() > limits.max_sequence_len {
                return Err(DataError::limit(format!(
                    "sequence length {} exceeds {}",
                    values.len(),
                    limits.max_sequence_len
                )));
            }
            values.iter().enumerate().try_for_each(|(index, value)| {
                validate_value(value, limits, depth + 1).map_err(|err| err.at_index(index))
            })
        }
        Value::Map(values) | Value::Record(values) => {
            if values.len() > limits.max_map_len {
                return Err(DataError::limit(format!(
                    "map length {} exceeds {}",
                    values.len(),
                    limits.max_map_len
                )));
            }
            values.iter().try_for_each(|(key, value)| {
                validate_value(value, limits, depth + 1).map_err(|err| err.at_field(key.clone()))
            })
        }
        Value::Enum { variant, payload } => {
            if variant.len() > limits.max_string_len {
                return Err(DataError::limit("enum variant name exceeds string limit"));
            }
            payload.as_deref().map_or(Ok(()), |value| {
                validate_value(value, limits, depth + 1)
                    .map_err(|err| err.at_variant(variant.clone()))
            })
        }
        _ => Ok(()),
    }
}
