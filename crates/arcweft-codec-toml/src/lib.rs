#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use arcweft_data::{
    BytesFormat, Codec, DataError, DataErrorKind, DecodeOptions, EncodeOptions, FormatId, Number,
    Result, TypeShape, Value,
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
        _shape: &TypeShape,
        options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let toml = to_toml(value, options.bytes_format)?;
        toml::to_string_pretty(&toml)
            .map(String::into_bytes)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
    }

    fn decode_value(
        &self,
        input: &[u8],
        _shape: &TypeShape,
        options: &DecodeOptions,
    ) -> Result<Value> {
        let source = std::str::from_utf8(input)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        let toml = source
            .parse::<TomlValue>()
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        let value = from_toml(&toml)?;
        options.limits.validate(&value)?;
        Ok(value)
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
        Value::Bytes(bytes) => match bytes_format {
            BytesFormat::Array => Ok(TomlValue::Array(
                bytes
                    .as_slice()
                    .iter()
                    .map(|byte| TomlValue::Integer(i64::from(*byte)))
                    .collect(),
            )),
            _ => Ok(TomlValue::String(BASE64_STANDARD.encode(bytes.as_slice()))),
        },
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
