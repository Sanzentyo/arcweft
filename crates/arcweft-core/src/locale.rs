//! Canonical locale identity shared across authored and runtime boundaries.

use core::{fmt, str::FromStr};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

/// Maximum encoded size of one canonical locale identity.
pub const MAX_LOCALE_ID_BYTES: usize = 64;

/// Canonical ASCII BCP-47 locale identity.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct LocaleId(String);

/// Why a locale identity could not be validated and canonicalized.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum LocaleIdErrorKind {
    #[error("locale must not be empty")]
    Empty,
    #[error("locale must contain at most 64 bytes")]
    TooLong,
    #[error("locale must contain only non-control ASCII")]
    NonAscii,
    #[error("locale contains an invalid subtag")]
    InvalidSubtag,
    #[error("language subtag must contain 2..=8 ASCII letters")]
    InvalidLanguage,
    #[error("locale contains a duplicate subtag")]
    DuplicateSubtag,
}

/// Invalid locale identity input.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("invalid locale `{value}`: {kind}")]
pub struct LocaleIdError {
    value: String,
    kind: LocaleIdErrorKind,
}

impl LocaleId {
    /// Validates and canonicalizes an ASCII BCP-47 locale.
    pub fn try_new(value: impl Into<String>) -> Result<Self, LocaleIdError> {
        let value = value.into();
        let canonical = canonicalize(&value).map_err(|kind| LocaleIdError {
            value: value.clone(),
            kind,
        })?;
        Ok(Self(canonical))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl LocaleIdErrorKind {
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::Empty => "locale must not be empty",
            Self::TooLong => "locale must contain at most 64 bytes",
            Self::NonAscii => "locale must contain only non-control ASCII",
            Self::InvalidSubtag => "locale contains an invalid subtag",
            Self::InvalidLanguage => "language subtag must contain 2..=8 ASCII letters",
            Self::DuplicateSubtag => "locale contains a duplicate subtag",
        }
    }
}

impl LocaleIdError {
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub const fn kind(&self) -> LocaleIdErrorKind {
        self.kind
    }

    #[must_use]
    pub fn into_value(self) -> String {
        self.value
    }
}

impl fmt::Display for LocaleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl AsRef<str> for LocaleId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for LocaleId {
    type Err = LocaleIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

impl TryFrom<String> for LocaleId {
    type Error = LocaleIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<LocaleId> for String {
    fn from(value: LocaleId) -> Self {
        value.0
    }
}

fn canonicalize(value: &str) -> Result<String, LocaleIdErrorKind> {
    if value.is_empty() {
        return Err(LocaleIdErrorKind::Empty);
    }
    if value.len() > MAX_LOCALE_ID_BYTES {
        return Err(LocaleIdErrorKind::TooLong);
    }
    if !value.is_ascii() || value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(LocaleIdErrorKind::NonAscii);
    }

    let subtags = value.split('-').collect::<Vec<_>>();
    if subtags.iter().any(|subtag| {
        subtag.is_empty()
            || subtag.len() > 8
            || !subtag.bytes().all(|byte| byte.is_ascii_alphanumeric())
    }) {
        return Err(LocaleIdErrorKind::InvalidSubtag);
    }
    let Some(language) = subtags.first() else {
        return Err(LocaleIdErrorKind::Empty);
    };
    if !(2..=8).contains(&language.len())
        || !language.bytes().all(|byte| byte.is_ascii_alphabetic())
    {
        return Err(LocaleIdErrorKind::InvalidLanguage);
    }

    let mut canonical = Vec::with_capacity(subtags.len());
    let mut seen = BTreeSet::new();
    for (index, subtag) in subtags.into_iter().enumerate() {
        let lower = subtag.to_ascii_lowercase();
        if index != 0 && !seen.insert(lower.clone()) {
            return Err(LocaleIdErrorKind::DuplicateSubtag);
        }
        if index == 0 {
            canonical.push(lower);
        } else if subtag.len() == 4 && subtag.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            let mut characters = lower.chars();
            let Some(first) = characters.next() else {
                return Err(LocaleIdErrorKind::InvalidSubtag);
            };
            canonical.push(format!(
                "{}{}",
                first.to_ascii_uppercase(),
                characters.as_str()
            ));
        } else if (subtag.len() == 2 && subtag.bytes().all(|byte| byte.is_ascii_alphabetic()))
            || (subtag.len() == 3 && subtag.bytes().all(|byte| byte.is_ascii_digit()))
        {
            canonical.push(subtag.to_ascii_uppercase());
        } else {
            canonical.push(lower);
        }
    }
    Ok(canonical.join("-"))
}

#[cfg(test)]
mod tests {
    use super::{LocaleId, LocaleIdErrorKind};

    #[test]
    fn locale_identity_canonicalizes_language_script_and_region() {
        assert_eq!(LocaleId::try_new("ja-jp").unwrap().as_str(), "ja-JP");
        assert_eq!(LocaleId::try_new("de-de").unwrap().as_str(), "de-DE");
        assert_eq!(
            LocaleId::try_new("zh-hant-tw").unwrap().as_str(),
            "zh-Hant-TW"
        );
    }

    #[test]
    fn locale_identity_rejects_malformed_and_duplicate_subtags() {
        for (value, kind) in [
            ("", LocaleIdErrorKind::Empty),
            ("e", LocaleIdErrorKind::InvalidLanguage),
            ("en-abcdefghi", LocaleIdErrorKind::InvalidSubtag),
            ("é-JP", LocaleIdErrorKind::NonAscii),
            ("en-US-us", LocaleIdErrorKind::DuplicateSubtag),
            (
                "en-abcdefgh-abcdefgh-abcdefgh-abcdefgh-abcdefgh-abcdefgh-abcdefgh-abcdefgh",
                LocaleIdErrorKind::TooLong,
            ),
        ] {
            assert_eq!(LocaleId::try_new(value).unwrap_err().kind(), kind);
        }
    }

    #[test]
    fn serde_cannot_bypass_locale_validation() {
        assert!(serde_json::from_str::<LocaleId>("\"e\"").is_err());
        let canonical: LocaleId = serde_json::from_str("\"ja-jp\"").unwrap();
        assert_eq!(canonical.as_str(), "ja-JP");
        assert_eq!(serde_json::to_string(&canonical).unwrap(), "\"ja-JP\"");
    }
}
