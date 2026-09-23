//! Lossless data reflection projection into the core schema algebra.
//!
//! Data owns format-neutral declarations; core owns their runtime schema and
//! canonical layout. Both semantic analysis and runtime adapters use this
//! context-free conversion. Named references remain references: this conversion
//! does not issue nominal identities or admit an incomplete schema graph.

use arcweft_data::{
    BytesFormat, EnumRepr, EnumTagStyle, FieldShape, MapKind, TypeShape, VariantShape,
};
use thiserror::Error;

use super::{
    RuntimeBytesFormat, RuntimeEnumRepr, RuntimeEnumTagStyle, RuntimeMapKind, RuntimeSchemaField,
    RuntimeSchemaVariant, RuntimeTypeSchema,
};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeDataSchemaProjectionError {
    #[error("data shape reference {id} requires a selected nominal graph identity")]
    UnboundReference { id: usize },
}

impl TryFrom<&TypeShape> for RuntimeTypeSchema {
    type Error = RuntimeDataSchemaProjectionError;

    fn try_from(shape: &TypeShape) -> Result<Self, Self::Error> {
        Ok(match shape {
            TypeShape::Unit => Self::Unit,
            TypeShape::Bool => Self::Bool,
            TypeShape::I8 => Self::I8,
            TypeShape::I16 => Self::I16,
            TypeShape::I32 => Self::I32,
            TypeShape::I64 => Self::I64,
            TypeShape::I128 => Self::I128,
            TypeShape::Isize => Self::ISize,
            TypeShape::U8 => Self::U8,
            TypeShape::U16 => Self::U16,
            TypeShape::U32 => Self::U32,
            TypeShape::U64 => Self::U64,
            TypeShape::U128 => Self::U128,
            TypeShape::Usize => Self::USize,
            TypeShape::F32 => Self::F32,
            TypeShape::F64 => Self::F64,
            TypeShape::String => Self::String,
            TypeShape::Char => Self::Char,
            TypeShape::Bytes { format } => Self::Bytes {
                format: (*format).into(),
            },
            TypeShape::Option(inner) => Self::option(Self::try_from(inner.as_ref())?),
            TypeShape::Seq(inner) => Self::Seq(Box::new(Self::try_from(inner.as_ref())?)),
            TypeShape::Tuple(items) => Self::Tuple(
                items
                    .iter()
                    .map(Self::try_from)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            ),
            TypeShape::Map { key, value, kind } => Self::Map {
                kind: (*kind).into(),
                key: Box::new(Self::try_from(key.as_ref())?),
                value: Box::new(Self::try_from(value.as_ref())?),
            },
            TypeShape::Record {
                name,
                fields,
                policy,
            } => Self::Record {
                name: name.clone(),
                fields: fields
                    .iter()
                    .map(RuntimeSchemaField::try_from)
                    .collect::<Result<Vec<_>, _>>()?,
                deny_unknown_fields: policy.deny_unknown_fields,
            },
            TypeShape::Enum {
                name,
                variants,
                tag,
                repr,
            } => Self::Enum {
                name: name.clone(),
                variants: variants
                    .iter()
                    .map(RuntimeSchemaVariant::try_from)
                    .collect::<Result<Vec<_>, _>>()?,
                tag: tag.into(),
                repr: repr.map(Into::into),
            },
            TypeShape::Ref(id) => {
                return Err(RuntimeDataSchemaProjectionError::UnboundReference { id: id.index() });
            }
        })
    }
}

impl TryFrom<&FieldShape> for RuntimeSchemaField {
    type Error = RuntimeDataSchemaProjectionError;

    fn try_from(field: &FieldShape) -> Result<Self, Self::Error> {
        Ok(Self {
            rust_name: field.rust_name.clone(),
            wire_name: field.wire_name.clone(),
            schema: RuntimeTypeSchema::try_from(&field.shape)?,
            has_default: field.has_default,
            skip: field.skip,
            bytes_format: field.bytes_format.map(Into::into),
        })
    }
}

impl TryFrom<&VariantShape> for RuntimeSchemaVariant {
    type Error = RuntimeDataSchemaProjectionError;

    fn try_from(variant: &VariantShape) -> Result<Self, Self::Error> {
        Ok(Self {
            rust_name: variant.rust_name.clone(),
            wire_name: variant.wire_name.clone(),
            payload: variant
                .payload
                .as_ref()
                .map(RuntimeTypeSchema::try_from)
                .transpose()?,
            discriminant: variant.discriminant,
        })
    }
}

impl From<MapKind> for RuntimeMapKind {
    fn from(kind: MapKind) -> Self {
        match kind {
            MapKind::Ordered => Self::Ordered,
            MapKind::Sorted => Self::Sorted,
            MapKind::BTree => Self::BTree,
        }
    }
}

