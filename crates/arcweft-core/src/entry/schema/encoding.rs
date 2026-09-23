//! Canonical schema traversal over the shared bounded byte writer.

use std::slice;

use super::{
    CanonicalSink, CanonicalWriter, RuntimeBuiltinSchema, RuntimeEnumTagStyle,
    RuntimeNominalSchemaGraph, RuntimeSchemaError, RuntimeSchemaField, RuntimeSchemaValueField,
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
    Schemas(slice::Iter<'a, RuntimeTypeSchema>),
    ValueFields(slice::Iter<'a, RuntimeSchemaValueField>),
    BuiltinCases(&'a RuntimeBuiltinSchema, usize),
}

/// The work stack retains one iterator per active aggregate, independent of
/// its width. Nested source schemas never consume the native call stack.
pub(super) fn schema<S: CanonicalSink + ?Sized>(
    root: &RuntimeTypeSchema,
    writer: &mut CanonicalWriter<'_, S>,
    graph: Option<&RuntimeNominalSchemaGraph>,
) -> Result<(), RuntimeSchemaError> {
    let mut work = vec![Work::Schema(root)];
    while let Some(next) = work.pop() {
        match next {
            Work::BuiltinCases(builtin, ordinal) => {
                if let Some((case, payload)) = builtin.case(ordinal) {
                    writer.var_u32(
                        u32::try_from(ordinal)
                            .map_err(|_| RuntimeSchemaError::SchemaEncodingOverflow)?,
                    )?;
                    writer.string(case.name())?;
                    writer.u8(u8::from(case.has_payload()))?;
                    work.push(Work::BuiltinCases(builtin, ordinal + 1));
                    if let Some(payload) = payload {
                        work.push(Work::Schema(payload));
                    }
                }
            }
            Work::Schemas(mut schemas) => {
                if let Some(schema) = schemas.next() {
                    work.push(Work::Schemas(schemas));
                    work.push(Work::Schema(schema));
                }
            }
            Work::ValueFields(mut fields) => {
                if let Some(field) = fields.next() {
                    writer.var_u32(field.field().get().get())?;
                    writer.string(field.name())?;
                    work.push(Work::ValueFields(fields));
                    work.push(Work::Schema(field.schema()));
                }
            }
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
                RuntimeTypeSchema::Never => writer.u8(31)?,
                RuntimeTypeSchema::Duration => writer.u8(32)?,
                RuntimeTypeSchema::Progress => writer.u8(33)?,
                RuntimeTypeSchema::EntityReference => writer.u8(34)?,
                RuntimeTypeSchema::AgentValue => writer.u8(35)?,
                RuntimeTypeSchema::Choice(alternatives) => {
                    writer.u8(36)?;
                    writer.len(alternatives.len())?;
                    work.push(Work::Schemas(alternatives.iter()));
                }
                RuntimeTypeSchema::Bytes { format } => {
                    writer.u8(19)?;
                    writer.u8(format.tag())?;
                }
                RuntimeTypeSchema::Builtin(builtin) => {
                    writer.u8(20)?;
                    writer.u8(builtin.owner().semantic_tag())?;
                    writer.len(builtin.owner().cases().len())?;
                    work.push(Work::BuiltinCases(builtin, 0));
                }
                RuntimeTypeSchema::Seq(inner) => {
                    writer.u8(21)?;
                    work.push(Work::Schema(inner));
                }
                RuntimeTypeSchema::Array { item, length } => {
                    writer.u8(37)?;
                    writer.u64(*length)?;
                    work.push(Work::Schema(item));
                }
                RuntimeTypeSchema::Map { kind, key, value } => {
                    writer.u8(22)?;
                    writer.u8(kind.semantic_tag())?;
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
                RuntimeTypeSchema::Tuple(items) => {
                    writer.u8(26)?;
                    writer.len(items.len())?;
                    work.push(Work::Schemas(items.iter()));
                }
                RuntimeTypeSchema::RecordValue { fields } => {
                    writer.u8(28)?;
                    writer.len(fields.len())?;
                    work.push(Work::ValueFields(fields.iter()));
                }
                RuntimeTypeSchema::ExactOpaque { owner, arguments } => {
                    writer.u8(29)?;
                    writer.string(owner.producer().as_str())?;
                    writer.extend(owner.semantic_identity().as_bytes())?;
                    writer.u8(owner.admission().encoded())?;
                    writer.u8(owner.value_class().semantic_tag())?;
                    if let crate::value::RuntimeOpaqueValueClass::AffineHandle(kind) =
                        owner.value_class()
                    {
                        writer.u8(kind.encoded())?;
                    }
                    writer.u8(owner.persistence().semantic_tag())?;
                    writer.len(arguments.len())?;
                    work.push(Work::Schemas(arguments.iter()));
                }
                RuntimeTypeSchema::NominalRef(identity) => {
                    let graph = graph.ok_or_else(|| RuntimeSchemaError::NominalGraphRequired {
                        identity: identity.clone(),
                    })?;
                    if graph
                        .definition(identity.semantic_identity())
                        .is_none_or(|definition| definition.identity() != identity)
                    {
                        return Err(RuntimeSchemaError::UnresolvedNominal {
                            identity: identity.clone(),
                        });
                    }
                    writer.u8(30)?;
                    writer.string(identity.nominal().as_str())?;
                    writer.extend(identity.semantic_identity().as_bytes())?;
                }
            },
        }
    }
    Ok(())
}
