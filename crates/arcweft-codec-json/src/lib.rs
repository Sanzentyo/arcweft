#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;

use arcweft_data::{
    Bytes, BytesFormat, Codec, DataError, DataErrorKind, DecodeBudget, DecodeOptions,
    EncodeOptions, FieldShape, FormatId, RawValue, Result, TypeShape, Value, decode_with_shape,
    encode_with_shape,
};
use base64::prelude::{BASE64_STANDARD, Engine as _};
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
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
        shape: &TypeShape,
        options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let raw = encode_with_shape(value, shape)?;
        let json = raw_to_json_value(&raw, shape)?;
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
        shape: &TypeShape,
        options: &DecodeOptions,
    ) -> Result<Value> {
        let mut budget = DecodeBudget::new(input.len(), &options.limits)?;
        let mut deserializer = serde_json::Deserializer::from_slice(input);
        let dynamic_raw = BudgetedJsonRawSeed {
            budget: &mut budget,
        }
        .deserialize(&mut deserializer)
        .map_err(|error| json_error(&error))??;
        if !options.limits.allow_trailing_data {
            deserializer.end().map_err(|error| json_error(&error))?;
        }
        let json = raw_dynamic_to_json(&dynamic_raw)?;
        let raw = json_to_raw_value(&json, shape)?;
        let value = decode_with_shape(&raw, shape)?;
        options.limits.validate(&value)?;
        Ok(value)
    }
}

struct BudgetedJsonRawSeed<'budget, 'limits> {
    budget: &'budget mut DecodeBudget<'limits>,
}

impl<'de> DeserializeSeed<'de> for BudgetedJsonRawSeed<'_, '_> {
    type Value = Result<RawValue>;

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(BudgetedJsonRawVisitor {
            budget: self.budget,
        })
    }
}

struct BudgetedJsonRawVisitor<'budget, 'limits> {
    budget: &'budget mut DecodeBudget<'limits>,
}

impl<'de> Visitor<'de> for BudgetedJsonRawVisitor<'_, '_> {
    type Value = Result<RawValue>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_unit<E>(self) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.scalar(RawValue::Null))
    }

    fn visit_bool<E>(self, value: bool) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.scalar(RawValue::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.scalar(RawValue::Signed(i128::from(value))))
    }

    fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.scalar(RawValue::Unsigned(u128::from(value))))
    }

    fn visit_f64<E>(self, value: f64) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.scalar(RawValue::F64(value)))
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.string(value))
    }

    fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.string(value))
    }

    fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.string(value.as_str()))
    }

    fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        if let Err(error) = self.budget.enter_node() {
            return Ok(Err(error));
        }
        let mut values = Vec::new();
        while let Some(value) = seq.next_element_seed(BudgetedJsonRawSeed {
            budget: self.budget,
        })? {
            if let Err(error) = self.budget.sequence_item(values.len().saturating_add(1)) {
                self.budget.exit_node();
                return Ok(Err(error));
            }
            match value {
                Ok(value) => values.push(value),
                Err(error) => {
                    self.budget.exit_node();
                    return Ok(Err(error.at_index(values.len())));
                }
            }
        }
        self.budget.exit_node();
        Ok(Ok(RawValue::Seq(values)))
    }

    fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        if let Err(error) = self.budget.enter_node() {
            return Ok(Err(error));
        }
        let mut entries = Vec::new();
        while let Some(key) = map.next_key_seed(BudgetedJsonRawSeed {
            budget: self.budget,
        })? {
            if let Err(error) = self.budget.map_item(entries.len().saturating_add(1)) {
                self.budget.exit_node();
                return Ok(Err(error));
            }
            let key = match key {
                Ok(key) => key,
                Err(error) => {
                    self.budget.exit_node();
                    return Ok(Err(error));
                }
            };
            let value = map.next_value_seed(BudgetedJsonRawSeed {
                budget: self.budget,
            })?;
            let value = match value {
                Ok(value) => value,
                Err(error) => {
                    self.budget.exit_node();
                    return Ok(Err(error));
                }
            };
            entries.push((key, value));
        }
        self.budget.exit_node();
        Ok(Ok(RawValue::Map(entries)))
    }
}

impl BudgetedJsonRawVisitor<'_, '_> {
    fn scalar(self, raw: RawValue) -> Result<RawValue> {
        self.budget.enter_node()?;
        self.budget.exit_node();
        Ok(raw)
    }

    fn string(self, value: &str) -> Result<RawValue> {
        self.budget.enter_node()?;
        if let Err(error) = self.budget.string_len(value.len()) {
            self.budget.exit_node();
            return Err(error);
        }
        self.budget.exit_node();
        Ok(RawValue::String(value.to_owned()))
    }
}

