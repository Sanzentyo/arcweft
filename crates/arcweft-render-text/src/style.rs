//! Authored rich-text styles and deterministic presentation cascade.

use crate::{
    Milli, RichTextAngle, RichTextEffectDescriptor, RichTextInlineDirection,
    RichTextJlreqStrictness, RichTextLayout, RichTextObjectProxy, RichTextParam,
    RichTextPresentation, RichTextRubyPosition, RichTextShaderRef, RichTextTransform,
    RichTextVerticalLatinMode, RichTextWritingMode, parse_decimal_milli, parse_milli_token,
    parse_z_index_token,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Inline style span applied until the matching `StyleEnd`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextStyle {
    Em {
        attrs: String,
    },
    Strong {
        attrs: String,
    },
    Italic {
        attrs: String,
    },
    Oblique {
        angle: RichTextAngle,
        raw: String,
    },
    Color {
        value: RichTextColor,
    },
    Font {
        family: RichTextFontFamily,
    },
    Size {
        points: Option<u16>,
        raw: String,
    },
    Speed {
        value: String,
    },
    Layout {
        layout: RichTextLayout,
    },
    Transform {
        transform: RichTextTransform,
    },
    Effect {
        effect: RichTextEffectDescriptor,
    },
    Shader {
        shader: RichTextShaderRef,
    },
    Object {
        proxy: RichTextObjectProxy,
    },
    Presentation {
        presentation: RichTextPresentationStyle,
    },
    Unknown {
        name: String,
        attrs: String,
    },
}

/// Scalar presentation metadata applied to rich-text objects.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextPresentationStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<Milli>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, RichTextParam>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z_index: Option<i16>,
}

/// Inline text color.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextColor {
    Rgb { red: u8, green: u8, blue: u8 },
    Named { name: String },
}

/// Font family requested by authored rich text.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextFontFamily {
    Serif,
    SansSerif,
    Monospace,
    Cursive,
    Fantasy,
    Named { name: String },
}

impl RichTextStyle {
    /// Creates a typed style from an authored tag name and raw attribute text.
    #[must_use]
    pub fn from_tag(name: &str, attrs: &str) -> Self {
        let attrs = attrs.trim();
        match name {
            "em" => Self::Em {
                attrs: attrs.to_owned(),
            },
            "strong" => Self::Strong {
                attrs: attrs.to_owned(),
            },
            "i" | "italic" => Self::Italic {
                attrs: attrs.to_owned(),
            },
            "oblique" | "slant" => Self::Oblique {
                angle: RichTextAngle {
                    degrees: parse_milli_token(attrs),
                },
                raw: attrs.to_owned(),
            },
            "color" => Self::Color {
                value: RichTextColor::from_attrs(attrs),
            },
            "font" => Self::Font {
                family: RichTextFontFamily::from_attrs(attrs),
            },
            "size" => {
                let value = scalar_tag_value(attrs);
                Self::Size {
                    points: value
                        .split_whitespace()
                        .next()
                        .and_then(|value| value.parse::<u16>().ok()),
                    raw: value.to_owned(),
                }
            }
            "speed" => Self::Speed {
                value: attrs.to_owned(),
            },
            "opacity" | "alpha" => Self::Presentation {
                presentation: RichTextPresentationStyle {
                    opacity: Some(parse_milli_token(attrs)),
                    layer: None,
                    params: BTreeMap::new(),
                    z_index: None,
                },
            },
            "layer" | "object_layer" => Self::Presentation {
                presentation: RichTextPresentationStyle {
                    opacity: None,
                    layer: (!attrs.trim().is_empty()).then(|| attrs.trim().to_owned()),
                    params: BTreeMap::new(),
                    z_index: None,
                },
            },
            "meta" | "metadata" | "data" => Self::Presentation {
                presentation: RichTextPresentationStyle {
                    opacity: None,
                    layer: None,
                    params: parse_presentation_params(attrs),
                    z_index: None,
                },
            },
            "z" | "z_index" => Self::Presentation {
                presentation: RichTextPresentationStyle {
                    opacity: None,
                    layer: None,
                    params: BTreeMap::new(),
                    z_index: parse_z_index_token(attrs),
                },
            },
            name => Self::Unknown {
                name: name.to_owned(),
                attrs: attrs.to_owned(),
            },
        }
    }

