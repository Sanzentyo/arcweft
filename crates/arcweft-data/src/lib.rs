#![forbid(unsafe_code)]
//! Format-independent data model, reflection metadata, and codec boundary.
//!
//! This crate is the builtin side of the serialization boundary. It does not
//! know about files, network, JSON, YAML, Arrow, or any other concrete format.

pub mod codec;
pub mod decode;
pub mod encode;
pub mod error;
pub mod limits;
pub mod shape;
pub mod value;

pub use codec::{
    Codec, CodecRegistry, DataFormat, DecodeOptions, EncodeOptions, FormatId, MediaType,
};
pub use decode::Decode;
pub use encode::Encode;
pub use error::{DataError, DataErrorKind, DataPath, PathSegment, Result};
pub use limits::DecodeLimits;
pub use shape::{
    BytesFormat, EnumRepr, EnumTagStyle, FieldShape, RecordPolicy, RenameRule, TypeShape,
    VariantShape,
};
pub use value::{Bytes, Number, Value};

/// Compile-time and syntax-derived shape information for Arcweft values.
pub trait Reflect {
    fn shape() -> TypeShape;
}

#[cfg(feature = "derive")]
pub use arcweft_data_derive::{ArcweftDecode, ArcweftEncode, ArcweftReflect};
