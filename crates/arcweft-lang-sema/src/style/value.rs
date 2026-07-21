//! Expected-kind checking for native Style expressions.

use std::collections::BTreeMap;

use arcweft_id::PublicId;
use arcweft_lang_hir::style::HirStyleExpr;
use arcweft_lang_syntax::{
    ast::common::TextRange,
    expr::{CallArg, DurationUnit, Expr, Literal, UnaryOp, UnitNumberSuffix},
    types::TypeRef,
};
use arcweft_presentation::appearance::{PresentationColor, SystemColor};
use arcweft_view::style::{
    ViewAlignment, ViewAngleMilliDegrees, ViewBlendMode, ViewBorderRadii, ViewBoxAxisMode,
    ViewClip, ViewColorValue, ViewDisplay, ViewFilter, ViewFlexDirection, ViewFlexWrap,
    ViewFontFamily, ViewFontFamilyList, ViewFontStyle, ViewFontWeight, ViewLengthMilli, ViewMask,
    ViewOverflow, ViewPosition, ViewPropertyKind, ViewRatioMilli, ViewScalarMilli, ViewShadow,
    ViewSpecifiedValue, ViewStyleTokenId, ViewStyleTransition, ViewStyleValueKind,
    ViewSystemFontFamily,
};

use super::diagnostic::{StyleDiagnostic, StyleDiagnosticCode};

const FIXED_MILLI_NANOS_PER_MILLISECOND: u64 = 1_000_000_000;

pub(crate) type CheckedTokenKinds = BTreeMap<String, (ViewStyleTokenId, ViewStyleValueKind)>;

pub(crate) fn annotation_kind(value: &TypeRef) -> Option<ViewStyleValueKind> {
    let TypeRef::Path(name) = value else {
        return None;
    };
    crate::types::direct_type_name(name).and_then(ViewStyleValueKind::from_source_name)
}

