//! Canonical locale identity shared by presentation metadata owners.

use core::{fmt, str::FromStr};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::collections::BTreeSet;
use thiserror::Error;

const MAX_LOCALE_TAG_BYTES: usize = 64;
const LOCALE_SEMANTIC_DOMAIN: &[u8] = b"arcweft.id.locale-semantic.v1\0";

/// A canonical locale tag in Arcweft's deterministic ASCII locale subset.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LocaleTag(Box<str>);

/// Stable semantic identity of one canonical [`LocaleTag`].
///
/// The digest is owner-issued so downstream semantic products never hash a
/// locale's display text or duplicate its canonicalization rules.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LocaleSemanticDigest([u8; 32]);

impl LocaleSemanticDigest {
    /// Returns the exact version-one digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Failure to validate or canonicalize a [`LocaleTag`].
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum LocaleTagError {
    #[error("locale tag must not be empty")]
    Empty,
    #[error("locale tag is {bytes} bytes; the maximum is {maximum}")]
    TooLong { bytes: usize, maximum: usize },
    #[error("locale tag contains a non-ASCII byte at offset {byte}")]
    NonAscii { byte: usize },
    #[error("locale tag language must contain 2 through 8 ASCII letters")]
    InvalidLanguage,
    #[error("locale tag has an invalid subtag at index {index}")]
    InvalidSubtag { index: usize },
    #[error("locale tag repeats canonical subtag `{subtag}`")]
    DuplicateCanonicalSubtag { subtag: String },
    #[error("locale tag is not canonical; use `{canonical}`")]
    NonCanonical { canonical: String },
}

impl LocaleTag {
    /// Accepts only an already-canonical locale spelling.
    pub fn try_new(value: impl AsRef<str>) -> Result<Self, LocaleTagError> {
        let value = value.as_ref();
        let canonical = canonical_text(value)?;
        if canonical != value {
            return Err(LocaleTagError::NonCanonical { canonical });
        }
        Ok(Self(canonical.into_boxed_str()))
    }

    /// Validates a locale spelling and returns its canonical representation.
    ///
    /// Acceptance boundaries use [`Self::try_new`]. This conversion is for
    /// diagnostics and explicit owner-approved normalization.
    pub fn canonicalize(value: impl AsRef<str>) -> Result<Self, LocaleTagError> {
        canonical_text(value.as_ref()).map(|value| Self(value.into_boxed_str()))
    }

    /// Returns the exact canonical locale bytes as UTF-8 text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the nominal value and returns its canonical text.
    #[must_use]
    pub fn into_boxed_str(self) -> Box<str> {
        self.0
    }

    /// Issues the semantic identity of this already-canonical locale.
    #[must_use]
    pub fn semantic_digest(&self) -> LocaleSemanticDigest {
        let bytes = self.as_str().as_bytes();
        let mut hasher = blake3::Hasher::new();
        hasher.update(LOCALE_SEMANTIC_DOMAIN);
        hasher.update(bytes);
        LocaleSemanticDigest(*hasher.finalize().as_bytes())
    }
}

impl fmt::Display for LocaleTag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for LocaleTag {
    type Err = LocaleTagError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

impl Serialize for LocaleTag {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for LocaleTag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

fn canonical_text(value: &str) -> Result<String, LocaleTagError> {
    if value.is_empty() {
        return Err(LocaleTagError::Empty);
    }
    if value.len() > MAX_LOCALE_TAG_BYTES {
        return Err(LocaleTagError::TooLong {
            bytes: value.len(),
            maximum: MAX_LOCALE_TAG_BYTES,
        });
    }
    if let Some((byte, _)) = value.bytes().enumerate().find(|(_, byte)| !byte.is_ascii()) {
        return Err(LocaleTagError::NonAscii { byte });
    }

    let subtags = value.split('-').collect::<Vec<_>>();
    let language = subtags[0];
    if !(2..=8).contains(&language.len())
        || !language.bytes().all(|byte| byte.is_ascii_alphabetic())
    {
        return Err(LocaleTagError::InvalidLanguage);
    }

    let mut canonical = Vec::with_capacity(subtags.len());
    canonical.push(language.to_ascii_lowercase());
    let mut cursor = 1;

    if subtags.get(cursor).is_some_and(|subtag| {
        subtag.len() == 4 && subtag.bytes().all(|byte| byte.is_ascii_alphabetic())
    }) {
        let script = subtags[cursor].to_ascii_lowercase();
        let mut bytes = script.into_bytes();
        bytes[0] = bytes[0].to_ascii_uppercase();
        canonical.push(String::from_utf8(bytes).expect("ASCII script remains UTF-8"));
        cursor += 1;
    }

    if subtags.get(cursor).is_some_and(|subtag| {
        (subtag.len() == 2 && subtag.bytes().all(|byte| byte.is_ascii_alphabetic()))
            || (subtag.len() == 3 && subtag.bytes().all(|byte| byte.is_ascii_digit()))
    }) {
        canonical.push(subtags[cursor].to_ascii_uppercase());
        cursor += 1;
    }

    for (index, subtag) in subtags.iter().enumerate().skip(cursor) {
        if !(1..=8).contains(&subtag.len())
            || !subtag.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return Err(LocaleTagError::InvalidSubtag { index });
        }
        canonical.push(subtag.to_ascii_lowercase());
    }

