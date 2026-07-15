//! Typed failures shared by registration and runtime character decoding.

use arcweft_source::{SourceDocumentIdentity, SourceRange, SourceSpan};
use thiserror::Error;

use super::{
    CharacterManifestError,
    limits::CharacterManifestLimitKind,
    registration::{CharacterManifestTokenPath, JsonObjectPath},
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum JsonStructuralErrorKind {
    UnexpectedEnd,
    UnexpectedToken,
    InvalidEscape,
    InvalidUnicodeEscape,
    InvalidNumber,
    TrailingData,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterIdentifierDomain {
    Character,
    Part,
    Variant,
    Look,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterRegistrationDecodeError {
    #[error("character registration source exceeds the byte limit")]
    SourceBytesLimit { observed: u64, maximum: u64 },
    #[error("duplicate JSON key `{key}`")]
    DuplicateKey {
        object: JsonObjectPath,
        key: String,
        first: SourceSpan,
        duplicate: SourceSpan,
    },
    #[error("invalid character registration JSON structure")]
    Syntax {
        kind: JsonStructuralErrorKind,
        span: SourceSpan,
    },
    #[error("invalid character identifier `{value}`")]
    InvalidIdentifier {
        domain: CharacterIdentifierDomain,
        value: String,
        span: SourceSpan,
    },
    #[error("character manifest validation failed")]
    Validation {
        error: CharacterManifestError,
        span: SourceSpan,
    },
    #[error("required character provenance token is missing")]
    MissingToken {
        token: CharacterManifestTokenPath,
        document: SourceDocumentIdentity,
    },
    #[error("character registration hard limit exceeded")]
    Limit {
        kind: CharacterManifestLimitKind,
        observed: u64,
        maximum: u64,
        span: Option<SourceSpan>,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterRuntimeDecodeError {
    #[error("character runtime source exceeds the byte limit")]
    SourceBytesLimit { observed: u64, maximum: u64 },
    #[error("duplicate JSON key `{key}`")]
    DuplicateKey {
        object: JsonObjectPath,
        key: String,
        first: SourceRange,
        duplicate: SourceRange,
    },
    #[error("invalid character runtime JSON structure")]
    Syntax {
        kind: JsonStructuralErrorKind,
        range: SourceRange,
    },
    #[error("invalid character identifier `{value}`")]
    InvalidIdentifier {
        domain: CharacterIdentifierDomain,
        value: String,
        range: SourceRange,
    },
    #[error("character manifest validation failed")]
    Validation(CharacterManifestError),
    #[error("character runtime hard limit exceeded")]
    Limit {
        kind: CharacterManifestLimitKind,
        observed: u64,
        maximum: u64,
        range: Option<SourceRange>,
    },
}
