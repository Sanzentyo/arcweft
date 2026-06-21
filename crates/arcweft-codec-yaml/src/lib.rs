#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;

use arcweft_data::{
    BytesFormat, Codec, DataError, DataErrorKind, DecodeOptions, EncodeOptions, FieldShape,
    FormatId, Number, RawValue, Result, TypeShape, Value, decode_with_shape, encode_with_shape,
};
use base64::prelude::{BASE64_STANDARD, Engine as _};
use yaml_rust2::yaml::Hash;
use yaml_rust2::{Yaml, YamlEmitter, YamlLoader};

#[derive(Clone, Copy, Debug, Default)]
pub struct YamlCodec;

impl Codec for YamlCodec {
    fn id(&self) -> FormatId {
        FormatId::new("yaml")
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["application/yaml", "application/x-yaml", "text/yaml"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["yaml", "yml"]
    }

    fn encode_value(
        &self,
        value: &Value,
        shape: &TypeShape,
        _options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let raw = encode_with_shape(value, shape)?;
        let yaml = raw_to_yaml(&raw, shape)?;
        let mut out = String::new();
        YamlEmitter::new(&mut out)
            .dump(&yaml)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        Ok(out.into_bytes())
    }

    fn decode_value(
        &self,
        input: &[u8],
        shape: &TypeShape,
        options: &DecodeOptions,
    ) -> Result<Value> {
        if input.len() > options.limits.max_input_len {
            return Err(DataError::limit(format!(
                "input length {} exceeds {}",
                input.len(),
                options.limits.max_input_len
            )));
        }
        let source = std::str::from_utf8(input)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        let documents = YamlLoader::load_from_str(source)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        let [document] = documents.as_slice() else {
            return Err(match documents.len() {
                0 => DataError::new(DataErrorKind::MissingField, "YAML document is empty"),
                _ => DataError::new(
                    DataErrorKind::TrailingData,
                    "YAML codec accepts exactly one document",
                ),
            });
        };
        let raw = yaml_to_raw(document, shape)?;
        let value = decode_with_shape(&raw, shape)?;
        options.limits.validate(&value)?;
        Ok(value)
    }
}

fn raw_to_yaml(raw: &RawValue, shape: &TypeShape) -> Result<Yaml> {
    match (shape, raw) {
        (TypeShape::Bool, RawValue::Bool(value)) => Ok(Yaml::Boolean(*value)),
        (
            TypeShape::I8
            | TypeShape::I16
            | TypeShape::I32
            | TypeShape::I64
            | TypeShape::I128
            | TypeShape::Isize,
            RawValue::Signed(value),
        ) => signed_to_yaml(*value),
        (
            TypeShape::U8
            | TypeShape::U16
            | TypeShape::U32
            | TypeShape::U64
            | TypeShape::U128
            | TypeShape::Usize,
            RawValue::Unsigned(value),
        ) => unsigned_to_yaml(*value),
        (TypeShape::F32, RawValue::F32(value)) => Ok(Yaml::Real(value.to_string())),
        (TypeShape::F64, RawValue::F64(value)) => Ok(Yaml::Real(value.to_string())),
        (TypeShape::String | TypeShape::Char, RawValue::String(value)) => {
            Ok(Yaml::String(value.clone()))
        }
        (TypeShape::Bytes { format }, RawValue::Bytes(bytes)) => bytes_to_yaml(bytes, *format),
        (TypeShape::Unit | TypeShape::Option(_), RawValue::Null) => Ok(Yaml::Null),
        (TypeShape::Option(inner), raw) => raw_to_yaml(raw, inner),
        (TypeShape::Seq(inner), RawValue::Seq(values)) => raw_seq_to_yaml(values, inner),
        (TypeShape::Map { key, value }, RawValue::Map(entries))
            if matches!(key.as_ref(), TypeShape::String) =>
        {
            raw_string_map_to_yaml(entries, value)
        }
        (TypeShape::Record { fields, .. }, RawValue::Map(entries)) => {
            raw_record_to_yaml(entries, fields)
        }
        (TypeShape::Enum { .. }, raw) => raw_dynamic_to_yaml(raw),
        (TypeShape::Named(_), _) => Err(DataError::unsupported(
            "named shape must be resolved before YAML encoding",
        )),
        _ => Err(DataError::invalid_type(shape.type_name(), raw.type_name())),
    }
}

fn signed_to_yaml(value: i128) -> Result<Yaml> {
    i64::try_from(value)
        .map(Yaml::Integer)
        .map_err(|_| yaml_integer_range_error())
}

fn unsigned_to_yaml(value: u128) -> Result<Yaml> {
    i64::try_from(value)
        .map(Yaml::Integer)
        .map_err(|_| yaml_integer_range_error())
}

fn raw_seq_to_yaml(values: &[RawValue], shape: &TypeShape) -> Result<Yaml> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| raw_to_yaml(value, shape).map_err(|error| error.at_index(index)))
        .collect::<Result<Vec<_>>>()
        .map(Yaml::Array)
}

