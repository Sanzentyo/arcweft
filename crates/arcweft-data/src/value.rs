use std::collections::BTreeMap;

use crate::error::{DataError, Result};

/// Exact numeric value carried across formats.
#[derive(Clone, Debug, PartialEq)]
pub enum Number {
    I(i128),
    U(u128),
    F32(f32),
    F64(f64),
}

impl Number {
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::I(_) => "signed integer",
            Self::U(_) => "unsigned integer",
            Self::F32(_) => "f32",
            Self::F64(_) => "f64",
        }
    }
}

/// Owned byte buffer with explicit semantics.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Bytes(Vec<u8>);

impl Bytes {
    #[must_use]
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self(bytes.into())
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(value: Vec<u8>) -> Self {
        Self::new(value)
    }
}

impl From<&[u8]> for Bytes {
    fn from(value: &[u8]) -> Self {
        Self::new(value)
    }
}

/// Arcweft-owned dynamic data representation for codec adapters and migration.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Unit,
    Bool(bool),
    Number(Number),
    String(String),
    Char(char),
    Bytes(Bytes),
    Seq(Vec<Value>),
    Map(BTreeMap<String, Value>),
    Record(BTreeMap<String, Value>),
    Enum {
        variant: String,
        payload: Option<Box<Value>>,
    },
}

impl Value {
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Unit => "unit",
            Self::Bool(_) => "bool",
            Self::Number(number) => number.type_name(),
            Self::String(_) => "string",
            Self::Char(_) => "char",
            Self::Bytes(_) => "bytes",
            Self::Seq(_) => "sequence",
            Self::Map(_) => "map",
            Self::Record(_) => "record",
            Self::Enum { .. } => "enum",
        }
    }

    pub fn as_record(&self) -> Result<&BTreeMap<String, Value>> {
        match self {
            Self::Record(fields) => Ok(fields),
            other => Err(DataError::invalid_type("record", other.type_name())),
        }
    }

    pub fn as_seq(&self) -> Result<&[Value]> {
        match self {
            Self::Seq(values) => Ok(values),
            other => Err(DataError::invalid_type("sequence", other.type_name())),
        }
    }

    #[must_use]
    pub fn stringify_scalar(&self) -> Option<String> {
        match self {
            Self::Unit => Some(String::new()),
            Self::Bool(value) => Some(value.to_string()),
            Self::Number(Number::I(value)) => Some(value.to_string()),
            Self::Number(Number::U(value)) => Some(value.to_string()),
            Self::Number(Number::F32(value)) => Some(value.to_string()),
            Self::Number(Number::F64(value)) => Some(value.to_string()),
            Self::String(value) => Some(value.clone()),
            Self::Char(value) => Some(value.to_string()),
            Self::Bytes(_) | Self::Seq(_) | Self::Map(_) | Self::Record(_) | Self::Enum { .. } => {
                None
            }
        }
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<Bytes> for Value {
    fn from(value: Bytes) -> Self {
        Self::Bytes(value)
    }
}
