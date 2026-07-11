//! Layout-space reservations for effects that participate before rendering.

use arcweft_render_text::{
    Milli, RichTextEffectDescriptor, RichTextEffectPhase, RichTextParam, RichTextPresentation,
    parse_decimal_milli,
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct LayoutEffectReserve {
    pub(crate) x: f32,
    pub(crate) y: f32,
}

impl LayoutEffectReserve {
    fn union(self, other: Self) -> Self {
        Self {
            x: self.x.max(other.x),
            y: self.y.max(other.y),
        }
    }
}

pub(crate) fn layout_phase_effect_reserve(
    presentation: &RichTextPresentation,
) -> LayoutEffectReserve {
    presentation
        .effects
        .iter()
        .filter(|effect| {
            matches!(
                effect.phase,
                RichTextEffectPhase::BeforeLayout | RichTextEffectPhase::LayoutTransform
            )
        })
        .map(layout_builtin_effect_reserve)
        .fold(LayoutEffectReserve::default(), LayoutEffectReserve::union)
}

fn layout_builtin_effect_reserve(effect: &RichTextEffectDescriptor) -> LayoutEffectReserve {
    match effect.id.as_str() {
        "wave" => {
            let amplitude = effect_param_milli(effect, "amp")
                .unwrap_or(Milli(4000))
                .as_f32()
                .abs();
            let direction = effect_param_vec2(effect, "dir")
                .or_else(|| effect_axis_direction(effect))
                .unwrap_or([0.0, 1.0]);
            LayoutEffectReserve {
                x: amplitude * direction[0].abs(),
                y: amplitude * direction[1].abs(),
            }
        }
        "shake" | "jitter" => {
            let amplitude = effect_param_milli(effect, "amp")
                .unwrap_or(Milli(2000))
                .as_f32()
                .abs();
            LayoutEffectReserve {
                x: amplitude,
                y: amplitude,
            }
        }
        "arc" => {
            let radius = effect_param_milli(effect, "radius")
                .unwrap_or(Milli(120_000))
                .as_f32()
                .abs();
            LayoutEffectReserve {
                x: radius,
                y: radius,
            }
        }
        _ => LayoutEffectReserve::default(),
    }
}

fn effect_param_milli(effect: &RichTextEffectDescriptor, name: &str) -> Option<Milli> {
    effect_param_as_milli(effect.params.get(name)?)
}

fn effect_param_vec2(effect: &RichTextEffectDescriptor, name: &str) -> Option<[f32; 2]> {
    effect_param_as_vec2(effect.params.get(name)?)
}

fn effect_param_as_milli(param: &RichTextParam) -> Option<Milli> {
    match param {
        RichTextParam::Milli { value } => Some(*value),
        RichTextParam::Int { value } => {
            Some(Milli(i32::try_from(*value).ok()?.saturating_mul(1000)))
        }
        RichTextParam::Raw { value } | RichTextParam::Text { value } => {
            parse_raw_effect_milli(value)
        }
        _ => None,
    }
}

fn effect_param_as_vec2(param: &RichTextParam) -> Option<[f32; 2]> {
    match param {
        RichTextParam::Vec2 { value } => Some([value.x.as_f32(), value.y.as_f32()]),
        RichTextParam::Raw { value } | RichTextParam::Text { value } => {
            parse_raw_effect_vec2(value)
        }
        _ => None,
    }
}

fn parse_raw_effect_milli(value: &str) -> Option<Milli> {
    let trimmed = value.trim();
    let numeric = trimmed
        .strip_suffix("px")
        .or_else(|| trimmed.strip_suffix("deg"))
        .or_else(|| trimmed.strip_suffix("ch"))
        .unwrap_or(trimmed)
        .trim();
    parse_decimal_milli(numeric)
}

fn parse_raw_effect_vec2(value: &str) -> Option<[f32; 2]> {
    let (x, y) = value.split_once(',')?;
    Some([
        parse_raw_effect_milli(x)?.as_f32(),
        parse_raw_effect_milli(y)?.as_f32(),
    ])
}

fn effect_axis_direction(effect: &RichTextEffectDescriptor) -> Option<[f32; 2]> {
    match effect.params.get("axis")? {
        RichTextParam::Raw { value }
        | RichTextParam::Text { value }
        | RichTextParam::Selector { value } => match value.as_str() {
            "x" | ".x" => Some([1.0, 0.0]),
            "y" | ".y" => Some([0.0, 1.0]),
            _ => None,
        },
        _ => None,
    }
}