fn raw_string_map_to_yaml(entries: &[(RawValue, RawValue)], shape: &TypeShape) -> Result<Yaml> {
    entries
        .iter()
        .map(|(key, raw_value)| {
            let RawValue::String(key) = key else {
                return Err(DataError::invalid_type("string map key", key.type_name()));
            };
            raw_to_yaml(raw_value, shape)
                .map(|yaml| (Yaml::String(key.clone()), yaml))
                .map_err(|error| error.at_field(key.clone()))
        })
        .collect::<Result<Hash>>()
        .map(Yaml::Hash)
}

fn raw_record_to_yaml(entries: &[(RawValue, RawValue)], fields: &[FieldShape]) -> Result<Yaml> {
    entries
        .iter()
        .map(|(key, raw_value)| {
            let RawValue::String(key) = key else {
                return Err(DataError::invalid_type("record field key", key.type_name()));
            };
            let shape = fields
                .iter()
                .find(|field| field.wire_name == *key)
                .map_or(TypeShape::Unit, FieldShape::value_shape);
            raw_to_yaml(raw_value, &shape)
                .map(|yaml| (Yaml::String(key.clone()), yaml))
                .map_err(|error| error.at_field(key.clone()))
        })
        .collect::<Result<Hash>>()
        .map(Yaml::Hash)
}

fn raw_map_to_yaml(entries: &[(RawValue, RawValue)]) -> Result<Yaml> {
    entries
        .iter()
        .map(|(key, value)| {
            let RawValue::String(key) = key else {
                return Err(DataError::invalid_type("hash key", key.type_name()));
            };
            raw_dynamic_to_yaml(value).map(|yaml| (Yaml::String(key.clone()), yaml))
        })
        .collect::<Result<Hash>>()
        .map(Yaml::Hash)
}

fn yaml_to_raw(value: &Yaml, shape: &TypeShape) -> Result<RawValue> {
    match shape {
        TypeShape::Unit => match value {
            Yaml::Null => Ok(RawValue::Null),
            other => Err(DataError::invalid_type("null", yaml_type_name(other))),
        },
        TypeShape::Bool => match value {
            Yaml::Boolean(value) => Ok(RawValue::Bool(*value)),
            other => Err(DataError::invalid_type("bool", yaml_type_name(other))),
        },
        TypeShape::String | TypeShape::Char => match value {
            Yaml::String(value) => Ok(RawValue::String(value.clone())),
            other => Err(DataError::invalid_type("string", yaml_type_name(other))),
        },
        TypeShape::Bytes { format } => yaml_to_bytes(value, *format).map(RawValue::Bytes),
        TypeShape::Option(inner) => match value {
            Yaml::Null => Ok(RawValue::Null),
            other => yaml_to_raw(other, inner),
        },
        TypeShape::Seq(inner) => match value {
            Yaml::Array(values) => values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    yaml_to_raw(value, inner).map_err(|error| error.at_index(index))
                })
                .collect::<Result<Vec<_>>>()
                .map(RawValue::Seq),
            other => Err(DataError::invalid_type("array", yaml_type_name(other))),
        },
        TypeShape::Map { key, value: inner } if matches!(key.as_ref(), TypeShape::String) => {
            yaml_hash_entries(value, inner).map(RawValue::Map)
        }
        TypeShape::Record { fields, .. } => match value {
            Yaml::Hash(entries) => entries
                .iter()
                .map(|(key, value)| {
                    let Some(key) = key.as_str() else {
                        return Err(DataError::invalid_type(
                            "record field key",
                            yaml_type_name(key),
                        ));
                    };
                    let raw = match fields.iter().find(|field| field.wire_name == key) {
                        Some(field) => yaml_to_raw(value, &field.value_shape()),
                        None => yaml_dynamic_to_raw(value),
                    }?;
                    Ok((RawValue::String(key.to_owned()), raw))
                })
                .collect::<Result<Vec<_>>>()
                .map(RawValue::Map),
            other => Err(DataError::invalid_type("hash", yaml_type_name(other))),
        },
        TypeShape::Enum { .. } => yaml_dynamic_to_raw(value),
        TypeShape::Named(_) => Err(DataError::unsupported(
            "named shape must be resolved before YAML decoding",
        )),
        TypeShape::F32 | TypeShape::F64 => yaml_float_to_raw(value, shape),
        TypeShape::I8
        | TypeShape::I16
        | TypeShape::I32
        | TypeShape::I64
        | TypeShape::I128
        | TypeShape::Isize
        | TypeShape::U8
        | TypeShape::U16
        | TypeShape::U32
        | TypeShape::U64
        | TypeShape::U128
        | TypeShape::Usize => yaml_integer_to_raw(value, shape),
        TypeShape::Map { .. } => Err(DataError::unsupported(
            "YAML shape codec supports string map keys only",
        )),
    }
}

