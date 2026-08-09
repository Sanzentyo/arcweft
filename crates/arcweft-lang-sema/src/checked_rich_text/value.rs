use arcweft_id::PublicId;
use arcweft_rich_text_schema::{
    RichTextDefaultValue, RichTextEnumSchemaId, RichTextPropertySpec, RichTextUnit,
    RichTextValueKind,
};

use super::RichTextDiagnosticCode;

/// Checked fixed-point thousandths.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Milli(pub i32);

/// Checked inclusive ratio encoded as thousandths.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RatioMilli(pub u16);

/// Length unit accepted by the `RichText` authoring boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LengthUnit {
    Px,
    Pt,
    Ch,
    Em,
}

/// Checked fixed-point length.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedLength {
    pub milli: i32,
    pub unit: LengthUnit,
}

/// Checked angle in milli-degrees.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedAngle {
    pub milli_degrees: i32,
}

/// Checked duration in exact milliseconds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedDuration {
    pub millis: u64,
}

/// Checked two-dimensional fixed-point vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedVec2 {
    pub x: Milli,
    pub y: Milli,
}

/// Deterministic 32-bit seed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Seed32(pub u32);

/// One value of a schema-owned closed enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedEnumValue {
    pub enum_id: RichTextEnumSchemaId,
    pub variant: u16,
}

/// Checked color representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedColor {
    Rgba8([u8; 4]),
    Resource(PublicId),
}

/// Closed checked scalar algebra. There is deliberately no raw-string escape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedRichTextValue {
    Bool(bool),
    Int(i64),
    Milli(Milli),
    Ratio(RatioMilli),
    Length(CheckedLength),
    Angle(CheckedAngle),
    Duration(CheckedDuration),
    Enum(CheckedEnumValue),
    PublicId(PublicId),
    Text(String),
    Color(CheckedColor),
    Vec2(CheckedVec2),
    Seed(Seed32),
}

pub(crate) fn parse_checked_value<P: Copy + Eq + 'static>(
    source: &str,
    spec: &RichTextPropertySpec<P>,
) -> Result<CheckedRichTextValue, RichTextDiagnosticCode> {
    if source.len() > usize::from(spec.limits.max_decoded_bytes) {
        return Err(RichTextDiagnosticCode::ResourceLimit);
    }
    if source.is_empty() && !spec.allow_empty {
        return Err(RichTextDiagnosticCode::EmptyValue);
    }
    match spec.kind {
        RichTextValueKind::Bool => match source {
            "true" => Ok(CheckedRichTextValue::Bool(true)),
            "false" => Ok(CheckedRichTextValue::Bool(false)),
            _ => Err(RichTextDiagnosticCode::InvalidBoolean),
        },
        RichTextValueKind::Int => {
            let value = parse_integer(source)?;
            enforce_integer_limits(value, spec)?;
            Ok(CheckedRichTextValue::Int(value))
        }
        RichTextValueKind::FixedMilli => {
            let value = parse_fixed_with_expected_unit(source, spec.limits.units)?.0;
            enforce_milli_limits(value, spec)?;
            let value = i32::try_from(value).map_err(|_| RichTextDiagnosticCode::Overflow)?;
            Ok(CheckedRichTextValue::Milli(Milli(value)))
        }
        RichTextValueKind::Ratio => {
            let (value, unit) = parse_fixed_with_expected_unit(source, &[RichTextUnit::Unitless])?;
            if unit != RichTextUnit::Unitless || !(0..=1_000).contains(&value) {
                return Err(RichTextDiagnosticCode::OutOfRange);
            }
            enforce_milli_limits(value, spec)?;
            let value = u16::try_from(value).map_err(|_| RichTextDiagnosticCode::Overflow)?;
            Ok(CheckedRichTextValue::Ratio(RatioMilli(value)))
        }
        RichTextValueKind::Length => {
            let (value, unit) = parse_fixed_with_expected_unit(source, spec.limits.units)?;
            enforce_milli_limits(value, spec)?;
            let milli = i32::try_from(value).map_err(|_| RichTextDiagnosticCode::Overflow)?;
            let unit = match unit {
                RichTextUnit::Px => LengthUnit::Px,
                RichTextUnit::Pt => LengthUnit::Pt,
                RichTextUnit::Ch => LengthUnit::Ch,
                RichTextUnit::Em => LengthUnit::Em,
                RichTextUnit::Unitless
                | RichTextUnit::Deg
                | RichTextUnit::Ms
                | RichTextUnit::S
                | RichTextUnit::Cps => return Err(RichTextDiagnosticCode::InvalidUnit),
            };
            Ok(CheckedRichTextValue::Length(CheckedLength { milli, unit }))
        }
        RichTextValueKind::Angle => {
            let (value, unit) = parse_fixed_with_expected_unit(source, spec.limits.units)?;
            if unit != RichTextUnit::Deg {
                return Err(RichTextDiagnosticCode::InvalidUnit);
            }
            enforce_milli_limits(value, spec)?;
            let milli_degrees =
                i32::try_from(value).map_err(|_| RichTextDiagnosticCode::Overflow)?;
            Ok(CheckedRichTextValue::Angle(CheckedAngle { milli_degrees }))
        }
        RichTextValueKind::Duration => parse_duration(source, spec),
        RichTextValueKind::ClosedEnum(enum_id) => {
            let variant = spec
                .limits
                .enum_values
                .iter()
                .position(|candidate| *candidate == source)
                .ok_or(RichTextDiagnosticCode::InvalidEnum)?;
            let variant = u16::try_from(variant).map_err(|_| RichTextDiagnosticCode::Overflow)?;
            Ok(CheckedRichTextValue::Enum(CheckedEnumValue {
                enum_id,
                variant,
            }))
        }
        RichTextValueKind::Selector(_) | RichTextValueKind::PublicId => {
            parse_public_id(source).map(CheckedRichTextValue::PublicId)
        }
        RichTextValueKind::Text => Ok(CheckedRichTextValue::Text(source.to_owned())),
        RichTextValueKind::Color => parse_color(source).map(CheckedRichTextValue::Color),
        RichTextValueKind::Vec2 => parse_vec2(source, spec),
        RichTextValueKind::Seed32 => parse_seed(source).map(CheckedRichTextValue::Seed),
        RichTextValueKind::TextProxyField => Err(RichTextDiagnosticCode::SchemaUnavailable),
    }
}

