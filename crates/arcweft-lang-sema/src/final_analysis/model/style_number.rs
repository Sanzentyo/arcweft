//! Exact source numeric operands, including the magnitude of signed minima.

use arcweft_lang_hir::leaf::{
    HirFloatLiteral, HirIntegerLiteral, HirLiteral, HirUnitNumberLiteral, HirUnitNumberUnit,
};
use arcweft_view::style::{
    ViewAngleMilliDegrees, ViewFontWeight, ViewLengthMilli, ViewRatioMilli, ViewScalarMilli,
    ViewSpecifiedValue,
};
use thiserror::Error;

use crate::types::StyleNumericType;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CheckedStyleNumberError {
    #[error("literal does not belong to the specified Style numeric grammar")]
    WrongLiteral,
    #[error("numeric literal is outside the native Style value range")]
    OutOfRange,
    #[error(transparent)]
    Decimal(#[from] arcweft_lang_hir::leaf::HirDecimalScaleError),
}

/// A source construction magnitude, never a standalone property/runtime value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedStyleNumber {
    kind: StyleNumericType,
    magnitude: u32,
}

impl TryFrom<(StyleNumericType, &HirLiteral)> for CheckedStyleNumber {
    type Error = CheckedStyleNumberError;

    fn try_from((kind, literal): (StyleNumericType, &HirLiteral)) -> Result<Self, Self::Error> {
        use CheckedStyleNumberError::{OutOfRange, WrongLiteral};
        let magnitude = match (kind, literal) {
            (
                StyleNumericType::Integer
                | StyleNumericType::FontWeight
                | StyleNumericType::Ratio
                | StyleNumericType::Scalar,
                HirLiteral::Integer(HirIntegerLiteral::Value {
                    magnitude,
                    suffix: None,
                    ..
                }),
            ) => {
                let value = match magnitude.limbs_le() {
                    [] => 0,
                    [value] => *value,
                    _ => return Err(OutOfRange),
                };
                if matches!(kind, StyleNumericType::Ratio | StyleNumericType::Scalar) {
                    value.checked_mul(1000).ok_or(OutOfRange)?
                } else {
                    value
                }
            }
            (
                StyleNumericType::Ratio | StyleNumericType::Scalar,
                HirLiteral::Float(HirFloatLiteral::Value {
                    decimal,
                    explicit_width: None,
                }),
            ) => u32::try_from(decimal.to_scaled_i64(3, 1, false)?).map_err(|_| OutOfRange)?,
            (
                StyleNumericType::Ratio,
                HirLiteral::UnitNumber(HirUnitNumberLiteral::Value {
                    decimal,
                    unit: HirUnitNumberUnit::Percent,
                }),
            ) => u32::try_from(decimal.to_scaled_i64(1, 1, false)?).map_err(|_| OutOfRange)?,
            (
                StyleNumericType::Length,
                HirLiteral::UnitNumber(HirUnitNumberLiteral::Value {
                    decimal,
                    unit: HirUnitNumberUnit::Px,
                }),
            ) => u32::try_from(decimal.to_scaled_i64(3, 1, false)?).map_err(|_| OutOfRange)?,
            (
                StyleNumericType::Angle,
                HirLiteral::UnitNumber(HirUnitNumberLiteral::Value {
                    decimal,
                    unit: unit @ (HirUnitNumberUnit::Deg | HirUnitNumberUnit::Turn),
                }),
            ) => u32::try_from(decimal.to_scaled_i64(
                3,
                if *unit == HirUnitNumberUnit::Turn {
                    360
                } else {
                    1
                },
                false,
            )?)
            .map_err(|_| OutOfRange)?,
            (
                StyleNumericType::Angle,
                HirLiteral::UnitNumber(HirUnitNumberLiteral::Value {
                    decimal,
                    unit: HirUnitNumberUnit::Rad,
                }),
            ) => {
                let radians = decimal.to_finite_f64().ok_or(OutOfRange)?;
                // The negative conversion also admits the magnitude of i32::MIN.
                arcweft_primitives::CheckedAngle::try_from_radians(-radians)
                    .ok_or(OutOfRange)?
                    .milli_degrees
                    .unsigned_abs()
            }
            _ => return Err(WrongLiteral),
        };
        if magnitude > kind.maximum_magnitude() {
            return Err(OutOfRange);
        }
        Ok(Self { kind, magnitude })
    }
}

impl CheckedStyleNumber {
    pub const fn kind(self) -> StyleNumericType {
        self.kind
    }
    pub const fn magnitude(self) -> u32 {
        self.magnitude
    }

    pub fn specified_value(
        self,
        negative: bool,
    ) -> Result<ViewSpecifiedValue, CheckedStyleNumberError> {
        use CheckedStyleNumberError::OutOfRange;
        let signed = i64::from(self.magnitude) * if negative { -1 } else { 1 };
        Ok(match self.kind {
            StyleNumericType::Integer => ViewSpecifiedValue::Integer {
                value: i32::try_from(signed).map_err(|_| OutOfRange)?,
            },
            StyleNumericType::Length => ViewSpecifiedValue::Length {
                value: ViewLengthMilli::new(i32::try_from(signed).map_err(|_| OutOfRange)?),
            },
            StyleNumericType::Angle => ViewSpecifiedValue::Angle {
                value: ViewAngleMilliDegrees::new(i32::try_from(signed).map_err(|_| OutOfRange)?),
            },
            StyleNumericType::Ratio => ViewSpecifiedValue::Ratio {
                value: ViewRatioMilli::new(u16::try_from(signed).map_err(|_| OutOfRange)?)
                    .ok_or(OutOfRange)?,
            },
            StyleNumericType::Scalar => ViewSpecifiedValue::Scalar {
                value: ViewScalarMilli::new(u32::try_from(signed).map_err(|_| OutOfRange)?),
            },
            StyleNumericType::FontWeight => ViewSpecifiedValue::FontWeight {
                value: ViewFontWeight::new(u16::try_from(signed).map_err(|_| OutOfRange)?)
                    .ok_or(OutOfRange)?,
            },
        })
    }
}
