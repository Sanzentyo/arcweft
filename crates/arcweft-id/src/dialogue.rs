//! Durable dialogue identities shared by language and runtime layers.

use crate::{IdError, PublicId, TextKey};
use core::{fmt, str::FromStr};
use thiserror::Error;

const LINE_FAMILY: &str = "say";
const TEXT_FAMILY: &str = "text";

/// Maximum UTF-8 byte length of a durable dialogue line ID or text key.
pub const MAX_DIALOGUE_ID_BYTES: usize = 256;

/// Stable public identity of one authored dialogue line.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DialogueLineId(PublicId);

/// Stable localization key associated with one authored dialogue line.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DialogueTextKey(TextKey);

/// Dialogue identity domain whose validation failed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueIdentityKind {
    Line,
    TextKey,
}

/// Failure to construct a durable dialogue identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DialogueIdentityError {
    #[error("invalid dialogue {kind} identity: {source}")]
    InvalidBase {
        kind: DialogueIdentityKind,
        #[source]
        source: IdError,
    },
    #[error("dialogue {kind} identity `{value}` must belong to the `{expected}.` family")]
    WrongFamily {
        kind: DialogueIdentityKind,
        expected: &'static str,
        value: String,
    },
    #[error("dialogue {kind} identity must have a nonempty `{family}.` tail")]
    EmptyTail {
        kind: DialogueIdentityKind,
        family: &'static str,
    },
    #[error("dialogue {kind} identity is {bytes} UTF-8 bytes; the maximum is {maximum}")]
    TooManyBytes {
        kind: DialogueIdentityKind,
        bytes: usize,
        maximum: usize,
    },
}

impl DialogueLineId {
    /// Validates and constructs an identity in the exact `say.*` family.
    pub fn try_new(value: impl Into<String>) -> Result<Self, DialogueIdentityError> {
        let value =
            PublicId::try_new(value).map_err(|source| DialogueIdentityError::InvalidBase {
                kind: DialogueIdentityKind::Line,
                source,
            })?;
        validate_family(value.as_str(), DialogueIdentityKind::Line, LINE_FAMILY)?;
        Ok(Self(value))
    }

    /// Returns the validated public identity.
    pub const fn as_public_id(&self) -> &PublicId {
        &self.0
    }

    /// Returns the exact source/public spelling without an `@` marker.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Extracts the validated public identity.
    pub fn into_public_id(self) -> PublicId {
        self.0
    }
}

impl DialogueTextKey {
    /// Validates and constructs a key in the exact `text.*` family.
    pub fn try_new(value: impl Into<String>) -> Result<Self, DialogueIdentityError> {
        let value =
            TextKey::try_new(value).map_err(|source| DialogueIdentityError::InvalidBase {
                kind: DialogueIdentityKind::TextKey,
                source,
            })?;
        validate_family(value.as_str(), DialogueIdentityKind::TextKey, TEXT_FAMILY)?;
        Ok(Self(value))
    }

    /// Returns the validated text key.
    pub const fn as_text_key(&self) -> &TextKey {
        &self.0
    }

    /// Returns the exact localization-key spelling.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Extracts the validated text key.
    pub fn into_text_key(self) -> TextKey {
        self.0
    }
}

impl DialogueIdentityKind {
    const fn noun(self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::TextKey => "text key",
        }
    }
}

impl fmt::Display for DialogueIdentityKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.noun())
    }
}

impl fmt::Display for DialogueLineId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for DialogueTextKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for DialogueLineId {
    type Err = DialogueIdentityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

impl FromStr for DialogueTextKey {
    type Err = DialogueIdentityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

impl TryFrom<PublicId> for DialogueLineId {
    type Error = DialogueIdentityError;

    fn try_from(value: PublicId) -> Result<Self, Self::Error> {
        validate_family(value.as_str(), DialogueIdentityKind::Line, LINE_FAMILY)?;
        Ok(Self(value))
    }
}

impl TryFrom<TextKey> for DialogueTextKey {
    type Error = DialogueIdentityError;

