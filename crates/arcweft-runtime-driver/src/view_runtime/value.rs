use arcweft_core::value::{RuntimeFieldValue, RuntimeValue};
use arcweft_presentation::fx::{
    Angle, FiniteF32, FxColor, FxRuntimeType, FxRuntimeValue, FxVec2, Length, Opacity, Seconds,
    Transform2D,
};
use std::collections::BTreeMap;
use thiserror::Error;

/// Strict failure to cross from general runtime values into the closed View/Fx scalar domain.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum BundleViewValueConversionError {
    #[error("runtime value has type {actual}, expected {expected:?}")]
    Type {
        expected: FxRuntimeType,
        actual: &'static str,
    },
    #[error("runtime record repeats field `{field}`")]
    DuplicateField { field: String },
    #[error("runtime record is missing field `{field}`")]
    MissingField { field: &'static str },
    #[error("runtime record has unexpected field `{field}`")]
    UnexpectedField { field: String },
    #[error("runtime number for {field} is invalid: {message}")]
    InvalidNumber {
        field: &'static str,
        message: String,
    },
    #[error("transform is invalid: {message}")]
    InvalidTransform { message: String },
}

pub(super) fn fx_placeholder(value_type: FxRuntimeType) -> FxRuntimeValue {
    match value_type {
        FxRuntimeType::Bool => FxRuntimeValue::Bool(false),
        FxRuntimeType::I32 => FxRuntimeValue::I32(0),
        FxRuntimeType::F32 => FxRuntimeValue::F32(FiniteF32::ZERO),
        FxRuntimeType::Length => FxRuntimeValue::Length(Length::ZERO),
        FxRuntimeType::Angle => FxRuntimeValue::Angle(Angle::ZERO),
        FxRuntimeType::Seconds => FxRuntimeValue::Seconds(Seconds::ZERO),
        FxRuntimeType::Color => FxRuntimeValue::Color(FxColor::TRANSPARENT),
        FxRuntimeType::Vec2 => FxRuntimeValue::Vec2(FxVec2 {
            x: FiniteF32::ZERO,
            y: FiniteF32::ZERO,
        }),
        FxRuntimeType::Transform2D => FxRuntimeValue::Transform2D(Transform2D::default()),
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the exhaustive closed-type conversion keeps every accepted runtime shape and unit boundary auditable in one match"
)]
pub(super) fn runtime_to_fx(
    value: &RuntimeValue,
    expected: FxRuntimeType,
) -> Result<FxRuntimeValue, BundleViewValueConversionError> {
    match expected {
        FxRuntimeType::Bool => match value {
            RuntimeValue::Bool(value) => Ok(FxRuntimeValue::Bool(*value)),
            _ => Err(type_error(expected, value)),
        },
        FxRuntimeType::I32 => match value {
            RuntimeValue::Int(value) => value.exact_i32().map(FxRuntimeValue::I32).ok_or(
                BundleViewValueConversionError::Type {
                    expected,
                    actual: signed_int_type_name(*value),
                },
            ),
            _ => Err(type_error(expected, value)),
        },
        FxRuntimeType::F32 => match value {
            RuntimeValue::F32(value) => finite(*value, "f32").map(FxRuntimeValue::F32),
            _ => Err(type_error(expected, value)),
        },
        FxRuntimeType::Length => {
            let fields = exact_record(value, expected, &["px"])?;
            finite_field(&fields, "px")
                .map(|value| Length::try_pixels(value.get()))
                .and_then(|value| {
                    value
                        .map(FxRuntimeValue::Length)
                        .map_err(|error| invalid_number("px", &error))
                })
        }
        FxRuntimeType::Angle => {
            let fields = exact_record(value, expected, &["rad"])?;
            finite_field(&fields, "rad")
                .map(|value| Angle::try_radians(value.get()))
                .and_then(|value| {
                    value
                        .map(FxRuntimeValue::Angle)
                        .map_err(|error| invalid_number("rad", &error))
                })
        }
        FxRuntimeType::Seconds => match value {
            RuntimeValue::Duration(value) => {
                Seconds::try_seconds_f64(logical_duration_seconds(value.as_nanos()))
                    .map(FxRuntimeValue::Seconds)
                    .map_err(|error| invalid_number("seconds", &error))
            }
            _ => Err(type_error(expected, value)),
        },
        FxRuntimeType::Color => {
            let fields = exact_record(value, expected, &["red", "green", "blue", "alpha"])?;
            let channel = |field| {
                finite_field(&fields, field).and_then(|value| {
                    Opacity::try_new(value).map_err(|error| {
                        BundleViewValueConversionError::InvalidNumber {
                            field,
                            message: error.to_string(),
                        }
                    })
                })
            };
            Ok(FxRuntimeValue::Color(FxColor::new(
                channel("red")?,
                channel("green")?,
                channel("blue")?,
                channel("alpha")?,
            )))
        }
        FxRuntimeType::Vec2 => {
            let fields = exact_record(value, expected, &["x", "y"])?;
            Ok(FxRuntimeValue::Vec2(FxVec2 {
                x: finite_field(&fields, "x")?,
                y: finite_field(&fields, "y")?,
            }))
        }
        FxRuntimeType::Transform2D => {
            let fields = exact_record(
                value,
                expected,
                &[
                    "translate_x",
                    "translate_y",
                    "scale_x",
                    "scale_y",
                    "skew_x",
                    "skew_y",
                    "rotation",
                    "origin_x",
                    "origin_y",
                    "opacity",
                ],
            )?;
            let length = |field| match runtime_to_fx(
                required_field(&fields, field)?,
                FxRuntimeType::Length,
            )? {
                FxRuntimeValue::Length(value) => Ok(value),
                _ => unreachable!("Length conversion returns Length"),
            };
            let angle =
                |field| match runtime_to_fx(required_field(&fields, field)?, FxRuntimeType::Angle)?
                {
                    FxRuntimeValue::Angle(value) => Ok(value),
                    _ => unreachable!("Angle conversion returns Angle"),
                };
            let value = Transform2D {
                translate_x: length("translate_x")?,
                translate_y: length("translate_y")?,
                scale_x: finite_field(&fields, "scale_x")?,
                scale_y: finite_field(&fields, "scale_y")?,
                skew_x: angle("skew_x")?,
                skew_y: angle("skew_y")?,
                rotation: angle("rotation")?,
                origin_x: length("origin_x")?,
                origin_y: length("origin_y")?,
                opacity: finite_field(&fields, "opacity")?,
            };
            value
                .validate()
                .map_err(|error| BundleViewValueConversionError::InvalidTransform {
                    message: error.to_string(),
                })?;
            Ok(FxRuntimeValue::Transform2D(value))
        }
    }
}