pub(crate) fn infer_value_kind(
    value: &HirStyleExpr,
    tokens: &CheckedTokenKinds,
) -> Option<ViewStyleValueKind> {
    let expr = value.expr();
    if let Some(token) = token_reference(expr) {
        return tokens.get(&token).map(|(_, kind)| *kind);
    }
    match expr {
        Expr::Literal(Literal::Bool(_)) => Some(ViewStyleValueKind::Bool),
        Expr::Literal(Literal::Int(_)) => Some(ViewStyleValueKind::Integer),
        Expr::Literal(Literal::Float { .. }) => Some(ViewStyleValueKind::Scalar),
        Expr::Literal(Literal::UnitNumber { suffix, .. }) => match suffix {
            UnitNumberSuffix::Milli => Some(ViewStyleValueKind::Scalar),
            UnitNumberSuffix::Percent => Some(ViewStyleValueKind::Ratio),
            UnitNumberSuffix::Px
            | UnitNumberSuffix::Pt
            | UnitNumberSuffix::Em
            | UnitNumberSuffix::Rem
            | UnitNumberSuffix::Vw
            | UnitNumberSuffix::Vh => Some(ViewStyleValueKind::Length),
            UnitNumberSuffix::Deg | UnitNumberSuffix::Rad | UnitNumberSuffix::Turn => {
                Some(ViewStyleValueKind::Angle)
            }
            UnitNumberSuffix::Db
            | UnitNumberSuffix::Lufs
            | UnitNumberSuffix::Bpm
            | UnitNumberSuffix::Bars => None,
        },
        Expr::Call(call) => match call.callee().dotted_selector_label().as_deref() {
            Some("rgba" | "system_color") => Some(ViewStyleValueKind::Color),
            Some("resource") => Some(ViewStyleValueKind::Resource),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn check_value(
    value: &HirStyleExpr,
    expected: ViewStyleValueKind,
    tokens: &CheckedTokenKinds,
) -> Result<ViewSpecifiedValue, StyleDiagnostic> {
    if let Some(token) = token_reference(value.expr()) {
        let Some((token_id, actual)) = tokens.get(&token) else {
            return Err(StyleDiagnostic::new(
                StyleDiagnosticCode::UnresolvedToken,
                format!("unknown style token `{token}`"),
                value.range(),
            )
            .with_subject(token));
        };
        if *actual != expected {
            return Err(type_mismatch(value.range(), expected, *actual));
        }
        return Ok(ViewSpecifiedValue::Token {
            token: token_id.clone(),
            value_kind: expected,
        });
    }

    let checked = match expected {
        ViewStyleValueKind::BoxAxes => box_axis_value(value.expr()),
        ViewStyleValueKind::Bool => bool_value(value.expr()),
        ViewStyleValueKind::Integer => {
            integer_value(value.expr()).map(|value| ViewSpecifiedValue::Integer { value })
        }
        ViewStyleValueKind::Ratio => {
            ratio_value(value.expr()).map(|value| ViewSpecifiedValue::Ratio { value })
        }
        ViewStyleValueKind::Scalar => {
            scalar_value(value.expr()).map(|value| ViewSpecifiedValue::Scalar { value })
        }
        ViewStyleValueKind::Length => {
            length_value(value.expr()).map(|value| ViewSpecifiedValue::Length { value })
        }
        ViewStyleValueKind::Angle => {
            angle_value(value.expr()).map(|value| ViewSpecifiedValue::Angle { value })
        }
        ViewStyleValueKind::Color => {
            color_value(value.expr()).map(|value| ViewSpecifiedValue::Color { value })
        }
        ViewStyleValueKind::FontFamilyList => {
            font_family_list(value.expr()).map(|value| ViewSpecifiedValue::FontFamilyList { value })
        }
        ViewStyleValueKind::FontWeight => integer_value(value.expr())
            .and_then(|value| u16::try_from(value).ok())
            .and_then(ViewFontWeight::new)
            .map(|value| ViewSpecifiedValue::FontWeight { value }),
        ViewStyleValueKind::FontStyle => enum_name(value.expr())
            .and_then(ViewFontStyle::from_source_name)
            .map(|value| ViewSpecifiedValue::FontStyle { value }),
        ViewStyleValueKind::Display => enum_name(value.expr())
            .and_then(ViewDisplay::from_source_name)
            .map(|value| ViewSpecifiedValue::Display { value }),
        ViewStyleValueKind::Position => enum_name(value.expr())
            .and_then(ViewPosition::from_source_name)
            .map(|value| ViewSpecifiedValue::Position { value }),
        ViewStyleValueKind::Overflow => enum_name(value.expr())
            .and_then(ViewOverflow::from_source_name)
            .map(|value| ViewSpecifiedValue::Overflow { value }),
        ViewStyleValueKind::FlexDirection => enum_name(value.expr())
            .and_then(ViewFlexDirection::from_source_name)
            .map(|value| ViewSpecifiedValue::FlexDirection { value }),
        ViewStyleValueKind::FlexWrap => enum_name(value.expr())
            .and_then(ViewFlexWrap::from_source_name)
            .map(|value| ViewSpecifiedValue::FlexWrap { value }),
        ViewStyleValueKind::Alignment => enum_name(value.expr())
            .and_then(ViewAlignment::from_source_name)
            .map(|value| ViewSpecifiedValue::Alignment { value }),
        ViewStyleValueKind::BorderRadii => {
            length_value(value.expr()).map(|radius| ViewSpecifiedValue::BorderRadii {
                value: ViewBorderRadii {
                    top_left: radius,
                    top_right: radius,
                    bottom_right: radius,
                    bottom_left: radius,
                },
            })
        }
        ViewStyleValueKind::ShadowList => {
            shadow_list(value.expr()).map(|value| ViewSpecifiedValue::ShadowList { value })
        }
        ViewStyleValueKind::FilterList => {
            filter_list(value.expr()).map(|value| ViewSpecifiedValue::FilterList { value })
        }
        ViewStyleValueKind::Clip => {
            clip_value(value.expr()).map(|value| ViewSpecifiedValue::Clip { value })
        }
        ViewStyleValueKind::Mask => {
            mask_value(value.expr()).map(|value| ViewSpecifiedValue::Mask { value })
        }
        ViewStyleValueKind::BlendMode => enum_name(value.expr())
            .and_then(ViewBlendMode::from_source_name)
            .map(|value| ViewSpecifiedValue::BlendMode { value }),
        ViewStyleValueKind::Transition => {
            transition_list(value.expr()).map(|value| ViewSpecifiedValue::Transition { value })
        }
        ViewStyleValueKind::Resource => {
            resource_value(value.expr()).map(|value| ViewSpecifiedValue::Resource { value })
        }
    };

    checked.ok_or_else(|| invalid_value_diagnostic(value, expected, tokens))
}

fn invalid_value_diagnostic(
    value: &HirStyleExpr,
    expected: ViewStyleValueKind,
    tokens: &CheckedTokenKinds,
) -> StyleDiagnostic {
    let accepted_units = accepted_units(expected);
    if let Some(unit) = unit_name(value.expr())
        && !accepted_units.is_empty()
        && !accepted_units.contains(&unit)
    {
        return StyleDiagnostic::new(
            StyleDiagnosticCode::InvalidUnit,
            format!("style value of kind {expected:?} does not accept unit `{unit}`"),
            value.range(),
        )
        .with_subject(unit)
        .with_accepted_units(
            accepted_units
                .iter()
                .map(|unit| (*unit).to_owned())
                .collect(),
        );
    }
    if numeric_conversion_overflowed(value.expr(), expected) {
        return StyleDiagnostic::new(
            StyleDiagnosticCode::NonFiniteValue,
            format!("style value of kind {expected:?} exceeds its finite fixed-point range"),
            value.range(),
        )
        .with_types(expected.source_name(), "non-finite or overflowing number");
    }

    let actual = infer_value_kind(value, tokens).map_or_else(
        || "unsupported expression".to_owned(),
        |kind| kind.source_name().to_owned(),
    );
    let diagnostic = StyleDiagnostic::new(
        StyleDiagnosticCode::InvalidValueType,
        format!("style value must have kind {expected:?}, found {actual}"),
        value.range(),
    )
    .with_types(expected.source_name(), actual);
    if expected == ViewStyleValueKind::BoxAxes {
        diagnostic.with_valid_inventory(
            ViewBoxAxisMode::ALL
                .iter()
                .map(|mode| mode.source_name().to_owned())
                .collect(),
        )
    } else {
        diagnostic
    }
}

fn accepted_units(expected: ViewStyleValueKind) -> &'static [&'static str] {
    match expected {
        ViewStyleValueKind::Ratio => &["milli", "%"],
        ViewStyleValueKind::Scalar => &["unitless", "milli", "%"],
        ViewStyleValueKind::Length | ViewStyleValueKind::BorderRadii => &["px"],
        ViewStyleValueKind::Angle => &["deg"],
        ViewStyleValueKind::BoxAxes
        | ViewStyleValueKind::Bool
        | ViewStyleValueKind::Integer
        | ViewStyleValueKind::Color
        | ViewStyleValueKind::FontFamilyList
        | ViewStyleValueKind::FontWeight
        | ViewStyleValueKind::FontStyle
        | ViewStyleValueKind::Display
        | ViewStyleValueKind::Position
        | ViewStyleValueKind::Overflow
        | ViewStyleValueKind::FlexDirection
        | ViewStyleValueKind::FlexWrap
        | ViewStyleValueKind::Alignment
        | ViewStyleValueKind::ShadowList
        | ViewStyleValueKind::FilterList
        | ViewStyleValueKind::Clip
        | ViewStyleValueKind::Mask
        | ViewStyleValueKind::BlendMode
        | ViewStyleValueKind::Transition
        | ViewStyleValueKind::Resource => &[],
    }
}

fn unit_name(expr: &Expr) -> Option<&'static str> {
    let expr = match expr {
        Expr::Unary { expr, .. } => expr.as_ref(),
        expr => expr,
    };
    let Expr::Literal(Literal::UnitNumber { suffix, .. }) = expr else {
        return None;
    };
    Some(suffix.as_str())
}

fn numeric_conversion_overflowed(expr: &Expr, expected: ViewStyleValueKind) -> bool {
    let expr = match expr {
        Expr::Unary { expr, .. } => expr.as_ref(),
        expr => expr,
    };
    match (expected, expr) {
        (ViewStyleValueKind::Scalar, Expr::Literal(Literal::Float { raw, suffix: None })) => {
            raw.replace('_', "")
                .parse::<f64>()
                .is_ok_and(|value| !value.is_finite())
                || fixed_milli(raw).is_none()
        }
        (
            ViewStyleValueKind::Scalar,
            Expr::Literal(Literal::UnitNumber {
                raw,
                suffix: UnitNumberSuffix::Milli,
            }),
        ) => raw
            .strip_suffix("milli")
            .is_none_or(|raw| raw.replace('_', "").parse::<u32>().is_err()),
        (
            ViewStyleValueKind::Ratio | ViewStyleValueKind::Scalar,
            Expr::Literal(Literal::UnitNumber {
                raw,
                suffix: UnitNumberSuffix::Percent,
            }),
        ) => raw
            .strip_suffix('%')
            .is_none_or(|raw| fixed_milli(raw).is_none()),
        (
            ViewStyleValueKind::Length | ViewStyleValueKind::BorderRadii,
            Expr::Literal(Literal::UnitNumber {
                raw,
                suffix: UnitNumberSuffix::Px,
            }),
        ) => raw
            .strip_suffix("px")
            .is_none_or(|raw| fixed_milli(raw).is_none()),
        (
            ViewStyleValueKind::Angle,
            Expr::Literal(Literal::UnitNumber {
                raw,
                suffix: UnitNumberSuffix::Deg,
            }),
        ) => raw
            .strip_suffix("deg")
            .is_none_or(|raw| fixed_milli(raw).is_none()),
        _ => false,
    }
}

fn bool_value(expr: &Expr) -> Option<ViewSpecifiedValue> {
    match expr {
        Expr::Literal(Literal::Bool(value)) => Some(ViewSpecifiedValue::Bool { value: *value }),
        _ => None,
    }
}

fn box_axis_value(expr: &Expr) -> Option<ViewSpecifiedValue> {
    enum_name(expr)
        .and_then(ViewBoxAxisMode::from_source_name)
        .map(|value| ViewSpecifiedValue::BoxAxes { value })
}

fn integer_value(expr: &Expr) -> Option<i32> {
    match expr {
        Expr::Literal(Literal::Int(value)) => i32::try_from(value.magnitude().ok()?).ok(),
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => integer_value(expr)?.checked_neg(),
        _ => None,
    }
}

fn ratio_value(expr: &Expr) -> Option<ViewRatioMilli> {
    match expr {
        Expr::Literal(Literal::UnitNumber {
            raw,
            suffix: UnitNumberSuffix::Milli,
        }) => raw
            .strip_suffix("milli")?
            .replace('_', "")
            .parse::<u16>()
            .ok()
            .and_then(ViewRatioMilli::new),
        Expr::Literal(Literal::UnitNumber {
            raw,
            suffix: UnitNumberSuffix::Percent,
        }) => {
            let percent = fixed_milli(raw.strip_suffix('%')?)?;
            let milli = percent.checked_div(100)?;
            u16::try_from(milli).ok().and_then(ViewRatioMilli::new)
        }
        _ => None,
    }
}

fn scalar_value(expr: &Expr) -> Option<ViewScalarMilli> {
    let value = match expr {
        Expr::Literal(Literal::Int(_)) => u32::try_from(integer_value(expr)?)
            .ok()?
            .checked_mul(ViewScalarMilli::ONE.value())?,
        Expr::Literal(Literal::Float { raw, suffix: None }) => {
            u32::try_from(fixed_milli(raw)?).ok()?
        }
        Expr::Literal(Literal::UnitNumber {
            raw,
            suffix: UnitNumberSuffix::Milli,
        }) => raw
            .strip_suffix("milli")?
            .replace('_', "")
            .parse::<u32>()
            .ok()?,
        Expr::Literal(Literal::UnitNumber {
            raw,
            suffix: UnitNumberSuffix::Percent,
        }) => u32::try_from(fixed_milli(raw.strip_suffix('%')?)?.checked_div(100)?).ok()?,
        _ => return None,
    };
    Some(ViewScalarMilli::new(value))
}

fn length_value(expr: &Expr) -> Option<ViewLengthMilli> {
    let negative = matches!(
        expr,
        Expr::Unary {
            op: UnaryOp::Neg,
            ..
        }
    );
    let expr = match expr {
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => expr.as_ref(),
        expr => expr,
    };
    let Expr::Literal(Literal::UnitNumber {
        raw,
        suffix: UnitNumberSuffix::Px,
    }) = expr
    else {
        return None;
    };
    let value = signed_fixed_milli(raw.strip_suffix("px")?, negative)?;
    Some(ViewLengthMilli::new(value))
}

fn angle_value(expr: &Expr) -> Option<ViewAngleMilliDegrees> {
    let sign = if matches!(
        expr,
        Expr::Unary {
            op: UnaryOp::Neg,
            ..
        }
    ) {
        -1
    } else {
        1
    };
    let expr = match expr {
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => expr.as_ref(),
        expr => expr,
    };
    let Expr::Literal(Literal::UnitNumber {
        raw,
        suffix: UnitNumberSuffix::Deg,
    }) = expr
    else {
        return None;
    };
    let value = fixed_milli(raw.strip_suffix("deg")?)?.checked_mul(sign)?;
    Some(ViewAngleMilliDegrees::new(value))
}

fn color_value(expr: &Expr) -> Option<ViewColorValue> {
    let Expr::Call(call) = expr else {
        return None;
    };
    match call.callee().dotted_selector_label().as_deref()? {
        "rgba" => {
            let channels = positional_args(call.args())
                .map(integer_value)
                .collect::<Option<Vec<_>>>()?;
            let [red, green, blue, alpha] = channels.as_slice() else {
                return None;
            };
            Some(ViewColorValue::Literal {
                color: PresentationColor::rgba(
                    u8::try_from(*red).ok()?,
                    u8::try_from(*green).ok()?,
                    u8::try_from(*blue).ok()?,
                    u8::try_from(*alpha).ok()?,
                ),
            })
        }
        "system_color" => {
            let role = positional_args(call.args()).next().and_then(enum_name)?;
            SystemColor::from_source_name(role).map(|role| ViewColorValue::System { role })
        }
        _ => None,
    }
}

fn font_family_list(expr: &Expr) -> Option<ViewFontFamilyList> {
    let Expr::BracketSeq(items) = expr else {
        return None;
    };
    let families = items
        .iter()
        .map(|item| match item {
            Expr::Literal(Literal::String(name)) => ViewFontFamily::named(name.clone()),
            Expr::Call(call)
                if call.callee().dotted_selector_label().as_deref() == Some("system_font") =>
            {
                positional_args(call.args())
                    .next()
                    .and_then(enum_name)
                    .and_then(ViewSystemFontFamily::from_source_name)
                    .map(ViewFontFamily::system)
            }
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    ViewFontFamilyList::new(families)
}

fn shadow_list(expr: &Expr) -> Option<Vec<ViewShadow>> {
    let Expr::BracketSeq(items) = expr else {
        return None;
    };
    items.iter().map(shadow_value).collect()
}

fn shadow_value(expr: &Expr) -> Option<ViewShadow> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let name = call.callee().dotted_selector_label()?;
    let inset = match name.as_str() {
        "shadow" => false,
        "inset_shadow" => true,
        _ => return None,
    };
    Some(ViewShadow {
        x: named_arg(call.args(), "x").and_then(length_value)?,
        y: named_arg(call.args(), "y").and_then(length_value)?,
        blur: named_arg(call.args(), "blur").and_then(length_value)?,
        spread: named_arg(call.args(), "spread").and_then(length_value)?,
        color: named_arg(call.args(), "color").and_then(color_value)?,
        inset,
    })
}

fn filter_list(expr: &Expr) -> Option<Vec<ViewFilter>> {
    let Expr::BracketSeq(items) = expr else {
        return None;
    };
    items
        .iter()
        .map(|item| {
            let Expr::Call(call) = item else {
                return None;
            };
            let argument = positional_args(call.args()).next()?;
            match call.callee().dotted_selector_label().as_deref()? {
                "blur" => length_value(argument).map(|radius| ViewFilter::Blur { radius }),
                "brightness" => {
                    scalar_value(argument).map(|amount| ViewFilter::Brightness { amount })
                }
                "contrast" => scalar_value(argument).map(|amount| ViewFilter::Contrast { amount }),
                "opacity" => ratio_value(argument).map(|amount| ViewFilter::Opacity { amount }),
                _ => None,
            }
        })
        .collect()
}

fn clip_value(expr: &Expr) -> Option<ViewClip> {
    if let Some(name) = enum_name(expr) {
        return ViewClip::from_source_name(name);
    }
    let Expr::Call(call) = expr else {
        return None;
    };
    if call.callee().dotted_selector_label().as_deref() != Some("rounded_rect") {
        return None;
    }
    let radius = named_arg(call.args(), "radius")
        .or_else(|| positional_args(call.args()).next())
        .and_then(length_value)?;
    Some(ViewClip::RoundedRect(ViewBorderRadii {
        top_left: radius,
        top_right: radius,
        bottom_right: radius,
        bottom_left: radius,
    }))
}

fn mask_value(expr: &Expr) -> Option<ViewMask> {
    enum_name(expr)
        .and_then(ViewMask::from_source_name)
        .or_else(|| resource_value(expr).map(ViewMask::Resource))
}

fn transition_list(expr: &Expr) -> Option<Vec<ViewStyleTransition>> {
    if enum_name(expr).is_some_and(|name| name == "None") {
        return Some(Vec::new());
    }
    let Expr::BracketSeq(items) = expr else {
        return None;
    };
    items.iter().map(transition_value).collect()
}

fn transition_value(expr: &Expr) -> Option<ViewStyleTransition> {
    let Expr::Call(call) = expr else {
        return None;
    };
    if call.callee().dotted_selector_label().as_deref() != Some("transition") {
        return None;
    }
    let property = match named_arg(call.args(), "property")? {
        Expr::Literal(Literal::String(name)) => ViewPropertyKind::from_source_name(name)?,
        value => ViewPropertyKind::from_source_name(value.dotted_selector_label()?.as_str())?,
    };
    let duration = named_arg(call.args(), "duration").and_then(duration_millis)?;
    let delay = named_arg(call.args(), "delay").map_or(Some(0), duration_millis)?;
    ViewStyleTransition::new(property, duration, delay)
}

fn duration_millis(expr: &Expr) -> Option<u32> {
    let Expr::Literal(Literal::Duration { amount, unit }) = expr else {
        return None;
    };
    let amount_milli = u64::try_from(fixed_milli(amount)?).ok()?;
    let nanos_per_unit = match unit {
        DurationUnit::Nanos => 1,
        DurationUnit::Micros => 1_000,
        DurationUnit::Millis => 1_000_000,
        DurationUnit::Seconds => 1_000_000_000,
        DurationUnit::Minutes => 60_000_000_000,
        DurationUnit::Hours => 3_600_000_000_000,
    };
    let numerator = amount_milli.checked_mul(nanos_per_unit)?;
    if numerator % FIXED_MILLI_NANOS_PER_MILLISECOND != 0 {
        return None;
    }
    u32::try_from(numerator / FIXED_MILLI_NANOS_PER_MILLISECOND).ok()
}

fn token_reference(expr: &Expr) -> Option<String> {
    let Expr::Call(call) = expr else {
        return None;
    };
    if call.callee().dotted_selector_label().as_deref() != Some("token") {
        return None;
    }
    positional_args(call.args()).next()?.dotted_selector_label()
}

fn resource_value(expr: &Expr) -> Option<PublicId> {
    let Expr::Call(call) = expr else {
        return None;
    };
    if call.callee().dotted_selector_label().as_deref() != Some("resource") {
        return None;
    }
    PublicId::try_new(
        positional_args(call.args())
            .next()?
            .dotted_selector_label()?,
    )
    .ok()
}

fn enum_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::ShortVariant(name) => Some(name.as_str()),
        Expr::Path(path) => Some(path.as_label()),
        _ => None,
    }
}

