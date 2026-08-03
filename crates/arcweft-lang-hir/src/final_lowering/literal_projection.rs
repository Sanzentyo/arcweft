//! Shared typed literal projection into final HIR without source readers.

use arcweft_lang_syntax::literal::{
    DurationUnit, FloatSuffix, IntRadix, IntSuffix, SyntaxCharacterIssue,
    SyntaxDecimalComponentIssue, SyntaxDecimalIssue, SyntaxDecimalLiteral, SyntaxDurationIssue,
    SyntaxIntegerIssue, SyntaxLiteralIssue, SyntaxLiteralSyntax, SyntaxLiteralValue,
    SyntaxStringIssue, SyntaxUnitNumberIssue, UnitNumberSuffix,
};
use num_bigint::BigUint as ArithmeticBigUint;

use crate::identity::HirLimit;
use crate::leaf::{
    HirBigUint, HirCharacterIssue, HirCharacterLiteral, HirDecimal, HirDecimalDigits,
    HirDecimalIssue, HirDurationIssue, HirDurationLiteral, HirDurationSemanticValue,
    HirDurationUnit, HirDurationValue, HirFloatIssue, HirFloatLiteral, HirFloatWidth,
    HirIntegerIssue, HirIntegerLiteral, HirIntegerRadix, HirIntegerSuffix, HirLiteral,
    HirStringIssue, HirStringLiteral, HirUnitNumberIssue, HirUnitNumberLiteral, HirUnitNumberUnit,
};
use crate::lower::{HirInvariantFailure, HirLimitError, HirLowerFailure};

use super::require_limit;

pub(crate) fn literal(value: &SyntaxLiteralSyntax) -> Result<HirLiteral, HirLowerFailure> {
    if let Some(digit_count) = value.numeric_digit_count() {
        require_limit(HirLimit::NumericDigitsPerLiteral, digit_count)?;
    }
    match value.value() {
        SyntaxLiteralValue::Bool(value) => Ok(HirLiteral::Boolean(*value)),
        SyntaxLiteralValue::String { value, .. } => {
            require_limit(HirLimit::DecodedStringBytes, value.len())?;
            Ok(HirLiteral::String(HirStringLiteral::Value(value.clone())))
        }
        SyntaxLiteralValue::Character(value) => {
            Ok(HirLiteral::Character(HirCharacterLiteral::Value(*value)))
        }
        SyntaxLiteralValue::Integer(value) => Ok(HirLiteral::Integer(integer_literal(value)?)),
        SyntaxLiteralValue::Decimal(value) => Ok(HirLiteral::Float(HirFloatLiteral::Value {
            decimal: decimal(value)?,
            explicit_width: value.suffix().map(float_width),
        })),
        SyntaxLiteralValue::Unit { value, unit } => {
            Ok(HirLiteral::UnitNumber(HirUnitNumberLiteral::Value {
                decimal: decimal(value)?,
                unit: unit_number(*unit),
            }))
        }
        SyntaxLiteralValue::Duration { value, unit } => {
            let decimal = decimal(value)?;
            let (unit, factor, power) = duration_unit(*unit);
            Ok(HirLiteral::Duration(
                match duration_nanoseconds(&decimal, factor, power)? {
                    Some(nanoseconds) => HirDurationLiteral::Value(HirDurationValue::new(
                        HirDurationSemanticValue::try_new(nanoseconds),
                        unit,
                    )),
                    None => HirDurationLiteral::Invalid(HirDurationIssue::FractionalNanosecond),
                },
            ))
        }
        SyntaxLiteralValue::Invalid(issue) => Ok(invalid_literal(issue)),
    }
}

