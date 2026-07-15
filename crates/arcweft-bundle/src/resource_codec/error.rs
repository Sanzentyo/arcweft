use super::field::{FieldId, ResourceWireType};
use super::kind::ProductSectionCodecKind;
use super::table::{PublicIdRef, StringId};
use thiserror::Error;

/// Compact resource section codec validation error.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SectionCodecError {
    #[error(transparent)]
    ViewExport(#[from] crate::resource_codec::view::ViewExportValidationError),
    #[error("section codec budget exceeded: {0}")]
    BudgetExceeded(&'static str),
    #[error("section codec magic {actual:?} does not match {expected:?}")]
    BadMagic { expected: [u8; 8], actual: [u8; 8] },
    #[error("unsupported section codec schema version {actual}; expected {expected}")]
    UnsupportedSchema { actual: u32, expected: u32 },
    #[error("unsupported section codec tag {0}")]
    UnsupportedCodecTag(u32),
    #[error("section codec expected {expected:?} but payload declares {actual:?}")]
    UnexpectedCodec {
        expected: ProductSectionCodecKind,
        actual: ProductSectionCodecKind,
    },
    #[error("section codec payload is truncated")]
    Truncated,
    #[error("section codec payload has trailing bytes")]
    TrailingBytes,
    #[error("section codec string index {0:?} is out of bounds")]
    StringOutOfBounds(StringId),
    #[error("section codec public id index {0:?} is out of bounds")]
    PublicIdOutOfBounds(PublicIdRef),
    #[error("section codec duplicate string `{0}`")]
    DuplicateString(String),
    #[error("section codec duplicate public id `{0}`")]
    DuplicatePublicId(String),
    #[error("section codec duplicate enum code {0}")]
    DuplicateEnumCode(u32),
    #[error("section codec enum symbol references missing string {0:?}")]
    EnumNameOutOfBounds(StringId),
    #[error("section codec table `{0}` is not canonical")]
    NonCanonicalTable(&'static str),
    #[error("section codec field id {0:?} is declared more than once in a registry")]
    DuplicateFieldSpec(FieldId),
    #[error("section codec unknown required field {0:?}")]
    UnknownRequiredField(FieldId),
    #[error("section codec required field {0:?} is missing")]
    MissingRequiredField(FieldId),
    #[error("section codec field {field:?} has wire type {actual:?}; expected {expected:?}")]
    FieldWireTypeMismatch {
        field: FieldId,
        expected: ResourceWireType,
        actual: ResourceWireType,
    },
    #[error("section codec invalid field flags {0:#04x}")]
    InvalidFieldFlags(u8),
    #[error("section codec unsupported wire type {0}")]
    UnsupportedWireType(u8),
    #[error("section codec invalid UTF-8 in {0}")]
    InvalidUtf8(&'static str),
    #[error("section codec integer length overflows target platform")]
    LengthOverflow,
}