fn raw_to_json_value(raw: &RawValue, shape: &TypeShape) -> Result<JsonValue> {
    match (shape, raw) {
        (TypeShape::Unit, RawValue::Null) => Ok(JsonValue::Null),
        (TypeShape::Bool, RawValue::Bool(value)) => Ok(JsonValue::Bool(*value)),
        (
            TypeShape::I8
            | TypeShape::I16
            | TypeShape::I32
            | TypeShape::I64
            | TypeShape::I128
            | TypeShape::Isize,
            RawValue::Signed(value),
        ) => signed_to_json(*value),
        (
            TypeShape::U8
            | TypeShape::U16
            | TypeShape::U32
            | TypeShape::U64
            | TypeShape::U128
            | TypeShape::Usize,
            RawValue::Unsigned(value),
        ) => unsigned_to_json(*value),
        (TypeShape::F32, RawValue::F32(value)) => float_to_json(f64::from(*value), "f32"),
        (TypeShape::F64, RawValue::F64(value)) => float_to_json(*value, "f64"),
        (TypeShape::String | TypeShape::Char, RawValue::String(value)) => {
            Ok(JsonValue::String(value.clone()))
        }
        (TypeShape::Bytes { format }, RawValue::Bytes(bytes)) => {
            bytes_to_json(&Bytes::new(bytes.clone()), *format)
        }
        (TypeShape::Option(inner), RawValue::Null) => {
            let _ = inner;
            Ok(JsonValue::Null)
        }
        (TypeShape::Option(inner), raw) => raw_to_json_value(raw, inner),
        (TypeShape::Seq(inner), RawValue::Seq(values)) => raw_seq_to_json(values, inner),
        (TypeShape::Map { key, value }, RawValue::Map(entries))
            if matches!(key.as_ref(), TypeShape::String) =>
        {
            raw_string_map_to_json(entries, value)
        }
        (TypeShape::Record { fields, .. }, RawValue::Map(entries)) => {
            raw_record_to_json(entries, fields)
        }
        (TypeShape::Enum { .. }, raw) => raw_dynamic_to_json(raw),
        (TypeShape::Named(_), _) => Err(DataError::unsupported(
            "named shape must be resolved before JSON encoding",
        )),
        _ => Err(DataError::invalid_type(shape.type_name(), raw.type_name())),
    }
}

fn signed_to_json(value: i128) -> Result<JsonValue> {
    i64::try_from(value)
        .map(JsonNumber::from)
        .map(JsonValue::Number)
        .map_err(|_| {
            DataError::new(
                DataErrorKind::NumberOutOfRange,
                "signed integer cannot be represented as JSON number",
            )
        })
}

fn unsigned_to_json(value: u128) -> Result<JsonValue> {
    u64::try_from(value)
        .map(JsonNumber::from)
        .map(JsonValue::Number)
        .map_err(|_| {
            DataError::new(
                DataErrorKind::NumberOutOfRange,
                "unsigned integer cannot be represented as JSON number",
            )
        })
}

fn float_to_json(value: f64, label: &'static str) -> Result<JsonValue> {
    JsonNumber::from_f64(value)
        .map(JsonValue::Number)
        .ok_or_else(|| DataError::new(DataErrorKind::InvalidEncoding, format!("invalid {label}")))
}

fn raw_seq_to_json(values: &[RawValue], shape: &TypeShape) -> Result<JsonValue> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            raw_to_json_value(value, shape).map_err(|error| error.at_index(index))
        })
        .collect::<Result<Vec<_>>>()
        .map(JsonValue::Array)
}

fn raw_string_map_to_json(
    entries: &[(RawValue, RawValue)],
    shape: &TypeShape,
) -> Result<JsonValue> {
    entries
        .iter()
        .map(|(key, raw_value)| {
            let RawValue::String(key) = key else {
                return Err(DataError::invalid_type("string map key", key.type_name()));
            };
            raw_to_json_value(raw_value, shape)
                .map(|json| (key.clone(), json))
                .map_err(|error| error.at_field(key.clone()))
        })
        .collect::<Result<Map<_, _>>>()
        .map(JsonValue::Object)
}

fn raw_record_to_json(
    entries: &[(RawValue, RawValue)],
    fields: &[FieldShape],
) -> Result<JsonValue> {
    entries
        .iter()
        .map(|(key, raw_value)| {
            let RawValue::String(key) = key else {
                return Err(DataError::invalid_type("record field key", key.type_name()));
            };
            let shape = fields
                .iter()
                .find(|field| field.wire_name == *key)
                .map_or(TypeShape::Unit, json_field_shape);
            raw_to_json_value(raw_value, &shape)
                .map(|json| (key.clone(), json))
                .map_err(|error| error.at_field(key.clone()))
        })
        .collect::<Result<Map<_, _>>>()
        .map(JsonValue::Object)
}