pub(super) fn invalid_literal(issue: &SyntaxLiteralIssue) -> HirLiteral {
    match issue {
        SyntaxLiteralIssue::String(issue) => {
            HirLiteral::String(HirStringLiteral::Invalid(match issue {
                SyntaxStringIssue::InvalidEscape { .. } => HirStringIssue::InvalidEscape,
                SyntaxStringIssue::Unterminated { .. } => HirStringIssue::Unterminated,
            }))
        }
        SyntaxLiteralIssue::Character(issue) => {
            HirLiteral::Character(HirCharacterLiteral::Invalid(match issue {
                SyntaxCharacterIssue::InvalidEscape { .. } => HirCharacterIssue::InvalidEscape,
                SyntaxCharacterIssue::Unterminated { .. } => HirCharacterIssue::Unterminated,
                SyntaxCharacterIssue::Empty { .. } => HirCharacterIssue::Empty,
                SyntaxCharacterIssue::MultipleScalars { .. } => HirCharacterIssue::MultipleScalars,
            }))
        }
        SyntaxLiteralIssue::Integer(issue) => {
            HirLiteral::Integer(HirIntegerLiteral::Invalid(integer_issue(issue)))
        }
        SyntaxLiteralIssue::Decimal(issue) => {
            HirLiteral::Float(HirFloatLiteral::Invalid(match issue {
                SyntaxDecimalIssue::Decimal(issue) => HirFloatIssue::Decimal(decimal_issue(issue)),
                SyntaxDecimalIssue::InvalidSuffix { .. } => HirFloatIssue::InvalidSuffix,
            }))
        }
        SyntaxLiteralIssue::UnitNumber(issue) => {
            HirLiteral::UnitNumber(HirUnitNumberLiteral::Invalid(match issue {
                SyntaxUnitNumberIssue::Decimal(issue) => {
                    HirUnitNumberIssue::Decimal(decimal_issue(issue))
                }
                SyntaxUnitNumberIssue::InvalidUnit { .. } => HirUnitNumberIssue::InvalidUnit,
            }))
        }
        SyntaxLiteralIssue::Duration(issue) => {
            HirLiteral::Duration(HirDurationLiteral::Invalid(match issue {
                SyntaxDurationIssue::Decimal(issue) => {
                    HirDurationIssue::Decimal(decimal_issue(issue))
                }
                SyntaxDurationIssue::InvalidUnit { .. } => HirDurationIssue::InvalidUnit,
            }))
        }
    }
}

/// Projects one lexer-owned integer without reopening its source spelling.
///
/// Compact numeric sequences share this owner after performing their aggregate
/// preflight, so scalar and ID-less integer payloads cannot drift in radix,
/// suffix, or arbitrary-precision magnitude semantics.
pub(crate) fn integer_literal(
    value: &arcweft_lang_syntax::literal::SyntaxIntegerLiteral,
) -> Result<HirIntegerLiteral, HirLowerFailure> {
    require_limit(HirLimit::NumericDigitsPerLiteral, value.digits().len())?;
    let magnitude = parse_big_uint(value.digits(), value.radix().base())?;
    Ok(HirIntegerLiteral::Value {
        magnitude,
        radix: integer_radix(value.radix()),
        suffix: value.suffix().map(integer_suffix),
    })
}

pub(crate) const fn integer_issue(value: &SyntaxIntegerIssue) -> HirIntegerIssue {
    match value {
        SyntaxIntegerIssue::MissingDigits { .. } => HirIntegerIssue::MissingDigits,
        SyntaxIntegerIssue::InvalidDigits { .. } | SyntaxIntegerIssue::InvalidSeparator { .. } => {
            HirIntegerIssue::InvalidDigit
        }
    }
}

const fn decimal_issue(issue: &SyntaxDecimalComponentIssue) -> HirDecimalIssue {
    match issue {
        SyntaxDecimalComponentIssue::MissingCoefficient { .. } => {
            HirDecimalIssue::MissingCoefficient
        }
        SyntaxDecimalComponentIssue::InvalidDigits { .. }
        | SyntaxDecimalComponentIssue::InvalidSeparator { .. } => HirDecimalIssue::InvalidDigit,
    }
}

