//! Typed failures for the closed Character presentation `LocaleCatalog` family.

use crate::resource_codec::{FieldId, SectionCodecError};
use arcweft_character::{
    id::CharacterIdError,
    presentation_name::{
        CharacterDisplayNameKeyError, CharacterDisplayNameValueError,
        CharacterNameLocalePolicyError, CharacterPresentationCatalogError,
    },
};
use arcweft_id::LocaleTagError;
use thiserror::Error;

/// Rejection produced by the sole canonical Character `LocaleCatalog` codec.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterPresentationCatalogCodecError {
    #[error(transparent)]
    Envelope(#[from] SectionCodecError),
    #[error(transparent)]
    Catalog(#[from] CharacterPresentationCatalogError),
    #[error(transparent)]
    LocalePolicy(#[from] CharacterNameLocalePolicyError),
    #[error("LocaleCatalog arithmetic overflow while computing {operation}")]
    ArithmeticOverflow { operation: &'static str },
    #[error("LocaleCatalog field {field:?} has {actual} bytes; expected {expected}")]
    FieldLength {
        field: FieldId,
        expected: usize,
        actual: usize,
    },
    #[error("LocaleCatalog {name} is {actual}; expected {expected}")]
    HeaderValue {
        name: &'static str,
        expected: u32,
        actual: u32,
    },
    #[error("LocaleCatalog {name} count {actual} exceeds maximum {maximum}")]
    Limit {
        name: &'static str,
        maximum: u32,
        actual: u32,
    },
    #[error("LocaleCatalog field {field:?} offset {offset} has nonzero reserved bytes")]
    NonzeroReserved { field: FieldId, offset: u32 },
    #[error("LocaleCatalog field {field:?} offset {offset} has unsupported {kind} tag {actual}")]
    UnsupportedTag {
        field: FieldId,
        offset: u32,
        kind: &'static str,
        actual: u8,
    },
    #[error("LocaleCatalog field {field:?} offset {offset} has an invalid sentinel combination")]
    InvalidSentinel { field: FieldId, offset: u32 },
    #[error("LocaleCatalog Character id `{value}` is invalid")]
    InvalidCharacterId {
        value: String,
        source: CharacterIdError,
    },
    #[error("LocaleCatalog locale `{value}` is invalid")]
    InvalidLocale {
        value: String,
        source: LocaleTagError,
    },
    #[error("LocaleCatalog display name is invalid")]
    InvalidDisplayName {
        source: CharacterDisplayNameValueError,
    },
    #[error("LocaleCatalog generated display-name key is invalid")]
    InvalidGeneratedKey {
        source: CharacterDisplayNameKeyError,
    },
    #[error(
        "LocaleCatalog generated key mismatch for `{character}`: expected `{expected}`, found `{actual}`"
    )]
    GeneratedKeyMismatch {
        character: String,
        expected: String,
        actual: String,
    },
    #[error("LocaleCatalog {table} records are not in strict canonical order")]
    NonCanonicalRecordOrder { table: &'static str },
    #[error("LocaleCatalog Character record spans do not cover the localized table exactly")]
    InvalidLocalizedSpan,
    #[error("LocaleCatalog references unknown {table} index {index}")]
    ReferenceOutOfBounds { table: &'static str, index: u32 },
    #[error("LocaleCatalog canonical {table} table is missing `{value}`")]
    MissingCanonicalEntry { table: &'static str, value: String },
    #[error("LocaleCatalog contains {count} unknown optional fields")]
    UnknownOptionalFields { count: usize },
    #[error("LocaleCatalog String table entry {index} is not referenced")]
    UnreferencedString { index: u32 },
    #[error(
        "LocaleCatalog PublicId table entry {index} is referenced {actual} times; expected once"
    )]
    PublicIdReferenceCount { index: u32, actual: u32 },
    #[error("LocaleCatalog semantic digest does not match its canonical records")]
    SemanticDigestMismatch,
    #[error("LocaleCatalog locale-policy digest does not match its canonical policy")]
    LocalePolicyDigestMismatch,
}
