use serde::{Deserialize, Deserializer, Serialize};
use std::{fmt, str::FromStr};
use thiserror::Error;

/// A reviewed external producer authority for an exported Rust opaque type.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ArcweftRustOpaqueTypeProducerId(String);

/// A violation of the Rust ABI opaque producer spelling grammar.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ArcweftRustOpaqueTypeProducerIdError {
    #[error("Rust ABI opaque type producer ID must not be empty")]
    Empty,
    #[error("Rust ABI opaque type producer ID contains a control character at byte {byte}")]
    ControlCharacter { byte: usize },
    #[error("Rust ABI opaque type producer ID `{producer}` uses the reserved `std.` namespace")]
    ReservedStandardNamespace { producer: String },
}

impl ArcweftRustOpaqueTypeProducerId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, ArcweftRustOpaqueTypeProducerIdError> {
        let value = value.into();
        Self::validate(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn validate(value: &str) -> Result<(), ArcweftRustOpaqueTypeProducerIdError> {
        if value.is_empty() {
            return Err(ArcweftRustOpaqueTypeProducerIdError::Empty);
        }
        if let Some((byte, _)) = value
            .char_indices()
            .find(|(_, character)| character.is_control())
        {
            return Err(ArcweftRustOpaqueTypeProducerIdError::ControlCharacter { byte });
        }
        if value.starts_with("std.") {
            return Err(
                ArcweftRustOpaqueTypeProducerIdError::ReservedStandardNamespace {
                    producer: value.to_owned(),
                },
            );
        }
        Ok(())
    }
}

impl fmt::Display for ArcweftRustOpaqueTypeProducerId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ArcweftRustOpaqueTypeProducerId {
    type Err = ArcweftRustOpaqueTypeProducerIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

impl TryFrom<String> for ArcweftRustOpaqueTypeProducerId {
    type Error = ArcweftRustOpaqueTypeProducerIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl<'de> Deserialize<'de> for ArcweftRustOpaqueTypeProducerId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}
