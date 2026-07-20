//! Checked display-name values, generated text keys, and accepted entries.

use super::{
    CharacterNameLocale,
    limits::{
        MAX_CHARACTER_DISPLAY_NAME_BYTES, MAX_CHARACTER_DISPLAY_NAME_SCALARS,
        MAX_CHARACTER_ID_BYTES, MAX_GENERATED_DISPLAY_NAME_KEY_BYTES,
    },
};
use crate::id::CharacterId;
use arcweft_id::{IdError, TextKey};
use core::fmt::{self, Write};
use thiserror::Error;

const DISPLAY_NAME_KEY_PREFIX: &str = "character.display_name.";

/// A nonempty visible Character display name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDisplayNameValue(Box<str>);

/// Generated localization key for one accepted Character display name.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterDisplayNameKey(TextKey);

/// Source/build input before generated keys are attached.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterDisplayNameInput {
    Visible(CharacterDisplayNameValue),
    Hidden,
}

/// Accepted Character display-name entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterDisplayNameEntry {
    Visible {
        key: CharacterDisplayNameKey,
        value: CharacterDisplayNameValue,
    },
    Hidden,
}

/// One locale-exact display-name input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalizedCharacterDisplayNameInput {
    locale: CharacterNameLocale,
    entry: CharacterDisplayNameInput,
}

/// One accepted, locale-exact display-name entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalizedCharacterDisplayName {
    locale: CharacterNameLocale,
    entry: CharacterDisplayNameEntry,
}

/// Accepted fallback generated from a declaration's semantic local name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDeclarationNameFallback {
    key: CharacterDisplayNameKey,
    value: CharacterDisplayNameValue,
}

/// Invalid visible Character display-name metadata.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterDisplayNameValueError {
    #[error("visible Character display name must not be empty")]
    Empty,
    #[error("visible Character display name is {bytes} bytes; the maximum is {maximum}")]
    TooManyBytes { bytes: usize, maximum: usize },
    #[error("visible Character display name has {scalars} scalars; the maximum is {maximum}")]
    TooManyScalars { scalars: usize, maximum: usize },
    #[error("visible Character display name must not start with Unicode whitespace")]
    LeadingWhitespace,
    #[error("visible Character display name must not end with Unicode whitespace")]
    TrailingWhitespace,
    #[error("visible Character display name contains a control scalar at index {scalar_index}")]
    Control { scalar_index: usize },
    #[error("visible Character display name must contain a non-whitespace scalar")]
    WhitespaceOnly,
}

/// Failure to generate a canonical Character display-name key.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterDisplayNameKeyError {
    #[error("Character ID is {bytes} bytes; the catalog-local maximum is {maximum}")]
    CharacterIdTooLong { bytes: usize, maximum: usize },
    #[error("generated Character display-name key is {bytes} bytes; the maximum is {maximum}")]
    GeneratedKeyTooLong { bytes: usize, maximum: usize },
    #[error("generated Character display-name key is invalid")]
    InvalidTextKey(#[from] IdError),
}