    fn try_from(value: TextKey) -> Result<Self, Self::Error> {
        validate_family(value.as_str(), DialogueIdentityKind::TextKey, TEXT_FAMILY)?;
        Ok(Self(value))
    }
}

impl From<DialogueLineId> for PublicId {
    fn from(value: DialogueLineId) -> Self {
        value.0
    }
}

impl From<DialogueTextKey> for TextKey {
    fn from(value: DialogueTextKey) -> Self {
        value.0
    }
}

fn validate_family(
    value: &str,
    kind: DialogueIdentityKind,
    family: &'static str,
) -> Result<(), DialogueIdentityError> {
    let Some(tail) = value
        .strip_prefix(family)
        .and_then(|suffix| suffix.strip_prefix('.'))
    else {
        return Err(DialogueIdentityError::WrongFamily {
            kind,
            expected: family,
            value: value.to_owned(),
        });
    };
    if tail.is_empty() {
        return Err(DialogueIdentityError::EmptyTail { kind, family });
    }
    if value.len() > MAX_DIALOGUE_ID_BYTES {
        return Err(DialogueIdentityError::TooManyBytes {
            kind,
            bytes: value.len(),
            maximum: MAX_DIALOGUE_ID_BYTES,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        DialogueIdentityError, DialogueIdentityKind, DialogueLineId, DialogueTextKey, LINE_FAMILY,
        MAX_DIALOGUE_ID_BYTES, TEXT_FAMILY,
    };

    #[test]
    fn dialogue_line_id_accepts_exact_say_family() {
        let id = DialogueLineId::try_new("say.opening.greeting").expect("valid dialogue line ID");

        assert_eq!(id.as_str(), "say.opening.greeting");
        assert_eq!(id.as_public_id().as_str(), "say.opening.greeting");
        assert_eq!(id.into_public_id().as_str(), "say.opening.greeting");
    }

    #[test]
    fn dialogue_line_id_rejects_line_alias_family() {
        assert_eq!(
            DialogueLineId::try_new("line.opening.greeting"),
            Err(DialogueIdentityError::WrongFamily {
                kind: DialogueIdentityKind::Line,
                expected: LINE_FAMILY,
                value: "line.opening.greeting".to_owned(),
            })
        );
    }

    #[test]
    fn dialogue_text_key_accepts_exact_text_family() {
        let key =
            DialogueTextKey::try_new("text.opening.greeting").expect("valid dialogue text key");

        assert_eq!(key.as_str(), "text.opening.greeting");
        assert_eq!(key.as_text_key().as_str(), "text.opening.greeting");
        assert_eq!(key.into_text_key().as_str(), "text.opening.greeting");
    }

    #[test]
    fn dialogue_line_id_accepts_exact_256_utf8_bytes() {
        let value = format!("say.{}abc", "界".repeat(83));
        assert_eq!(value.len(), MAX_DIALOGUE_ID_BYTES);

        assert_eq!(
            DialogueLineId::try_new(value.clone())
                .expect("inclusive byte limit accepts the identity")
                .as_str(),
            value
        );
    }

    #[test]
    fn dialogue_line_id_rejects_257_utf8_bytes() {
        let value = format!("say.{}abcd", "界".repeat(83));
        assert_eq!(value.len(), MAX_DIALOGUE_ID_BYTES + 1);

        assert_eq!(
            DialogueLineId::try_new(value),
            Err(DialogueIdentityError::TooManyBytes {
                kind: DialogueIdentityKind::Line,
                bytes: MAX_DIALOGUE_ID_BYTES + 1,
                maximum: MAX_DIALOGUE_ID_BYTES,
            })
        );
    }

    #[test]
    fn dialogue_identity_families_require_nonempty_tails() {
        assert_eq!(
            DialogueLineId::try_new("say."),
            Err(DialogueIdentityError::EmptyTail {
                kind: DialogueIdentityKind::Line,
                family: LINE_FAMILY,
            })
        );
        assert_eq!(
            DialogueTextKey::try_new("text."),
            Err(DialogueIdentityError::EmptyTail {
                kind: DialogueIdentityKind::TextKey,
                family: TEXT_FAMILY,
            })
        );
    }
}
