//! Exact compile-time lowering of authored Fx constants into the closed runtime domain.

use std::collections::BTreeMap;

use arcweft_lang_syntax::{
    expr::{CallArg, Expr, Literal},
    literal::{DurationUnit, UnitNumberSuffix},
    types::TypeRef,
};
use arcweft_presentation::fx::{
    Angle, FiniteF32, FxColor, FxPhase, FxResourceId, FxRuntimeType, FxRuntimeValue, FxStaticType,
    FxStaticValue, FxTarget, FxVec2, Length, Opacity, Seconds, Transform2D,
};

use crate::{errors::RuntimePlanLowerError, labels::expr_label};

pub(super) fn runtime_type(ty: &TypeRef) -> Result<FxRuntimeType, RuntimePlanLowerError> {
    let TypeRef::Path(path) = ty else {
        return Err(error(format!(
            "Fx parameters require a closed runtime type, found `{ty:?}`"
        )));
    };
    match path.canonical_string().as_str() {
        "bool" => Ok(FxRuntimeType::Bool),
        "i32" => Ok(FxRuntimeType::I32),
        "f32" => Ok(FxRuntimeType::F32),
        "Length" => Ok(FxRuntimeType::Length),
        "Angle" => Ok(FxRuntimeType::Angle),
        "Duration" | "Seconds" => Ok(FxRuntimeType::Seconds),
        "Color" => Ok(FxRuntimeType::Color),
        "Vec2" => Ok(FxRuntimeType::Vec2),
        "Transform2D" => Ok(FxRuntimeType::Transform2D),
        _ => Err(error(format!(
            "Fx parameter type `{path}` is outside the closed runtime value set"
        ))),
    }
}

