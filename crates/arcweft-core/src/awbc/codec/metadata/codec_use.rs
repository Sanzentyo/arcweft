use super::super::AwbcCodecError;
use super::super::wire::{Reader, Wire, Writer};
use crate::entry::{
    RuntimeBytesFormat, RuntimeCodecUse, RuntimeEnumRepr, RuntimeEnumTagStyle,
    RuntimeFieldCodecUse, RuntimeVariantCodecUse,
};

impl Wire for RuntimeCodecUse {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Plain => writer.write_u8(0),
            Self::Bytes { format } => {
                writer.write_u8(1);
                format.write_wire(writer)?;
            }
            Self::Unary { item } => {
                writer.write_u8(2);
                item.write_wire(writer)?;
            }
            Self::Tuple { items } => {
                writer.write_u8(3);
                writer.write_table(items)?;
            }
            Self::Map { key, value } => {
                writer.write_u8(4);
                key.write_wire(writer)?;
                value.write_wire(writer)?;
            }
            Self::RecordFields { fields } => {
                writer.write_u8(5);
                writer.write_table(fields)?;
            }
            Self::Record {
                name,
                deny_unknown_fields,
                fields,
            } => {
                writer.write_u8(6);
                name.write_wire(writer)?;
                deny_unknown_fields.write_wire(writer)?;
                writer.write_table(fields)?;
            }
            Self::Enum {
                name,
                tag,
                repr,
                cases,
            } => {
                writer.write_u8(7);
                name.write_wire(writer)?;
                tag.write_wire(writer)?;
                repr.write_wire(writer)?;
                writer.write_table(cases)?;
            }
            Self::Builtin { payloads } => {
                writer.write_u8(8);
                writer.write_table(payloads)?;
            }
            Self::Choice { alternatives } => {
                writer.write_u8(9);
                writer.write_table(alternatives)?;
            }
            Self::Opaque { arguments } => {
                writer.write_u8(10);
                writer.write_table(arguments)?;
            }
            Self::NominalRef => writer.write_u8(11),
            Self::Newtype { inner } => {
                writer.write_u8(12);
                inner.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        reader.read_nested(|reader| {
            let offset = reader.offset();
            Ok(match reader.read_u8()? {
                0 => Self::Plain,
                1 => Self::Bytes {
                    format: RuntimeBytesFormat::read_wire(reader)?,
                },
                2 => Self::Unary {
                    item: Box::new(Self::read_wire(reader)?),
                },
                3 => Self::Tuple {
                    items: Vec::<Self>::read_wire(reader)?.into_boxed_slice(),
                },
                4 => Self::Map {
                    key: Box::new(Self::read_wire(reader)?),
                    value: Box::new(Self::read_wire(reader)?),
                },
                5 => Self::RecordFields {
                    fields: Vec::<Self>::read_wire(reader)?.into_boxed_slice(),
                },
                6 => Self::Record {
                    name: String::read_wire(reader)?,
                    deny_unknown_fields: bool::read_wire(reader)?,
                    fields: Vec::<RuntimeFieldCodecUse>::read_wire(reader)?.into_boxed_slice(),
                },
                7 => Self::Enum {
                    name: String::read_wire(reader)?,
                    tag: RuntimeEnumTagStyle::read_wire(reader)?,
                    repr: Option::<RuntimeEnumRepr>::read_wire(reader)?,
                    cases: Vec::<RuntimeVariantCodecUse>::read_wire(reader)?.into_boxed_slice(),
                },
                8 => Self::Builtin {
                    payloads: Vec::<Self>::read_wire(reader)?.into_boxed_slice(),
                },
                9 => Self::Choice {
                    alternatives: Vec::<Self>::read_wire(reader)?.into_boxed_slice(),
                },
                10 => Self::Opaque {
                    arguments: Vec::<Self>::read_wire(reader)?.into_boxed_slice(),
                },
                11 => Self::NominalRef,
                12 => Self::Newtype {
                    inner: Box::new(Self::read_wire(reader)?),
                },
                tag => {
                    return Err(AwbcCodecError::UnknownTag {
                        kind: "runtime codec use",
                        tag,
                        offset,
                    });
                }
            })
        })
    }
}

impl Wire for RuntimeFieldCodecUse {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.wire_name.write_wire(writer)?;
        self.has_default.write_wire(writer)?;
        self.default_program.write_wire(writer)?;
        self.skip.write_wire(writer)?;
        self.bytes_format.write_wire(writer)?;
        self.value.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            wire_name: String::read_wire(reader)?,
            has_default: bool::read_wire(reader)?,
            default_program: Option::read_wire(reader)?,
            skip: bool::read_wire(reader)?,
            bytes_format: Option::<RuntimeBytesFormat>::read_wire(reader)?,
            value: RuntimeCodecUse::read_wire(reader)?,
        })
    }
}

