#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;

use arcweft_data::{
    BytesFormat, Codec, DataError, DataErrorKind, DecodeOptions, EncodeOptions, FormatId, Number,
    RawValue, Result, TypeShape, Value, decode_with_shape, encode_with_shape,
};
use base64::prelude::{BASE64_STANDARD, Engine as _};
use toml::Value as TomlValue;

#[derive(Clone, Copy, Debug, Default)]
pub struct TomlCodec;

impl Codec for TomlCodec {
    fn id(&self) -> FormatId {
        FormatId::new("toml")
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["application/toml", "application/x-toml"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["toml"]
    }

    fn encode_value(
        &self,
        value: &Value,
        shape: &TypeShape,
        _options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let raw = encode_with_shape(value, shape)?;
        let toml = raw_to_toml_value(&raw, shape)?;
        toml::to_string_pretty(&toml)
            .map(String::into_bytes)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
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
        let toml = toml::from_str::<TomlValue>(source)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        let raw = toml_to_raw_value(&toml, shape)?;
        let value = decode_with_shape(&raw, shape)?;
        options.limits.validate(&value)?;
        Ok(value)
    }
}

fn raw_to_toml_value(raw: &RawValue, shape: &TypeShape) -> Result<TomlValue> {
    match (shape, raw) {
        (TypeShape::Bool, RawValue::Bool(value)) => Ok(TomlValue::Boolean(*value)),
        (
            TypeShape::I8
            | TypeShape::I16
            | TypeShape::I32
            | TypeShape::I64
            | TypeShape::I128
            | TypeShape::Isize,
            RawValue::Signed(value),
        ) => signed_to_toml(*value),
        (
            TypeShape::U8
            | TypeShape::U16
            | TypeShape::U32
            | TypeShape::U64
            | TypeShape::U128
            | TypeShape::Usize,
            RawValue::Unsigned(value),
        ) => unsigned_to_toml(*value),
        (TypeShape::F32, RawValue::F32(value)) => Ok(TomlValue::Float(f64::from(*value))),
        (TypeShape::F64, RawValue::F64(value)) => Ok(TomlValue::Float(*value)),
        (TypeShape::String | TypeShape::Char, RawValue::String(value)) => {
            Ok(TomlValue::String(value.clone()))
        }
        (TypeShape::Bytes { format }, RawValue::Bytes(bytes)) => bytes_to_toml(bytes, *format),
        (TypeShape::Unit | TypeShape::Option(_), RawValue::Null) => Err(toml_null_error()),
        (TypeShape::Option(inner), raw) => raw_to_toml_value(raw, inner),
        (TypeShape::Seq(inner), RawValue::Seq(values)) => raw_seq_to_toml(values, inner),
        (TypeShape::Map { key, value }, RawValue::Map(entries))
            if matches!(key.as_ref(), TypeShape::String) =>
        {
            raw_string_map_to_toml(entries, value)
        }
        (TypeShape::Record { fields, .. }, RawValue::Map(entries)) => {
            raw_record_to_toml(entries, fields)
        }
        (TypeShape::Enum { .. }, raw) => raw_dynamic_to_toml(raw),
        (TypeShape::Named(_), _) => Err(DataError::unsupported(
            "named shape must be resolved before TOML encoding",
        )),
        _ => Err(DataError::invalid_type(shape.type_name(), raw.type_name())),
    }
}

fn signed_to_toml(value: i128) -> Result<TomlValue> {
    i64::try_from(value)
        .map(TomlValue::Integer)
        .map_err(|_| toml_integer_range_error())
}

fn unsigned_to_toml(value: u128) -> Result<TomlValue> {
    i64::try_from(value)
        .map(TomlValue::Integer)
        .map_err(|_| toml_integer_range_error())
}

fn raw_seq_to_toml(values: &[RawValue], shape: &TypeShape) -> Result<TomlValue> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            raw_to_toml_value(value, shape).map_err(|error| error.at_index(index))
        })
        .collect::<Result<Vec<_>>>()
        .map(TomlValue::Array)
}

fn raw_string_map_to_toml(
    entries: &[(RawValue, RawValue)],
    shape: &TypeShape,
) -> Result<TomlValue> {
    entries
        .iter()
        .map(|(key, raw_value)| {
            let RawValue::String(key) = key else {
                return Err(DataError::invalid_type("string map key", key.type_name()));
            };
            raw_to_toml_value(raw_value, shape)
                .map(|toml| (key.clone(), toml))
                .map_err(|error| error.at_field(key.clone()))
        })
        .collect::<Result<toml::Table>>()
        .map(TomlValue::Table)
}