    /// Authored tag name used to match the corresponding end tag.
    #[must_use]
    pub fn tag_name(&self) -> &str {
        match self {
            Self::Em { .. } => "em",
            Self::Strong { .. } => "strong",
            Self::Italic { .. } | Self::Oblique { .. } | Self::Presentation { .. } => "style",
            Self::Color { .. } => "color",
            Self::Font { .. } => "font",
            Self::Size { .. } => "size",
            Self::Speed { .. } => "speed",
            Self::Layout { .. } => "layout",
            Self::Transform { .. } => "transform",
            Self::Object { .. } => "object",
            Self::Effect { .. } | Self::Shader { .. } => "effect",
            Self::Unknown { name, .. } => name,
        }
    }
}

impl RichTextColor {
    /// Parses a direct scalar or canonical `value=...` attribute into a color.
    #[must_use]
    pub fn from_attrs(attrs: &str) -> Self {
        let value = scalar_tag_value(attrs);
        parse_hex_color(value).unwrap_or_else(|| Self::Named {
            name: value.to_owned(),
        })
    }
}

impl RichTextFontFamily {
    /// Parses a direct scalar or canonical `value=...` attribute into a font.
    #[must_use]
    pub fn from_attrs(attrs: &str) -> Self {
        let value = scalar_tag_value(attrs);
        match value.to_ascii_lowercase().as_str() {
            "" | "sans" | "sans-serif" | "sans_serif" | "ui-sans" => Self::SansSerif,
            "serif" | "ui-serif" => Self::Serif,
            "mono" | "monospace" | "ui-monospace" => Self::Monospace,
            "cursive" => Self::Cursive,
            "fantasy" => Self::Fantasy,
            _ => Self::Named {
                name: value.to_owned(),
            },
        }
    }
}

/// Aggregates presentation metadata from active rich-text styles.
#[must_use]
pub fn presentation_from_styles<'a>(
    styles: impl IntoIterator<Item = &'a RichTextStyle>,
) -> RichTextPresentation {
    styles
        .into_iter()
        .fold(RichTextPresentation::default(), |mut out, style| {
            match style {
                RichTextStyle::Em { .. } | RichTextStyle::Italic { .. } => out.italic = true,
                RichTextStyle::Oblique { angle, .. } => out.oblique = Some(*angle),
                RichTextStyle::Layout { layout } => merge_layout_presentation(&mut out, layout),
                RichTextStyle::Transform { transform } => {
                    out.transform = Some(transform.clone());
                }
                RichTextStyle::Effect { effect } => out.effects.push(effect.clone()),
                RichTextStyle::Shader { shader } => out.shaders.push(shader.clone()),
                RichTextStyle::Object { proxy } => out.object_proxies.push(proxy.clone()),
                RichTextStyle::Presentation { presentation } => {
                    if let Some(opacity) = presentation.opacity {
                        out.opacity = Some(opacity);
                    }
                    if let Some(layer) = &presentation.layer {
                        out.layer = Some(layer.clone());
                    }
                    out.params.extend(presentation.params.clone());
                    if let Some(z_index) = presentation.z_index {
                        out.z_index = z_index;
                    }
                }
                RichTextStyle::Strong { .. }
                | RichTextStyle::Color { .. }
                | RichTextStyle::Font { .. }
                | RichTextStyle::Size { .. }
                | RichTextStyle::Speed { .. }
                | RichTextStyle::Unknown { .. } => {}
            }
            out
        })
}

