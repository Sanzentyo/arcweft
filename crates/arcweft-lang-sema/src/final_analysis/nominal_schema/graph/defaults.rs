//! Declaration-owned default policies joined while the original typed fields
//! and their source schema occurrences are still available together.

use arcweft_core::entry::{
    RuntimeCodecUse as Use, RuntimeEnumTagStyle, RuntimeFieldCodecUse, RuntimeVariantCodecUse,
};

use super::*;
use crate::env::EnvironmentRecordField;

/// Executable members have no standalone data codec. This is an absent
/// capability, not a failure of the original executable type/schema owner.
fn schema_codec(schema: &Schema) -> Result<Option<Use>, Error> {
    match Use::from_schema(schema, RuntimeSchemaLimits::engine_default()) {
        Ok(codec) => Ok(Some(codec)),
        Err(arcweft_core::entry::RuntimeCodecUseError::ExecutableType { .. }) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn schema_codecs<'a>(
    schemas: impl Iterator<Item = &'a Schema>,
) -> Result<Option<Box<[Use]>>, Error> {
    // Visit all members before deciding the complete capability. In record and
    // enum callers, later members may retain legitimate constructor defaults.
    Ok(schemas
        .map(schema_codec)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .collect())
}

fn tag_style(source: &arcweft_rust_abi::ArcweftRustEnumTagStyle) -> RuntimeEnumTagStyle {
    use arcweft_rust_abi::ArcweftRustEnumTagStyle as Tag;
    match source {
        Tag::External => RuntimeEnumTagStyle::External,
        Tag::Internal { tag } => RuntimeEnumTagStyle::Internal { tag: tag.clone() },
        Tag::Adjacent { tag, content } => RuntimeEnumTagStyle::Adjacent {
            tag: tag.clone(),
            content: content.clone(),
        },
    }
}

fn bytes_format(source: arcweft_rust_abi::ArcweftRustBytesFormat) -> RuntimeBytesFormat {
    use arcweft_rust_abi::ArcweftRustBytesFormat as Format;
    match source {
        Format::Binary => RuntimeBytesFormat::Binary,
        Format::Base64 => RuntimeBytesFormat::Base64,
        Format::Hex => RuntimeBytesFormat::Hex,
        Format::Array => RuntimeBytesFormat::Array,
    }
}

fn enum_repr(
    source: arcweft_rust_abi::ArcweftRustEnumRepr,
) -> arcweft_core::entry::RuntimeEnumRepr {
    use arcweft_core::entry::RuntimeEnumRepr as Target;
    use arcweft_rust_abi::ArcweftRustEnumRepr as Source;
    match source {
        Source::I8 => Target::I8,
        Source::I16 => Target::I16,
        Source::I32 => Target::I32,
        Source::I64 => Target::I64,
        Source::I128 => Target::I128,
        Source::Isize => Target::ISize,
        Source::U8 => Target::U8,
        Source::U16 => Target::U16,
        Source::U32 => Target::U32,
        Source::U64 => Target::U64,
        Source::U128 => Target::U128,
        Source::Usize => Target::USize,
    }
}

impl NominalGraphProjection<'_> {
    pub(super) fn rust_default_codec(
        &mut self,
        metadata: &InstantiatedRustTypeMetadata,
        body: &RuntimeNominalSchemaBody,
    ) -> Result<Option<Use>, Error> {
        let name = metadata.data_policy().name.clone();
        Ok(match (metadata.kind(), body) {
            (
                AcceptedRustTypeMetadataKind::Struct {
                    shape: AcceptedRustStructShape::Unit,
                },
                RuntimeNominalSchemaBody::Record { .. },
            ) => Some(Use::Plain),
            (
                AcceptedRustTypeMetadataKind::Struct {
                    shape: AcceptedRustStructShape::Tuple(_),
                },
                RuntimeNominalSchemaBody::Record { fields, .. },
            ) => schema_codecs(fields.iter().map(|field| field.schema()))?
                .map(|items| Use::Tuple { items }),
            (
                AcceptedRustTypeMetadataKind::Newtype { .. },
                RuntimeNominalSchemaBody::Record { fields, .. },
            ) => schema_codec(fields[0].schema())?.map(|inner| Use::Newtype {
                inner: Box::new(inner),
            }),
            (
                AcceptedRustTypeMetadataKind::Struct {
                    shape: AcceptedRustStructShape::Record(fields),
                },
                RuntimeNominalSchemaBody::Record {
                    fields: schemas, ..
                },
            ) => self.rust_record_codec(
                metadata,
                &name,
                fields,
                schemas.iter().map(|field| field.schema()),
            )?,
            (
                AcceptedRustTypeMetadataKind::Enum { variants },
                RuntimeNominalSchemaBody::Variant { cases },
            ) => {
                let cases = variants
                    .iter()
                    .zip(cases)
                    .map(|(variant, case)| {
                        let payload = match (variant.payload(), case.payload()) {
                            (EnumVariantPayload::Tuple(fields), Some(Schema::Tuple(schemas)))
                                if fields.len() == 1 && schemas.len() == 1 =>
                            {
                                schema_codec(&schemas[0])?.map(|inner| {
                                    Some(Use::Newtype {
                                        inner: Box::new(inner),
                                    })
                                })
                            }
                            (
                                EnumVariantPayload::Record(fields),
                                Some(Schema::RecordValue { fields: schemas }),
                            ) => self
                                .rust_record_codec(
                                    metadata,
                                    &format!("{name}::{}", variant.name()),
                                    fields,
                                    schemas.iter().map(|field| field.schema()),
                                )?
                                .map(Some),
                            (_, Some(schema)) => schema_codec(schema)?.map(Some),
                            (_, None) => Some(None),
                        };
                        Ok(payload.map(|payload| RuntimeVariantCodecUse {
                            wire_name: variant.wire_name().to_owned(),
                            discriminant: variant.discriminant(),
                            payload,
                        }))
                    })
                    .collect::<Result<Vec<_>, Error>>()?
                    .into_iter()
                    .collect::<Option<Box<[_]>>>();
                cases.map(|cases| Use::Enum {
                    name,
                    tag: tag_style(&metadata.data_policy().tag),
                    repr: metadata.data_policy().repr.map(enum_repr),
                    cases,
                })
            }
            _ => {
                return Err(Error::MetadataMismatch {
                    declaration: Box::new(metadata.id().clone()),
                });
            }
        })
    }

    fn rust_record_codec<'a>(
        &mut self,
        metadata: &InstantiatedRustTypeMetadata,
        name: &str,
        fields: &[EnvironmentRecordField],
        schemas: impl Iterator<Item = &'a Schema>,
    ) -> Result<Option<Use>, Error> {
        let fields = fields
            .iter()
            .zip(schemas)
            .map(|(field, schema)| {
                let default_program = field
                    .default_request()
                    .map(|source| {
                        let program = self
                            .environment
                            .ok_or(Error::StaleGeneration)?
                            .callable_catalog()
                            .rust_field_default(source, field.ty())
                            .map_err(|source| Error::FieldDefault {
                                declaration: Box::new(metadata.id().clone()),
                                field: format!("{name}.{}", field.name()),
                                source,
                            })?;
                        let id = program.program();
                        self.default_programs.insert(id, program);
                        Ok::<_, Error>(id)
                    })
                    .transpose()?;
                let value = schema_codec(schema)?;
                Ok(value.map(|value| RuntimeFieldCodecUse {
                    wire_name: field.wire_name().to_owned(),
                    has_default: field.data_default().is_some(),
                    default_program,
                    skip: field.skip(),
                    bytes_format: field.bytes_format().map(bytes_format),
                    value,
                }))
            })
            .collect::<Result<Vec<_>, Error>>()?
            .into_iter()
            .collect::<Option<Box<[_]>>>();
        // These are the standard Rust data record/tag semantics. The admitted
        // default edge, rather than a bool annotation, grants value generation.
        Ok(fields.map(|fields| Use::Record {
            name: name.to_owned(),
            deny_unknown_fields: metadata.data_policy().deny_unknown_fields,
            fields,
        }))
    }
}