fn raw_record_to_toml(
    entries: &[(RawValue, RawValue)],
    fields: &[arcweft_data::FieldShape],
) -> Result<TomlValue> {
    entries
        .iter()
        .filter_map(|(key, raw_value)| {
            let RawValue::String(key) = key else {
                return Some(Err(DataError::invalid_type(
                    "record field key",
                    key.type_name(),
                )));
            };
            let shape = fields
                .iter()
                .find(|field| field.wire_name == *key)
                .map_or(TypeShape::Unit, arcweft_data::FieldShape::value_shape);
            if matches!(shape, TypeShape::Option(_)) && matches!(raw_value, RawValue::Null) {
                return None;
            }
            Some(
                raw_to_toml_value(raw_value, &shape)
                    .map(|toml| (key.clone(), toml))
                    .map_err(|error| error.at_field(key.clone())),
            )
        })
        .collect::<Result<toml::Table>>()
        .map(TomlValue::Table)
}

fn raw_map_to_toml(entries: &[(RawValue, RawValue)]) -> Result<TomlValue> {
    entries
        .iter()
        .map(|(key, value)| {
            let RawValue::String(key) = key else {
                return Err(DataError::invalid_type("table key", key.type_name()));
            };
            raw_dynamic_to_toml(value).map(|toml| (key.clone(), toml))
        })
        .collect::<Result<toml::Table>>()
        .map(TomlValue::Table)
}

fn toml_to_raw_value(value: &TomlValue, shape: &TypeShape) -> Result<RawValue> {
    match shape {
        TypeShape::Unit => Err(DataError::invalid_type("null", toml_type_name(value))),
        TypeShape::Bool => match value {
            TomlValue::Boolean(value) => Ok(RawValue::Bool(*value)),
            other => Err(DataError::invalid_type("bool", toml_type_name(other))),
        },
        TypeShape::String | TypeShape::Char => match value {
            TomlValue::String(value) => Ok(RawValue::String(value.clone())),
            TomlValue::Datetime(value) => Ok(RawValue::String(value.to_string())),
            other => Err(DataError::invalid_type("string", toml_type_name(other))),
        },
        TypeShape::Bytes { format } => toml_to_bytes(value, *format).map(RawValue::Bytes),
        TypeShape::Option(inner) => toml_to_raw_value(value, inner),
        TypeShape::Seq(inner) => match value {
            TomlValue::Array(values) => values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    toml_to_raw_value(value, inner).map_err(|error| error.at_index(index))
                })
                .collect::<Result<Vec<_>>>()
                .map(RawValue::Seq),
            other => Err(DataError::invalid_type("array", toml_type_name(other))),
        },
        TypeShape::Map { key, value: inner } if matches!(key.as_ref(), TypeShape::String) => {
            toml_table_entries(value, inner).map(RawValue::Map)
        }
        TypeShape::Record { fields, .. } => match value {
            TomlValue::Table(entries) => entries
                .iter()
                .map(|(key, value)| {
                    let raw = match fields.iter().find(|field| field.wire_name == *key) {
                        Some(field) => toml_to_raw_value(value, &field.value_shape()),
                        None => toml_dynamic_to_raw(value),
                    }?;
                    Ok((RawValue::String(key.clone()), raw))
                })
                .collect::<Result<Vec<_>>>()
                .map(RawValue::Map),
            other => Err(DataError::invalid_type("table", toml_type_name(other))),
        },
        TypeShape::Enum { .. } => toml_dynamic_to_raw(value),
        TypeShape::Named(_) => Err(DataError::unsupported(
            "named shape must be resolved before TOML decoding",
        )),
        TypeShape::F32 | TypeShape::F64 => toml_float_to_raw(value, shape),
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
        | TypeShape::Usize => toml_integer_to_raw(value, shape),
        TypeShape::Map { .. } => Err(DataError::unsupported(
            "TOML shape codec supports string map keys only",
        )),
    }
}

fn toml_table_entries(value: &TomlValue, shape: &TypeShape) -> Result<Vec<(RawValue, RawValue)>> {
    match value {
        TomlValue::Table(entries) => entries
            .iter()
            .map(|(key, value)| {
                toml_to_raw_value(value, shape)
                    .map(|raw| (RawValue::String(key.clone()), raw))
                    .map_err(|error| error.at_field(key.clone()))
            })
            .collect(),
        other => Err(DataError::invalid_type("table", toml_type_name(other))),
    }
}