pub(super) fn runtime_scalar_text(value: &RuntimeValue) -> Option<String> {
    match value {
        RuntimeValue::Bool(value) => Some(value.to_string()),
        RuntimeValue::Int(value) => Some(value.label()),
        RuntimeValue::UInt(value) => Some(value.label()),
        RuntimeValue::F32(value) if value.is_finite() => Some(value.to_string()),
        RuntimeValue::F64(value) if value.is_finite() => Some(value.to_string()),
        RuntimeValue::String(value) | RuntimeValue::EntityRef(value) => Some(value.clone()),
        RuntimeValue::Char(value) => Some(value.to_string()),
        RuntimeValue::Duration(value) => Some(format_duration(value.as_nanos())),
        RuntimeValue::Unit
        | RuntimeValue::F32(_)
        | RuntimeValue::F64(_)
        | RuntimeValue::MatrixF32(_)
        | RuntimeValue::MatrixF64(_)
        | RuntimeValue::TensorF32(_)
        | RuntimeValue::TensorF64(_)
        | RuntimeValue::Range(_)
        | RuntimeValue::Iterator(_)
        | RuntimeValue::Tuple(_)
        | RuntimeValue::Seq(_)
        | RuntimeValue::Record(_)
        | RuntimeValue::NominalRecord(_)
        | RuntimeValue::Opaque(_)
        | RuntimeValue::Function(_)
        | RuntimeValue::Variant { .. } => None,
    }
}

pub(super) fn fx_scalar_text(value: FxRuntimeValue) -> String {
    match value {
        FxRuntimeValue::Bool(value) => value.to_string(),
        FxRuntimeValue::I32(value) => value.to_string(),
        FxRuntimeValue::F32(value) => value.to_string(),
        FxRuntimeValue::Length(value) => format!("{}px", value.pixels()),
        FxRuntimeValue::Angle(value) => format!("{}rad", value.radians()),
        FxRuntimeValue::Seconds(value) => format!("{}s", value.seconds()),
        FxRuntimeValue::Color(value) => format!(
            "color({}, {}, {}, {})",
            value.red().value(),
            value.green().value(),
            value.blue().value(),
            value.alpha().value()
        ),
        FxRuntimeValue::Vec2(value) => format!("vec2({}, {})", value.x, value.y),
        FxRuntimeValue::Transform2D(value) => format!(
            "transform2d({}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
            value.translate_x.pixels(),
            value.translate_y.pixels(),
            value.scale_x,
            value.scale_y,
            value.skew_x.radians(),
            value.skew_y.radians(),
            value.rotation.radians(),
            value.origin_x.pixels(),
            value.origin_y.pixels(),
            value.opacity
        ),
    }
}

