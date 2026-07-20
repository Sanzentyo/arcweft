//! Typed launch-profile localization policy.

use arcweft_id::LocaleTag;
use serde::{Deserialize, Deserializer, Serialize, de};
use std::collections::BTreeMap;
use thiserror::Error;

/// Maximum ordered Character-name fallback locales in one selected profile.
pub const MAX_PROFILE_CHARACTER_NAME_FALLBACKS: usize = 16;

/// Localization policy retained by one authored launch profile.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileLocalizationSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    character_names: Option<CharacterNameLocalePolicySpec>,
}

/// Strict default locale and ordered Character-name fallbacks.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterNameLocalePolicySpec {
    active: LocaleTag,
    fallbacks: Box<[LocaleTag]>,
}

/// Failure to construct a valid Character-name locale policy.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterNameLocalePolicySpecError {
    #[error("Character-name fallback count {observed} exceeds maximum {maximum}")]
    TooManyFallbacks { observed: usize, maximum: usize },
    #[error("Character-name fallback `{locale}` at ordinal {duplicate} duplicates ordinal {first}")]
    DuplicateFallback {
        locale: LocaleTag,
        first: u16,
        duplicate: u16,
    },
    #[error("Character-name fallback `{locale}` at ordinal {ordinal} repeats the active locale")]
    ActiveRepeated { locale: LocaleTag, ordinal: u16 },
}

impl ProfileLocalizationSpec {
    pub const fn new(character_names: Option<CharacterNameLocalePolicySpec>) -> Self {
        Self { character_names }
    }

    pub const fn character_names(&self) -> Option<&CharacterNameLocalePolicySpec> {
        self.character_names.as_ref()
    }
}

impl CharacterNameLocalePolicySpec {
    pub fn try_new(
        active: LocaleTag,
        fallbacks: impl Into<Box<[LocaleTag]>>,
    ) -> Result<Self, CharacterNameLocalePolicySpecError> {
        let fallbacks = fallbacks.into();
        if fallbacks.len() > MAX_PROFILE_CHARACTER_NAME_FALLBACKS {
            return Err(CharacterNameLocalePolicySpecError::TooManyFallbacks {
                observed: fallbacks.len(),
                maximum: MAX_PROFILE_CHARACTER_NAME_FALLBACKS,
            });
        }

        let mut first_ordinals = BTreeMap::new();
        let mut ordinal = 0_u16;
        for fallback in &fallbacks {
            if fallback == &active {
                return Err(CharacterNameLocalePolicySpecError::ActiveRepeated {
                    locale: fallback.clone(),
                    ordinal,
                });
            }
            if let Some(first) = first_ordinals.insert(fallback.clone(), ordinal) {
                return Err(CharacterNameLocalePolicySpecError::DuplicateFallback {
                    locale: fallback.clone(),
                    first,
                    duplicate: ordinal,
                });
            }
            ordinal = ordinal.saturating_add(1);
        }

        Ok(Self { active, fallbacks })
    }

    pub const fn active(&self) -> &LocaleTag {
        &self.active
    }

    pub fn fallbacks(&self) -> &[LocaleTag] {
        &self.fallbacks
    }
}

impl<'de> Deserialize<'de> for CharacterNameLocalePolicySpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Value {
            active: LocaleTag,
            #[serde(default)]
            fallbacks: Box<[LocaleTag]>,
        }

        let value = Value::deserialize(deserializer)?;
        Self::try_new(value.active, value.fallbacks).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CharacterNameLocalePolicySpec, CharacterNameLocalePolicySpecError,
        MAX_PROFILE_CHARACTER_NAME_FALLBACKS, ProfileLocalizationSpec,
    };
    use arcweft_id::LocaleTag;

    fn locale(value: &str) -> LocaleTag {
        LocaleTag::try_new(value).expect("canonical locale")
    }

    #[test]
    fn policy_retains_order_and_rejects_duplicates() {
        let policy =
            CharacterNameLocalePolicySpec::try_new(locale("ja-JP"), [locale("en"), locale("fr")])
                .unwrap();
        assert_eq!(policy.active().as_str(), "ja-JP");
        assert_eq!(
            policy
                .fallbacks()
                .iter()
                .map(LocaleTag::as_str)
                .collect::<Vec<_>>(),
            ["en", "fr"]
        );

        assert_eq!(
            CharacterNameLocalePolicySpec::try_new(locale("ja-JP"), [locale("en"), locale("en")]),
            Err(CharacterNameLocalePolicySpecError::DuplicateFallback {
                locale: locale("en"),
                first: 0,
                duplicate: 1,
            })
        );
        assert_eq!(
            CharacterNameLocalePolicySpec::try_new(locale("ja-JP"), [locale("ja-JP")]),
            Err(CharacterNameLocalePolicySpecError::ActiveRepeated {
                locale: locale("ja-JP"),
                ordinal: 0,
            })
        );
    }

    #[test]
    fn fallback_limit_is_exact() {
        let exact = (0..MAX_PROFILE_CHARACTER_NAME_FALLBACKS)
            .map(|index| locale(&format!("qaa-x{index}")))
            .collect::<Vec<_>>();
        assert!(CharacterNameLocalePolicySpec::try_new(locale("ja-JP"), exact.clone()).is_ok());

        let mut one_over = exact;
        one_over.push(locale("qaa-x16"));
        assert_eq!(
            CharacterNameLocalePolicySpec::try_new(locale("ja-JP"), one_over),
            Err(CharacterNameLocalePolicySpecError::TooManyFallbacks {
                observed: 17,
                maximum: MAX_PROFILE_CHARACTER_NAME_FALLBACKS,
            })
        );
    }

    #[test]
    fn serde_is_strict_and_defaults_fallbacks() {
        let policy: CharacterNameLocalePolicySpec =
            serde_json::from_str(r#"{"active":"ja-JP"}"#).unwrap();
        assert!(policy.fallbacks().is_empty());
        assert!(
            serde_json::from_str::<CharacterNameLocalePolicySpec>(
                r#"{"active":"ja-jp","fallbacks":[]}"#
            )
            .is_err()
        );

        let localization = ProfileLocalizationSpec::new(Some(policy));
        assert!(localization.character_names().is_some());
    }
}