pub(crate) fn checked_default(
    value: RichTextDefaultValue,
    enum_id: Option<RichTextEnumSchemaId>,
) -> Result<CheckedRichTextValue, RichTextDiagnosticCode> {
    Ok(match value {
        RichTextDefaultValue::Bool(value) => CheckedRichTextValue::Bool(value),
        RichTextDefaultValue::Int(value) => CheckedRichTextValue::Int(value),
        RichTextDefaultValue::Milli(value) => CheckedRichTextValue::Milli(Milli(value)),
        RichTextDefaultValue::RatioMilli(value) => CheckedRichTextValue::Ratio(RatioMilli(value)),
        RichTextDefaultValue::Length { milli, unit } => {
            let unit = match unit {
                RichTextUnit::Px => LengthUnit::Px,
                RichTextUnit::Pt => LengthUnit::Pt,
                RichTextUnit::Ch => LengthUnit::Ch,
                RichTextUnit::Em => LengthUnit::Em,
                RichTextUnit::Unitless
                | RichTextUnit::Deg
                | RichTextUnit::Ms
                | RichTextUnit::S
                | RichTextUnit::Cps => return Err(RichTextDiagnosticCode::InvalidUnit),
            };
            CheckedRichTextValue::Length(CheckedLength { milli, unit })
        }
        RichTextDefaultValue::AngleMilliDegrees(milli_degrees) => {
            CheckedRichTextValue::Angle(CheckedAngle { milli_degrees })
        }
        RichTextDefaultValue::DurationMillis(millis) => {
            CheckedRichTextValue::Duration(CheckedDuration { millis })
        }
        RichTextDefaultValue::EnumVariant(variant) => {
            CheckedRichTextValue::Enum(CheckedEnumValue {
                enum_id: enum_id.ok_or(RichTextDiagnosticCode::InvalidEnum)?,
                variant,
            })
        }
        RichTextDefaultValue::PublicId(value) => {
            CheckedRichTextValue::PublicId(parse_public_id(value)?)
        }
        RichTextDefaultValue::Text(value) => CheckedRichTextValue::Text(value.to_owned()),
        RichTextDefaultValue::ColorRgba8(value) => {
            CheckedRichTextValue::Color(CheckedColor::Rgba8(value))
        }
        RichTextDefaultValue::Vec2Milli([x, y]) => CheckedRichTextValue::Vec2(CheckedVec2 {
            x: Milli(x),
            y: Milli(y),
        }),
        RichTextDefaultValue::Seed32(value) => CheckedRichTextValue::Seed(Seed32(value)),
    })
}

