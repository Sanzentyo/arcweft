use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error returned when a stable textual identifier is empty.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("identifier must not be empty")]
pub struct IdentifierError;

/// Public Arcweft entity identifier.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct PublicId(String);

/// Canonical identity of an ordinary Arcweft callable declaration.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct CallableId(String);

/// Content or program hash encoded with its algorithm prefix.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct StableHash(String);

/// Agent run identifier.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AgentRunId(String);

/// Target Agent session identifier.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct SessionId(String);

/// Opaque `arcweft://` resource URI.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AgentResourceUri(String);

impl PublicId {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        nonempty(value).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl CallableId {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        nonempty(value).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl StableHash {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        nonempty(value).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Encodes one canonical BLAKE3 digest using the artifact hash spelling.
    #[must_use]
    pub fn from_blake3_bytes(bytes: [u8; 32]) -> Self {
        use std::fmt::Write as _;

        let mut value = String::with_capacity("blake3:".len() + bytes.len() * 2);
        value.push_str("blake3:");
        for byte in bytes {
            write!(value, "{byte:02x}").expect("writing to String cannot fail");
        }
        Self(value)
    }
}

impl AgentRunId {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        nonempty(value).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl SessionId {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        nonempty(value).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AgentResourceUri {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        nonempty(value).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn nonempty(value: impl Into<String>) -> Result<String, IdentifierError> {
    let value = value.into();
    if value.is_empty() {
        Err(IdentifierError)
    } else {
        Ok(value)
    }
}
