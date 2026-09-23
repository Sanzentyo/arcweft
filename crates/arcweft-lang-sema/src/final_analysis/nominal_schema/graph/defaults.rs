//! Declaration-owned default policies joined while the original typed fields
//! and their source schema occurrences are still available together.

use arcweft_core::entry::{
    RuntimeCodecUse as Use, RuntimeEnumTagStyle, RuntimeFieldCodecUse, RuntimeVariantCodecUse,
};

use super::*;
use crate::env::EnvironmentRecordField;

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
            ) => Some(Use::Tuple {
                items: fields
                    .iter()
                    .map(|field| {
                        Use::from_schema(field.schema(), RuntimeSchemaLimits::engine_default())
                    })
                    .collect::<Result<_, _>>()?,
            }),
            (
                AcceptedRustTypeMetadataKind::Newtype { .. },
                RuntimeNominalSchemaBody::Record { fields, .. },
            ) => Some(Use::Newtype {
                inner: Box::new(Use::from_schema(
                    fields[0].schema(),
                    RuntimeSchemaLimits::engine_default(),
                )?),
            }),
            (
                AcceptedRustTypeMetadataKind::Struct {
                    shape: AcceptedRustStructShape::Record(fields),
                },
                RuntimeNominalSchemaBody::Record {
                    fields: schemas, ..
                },
            ) => Some(self.rust_record_codec(
                metadata,
                &name,
                fields,
                schemas.iter().map(|field| field.schema()),
            )?),
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
                                Some(Use::Newtype {
                                    inner: Box::new(Use::from_schema(
                                        &schemas[0],
                                        RuntimeSchemaLimits::engine_default(),
                                    )?),
                                })
                            }
                            (
                                EnumVariantPayload::Record(fields),
                                Some(Schema::RecordValue { fields: schemas }),
                            ) => Some(self.rust_record_codec(
                                metadata,
                                &format!("{name}::{}", variant.name()),
                                fields,
                                schemas.iter().map(|field| field.schema()),
                            )?),
                            (_, Some(schema)) => Some(Use::from_schema(
                                schema,
                                RuntimeSchemaLimits::engine_default(),
                            )?),
                            (_, None) => None,
                        };
                        Ok(RuntimeVariantCodecUse {
                            wire_name: variant.wire_name().to_owned(),
                            discriminant: variant.discriminant(),
                            payload,
                        })
                    })
                    .collect::<Result<Box<[_]>, Error>>()?;
                Some(Use::Enum {
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
    ) -> Result<Use, Error> {
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
                Ok(RuntimeFieldCodecUse {
                    wire_name: field.wire_name().to_owned(),
                    has_default: field.data_default().is_some(),
                    default_program,
                    skip: field.skip(),
                    bytes_format: field.bytes_format().map(bytes_format),
                    value: Use::from_schema(schema, RuntimeSchemaLimits::engine_default())?,
                })
            })
            .collect::<Result<_, Error>>()?;
        // These are the standard Rust data record/tag semantics. The admitted
        // default edge, rather than a bool annotation, grants value generation.
        Ok(Use::Record {
            name: name.to_owned(),
            deny_unknown_fields: metadata.data_policy().deny_unknown_fields,
            fields,
        })
    }
}