impl Wire for RuntimeVariantCodecUse {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.wire_name.write_wire(writer)?;
        match self.discriminant {
            None => writer.write_u8(0),
            Some(discriminant) => {
                writer.write_u8(1);
                writer.write_bytes(&discriminant.to_le_bytes());
            }
        }
        self.payload.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let wire_name = String::read_wire(reader)?;
        let offset = reader.offset();
        let discriminant = match reader.read_u8()? {
            0 => None,
            1 => Some(i128::from_le_bytes(
                reader
                    .read_exact(16)?
                    .try_into()
                    .expect("fixed wire width checked"),
            )),
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "runtime codec discriminant",
                    tag,
                    offset,
                });
            }
        };
        Ok(Self {
            wire_name,
            discriminant,
            payload: Option::<RuntimeCodecUse>::read_wire(reader)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::awbc::codec::AwbcDecodeBudget;

    fn complete_policy() -> RuntimeCodecUse {
        RuntimeCodecUse::Choice {
            alternatives: vec![
                RuntimeCodecUse::Plain,
                RuntimeCodecUse::Bytes {
                    format: RuntimeBytesFormat::Hex,
                },
                RuntimeCodecUse::Unary {
                    item: Box::new(RuntimeCodecUse::NominalRef),
                },
                RuntimeCodecUse::Tuple {
                    items: vec![RuntimeCodecUse::Builtin {
                        payloads: vec![RuntimeCodecUse::Plain].into_boxed_slice(),
                    }]
                    .into_boxed_slice(),
                },
                RuntimeCodecUse::Map {
                    key: Box::new(RuntimeCodecUse::Plain),
                    value: Box::new(RuntimeCodecUse::Bytes {
                        format: RuntimeBytesFormat::Base64,
                    }),
                },
                RuntimeCodecUse::RecordFields {
                    fields: vec![RuntimeCodecUse::Plain].into_boxed_slice(),
                },
                RuntimeCodecUse::Record {
                    name: "Row".to_owned(),
                    deny_unknown_fields: true,
                    fields: vec![RuntimeFieldCodecUse {
                        wire_name: "bytes".to_owned(),
                        has_default: false,
                        default_program: None,
                        skip: false,
                        bytes_format: Some(RuntimeBytesFormat::Array),
                        value: RuntimeCodecUse::Bytes {
                            format: RuntimeBytesFormat::Binary,
                        },
                    }]
                    .into_boxed_slice(),
                },
                RuntimeCodecUse::Enum {
                    name: "Tag".to_owned(),
                    tag: RuntimeEnumTagStyle::Adjacent {
                        tag: "type".to_owned(),
                        content: "value".to_owned(),
                    },
                    repr: Some(RuntimeEnumRepr::U16),
                    cases: vec![RuntimeVariantCodecUse {
                        wire_name: "Value".to_owned(),
                        discriminant: Some(-7),
                        payload: Some(RuntimeCodecUse::Plain),
                    }]
                    .into_boxed_slice(),
                },
                RuntimeCodecUse::Opaque {
                    arguments: vec![RuntimeCodecUse::Plain].into_boxed_slice(),
                },
                RuntimeCodecUse::Newtype {
                    inner: Box::new(RuntimeCodecUse::Bytes {
                        format: RuntimeBytesFormat::Array,
                    }),
                },
            ]
            .into_boxed_slice(),
        }
    }

    #[test]
    fn runtime_codec_use_wire_roundtrips_every_v1_variant() {
        let policy = complete_policy();
        let mut writer = Writer::default();
        policy.write_wire(&mut writer).expect("encode codec use");
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert_eq!(
            RuntimeCodecUse::read_wire(&mut reader).expect("decode codec use"),
            policy
        );
        reader.finish().expect("consume codec use");
    }

    #[test]
    fn runtime_codec_use_wire_rejects_unknown_tags_and_excessive_nesting() {
        let mut reader = Reader::new(&[13], &AwbcDecodeBudget::default());
        assert_eq!(
            RuntimeCodecUse::read_wire(&mut reader).expect_err("unknown codec-use tag must reject"),
            AwbcCodecError::UnknownTag {
                kind: "runtime codec use",
                tag: 13,
                offset: 0,
            }
        );

        let mut writer = Writer::default();
        let mut deeply_nested = RuntimeCodecUse::Plain;
        for _ in 0..32 {
            deeply_nested = RuntimeCodecUse::Unary {
                item: Box::new(deeply_nested),
            };
        }
        deeply_nested
            .write_wire(&mut writer)
            .expect("encode nested policy");
        let bytes = writer.into_bytes();
        let budget = AwbcDecodeBudget {
            nesting_depth: 8,
            ..AwbcDecodeBudget::default()
        };
        assert_eq!(
            RuntimeCodecUse::read_wire(&mut Reader::new(&bytes, &budget))
                .expect_err("nesting budget protects recursive policy decode"),
            AwbcCodecError::NestingDepthExceeded { limit: 8 }
        );
    }
}
