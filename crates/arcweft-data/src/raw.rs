use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Bytes, DataError, DataErrorKind, Number, Result,
    shape::{FieldShape, TypeShape, VariantShape},
    value::Value,
};

/// Format-capability-preserving syntax value used between concrete codecs and
/// Arcweft's typed `Value`.
#[derive(Clone, Debug, PartialEq)]
pub enum RawValue {
    Null,
    Bool(bool),
    Signed(i128),
    Unsigned(u128),
    F32(f32),
    F64(f64),
    String(String),
    Bytes(Vec<u8>),
    Seq(Vec<RawValue>),
    Map(Vec<(RawValue, RawValue)>),
}

impl RawValue {
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "bool",
            Self::Signed(_) => "signed integer",
            Self::Unsigned(_) => "unsigned integer",
            Self::F32(_) => "f32",
            Self::F64(_) => "f64",
            Self::String(_) => "string",
            Self::Bytes(_) => "bytes",
            Self::Seq(_) => "sequence",
            Self::Map(_) => "map",
        }
    }
}

/// Validates and projects a typed value into a raw value according to shape.
pub fn encode_with_shape(value: &Value, shape: &TypeShape) -> Result<RawValue> {
    match shape {
        TypeShape::Unit => match value {
            Value::Unit => Ok(RawValue::Null),
            other => Err(DataError::invalid_type("unit", other.type_name())),
        },
        TypeShape::Bool => match value {
            Value::Bool(value) => Ok(RawValue::Bool(*value)),
            other => Err(DataError::invalid_type("bool", other.type_name())),
        },
        TypeShape::String => match value {
            Value::String(value) => Ok(RawValue::String(value.clone())),
            other => Err(DataError::invalid_type("string", other.type_name())),
        },
        TypeShape::Char => match value {
            Value::Char(value) => Ok(RawValue::String(value.to_string())),
            other => Err(DataError::invalid_type("char", other.type_name())),
        },
        TypeShape::Bytes { .. } => match value {
            Value::Bytes(bytes) => Ok(RawValue::Bytes(bytes.as_slice().to_vec())),
            other => Err(DataError::invalid_type("bytes", other.type_name())),
        },
        TypeShape::Option(inner) => match value {
            Value::Unit => Ok(RawValue::Null),
            other => encode_with_shape(other, inner),
        },
        TypeShape::Seq(inner) => match value {
            Value::Seq(values) => values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    encode_with_shape(value, inner).map_err(|error| error.at_index(index))
                })
                .collect::<Result<Vec<_>>>()
                .map(RawValue::Seq),
            other => Err(DataError::invalid_type("sequence", other.type_name())),
        },
        TypeShape::Map { key, value: inner } => {
            if !matches!(key.as_ref(), TypeShape::String) {
                return Err(DataError::unsupported(
                    "arcweft-data v1 raw transcoder supports string map keys only",
                ));
            }
            match value {
                Value::Map(values) => values
                    .iter()
                    .map(|(key, value)| {
                        encode_with_shape(value, inner)
                            .map(|raw| (RawValue::String(key.clone()), raw))
                            .map_err(|error| error.at_field(key.clone()))
                    })
                    .collect::<Result<Vec<_>>>()
                    .map(RawValue::Map),
                other => Err(DataError::invalid_type("map", other.type_name())),
            }
        }
        TypeShape::Record { fields, policy, .. } => encode_record(value, fields, *policy),
        TypeShape::Enum { variants, .. } => encode_enum(value, variants),
        TypeShape::Named(name) => Err(DataError::unsupported(format!(
            "named shape `{name}` must be resolved before raw transcoding"
        ))),
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
        | TypeShape::Usize
        | TypeShape::F32
        | TypeShape::F64 => encode_number(value, shape),
    }
}