    let mut seen = BTreeSet::new();
    for subtag in &subtags {
        let folded = subtag.to_ascii_lowercase();
        if !seen.insert(folded.clone()) {
            return Err(LocaleTagError::DuplicateCanonicalSubtag { subtag: folded });
        }
    }

    Ok(canonical.join("-"))
}

#[cfg(test)]
mod tests {
    use super::{LocaleTag, LocaleTagError};

    #[test]
    fn strict_construction_accepts_only_canonical_locale_tags() {
        for value in ["en", "ja-JP", "zh-Hant-TW", "en-US-u-ca-gregory"] {
            assert_eq!(LocaleTag::try_new(value).unwrap().as_str(), value);
        }

        assert_eq!(
            LocaleTag::try_new("zh-hant-tw"),
            Err(LocaleTagError::NonCanonical {
                canonical: "zh-Hant-TW".to_owned(),
            })
        );
        assert_eq!(
            LocaleTag::canonicalize("ZH-hant-tw").unwrap().as_str(),
            "zh-Hant-TW"
        );
    }

    #[test]
    fn structural_failures_are_reported_before_canonical_spelling() {
        assert_eq!(LocaleTag::try_new(""), Err(LocaleTagError::Empty));
        assert_eq!(
            LocaleTag::try_new("x"),
            Err(LocaleTagError::InvalidLanguage)
        );
        assert_eq!(
            LocaleTag::try_new("en--US"),
            Err(LocaleTagError::InvalidSubtag { index: 1 })
        );
        assert_eq!(
            LocaleTag::try_new("ja_日本"),
            Err(LocaleTagError::NonAscii { byte: 3 })
        );
        assert_eq!(
            LocaleTag::try_new("en-US-us"),
            Err(LocaleTagError::DuplicateCanonicalSubtag {
                subtag: "us".to_owned(),
            })
        );
    }

    #[test]
    fn locale_byte_limit_is_exact() {
        let subtags = (0..10)
            .map(|index| format!("a{index:04}"))
            .collect::<Vec<_>>();
        let exact = format!("en-{}", subtags.join("-"));
        assert_eq!(exact.len(), 62);
        let exact = format!("{exact}-x");
        assert_eq!(exact.len(), 64);
        assert!(LocaleTag::try_new(&exact).is_ok());

        let one_over = format!("{exact}x");
        assert_eq!(
            LocaleTag::try_new(&one_over),
            Err(LocaleTagError::TooLong {
                bytes: 65,
                maximum: 64,
            })
        );
    }

    #[test]
    fn serde_accepts_only_canonical_locale_text() {
        let accepted: LocaleTag = serde_json::from_str("\"zh-Hant-TW\"").unwrap();
        assert_eq!(accepted.as_str(), "zh-Hant-TW");
        assert_eq!(serde_json::to_string(&accepted).unwrap(), "\"zh-Hant-TW\"");
        assert!(serde_json::from_str::<LocaleTag>("\"zh-hant-tw\"").is_err());
    }

    #[test]
    fn semantic_digest_is_owned_by_the_canonical_locale() {
        let canonical = LocaleTag::try_new("zh-Hant-TW").unwrap();
        let normalized = LocaleTag::canonicalize("ZH-hant-tw").unwrap();
        let different = LocaleTag::try_new("zh-Hans-TW").unwrap();

        assert_eq!(canonical.semantic_digest(), normalized.semantic_digest());
        assert_ne!(canonical.semantic_digest(), different.semantic_digest());
        assert_eq!(canonical.semantic_digest().as_bytes().len(), 32);
    }
}
