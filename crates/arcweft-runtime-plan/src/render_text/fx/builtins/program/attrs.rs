//! Typed parsing and validation for built-in rich-text Fx attributes.

use std::collections::BTreeMap;

use arcweft_presentation::fx::{FiniteF32, FxColor, FxRuntimeValue, FxStaticValue, FxVec2, Length};

use crate::errors::RuntimePlanLowerError;

use super::super::fx_error;

pub(super) fn validate_symbolic_origin(
    attrs: &BTreeMap<String, String>,
) -> Result<(), RuntimePlanLowerError> {
    if let Some(origin) = attrs.get("origin")
        && !matches!(origin.as_str(), "glyph_center" | "glyph" | "center")
    {
        return Err(fx_error(format!(
            "built-in animated transform origin `{origin}` is unsupported"
        )));
    }
    Ok(())
}

pub(super) fn direction(
    attrs: &BTreeMap<String, String>,
    default: [f32; 2],
) -> Result<FxVec2, RuntimePlanLowerError> {
    let value = if let Some(value) = attrs.get("dir") {
        let (x, y) = value
            .split_once(',')
            .ok_or_else(|| fx_error("direction must contain `x,y`"))?;
        [parse_f32(x, "direction x")?, parse_f32(y, "direction y")?]
    } else if let Some(axis) = attrs.get("axis") {
        match axis.trim().trim_start_matches('.') {
            "x" => [1.0, 0.0],
            "y" => [0.0, 1.0],
            value => return Err(fx_error(format!("unknown effect axis `{value}`"))),
        }
    } else {
        default
    };
    if value[0].hypot(value[1]) <= f32::EPSILON {
        return Err(fx_error("effect direction must be non-zero"));
    }
    Ok(FxVec2 {
        x: finite(value[0], "direction x")?,
        y: finite(value[1], "direction y")?,
    })
}

pub(super) fn number(
    attrs: &BTreeMap<String, String>,
    name: &'static str,
    default: f32,
) -> Result<f32, RuntimePlanLowerError> {
    attrs.get(name).map_or(Ok(default), |value| {
        let value = ["px", "deg", "ch"]
            .iter()
            .find_map(|suffix| value.strip_suffix(suffix))
            .unwrap_or(value);
        parse_f32(value, name)
    })
}

pub(super) fn optional_number(
    attrs: &BTreeMap<String, String>,
    name: &'static str,
) -> Result<Option<f32>, RuntimePlanLowerError> {
    attrs
        .get(name)
        .map(|_| number(attrs, name, 0.0))
        .transpose()
}

pub(super) fn positive_number(
    attrs: &BTreeMap<String, String>,
    name: &'static str,
    default: f32,
) -> Result<f32, RuntimePlanLowerError> {
    let value = number(attrs, name, default)?;
    if value > 0.0 {
        Ok(value)
    } else {
        Err(fx_error(format!("effect `{name}` must be positive")))
    }
}

pub(super) fn alias_number(
    attrs: &BTreeMap<String, String>,
    name: &'static str,
    alias: &'static str,
    default: f32,
) -> Result<f32, RuntimePlanLowerError> {
    if attrs.contains_key(name) {
        number(attrs, name, default)
    } else {
        number(attrs, alias, default)
    }
}

pub(super) fn alias_seconds(
    attrs: &BTreeMap<String, String>,
    name: &'static str,
    alias: &'static str,
    default: f32,
) -> Result<f32, RuntimePlanLowerError> {
    let value = attrs.get(name).or_else(|| attrs.get(alias));
    value.map_or(Ok(default), |value| {
        if let Some(value) = value.strip_suffix("ms") {
            parse_f32(value, name).map(|value| value / 1_000.0)
        } else if let Some(value) = value.strip_suffix('s') {
            parse_f32(value, name)
        } else {
            parse_f32(value, name)
        }
    })
}

pub(super) fn bool_attr(
    attrs: &BTreeMap<String, String>,
    name: &'static str,
) -> Result<Option<bool>, RuntimePlanLowerError> {
    attrs
        .get(name)
        .map_or(Ok(None), |value| match value.as_str() {
            "true" => Ok(Some(true)),
            "false" => Ok(Some(false)),
            _ => Err(fx_error(format!("effect `{name}` must be true or false"))),
        })
}

pub(super) fn non_negative(value: f32, name: &'static str) -> Result<f32, RuntimePlanLowerError> {
    if value >= 0.0 {
        Ok(value)
    } else {
        Err(fx_error(format!("effect `{name}` must be non-negative")))
    }
}

fn parse_f32(value: &str, name: &str) -> Result<f32, RuntimePlanLowerError> {
    value
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| fx_error(format!("effect `{name}` is not a finite number")))
}

pub(super) fn parse_color(value: &str) -> Result<FxColor, RuntimePlanLowerError> {
    let value = value.trim().trim_matches(['"', '\'']);
    let channels = match value.to_ascii_lowercase().as_str() {
        "red" => [255, 0, 0, 255],
        "green" => [0, 128, 0, 255],
        "blue" => [0, 0, 255, 255],
        "white" => [255, 255, 255, 255],
        "black" => [0, 0, 0, 255],
        _ => {
            let hex = value
                .strip_prefix('#')
                .ok_or_else(|| fx_error(format!("unsupported shader color `{value}`")))?;
            if hex.len() != 6 {
                return Err(fx_error(format!("shader color `{value}` must use #RRGGBB")));
            }
            let channel = |range: std::ops::Range<usize>| {
                u8::from_str_radix(&hex[range], 16)
                    .map_err(|_| fx_error(format!("shader color `{value}` is not hexadecimal")))
            };
            [channel(0..2)?, channel(2..4)?, channel(4..6)?, 255]
        }
    };
    Ok(FxColor::from_rgba8(channels))
}

pub(super) fn authored_seed(attrs: &BTreeMap<String, String>) -> i32 {
    attrs.get("seed").map_or(0, |value| {
        let hash = value.as_bytes().iter().fold(0x811c_9dc5_u32, |hash, byte| {
            (hash ^ u32::from(*byte)).wrapping_mul(0x0100_0193)
        });
        i32::from_ne_bytes(hash.to_ne_bytes())
    })
}

pub(super) fn finite(value: f32, name: &str) -> Result<FiniteF32, RuntimePlanLowerError> {
    FiniteF32::try_new(value).map_err(|error| fx_error(format!("invalid `{name}`: {error}")))
}

pub(super) fn static_f32(value: f32) -> Result<FxStaticValue, RuntimePlanLowerError> {
    Ok(FxStaticValue::Runtime(FxRuntimeValue::F32(finite(
        value, "f32",
    )?)))
}

pub(super) fn static_length(value: f32) -> Result<FxStaticValue, RuntimePlanLowerError> {
    Ok(FxStaticValue::Runtime(FxRuntimeValue::Length(
        Length::try_pixels(value).map_err(|error| fx_error(format!("invalid length: {error}")))?,
    )))
}