pub(super) fn lower_static_value(
    expr: &Expr,
    expected: FxStaticType,
    bindings: &BTreeMap<String, FxStaticValue>,
) -> Result<FxStaticValue, RuntimePlanLowerError> {
    if let Some(value) = bound_value(expr, bindings) {
        if expected.accepts(value) {
            return Ok(value.clone());
        }
        return Err(error(format!(
            "Fx value `{}` has type {}, expected {}",
            expr_label(expr),
            value.static_type().as_str(),
            expected.as_str()
        )));
    }

    let value = match expected {
        FxStaticType::Runtime(expected) => {
            FxStaticValue::Runtime(lower_runtime_constant(expr, expected)?)
        }
        FxStaticType::Resource => FxStaticValue::Resource(lower_resource(expr)?),
        FxStaticType::Selector => FxStaticValue::Selector(lower_selector(expr)?),
        FxStaticType::String => FxStaticValue::String(lower_string(expr)?),
        FxStaticType::Target => FxStaticValue::Target(lower_target(expr)?),
        FxStaticType::Phase => FxStaticValue::Phase(lower_phase(expr)?),
        FxStaticType::List => {
            let Expr::BracketSeq(values) = expr else {
                return Err(expected_error(expr, expected));
            };
            FxStaticValue::List(
                values
                    .iter()
                    .map(|value| lower_inferred_static_value(value, bindings))
                    .collect::<Result<Vec<_>, _>>()?,
            )
        }
        FxStaticType::Record => {
            let Expr::RecordLiteral(fields) = expr else {
                return Err(expected_error(expr, expected));
            };
            FxStaticValue::Record(
                fields
                    .iter()
                    .map(|(name, value)| {
                        Ok(arcweft_presentation::fx::FxProperty::new(
                            name,
                            lower_inferred_static_value(value, bindings)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, RuntimePlanLowerError>>()?,
            )
        }
    };
    Ok(value)
}

pub(crate) fn lower_closed_runtime_value(
    expr: &Expr,
    expected: FxRuntimeType,
) -> Result<FxRuntimeValue, RuntimePlanLowerError> {
    lower_runtime_constant(expr, expected)
}

fn lower_inferred_static_value(
    expr: &Expr,
    bindings: &BTreeMap<String, FxStaticValue>,
) -> Result<FxStaticValue, RuntimePlanLowerError> {
    if let Some(value) = bound_value(expr, bindings) {
        return Ok(value.clone());
    }
    match expr {
        Expr::Literal(Literal::Bool(_)) => {
            lower_static_value(expr, FxStaticType::Runtime(FxRuntimeType::Bool), bindings)
        }
        Expr::Literal(Literal::Int(_)) => {
            lower_static_value(expr, FxStaticType::Runtime(FxRuntimeType::I32), bindings)
        }
        Expr::Literal(Literal::Float { .. }) => {
            lower_static_value(expr, FxStaticType::Runtime(FxRuntimeType::F32), bindings)
        }
        Expr::Literal(Literal::UnitNumber { suffix, .. }) => {
            let ty = match suffix {
                UnitNumberSuffix::Px => FxRuntimeType::Length,
                UnitNumberSuffix::Deg | UnitNumberSuffix::Rad | UnitNumberSuffix::Turn => {
                    FxRuntimeType::Angle
                }
                _ => {
                    return Err(error(format!(
                        "unit `{}` is not accepted by Fx runtime programs",
                        suffix.as_str()
                    )));
                }
            };
            lower_static_value(expr, FxStaticType::Runtime(ty), bindings)
        }
        Expr::Literal(Literal::Duration { .. }) => lower_static_value(
            expr,
            FxStaticType::Runtime(FxRuntimeType::Seconds),
            bindings,
        ),
        Expr::Literal(Literal::String(_)) => {
            lower_static_value(expr, FxStaticType::String, bindings)
        }
        Expr::ShortVariant(_) => lower_static_value(expr, FxStaticType::Selector, bindings),
        Expr::EntityRef(_) => lower_static_value(expr, FxStaticType::Resource, bindings),
        Expr::BracketSeq(_) => lower_static_value(expr, FxStaticType::List, bindings),
        Expr::RecordLiteral(_) => lower_static_value(expr, FxStaticType::Record, bindings),
        Expr::Call(call) if simple_path(call.callee()) == Some("rgb") => {
            lower_static_value(expr, FxStaticType::Runtime(FxRuntimeType::Color), bindings)
        }
        Expr::Call(call) if simple_path(call.callee()) == Some("vec2") => {
            lower_static_value(expr, FxStaticType::Runtime(FxRuntimeType::Vec2), bindings)
        }
        Expr::Unary { .. } => {
            let ty = infer_signed_runtime_type(expr)?;
            lower_static_value(expr, FxStaticType::Runtime(ty), bindings)
        }
        _ => Err(error(format!(
            "Fx static value `{}` requires an explicit closed type",
            expr_label(expr)
        ))),
    }
}

fn bound_value<'a>(
    expr: &Expr,
    bindings: &'a BTreeMap<String, FxStaticValue>,
) -> Option<&'a FxStaticValue> {
    let Expr::Path(path) = expr else {
        return None;
    };
    (path.segments().len() == 1)
        .then(|| bindings.get(path.as_label()))
        .flatten()
}

fn lower_runtime_constant(
    expr: &Expr,
    expected: FxRuntimeType,
) -> Result<FxRuntimeValue, RuntimePlanLowerError> {
    match expected {
        FxRuntimeType::Bool => match expr {
            Expr::Literal(Literal::Bool(value)) => Ok(FxRuntimeValue::Bool(*value)),
            _ => Err(runtime_expected_error(expr, expected)),
        },
        FxRuntimeType::I32 => lower_i32(expr).map(FxRuntimeValue::I32),
        FxRuntimeType::F32 => lower_decimal(expr)
            .and_then(finite)
            .map(FxRuntimeValue::F32),
        FxRuntimeType::Length => {
            Length::try_pixels_f64(lower_unit_decimal(expr, UnitFamily::Length)?)
                .map(FxRuntimeValue::Length)
                .map_err(number_error)
        }
        FxRuntimeType::Angle => lower_angle(expr).map(FxRuntimeValue::Angle),
        FxRuntimeType::Seconds => lower_seconds(expr).map(FxRuntimeValue::Seconds),
        FxRuntimeType::Color => lower_color(expr).map(FxRuntimeValue::Color),
        FxRuntimeType::Vec2 => lower_vec2(expr).map(FxRuntimeValue::Vec2),
        FxRuntimeType::Transform2D => lower_transform(expr).map(FxRuntimeValue::Transform2D),
    }
}

fn lower_i32(expr: &Expr) -> Result<i32, RuntimePlanLowerError> {
    if let Expr::ShortVariant(value) = expr {
        return match value.as_str() {
            "thin" => Ok(100),
            "extra_light" => Ok(200),
            "light" => Ok(300),
            "normal" | "regular" => Ok(400),
            "medium" => Ok(500),
            "semi_bold" => Ok(600),
            "strong" | "bold" => Ok(700),
            "extra_bold" => Ok(800),
            "black" => Ok(900),
            other => Err(error(format!("unknown typed i32 selector `.{other}`"))),
        };
    }
    let (negative, literal) = signed_literal(expr)?;
    let Literal::Int(value) = literal else {
        return Err(runtime_expected_error(expr, FxRuntimeType::I32));
    };
    if value
        .suffix()
        .is_some_and(|suffix| suffix.as_str() != "i32")
    {
        return Err(error(format!(
            "Fx i32 literal `{}` uses incompatible suffix",
            value.raw()
        )));
    }
    let magnitude = value
        .magnitude()
        .map_err(|source| error(format!("invalid Fx integer literal: {source}")))?;
    if negative {
        if magnitude == (i32::MAX as u128) + 1 {
            Ok(i32::MIN)
        } else {
            i32::try_from(magnitude)
                .map(|value| -value)
                .map_err(|_| error("Fx integer literal is outside the i32 domain"))
        }
    } else {
        i32::try_from(magnitude).map_err(|_| error("Fx integer literal is outside the i32 domain"))
    }
}

fn lower_decimal(expr: &Expr) -> Result<f64, RuntimePlanLowerError> {
    let (negative, literal) = signed_literal(expr)?;
    let value = match literal {
        Literal::Float { raw, suffix } => {
            let body = suffix.map_or(raw.as_str(), |suffix| {
                raw.strip_suffix(suffix.as_str()).unwrap_or(raw.as_str())
            });
            parse_decimal(body)?
        }
        Literal::Int(value) => {
            let magnitude = value
                .magnitude()
                .map_err(|source| error(format!("invalid Fx integer literal: {source}")))?;
            magnitude
                .to_string()
                .parse::<f64>()
                .map_err(|source| error(format!("invalid Fx decimal literal: {source}")))?
        }
        _ => return Err(runtime_expected_error(expr, FxRuntimeType::F32)),
    };
    Ok(if negative { -value } else { value })
}

#[derive(Clone, Copy)]
enum UnitFamily {
    Length,
    Angle,
}

fn lower_unit_decimal(expr: &Expr, family: UnitFamily) -> Result<f64, RuntimePlanLowerError> {
    let (negative, literal) = signed_literal(expr)?;
    let Literal::UnitNumber { raw, suffix } = literal else {
        return Err(error(format!(
            "Fx unit value `{}` is not an authored unit literal",
            expr_label(expr)
        )));
    };
    let accepted = match family {
        UnitFamily::Length => matches!(suffix, UnitNumberSuffix::Px),
        UnitFamily::Angle => matches!(
            suffix,
            UnitNumberSuffix::Deg | UnitNumberSuffix::Rad | UnitNumberSuffix::Turn
        ),
    };
    if !accepted {
        return Err(error(format!(
            "unit `{}` is not accepted for this Fx value",
            suffix.as_str()
        )));
    }
    let body = raw.strip_suffix(suffix.as_str()).unwrap_or(raw);
    let value = parse_decimal(body)?;
    Ok(if negative { -value } else { value })
}

fn lower_angle(expr: &Expr) -> Result<Angle, RuntimePlanLowerError> {
    let value = lower_unit_decimal(expr, UnitFamily::Angle)?;
    let suffix = match strip_negation(expr) {
        Expr::Literal(Literal::UnitNumber { suffix, .. }) => *suffix,
        _ => return Err(runtime_expected_error(expr, FxRuntimeType::Angle)),
    };
    match suffix {
        UnitNumberSuffix::Rad => FiniteF32::try_from_f64(value)
            .map(|value| Angle::try_radians(value.get()))
            .map_err(number_error)?,
        UnitNumberSuffix::Deg => Angle::try_degrees(value),
        UnitNumberSuffix::Turn => Angle::try_turns(value),
        _ => unreachable!("angle family was checked above"),
    }
    .map_err(number_error)
}

fn lower_seconds(expr: &Expr) -> Result<Seconds, RuntimePlanLowerError> {
    let (negative, literal) = signed_literal(expr)?;
    let Literal::Duration { amount, unit } = literal else {
        return Err(runtime_expected_error(expr, FxRuntimeType::Seconds));
    };
    if !matches!(unit, DurationUnit::Seconds | DurationUnit::Millis) {
        return Err(error(format!(
            "duration unit `{}` is not accepted by Fx samplers; use `s` or `ms`",
            unit.as_str()
        )));
    }
    let value = parse_decimal(amount)?;
    let value = if negative { -value } else { value };
    match unit {
        DurationUnit::Seconds => Seconds::try_seconds_f64(value),
        DurationUnit::Millis => Seconds::try_milliseconds(value),
        _ => unreachable!("duration unit was checked above"),
    }
    .map_err(number_error)
}

fn lower_color(expr: &Expr) -> Result<FxColor, RuntimePlanLowerError> {
    let Expr::Call(call) = expr else {
        return Err(runtime_expected_error(expr, FxRuntimeType::Color));
    };
    let Some(function) = simple_path(call.callee()) else {
        return Err(runtime_expected_error(expr, FxRuntimeType::Color));
    };
    if !matches!(function, "rgb" | "rgba") {
        return Err(runtime_expected_error(expr, FxRuntimeType::Color));
    }
    let [CallArg::Positional(value)] = call.args() else {
        return Err(error(format!(
            "Fx `{function}` currently requires one hexadecimal string literal"
        )));
    };
    let Expr::Literal(Literal::String(hex)) = value.as_ref() else {
        return Err(error(format!(
            "Fx `{function}` currently requires one hexadecimal string literal"
        )));
    };
    parse_hex_color(hex, function == "rgba")
}

fn lower_vec2(expr: &Expr) -> Result<FxVec2, RuntimePlanLowerError> {
    let Expr::Call(call) = expr else {
        return Err(runtime_expected_error(expr, FxRuntimeType::Vec2));
    };
    if simple_path(call.callee()) != Some("vec2") {
        return Err(runtime_expected_error(expr, FxRuntimeType::Vec2));
    }
    let [first, second] = call.args() else {
        return Err(error("Fx vec2 requires exactly two positional values"));
    };
    let [CallArg::Positional(first), CallArg::Positional(second)] = [first, second] else {
        return Err(error("Fx vec2 accepts positional values only"));
    };
    Ok(FxVec2 {
        x: finite(lower_decimal(first)?)?,
        y: finite(lower_decimal(second)?)?,
    })
}

fn lower_transform(expr: &Expr) -> Result<Transform2D, RuntimePlanLowerError> {
    let Expr::Record { path, fields } = expr else {
        return Err(runtime_expected_error(expr, FxRuntimeType::Transform2D));
    };
    if path != "Transform2D" {
        return Err(runtime_expected_error(expr, FxRuntimeType::Transform2D));
    }
    let mut value = Transform2D::default();
    let mut seen = std::collections::BTreeSet::new();
    for (name, expr) in fields {
        if !seen.insert(name.as_str()) {
            return Err(error(format!("Transform2D repeats field `{name}`")));
        }
        match name.as_str() {
            "translate_x" => value.translate_x = runtime_length(expr)?,
            "translate_y" => value.translate_y = runtime_length(expr)?,
            "scale_x" => value.scale_x = finite(lower_decimal(expr)?)?,
            "scale_y" => value.scale_y = finite(lower_decimal(expr)?)?,
            "skew_x" => value.skew_x = lower_angle(expr)?,
            "skew_y" => value.skew_y = lower_angle(expr)?,
            "rotation" => value.rotation = lower_angle(expr)?,
            "origin_x" => value.origin_x = runtime_length(expr)?,
            "origin_y" => value.origin_y = runtime_length(expr)?,
            "opacity" => value.opacity = finite(lower_decimal(expr)?)?,
            _ => return Err(error(format!("Transform2D has no field `{name}`"))),
        }
    }
    value
        .validate()
        .map_err(|source| error(format!("invalid Transform2D: {source}")))?;
    Ok(value)
}

fn runtime_length(expr: &Expr) -> Result<Length, RuntimePlanLowerError> {
    match lower_runtime_constant(expr, FxRuntimeType::Length)? {
        FxRuntimeValue::Length(value) => Ok(value),
        _ => unreachable!("Length lowering returns Length"),
    }
}

fn signed_literal(expr: &Expr) -> Result<(bool, &Literal), RuntimePlanLowerError> {
    match expr {
        Expr::Unary {
            op: arcweft_lang_syntax::expr::UnaryOp::Neg,
            expr,
        } => match expr.as_ref() {
            Expr::Literal(literal) => Ok((true, literal)),
            _ => Err(error(format!(
                "Fx negation requires a literal, found `{}`",
                expr_label(expr)
            ))),
        },
        Expr::Literal(literal) => Ok((false, literal)),
        _ => Err(error(format!(
            "Fx runtime constant must be a literal, found `{}`",
            expr_label(expr)
        ))),
    }
}

fn strip_negation(expr: &Expr) -> &Expr {
    match expr {
        Expr::Unary {
            op: arcweft_lang_syntax::expr::UnaryOp::Neg,
            expr,
        } => expr,
        _ => expr,
    }
}

fn infer_signed_runtime_type(expr: &Expr) -> Result<FxRuntimeType, RuntimePlanLowerError> {
    match strip_negation(expr) {
        Expr::Literal(Literal::Int(_)) => Ok(FxRuntimeType::I32),
        Expr::Literal(Literal::Float { .. }) => Ok(FxRuntimeType::F32),
        Expr::Literal(Literal::UnitNumber { suffix, .. }) => match suffix {
            UnitNumberSuffix::Px => Ok(FxRuntimeType::Length),
            UnitNumberSuffix::Deg | UnitNumberSuffix::Rad | UnitNumberSuffix::Turn => {
                Ok(FxRuntimeType::Angle)
            }
            _ => Err(error(format!(
                "unit `{}` is not accepted by Fx runtime programs",
                suffix.as_str()
            ))),
        },
        Expr::Literal(Literal::Duration { .. }) => Ok(FxRuntimeType::Seconds),
        _ => Err(error(format!(
            "cannot infer Fx numeric type for `{}`",
            expr_label(expr)
        ))),
    }
}

fn lower_resource(expr: &Expr) -> Result<FxResourceId, RuntimePlanLowerError> {
    let value = match expr {
        Expr::EntityRef(entity) => entity.canonical_body(),
        Expr::Literal(Literal::String(value)) => value.clone(),
        _ => return Err(expected_error(expr, FxStaticType::Resource)),
    };
    FxResourceId::try_new(value).map_err(error)
}

fn lower_selector(expr: &Expr) -> Result<String, RuntimePlanLowerError> {
    match expr {
        Expr::ShortVariant(value) => Ok(value.as_str().to_owned()),
        Expr::Path(path) if path.segments().len() == 1 => Ok(path.as_label().to_owned()),
        _ => Err(expected_error(expr, FxStaticType::Selector)),
    }
}

fn lower_string(expr: &Expr) -> Result<String, RuntimePlanLowerError> {
    match expr {
        Expr::Literal(Literal::String(value)) => Ok(value.clone()),
        _ => Err(expected_error(expr, FxStaticType::String)),
    }
}

fn lower_target(expr: &Expr) -> Result<FxTarget, RuntimePlanLowerError> {
    match lower_selector(expr)?.as_str() {
        "node" => Ok(FxTarget::Node),
        "content" => Ok(FxTarget::Content),
        "background" => Ok(FxTarget::Background),
        "line" => Ok(FxTarget::Line),
        "glyph" => Ok(FxTarget::Glyph),
        "viewport" => Ok(FxTarget::Viewport),
        value => Err(error(format!("unknown Fx target `.{value}`"))),
    }
}

fn lower_phase(expr: &Expr) -> Result<FxPhase, RuntimePlanLowerError> {
    match lower_selector(expr)?.as_str() {
        "before_layout" => Ok(FxPhase::BeforeLayout),
        "layout_transform" => Ok(FxPhase::LayoutTransform),
        "glyph_transform" => Ok(FxPhase::GlyphTransform),
        "glyph_color" => Ok(FxPhase::GlyphColor),
        "glyph_mask" => Ok(FxPhase::GlyphMask),
        "offscreen_pass" | "run_offscreen_pass" => Ok(FxPhase::OffscreenPass),
        "post_process" => Ok(FxPhase::PostProcess),
        "transition" => Ok(FxPhase::Transition),
        value => Err(error(format!("unknown Fx phase `.{value}`"))),
    }
}

fn parse_hex_color(value: &str, require_alpha: bool) -> Result<FxColor, RuntimePlanLowerError> {
    let value = value.strip_prefix('#').unwrap_or(value);
    let channels = match value.len() {
        3 if !require_alpha => value
            .chars()
            .map(|digit| format!("{digit}{digit}"))
            .map(|pair| u8::from_str_radix(&pair, 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| hex_error(&source))?,
        4 => value
            .chars()
            .map(|digit| format!("{digit}{digit}"))
            .map(|pair| u8::from_str_radix(&pair, 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| hex_error(&source))?,
        6 if !require_alpha => (0..3)
            .map(|index| u8::from_str_radix(&value[index * 2..index * 2 + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| hex_error(&source))?,
        8 => (0..4)
            .map(|index| u8::from_str_radix(&value[index * 2..index * 2 + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| hex_error(&source))?,
        _ => {
            return Err(error(
                "Fx color requires #rgb, #rgba, #rrggbb, or #rrggbbaa",
            ));
        }
    };
    let alpha = channels.get(3).copied().unwrap_or(255);
    Ok(FxColor::new(
        color_channel(channels[0])?,
        color_channel(channels[1])?,
        color_channel(channels[2])?,
        color_channel(alpha)?,
    ))
}

fn color_channel(value: u8) -> Result<Opacity, RuntimePlanLowerError> {
    Opacity::try_new(finite(f64::from(value) / 255.0)?)
        .map_err(|source| error(format!("invalid Fx color channel: {source}")))
}

fn hex_error(source: &std::num::ParseIntError) -> RuntimePlanLowerError {
    error(format!("invalid hexadecimal Fx color: {source}"))
}

fn parse_decimal(value: &str) -> Result<f64, RuntimePlanLowerError> {
    value
        .replace('_', "")
        .parse::<f64>()
        .map_err(|source| error(format!("invalid Fx decimal literal `{value}`: {source}")))
}

fn finite(value: f64) -> Result<FiniteF32, RuntimePlanLowerError> {
    FiniteF32::try_from_f64(value).map_err(number_error)
}

fn number_error(source: impl std::fmt::Display) -> RuntimePlanLowerError {
    error(format!("invalid Fx numeric value: {source}"))
}

fn expected_error(expr: &Expr, expected: FxStaticType) -> RuntimePlanLowerError {
    error(format!(
        "Fx value `{}` does not satisfy expected type {}",
        expr_label(expr),
        expected.as_str()
    ))
}

fn runtime_expected_error(expr: &Expr, expected: FxRuntimeType) -> RuntimePlanLowerError {
    expected_error(expr, FxStaticType::Runtime(expected))
}

fn simple_path(expr: &Expr) -> Option<&str> {
    let Expr::Path(path) = expr else {
        return None;
    };
    (path.segments().len() == 1).then(|| path.as_label())
}

fn error(message: impl Into<String>) -> RuntimePlanLowerError {
    RuntimePlanLowerError::new(format!("Fx: {}", message.into()))
}
