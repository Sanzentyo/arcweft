use arcweft_lang_hir::{
    expr::{HirCallArgument, HirExprKind, HirUnaryOp},
    identity::ExprId,
    leaf::{
        HirBigUint, HirDecimal, HirDurationLiteral, HirFloatLiteral, HirIntegerLiteral, HirLiteral,
        HirStringLiteral, HirUnitNumberLiteral, HirUnitNumberUnit,
    },
    module::HirModule,
    symbol::nominal::ProjectNominalDeclarationId,
};

use crate::{
    checked_rich_text::{
        CheckedAngle, CheckedDuration, CheckedLength, LengthUnit, Milli, RatioMilli,
    },
    types::AcceptedVariantCaseSemanticId,
};

use super::{
    CheckedCompileTimeScalar, CheckedCompileTimeScalarEnum, CheckedCompileTimeScalarEnumValue,
    CheckedCompileTimeScalarKind, CompileTimeScalarReductionError,
};

const MAX_PROXY_VALUE_BYTES: usize = 4_096;

pub(crate) fn reduce_literal_expression(
    module: &HirModule,
    owner: ExprId,
    kind: &CheckedCompileTimeScalarKind,
) -> Result<CheckedCompileTimeScalar, CompileTimeScalarReductionError> {
    let expression = module
        .resolve_expr(owner)
        .map_err(|_| CompileTimeScalarReductionError::WrongHir)?;
    match expression.kind() {
        HirExprKind::Literal(literal) => reduce_literal(literal, kind, false),
        HirExprKind::Unary(unary) if unary.operator() == HirUnaryOp::Negate => {
            let operand = module
                .resolve_expr(unary.operand())
                .map_err(|_| CompileTimeScalarReductionError::WrongHir)?;
            let HirExprKind::Literal(literal) = operand.kind() else {
                return Err(CompileTimeScalarReductionError::WrongHir);
            };
            reduce_literal(literal, kind, true)
        }
        HirExprKind::Unary(_) | _ => Err(CompileTimeScalarReductionError::WrongHir),
    }
}

pub(crate) fn reduce_color_argument(
    module: &HirModule,
    owner: ExprId,
) -> Result<CheckedCompileTimeScalar, CompileTimeScalarReductionError> {
    let expression = module
        .resolve_expr(owner)
        .map_err(|_| CompileTimeScalarReductionError::WrongHir)?;
    let HirExprKind::Call(call) = expression.kind() else {
        return Err(CompileTimeScalarReductionError::WrongBuiltinCall);
    };
    let [argument] = call.arguments() else {
        return Err(CompileTimeScalarReductionError::WrongBuiltinCall);
    };
    let HirCallArgument::Positional {
        value: arcweft_lang_hir::expr::HirCallValue::Present { value },
    } = argument
    else {
        return Err(CompileTimeScalarReductionError::WrongBuiltinCall);
    };
    let argument = module
        .resolve_expr(*value)
        .map_err(|_| CompileTimeScalarReductionError::WrongHir)?;
    let HirExprKind::Literal(HirLiteral::String(HirStringLiteral::Value(value))) = argument.kind()
    else {
        return Err(CompileTimeScalarReductionError::WrongLiteral);
    };
    if value.len() > MAX_PROXY_VALUE_BYTES {
        return Err(CompileTimeScalarReductionError::OutOfRange);
    }
    crate::checked_rich_text::parse_color(value)
        .map(CheckedCompileTimeScalar::Color)
        .map_err(|_| CompileTimeScalarReductionError::WrongLiteral)
}

pub(crate) fn reduce_enum_value(
    schema: &CheckedCompileTimeScalarEnum,
    declaration: &ProjectNominalDeclarationId,
    case: AcceptedVariantCaseSemanticId,
    ordinal: u32,
) -> Result<CheckedCompileTimeScalar, CompileTimeScalarReductionError> {
    if declaration != schema.declaration() {
        return Err(CompileTimeScalarReductionError::WrongEnumDeclaration);
    }
    let accepted = schema
        .cases()
        .get(usize::try_from(ordinal).map_err(|_| CompileTimeScalarReductionError::WrongEnumCase)?)
        .filter(|accepted| accepted.ordinal() == ordinal && accepted.semantic_id() == case)
        .ok_or(CompileTimeScalarReductionError::WrongEnumCase)?;
    Ok(CheckedCompileTimeScalar::Enum(
        CheckedCompileTimeScalarEnumValue::new(
            schema.declaration().clone(),
            accepted.semantic_id(),
            ordinal,
        ),
    ))
}