fn positional_args(args: &[CallArg]) -> impl Iterator<Item = &Expr> {
    args.iter().filter_map(|arg| match arg {
        CallArg::Positional(value) => Some(value),
        CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })
}

fn named_arg<'a>(args: &'a [CallArg], expected: &str) -> Option<&'a Expr> {
    args.iter().find_map(|arg| match arg {
        CallArg::Named { name, value } if name == expected => Some(value.as_ref()),
        _ => None,
    })
}

fn fixed_milli(source: &str) -> Option<i32> {
    let source = source.replace('_', "");
    let (whole, fraction) = source.split_once('.').unwrap_or((&source, ""));
    if fraction.len() > 3 || whole.is_empty() || !fraction.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let whole = whole.parse::<i32>().ok()?.checked_mul(1_000)?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<i32>().ok()? * 10_i32.pow(u32::try_from(3 - fraction.len()).ok()?)
    };
    whole.checked_add(fraction)
}

fn signed_fixed_milli(source: &str, negative: bool) -> Option<i32> {
    let source = source.replace('_', "");
    let (whole, fraction) = source.split_once('.').unwrap_or((&source, ""));
    if fraction.len() > 3 || whole.is_empty() || !fraction.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let whole = whole.parse::<i64>().ok()?.checked_mul(1_000)?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<i64>().ok()? * 10_i64.pow(u32::try_from(3 - fraction.len()).ok()?)
    };
    let magnitude = whole.checked_add(fraction)?;
    i32::try_from(if negative {
        magnitude.checked_neg()?
    } else {
        magnitude
    })
    .ok()
}

fn type_mismatch(
    range: TextRange,
    expected: ViewStyleValueKind,
    actual: ViewStyleValueKind,
) -> StyleDiagnostic {
    StyleDiagnostic::new(
        StyleDiagnosticCode::TokenTypeMismatch,
        format!("style token has kind {actual:?}, expected {expected:?}"),
        range,
    )
    .with_types(format!("{expected:?}"), format!("{actual:?}"))
}