pub(crate) fn parse_integer(source: &str) -> Result<i64, RichTextDiagnosticCode> {
    let (negative, digits) = signed_digits(source, 19, false)?;
    let magnitude = digits.iter().try_fold(0_u64, |value, digit| {
        value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u64::from(*digit - b'0')))
            .ok_or(RichTextDiagnosticCode::Overflow)
    })?;
    if negative {
        if magnitude == i64::MAX as u64 + 1 {
            Ok(i64::MIN)
        } else {
            i64::try_from(magnitude)
                .map(|value| -value)
                .map_err(|_| RichTextDiagnosticCode::Overflow)
        }
    } else {
        i64::try_from(magnitude).map_err(|_| RichTextDiagnosticCode::Overflow)
    }
}

pub(crate) fn parse_fixed(source: &str) -> Result<i64, RichTextDiagnosticCode> {
    if is_non_finite_spelling(source) {
        return Err(RichTextDiagnosticCode::NonFinite);
    }
    let (negative, unsigned) = strip_sign(source)?;
    let (whole, fraction) = unsigned
        .split_once('.')
        .map_or((unsigned, None), |parts| (parts.0, Some(parts.1)));
    if whole.is_empty() || whole.len() > 19 || !whole.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(RichTextDiagnosticCode::InvalidDecimal);
    }
    let fraction = fraction.unwrap_or("");
    if unsigned.contains('.') && (fraction.is_empty() || fraction.len() > 3) {
        return Err(RichTextDiagnosticCode::InvalidDecimal);
    }
    if !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(RichTextDiagnosticCode::InvalidDecimal);
    }
    let whole = whole.bytes().try_fold(0_i64, |value, digit| {
        value
            .checked_mul(10)
            .and_then(|value| value.checked_add(i64::from(digit - b'0')))
            .ok_or(RichTextDiagnosticCode::Overflow)
    })?;
    let fractional = fraction
        .bytes()
        .fold(0_i64, |value, digit| value * 10 + i64::from(digit - b'0'));
    let scale = match fraction.len() {
        0 | 3 => 1,
        1 => 100,
        2 => 10,
        _ => unreachable!("fraction length was checked above"),
    };
    let magnitude = whole
        .checked_mul(1_000)
        .and_then(|value| value.checked_add(fractional * scale))
        .ok_or(RichTextDiagnosticCode::Overflow)?;
    if negative {
        magnitude
            .checked_neg()
            .ok_or(RichTextDiagnosticCode::Overflow)
    } else {
        Ok(magnitude)
    }
}

fn parse_fixed_with_expected_unit(
    source: &str,
    accepted: &[RichTextUnit],
) -> Result<(i64, RichTextUnit), RichTextDiagnosticCode> {
    let (number, authored_unit) = split_unit(source);
    let unit = match authored_unit {
        Some(unit) if accepted.contains(&unit) => unit,
        None if accepted.contains(&RichTextUnit::Unitless) => RichTextUnit::Unitless,
        None if accepted.len() == 1 => accepted[0],
        Some(_) | None => return Err(RichTextDiagnosticCode::InvalidUnit),
    };
    Ok((parse_fixed(number)?, unit))
}