impl CharacterDisplayNameValue {
    pub fn try_new(value: impl Into<String>) -> Result<Self, CharacterDisplayNameValueError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CharacterDisplayNameValueError::Empty);
        }
        if value.len() > MAX_CHARACTER_DISPLAY_NAME_BYTES {
            return Err(CharacterDisplayNameValueError::TooManyBytes {
                bytes: value.len(),
                maximum: MAX_CHARACTER_DISPLAY_NAME_BYTES,
            });
        }

        let scalar_count = value.chars().count();
        if scalar_count > MAX_CHARACTER_DISPLAY_NAME_SCALARS {
            return Err(CharacterDisplayNameValueError::TooManyScalars {
                scalars: scalar_count,
                maximum: MAX_CHARACTER_DISPLAY_NAME_SCALARS,
            });
        }
        if !value.chars().any(|scalar| !scalar.is_whitespace()) {
            return Err(CharacterDisplayNameValueError::WhitespaceOnly);
        }
        if value.chars().next().is_some_and(char::is_whitespace) {
            return Err(CharacterDisplayNameValueError::LeadingWhitespace);
        }
        if value.chars().next_back().is_some_and(char::is_whitespace) {
            return Err(CharacterDisplayNameValueError::TrailingWhitespace);
        }
        if let Some((scalar_index, _)) = value
            .chars()
            .enumerate()
            .find(|(_, scalar)| scalar.is_control())
        {
            return Err(CharacterDisplayNameValueError::Control { scalar_index });
        }

        Ok(Self(value.into_boxed_str()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl CharacterDisplayNameKey {
    pub fn for_base(character: &CharacterId) -> Result<Self, CharacterDisplayNameKeyError> {
        generate_key(character, None, "base")
    }

    pub fn for_locale(
        character: &CharacterId,
        locale: &CharacterNameLocale,
    ) -> Result<Self, CharacterDisplayNameKeyError> {
        generate_key(character, Some(locale), "locale")
    }

    pub fn for_declaration(character: &CharacterId) -> Result<Self, CharacterDisplayNameKeyError> {
        generate_key(character, None, "declaration")
    }

    #[must_use]
    pub const fn text_key(&self) -> &TextKey {
        &self.0
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for CharacterDisplayNameKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl CharacterDisplayNameEntry {
    #[must_use]
    pub const fn key(&self) -> Option<&CharacterDisplayNameKey> {
        match self {
            Self::Visible { key, .. } => Some(key),
            Self::Hidden => None,
        }
    }

    #[must_use]
    pub const fn value(&self) -> Option<&CharacterDisplayNameValue> {
        match self {
            Self::Visible { value, .. } => Some(value),
            Self::Hidden => None,
        }
    }

    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        matches!(self, Self::Hidden)
    }
}

impl LocalizedCharacterDisplayNameInput {
    #[must_use]
    pub const fn new(locale: CharacterNameLocale, entry: CharacterDisplayNameInput) -> Self {
        Self { locale, entry }
    }

    #[must_use]
    pub const fn locale(&self) -> &CharacterNameLocale {
        &self.locale
    }

    #[must_use]
    pub const fn entry(&self) -> &CharacterDisplayNameInput {
        &self.entry
    }
}

impl LocalizedCharacterDisplayName {
    pub(super) const fn new(locale: CharacterNameLocale, entry: CharacterDisplayNameEntry) -> Self {
        Self { locale, entry }
    }

    #[must_use]
    pub const fn locale(&self) -> &CharacterNameLocale {
        &self.locale
    }

    #[must_use]
    pub const fn entry(&self) -> &CharacterDisplayNameEntry {
        &self.entry
    }
}

impl CharacterDeclarationNameFallback {
    pub(super) const fn new(
        key: CharacterDisplayNameKey,
        value: CharacterDisplayNameValue,
    ) -> Self {
        Self { key, value }
    }

    #[must_use]
    pub const fn key(&self) -> &CharacterDisplayNameKey {
        &self.key
    }

    #[must_use]
    pub const fn value(&self) -> &CharacterDisplayNameValue {
        &self.value
    }
}

fn generate_key(
    character: &CharacterId,
    locale: Option<&CharacterNameLocale>,
    suffix: &'static str,
) -> Result<CharacterDisplayNameKey, CharacterDisplayNameKeyError> {
    let character_bytes = character.as_str().as_bytes();
    if character_bytes.len() > MAX_CHARACTER_ID_BYTES {
        return Err(CharacterDisplayNameKeyError::CharacterIdTooLong {
            bytes: character_bytes.len(),
            maximum: MAX_CHARACTER_ID_BYTES,
        });
    }

    let mut value = String::with_capacity(
        DISPLAY_NAME_KEY_PREFIX.len()
            + character_bytes.len().saturating_mul(2)
            + locale.map_or(0, |value| {
                value.locale_tag().as_str().len().saturating_mul(2)
            })
            + suffix.len()
            + 2,
    );
    value.push_str(DISPLAY_NAME_KEY_PREFIX);
    push_lower_hex(&mut value, character_bytes);
    value.push('.');
    if let Some(locale) = locale {
        value.push_str(suffix);
        value.push('.');
        push_lower_hex(&mut value, locale.locale_tag().as_str().as_bytes());
    } else {
        value.push_str(suffix);
    }

    if value.len() > MAX_GENERATED_DISPLAY_NAME_KEY_BYTES {
        return Err(CharacterDisplayNameKeyError::GeneratedKeyTooLong {
            bytes: value.len(),
            maximum: MAX_GENERATED_DISPLAY_NAME_KEY_BYTES,
        });
    }
    TextKey::try_new(value)
        .map(CharacterDisplayNameKey)
        .map_err(Into::into)
}

fn push_lower_hex(output: &mut String, bytes: &[u8]) {
    for byte in bytes {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
}
