//! Shared authored identity for adapter-owned opaque runtime producers.

use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use thiserror::Error;

/// Stable producer domain explicitly authored for one opaque adapter type.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AdapterOpaqueTypeProducerId(String);

/// Invalid adapter-owned opaque producer identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AdapterOpaqueTypeProducerIdError {
    #[error("adapter opaque type producer ID must not be empty")]
    Empty,
    #[error("adapter opaque type producer ID contains a control character at byte {byte}")]
    ControlCharacter { byte: usize },
    #[error("adapter opaque type producer ID `{producer}` uses the reserved `std.` namespace")]
    ReservedStandardNamespace { producer: String },
}

impl AdapterOpaqueTypeProducerId {
    /// Validates and constructs one exact external producer identity.
    pub fn try_new(value: impl Into<String>) -> Result<Self, AdapterOpaqueTypeProducerIdError> {
        let value = value.into();
        Self::validate(&value)?;
        Ok(Self(value))
    }

    /// Exact authored producer spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(value: &str) -> Result<(), AdapterOpaqueTypeProducerIdError> {
        if value.is_empty() {
            return Err(AdapterOpaqueTypeProducerIdError::Empty);
        }
        if let Some((byte, _)) = value.char_indices().find(|(_, ch)| ch.is_control()) {
            return Err(AdapterOpaqueTypeProducerIdError::ControlCharacter { byte });
        }
        if value.starts_with("std.") {
            return Err(
                AdapterOpaqueTypeProducerIdError::ReservedStandardNamespace {
                    producer: value.to_owned(),
                },
            );
        }
        Ok(())
    }
}

impl fmt::Display for AdapterOpaqueTypeProducerId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for AdapterOpaqueTypeProducerId {
    type Err = AdapterOpaqueTypeProducerIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

impl TryFrom<String> for AdapterOpaqueTypeProducerId {
    type Error = AdapterOpaqueTypeProducerIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl<'de> Deserialize<'de> for AdapterOpaqueTypeProducerId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_exact_valid_spelling() {
        let producer =
            AdapterOpaqueTypeProducerId::try_new("vendor.Exact-domain_1").expect("valid producer");
        assert_eq!(producer.as_str(), "vendor.Exact-domain_1");
    }

    #[test]
    fn rejects_empty_control_and_reserved_spelling() {
        assert_eq!(
            AdapterOpaqueTypeProducerId::try_new(""),
            Err(AdapterOpaqueTypeProducerIdError::Empty)
        );
        assert_eq!(
            AdapterOpaqueTypeProducerId::try_new("vendor.\nvalue"),
            Err(AdapterOpaqueTypeProducerIdError::ControlCharacter { byte: 7 })
        );
        assert_eq!(
            AdapterOpaqueTypeProducerId::try_new("std.claimed"),
            Err(
                AdapterOpaqueTypeProducerIdError::ReservedStandardNamespace {
                    producer: "std.claimed".to_owned()
                }
            )
        );
    }
}