fn parse_duration<P: Copy + Eq + 'static>(
    source: &str,
    spec: &RichTextPropertySpec<P>,
) -> Result<CheckedRichTextValue, RichTextDiagnosticCode> {
    let (number, unit) = split_unit(source);
    let unit = unit.ok_or(RichTextDiagnosticCode::InvalidUnit)?;
    if !spec.limits.units.contains(&unit) {
        return Err(RichTextDiagnosticCode::InvalidUnit);
    }
    let fixed = parse_fixed(number).map_err(|code| match code {
        RichTextDiagnosticCode::InvalidDecimal | RichTextDiagnosticCode::InvalidInteger => {
            RichTextDiagnosticCode::InvalidDuration
        }
        other => other,
    })?;
    if fixed < 0 {
        return Err(RichTextDiagnosticCode::Negative);
    }
    let millis = match unit {
        RichTextUnit::Ms if fixed % 1_000 == 0 => fixed / 1_000,
        RichTextUnit::Ms => return Err(RichTextDiagnosticCode::Underflow),
        RichTextUnit::S => fixed,
        RichTextUnit::Unitless
        | RichTextUnit::Px
        | RichTextUnit::Pt
        | RichTextUnit::Ch
        | RichTextUnit::Em
        | RichTextUnit::Deg
        | RichTextUnit::Cps => return Err(RichTextDiagnosticCode::InvalidUnit),
    };
    let millis_milli = millis
        .checked_mul(1_000)
        .ok_or(RichTextDiagnosticCode::Overflow)?;
    enforce_raw_limits(millis_milli, spec)?;
    let millis = u64::try_from(millis).map_err(|_| RichTextDiagnosticCode::Overflow)?;
    Ok(CheckedRichTextValue::Duration(CheckedDuration { millis }))
}

pub(crate) fn parse_public_id(source: &str) -> Result<PublicId, RichTextDiagnosticCode> {
    let source = source
        .strip_prefix('.')
        .or_else(|| source.strip_prefix('@'))
        .unwrap_or(source);
    PublicId::try_new(source.to_owned()).map_err(|_| RichTextDiagnosticCode::InvalidSelector)
}

pub(crate) fn parse_color(source: &str) -> Result<CheckedColor, RichTextDiagnosticCode> {
    if let Some(hex) = source.strip_prefix('#') {
        if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(RichTextDiagnosticCode::InvalidColor);
        }
        let mut rgba = [255_u8; 4];
        for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
            let pair =
                std::str::from_utf8(pair).map_err(|_| RichTextDiagnosticCode::InvalidColor)?;
            rgba[index] =
                u8::from_str_radix(pair, 16).map_err(|_| RichTextDiagnosticCode::InvalidColor)?;
        }
        return Ok(CheckedColor::Rgba8(rgba));
    }
    let rgba = match source {
        "black" => [0, 0, 0, 255],
        "white" => [255, 255, 255, 255],
        "red" => [255, 0, 0, 255],
        "green" => [0, 128, 0, 255],
        "blue" => [0, 0, 255, 255],
        "transparent" => [0, 0, 0, 0],
        _ if source.starts_with('@') => return parse_public_id(source).map(CheckedColor::Resource),
        _ => return Err(RichTextDiagnosticCode::InvalidColor),
    };
    Ok(CheckedColor::Rgba8(rgba))
}

fn parse_vec2<P: Copy + Eq + 'static>(
    source: &str,
    spec: &RichTextPropertySpec<P>,
) -> Result<CheckedRichTextValue, RichTextDiagnosticCode> {
    let (x, y) = source
        .split_once(',')
        .ok_or(RichTextDiagnosticCode::InvalidVec2)?;
    if y.contains(',') {
        return Err(RichTextDiagnosticCode::InvalidVec2);
    }
    let x = parse_fixed(x)?;
    let y = parse_fixed(y)?;
    enforce_milli_limits(x, spec)?;
    enforce_milli_limits(y, spec)?;
    if x == 0 && y == 0 {
        return Err(RichTextDiagnosticCode::InvalidVec2);
    }
    Ok(CheckedRichTextValue::Vec2(CheckedVec2 {
        x: Milli(i32::try_from(x).map_err(|_| RichTextDiagnosticCode::Overflow)?),
        y: Milli(i32::try_from(y).map_err(|_| RichTextDiagnosticCode::Overflow)?),
    }))
}

fn parse_seed(source: &str) -> Result<Seed32, RichTextDiagnosticCode> {
    if source.is_empty() || source.len() > 64 {
        return Err(RichTextDiagnosticCode::ResourceLimit);
    }
    if source.bytes().all(|byte| byte.is_ascii_digit()) {
        return source
            .parse::<u32>()
            .map(Seed32)
            .map_err(|_| RichTextDiagnosticCode::Overflow);
    }
    Ok(Seed32(
        source
            .as_bytes()
            .iter()
            .fold(0x811c_9dc5_u32, |hash, byte| {
                (hash ^ u32::from(*byte)).wrapping_mul(0x0100_0193)
            }),
    ))
}

