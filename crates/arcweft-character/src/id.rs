use arcweft_id::{IdError, PublicId};
use core::{fmt, str::FromStr};
use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;

/// Stable public character identifier such as `character.akane`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct CharacterId(String);

/// Identifier for one independently selectable character part.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct CharacterPartId(String);

/// Identifier for one image choice inside a part.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct CharacterVariantId(String);

/// Identifier for one complete part-selection state.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct CharacterLookId(String);

/// Character-format identifier validation failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterIdError {
    #[error("invalid public character id: {0}")]
    Public(#[from] IdError),
    #[error("character id must start with `character.`")]
    MissingCharacterPrefix,
    #[error("character id `{value}` has an invalid owner component at index {component_index}")]
    InvalidOwnerComponent { value: String, component_index: u32 },
    #[error("character id `{value}` has too many owner components")]
    OwnerComponentIndexOverflow { value: String },
    #[error("character id `{value}` uses reserved compact root `{root}`")]
    ReservedCompactRoot { value: String, root: String },
    #[error("{kind} id `{value}` must match [A-Za-z_][A-Za-z0-9_.-]*")]
    InvalidLocal { kind: &'static str, value: String },
}

impl CharacterId {
    /// Validates and constructs a public character identifier.
    pub fn try_new(value: impl Into<String>) -> Result<Self, CharacterIdError> {
        let value = value.into();
        PublicId::try_new(value.clone())?;
        let Some(local) = value.strip_prefix("character.") else {
            return Err(CharacterIdError::MissingCharacterPrefix);
        };
        validate_local_id("character", local.to_owned())?;
        for (component_index, component) in local.split('.').enumerate() {
            if component.is_empty()
                || !component
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
            {
                let component_index = u32::try_from(component_index).map_err(|_| {
                    CharacterIdError::OwnerComponentIndexOverflow {
                        value: value.clone(),
                    }
                })?;
                return Err(CharacterIdError::InvalidOwnerComponent {
                    value,
                    component_index,
                });
            }
        }
        let root = local.split('.').next().unwrap_or(local);
        if matches!(root, "crate" | "self" | "super" | "parent" | "character") {
            let root = root.to_owned();
            return Err(CharacterIdError::ReservedCompactRoot { value, root });
        }
        Ok(Self(value))
    }

    /// Source-visible identifier without an `@` reference marker.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Canonical owner path without the reserved `character.` namespace.
    pub fn compact_str(&self) -> &str {
        self.0.strip_prefix("character.").unwrap_or(&self.0)
    }

    /// Components of the canonical compact owner path.
    pub fn compact_segments(&self) -> impl Iterator<Item = &str> {
        self.compact_str().split('.')
    }

    /// Converts the format identifier into Arcweft's shared public-id type.
    ///
    /// # Panics
    ///
    /// Panics only if a `CharacterId` value was constructed without running
    /// `CharacterId::try_new`.
    pub fn as_public_id(&self) -> PublicId {
        PublicId::try_new(self.0.clone()).expect("validated character id remains a public id")
    }
}

impl CharacterPartId {
    /// Validates and constructs a part identifier.
    pub fn try_new(value: impl Into<String>) -> Result<Self, CharacterIdError> {
        validate_local_id("part", value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl CharacterVariantId {
    /// Validates and constructs a variant identifier.
    pub fn try_new(value: impl Into<String>) -> Result<Self, CharacterIdError> {
        validate_local_id("variant", value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl CharacterLookId {
    /// Validates and constructs a look identifier.
    pub fn try_new(value: impl Into<String>) -> Result<Self, CharacterIdError> {
        validate_local_id("look", value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn validate_local_id(kind: &'static str, value: String) -> Result<String, CharacterIdError> {
    let mut chars = value.chars();
    let first_valid = chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_');
    let rest_valid = chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-'));
    if first_valid && rest_valid {
        Ok(value)
    } else {
        Err(CharacterIdError::InvalidLocal { kind, value })
    }
}

macro_rules! impl_text_id {
    ($ty:ty, $constructor:path) => {
        impl fmt::Display for $ty {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $ty {
            type Err = CharacterIdError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                $constructor(value)
            }
        }

        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                $constructor(value).map_err(de::Error::custom)
            }
        }
    };
}

impl_text_id!(CharacterId, CharacterId::try_new);
impl_text_id!(CharacterPartId, CharacterPartId::try_new);
impl_text_id!(CharacterVariantId, CharacterVariantId::try_new);
impl_text_id!(CharacterLookId, CharacterLookId::try_new);

#[cfg(test)]
mod tests {
    use super::{CharacterId, CharacterIdError, CharacterLookId};

    #[test]
    fn character_ids_require_the_character_family() {
        assert!(CharacterId::try_new("character.akane").is_ok());
        assert!(CharacterId::try_new("asset.akane").is_err());
    }

    #[test]
    fn local_ids_reject_whitespace_and_path_punctuation() {
        assert!(CharacterLookId::try_new("smile.open").is_ok());
        assert!(CharacterLookId::try_new("smile open").is_err());
        assert!(CharacterLookId::try_new("../smile").is_err());
    }

    #[test]
    fn owner_components_and_compact_roots_are_unambiguous() {
        assert!(matches!(
            CharacterId::try_new("character.cast..alice"),
            Err(CharacterIdError::InvalidOwnerComponent {
                component_index: 1,
                ..
            })
        ));
        assert!(matches!(
            CharacterId::try_new("character.self.alice"),
            Err(CharacterIdError::ReservedCompactRoot { root, .. }) if root == "self"
        ));
        let id = CharacterId::try_new("character.cast.alice-2").expect("character id");
        assert_eq!(id.compact_str(), "cast.alice-2");
        assert_eq!(
            id.compact_segments().collect::<Vec<_>>(),
            ["cast", "alice-2"]
        );
    }
}