/// Validates and projects a raw value into Arcweft's typed value according to shape.
pub fn decode_with_shape(raw: &RawValue, shape: &TypeShape) -> Result<Value> {
    match shape {
        TypeShape::Unit => match raw {
            RawValue::Null => Ok(Value::Unit),
            other => Err(DataError::invalid_type("unit", other.type_name())),
        },
        TypeShape::Bool => match raw {
            RawValue::Bool(value) => Ok(Value::Bool(*value)),
            other => Err(DataError::invalid_type("bool", other.type_name())),
        },
        TypeShape::String => match raw {
            RawValue::String(value) => Ok(Value::String(value.clone())),
            other => Err(DataError::invalid_type("string", other.type_name())),
        },
        TypeShape::Char => match raw {
            RawValue::String(value) => {
                let mut chars = value.chars();
                let Some(ch) = chars.next() else {
                    return Err(DataError::invalid_type(
                        "single char string",
                        "empty string",
                    ));
                };
                if chars.next().is_some() {
                    return Err(DataError::invalid_type(
                        "single char string",
                        "multi-char string",
                    ));
                }
                Ok(Value::Char(ch))
            }
            other => Err(DataError::invalid_type("char", other.type_name())),
        },
        TypeShape::Bytes { .. } => match raw {
            RawValue::Bytes(bytes) => Ok(Value::Bytes(Bytes::new(bytes.clone()))),
            other => Err(DataError::invalid_type("bytes", other.type_name())),
        },
        TypeShape::Option(inner) => match raw {
            RawValue::Null => Ok(Value::Unit),
            other => decode_with_shape(other, inner),
        },
        TypeShape::Seq(inner) => match raw {
            RawValue::Seq(values) => values
                .iter()
                .enumerate()
                .map(|(index, raw)| {
                    decode_with_shape(raw, inner).map_err(|error| error.at_index(index))
                })
                .collect::<Result<Vec<_>>>()
                .map(Value::Seq),
            other => Err(DataError::invalid_type("sequence", other.type_name())),
        },
        TypeShape::Map { key, value } => {
            if !matches!(key.as_ref(), TypeShape::String) {
                return Err(DataError::unsupported(
                    "arcweft-data v1 raw transcoder supports string map keys only",
                ));
            }
            match raw {
                RawValue::Map(entries) => entries
                    .iter()
                    .map(|(key, raw_value)| {
                        let RawValue::String(key) = key else {
                            return Err(DataError::invalid_type("string map key", key.type_name()));
                        };
                        decode_with_shape(raw_value, value)
                            .map(|value| (key.clone(), value))
                            .map_err(|error| error.at_field(key.clone()))
                    })
                    .collect::<Result<BTreeMap<_, _>>>()
                    .map(Value::Map),
                other => Err(DataError::invalid_type("map", other.type_name())),
            }
        }
        TypeShape::Record { fields, policy, .. } => decode_record(raw, fields, *policy),
        TypeShape::Enum { variants, .. } => decode_enum(raw, variants),
        TypeShape::Named(name) => Err(DataError::unsupported(format!(
            "named shape `{name}` must be resolved before raw transcoding"
        ))),
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
        | TypeShape::Usize
        | TypeShape::F32
        | TypeShape::F64 => decode_number(raw, shape),
    }
}

fn encode_record(
    value: &Value,
    fields: &[FieldShape],
    policy: crate::shape::RecordPolicy,
) -> Result<RawValue> {
    let Value::Record(values) = value else {
        return Err(DataError::invalid_type("record", value.type_name()));
    };
    if policy.deny_unknown_fields {
        let known = fields
            .iter()
            .map(|field| field.wire_name.as_str())
            .collect::<BTreeSet<_>>();
        if let Some(unknown) = values.keys().find(|key| !known.contains(key.as_str())) {
            return Err(DataError::new(
                DataErrorKind::UnknownField,
                format!("unknown record field `{unknown}`"),
            )
            .at_field(unknown.clone()));
        }
    }
    fields
        .iter()
        .filter(|field| !field.skip)
        .map(|field| {
            let Some(value) = values.get(&field.wire_name) else {
                return Err(DataError::new(
                    DataErrorKind::MissingField,
                    format!("missing record field `{}`", field.wire_name),
                )
                .at_field(field.wire_name.clone()));
            };
            encode_with_shape(value, &field.value_shape())
                .map(|raw| (RawValue::String(field.wire_name.clone()), raw))
                .map_err(|error| error.at_field(field.wire_name.clone()))
        })
        .collect::<Result<Vec<_>>>()
        .map(RawValue::Map)
}

