//! Stable presentation occurrence identity shared by executable commands,
//! retained View state, snapshots and adapters.

use std::{borrow::Cow, fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;

/// Opaque identity of one presentation handle within its save lineage.
///
/// This is an occurrence identity, distinct from the public ID of a View,
/// asset, image, target or layer. Its canonical bytes are never normalized.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct PresentationHandleId(String);

/// Invalid canonical presentation occurrence identity.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum PresentationHandleIdError {
    #[error("presentation handle identity must not be empty")]
    Empty,
    #[error("presentation handle identity has an invalid byte at offset {offset}")]
    InvalidByte { offset: usize },
}

impl PresentationHandleId {
    /// Checks borrowed canonical bytes before copying or constructing an ID.
    pub fn validate_text(value: &str) -> Result<(), PresentationHandleIdError> {
        if value.is_empty() {
            return Err(PresentationHandleIdError::Empty);
        }
        for (offset, byte) in value.bytes().enumerate() {
            if !byte.is_ascii_alphanumeric() && !matches!(byte, b'.' | b'_' | b'-' | b'/' | b':') {
                return Err(PresentationHandleIdError::InvalidByte { offset });
            }
        }
        Ok(())
    }

    /// Validates before retaining text, reusing an owned string when supplied.
    pub fn try_new<'text>(
        value: impl Into<Cow<'text, str>>,
    ) -> Result<Self, PresentationHandleIdError> {
        let value = value.into();
        Self::validate_text(&value)?;
        Ok(Self(value.into_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PresentationHandleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl AsRef<str> for PresentationHandleId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for PresentationHandleId {
    type Err = PresentationHandleIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

impl TryFrom<String> for PresentationHandleId {
    type Error = PresentationHandleIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<PresentationHandleId> for String {
    fn from(value: PresentationHandleId) -> Self {
        value.0
    }
}

impl<'de> Deserialize<'de> for PresentationHandleId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct IdentityVisitor;

        impl de::Visitor<'_> for IdentityVisitor {
            type Value = PresentationHandleId;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a canonical presentation handle identity")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                PresentationHandleId::try_new(value).map_err(E::custom)
            }

            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                PresentationHandleId::try_new(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(IdentityVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn occurrence_identity_preserves_canonical_bytes_without_a_resource_family() {
        for source in [
            "dialogue.41",
            "view.scope.3",
            "handle.scene:0/slot-1",
            "a_B",
        ] {
            let identity = PresentationHandleId::try_new(source).expect("valid occurrence");
            assert_eq!(identity.as_str(), source);
            assert_eq!(identity.to_string(), source);
            assert_eq!(source.parse::<PresentationHandleId>().unwrap(), identity);
        }
    }

    #[test]
    fn borrowed_validation_rejects_the_exact_first_invalid_byte() {
        assert_eq!(
            PresentationHandleId::validate_text(""),
            Err(PresentationHandleIdError::Empty)
        );
        for (source, offset) in [
            ("@handle.bad", 0),
            ("bad id", 3),
            ("id.\n", 3),
            ("id.雪", 3),
        ] {
            assert_eq!(
                PresentationHandleId::validate_text(source),
                Err(PresentationHandleIdError::InvalidByte { offset })
            );
            assert!(PresentationHandleId::try_new(source).is_err());
        }
    }

    #[test]
    fn owned_construction_reuses_the_validated_original_storage() {
        let source = String::from("handle.owned");
        let pointer = source.as_ptr();
        let identity = PresentationHandleId::try_from(source).expect("valid owned identity");
        assert_eq!(identity.as_str().as_ptr(), pointer);
    }

    #[test]
    fn serde_keeps_the_canonical_string_and_rejects_invalid_or_non_string_input() {
        let identity = PresentationHandleId::try_new("dialogue.41").unwrap();
        let wire = serde_json::to_string(&identity).unwrap();
        assert_eq!(wire, r#""dialogue.41""#);
        assert_eq!(
            serde_json::from_str::<PresentationHandleId>(&wire).unwrap(),
            identity
        );
        for value in ["", "bad handle", "@handle.bad", "id.\n", "id.雪"] {
            let wire = serde_json::to_string(value).unwrap();
            assert!(serde_json::from_str::<PresentationHandleId>(&wire).is_err());
        }
        for wire in ["null", "42", "{}", "[]"] {
            assert!(serde_json::from_str::<PresentationHandleId>(wire).is_err());
        }
    }

    #[test]
    fn owned_serde_value_reuses_its_validated_string_storage() {
        let source = String::from("handle.decoded");
        let pointer = source.as_ptr();
        let identity: PresentationHandleId =
            serde_json::from_value(serde_json::Value::String(source)).unwrap();
        assert_eq!(identity.as_str().as_ptr(), pointer);
    }
}