fn yaml_hash_entries(value: &Yaml, shape: &TypeShape) -> Result<Vec<(RawValue, RawValue)>> {
    match value {
        Yaml::Hash(entries) => entries
            .iter()
            .map(|(key, value)| {
                let Some(key) = key.as_str() else {
                    return Err(DataError::invalid_type(
                        "string map key",
                        yaml_type_name(key),
                    ));
                };
                yaml_to_raw(value, shape)
                    .map(|raw| (RawValue::String(key.to_owned()), raw))
                    .map_err(|error| error.at_field(key))
            })
            .collect(),
        other => Err(DataError::invalid_type("hash", yaml_type_name(other))),
    }
}

fn yaml_integer_to_raw(value: &Yaml, shape: &TypeShape) -> Result<RawValue> {
    let Yaml::Integer(value) = value else {
        return Err(DataError::invalid_type("integer", yaml_type_name(value)));
    };
    if shape.unsigned_max().is_some() {
        return u128::try_from(*value)
            .map(RawValue::Unsigned)
            .map_err(|_| DataError::invalid_type(shape.type_name(), "negative integer"));
    }
    Ok(RawValue::Signed(i128::from(*value)))
}

fn yaml_float_to_raw(value: &Yaml, shape: &TypeShape) -> Result<RawValue> {
    let text = match value {
        Yaml::Real(value) => value.as_str(),
        Yaml::Integer(value) => return integer_to_float_raw(*value, shape),
        other => return Err(DataError::invalid_type("float", yaml_type_name(other))),
    };
    match shape {
        TypeShape::F32 => text
            .parse::<f32>()
            .map(RawValue::F32)
            .map_err(|error| DataError::new(DataErrorKind::NumberOutOfRange, error.to_string())),
        TypeShape::F64 => text
            .parse::<f64>()
            .map(RawValue::F64)
            .map_err(|error| DataError::new(DataErrorKind::NumberOutOfRange, error.to_string())),
        _ => unreachable!("caller passes float shape"),
    }
}

fn integer_to_float_raw(value: i64, shape: &TypeShape) -> Result<RawValue> {
    let text = value.to_string();
    match shape {
        TypeShape::F32 => text
            .parse::<f32>()
            .map(RawValue::F32)
            .map_err(|error| DataError::new(DataErrorKind::NumberOutOfRange, error.to_string())),
        TypeShape::F64 => text
            .parse::<f64>()
            .map(RawValue::F64)
            .map_err(|error| DataError::new(DataErrorKind::NumberOutOfRange, error.to_string())),
        _ => unreachable!("caller passes float shape"),
    }
}

