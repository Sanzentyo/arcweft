//! Stable identities owned by `CharacterDialogue`.

use super::{
    CharacterDialogueValueError, PRODUCTION_CHARACTER_DIALOGUE_LIMITS, limits::MAX_PUBLIC_ID_BYTES,
};
use arcweft_core::entry::RuntimeValueDigest;
use arcweft_id::PublicId;
use core::fmt;

/// Contract provenance retained by every `CharacterDialogue` value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterDialogueContractIdentity {
    character_manifest: RuntimeValueDigest,
    defaults: RuntimeValueDigest,
    custom_schema: RuntimeValueDigest,
    view_contracts: RuntimeValueDigest,
}

/// Reusable voice selection for one `CharacterDialogue`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterDialogueVoice {
    Auto,
    Id(CharacterDialogueVoiceId),
}

/// Stable character-dialogue voice identity in the `voice.*` family.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterDialogueVoiceId(PublicId);

/// Canonical source-locale identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DialogueLocaleId(String);

/// Stable custom-field identity in the `character_dialogue_field.*` family.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterDialogueCustomFieldId(PublicId);

impl CharacterDialogueContractIdentity {
    #[must_use]
    pub const fn new(
        character_manifest: RuntimeValueDigest,
        defaults: RuntimeValueDigest,
        custom_schema: RuntimeValueDigest,
        view_contracts: RuntimeValueDigest,
    ) -> Self {
        Self {
            character_manifest,
            defaults,
            custom_schema,
            view_contracts,
        }
    }

    #[must_use]
    pub const fn character_manifest(self) -> RuntimeValueDigest {
        self.character_manifest
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

impl CharacterDialogueCustomFieldId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, CharacterDialogueValueError> {
        let value = value.into();
        let maximum = usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_custom_field_id_bytes);
        if value.len() > maximum {
            return Err(CharacterDialogueValueError::Limit {
                limit: "custom_field_id_bytes",
                maximum,
            });
        }
        if !value.starts_with("character_dialogue_field.") {
            return Err(CharacterDialogueValueError::Identity {
                kind: "CharacterDialogue custom field",
                value,
            });
        }
        PublicId::try_new(value.clone()).map(Self).map_err(|_| {
            CharacterDialogueValueError::Identity {
                kind: "CharacterDialogue custom field",
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
    /// Validates and canonicalizes an ASCII BCP-47 locale.
    pub fn try_new(value: impl Into<String>) -> Result<Self, CharacterDialogueValueError> {
        let value = value.into();
        let maximum = usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_locale_bytes);
        if value.is_empty() || value.len() > maximum {
            return Err(CharacterDialogueValueError::Locale {
                value,
                reason: "locale must contain 1..=64 bytes",
            });
        }
        if !value.is_ascii() || value.chars().any(char::is_control) {
            return Err(CharacterDialogueValueError::Locale {
                value,
                reason: "locale must contain only non-control ASCII",
            });
        }

        let source = value.split('-').collect::<Vec<_>>();
        if source.iter().any(|part| {
            part.is_empty()
                || part.len() > 8
                || !part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        }) {
            return Err(CharacterDialogueValueError::Locale {
                value,
                reason: "locale contains an invalid subtag",
            });
        }
        let Some(language) = source.first() else {
            unreachable!("empty locale was rejected");
        };
        if !(2..=8).contains(&language.len())
            || !language.bytes().all(|byte| byte.is_ascii_alphabetic())
        {
            return Err(CharacterDialogueValueError::Locale {
                value,
                reason: "language subtag must contain 2..=8 ASCII letters",
            });
        }

        let mut canonical = Vec::with_capacity(source.len());
        let mut seen = std::collections::BTreeSet::new();
        for (index, part) in source.into_iter().enumerate() {
            let lower = part.to_ascii_lowercase();
            if !seen.insert(lower.clone()) {
                return Err(CharacterDialogueValueError::Locale {
                    value,
                    reason: "locale contains a duplicate subtag",
                });
            }
            let part = if index == 0 {
                lower
            } else if part.len() == 4 && part.bytes().all(|byte| byte.is_ascii_alphabetic()) {
                let mut chars = lower.chars();
                let Some(first) = chars.next() else {
                    return Err(CharacterDialogueValueError::Locale {
                        value,
                        reason: "script subtag is empty",
                    });
                };
                let first = first.to_ascii_uppercase();
                format!("{first}{}", chars.as_str())
            } else if (part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_alphabetic()))
                || (part.len() == 3 && part.bytes().all(|byte| byte.is_ascii_digit()))
            {
                part.to_ascii_uppercase()
            } else {
                lower
            };
            canonical.push(part);
        }
        Ok(Self(canonical.join("-")))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CharacterDialogueVoiceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for CharacterDialogueCustomFieldId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for DialogueLocaleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
