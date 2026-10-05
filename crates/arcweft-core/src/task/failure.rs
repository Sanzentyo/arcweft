//! Finite infrastructure diagnostics. Domain failures remain typed Result values.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct BoundedRuntimeDiagnostic(String);

impl BoundedRuntimeDiagnostic {
    pub const MAX_BYTES: usize = 4096;

    /// Trusted runtime/adapter formatting keeps a deterministic UTF-8 prefix.
    pub fn from_message(message: impl Into<String>) -> Self {
        let mut message = message.into();
        if message.len() > Self::MAX_BYTES {
            let mut end = Self::MAX_BYTES;
            while !message.is_char_boundary(end) {
                end -= 1;
            }
            message.truncate(end);
        }
        Self(message)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for BoundedRuntimeDiagnostic {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let message = String::deserialize(deserializer)?;
        if message.len() > Self::MAX_BYTES {
            return Err(serde::de::Error::custom(
                "runtime diagnostic exceeds its byte bound",
            ));
        }
        Ok(Self(message))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeTaskFailureKind {
    AdapterUnavailable,
    AdapterProtocolViolation,
    WorkerFailure,
    RuntimeInvariant,
    RestoreFailure,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeTaskFailure {
    pub kind: RuntimeTaskFailureKind,
    pub diagnostic: BoundedRuntimeDiagnostic,
}

impl RuntimeTaskFailure {
    pub fn new(kind: RuntimeTaskFailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            diagnostic: BoundedRuntimeDiagnostic::from_message(message),
        }
    }
}

impl std::fmt::Display for RuntimeTaskFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.diagnostic.as_str())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeNeedOutcome {
    Value(super::RuntimePayload),
    InfrastructureFailure(RuntimeTaskFailure),
}
