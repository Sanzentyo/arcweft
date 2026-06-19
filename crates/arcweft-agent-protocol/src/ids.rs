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

impl StableHash {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        nonempty(value).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
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
