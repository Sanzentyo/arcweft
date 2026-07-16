use arcweft_source::{SourceDocumentId, SourceRevision};
use thiserror::Error;

use super::ProductSourceId;
use crate::resource_codec::SectionCodecError;

/// Candidate-construction failures for one complete product source map.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SourceMapBuildError {
    #[error("too many source documents: {actual} > {limit}")]
    TooManyDocuments { actual: usize, limit: usize },
    #[error("source document id is too long for the product boundary")]
    DocumentIdTooLong {
        id: SourceDocumentId,
        bytes: usize,
        limit: usize,
    },
    #[error("source display name is too long for the product boundary")]
    DisplayNameTooLong {
        id: SourceDocumentId,
        bytes: usize,
        limit: usize,
    },
    #[error("source document exceeds the per-document byte limit")]
    DocumentTooLarge {
        id: SourceDocumentId,
        bytes: u64,
        limit: u64,
    },
    #[error("source map total byte limit exceeded")]
    TotalBytesExceeded { actual: u64, limit: u64 },
    #[error("duplicate source document id {0}")]
    DuplicateDocument(SourceDocumentId),
    #[error("distinct document ids produced the same product source id")]
    ProductSourceIdCollision {
        product: ProductSourceId,
        first: SourceDocumentId,
        second: SourceDocumentId,
    },
    #[error("source-map size arithmetic overflow")]
    ArithmeticOverflow,
}

/// Malformed or non-canonical schema-v2 source-map section.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SourceMapCodecError {
    #[error(transparent)]
    Envelope(#[from] SectionCodecError),
    #[error(transparent)]
    Build(#[from] SourceMapBuildError),
    #[error("unsupported source-map schema {actual}; expected {expected}")]
    UnsupportedSchema { actual: u32, expected: u32 },
    #[error("source-map record count does not match its canonical transcript")]
    RecordCountMismatch,
    #[error("source-map document id is malformed: {0}")]
    InvalidDocumentId(String),
    #[error("source-map product source id is malformed: {0}")]
    InvalidProductSourceId(String),
    #[error("source-map display-name tag {0} is unsupported")]
    InvalidDisplayNameTag(u8),
    #[error("source-map display-name reference is malformed")]
    InvalidDisplayNameReference,
    #[error("source-map document payload is not valid UTF-8")]
    InvalidUtf8,
    #[error("source-map product identity does not match document {document}")]
    ProductSourceIdMismatch {
        document: SourceDocumentId,
        expected: ProductSourceId,
        actual: ProductSourceId,
    },
    #[error("source-map digest does not match exact UTF-8 for {id:?}")]
    RevisionMismatch {
        id: ProductSourceId,
        encoded: [u8; 32],
        actual: SourceRevision,
    },
    #[error("source-map extent does not match exact UTF-8 for {id:?}")]
    ExtentMismatch {
        id: ProductSourceId,
        declared: u64,
        payload: u64,
        actual: u64,
    },
    #[error("source-map set revision does not match its document inventory")]
    SourceSetRevisionMismatch,
    #[error("source-map transcript is not in its one canonical encoding")]
    NonCanonicalEncoding,
    #[error("source-map transcript arithmetic overflow")]
    ArithmeticOverflow,
}
