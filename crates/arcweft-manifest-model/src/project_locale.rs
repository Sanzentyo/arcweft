//! Project-wide source locale, default locale, and ordered content fallbacks.

use arcweft_id::LocaleTag;
use serde::{Deserialize, Deserializer, Serialize, de};
use std::collections::BTreeMap;
use thiserror::Error;

/// Maximum ordered project fallback locales.
pub const MAX_PROJECT_LOCALE_FALLBACKS: usize = 16;

/// Locale policy shared by all launch profiles in one project.
///
/// An omitted policy, source locale, or default locale uses Japanese. Fallbacks
/// are explicitly ordered and may include the default locale.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectLocaleSpec {
    source: LocaleTag,
    default: LocaleTag,
    fallback: Box<[LocaleTag]>,
}

/// Failure to construct a valid project locale policy.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProjectLocaleSpecError {
    #[error("project fallback count {observed} exceeds maximum {maximum}")]
    TooManyFallbacks { observed: usize, maximum: usize },
    #[error("project fallback `{locale}` at ordinal {duplicate} duplicates ordinal {first}")]
    DuplicateFallback {
        locale: LocaleTag,
        first: u16,
        duplicate: u16,
    },
}

impl ProjectLocaleSpec {
    pub fn try_new(
        source: LocaleTag,
        default: LocaleTag,
        fallback: impl Into<Box<[LocaleTag]>>,
    ) -> Result<Self, ProjectLocaleSpecError> {
        let fallback = fallback.into();
        if fallback.len() > MAX_PROJECT_LOCALE_FALLBACKS {
            return Err(ProjectLocaleSpecError::TooManyFallbacks {
                observed: fallback.len(),
                maximum: MAX_PROJECT_LOCALE_FALLBACKS,
            });
        }

        let mut first_ordinals = BTreeMap::new();
        for (ordinal, locale) in fallback.iter().enumerate() {
            let ordinal = u16::try_from(ordinal).expect("fallback count is bounded");
            if let Some(first) = first_ordinals.insert(locale.clone(), ordinal) {
                return Err(ProjectLocaleSpecError::DuplicateFallback {
                    locale: locale.clone(),
                    first,
                    duplicate: ordinal,
                });
            }
        }

        Ok(Self {
            source,
            default,
            fallback,
        })
    }

    pub const fn source(&self) -> &LocaleTag {
        &self.source
    }

    pub const fn default_locale(&self) -> &LocaleTag {
        &self.default
    }

    pub fn fallback(&self) -> &[LocaleTag] {
        &self.fallback
    }
}

impl Default for ProjectLocaleSpec {
    fn default() -> Self {
        let japanese = japanese_locale();
        Self {
            source: japanese.clone(),
            default: japanese,
            fallback: Box::default(),
        }
    }
}

impl<'de> Deserialize<'de> for ProjectLocaleSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Value {
            #[serde(default = "japanese_locale")]
            source: LocaleTag,
            #[serde(default = "japanese_locale")]
            default: LocaleTag,
            #[serde(default)]
            fallback: Box<[LocaleTag]>,
        }

        let value = Value::deserialize(deserializer)?;
        Self::try_new(value.source, value.default, value.fallback).map_err(de::Error::custom)
    }
}

fn japanese_locale() -> LocaleTag {
    LocaleTag::try_new("ja-JP").expect("built-in project locale is canonical")
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_PROJECT_LOCALE_FALLBACKS, ProjectLocaleSpec, ProjectLocaleSpecError, japanese_locale,
    };
    use arcweft_id::LocaleTag;

    fn locale(value: &str) -> LocaleTag {
        LocaleTag::try_new(value).expect("canonical locale")
    }

    #[test]
    fn default_policy_is_japanese_with_no_implicit_fallbacks() {
        let policy = ProjectLocaleSpec::default();
        assert_eq!(policy.source(), &japanese_locale());
        assert_eq!(policy.default_locale(), &japanese_locale());
        assert!(policy.fallback().is_empty());
    }

    #[test]
    fn fallback_order_is_retained_and_default_may_be_repeated_as_fallback() {
        let policy = ProjectLocaleSpec::try_new(
            locale("ja-JP"),
            locale("en-US"),
            [locale("fr-FR"), locale("en-US")],
        )
        .unwrap();
        assert_eq!(policy.source().as_str(), "ja-JP");
        assert_eq!(policy.default_locale().as_str(), "en-US");
        assert_eq!(
            policy
                .fallback()
                .iter()
                .map(LocaleTag::as_str)
                .collect::<Vec<_>>(),
            ["fr-FR", "en-US"]
        );

        assert_eq!(
            ProjectLocaleSpec::try_new(
                locale("ja-JP"),
                locale("en-US"),
                [locale("fr-FR"), locale("fr-FR")]
            ),
            Err(ProjectLocaleSpecError::DuplicateFallback {
                locale: locale("fr-FR"),
                first: 0,
                duplicate: 1,
            })
        );
    }

    #[test]
    fn fallback_limit_is_exact() {
        let exact = (0..MAX_PROJECT_LOCALE_FALLBACKS)
            .map(|index| locale(&format!("qaa-x{index}")))
            .collect::<Vec<_>>();
        assert!(
            ProjectLocaleSpec::try_new(locale("ja-JP"), locale("ja-JP"), exact.clone()).is_ok()
        );

        let mut one_over = exact;
        one_over.push(locale("qaa-x16"));
        assert_eq!(
            ProjectLocaleSpec::try_new(locale("ja-JP"), locale("ja-JP"), one_over),
            Err(ProjectLocaleSpecError::TooManyFallbacks {
                observed: 17,
                maximum: MAX_PROJECT_LOCALE_FALLBACKS,
            })
        );
    }

    #[test]
    fn serde_round_trips_and_defaults_omitted_fields_strictly() {
        let encoded = serde_json::to_string(&ProjectLocaleSpec::default()).unwrap();
        let decoded: ProjectLocaleSpec = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, ProjectLocaleSpec::default());

        let partial: ProjectLocaleSpec = serde_json::from_str(r#"{"fallback":["ja-JP"]}"#).unwrap();
        assert_eq!(
            partial,
            ProjectLocaleSpec::try_new(locale("ja-JP"), locale("ja-JP"), [locale("ja-JP")])
                .unwrap()
        );
        assert!(
            serde_json::from_str::<ProjectLocaleSpec>(
                r#"{"source":"ja-jp","default":"ja-JP","fallback":[]}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<ProjectLocaleSpec>(
                r#"{"source":"ja-JP","default":"ja-JP","fallback":[],"extraction":{}}"#
            )
            .is_err()
        );
    }
}