fn decode_record(
    raw: &RawValue,
    fields: &[FieldShape],
    policy: crate::shape::RecordPolicy,
) -> Result<Value> {
    let RawValue::Map(entries) = raw else {
        return Err(DataError::invalid_type("record map", raw.type_name()));
    };
    let raw_fields = string_map(entries)?;
    let known = fields
        .iter()
        .map(|field| field.wire_name.as_str())
        .collect::<BTreeSet<_>>();
    if policy.deny_unknown_fields
        && let Some(unknown) = raw_fields.keys().find(|key| !known.contains(key.as_str()))
    {
        return Err(DataError::new(
            DataErrorKind::UnknownField,
            format!("unknown record field `{unknown}`"),
        )
        .at_field(unknown.clone()));
    }
    fields
        .iter()
        .filter(|field| !field.skip)
        .map(|field| {
            let shape = field.value_shape();
            let Some(raw) = raw_fields.get(&field.wire_name) else {
                return match shape {
                    TypeShape::Option(_) => Ok((field.wire_name.clone(), Value::Unit)),
                    _ => Err(DataError::new(
                        DataErrorKind::MissingField,
                        format!("missing record field `{}`", field.wire_name),
                    )
                    .at_field(field.wire_name.clone())),
                };
            };
            decode_with_shape(raw, &shape)
                .map(|value| (field.wire_name.clone(), value))
                .map_err(|error| error.at_field(field.wire_name.clone()))
        })
        .collect::<Result<BTreeMap<_, _>>>()
        .map(Value::Record)
}

fn encode_enum(value: &Value, variants: &[VariantShape]) -> Result<RawValue> {
    let Value::Enum { variant, payload } = value else {
        return Err(DataError::invalid_type("enum", value.type_name()));
    };
    let shape = variants
        .iter()
        .find(|shape| shape.wire_name == *variant)
        .ok_or_else(|| {
            DataError::new(
                DataErrorKind::InvalidEnumTag,
                format!("unknown enum variant `{variant}`"),
            )
        })?;
    let mut entries = vec![(
        RawValue::String("variant".to_owned()),
        RawValue::String(variant.clone()),
    )];
    match (&shape.payload, payload) {
        (Some(shape), Some(payload)) => {
            entries.push((
                RawValue::String("payload".to_owned()),
                encode_with_shape(payload, shape).map_err(|error| error.at_variant(variant))?,
            ));
        }
        (None, None) => {}
        (Some(_), None) => {
            return Err(DataError::new(
                DataErrorKind::MissingField,
                format!("missing payload for enum variant `{variant}`"),
            )
            .at_variant(variant));
        }
        (None, Some(_)) => {
            return Err(DataError::invalid_type("unit enum variant", "payload").at_variant(variant));
        }
    }
    Ok(RawValue::Map(entries))
}