fn decimal(value: &SyntaxDecimalLiteral) -> Result<HirDecimal, HirLowerFailure> {
    let authored_scale = value.fractional_digits().map_or(0usize, str::len);
    require_limit(HirLimit::DecimalScale, authored_scale)?;
    let authored_exponent = authored_decimal_exponent(value)?;

    let mut digits = value
        .integral_digits()
        .bytes()
        .map(|digit| digit - b'0')
        .collect::<Vec<_>>();
    if let Some(fractional) = value.fractional_digits() {
        digits.extend(fractional.bytes().map(|digit| digit - b'0'));
    }
    let first_nonzero = digits.iter().position(|digit| *digit != 0);
    let Some(first_nonzero) = first_nonzero else {
        return HirDecimal::try_new(
            HirDecimalDigits::try_new(Box::new([0])).expect("zero is canonical"),
            0,
            0,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit.into());
    };
    let trailing = digits.iter().rev().take_while(|digit| **digit == 0).count();
    let fraction_trim = trailing.min(authored_scale);
    let integral_trim = trailing
        .checked_sub(fraction_trim)
        .ok_or_else(|| limit_overflow(HirLimit::DecimalExponentAbs))?;
    let scale = authored_scale
        .checked_sub(fraction_trim)
        .ok_or_else(|| limit_overflow(HirLimit::DecimalScale))?;
    let canonical_end = digits
        .len()
        .checked_sub(trailing)
        .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
    let coefficient_digits = canonical_end
        .checked_sub(first_nonzero)
        .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
    require_limit(HirLimit::DecimalCoefficientDigits, coefficient_digits)?;
    let trailing_integral =
        i64::try_from(integral_trim).map_err(|_| limit_overflow(HirLimit::DecimalExponentAbs))?;
    let exponent = authored_exponent
        .checked_add(trailing_integral)
        .ok_or_else(|| limit_overflow(HirLimit::DecimalExponentAbs))?;
    require_limit(
        HirLimit::DecimalExponentAbs,
        usize::try_from(exponent.unsigned_abs()).unwrap_or(usize::MAX),
    )?;
    let coefficient = HirDecimalDigits::try_new(
        digits[first_nonzero..canonical_end]
            .to_vec()
            .into_boxed_slice(),
    )
    .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
    let scale = u32::try_from(scale).map_err(|_| limit_overflow(HirLimit::DecimalScale))?;
    let exponent =
        i32::try_from(exponent).map_err(|_| limit_overflow(HirLimit::DecimalExponentAbs))?;
    HirDecimal::try_new(coefficient, scale, exponent)
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit.into())
}

fn authored_decimal_exponent(value: &SyntaxDecimalLiteral) -> Result<i64, HirLowerFailure> {
    let Some(exponent) = value.exponent() else {
        return Ok(0);
    };
    let magnitude = exponent.digits().bytes().try_fold(0usize, |value, digit| {
        value
            .checked_mul(10)
            .and_then(|value| value.checked_add(usize::from(digit - b'0')))
            .ok_or_else(|| limit_overflow(HirLimit::DecimalExponentAbs))
    })?;
    require_limit(HirLimit::DecimalExponentAbs, magnitude)?;
    let magnitude =
        i64::try_from(magnitude).map_err(|_| limit_overflow(HirLimit::DecimalExponentAbs))?;
    Ok(if exponent.is_negative() {
        -magnitude
    } else {
        magnitude
    })
}

fn parse_big_uint(digits: &str, radix: u32) -> Result<HirBigUint, HirLowerFailure> {
    let limbs = match radix {
        2 => bit_packed_limbs(digits, 1, radix)?,
        8 => bit_packed_limbs(digits, 3, radix)?,
        10 => decimal_chunk_limbs(digits)?,
        16 => bit_packed_limbs(digits, 4, radix)?,
        _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
    };
    HirBigUint::try_new(limbs.into_boxed_slice())
        .ok_or_else(|| HirInvariantFailure::InvalidArenaCommit.into())
}

fn bit_packed_limbs(
    digits: &str,
    bits_per_digit: u32,
    radix: u32,
) -> Result<Vec<u32>, HirLowerFailure> {
    let bit_count = digits
        .len()
        .checked_mul(usize::try_from(bits_per_digit).expect("digit bit width fits usize"))
        .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
    let limb_bits = usize::try_from(u32::BITS).expect("u32 bit width fits usize");
    let mut limbs = Vec::with_capacity(bit_count.div_ceil(limb_bits));
    let mut accumulator = 0_u64;
    let mut accumulator_bits = 0_u32;
    for character in digits.chars().rev() {
        let digit = character
            .to_digit(radix)
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        accumulator |= u64::from(digit) << accumulator_bits;
        accumulator_bits += bits_per_digit;
        if accumulator_bits >= u32::BITS {
            limbs.push(
                u32::try_from(accumulator & u64::from(u32::MAX))
                    .expect("masked accumulator fits one limb"),
            );
            accumulator >>= u32::BITS;
            accumulator_bits -= u32::BITS;
        }
    }
    if accumulator_bits != 0 {
        limbs.push(u32::try_from(accumulator).expect("partial accumulator fits one limb"));
    }
    trim_high_zero_limbs(&mut limbs);
    Ok(limbs)
}