fn toml_integer_to_raw(value: &TomlValue, _shape: &TypeShape) -> Result<RawValue> {
    let TomlValue::Integer(value) = value else {
        return Err(DataError::invalid_type("integer", toml_type_name(value)));
    };
    Ok(RawValue::Signed(i128::from(*value)))
}

fn toml_float_to_raw(value: &TomlValue, shape: &TypeShape) -> Result<RawValue> {
    match value {
        TomlValue::Float(value) => match shape {
            TypeShape::F32 => {
                value
                    .to_string()
                    .parse::<f32>()
                    .map(RawValue::F32)
                    .map_err(|error| {
                        DataError::new(DataErrorKind::NumberOutOfRange, error.to_string())
                    })
            }
            TypeShape::F64 => Ok(RawValue::F64(*value)),
            _ => unreachable!("caller passes float shape"),
        },
        TomlValue::Integer(value) => match shape {
            TypeShape::F32 => {
                value
                    .to_string()
                    .parse::<f32>()
                    .map(RawValue::F32)
                    .map_err(|error| {
                        DataError::new(DataErrorKind::NumberOutOfRange, error.to_string())
                    })
            }
            TypeShape::F64 => {
                value
                    .to_string()
                    .parse::<f64>()
                    .map(RawValue::F64)
                    .map_err(|error| {
                        DataError::new(DataErrorKind::NumberOutOfRange, error.to_string())
                    })
            }
            _ => unreachable!("caller passes float shape"),
        },
        other => Err(DataError::invalid_type("float", toml_type_name(other))),
    }
}

fn toml_dynamic_to_raw(value: &TomlValue) -> Result<RawValue> {
    match value {
        TomlValue::String(value) => Ok(RawValue::String(value.clone())),
        TomlValue::Integer(value) => Ok(RawValue::Signed(i128::from(*value))),
        TomlValue::Float(value) => Ok(RawValue::F64(*value)),
        TomlValue::Boolean(value) => Ok(RawValue::Bool(*value)),
        TomlValue::Datetime(value) => Ok(RawValue::String(value.to_string())),
        TomlValue::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| toml_dynamic_to_raw(value).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(RawValue::Seq),
        TomlValue::Table(values) => values
            .iter()
            .map(|(key, value)| {
                toml_dynamic_to_raw(value).map(|decoded| (RawValue::String(key.clone()), decoded))
            })
            .collect::<Result<Vec<_>>>()
            .map(RawValue::Map),
    }
}

