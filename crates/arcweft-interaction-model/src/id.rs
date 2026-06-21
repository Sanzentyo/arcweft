use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

/// Validated non-empty identifier used inside typed interaction newtypes.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct Identifier(String);

impl Identifier {
    /// Creates an identifier after trimming and rejecting empty values.
    ///
    /// # Errors
    ///
    /// Returns [`IdentifierError`] when the trimmed value is empty.
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(IdentifierError);
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl TryFrom<String> for Identifier {
    type Error = IdentifierError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Identifier> for String {
    fn from(value: Identifier) -> Self {
        value.0
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Error returned when an interaction identifier is empty.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdentifierError;

impl fmt::Display for IdentifierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("interaction identifier must not be empty")
    }
}

impl Error for IdentifierError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_trims_outer_whitespace() {
        let identifier = Identifier::new("  action.advance  ").expect("identifier");
        assert_eq!(identifier.as_str(), "action.advance");
    }

    #[test]
    fn identifier_rejects_empty_values() {
        assert_eq!(Identifier::new(" \t\n"), Err(IdentifierError));
    }
}
