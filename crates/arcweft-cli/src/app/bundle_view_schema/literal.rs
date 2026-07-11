//! Exact source-literal conversion for typed View value programs.

use arcweft_lang_syntax::expr::{DurationUnit, Expr, Literal, UnitNumberSuffix};
use arcweft_presentation::fx::{
    Angle, FiniteF32, FxRuntimeType, FxRuntimeValue, Length, Seconds, ValueInstruction,
};

use super::ViewValueCompileError;

pub(super) fn emit_literal(
    literal: &Literal,
    expected: Option<FxRuntimeType>,
    instructions: &mut Vec<ValueInstruction>,
) -> Result<FxRuntimeType, ViewValueCompileError> {
    let (value, value_type) = literal_value(literal, expected)?;
    instructions.push(ValueInstruction::Constant { value });
    Ok(value_type)
}

pub(super) fn infer_literal_type(expression: &Expr) -> Option<FxRuntimeType> {
    match expression {
        Expr::Literal(Literal::Bool(_)) => Some(FxRuntimeType::Bool),
        Expr::Literal(Literal::Int(_)) => Some(FxRuntimeType::I32),
        Expr::Literal(Literal::Float { .. }) => Some(FxRuntimeType::F32),
        Expr::Literal(Literal::UnitNumber { suffix, .. }) => match suffix {
            UnitNumberSuffix::Percent => Some(FxRuntimeType::F32),
            UnitNumberSuffix::Px => Some(FxRuntimeType::Length),
            UnitNumberSuffix::Deg | UnitNumberSuffix::Rad | UnitNumberSuffix::Turn => {
                Some(FxRuntimeType::Angle)
            }
            _ => None,
        },
        Expr::Literal(Literal::Duration { .. }) => Some(FxRuntimeType::Seconds),
        _ => None,
    }
}

fn literal_value(
    literal: &Literal,
    expected: Option<FxRuntimeType>,
) -> Result<(FxRuntimeValue, FxRuntimeType), ViewValueCompileError> {
    match literal {
        Literal::Bool(value) => {
            require_literal_type(literal, expected, FxRuntimeType::Bool)?;
            Ok((FxRuntimeValue::Bool(*value), FxRuntimeType::Bool))
        }
        Literal::Int(value) => {
            let magnitude = value.magnitude().map_err(|error| {
                invalid_literal(
                    value.raw(),
                    expected.unwrap_or(FxRuntimeType::I32),
                    error.to_string(),
                )
            })?;
            let expected = expected.unwrap_or(FxRuntimeType::I32);
            integer_literal_value(value.raw(), magnitude, expected)
        }
        Literal::Float { raw, .. } => {
            let expected = expected.unwrap_or(FxRuntimeType::F32);
            let source = raw
                .trim_end_matches("f32")
                .trim_end_matches("f64")
                .replace('_', "");
            let value = source
                .parse::<f64>()
                .map_err(|error| invalid_literal(raw, expected, error.to_string()))?;
            numeric_value(raw, value, expected)
        }
        Literal::UnitNumber { raw, suffix } => unit_literal_value(literal, raw, *suffix, expected),
        Literal::Duration { amount, unit } => {
            duration_literal_value(literal, amount, *unit, expected)
        }
        Literal::String(value) => Err(ViewValueCompileError::UnsupportedExpression {
            expression: format!("string literal {value:?}"),
        }),
        Literal::Char { value, .. } => Err(ViewValueCompileError::UnsupportedExpression {
            expression: format!("character literal {value:?}"),
        }),
    }
}

fn integer_literal_value(
    literal: &str,
    magnitude: u128,
    expected: FxRuntimeType,
) -> Result<(FxRuntimeValue, FxRuntimeType), ViewValueCompileError> {
    if expected == FxRuntimeType::I32 {
        let value = i32::try_from(magnitude).map_err(|_| {
            invalid_literal(
                literal,
                expected,
                "value is outside the exact i32 domain".to_owned(),
            )
        })?;
        return Ok((FxRuntimeValue::I32(value), FxRuntimeType::I32));
    }
    let value = magnitude
        .to_string()
        .parse::<f64>()
        .map_err(|error| invalid_literal(literal, expected, error.to_string()))?;
    numeric_value(literal, value, expected)
}