fn raw_dynamic_to_toml(raw: &RawValue) -> Result<TomlValue> {
    match raw {
        RawValue::Null => Err(toml_null_error()),
        RawValue::Bool(value) => Ok(TomlValue::Boolean(*value)),
        RawValue::Signed(value) => signed_to_toml(*value),
        RawValue::Unsigned(value) => unsigned_to_toml(*value),
        RawValue::F32(value) => Ok(TomlValue::Float(f64::from(*value))),
        RawValue::F64(value) => Ok(TomlValue::Float(*value)),
        RawValue::String(value) => Ok(TomlValue::String(value.clone())),
        RawValue::Bytes(value) => bytes_to_toml(value, BytesFormat::Base64),
        RawValue::Seq(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| raw_dynamic_to_toml(value).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(TomlValue::Array),
        RawValue::Map(entries) => raw_map_to_toml(entries),
    }
}

pub fn to_toml(value: &Value, bytes_format: BytesFormat) -> Result<TomlValue> {
    match value {
        Value::Unit => Ok(TomlValue::String(String::new())),
        Value::Bool(value) => Ok(TomlValue::Boolean(*value)),
        Value::Number(Number::I(value)) => {
            i64::try_from(*value).map(TomlValue::Integer).map_err(|_| {
                DataError::new(
                    DataErrorKind::NumberOutOfRange,
                    "TOML integer is i64-limited",
                )
            })
        }
        Value::Number(Number::U(value)) => {
            i64::try_from(*value).map(TomlValue::Integer).map_err(|_| {
                DataError::new(
                    DataErrorKind::NumberOutOfRange,
                    "TOML integer is i64-limited",
                )
            })
        }
        Value::Number(Number::F32(value)) => Ok(TomlValue::Float(f64::from(*value))),
        Value::Number(Number::F64(value)) => Ok(TomlValue::Float(*value)),
        Value::String(value) => Ok(TomlValue::String(value.clone())),
        Value::Char(value) => Ok(TomlValue::String(value.to_string())),
        Value::Bytes(bytes) => bytes_to_toml(bytes.as_slice(), bytes_format),
        Value::Seq(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| to_toml(value, bytes_format).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(TomlValue::Array),
        Value::Map(values) | Value::Record(values) => values
            .iter()
            .map(|(key, value)| {
                to_toml(value, bytes_format)
                    .map(|toml| (key.clone(), toml))
                    .map_err(|err| err.at_field(key.clone()))
            })
            .collect::<Result<toml::Table>>()
            .map(TomlValue::Table),
        Value::Enum { variant, payload } => {
            let mut table = toml::Table::new();
            table.insert("variant".to_owned(), TomlValue::String(variant.clone()));
            if let Some(payload) = payload {
                table.insert("payload".to_owned(), to_toml(payload, bytes_format)?);
            }
            Ok(TomlValue::Table(table))
        }
    }
}

fn bytes_to_toml(bytes: &[u8], bytes_format: BytesFormat) -> Result<TomlValue> {
    match bytes_format {
        BytesFormat::Binary | BytesFormat::Base64 => {
            Ok(TomlValue::String(BASE64_STANDARD.encode(bytes)))
        }
        BytesFormat::Hex => {
            let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
            bytes.iter().try_for_each(|byte| {
                write!(&mut encoded, "{byte:02x}").map_err(|error| {
                    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
                })
            })?;
            Ok(TomlValue::String(encoded))
        }
        BytesFormat::Array => Ok(TomlValue::Array(
            bytes
                .iter()
                .map(|byte| TomlValue::Integer(i64::from(*byte)))
                .collect(),
        )),
    }
}

fn toml_to_bytes(value: &TomlValue, bytes_format: BytesFormat) -> Result<Vec<u8>> {
    match bytes_format {
        BytesFormat::Binary | BytesFormat::Base64 => {
            let TomlValue::String(value) = value else {
                return Err(DataError::invalid_type(
                    "base64 string",
                    toml_type_name(value),
                ));
            };
            BASE64_STANDARD
                .decode(value.as_bytes())
                .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
        }
        BytesFormat::Hex => {
            let TomlValue::String(value) = value else {
                return Err(DataError::invalid_type("hex string", toml_type_name(value)));
            };
            decode_hex(value)
        }
        BytesFormat::Array => {
            let TomlValue::Array(values) = value else {
                return Err(DataError::invalid_type("byte array", toml_type_name(value)));
            };
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let TomlValue::Integer(value) = value else {
                        return Err(
                            DataError::invalid_type("byte", toml_type_name(value)).at_index(index)
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

fn toml_null_error() -> DataError {
    DataError::unsupported(
        "TOML has no null value; Option::None is only supported as record field omission",
    )
}

fn toml_integer_range_error() -> DataError {
    DataError::new(
        DataErrorKind::NumberOutOfRange,
        "TOML integer is i64-limited",
    )
}

const fn toml_type_name(value: &TomlValue) -> &'static str {
    match value {
        TomlValue::String(_) => "string",
        TomlValue::Integer(_) => "integer",
        TomlValue::Float(_) => "float",
        TomlValue::Boolean(_) => "bool",
        TomlValue::Datetime(_) => "datetime",
        TomlValue::Array(_) => "array",
        TomlValue::Table(_) => "table",
    }
}

pub fn from_toml(value: &TomlValue) -> Result<Value> {
    match value {
        TomlValue::String(value) => Ok(Value::String(value.clone())),
        TomlValue::Integer(value) => Ok(Value::Number(Number::I(i128::from(*value)))),
        TomlValue::Float(value) => Ok(Value::Number(Number::F64(*value))),
        TomlValue::Boolean(value) => Ok(Value::Bool(*value)),
        TomlValue::Datetime(value) => Ok(Value::String(value.to_string())),
        TomlValue::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| from_toml(value).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(Value::Seq),
        TomlValue::Table(values) => values
            .iter()
            .map(|(key, value)| {
                from_toml(value)
                    .map(|decoded| (key.clone(), decoded))
                    .map_err(|err| err.at_field(key.clone()))
            })
            .collect::<Result<BTreeMap<_, _>>>()
            .map(Value::Record),
    }
}
