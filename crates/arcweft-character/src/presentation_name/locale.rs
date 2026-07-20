//! Nominal Character-name locale roles and fallback policy.

use super::limits::MAX_FALLBACK_LOCALES;
use arcweft_id::LocaleTag;
use core::fmt;
use std::collections::BTreeMap;
use thiserror::Error;

/// Locale identity used by Character display-name metadata.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterNameLocale(LocaleTag);

/// Character-declared locale used after project fallback locales.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterNameSourceLocale(CharacterNameLocale);

/// One authored locale in the ordered project fallback policy.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterNameFallbackLocale(CharacterNameLocale);

/// Ordered Character display-name locale policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterNameLocalePolicy {
    default_active: CharacterNameLocale,
    fallbacks: Box<[CharacterNameFallbackLocale]>,
}

/// Invalid Character display-name locale policy.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterNameLocalePolicyError {
    #[error("Character-name fallback count {observed} exceeds maximum {maximum}")]
    TooManyFallbacks { observed: usize, maximum: usize },
    #[error("Character-name fallback `{locale}` at ordinal {duplicate} duplicates ordinal {first}")]
    DuplicateFallback {
        locale: CharacterNameLocale,
        first: u32,
        duplicate: u32,
    },
    #[error(
        "Character-name fallback `{locale}` at ordinal {ordinal} repeats the default active locale"
    )]
    RepeatsDefaultActive {
        locale: CharacterNameLocale,
        ordinal: u32,
    },
    #[error("Character-name fallback ordinal exceeds the supported diagnostic range")]
    OrdinalOverflow,
}

impl CharacterNameLocale {
    #[must_use]
    pub const fn new(value: LocaleTag) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn locale_tag(&self) -> &LocaleTag {
        &self.0
    }
}

impl fmt::Display for CharacterNameLocale {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.locale_tag().as_str())
    }
}

impl fmt::Display for CharacterNameSourceLocale {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.locale().fmt(formatter)
    }
}

impl fmt::Display for CharacterNameFallbackLocale {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.locale().fmt(formatter)
    }
}

impl CharacterNameSourceLocale {
    #[must_use]
    pub const fn new(value: CharacterNameLocale) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn locale(&self) -> &CharacterNameLocale {
        &self.0
    }
}

impl CharacterNameFallbackLocale {
    #[must_use]
    pub const fn new(value: CharacterNameLocale) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn locale(&self) -> &CharacterNameLocale {
        &self.0
    }
}

impl CharacterNameLocalePolicy {
    pub fn try_new(
        default_active: CharacterNameLocale,
        fallbacks: Vec<CharacterNameFallbackLocale>,
    ) -> Result<Self, CharacterNameLocalePolicyError> {
        if fallbacks.len() > MAX_FALLBACK_LOCALES {
            return Err(CharacterNameLocalePolicyError::TooManyFallbacks {
                observed: fallbacks.len(),
                maximum: MAX_FALLBACK_LOCALES,
            });
        }

        let mut first_ordinals = BTreeMap::new();
        for (ordinal, fallback) in fallbacks.iter().enumerate() {
            let ordinal = u32::try_from(ordinal)
                .map_err(|_| CharacterNameLocalePolicyError::OrdinalOverflow)?;
            if fallback.locale() == &default_active {
                return Err(CharacterNameLocalePolicyError::RepeatsDefaultActive {
                    locale: fallback.locale().clone(),
                    ordinal,
                });
            }
            if let Some(first) = first_ordinals.insert(fallback.locale().clone(), ordinal) {
                return Err(CharacterNameLocalePolicyError::DuplicateFallback {
                    locale: fallback.locale().clone(),
                    first,
                    duplicate: ordinal,
                });
            }
        }

        Ok(Self {
            default_active,
            fallbacks: fallbacks.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn default_active(&self) -> &CharacterNameLocale {
        &self.default_active
    }

    #[must_use]
    pub fn fallbacks(&self) -> &[CharacterNameFallbackLocale] {
        &self.fallbacks
    }
}
