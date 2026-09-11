//! Canonical schema traversal over the shared bounded byte writer.

use std::slice;

use super::{
    CanonicalSink, CanonicalWriter, RuntimeEnumTagStyle, RuntimeSchemaError, RuntimeSchemaField,
    RuntimeSchemaVariant, RuntimeTypeSchema,
};

#[cfg(test)]
mod tests;

enum Work<'a> {
    Schema(&'a RuntimeTypeSchema),
    Fields(slice::Iter<'a, RuntimeSchemaField>),
    FieldSuffix(&'a RuntimeSchemaField),
    Variants(slice::Iter<'a, RuntimeSchemaVariant>),
    VariantSuffix(&'a RuntimeSchemaVariant),
}

/// The work stack retains one iterator per active aggregate, independent of
/// its width. Nested source schemas never consume the native call stack.
pub(super) fn schema<S: CanonicalSink + ?Sized>(
    root: &RuntimeTypeSchema,
    writer: &mut CanonicalWriter<'_, S>,
) -> Result<(), RuntimeSchemaError> {
    let mut work = vec![Work::Schema(root)];
    while let Some(next) = work.pop() {
        match next {
            Work::Fields(mut fields) => {
                if let Some(field) = fields.next() {
                    writer.string(&field.rust_name)?;
                    writer.string(&field.wire_name)?;
                    work.push(Work::Fields(fields));
                    work.push(Work::FieldSuffix(field));
                    work.push(Work::Schema(&field.schema));
                }
            }
            Work::FieldSuffix(field) => {
                writer.u8(u8::from(field.has_default))?;
                writer.u8(u8::from(field.skip))?;
                writer.option(field.bytes_format.as_ref(), |writer, format| {
                    writer.u8(format.tag())
                })?;
            }
            Work::Variants(mut variants) => {
                if let Some(variant) = variants.next() {
                    writer.string(&variant.rust_name)?;
                    writer.string(&variant.wire_name)?;
                    writer.u8(u8::from(variant.payload.is_some()))?;
                    work.push(Work::Variants(variants));
                    work.push(Work::VariantSuffix(variant));
                    if let Some(payload) = &variant.payload {
                        work.push(Work::Schema(payload));
                    }
                }
            }
            Work::VariantSuffix(variant) => {
                writer.option(variant.discriminant.as_ref(), |writer, value| {
                    writer.i128(*value)
                })?;
            }
            Work::Schema(schema) => match schema {
                RuntimeTypeSchema::Unit => writer.u8(1)?,
                RuntimeTypeSchema::Bool => writer.u8(2)?,
                RuntimeTypeSchema::I8 => writer.u8(3)?,
                RuntimeTypeSchema::I16 => writer.u8(4)?,
                RuntimeTypeSchema::I32 => writer.u8(5)?,
                RuntimeTypeSchema::I64 => writer.u8(6)?,
                RuntimeTypeSchema::I128 => writer.u8(7)?,
                RuntimeTypeSchema::ISize => writer.u8(8)?,
                RuntimeTypeSchema::U8 => writer.u8(9)?,
                RuntimeTypeSchema::U16 => writer.u8(10)?,
                RuntimeTypeSchema::U32 => writer.u8(11)?,
                RuntimeTypeSchema::U64 => writer.u8(12)?,
                RuntimeTypeSchema::U128 => writer.u8(13)?,
                RuntimeTypeSchema::USize => writer.u8(14)?,
                RuntimeTypeSchema::F32 => writer.u8(15)?,
                RuntimeTypeSchema::F64 => writer.u8(16)?,
                RuntimeTypeSchema::String => writer.u8(17)?,
                RuntimeTypeSchema::Char => writer.u8(18)?,
                RuntimeTypeSchema::Bytes { format } => {
                    writer.u8(19)?;
                    writer.u8(format.tag())?;
                }
                RuntimeTypeSchema::Option(inner) => {
                    writer.u8(20)?;
                    work.push(Work::Schema(inner));
                }
                RuntimeTypeSchema::Seq(inner) => {
                    writer.u8(21)?;
                    work.push(Work::Schema(inner));
                }
                RuntimeTypeSchema::Map { key, value } => {
                    writer.u8(22)?;
                    work.push(Work::Schema(value));
                    work.push(Work::Schema(key));
                }
                RuntimeTypeSchema::Record {
                    name,
                    fields,
                    deny_unknown_fields,
                } => {
                    writer.u8(23)?;
                    writer.string(name)?;
                    writer.u8(u8::from(*deny_unknown_fields))?;
                    writer.len(fields.len())?;
                    work.push(Work::Fields(fields.iter()));
                }
                RuntimeTypeSchema::Enum {
                    name,
                    variants,
                    tag,
                    repr,
                } => {
                    writer.u8(24)?;
                    writer.string(name)?;
                    match tag {
                        RuntimeEnumTagStyle::External => writer.u8(1)?,
                        RuntimeEnumTagStyle::Internal { tag } => {
                            writer.u8(2)?;
                            writer.string(tag)?;
                        }
                        RuntimeEnumTagStyle::Adjacent { tag, content } => {
                            writer.u8(3)?;
                            writer.string(tag)?;
                            writer.string(content)?;
                        }
                    }
                    writer.option(repr.as_ref(), |writer, repr| writer.u8(repr.tag()))?;
                    writer.len(variants.len())?;
                    work.push(Work::Variants(variants.iter()));
                }
                RuntimeTypeSchema::Named(name) => {
                    writer.u8(25)?;
                    writer.string(name)?;
                }
            },
        }
    }
    Ok(())
}
