//! Stable identities owned by `CharacterDialogue`.

use super::{CharacterDialogueValueError, limits::MAX_PUBLIC_ID_BYTES};
use arcweft_core::entry::RuntimeValueDigest;
use arcweft_id::{LocaleTag, LocaleTagError, PublicId};
use core::fmt;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Contract provenance retained by every `CharacterDialogue` value.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterDialogueContractIdentity {
    visual_manifest: CharacterDialogueVisualManifestEvidence,
    defaults: RuntimeValueDigest,
    custom_schema: RuntimeValueDigest,
    view_contracts: RuntimeValueDigest,
}

/// Accepted visual-manifest evidence for one logical Character declaration.
///
/// A Character may be a logical runtime member without a visual manifest. The
/// `Absent` case is explicit and cannot be confused with an empty manifest or
/// a fabricated digest.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", content = "fingerprint", rename_all = "snake_case")]
pub enum CharacterDialogueVisualManifestEvidence {
    Absent,
    Present(RuntimeValueDigest),
}

/// Reusable voice selection for one `CharacterDialogue`.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum CharacterDialogueVoice {
    Auto,
    Id(CharacterDialogueVoiceId),
}

/// Stable character-dialogue voice identity in the `voice.*` family.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterDialogueVoiceId(PublicId);

/// Canonical source-locale identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DialogueLocaleId(LocaleTag);

impl CharacterDialogueContractIdentity {
    #[must_use]
    pub const fn with_visual_manifest(
        visual_manifest: CharacterDialogueVisualManifestEvidence,
        defaults: RuntimeValueDigest,
        custom_schema: RuntimeValueDigest,
        view_contracts: RuntimeValueDigest,
    ) -> Self {
        Self {
            visual_manifest,
            defaults,
            custom_schema,
            view_contracts,
        }
    }

    #[must_use]
    pub const fn visual_manifest(self) -> CharacterDialogueVisualManifestEvidence {
        self.visual_manifest
    }

    /// Returns the actual manifest fingerprint when visual evidence is present.
    #[must_use]
    pub const fn character_manifest(self) -> Option<RuntimeValueDigest> {
        match self.visual_manifest {
            CharacterDialogueVisualManifestEvidence::Absent => None,
            CharacterDialogueVisualManifestEvidence::Present(fingerprint) => Some(fingerprint),
        }
    }

    #[must_use]
    pub const fn defaults(self) -> RuntimeValueDigest {
        self.defaults
    }

    #[must_use]
    pub const fn custom_schema(self) -> RuntimeValueDigest {
        self.custom_schema
    }

    #[must_use]
    pub const fn view_contracts(self) -> RuntimeValueDigest {
        self.view_contracts
    }
}

impl CharacterDialogueVoiceId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, CharacterDialogueValueError> {
        let value = value.into();
        if value.len() > MAX_PUBLIC_ID_BYTES {
            return Err(CharacterDialogueValueError::Limit {
                limit: "voice_id_bytes",
                maximum: MAX_PUBLIC_ID_BYTES,
            });
        }
        if !value.starts_with("voice.") {
            return Err(CharacterDialogueValueError::Identity {
                kind: "CharacterDialogue voice",
                value,
            });
        }
        PublicId::try_new(value.clone()).map(Self).map_err(|_| {
            CharacterDialogueValueError::Identity {
                kind: "CharacterDialogue voice",
                value,
            }
        })
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

impl DialogueLocaleId {
    /// Validates one already-canonical ASCII BCP-47 locale.
    pub fn try_new(value: impl Into<String>) -> Result<Self, CharacterDialogueValueError> {
        let value = value.into();
        Self::from_result(value.clone(), LocaleTag::try_new(&value))
    }

    /// Validates and canonicalizes authored locale text.
    pub fn canonicalize(value: impl Into<String>) -> Result<Self, CharacterDialogueValueError> {
        let value = value.into();
        Self::from_result(value.clone(), LocaleTag::canonicalize(&value))
    }

    fn from_result(
        value: String,
        result: Result<LocaleTag, LocaleTagError>,
    ) -> Result<Self, CharacterDialogueValueError> {
        result
            .map(Self)
            .map_err(|error| CharacterDialogueValueError::Locale {
                value,
                reason: error.to_string(),
            })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    #[must_use]
    pub const fn locale_tag(&self) -> &LocaleTag {
        &self.0
    }
}

impl fmt::Display for CharacterDialogueVoiceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for DialogueLocaleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

macro_rules! validated_string_serde {
    ($ty:ty, $constructor:path) => {
        impl Serialize for $ty {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                $constructor(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
            }
        }
    };
}

validated_string_serde!(CharacterDialogueVoiceId, CharacterDialogueVoiceId::try_new);
validated_string_serde!(DialogueLocaleId, DialogueLocaleId::try_new);
