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
        let roles = [
            CharacterDialogueRuntimeRole::Stage,
            CharacterDialogueRuntimeRole::Portrait,
            CharacterDialogueRuntimeRole::Focus,
            CharacterDialogueRuntimeRole::Cleanup,
            CharacterDialogueRuntimeRole::Hook,
            CharacterDialogueRuntimeRole::Style,
            CharacterDialogueRuntimeRole::RichText,
        ];
        let encoded = serde_json::to_string(&roles).expect("serialize roles");
        assert_eq!(
            encoded,
            r#"["stage","portrait","focus","cleanup","hook","style","rich_text"]"#
        );
    }
}