fn unit_literal_value(
    literal: &Literal,
    raw: &str,
    suffix: UnitNumberSuffix,
    expected: Option<FxRuntimeType>,
) -> Result<(FxRuntimeValue, FxRuntimeType), ViewValueCompileError> {
    let source = raw
        .strip_suffix(suffix.as_str())
        .unwrap_or(raw)
        .replace('_', "");
    let value = source.parse::<f64>().map_err(|error| {
        invalid_literal(
            raw,
            expected.unwrap_or(FxRuntimeType::F32),
            error.to_string(),
        )
    })?;
    let (value, value_type) =
        match suffix {
            UnitNumberSuffix::Percent => (
                FxRuntimeValue::F32(finite(raw, value / 100.0, FxRuntimeType::F32)?),
                FxRuntimeType::F32,
            ),
            UnitNumberSuffix::Px => (
                FxRuntimeValue::Length(Length::try_pixels_f64(value).map_err(|error| {
                    invalid_literal(raw, FxRuntimeType::Length, error.to_string())
                })?),
                FxRuntimeType::Length,
            ),
            UnitNumberSuffix::Deg => (
                FxRuntimeValue::Angle(Angle::try_degrees(value).map_err(|error| {
                    invalid_literal(raw, FxRuntimeType::Angle, error.to_string())
                })?),
                FxRuntimeType::Angle,
            ),
            UnitNumberSuffix::Rad => (
                FxRuntimeValue::Angle(
                    Angle::try_radians(finite(raw, value, FxRuntimeType::Angle)?.get()).map_err(
                        |error| invalid_literal(raw, FxRuntimeType::Angle, error.to_string()),
                    )?,
                ),
                FxRuntimeType::Angle,
            ),
            UnitNumberSuffix::Turn => (
                FxRuntimeValue::Angle(Angle::try_turns(value).map_err(|error| {
                    invalid_literal(raw, FxRuntimeType::Angle, error.to_string())
                })?),
                FxRuntimeType::Angle,
            ),
            _ => {
                return Err(invalid_literal(
                    raw,
                    expected.unwrap_or(FxRuntimeType::F32),
                    format!("unit `{suffix}` is not part of View scalar programs"),
                ));
            }
        };
    require_literal_type(literal, expected, value_type)?;
    Ok((value, value_type))
}

fn duration_literal_value(
    literal: &Literal,
    amount: &str,
    unit: DurationUnit,
    expected: Option<FxRuntimeType>,
) -> Result<(FxRuntimeValue, FxRuntimeType), ViewValueCompileError> {
    require_literal_type(literal, expected, FxRuntimeType::Seconds)?;
    let value = amount
        .replace('_', "")
        .parse::<f64>()
        .map_err(|error| invalid_literal(amount, FxRuntimeType::Seconds, error.to_string()))?;
    let seconds = match unit {
        DurationUnit::Nanos => value / 1_000_000_000.0,
        DurationUnit::Micros => value / 1_000_000.0,
        DurationUnit::Millis => value / 1_000.0,
        DurationUnit::Seconds => value,
        DurationUnit::Minutes => value * 60.0,
        DurationUnit::Hours => value * 3_600.0,
    };
    Ok((
        FxRuntimeValue::Seconds(
            Seconds::try_seconds_f64(seconds).map_err(|error| {
                invalid_literal(amount, FxRuntimeType::Seconds, error.to_string())
            })?,
        ),
        FxRuntimeType::Seconds,
    ))
}

fn numeric_value(
    literal: &str,
    value: f64,
    expected: FxRuntimeType,
) -> Result<(FxRuntimeValue, FxRuntimeType), ViewValueCompileError> {
    let runtime = match expected {
        FxRuntimeType::I32 => {
            if value.fract() != 0.0 || value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
                return Err(invalid_literal(
                    literal,
                    expected,
                    "value is outside the exact i32 domain".to_owned(),
                ));
            }
            #[expect(
                clippy::cast_possible_truncation,
                reason = "range and integral value are checked immediately above"
            )]
            FxRuntimeValue::I32(value as i32)
        }
        FxRuntimeType::F32 => FxRuntimeValue::F32(finite(literal, value, expected)?),
        FxRuntimeType::Length => FxRuntimeValue::Length(
            Length::try_pixels_f64(value)
                .map_err(|error| invalid_literal(literal, expected, error.to_string()))?,
        ),
        FxRuntimeType::Angle => FxRuntimeValue::Angle(
            Angle::try_radians(finite(literal, value, expected)?.get())
                .map_err(|error| invalid_literal(literal, expected, error.to_string()))?,
        ),
        FxRuntimeType::Seconds => FxRuntimeValue::Seconds(
            Seconds::try_seconds_f64(value)
                .map_err(|error| invalid_literal(literal, expected, error.to_string()))?,
        ),
        _ => {
            return Err(invalid_literal(
                literal,
                expected,
                "numeric literal cannot produce this runtime type".to_owned(),
            ));
        }
    };
    Ok((runtime, expected))
}

fn finite(
    literal: &str,
    value: f64,
    expected: FxRuntimeType,
) -> Result<FiniteF32, ViewValueCompileError> {
    FiniteF32::try_from_f64(value)
        .map_err(|error| invalid_literal(literal, expected, error.to_string()))
}

fn require_literal_type(
    literal: &Literal,
    expected: Option<FxRuntimeType>,
    actual: FxRuntimeType,
) -> Result<(), ViewValueCompileError> {
    if expected.is_none_or(|expected| expected == actual) {
        Ok(())
    } else {
        Err(invalid_literal(
            &format!("{literal:?}"),
            expected.unwrap(),
            format!("literal has type {actual:?}"),
        ))
    }
}

fn invalid_literal(
    literal: &str,
    expected: FxRuntimeType,
    reason: String,
) -> ViewValueCompileError {
    ViewValueCompileError::InvalidLiteral {
        literal: literal.to_owned(),
        expected,
        reason,
    }
}
