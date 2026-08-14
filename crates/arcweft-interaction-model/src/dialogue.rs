//! Lower-layer coordinates shared by dialogue semantics and runtime admission.

use arcweft_id::PublicId;
use core::fmt;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Maximum encoded byte length of a `CharacterDialogue` custom-field identity.
pub const MAX_CHARACTER_DIALOGUE_CUSTOM_FIELD_ID_BYTES: usize = 128;

/// Stable custom-field identity in the `character_dialogue_field.*` family.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterDialogueCustomFieldId(PublicId);

/// Failure to construct a [`CharacterDialogueCustomFieldId`].
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterDialogueCustomFieldIdError {
    #[error("CharacterDialogue custom-field identity exceeds {maximum} encoded bytes: {actual}")]
    TooLong { actual: usize, maximum: usize },
    #[error("invalid CharacterDialogue custom-field identity `{value}`")]
    Invalid { value: String },
}

impl CharacterDialogueCustomFieldId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, CharacterDialogueCustomFieldIdError> {
        let value = value.into();
        if value.len() > MAX_CHARACTER_DIALOGUE_CUSTOM_FIELD_ID_BYTES {
            return Err(CharacterDialogueCustomFieldIdError::TooLong {
                actual: value.len(),
                maximum: MAX_CHARACTER_DIALOGUE_CUSTOM_FIELD_ID_BYTES,
            });
        }
        if !value.starts_with("character_dialogue_field.") {
            return Err(CharacterDialogueCustomFieldIdError::Invalid { value });
        }
        PublicId::try_new(value.clone())
            .map(Self)
            .map_err(|_| CharacterDialogueCustomFieldIdError::Invalid { value })
    }

    #[must_use]
    pub const fn public_id(&self) -> &PublicId {
        &self.0
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for CharacterDialogueCustomFieldId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for CharacterDialogueCustomFieldId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for CharacterDialogueCustomFieldId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Canonical typed coordinate for one `CharacterDialogue` runtime role.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
#[serde(rename_all = "snake_case")]
pub enum CharacterDialogueRuntimeRole {
    Stage = 0,
    Portrait = 1,
    Focus = 2,
    Cleanup = 3,
    Hook = 4,
    Style = 5,
    RichText = 6,
}

impl CharacterDialogueRuntimeRole {
    /// Every runtime role in canonical wire order.
    pub const ALL: [Self; 7] = [
        Self::Stage,
        Self::Portrait,
        Self::Focus,
        Self::Cleanup,
        Self::Hook,
        Self::Style,
        Self::RichText,
    ];

    /// Roles backed by authored accepted nominal declarations.
    ///
    /// `Style` is derived from the entity-reference and rich-text alternatives
    /// and therefore has no independent authored declaration.
    pub const AUTHORED_BASE: [Self; 6] = [
        Self::Stage,
        Self::Portrait,
        Self::Focus,
        Self::Cleanup,
        Self::Hook,
        Self::RichText,
    ];

    /// Stable version-one ordinal used by canonical binary transcripts.
    #[must_use]
    pub const fn canonical_tag(self) -> u8 {
        self as u8
    }

    /// Returns whether this role owns an authored accepted nominal row.
    #[must_use]
    pub const fn is_authored_base(self) -> bool {
        !matches!(self, Self::Style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_field_id_preserves_the_accepted_family_and_serde_spelling() {
        let id = CharacterDialogueCustomFieldId::try_new("character_dialogue_field.mood")
            .expect("valid custom-field identity");
        assert_eq!(id.as_str(), "character_dialogue_field.mood");
        let encoded = serde_json::to_string(&id).expect("serialize identity");
        assert_eq!(encoded, r#""character_dialogue_field.mood""#);
        assert_eq!(
            serde_json::from_str::<CharacterDialogueCustomFieldId>(&encoded)
                .expect("deserialize identity"),
            id
        );
    }

    #[test]
    fn custom_field_id_rejects_wrong_family_invalid_text_and_oversize() {
        assert!(matches!(
            CharacterDialogueCustomFieldId::try_new("view.mood"),
            Err(CharacterDialogueCustomFieldIdError::Invalid { .. })
        ));
        assert!(matches!(
            CharacterDialogueCustomFieldId::try_new("character_dialogue_field.bad value"),
            Err(CharacterDialogueCustomFieldIdError::Invalid { .. })
        ));
        let prefix = "character_dialogue_field.";
        let oversized = format!(
            "{prefix}{}",
            "x".repeat(MAX_CHARACTER_DIALOGUE_CUSTOM_FIELD_ID_BYTES - prefix.len() + 1)
        );
        assert!(matches!(
            CharacterDialogueCustomFieldId::try_new(oversized),
            Err(CharacterDialogueCustomFieldIdError::TooLong {
                maximum: MAX_CHARACTER_DIALOGUE_CUSTOM_FIELD_ID_BYTES,
                ..
            })
        ));
    }

    #[test]
    fn runtime_roles_have_fixed_version_one_wire_names() {
        let encoded =
            serde_json::to_string(&CharacterDialogueRuntimeRole::ALL).expect("serialize roles");
        assert_eq!(
            encoded,
            r#"["stage","portrait","focus","cleanup","hook","style","rich_text"]"#
        );
        assert_eq!(
            CharacterDialogueRuntimeRole::ALL.map(CharacterDialogueRuntimeRole::canonical_tag),
            [0, 1, 2, 3, 4, 5, 6]
        );
        assert_eq!(
            CharacterDialogueRuntimeRole::AUTHORED_BASE,
            [
                CharacterDialogueRuntimeRole::Stage,
                CharacterDialogueRuntimeRole::Portrait,
                CharacterDialogueRuntimeRole::Focus,
                CharacterDialogueRuntimeRole::Cleanup,
                CharacterDialogueRuntimeRole::Hook,
                CharacterDialogueRuntimeRole::RichText,
            ]
        );
        assert!(
            CharacterDialogueRuntimeRole::AUTHORED_BASE
                .into_iter()
                .all(CharacterDialogueRuntimeRole::is_authored_base)
        );
        assert!(!CharacterDialogueRuntimeRole::Style.is_authored_base());
    }
}