fn decimal_chunk_limbs(digits: &str) -> Result<Vec<u32>, HirLowerFailure> {
    const CHUNK_DIGITS: usize = 9;
    const CHUNK_RADIX: u32 = 1_000_000_000;

    if digits.is_empty() {
        return Err(HirInvariantFailure::InvalidArenaCommit.into());
    }
    let mut limbs = Vec::with_capacity(digits.len().div_ceil(CHUNK_DIGITS));
    let first_len = match digits.len() % CHUNK_DIGITS {
        0 => CHUNK_DIGITS,
        remainder => remainder,
    };
    multiply_add(&mut limbs, 1, decimal_chunk(&digits[..first_len])?);
    for chunk in digits.as_bytes()[first_len..].chunks_exact(CHUNK_DIGITS) {
        let chunk =
            std::str::from_utf8(chunk).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        multiply_add(&mut limbs, CHUNK_RADIX, decimal_chunk(chunk)?);
    }
    trim_high_zero_limbs(&mut limbs);
    Ok(limbs)
}

fn decimal_chunk(digits: &str) -> Result<u32, HirLowerFailure> {
    digits.bytes().try_fold(0_u32, |value, digit| {
        if !digit.is_ascii_digit() {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u32::from(digit - b'0')))
            .ok_or_else(|| HirInvariantFailure::InvalidArenaCommit.into())
    })
}

fn trim_high_zero_limbs(limbs: &mut Vec<u32>) {
    while limbs.last() == Some(&0) {
        limbs.pop();
    }
}

fn multiply_add(limbs: &mut Vec<u32>, multiplier: u32, addition: u32) {
    let mut carry = u64::from(addition);
    for limb in limbs.iter_mut() {
        let value = u64::from(*limb) * u64::from(multiplier) + carry;
        *limb = u32::try_from(value & u64::from(u32::MAX)).expect("masked product fits one limb");
        carry = value >> 32;
    }
    if carry != 0 {
        limbs.push(u32::try_from(carry).expect("base-2^32 carry fits one limb"));
    }
}

fn duration_nanoseconds(
    decimal: &HirDecimal,
    factor: u32,
    unit_power: i32,
) -> Result<Option<HirBigUint>, HirLowerFailure> {
    let coefficient = decimal.coefficient().digits();
    if coefficient == [0] {
        return Ok(Some(
            HirBigUint::try_new(Box::new([])).expect("empty limb array is canonical zero"),
        ));
    }
    let power = i64::from(decimal.exponent10())
        .checked_sub(i64::from(decimal.scale()))
        .and_then(|value| value.checked_add(i64::from(unit_power)))
        .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
    if power < 0 {
        let mut digits = multiply_decimal_digits(coefficient, factor);
        let required_zeros = usize::try_from(power.unsigned_abs())
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let available_zeros = digits.iter().rev().take_while(|digit| **digit == 0).count();
        if available_zeros < required_zeros {
            return Ok(None);
        }
        digits.truncate(
            digits
                .len()
                .checked_sub(required_zeros)
                .ok_or(HirInvariantFailure::InvalidArenaCommit)?,
        );
        let limbs = decimal_value_limbs(&digits)?;
        return HirBigUint::try_new(limbs.into_boxed_slice())
            .map(Some)
            .ok_or_else(|| HirInvariantFailure::InvalidArenaCommit.into());
    }

    let coefficient_limbs = decimal_value_limbs(coefficient)?;
    let mut value = ArithmeticBigUint::new(coefficient_limbs);
    value *= factor;
    let power = u32::try_from(power).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
    if power != 0 {
        value *= ArithmeticBigUint::from(5_u8).pow(power);
        value <<= usize::try_from(power).expect("u32 decimal power fits usize");
    }
    HirBigUint::try_new(value.to_u32_digits().into_boxed_slice())
        .map(Some)
        .ok_or_else(|| HirInvariantFailure::InvalidArenaCommit.into())
}

fn multiply_decimal_digits(digits: &[u8], factor: u32) -> Vec<u8> {
    let mut reversed = Vec::with_capacity(digits.len().saturating_add(2));
    let mut carry = 0_u32;
    for digit in digits.iter().rev() {
        let value = u32::from(*digit) * factor + carry;
        reversed.push(u8::try_from(value % 10).expect("base-10 digit fits u8"));
        carry = value / 10;
    }
    while carry != 0 {
        reversed.push(u8::try_from(carry % 10).expect("base-10 digit fits u8"));
        carry /= 10;
    }
    reversed.reverse();
    reversed
}

