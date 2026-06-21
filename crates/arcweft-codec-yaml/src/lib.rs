#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use arcweft_data::{
    BytesFormat, Codec, DataError, DataErrorKind, DecodeOptions, EncodeOptions, FormatId, Number,
    Result, TypeShape, Value,
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
        _shape: &TypeShape,
        options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let yaml = to_yaml(value, options.bytes_format)?;
        let mut out = String::new();
        YamlEmitter::new(&mut out)
            .dump(&yaml)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        Ok(out.into_bytes())
    }

    fn decode_value(
        &self,
        input: &[u8],
        _shape: &TypeShape,
        options: &DecodeOptions,
    ) -> Result<Value> {
        let source = std::str::from_utf8(input)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        let documents = YamlLoader::load_from_str(source)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        let document = documents
            .first()
            .ok_or_else(|| DataError::new(DataErrorKind::MissingField, "YAML document is empty"))?;
        let value = from_yaml(document)?;
        options.limits.validate(&value)?;
        Ok(value)
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
        Value::Bytes(bytes) => match bytes_format {
            BytesFormat::Array => Ok(Yaml::Array(
                bytes
                    .as_slice()
                    .iter()
                    .map(|byte| Yaml::Integer(i64::from(*byte)))
                    .collect(),
            )),
            _ => Ok(Yaml::String(BASE64_STANDARD.encode(bytes.as_slice()))),
        },
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
