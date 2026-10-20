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
        DecodeBudget::new(0, self)?.validate_value(value)
    }
}

/// One resource bank for parse-time construction or borrowed value admission.
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

    pub const fn max_bytes_len(&self) -> usize {
        self.limits.max_bytes_len
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

    // A logical Value is one node. Record names and enum tags are metadata,
    // checked as strings without inventing extra Value nodes. Collection items
    // are sequence/tuple elements and map/record entries across the whole tree.
    fn validate_value(&mut self, value: &Value) -> Result<()> {
        self.enter_node()?;
        match value {
            Value::Unit | Value::Bool(_) | Value::Number(_) => Ok(()),
            Value::String(value) => self.string_len(value.len()),
            Value::Char(value) => self.string_len(value.len_utf8()),
            Value::Bytes(value) => self.bytes_len(value.as_slice().len()),
            Value::Seq(values) | Value::Tuple(values) => {
                self.sequence_len(values.len())?;
                values.iter().enumerate().try_for_each(|(index, value)| {
                    self.validate_value(value)
                        .map_err(|error| error.at_index(index))
                })
            }
            Value::Option(value) => value
                .as_deref()
                .map_or(Ok(()), |value| self.validate_value(value)),
            Value::Map { entries, .. } => {
                self.map_len(entries.len())?;
                entries
                    .iter()
                    .enumerate()
                    .try_for_each(|(index, (key, value))| {
                        self.validate_value(key)
                            .map_err(|error| error.at_index(index))?;
                        self.validate_value(value)
                            .map_err(|error| error.at_index(index))
                    })
            }
            Value::Record(values) => {
                self.map_len(values.len())?;
                values.iter().try_for_each(|(key, value)| {
                    // Refuse an oversized name before cloning it into an error path.
                    self.string_len(key.len())?;
                    self.validate_value(value)
                        .map_err(|error| error.at_field(key.clone()))
                })
            }
            Value::Enum { variant, payload } => {
                self.string_len(variant.len())?;
                payload.as_deref().map_or(Ok(()), |value| {
                    self.validate_value(value)
                        .map_err(|error| error.at_variant(variant.clone()))
                })
            }
        }?;
        self.exit_node();
        Ok(())
    }
}