pub(super) fn fx_to_runtime(
    value: FxRuntimeValue,
) -> Result<RuntimeValue, BundleViewValueConversionError> {
    Ok(match value {
        FxRuntimeValue::Bool(value) => RuntimeValue::Bool(value),
        FxRuntimeValue::I32(value) => {
            RuntimeValue::Int(arcweft_core::value::RuntimeInt::i32(value))
        }
        FxRuntimeValue::F32(value) => RuntimeValue::F32(value.get()),
        FxRuntimeValue::Length(value) => RuntimeValue::Record(vec![RuntimeFieldValue {
            name: "px".to_owned(),
            value: RuntimeValue::F32(value.pixels()),
        }]),
        FxRuntimeValue::Angle(value) => RuntimeValue::Record(vec![RuntimeFieldValue {
            name: "rad".to_owned(),
            value: RuntimeValue::F32(value.radians()),
        }]),
        FxRuntimeValue::Seconds(value) => RuntimeValue::Duration(
            arcweft_core::time::LogicalDuration::from_nanos(seconds_to_nanos(value.seconds())?),
        ),
        FxRuntimeValue::Color(value) => RuntimeValue::Record(vec![
            runtime_field("red", value.red().value().get()),
            runtime_field("green", value.green().value().get()),
            runtime_field("blue", value.blue().value().get()),
            runtime_field("alpha", value.alpha().value().get()),
        ]),
        FxRuntimeValue::Vec2(value) => RuntimeValue::Record(vec![
            runtime_field("x", value.x.get()),
            runtime_field("y", value.y.get()),
        ]),
        FxRuntimeValue::Transform2D(value) => RuntimeValue::Record(vec![
            runtime_record_field("translate_x", "px", value.translate_x.pixels()),
            runtime_record_field("translate_y", "px", value.translate_y.pixels()),
            runtime_field("scale_x", value.scale_x.get()),
            runtime_field("scale_y", value.scale_y.get()),
            runtime_record_field("skew_x", "rad", value.skew_x.radians()),
            runtime_record_field("skew_y", "rad", value.skew_y.radians()),
            runtime_record_field("rotation", "rad", value.rotation.radians()),
            runtime_record_field("origin_x", "px", value.origin_x.pixels()),
            runtime_record_field("origin_y", "px", value.origin_y.pixels()),
            runtime_field("opacity", value.opacity.get()),
        ]),
    })
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "range and sign are checked before the explicit seconds-to-logical-nanoseconds boundary"
)]
fn seconds_to_nanos(seconds: f32) -> Result<u64, BundleViewValueConversionError> {
    const MAX_LOGICAL_SECONDS: f64 = 18_446_744_073.709_553;
    if seconds < 0.0 || f64::from(seconds) > MAX_LOGICAL_SECONDS {
        return Err(BundleViewValueConversionError::InvalidNumber {
            field: "seconds",
            message: "seconds do not fit the non-negative u64 nanosecond domain".to_owned(),
        });
    }
    let nanos = f64::from(seconds) * 1_000_000_000.0;
    Ok(nanos as u64)
}

fn runtime_field(name: &str, value: f32) -> RuntimeFieldValue {
    RuntimeFieldValue {
        name: name.to_owned(),
        value: RuntimeValue::F32(value),
    }
}

fn runtime_record_field(name: &str, unit: &str, value: f32) -> RuntimeFieldValue {
    RuntimeFieldValue {
        name: name.to_owned(),
        value: RuntimeValue::Record(vec![runtime_field(unit, value)]),
    }
}

