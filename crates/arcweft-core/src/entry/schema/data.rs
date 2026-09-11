//! Lossless data reflection projection into the core schema algebra.
//!
//! Data owns format-neutral declarations; core owns their runtime schema and
//! canonical layout. Both semantic analysis and runtime adapters use this
//! context-free conversion. Named references remain references: this conversion
//! does not issue nominal identities or admit an incomplete schema graph.

use arcweft_data::{BytesFormat, EnumRepr, EnumTagStyle, FieldShape, TypeShape, VariantShape};

use super::{
    RuntimeBytesFormat, RuntimeEnumRepr, RuntimeEnumTagStyle, RuntimeSchemaField,
    RuntimeSchemaVariant, RuntimeTypeSchema,
};

impl From<&TypeShape> for RuntimeTypeSchema {
    fn from(shape: &TypeShape) -> Self {
        match shape {
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
            TypeShape::Option(inner) => Self::Option(Box::new(Self::from(inner.as_ref()))),
            TypeShape::Seq(inner) => Self::Seq(Box::new(Self::from(inner.as_ref()))),
            TypeShape::Map { key, value } => Self::Map {
                key: Box::new(Self::from(key.as_ref())),
                value: Box::new(Self::from(value.as_ref())),
            },
            TypeShape::Record {
                name,
                fields,
                policy,
            } => Self::Record {
                name: name.clone(),
                fields: fields.iter().map(RuntimeSchemaField::from).collect(),
                deny_unknown_fields: policy.deny_unknown_fields,
            },
            TypeShape::Enum {
                name,
                variants,
                tag,
                repr,
            } => Self::Enum {
                name: name.clone(),
                variants: variants.iter().map(RuntimeSchemaVariant::from).collect(),
                tag: tag.into(),
                repr: repr.map(Into::into),
            },
            TypeShape::Named(name) => Self::Named(name.clone()),
        }
    }
}

impl From<&FieldShape> for RuntimeSchemaField {
    fn from(field: &FieldShape) -> Self {
        Self {
            rust_name: field.rust_name.clone(),
            wire_name: field.wire_name.clone(),
            schema: RuntimeTypeSchema::from(&field.shape),
            has_default: field.has_default,
            skip: field.skip,
            bytes_format: field.bytes_format.map(Into::into),
        }
    }
}

impl From<&VariantShape> for RuntimeSchemaVariant {
    fn from(variant: &VariantShape) -> Self {
        Self {
            rust_name: variant.rust_name.clone(),
            wire_name: variant.wire_name.clone(),
            payload: variant.payload.as_ref().map(RuntimeTypeSchema::from),
            discriminant: variant.discriminant,
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

#[cfg(test)]
mod tests;
