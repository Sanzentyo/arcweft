use crate::error::{DataError, Result};
use crate::value::Value;

/// Decode resource limits applied to all externally supplied data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeLimits {
    pub max_input_len: usize,
    pub max_depth: usize,
    pub max_nodes: usize,
    pub max_sequence_len: usize,
    pub max_map_len: usize,
    pub max_collection_items: usize,
    pub max_string_len: usize,
    pub max_bytes_len: usize,
    pub allow_trailing_data: bool,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_input_len: 256 * 1024 * 1024,
            max_depth: 64,
            max_nodes: 10_000_000,
            max_sequence_len: 1_000_000,
            max_map_len: 200_000,
            max_collection_items: 10_000_000,
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

/// Parse-time budget that codecs consume before allocating decoded values.
#[derive(Debug)]
pub struct DecodeBudget<'a> {
    limits: &'a DecodeLimits,
    depth: usize,
    remaining_nodes: usize,
    remaining_collection_items: usize,
}

impl<'a> DecodeBudget<'a> {
    pub fn new(input_len: usize, limits: &'a DecodeLimits) -> Result<Self> {
        if input_len > limits.max_input_len {
            return Err(DataError::limit(format!(
                "input length {input_len} exceeds {}",
                limits.max_input_len
            )));
        }
        Ok(Self {
            limits,
            depth: 0,
            remaining_nodes: limits.max_nodes,
            remaining_collection_items: limits.max_collection_items,
        })
    }

    pub fn enter_node(&mut self) -> Result<()> {
        if self.depth > self.limits.max_depth {
            return Err(DataError::limit(format!(
                "maximum nesting depth {} exceeded",
                self.limits.max_depth
            )));
        }
        self.remaining_nodes = self
            .remaining_nodes
            .checked_sub(1)
            .ok_or_else(|| DataError::limit("decoded node budget exhausted"))?;
        self.depth += 1;
        Ok(())
    }

    pub fn exit_node(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    pub fn sequence_len(&mut self, len: usize) -> Result<()> {
        if len > self.limits.max_sequence_len {
            return Err(DataError::limit(format!(
                "sequence length {len} exceeds {}",
                self.limits.max_sequence_len
            )));
        }
        self.consume_collection_items(len)
    }

    pub fn sequence_item(&mut self, len_after_item: usize) -> Result<()> {
        if len_after_item > self.limits.max_sequence_len {
            return Err(DataError::limit(format!(
                "sequence length {len_after_item} exceeds {}",
                self.limits.max_sequence_len
            )));
        }
        self.consume_collection_items(1)
    }

    pub fn map_len(&mut self, len: usize) -> Result<()> {
        if len > self.limits.max_map_len {
            return Err(DataError::limit(format!(
                "map length {len} exceeds {}",
                self.limits.max_map_len
            )));
        }
        self.consume_collection_items(len)
    }

    pub fn map_item(&mut self, len_after_item: usize) -> Result<()> {
        if len_after_item > self.limits.max_map_len {
            return Err(DataError::limit(format!(
                "map length {len_after_item} exceeds {}",
                self.limits.max_map_len
            )));
        }
        self.consume_collection_items(1)
    }

    pub fn string_len(&self, len: usize) -> Result<()> {
        if len > self.limits.max_string_len {
            return Err(DataError::limit(format!(
                "string length {len} exceeds {}",
                self.limits.max_string_len
            )));
        }
        Ok(())
    }

    pub fn bytes_len(&self, len: usize) -> Result<()> {
        if len > self.limits.max_bytes_len {
            return Err(DataError::limit(format!(
                "bytes length {len} exceeds {}",
                self.limits.max_bytes_len
            )));
        }
        Ok(())
    }

    fn consume_collection_items(&mut self, len: usize) -> Result<()> {
        self.remaining_collection_items = self
            .remaining_collection_items
            .checked_sub(len)
            .ok_or_else(|| {
                DataError::limit(format!("collection item budget exhausted by length {len}"))
            })?;
        Ok(())
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