fn parse_hex_color(value: &str) -> Option<RichTextColor> {
    let hex = value.trim().strip_prefix('#')?;
    let bytes = hex.as_bytes();
    if bytes.len() != 6 {
        return None;
    }
    let channel = |high: u8, low: u8| {
        let high = u8::try_from(char::from(high).to_digit(16)?).ok()?;
        let low = u8::try_from(char::from(low).to_digit(16)?).ok()?;
        Some((high << 4) | low)
    };
    let red = channel(bytes[0], bytes[1])?;
    let green = channel(bytes[2], bytes[3])?;
    let blue = channel(bytes[4], bytes[5])?;
    Some(RichTextColor::Rgb { red, green, blue })
}

fn trim_quoted(value: &str) -> &str {
    let trimmed = value.trim();
    trimmed
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            trimmed
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(trimmed)
}

fn scalar_tag_value(attrs: &str) -> &str {
    let attrs = attrs.trim();
    let value = attrs.strip_prefix("value=").map_or(attrs, str::trim);
    trim_quoted(value)
}

fn parse_presentation_params(attrs: &str) -> BTreeMap<String, RichTextParam> {
    attrs
        .split_whitespace()
        .filter_map(|item| {
            let (key, value) = item.split_once('=')?;
            Some((key.to_owned(), param_from_style_value(value)))
        })
        .collect()
}

fn param_from_style_value(value: &str) -> RichTextParam {
    let value = value.trim().trim_matches('"');
    if value == "true" {
        return RichTextParam::Bool { value: true };
    }
    if value == "false" {
        return RichTextParam::Bool { value: false };
    }
    if value.starts_with('.') {
        return RichTextParam::Selector {
            value: value.to_owned(),
        };
    }
    if let Ok(value) = value.parse::<i64>() {
        return RichTextParam::Int { value };
    }
    if let Some(value) = parse_style_param_milli(value) {
        return RichTextParam::Milli { value };
    }
    RichTextParam::Raw {
        value: value.to_owned(),
    }
}

fn parse_style_param_milli(value: &str) -> Option<Milli> {
    let trimmed = value.trim();
    let numeric = trimmed
        .strip_suffix("px")
        .or_else(|| trimmed.strip_suffix("deg"))
        .or_else(|| trimmed.strip_suffix("ch"))
        .unwrap_or(trimmed)
        .trim();
    parse_decimal_milli(numeric)
}

fn merge_layout_presentation(out: &mut RichTextPresentation, layout: &RichTextLayout) {
    let mut merged = out.layout.clone().unwrap_or_default();
    if !matches!(layout.writing_mode, RichTextWritingMode::HorizontalTb)
        || out.layout.is_none()
        || matches!(layout.ruby_position, RichTextRubyPosition::Auto)
    {
        merged.writing_mode = layout.writing_mode;
    }
    if !matches!(layout.direction, RichTextInlineDirection::Auto) {
        merged.direction = layout.direction;
    }
    if !matches!(layout.vertical_latin, RichTextVerticalLatinMode::Mixed) {
        merged.vertical_latin = layout.vertical_latin;
    }
    if !matches!(layout.ruby_position, RichTextRubyPosition::Auto) {
        merged.ruby_position = layout.ruby_position;
    }
    if !matches!(layout.jlreq_strictness, RichTextJlreqStrictness::Auto) {
        merged.jlreq_strictness = layout.jlreq_strictness;
    }
    if layout.column_gap != RichTextLayout::default().column_gap {
        merged.column_gap = layout.column_gap;
    }
    if layout.ruby_font_size.is_some() {
        merged.ruby_font_size = layout.ruby_font_size;
    }
    if layout.ruby_gap.is_some() {
        merged.ruby_gap = layout.ruby_gap;
    }
    if layout.ruby_overhang.is_some() {
        merged.ruby_overhang = layout.ruby_overhang;
    }
    if layout.ruby_collision_gap.is_some() {
        merged.ruby_collision_gap = layout.ruby_collision_gap;
    }
    out.layout = Some(merged);
}
