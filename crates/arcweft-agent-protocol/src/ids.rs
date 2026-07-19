use serde::{Deserialize, Deserializer, Serialize};
use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
};
use thiserror::Error;

/// Error returned when a stable textual identifier is malformed.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum IdentifierError {
    #[error("identifier must not be empty")]
    Empty,
    #[error(
        "Agent run identifier must use canonical `run.<segment>` spelling with lowercase ASCII letters, digits, `_`, or `-`"
    )]
    InvalidAgentRunId,
}

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
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AgentRunId(String);

/// Target Agent session identifier.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct SessionId(String);

/// Checked Agent resource URI.
///
/// This type validates only the transport-level non-empty string contract. A
/// URI spelling never grants publication privileges by itself.
#[derive(Clone, Debug)]
pub struct AgentResourceUri {
    value: String,
    provenance: AgentResourceUriProvenance,
}

#[derive(Clone, Copy, Debug)]
enum AgentResourceUriProvenance {
    Generic,
    CanonicalTrace { body_digest: [u8; 32] },
}

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
        canonical_agent_run_id(value.into()).map(Self)
    }

    pub(crate) fn unknown_trace() -> Self {
        Self("run.unknown".to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AgentRunId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl<'de> Deserialize<'de> for AgentRunId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
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
        nonempty(value).map(|value| Self {
            value,
            provenance: AgentResourceUriProvenance::Generic,
        })
    }

    pub(crate) fn sealed_trace(run_id: &AgentRunId, body_digest: [u8; 32]) -> Self {
        Self {
            value: format!("arcweft://run/{}/trace.arcwx", run_id.as_str()),
            provenance: AgentResourceUriProvenance::CanonicalTrace { body_digest },
        }
    }

    pub(crate) fn certifies_trace_body(&self, body_digest: [u8; 32]) -> bool {
        matches!(
            self.provenance,
            AgentResourceUriProvenance::CanonicalTrace {
                body_digest: certified,
            } if certified == body_digest
        )
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn into_string(self) -> String {
        self.value
    }
}

impl PartialEq for AgentResourceUri {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for AgentResourceUri {}

impl PartialOrd for AgentResourceUri {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AgentResourceUri {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl Hash for AgentResourceUri {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl Serialize for AgentResourceUri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AgentResourceUri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for AgentResourceUri {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(formatter)
    }
}

impl std::ops::Deref for AgentResourceUri {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for AgentResourceUri {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for AgentResourceUri {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq<str> for AgentResourceUri {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for AgentResourceUri {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

fn nonempty(value: impl Into<String>) -> Result<String, IdentifierError> {
    let value = value.into();
    if value.is_empty() {
        Err(IdentifierError::Empty)
    } else {
        Ok(value)
    }
}

fn canonical_agent_run_id(value: String) -> Result<String, IdentifierError> {
    let Some(suffix) = value.strip_prefix("run.") else {
        return Err(IdentifierError::InvalidAgentRunId);
    };
    let is_canonical = !suffix.is_empty()
        && suffix.split('.').all(|segment| {
            !segment.is_empty()
                && segment.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'_' | b'-')
                })
                && segment
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && segment
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
        });
    if is_canonical {
        Ok(value)
    } else {
        Err(IdentifierError::InvalidAgentRunId)
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentResourceUri, AgentRunId, IdentifierError};

    #[test]
    fn agent_run_id_accepts_canonical_segments() {
        for value in [
            "run.cli",
            "run.debug.123",
            "run.mcp.first",
            "run.release_candidate-1",
        ] {
            assert_eq!(
                AgentRunId::new(value)
                    .expect("run id is canonical")
                    .as_str(),
                value
            );
        }
    }

    #[test]
    fn agent_run_id_rejects_noncanonical_spelling_and_uri_delimiters() {
        for value in [
            "",
            "run",
            "Run.cli",
            "run.CLI",
            "run..cli",
            "run.-cli",
            "run.cli-",
            "run.cli/path",
            "run.cli?query",
            "run.cli#fragment",
            "run.cli value",
            "run.cli\nvalue",
        ] {
            assert!(
                AgentRunId::new(value).is_err(),
                "`{value:?}` must not be a canonical run id"
            );
        }
        assert_eq!(
            AgentRunId::new("not-a-run").expect_err("prefix is required"),
            IdentifierError::InvalidAgentRunId
        );
        assert!(
            serde_json::from_str::<AgentRunId>("\"run.cli/path\"").is_err(),
            "deserialization must enforce the same grammar"
        );
    }

    #[test]
    fn resource_uri_serde_preserves_only_the_transport_value() {
        let uri = AgentResourceUri::new("arcweft://run/run.cli/trace.arcwx")
            .expect("canonical-looking URI is nonempty");
        let encoded = serde_json::to_string(&uri).expect("URI serializes");
        let decoded: AgentResourceUri = serde_json::from_str(&encoded).expect("URI deserializes");

        assert_eq!(decoded, uri);
        assert_eq!(decoded.as_str(), "arcweft://run/run.cli/trace.arcwx");
    }
}