impl From<BytesFormat> for RuntimeBytesFormat {
    fn from(format: BytesFormat) -> Self {
        match format {
            BytesFormat::Binary => Self::Binary,
            BytesFormat::Base64 => Self::Base64,
            BytesFormat::Hex => Self::Hex,
            BytesFormat::Array => Self::Array,
        }
    }
}

impl From<&EnumTagStyle> for RuntimeEnumTagStyle {
    fn from(tag: &EnumTagStyle) -> Self {
        match tag {
            EnumTagStyle::External => Self::External,
            EnumTagStyle::Internal { tag } => Self::Internal { tag: tag.clone() },
            EnumTagStyle::Adjacent { tag, content } => Self::Adjacent {
                tag: tag.clone(),
                content: content.clone(),
            },
        }
    }
}

impl From<EnumRepr> for RuntimeEnumRepr {
    fn from(repr: EnumRepr) -> Self {
        match repr {
            EnumRepr::I8 => Self::I8,
            EnumRepr::I16 => Self::I16,
            EnumRepr::I32 => Self::I32,
            EnumRepr::I64 => Self::I64,
            EnumRepr::I128 => Self::I128,
            EnumRepr::Isize => Self::ISize,
            EnumRepr::U8 => Self::U8,
            EnumRepr::U16 => Self::U16,
            EnumRepr::U32 => Self::U32,
            EnumRepr::U64 => Self::U64,
            EnumRepr::U128 => Self::U128,
            EnumRepr::Usize => Self::USize,
        }
    }
}

impl From<RuntimeBytesFormat> for BytesFormat {
    fn from(format: RuntimeBytesFormat) -> Self {
        match format {
            RuntimeBytesFormat::Binary => Self::Binary,
            RuntimeBytesFormat::Base64 => Self::Base64,
            RuntimeBytesFormat::Hex => Self::Hex,
            RuntimeBytesFormat::Array => Self::Array,
        }
    }
}

impl From<RuntimeMapKind> for MapKind {
    fn from(kind: RuntimeMapKind) -> Self {
        match kind {
            RuntimeMapKind::Ordered => Self::Ordered,
            RuntimeMapKind::Sorted => Self::Sorted,
            RuntimeMapKind::BTree => Self::BTree,
        }
    }
}

impl From<&RuntimeEnumTagStyle> for EnumTagStyle {
    fn from(tag: &RuntimeEnumTagStyle) -> Self {
        match tag {
            RuntimeEnumTagStyle::External => Self::External,
            RuntimeEnumTagStyle::Internal { tag } => Self::Internal { tag: tag.clone() },
            RuntimeEnumTagStyle::Adjacent { tag, content } => Self::Adjacent {
                tag: tag.clone(),
                content: content.clone(),
            },
        }
    }
}

impl From<RuntimeEnumRepr> for EnumRepr {
    fn from(repr: RuntimeEnumRepr) -> Self {
        match repr {
            RuntimeEnumRepr::I8 => Self::I8,
            RuntimeEnumRepr::I16 => Self::I16,
            RuntimeEnumRepr::I32 => Self::I32,
            RuntimeEnumRepr::I64 => Self::I64,
            RuntimeEnumRepr::I128 => Self::I128,
            RuntimeEnumRepr::ISize => Self::Isize,
            RuntimeEnumRepr::U8 => Self::U8,
            RuntimeEnumRepr::U16 => Self::U16,
            RuntimeEnumRepr::U32 => Self::U32,
            RuntimeEnumRepr::U64 => Self::U64,
            RuntimeEnumRepr::U128 => Self::U128,
            RuntimeEnumRepr::USize => Self::Usize,
        }
    }
}

impl From<crate::value::RuntimeSignedIntWidth> for TypeShape {
    fn from(width: crate::value::RuntimeSignedIntWidth) -> Self {
        use crate::value::RuntimeSignedIntWidth as Width;
        match width {
            Width::I8 => Self::I8,
            Width::I16 => Self::I16,
            Width::I32 => Self::I32,
            Width::I64 => Self::I64,
            Width::I128 => Self::I128,
            Width::ISize => Self::Isize,
        }
    }
}

impl From<crate::value::RuntimeUnsignedIntWidth> for TypeShape {
    fn from(width: crate::value::RuntimeUnsignedIntWidth) -> Self {
        use crate::value::RuntimeUnsignedIntWidth as Width;
        match width {
            Width::U8 => Self::U8,
            Width::U16 => Self::U16,
            Width::U32 => Self::U32,
            Width::U64 => Self::U64,
            Width::U128 => Self::U128,
            Width::USize => Self::Usize,
        }
    }
}

#[cfg(test)]
mod tests;