fn json_to_raw_value(value: &JsonValue, shape: &TypeShape) -> Result<RawValue> {
    match shape {
        TypeShape::Unit => match value {
            JsonValue::Null => Ok(RawValue::Null),
            other => Err(DataError::invalid_type("null", json_type_name(other))),
        },
        TypeShape::Bool => match value {
            JsonValue::Bool(value) => Ok(RawValue::Bool(*value)),
            other => Err(DataError::invalid_type("bool", json_type_name(other))),
        },
        TypeShape::String | TypeShape::Char => match value {
            JsonValue::String(value) => Ok(RawValue::String(value.clone())),
            other => Err(DataError::invalid_type("string", json_type_name(other))),
        },
        TypeShape::Bytes { format } => json_to_bytes(value, *format).map(RawValue::Bytes),
        TypeShape::Option(inner) => match value {
            JsonValue::Null => Ok(RawValue::Null),
            other => json_to_raw_value(other, inner),
        },
        TypeShape::Seq(inner) => match value {
            JsonValue::Array(values) => values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    json_to_raw_value(value, inner).map_err(|error| error.at_index(index))
                })
                .collect::<Result<Vec<_>>>()
                .map(RawValue::Seq),
            other => Err(DataError::invalid_type("array", json_type_name(other))),
        },
        TypeShape::Map { key, value: inner } if matches!(key.as_ref(), TypeShape::String) => {
            json_object_entries(value, inner).map(RawValue::Map)
        }
        TypeShape::Record { fields, .. } => match value {
            JsonValue::Object(entries) => entries
                .iter()
                .map(|(key, value)| {
                    let raw = match fields.iter().find(|field| field.wire_name == *key) {
                        Some(field) => json_to_raw_value(value, &json_field_shape(field)),
                        None => json_dynamic_to_raw(value),
                    }?;
                    Ok((RawValue::String(key.clone()), raw))
                })
                .collect::<Result<Vec<_>>>()
                .map(RawValue::Map),
            other => Err(DataError::invalid_type("object", json_type_name(other))),
        },
        TypeShape::Enum { .. } => json_dynamic_to_raw(value),
        TypeShape::Named(_) => Err(DataError::unsupported(
            "named shape must be resolved before JSON decoding",
        )),
        TypeShape::F32 | TypeShape::F64 => json_float_to_raw(value, shape),
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
        | TypeShape::Usize => json_integer_to_raw(value),
        TypeShape::Map { .. } => Err(DataError::unsupported(
            "JSON shape codec supports string map keys only",
        )),
    }
}

fn json_object_entries(value: &JsonValue, shape: &TypeShape) -> Result<Vec<(RawValue, RawValue)>> {
    match value {
        JsonValue::Object(entries) => entries
            .iter()
            .map(|(key, value)| {
                json_to_raw_value(value, shape)
                    .map(|raw| (RawValue::String(key.clone()), raw))
                    .map_err(|error| error.at_field(key.clone()))
            })
            .collect(),
        other => Err(DataError::invalid_type("object", json_type_name(other))),
    }
}

fn raw_map_to_json(entries: &[(RawValue, RawValue)]) -> Result<JsonValue> {
    entries
        .iter()
        .map(|(key, value)| {
            let RawValue::String(key) = key else {
                return Err(DataError::invalid_type("object key", key.type_name()));
            };
            raw_dynamic_to_json(value).map(|json| (key.clone(), json))
        })
        .collect::<Result<Map<_, _>>>()
        .map(JsonValue::Object)
}

fn json_integer_to_raw(value: &JsonValue) -> Result<RawValue> {
    let JsonValue::Number(number) = value else {
        return Err(DataError::invalid_type("number", json_type_name(value)));
    };
    if let Some(value) = number.as_i64() {
        return Ok(RawValue::Signed(i128::from(value)));
    }
    if let Some(value) = number.as_u64() {
        return Ok(RawValue::Unsigned(u128::from(value)));
    }
    Err(DataError::new(
        DataErrorKind::InvalidEncoding,
        "floating-point JSON number cannot decode as integer",
    ))
}

fn json_float_to_raw(value: &JsonValue, shape: &TypeShape) -> Result<RawValue> {
    let JsonValue::Number(number) = value else {
        return Err(DataError::invalid_type("number", json_type_name(value)));
    };
    let Some(value) = number.as_f64() else {
        return Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "invalid JSON float",
        ));
    };
    match shape {
        TypeShape::F32 => parse_json_f32(value).map(RawValue::F32),
        TypeShape::F64 => Ok(RawValue::F64(value)),
        _ => unreachable!("caller passes float shape"),
    }
}

