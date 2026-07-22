//! Typed identities owned by semantic environments.

use core::fmt;

use thiserror::Error;

/// Stable identity of one source-visible environment binding owner.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EnvironmentBindingId(String);

/// Invalid environment binding identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EnvironmentBindingIdError {
    #[error("environment binding identity must not be empty")]
    Empty,
    #[error("environment binding identity contains a control character at byte {byte}")]
    Control { byte: usize },
}

impl EnvironmentBindingId {
    /// Validates and creates an environment binding identity.
    pub fn try_new(value: impl Into<String>) -> Result<Self, EnvironmentBindingIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(EnvironmentBindingIdError::Empty);
        }
        if let Some((byte, _)) = value
            .char_indices()
            .find(|(_, character)| character.is_control())
        {
            return Err(EnvironmentBindingIdError::Control { byte });
        }
        Ok(Self(value))
    }

    /// Canonical owner spelling used for presentation and manifest matching.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EnvironmentBindingId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