fn enforce_integer_limits<P: Copy + Eq + 'static>(
    value: i64,
    spec: &RichTextPropertySpec<P>,
) -> Result<(), RichTextDiagnosticCode> {
    enforce_raw_limits(value, spec)
}

fn enforce_milli_limits<P: Copy + Eq + 'static>(
    value: i64,
    spec: &RichTextPropertySpec<P>,
) -> Result<(), RichTextDiagnosticCode> {
    enforce_raw_limits(value, spec)
}

fn enforce_raw_limits<P: Copy + Eq + 'static>(
    value: i64,
    spec: &RichTextPropertySpec<P>,
) -> Result<(), RichTextDiagnosticCode> {
    let Some(limits) = spec.limits.numeric else {
        return Ok(());
    };
    if let Some(minimum) = limits.inclusive_min_milli
        && value < minimum
    {
        return Err(if value < 0 && minimum >= 0 {
            RichTextDiagnosticCode::Negative
        } else {
            RichTextDiagnosticCode::OutOfRange
        });
    }
    if limits
        .inclusive_max_milli
        .is_some_and(|maximum| value > maximum)
    {
        return Err(RichTextDiagnosticCode::OutOfRange);
    }
    Ok(())
}

pub(crate) fn split_unit(source: &str) -> (&str, Option<RichTextUnit>) {
    const UNITS: [(&str, RichTextUnit); 9] = [
        ("cps", RichTextUnit::Cps),
        ("deg", RichTextUnit::Deg),
        ("px", RichTextUnit::Px),
        ("pt", RichTextUnit::Pt),
        ("ch", RichTextUnit::Ch),
        ("em", RichTextUnit::Em),
        ("ms", RichTextUnit::Ms),
        ("s", RichTextUnit::S),
        ("", RichTextUnit::Unitless),
    ];
    UNITS
        .iter()
        .find_map(|(suffix, unit)| {
            (!suffix.is_empty())
                .then(|| {
                    source
                        .strip_suffix(suffix)
                        .map(|number| (number, Some(*unit)))
                })
                .flatten()
        })
        .unwrap_or((source, None))
}

fn signed_digits(
    source: &str,
    maximum: usize,
    decimal: bool,
) -> Result<(bool, &[u8]), RichTextDiagnosticCode> {
    let (negative, unsigned) = strip_sign(source)?;
    let digits = unsigned.as_bytes();
    if digits.is_empty() || digits.len() > maximum || !digits.iter().all(u8::is_ascii_digit) {
        return Err(if decimal {
            RichTextDiagnosticCode::InvalidDecimal
        } else {
            RichTextDiagnosticCode::InvalidInteger
        });
    }
    Ok((negative, digits))
}

fn strip_sign(source: &str) -> Result<(bool, &str), RichTextDiagnosticCode> {
    if let Some(value) = source.strip_prefix('-') {
        Ok((true, value))
    } else if let Some(value) = source.strip_prefix('+') {
        Ok((false, value))
    } else if source.is_empty() {
        Err(RichTextDiagnosticCode::InvalidDecimal)
    } else {
        Ok((false, source))
    }
}

fn is_non_finite_spelling(source: &str) -> bool {
    matches!(
        source.trim_start_matches(['+', '-']),
        "NaN" | "nan" | "inf" | "Inf" | "Infinity" | "infinity"
    )
}

#[cfg(test)]
mod tests {
    use super::parse_integer;
    use crate::checked_rich_text::RichTextDiagnosticCode;

    #[test]
    fn integer_parser_accepts_exact_i64_boundaries() {
        assert_eq!(parse_integer("9223372036854775807"), Ok(i64::MAX));
        assert_eq!(parse_integer("-9223372036854775808"), Ok(i64::MIN));
    }

    #[test]
    fn integer_parser_rejects_values_beyond_i64_boundaries() {
        assert_eq!(
            parse_integer("9223372036854775808"),
            Err(RichTextDiagnosticCode::Overflow)
        );
        assert_eq!(
            parse_integer("-9223372036854775809"),
            Err(RichTextDiagnosticCode::Overflow)
        );
    }
}