fn reduce_literal(
    literal: &HirLiteral,
    kind: &CheckedCompileTimeScalarKind,
    negative: bool,
) -> Result<CheckedCompileTimeScalar, CompileTimeScalarReductionError> {
    match (literal, kind) {
        (HirLiteral::Boolean(value), CheckedCompileTimeScalarKind::Bool) if !negative => {
            Ok(CheckedCompileTimeScalar::Bool(*value))
        }
        (HirLiteral::Boolean(_), CheckedCompileTimeScalarKind::Bool) => {
            Err(CompileTimeScalarReductionError::WrongSign)
        }
        (
            HirLiteral::Integer(HirIntegerLiteral::Value { magnitude, .. }),
            CheckedCompileTimeScalarKind::Int,
        ) => signed_i64(magnitude, negative).map(CheckedCompileTimeScalar::Int),
        (
            HirLiteral::Integer(HirIntegerLiteral::Value { magnitude, .. }),
            CheckedCompileTimeScalarKind::Milli,
        ) => checked_signed_milli(
            signed_i64(magnitude, negative)?
                .checked_mul(1_000)
                .ok_or(CompileTimeScalarReductionError::Overflow)?,
        )
        .map(|value| CheckedCompileTimeScalar::Milli(Milli(value))),
        (
            HirLiteral::Float(HirFloatLiteral::Value { decimal, .. }),
            CheckedCompileTimeScalarKind::Milli,
        ) => checked_signed_milli(signed_decimal_milli(decimal, negative)?)
            .map(|value| CheckedCompileTimeScalar::Milli(Milli(value))),
        (
            HirLiteral::Integer(HirIntegerLiteral::Value { magnitude, .. }),
            CheckedCompileTimeScalarKind::Ratio,
        ) if !negative => checked_ratio(
            unsigned_i64(magnitude)?
                .checked_mul(1_000)
                .ok_or(CompileTimeScalarReductionError::Overflow)?,
        ),
        (
            HirLiteral::Float(HirFloatLiteral::Value { decimal, .. }),
            CheckedCompileTimeScalarKind::Ratio,
        ) if !negative => checked_ratio(signed_decimal_milli(decimal, false)?),
        (
            HirLiteral::Integer(HirIntegerLiteral::Value { .. })
            | HirLiteral::Float(HirFloatLiteral::Value { .. }),
            CheckedCompileTimeScalarKind::Ratio,
        ) => Err(CompileTimeScalarReductionError::WrongSign),
        (
            HirLiteral::UnitNumber(HirUnitNumberLiteral::Value { decimal, unit }),
            CheckedCompileTimeScalarKind::Length,
        ) => {
            let unit = match unit {
                HirUnitNumberUnit::Px => LengthUnit::Px,
                HirUnitNumberUnit::Pt => LengthUnit::Pt,
                HirUnitNumberUnit::Em => LengthUnit::Em,
                HirUnitNumberUnit::Percent
                | HirUnitNumberUnit::Rem
                | HirUnitNumberUnit::Vw
                | HirUnitNumberUnit::Vh
                | HirUnitNumberUnit::Deg
                | HirUnitNumberUnit::Rad
                | HirUnitNumberUnit::Turn
                | HirUnitNumberUnit::Db
                | HirUnitNumberUnit::Lufs
                | HirUnitNumberUnit::Bpm
                | HirUnitNumberUnit::Bars => {
                    return Err(CompileTimeScalarReductionError::WrongUnit);
                }
            };
            Ok(CheckedCompileTimeScalar::Length(CheckedLength {
                milli: checked_signed_milli(signed_decimal_milli(decimal, negative)?)?,
                unit,
            }))
        }
        (
            HirLiteral::UnitNumber(HirUnitNumberLiteral::Value {
                decimal,
                unit: HirUnitNumberUnit::Deg,
            }),
            CheckedCompileTimeScalarKind::Angle,
        ) => Ok(CheckedCompileTimeScalar::Angle(CheckedAngle {
            milli_degrees: checked_angle_milli(signed_decimal_milli(decimal, negative)?)?,
        })),
        (
            HirLiteral::UnitNumber(HirUnitNumberLiteral::Value { .. }),
            CheckedCompileTimeScalarKind::Angle,
        ) => Err(CompileTimeScalarReductionError::WrongUnit),
        (
            HirLiteral::Duration(HirDurationLiteral::Value(value)),
            CheckedCompileTimeScalarKind::Duration,
        ) if !negative => {
            let nanos = limbs_to_u64(value.semantic_value().nanoseconds().limbs_le())
                .ok_or(CompileTimeScalarReductionError::Overflow)?;
            if nanos % 1_000_000 != 0 {
                return Err(CompileTimeScalarReductionError::PrecisionLoss);
            }
            let millis = nanos / 1_000_000;
            if millis > 86_400_000 {
                return Err(CompileTimeScalarReductionError::OutOfRange);
            }
            Ok(CheckedCompileTimeScalar::Duration(CheckedDuration {
                millis,
            }))
        }
        (
            HirLiteral::Duration(HirDurationLiteral::Value(_)),
            CheckedCompileTimeScalarKind::Duration,
        ) => Err(CompileTimeScalarReductionError::WrongSign),
        (
            HirLiteral::String(HirStringLiteral::Value(value)),
            CheckedCompileTimeScalarKind::PublicId,
        ) if !negative && value.len() <= MAX_PROXY_VALUE_BYTES => {
            arcweft_id::PublicId::try_new(value.to_string())
                .map(CheckedCompileTimeScalar::PublicId)
                .map_err(|_| CompileTimeScalarReductionError::WrongLiteral)
        }
        (
            HirLiteral::String(HirStringLiteral::Value(value)),
            CheckedCompileTimeScalarKind::Text,
        ) if !negative && value.len() <= MAX_PROXY_VALUE_BYTES => {
            Ok(CheckedCompileTimeScalar::Text(value.to_string()))
        }
        (
            HirLiteral::String(HirStringLiteral::Value(_)),
            CheckedCompileTimeScalarKind::PublicId | CheckedCompileTimeScalarKind::Text,
        ) if negative => Err(CompileTimeScalarReductionError::WrongSign),
        (
            HirLiteral::String(HirStringLiteral::Value(_)),
            CheckedCompileTimeScalarKind::PublicId | CheckedCompileTimeScalarKind::Text,
        ) => Err(CompileTimeScalarReductionError::OutOfRange),
        (HirLiteral::String(HirStringLiteral::Value(_)), _)
        | (HirLiteral::Character(_), _)
        | (HirLiteral::Integer(HirIntegerLiteral::Invalid(_)), _)
        | (HirLiteral::Float(HirFloatLiteral::Invalid(_)), _)
        | (HirLiteral::UnitNumber(HirUnitNumberLiteral::Invalid(_)), _)
        | (HirLiteral::Duration(HirDurationLiteral::Invalid(_)), _) => {
            Err(CompileTimeScalarReductionError::WrongLiteral)
        }
        _ if negative => Err(CompileTimeScalarReductionError::WrongSign),
        _ => Err(CompileTimeScalarReductionError::WrongLiteral),
    }
}