fn decode_enum(raw: &RawValue, variants: &[VariantShape]) -> Result<Value> {
    let RawValue::Map(entries) = raw else {
        return Err(DataError::invalid_type("enum map", raw.type_name()));
    };
    let fields = string_map(entries)?;
    let variant = match fields.get("variant") {
        Some(RawValue::String(value)) => value,
        Some(other) => {
            return Err(DataError::invalid_type(
                "enum variant string",
                other.type_name(),
            ));
        }
        None => {
            return Err(DataError::new(
                DataErrorKind::MissingField,
                "missing enum variant field",
            ));
        }
    };
    let shape = variants
        .iter()
        .find(|shape| shape.wire_name == *variant)
        .ok_or_else(|| {
            DataError::new(
                DataErrorKind::InvalidEnumTag,
                format!("unknown enum variant `{variant}`"),
            )
        })?;
    let payload = match (&shape.payload, fields.get("payload")) {
        (Some(shape), Some(raw)) => Some(Box::new(
            decode_with_shape(raw, shape).map_err(|error| error.at_variant(variant))?,
        )),
        (None, None) => None,
        (Some(_), None) => {
            return Err(DataError::new(
                DataErrorKind::MissingField,
                format!("missing payload for enum variant `{variant}`"),
            )
            .at_variant(variant));
        }
        (None, Some(_)) => {
            return Err(DataError::invalid_type("unit enum variant", "payload").at_variant(variant));
        }
    };
    Ok(Value::Enum {
        variant: variant.clone(),
        payload,
    })
}

fn string_map(entries: &[(RawValue, RawValue)]) -> Result<BTreeMap<String, &RawValue>> {
    let mut out = BTreeMap::new();
    for (key, value) in entries {
        let RawValue::String(key) = key else {
            return Err(DataError::invalid_type("string map key", key.type_name()));
        };
        if out.insert(key.clone(), value).is_some() {
            return Err(DataError::new(
                DataErrorKind::DuplicateField,
                format!("duplicate map key `{key}`"),
            )
            .at_field(key));
        }
    }
    Ok(out)
}

fn encode_number(value: &Value, shape: &TypeShape) -> Result<RawValue> {
    let Value::Number(number) = value else {
        return Err(DataError::invalid_type("number", value.type_name()));
    };
    match (shape, number) {
        (TypeShape::F32, Number::F32(value)) => Ok(RawValue::F32(*value)),
        (TypeShape::F64, Number::F64(value)) => Ok(RawValue::F64(*value)),
        (TypeShape::F32 | TypeShape::F64, _) => Err(DataError::invalid_type(
            shape.type_name(),
            number.type_name(),
        )),
        (shape, Number::I(value))
            if shape
                .signed_bounds()
                .is_some_and(|(min, max)| *value >= min && *value <= max) =>
        {
            Ok(RawValue::Signed(*value))
        }
        (shape, Number::U(value)) if shape.unsigned_max().is_some_and(|max| *value <= max) => {
            Ok(RawValue::Unsigned(*value))
        }
        (shape, Number::I(_) | Number::U(_)) => Err(DataError::new(
            DataErrorKind::NumberOutOfRange,
            format!("number is out of range for {}", shape.type_name()),
        )),
        (_, Number::F32(_) | Number::F64(_)) => {
            Err(DataError::invalid_type("integer", number.type_name()))
        }
    }
}

fn decode_number(raw: &RawValue, shape: &TypeShape) -> Result<Value> {
    match (shape, raw) {
        (TypeShape::F32, RawValue::F32(value)) => Ok(Value::Number(Number::F32(*value))),
        (TypeShape::F64, RawValue::F64(value)) => Ok(Value::Number(Number::F64(*value))),
        (shape, RawValue::Signed(value))
            if shape
                .signed_bounds()
                .is_some_and(|(min, max)| *value >= min && *value <= max) =>
        {
            Ok(Value::Number(Number::I(*value)))
        }
        (shape, RawValue::Unsigned(value))
            if shape.unsigned_max().is_some_and(|max| *value <= max) =>
        {
            Ok(Value::Number(Number::U(*value)))
        }
        (TypeShape::F32 | TypeShape::F64, other) => Err(DataError::invalid_type(
            shape.type_name(),
            other.type_name(),
        )),
        (_, RawValue::Signed(_) | RawValue::Unsigned(_)) => Err(DataError::new(
            DataErrorKind::NumberOutOfRange,
            format!("number is out of range for {}", shape.type_name()),
        )),
        (_, other) => Err(DataError::invalid_type("number", other.type_name())),
    }
}