fn parse_json_f32(value: f64) -> Result<f32> {
    let decoded = value.to_string().parse::<f32>().map_err(|error| {
        DataError::new(
            DataErrorKind::NumberOutOfRange,
            format!("cannot decode f32 from JSON number: {error}"),
        )
    })?;
    if decoded.is_finite() {
        Ok(decoded)
    } else {
        Err(DataError::new(
            DataErrorKind::NumberOutOfRange,
            "JSON number is out of range for f32",
        ))
    }
}

fn json_dynamic_to_raw(value: &JsonValue) -> Result<RawValue> {
    match value {
        JsonValue::Null => Ok(RawValue::Null),
        JsonValue::Bool(value) => Ok(RawValue::Bool(*value)),
        JsonValue::Number(number) => {
            if let Some(value) = number.as_i64() {
                return Ok(RawValue::Signed(i128::from(value)));
            }
            if let Some(value) = number.as_u64() {
                return Ok(RawValue::Unsigned(u128::from(value)));
            }
            number.as_f64().map(RawValue::F64).ok_or_else(|| {
                DataError::new(DataErrorKind::InvalidEncoding, "invalid JSON number")
            })
        }
        JsonValue::String(value) => Ok(RawValue::String(value.clone())),
        JsonValue::Array(values) => values
            .iter()
            .map(json_dynamic_to_raw)
            .collect::<Result<Vec<_>>>()
            .map(RawValue::Seq),
        JsonValue::Object(entries) => entries
            .iter()
            .map(|(key, value)| {
                json_dynamic_to_raw(value).map(|raw| (RawValue::String(key.clone()), raw))
            })
            .collect::<Result<Vec<_>>>()
            .map(RawValue::Map),
    }
}

fn raw_dynamic_to_json(raw: &RawValue) -> Result<JsonValue> {
    match raw {
        RawValue::Null => Ok(JsonValue::Null),
        RawValue::Bool(value) => Ok(JsonValue::Bool(*value)),
        RawValue::Signed(value) => i64::try_from(*value)
            .map(JsonNumber::from)
            .map(JsonValue::Number)
            .map_err(|_| {
                DataError::new(DataErrorKind::NumberOutOfRange, "i128 outside JSON range")
            }),
        RawValue::Unsigned(value) => u64::try_from(*value)
            .map(JsonNumber::from)
            .map(JsonValue::Number)
            .map_err(|_| {
                DataError::new(DataErrorKind::NumberOutOfRange, "u128 outside JSON range")
            }),
        RawValue::F32(value) => JsonNumber::from_f64(f64::from(*value))
            .map(JsonValue::Number)
            .ok_or_else(|| DataError::new(DataErrorKind::InvalidEncoding, "invalid f32")),
        RawValue::F64(value) => JsonNumber::from_f64(*value)
            .map(JsonValue::Number)
            .ok_or_else(|| DataError::new(DataErrorKind::InvalidEncoding, "invalid f64")),
        RawValue::String(value) => Ok(JsonValue::String(value.clone())),
        RawValue::Bytes(value) => bytes_to_json(&Bytes::new(value.clone()), BytesFormat::Base64),
        RawValue::Seq(values) => values
            .iter()
            .map(raw_dynamic_to_json)
            .collect::<Result<Vec<_>>>()
            .map(JsonValue::Array),
        RawValue::Map(entries) => raw_map_to_json(entries),
    }
}

fn json_to_bytes(value: &JsonValue, bytes_format: BytesFormat) -> Result<Vec<u8>> {
    match bytes_format {
        BytesFormat::Binary | BytesFormat::Base64 => {
            let JsonValue::String(value) = value else {
                return Err(DataError::invalid_type(
                    "base64 string",
                    json_type_name(value),
                ));
            };
            BASE64_STANDARD
                .decode(value.as_bytes())
                .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
        }
        BytesFormat::Hex => {
            let JsonValue::String(value) = value else {
                return Err(DataError::invalid_type("hex string", json_type_name(value)));
            };
            decode_hex(value)
        }
        BytesFormat::Array => {
            let JsonValue::Array(values) = value else {
                return Err(DataError::invalid_type("byte array", json_type_name(value)));
            };
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let JsonValue::Number(number) = value else {
                        return Err(
                            DataError::invalid_type("byte", json_type_name(value)).at_index(index)
                        );
                    };
                    let Some(value) = number.as_u64() else {
                        return Err(
                            DataError::invalid_type("byte", "negative or float").at_index(index)
                        );
                    };
                    u8::try_from(value)
                        .map_err(|_| {
                            DataError::new(
                                DataErrorKind::NumberOutOfRange,
                                format!("byte value {value} exceeds 255"),
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

fn json_field_shape(field: &FieldShape) -> TypeShape {
    match (field.bytes_format, &field.shape) {
        (Some(format), TypeShape::Bytes { .. }) => TypeShape::Bytes { format },
        _ => field.shape.clone(),
    }
}

const fn json_type_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "bool",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
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