fn decimal_value_limbs(digits: &[u8]) -> Result<Vec<u32>, HirLowerFailure> {
    const CHUNK_DIGITS: usize = 9;
    const CHUNK_RADIX: u32 = 1_000_000_000;

    if digits.is_empty() || digits.iter().any(|digit| *digit > 9) {
        return Err(HirInvariantFailure::InvalidArenaCommit.into());
    }
    let mut limbs = Vec::with_capacity(digits.len().div_ceil(CHUNK_DIGITS));
    let first_len = match digits.len() % CHUNK_DIGITS {
        0 => CHUNK_DIGITS,
        remainder => remainder,
    };
    multiply_add(&mut limbs, 1, decimal_value_chunk(&digits[..first_len])?);
    for chunk in digits[first_len..].chunks_exact(CHUNK_DIGITS) {
        multiply_add(&mut limbs, CHUNK_RADIX, decimal_value_chunk(chunk)?);
    }
    trim_high_zero_limbs(&mut limbs);
    Ok(limbs)
}

fn decimal_value_chunk(digits: &[u8]) -> Result<u32, HirLowerFailure> {
    digits.iter().try_fold(0_u32, |value, digit| {
        value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u32::from(*digit)))
            .ok_or_else(|| HirInvariantFailure::InvalidArenaCommit.into())
    })
}

pub(crate) const fn integer_radix(value: IntRadix) -> HirIntegerRadix {
    match value {
        IntRadix::Binary => HirIntegerRadix::Binary,
        IntRadix::Octal => HirIntegerRadix::Octal,
        IntRadix::Decimal => HirIntegerRadix::Decimal,
        IntRadix::Hexadecimal => HirIntegerRadix::Hexadecimal,
    }
}

pub(crate) const fn integer_suffix(value: IntSuffix) -> HirIntegerSuffix {
    match value {
        IntSuffix::I8 => HirIntegerSuffix::I8,
        IntSuffix::I16 => HirIntegerSuffix::I16,
        IntSuffix::I32 => HirIntegerSuffix::I32,
        IntSuffix::I64 => HirIntegerSuffix::I64,
        IntSuffix::I128 => HirIntegerSuffix::I128,
        IntSuffix::ISize => HirIntegerSuffix::ISize,
        IntSuffix::U8 => HirIntegerSuffix::U8,
        IntSuffix::U16 => HirIntegerSuffix::U16,
        IntSuffix::U32 => HirIntegerSuffix::U32,
        IntSuffix::U64 => HirIntegerSuffix::U64,
        IntSuffix::U128 => HirIntegerSuffix::U128,
        IntSuffix::USize => HirIntegerSuffix::USize,
    }
}

const fn float_width(value: FloatSuffix) -> HirFloatWidth {
    match value {
        FloatSuffix::F32 => HirFloatWidth::F32,
        FloatSuffix::F64 => HirFloatWidth::F64,
    }
}

const fn unit_number(value: UnitNumberSuffix) -> HirUnitNumberUnit {
    match value {
        UnitNumberSuffix::Percent => HirUnitNumberUnit::Percent,
        UnitNumberSuffix::Px => HirUnitNumberUnit::Px,
        UnitNumberSuffix::Pt => HirUnitNumberUnit::Pt,
        UnitNumberSuffix::Em => HirUnitNumberUnit::Em,
        UnitNumberSuffix::Rem => HirUnitNumberUnit::Rem,
        UnitNumberSuffix::Vw => HirUnitNumberUnit::Vw,
        UnitNumberSuffix::Vh => HirUnitNumberUnit::Vh,
        UnitNumberSuffix::Deg => HirUnitNumberUnit::Deg,
        UnitNumberSuffix::Rad => HirUnitNumberUnit::Rad,
        UnitNumberSuffix::Turn => HirUnitNumberUnit::Turn,
        UnitNumberSuffix::Db => HirUnitNumberUnit::Db,
        UnitNumberSuffix::Lufs => HirUnitNumberUnit::Lufs,
        UnitNumberSuffix::Bpm => HirUnitNumberUnit::Bpm,
        UnitNumberSuffix::Bars => HirUnitNumberUnit::Bars,
    }
}

const fn duration_unit(value: DurationUnit) -> (HirDurationUnit, u32, i32) {
    match value {
        DurationUnit::Nanos => (HirDurationUnit::Nanos, 1, 0),
        DurationUnit::Micros => (HirDurationUnit::Micros, 1, 3),
        DurationUnit::Millis => (HirDurationUnit::Millis, 1, 6),
        DurationUnit::Seconds => (HirDurationUnit::Seconds, 1, 9),
        DurationUnit::Minutes => (HirDurationUnit::Minutes, 6, 10),
        DurationUnit::Hours => (HirDurationUnit::Hours, 36, 11),
    }
}

fn limit_overflow(limit: HirLimit) -> HirLowerFailure {
    HirLimitError::with_maximum(limit, usize::MAX, limit.maximum()).into()
}
