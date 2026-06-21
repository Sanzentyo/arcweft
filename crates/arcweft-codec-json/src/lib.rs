#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;

use arcweft_data::{
    Bytes, BytesFormat, Codec, DataError, DataErrorKind, DecodeOptions, EncodeOptions, FormatId,
    Result, TypeShape, Value,
};
use base64::prelude::{BASE64_STANDARD, Engine as _};
use serde_json::{Map, Number as JsonNumber, Value as JsonValue};

#[derive(Clone, Copy, Debug, Default)]
pub struct JsonCodec;

impl Codec for JsonCodec {
    fn id(&self) -> FormatId {
        FormatId::new("json")
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["application/json", "text/json"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["json"]
    }

    fn encode_value(
        &self,
        value: &Value,
        _shape: &TypeShape,
        options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let json = to_json_value(value, options.bytes_format)?;
        let bytes = if options.pretty {
            serde_json::to_vec_pretty(&json)
        } else {
            serde_json::to_vec(&json)
        }
        .map_err(|error| json_error(&error))?;
        Ok(bytes)
    }

    fn decode_value(
        &self,
        input: &[u8],
        _shape: &TypeShape,
        options: &DecodeOptions,
    ) -> Result<Value> {
        let json =
            serde_json::from_slice::<JsonValue>(input).map_err(|error| json_error(&error))?;
        let value = from_json_value(&json)?;
        options.limits.validate(&value)?;
        Ok(value)
    }
}

pub fn to_json_value(value: &Value, bytes_format: BytesFormat) -> Result<JsonValue> {
    match value {
        Value::Unit => Ok(JsonValue::Null),
        Value::Bool(value) => Ok(JsonValue::Bool(*value)),
        Value::Number(arcweft_data::Number::I(value)) => i64::try_from(*value)
            .map(JsonNumber::from)
            .map(JsonValue::Number)
            .map_err(|_| {
                DataError::new(
                    DataErrorKind::NumberOutOfRange,
                    "i128 cannot be represented as JSON number",
                )
            }),
        Value::Number(arcweft_data::Number::U(value)) => u64::try_from(*value)
            .map(JsonNumber::from)
            .map(JsonValue::Number)
            .map_err(|_| {
                DataError::new(
                    DataErrorKind::NumberOutOfRange,
                    "u128 cannot be represented as JSON number",
                )
            }),
        Value::Number(arcweft_data::Number::F32(value)) => JsonNumber::from_f64(f64::from(*value))
            .map(JsonValue::Number)
            .ok_or_else(|| {
                DataError::new(
                    DataErrorKind::InvalidEncoding,
                    "non-finite f32 cannot be represented as JSON number",
                )
            }),
        Value::Number(arcweft_data::Number::F64(value)) => JsonNumber::from_f64(*value)
            .map(JsonValue::Number)
            .ok_or_else(|| {
                DataError::new(
                    DataErrorKind::InvalidEncoding,
                    "non-finite f64 cannot be represented as JSON number",
                )
            }),
        Value::String(value) => Ok(JsonValue::String(value.clone())),
        Value::Char(value) => Ok(JsonValue::String(value.to_string())),
        Value::Bytes(bytes) => bytes_to_json(bytes, bytes_format),
        Value::Seq(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                to_json_value(value, bytes_format).map_err(|err| err.at_index(index))
            })
            .collect::<Result<Vec<_>>>()
            .map(JsonValue::Array),
        Value::Map(values) | Value::Record(values) => values
            .iter()
            .map(|(key, value)| {
                to_json_value(value, bytes_format)
                    .map(|json| (key.clone(), json))
                    .map_err(|err| err.at_field(key.clone()))
            })
            .collect::<Result<Map<_, _>>>()
            .map(JsonValue::Object),
        Value::Enum { variant, payload } => {
            let mut object = Map::new();
            object.insert("variant".to_owned(), JsonValue::String(variant.clone()));
            if let Some(payload) = payload {
                object.insert("payload".to_owned(), to_json_value(payload, bytes_format)?);
            }
            Ok(JsonValue::Object(object))
        }
    }
}

pub fn from_json_value(value: &JsonValue) -> Result<Value> {
    match value {
        JsonValue::Null => Ok(Value::Unit),
        JsonValue::Bool(value) => Ok(Value::Bool(*value)),
        JsonValue::Number(number) => json_number(number),
        JsonValue::String(value) => Ok(Value::String(value.clone())),
        JsonValue::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| from_json_value(value).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(Value::Seq),
        JsonValue::Object(values) => values
            .iter()
            .map(|(key, value)| {
                from_json_value(value)
                    .map(|decoded| (key.clone(), decoded))
                    .map_err(|err| err.at_field(key.clone()))
            })
            .collect::<Result<BTreeMap<_, _>>>()
            .map(Value::Record),
    }
}

fn bytes_to_json(bytes: &Bytes, bytes_format: BytesFormat) -> Result<JsonValue> {
    match bytes_format {
        BytesFormat::Binary | BytesFormat::Base64 => {
            Ok(JsonValue::String(BASE64_STANDARD.encode(bytes.as_slice())))
        }
        BytesFormat::Array => bytes
            .as_slice()
            .iter()
            .map(|byte| Ok(JsonValue::Number(JsonNumber::from(*byte))))
            .collect::<Result<Vec<_>>>()
            .map(JsonValue::Array),
        BytesFormat::Hex => {
            let mut encoded = String::with_capacity(bytes.as_slice().len().saturating_mul(2));
            bytes
                .as_slice()
                .iter()
                .try_for_each(|byte| write!(&mut encoded, "{byte:02x}"))
                .map_err(|error| {
                    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
                })?;
            Ok(JsonValue::String(encoded))
        }
    }
}

fn json_number(number: &JsonNumber) -> Result<Value> {
    if let Some(value) = number.as_i64() {
        return Ok(Value::Number(arcweft_data::Number::I(i128::from(value))));
    }
    if let Some(value) = number.as_u64() {
        return Ok(Value::Number(arcweft_data::Number::U(u128::from(value))));
    }
    if let Some(value) = number.as_f64() {
        return Ok(Value::Number(arcweft_data::Number::F64(value)));
    }
    Err(DataError::new(
        DataErrorKind::InvalidEncoding,
        "invalid JSON number",
    ))
}

fn json_error(error: &serde_json::Error) -> DataError {
    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
}