fn yaml_dynamic_to_raw(value: &Yaml) -> Result<RawValue> {
    match value {
        Yaml::Real(value) => value
            .parse::<f64>()
            .map(RawValue::F64)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string())),
        Yaml::Integer(value) => Ok(RawValue::Signed(i128::from(*value))),
        Yaml::String(value) => Ok(RawValue::String(value.clone())),
        Yaml::Boolean(value) => Ok(RawValue::Bool(*value)),
        Yaml::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| yaml_dynamic_to_raw(value).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(RawValue::Seq),
        Yaml::Hash(values) => values
            .iter()
            .map(|(key, value)| {
                let Some(key) = key.as_str() else {
                    return Err(DataError::invalid_type("hash key", yaml_type_name(key)));
                };
                yaml_dynamic_to_raw(value)
                    .map(|decoded| (RawValue::String(key.to_owned()), decoded))
            })
            .collect::<Result<Vec<_>>>()
            .map(RawValue::Map),
        Yaml::Null => Ok(RawValue::Null),
        Yaml::BadValue => Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "invalid YAML value",
        )),
        Yaml::Alias(_) => Err(DataError::unsupported(
            "YAML aliases are not supported by Arcweft data",
        )),
    }
}

fn raw_dynamic_to_yaml(raw: &RawValue) -> Result<Yaml> {
    match raw {
        RawValue::Null => Ok(Yaml::Null),
        RawValue::Bool(value) => Ok(Yaml::Boolean(*value)),
        RawValue::Signed(value) => signed_to_yaml(*value),
        RawValue::Unsigned(value) => unsigned_to_yaml(*value),
        RawValue::F32(value) => Ok(Yaml::Real(value.to_string())),
        RawValue::F64(value) => Ok(Yaml::Real(value.to_string())),
        RawValue::String(value) => Ok(Yaml::String(value.clone())),
        RawValue::Bytes(value) => bytes_to_yaml(value, BytesFormat::Base64),
        RawValue::Seq(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| raw_dynamic_to_yaml(value).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(Yaml::Array),
        RawValue::Map(entries) => raw_map_to_yaml(entries),
    }
}

pub fn to_yaml(value: &Value, bytes_format: BytesFormat) -> Result<Yaml> {
    match value {
        Value::Unit => Ok(Yaml::Null),
        Value::Bool(value) => Ok(Yaml::Boolean(*value)),
        Value::Number(Number::I(value)) => i64::try_from(*value).map(Yaml::Integer).map_err(|_| {
            DataError::new(
                DataErrorKind::NumberOutOfRange,
                "YAML integer is i64-limited",
            )
        }),
        Value::Number(Number::U(value)) => i64::try_from(*value).map(Yaml::Integer).map_err(|_| {
            DataError::new(
                DataErrorKind::NumberOutOfRange,
                "YAML integer is i64-limited",
            )
        }),
        Value::Number(Number::F32(value)) => Ok(Yaml::Real(value.to_string())),
        Value::Number(Number::F64(value)) => Ok(Yaml::Real(value.to_string())),
        Value::String(value) => Ok(Yaml::String(value.clone())),
        Value::Char(value) => Ok(Yaml::String(value.to_string())),
        Value::Bytes(bytes) => bytes_to_yaml(bytes.as_slice(), bytes_format),
        Value::Seq(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| to_yaml(value, bytes_format).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(Yaml::Array),
        Value::Map(values) | Value::Record(values) => values
            .iter()
            .map(|(key, value)| {
                to_yaml(value, bytes_format)
                    .map(|yaml| (Yaml::String(key.clone()), yaml))
                    .map_err(|err| err.at_field(key.clone()))
            })
            .collect::<Result<Hash>>()
            .map(Yaml::Hash),
        Value::Enum { variant, payload } => {
            let mut hash = Hash::new();
            hash.insert(
                Yaml::String("variant".to_owned()),
                Yaml::String(variant.clone()),
            );
            if let Some(payload) = payload {
                hash.insert(
                    Yaml::String("payload".to_owned()),
                    to_yaml(payload, bytes_format)?,
                );
            }
            Ok(Yaml::Hash(hash))
        }
    }
}

fn bytes_to_yaml(bytes: &[u8], bytes_format: BytesFormat) -> Result<Yaml> {
    match bytes_format {
        BytesFormat::Binary | BytesFormat::Base64 => {
            Ok(Yaml::String(BASE64_STANDARD.encode(bytes)))
        }
        BytesFormat::Hex => {
            let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
            bytes.iter().try_for_each(|byte| {
                write!(&mut encoded, "{byte:02x}").map_err(|error| {
                    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
                })
            })?;
            Ok(Yaml::String(encoded))
        }
        BytesFormat::Array => Ok(Yaml::Array(
            bytes
                .iter()
                .map(|byte| Yaml::Integer(i64::from(*byte)))
                .collect(),
        )),
    }
}

fn yaml_to_bytes(value: &Yaml, bytes_format: BytesFormat) -> Result<Vec<u8>> {
    match bytes_format {
        BytesFormat::Binary | BytesFormat::Base64 => {
            let Yaml::String(value) = value else {
                return Err(DataError::invalid_type(
                    "base64 string",
                    yaml_type_name(value),
                ));
            };
            BASE64_STANDARD
                .decode(value.as_bytes())
                .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
        }
        BytesFormat::Hex => {
            let Yaml::String(value) = value else {
                return Err(DataError::invalid_type("hex string", yaml_type_name(value)));
            };
            decode_hex(value)
        }
        BytesFormat::Array => {
            let Yaml::Array(values) = value else {
                return Err(DataError::invalid_type("byte array", yaml_type_name(value)));
            };
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let Yaml::Integer(value) = value else {
                        return Err(
                            DataError::invalid_type("byte", yaml_type_name(value)).at_index(index)
                        );
                    };
                    u8::try_from(*value)
                        .map_err(|_| {
                            DataError::new(
                                DataErrorKind::NumberOutOfRange,
                                format!("byte value {value} is outside 0..=255"),
                            )
                        })
                        .map_err(|error| error.at_index(index))
                })
                .collect()
        }
    }
}

