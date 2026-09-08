//! Authored rich-text styles and deterministic presentation cascade.

use crate::{
    Milli, RichTextAngle, RichTextInlineDirection, RichTextJlreqStrictness, RichTextLayout,
    RichTextObjectProxy, RichTextPresentation, RichTextRubyPosition, RichTextTransform,
    RichTextVerticalLatinMode, RichTextWritingMode,
};
use arcweft_presentation::fx::FxApplication;
use serde::{Deserialize, Serialize};

/// One typed lexical modifier carried by a structural rich-text scope.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextStyle {
    Em,
    Strong,
    Italic,
    Oblique {
        angle: RichTextAngle,
    },
    Color {
        value: RichTextColor,
    },
    Font {
        family: RichTextFontFamily,
    },
    Size {
        milli_points: Milli,
    },
    Speed {
        milli_cps: Milli,
    },
    Layout {
        layout: RichTextLayout,
    },
    Transform {
        transform: RichTextTransform,
    },
    Object {
        proxy: RichTextObjectProxy,
    },
    Presentation {
        presentation: RichTextPresentationStyle,
    },
    /// Typed reusable Fx graph application retained until shared evaluation.
    Fx {
        application: FxApplication,
    },
}

/// Scalar presentation metadata applied to rich-text objects.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextPresentationStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<Milli>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z_index: Option<i16>,
}

/// Inline text color.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextColor {
    Rgba8 { value: [u8; 4] },
    Resource { id: String },
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

/// Aggregates presentation metadata from active rich-text styles.
#[must_use]
pub fn presentation_from_styles<'a>(
    styles: impl IntoIterator<Item = &'a RichTextStyle>,
) -> RichTextPresentation {
    styles
        .into_iter()
        .fold(RichTextPresentation::default(), |mut out, style| {
            match style {
                RichTextStyle::Em | RichTextStyle::Italic => out.italic = true,
                RichTextStyle::Oblique { angle } => out.oblique = Some(*angle),
                RichTextStyle::Layout { layout } => merge_layout_presentation(&mut out, layout),
                RichTextStyle::Transform { transform } => {
                    out.transform = Some(transform.clone());
                }
                RichTextStyle::Object { proxy } => out.object_proxies.push(proxy.clone()),
                RichTextStyle::Fx { application } => out.fx.push(application.clone()),
                RichTextStyle::Presentation { presentation } => {
                    if let Some(opacity) = presentation.opacity {
                        out.opacity = Some(opacity);
                    }
                    if let Some(layer) = &presentation.layer {
                        out.layer = Some(layer.clone());
                    }
                    if let Some(z_index) = presentation.z_index {
                        out.z_index = z_index;
                    }
                }
                RichTextStyle::Strong
                | RichTextStyle::Color { .. }
                | RichTextStyle::Font { .. }
                | RichTextStyle::Size { .. }
                | RichTextStyle::Speed { .. } => {}
            }
            out
        })
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
