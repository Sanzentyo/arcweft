#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use apache_avro::types::Value as AvroValue;
use apache_avro::{Reader, Schema, Writer};
use arcweft_data::{
    Bytes, Codec, DataError, DataErrorKind, DecodeOptions, EncodeOptions, FormatId, Number, Result,
    TypeShape, Value,
};

#[derive(Clone, Debug)]
pub struct AvroCodec {
    schema: Schema,
}

impl AvroCodec {
    pub fn new(schema_json: &str) -> Result<Self> {
        Schema::parse_str(schema_json)
            .map(|schema| Self { schema })
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
    }
}

impl Codec for AvroCodec {
    fn id(&self) -> FormatId {
        FormatId::new("avro")
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["application/avro", "application/vnd.apache.avro"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["avro"]
    }

    fn encode_value(
        &self,
        value: &Value,
        _shape: &TypeShape,
        _options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let mut writer = Writer::new(&self.schema, Vec::new());
        match value {
            Value::Seq(values) => values.iter().enumerate().try_for_each(|(index, value)| {
                writer.append(to_avro(value)?).map(|_| ()).map_err(|error| {
                    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
                        .at_index(index)
                })
            })?,
            other => writer
                .append(to_avro(other)?)
                .map(|_| ())
                .map_err(|error| {
                    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
                })?,
        }
        writer
            .into_inner()
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
    }

    fn decode_value(
        &self,
        input: &[u8],
        _shape: &TypeShape,
        options: &DecodeOptions,
    ) -> Result<Value> {
        let reader = Reader::new(input)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        let rows = reader
            .enumerate()
            .map(|(index, value)| {
                value
                    .map_err(|error| {
                        DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
                            .at_index(index)
                    })
                    .and_then(|value| from_avro(&value).map_err(|err| err.at_index(index)))
            })
            .collect::<Result<Vec<_>>>()?;
        let value = Value::Seq(rows);
        options.limits.validate(&value)?;
        Ok(value)
    }
}

pub fn to_avro(value: &Value) -> Result<AvroValue> {
    match value {
        Value::Unit => Ok(AvroValue::Null),
        Value::Bool(value) => Ok(AvroValue::Boolean(*value)),
        Value::Number(Number::I(value)) => {
            i64::try_from(*value).map(AvroValue::Long).map_err(|_| {
                DataError::new(DataErrorKind::NumberOutOfRange, "Avro long is i64-limited")
            })
        }
        Value::Number(Number::U(value)) => {
            i64::try_from(*value).map(AvroValue::Long).map_err(|_| {
                DataError::new(DataErrorKind::NumberOutOfRange, "Avro long is i64-limited")
            })
        }
        Value::Number(Number::F32(value)) => Ok(AvroValue::Float(*value)),
        Value::Number(Number::F64(value)) => Ok(AvroValue::Double(*value)),
        Value::String(value) => Ok(AvroValue::String(value.clone())),
        Value::Char(value) => Ok(AvroValue::String(value.to_string())),
        Value::Bytes(bytes) => Ok(AvroValue::Bytes(bytes.as_slice().to_vec())),
        Value::Seq(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| to_avro(value).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(AvroValue::Array),
        Value::Map(values) | Value::Record(values) => values
            .iter()
            .map(|(key, value)| {
                to_avro(value)
                    .map(|avro| (key.clone(), avro))
                    .map_err(|err| err.at_field(key.clone()))
            })
            .collect::<Result<BTreeMap<_, _>>>()
            .map(|map| AvroValue::Map(map.into_iter().collect())),
        Value::Enum { variant, payload } => match payload {
            Some(payload) => Ok(AvroValue::Record(vec![
                ("variant".to_owned(), AvroValue::String(variant.clone())),
                ("payload".to_owned(), to_avro(payload)?),
            ])),
            None => Ok(AvroValue::Enum(0, variant.clone())),
        },
    }
}

pub fn from_avro(value: &AvroValue) -> Result<Value> {
    match value {
        AvroValue::Null => Ok(Value::Unit),
        AvroValue::Boolean(value) => Ok(Value::Bool(*value)),
        AvroValue::Int(value) | AvroValue::Date(value) => {
            Ok(Value::Number(Number::I(i128::from(*value))))
        }
        AvroValue::Long(value) => Ok(Value::Number(Number::I(i128::from(*value)))),
        AvroValue::Float(value) => Ok(Value::Number(Number::F32(*value))),
        AvroValue::Double(value) => Ok(Value::Number(Number::F64(*value))),
        AvroValue::Bytes(value) | AvroValue::Fixed(_, value) => {
            Ok(Value::Bytes(Bytes::new(value.clone())))
        }
        AvroValue::String(value) => Ok(Value::String(value.clone())),
        AvroValue::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| from_avro(value).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(Value::Seq),
        AvroValue::Map(values) => values
            .iter()
            .map(|(key, value)| {
                from_avro(value)
                    .map(|value| (key.clone(), value))
                    .map_err(|err| err.at_field(key.clone()))
            })
            .collect::<Result<BTreeMap<_, _>>>()
            .map(Value::Record),
        AvroValue::Record(values) => values
            .iter()
            .map(|(key, value)| {
                from_avro(value)
                    .map(|value| (key.clone(), value))
                    .map_err(|err| err.at_field(key.clone()))
            })
            .collect::<Result<BTreeMap<_, _>>>()
            .map(Value::Record),
        AvroValue::Enum(_, value) => Ok(Value::Enum {
            variant: value.clone(),
            payload: None,
        }),
        AvroValue::Union(_, value) => from_avro(value),
        AvroValue::Decimal(value) => Ok(Value::String(format!("{value:?}"))),
        other => Err(DataError::unsupported(format!(
            "Avro value {other:?} is not mapped yet"
        ))),
    }
}