fn unsigned_i64(value: &HirBigUint) -> Result<i64, CompileTimeScalarReductionError> {
    i64::try_from(limbs_to_u64(value.limbs_le()).ok_or(CompileTimeScalarReductionError::Overflow)?)
        .map_err(|_| CompileTimeScalarReductionError::Overflow)
}

fn signed_i64(value: &HirBigUint, negative: bool) -> Result<i64, CompileTimeScalarReductionError> {
    let value = limbs_to_u64(value.limbs_le()).ok_or(CompileTimeScalarReductionError::Overflow)?;
    if negative {
        i64::try_from(-i128::from(value)).map_err(|_| CompileTimeScalarReductionError::Overflow)
    } else {
        i64::try_from(value).map_err(|_| CompileTimeScalarReductionError::Overflow)
    }
}

fn signed_decimal_milli(
    decimal: &HirDecimal,
    negative: bool,
) -> Result<i64, CompileTimeScalarReductionError> {
    let mut coefficient = 0_i128;
    for digit in decimal.coefficient().digits() {
        coefficient = coefficient
            .checked_mul(10)
            .and_then(|value| value.checked_add(i128::from(*digit)))
            .ok_or(CompileTimeScalarReductionError::Overflow)?;
    }
    let power = i64::from(decimal.exponent10()) - i64::from(decimal.scale()) + 3;
    let milli = if power >= 0 {
        coefficient
            .checked_mul(
                10_i128
                    .checked_pow(
                        u32::try_from(power)
                            .map_err(|_| CompileTimeScalarReductionError::Overflow)?,
                    )
                    .ok_or(CompileTimeScalarReductionError::Overflow)?,
            )
            .ok_or(CompileTimeScalarReductionError::Overflow)?
    } else {
        let divisor = 10_i128
            .checked_pow(
                u32::try_from(power.unsigned_abs())
                    .map_err(|_| CompileTimeScalarReductionError::Overflow)?,
            )
            .ok_or(CompileTimeScalarReductionError::Overflow)?;
        if coefficient % divisor != 0 {
            return Err(CompileTimeScalarReductionError::PrecisionLoss);
        }
        coefficient / divisor
    };
    i64::try_from(if negative { -milli } else { milli })
        .map_err(|_| CompileTimeScalarReductionError::Overflow)
}

fn checked_signed_milli(value: i64) -> Result<i32, CompileTimeScalarReductionError> {
    if !(-1_000_000_000..=1_000_000_000).contains(&value) {
        return Err(CompileTimeScalarReductionError::OutOfRange);
    }
    i32::try_from(value).map_err(|_| CompileTimeScalarReductionError::Overflow)
}

fn checked_angle_milli(value: i64) -> Result<i32, CompileTimeScalarReductionError> {
    if !(-360_000_000..=360_000_000).contains(&value) {
        return Err(CompileTimeScalarReductionError::OutOfRange);
    }
    i32::try_from(value).map_err(|_| CompileTimeScalarReductionError::Overflow)
}

fn checked_ratio(value: i64) -> Result<CheckedCompileTimeScalar, CompileTimeScalarReductionError> {
    if !(0..=1_000).contains(&value) {
        return Err(CompileTimeScalarReductionError::OutOfRange);
    }
    Ok(CheckedCompileTimeScalar::Ratio(RatioMilli(
        u16::try_from(value).map_err(|_| CompileTimeScalarReductionError::Overflow)?,
    )))
}

fn limbs_to_u64(limbs: &[u32]) -> Option<u64> {
    match limbs {
        [] => Some(0),
        [low] => Some(u64::from(*low)),
        [low, high] => Some(u64::from(*low) | (u64::from(*high) << 32)),
        _ => None,
    }
}