fn exact_record<'a>(
    value: &'a RuntimeValue,
    expected_type: FxRuntimeType,
    expected: &[&'static str],
) -> Result<BTreeMap<&'a str, &'a RuntimeValue>, BundleViewValueConversionError> {
    let RuntimeValue::Record(fields) = value else {
        return Err(BundleViewValueConversionError::Type {
            expected: expected_type,
            actual: runtime_type_name(value),
        });
    };
    let mut indexed = BTreeMap::new();
    for RuntimeFieldValue { name, value } in fields {
        if indexed.insert(name.as_str(), value).is_some() {
            return Err(BundleViewValueConversionError::DuplicateField {
                field: name.clone(),
            });
        }
    }
    for field in expected {
        if !indexed.contains_key(field) {
            return Err(BundleViewValueConversionError::MissingField { field });
        }
    }
    if let Some(field) = indexed.keys().find(|field| !expected.contains(field)) {
        return Err(BundleViewValueConversionError::UnexpectedField {
            field: (*field).to_owned(),
        });
    }
    Ok(indexed)
}

fn required_field<'a>(
    fields: &BTreeMap<&str, &'a RuntimeValue>,
    field: &'static str,
) -> Result<&'a RuntimeValue, BundleViewValueConversionError> {
    fields
        .get(field)
        .copied()
        .ok_or(BundleViewValueConversionError::MissingField { field })
}

fn finite_field(
    fields: &BTreeMap<&str, &RuntimeValue>,
    field: &'static str,
) -> Result<FiniteF32, BundleViewValueConversionError> {
    match required_field(fields, field)? {
        RuntimeValue::F32(value) => finite(*value, field),
        value => Err(BundleViewValueConversionError::Type {
            expected: FxRuntimeType::F32,
            actual: runtime_type_name(value),
        }),
    }
}

fn finite(value: f32, field: &'static str) -> Result<FiniteF32, BundleViewValueConversionError> {
    FiniteF32::try_new(value).map_err(|error| invalid_number(field, &error))
}

fn invalid_number(field: &'static str, error: &impl ToString) -> BundleViewValueConversionError {
    BundleViewValueConversionError::InvalidNumber {
        field,
        message: error.to_string(),
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "general logical nanoseconds intentionally cross once into the specified finite-f32 View seconds domain"
)]
fn logical_duration_seconds(nanos: u64) -> f64 {
    nanos as f64 / 1_000_000_000.0
}

fn format_duration(nanos: u64) -> String {
    let whole = nanos / 1_000_000_000;
    let fraction = nanos % 1_000_000_000;
    if fraction == 0 {
        return format!("{whole}s");
    }
    let fraction = format!("{fraction:09}");
    format!("{whole}.{}s", fraction.trim_end_matches('0'))
}

fn type_error(expected: FxRuntimeType, value: &RuntimeValue) -> BundleViewValueConversionError {
    BundleViewValueConversionError::Type {
        expected,
        actual: runtime_type_name(value),
    }
}

fn runtime_type_name(value: &RuntimeValue) -> &'static str {
    match value {
        RuntimeValue::Unit => "unit",
        RuntimeValue::Bool(_) => "bool",
        RuntimeValue::Int(value) => signed_int_type_name(*value),
        RuntimeValue::UInt(_) => "unsigned integer",
        RuntimeValue::F32(_) => "f32",
        RuntimeValue::F64(_) => "f64",
        RuntimeValue::MatrixF32(_) => "matrix<f32>",
        RuntimeValue::MatrixF64(_) => "matrix<f64>",
        RuntimeValue::TensorF32(_) => "tensor<f32>",
        RuntimeValue::TensorF64(_) => "tensor<f64>",
        RuntimeValue::String(_) => "string",
        RuntimeValue::Char(_) => "char",
        RuntimeValue::Duration(_) => "duration",
        RuntimeValue::Range(_) => "range",
        RuntimeValue::Iterator(_) => "iterator",
        RuntimeValue::EntityRef(_) => "entity_ref",
        RuntimeValue::Tuple(_) => "tuple",
        RuntimeValue::Seq(_) => "sequence",
        RuntimeValue::Record(_) => "record",
        RuntimeValue::NominalRecord(_) => "nominal record",
        RuntimeValue::Opaque(_) => "opaque",
        RuntimeValue::Function(_) => "function",
        RuntimeValue::Variant { .. } => "variant",
    }
}

fn signed_int_type_name(value: arcweft_core::value::RuntimeInt) -> &'static str {
    match value.width() {
        arcweft_core::value::RuntimeSignedIntWidth::I8 => "i8",
        arcweft_core::value::RuntimeSignedIntWidth::I16 => "i16",
        arcweft_core::value::RuntimeSignedIntWidth::I32 => "i32",
        arcweft_core::value::RuntimeSignedIntWidth::I64 => "i64",
        arcweft_core::value::RuntimeSignedIntWidth::I128 => "i128",
        arcweft_core::value::RuntimeSignedIntWidth::ISize => "isize",
    }
}