fn decode_hex(value: &str) -> Result<Vec<u8>> {
    let chunks = value.as_bytes().chunks_exact(2);
    if !chunks.remainder().is_empty() {
        return Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "hex byte string has odd length",
        ));
    }
    chunks
        .map(|chunk| {
            let text = std::str::from_utf8(chunk).map_err(|error| {
                DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
            })?;
            u8::from_str_radix(text, 16)
                .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
        })
        .collect()
}

fn yaml_integer_range_error() -> DataError {
    DataError::new(
        DataErrorKind::NumberOutOfRange,
        "YAML integer is i64-limited",
    )
}

const fn yaml_type_name(value: &Yaml) -> &'static str {
    match value {
        Yaml::Real(_) => "float",
        Yaml::Integer(_) => "integer",
        Yaml::String(_) => "string",
        Yaml::Boolean(_) => "bool",
        Yaml::Array(_) => "array",
        Yaml::Hash(_) => "hash",
        Yaml::Alias(_) => "alias",
        Yaml::Null => "null",
        Yaml::BadValue => "bad value",
    }
}

pub fn from_yaml(value: &Yaml) -> Result<Value> {
    match value {
        Yaml::Real(value) => value
            .parse::<f64>()
            .map(|value| Value::Number(Number::F64(value)))
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string())),
        Yaml::Integer(value) => Ok(Value::Number(Number::I(i128::from(*value)))),
        Yaml::String(value) => Ok(Value::String(value.clone())),
        Yaml::Boolean(value) => Ok(Value::Bool(*value)),
        Yaml::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| from_yaml(value).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(Value::Seq),
        Yaml::Hash(values) => values
            .iter()
            .map(|(key, value)| {
                let Some(key) = key.as_str() else {
                    return Err(DataError::invalid_type(
                        "string map key",
                        "non-string YAML key",
                    ));
                };
                from_yaml(value)
                    .map(|decoded| (key.to_owned(), decoded))
                    .map_err(|err| err.at_field(key))
            })
            .collect::<Result<BTreeMap<_, _>>>()
            .map(Value::Record),
        Yaml::Null | Yaml::BadValue => Ok(Value::Unit),
        Yaml::Alias(_) => Err(DataError::unsupported(
            "YAML aliases are not supported by Arcweft data",
        )),
    }
}
