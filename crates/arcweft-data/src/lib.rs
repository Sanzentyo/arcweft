#![forbid(unsafe_code)]
//! Format-independent data model, reflection metadata, and codec boundary.
//!
//! This crate is the builtin side of the serialization boundary. It does not
//! know about files, network, JSON, YAML, Arrow, or any other concrete format.

pub mod codec;
pub mod decode;
pub mod defaults;
pub mod encode;
pub mod error;
pub mod limits;
pub mod raw;
pub mod shape;
pub mod shape_graph;
pub mod value;

pub use codec::{
    Codec, CodecRegistry, DataFormat, DecodeOptions, EncodeOptions, FormatId, MediaType,
};
pub use decode::Decode;
pub use defaults::{DecodeShapeAccess, FieldDefaultProvider, FieldDefaultRequest};
pub use encode::Encode;
pub use error::{DataError, DataErrorKind, DataPath, PathSegment, Result};
pub use limits::{DecodeBudget, DecodeLimits};
pub use raw::{
    RawValue, decode_with_shape, decode_with_shape_ref, encode_with_shape, encode_with_shape_ref,
};
pub use shape::{
    BytesFormat, EnumRepr, EnumTagStyle, FieldShape, MapKind, RecordPolicy, RenameRule, TypeShape,
    VariantShape,
};
pub use shape_graph::{
    EmptyShapeAccess, ShapeAccess, ShapeGraph, ShapeGraphBuilder, ShapeId, ShapeRef,
};
pub use value::{Bytes, Number, Value};

/// Compile-time and syntax-derived shape information for Arcweft values.
pub trait Reflect {
    fn shape() -> TypeShape;

    /// Registers this concrete Rust type and its reflected children in `builder`.
    ///
    /// Implementations that contain other reflected types should reserve their
    /// own identity before registering children, then define the node after the
    /// children have been registered. This permits recursive type graphs.
    /// Graph registration requires a `'static` type because nodes are joined by
    /// concrete Rust type identity for the lifetime of this build.
    fn register_shape(builder: &mut ShapeGraphBuilder) -> Result<ShapeId>
    where
        Self: 'static + Sized,
    {
        let (id, is_new) = builder.reserve_type::<Self>();
        if is_new {
            builder.define(id, Self::shape())?;
        }
        Ok(id)
    }

    /// Builds the complete graph reachable from this concrete reflected type.
    ///
    /// This requires `Self: 'static`; [`Reflect::shape`] remains available for
    /// borrowed types that need only an inline shape tree.
    fn shape_graph() -> Result<(ShapeGraph, ShapeId)>
    where
        Self: 'static + Sized,
    {
        let mut builder = ShapeGraphBuilder::new();
        let root = Self::register_shape(&mut builder)?;
        Ok((builder.finish()?, root))
    }
}

#[cfg(feature = "derive")]
pub use arcweft_data_derive::{ArcweftDecode, ArcweftEncode, ArcweftReflect};
