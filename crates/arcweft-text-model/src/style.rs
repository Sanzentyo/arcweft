//! Authored rich-text styles and deterministic presentation cascade.

use crate::{
    Milli, RichTextAngle, RichTextLayoutUpdate, RichTextObjectProxy, RichTextPresentation,
    RichTextTransform,
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
        layout: RichTextLayoutUpdate,
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

fn merge_layout_presentation(out: &mut RichTextPresentation, layout: &RichTextLayoutUpdate) {
    if layout.is_empty() {
        return;
    }
    let mut merged = out.layout.clone().unwrap_or_default();
    layout.apply_to(&mut merged);
    out.layout = Some(merged);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        RichTextInlineDirection, RichTextJlreqStrictness, RichTextRubyPosition,
        RichTextVerticalLatinMode, RichTextWritingMode,
    };

    #[test]
    fn empty_layout_update_preserves_absent_or_inherited_layout() {
        let empty = RichTextStyle::Layout {
            layout: RichTextLayoutUpdate::default(),
        };
        assert!(presentation_from_styles([&empty]).layout.is_none());
        let inherited = RichTextStyle::Layout {
            layout: RichTextLayoutUpdate {
                writing_mode: Some(RichTextWritingMode::VerticalLr),
                ruby_position: Some(RichTextRubyPosition::Under),
                ..RichTextLayoutUpdate::default()
            },
        };
        assert_eq!(
            presentation_from_styles([&inherited, &empty]),
            presentation_from_styles([&inherited])
        );
        let json = serde_json::to_value(&empty).unwrap();
        assert_eq!(json["layout"], serde_json::json!({}));
        assert_eq!(
            serde_json::from_value::<RichTextStyle>(json).unwrap(),
            empty
        );
    }

    #[test]
    fn explicit_layout_defaults_override_inherited_nondefaults() {
        let inherited = RichTextStyle::Layout {
            layout: RichTextLayoutUpdate {
                writing_mode: Some(RichTextWritingMode::VerticalRl),
                direction: Some(RichTextInlineDirection::Rtl),
                vertical_latin: Some(RichTextVerticalLatinMode::Upright),
                ruby_position: Some(RichTextRubyPosition::Under),
                jlreq_strictness: Some(RichTextJlreqStrictness::Strict),
                column_gap: Some(Milli(15_000)),
                ruby_gap: Some(Milli(3_000)),
                ..RichTextLayoutUpdate::default()
            },
        };
        let reset = RichTextStyle::Layout {
            layout: RichTextLayoutUpdate {
                writing_mode: Some(RichTextWritingMode::HorizontalTb),
                direction: Some(RichTextInlineDirection::Auto),
                vertical_latin: Some(RichTextVerticalLatinMode::Mixed),
                ruby_position: Some(RichTextRubyPosition::Auto),
                jlreq_strictness: Some(RichTextJlreqStrictness::Auto),
                column_gap: Some(Milli(8_000)),
                ruby_gap: Some(Milli::ZERO),
                ..RichTextLayoutUpdate::default()
            },
        };
        let encoded = serde_json::to_vec(&reset).unwrap();
        let decoded: RichTextStyle = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, reset);
        let resolved = presentation_from_styles([&inherited, &decoded])
            .layout
            .unwrap();
        assert_eq!(resolved.writing_mode, RichTextWritingMode::HorizontalTb);
        assert_eq!(resolved.direction, RichTextInlineDirection::Auto);
        assert_eq!(resolved.vertical_latin, RichTextVerticalLatinMode::Mixed);
        assert_eq!(resolved.ruby_position, RichTextRubyPosition::Auto);
        assert_eq!(resolved.jlreq_strictness, RichTextJlreqStrictness::Auto);
        assert_eq!(resolved.column_gap, Milli(8_000));
        assert_eq!(resolved.ruby_gap, Some(Milli::ZERO));
        let reversed = presentation_from_styles([&decoded, &inherited])
            .layout
            .unwrap();
        assert_eq!(reversed.writing_mode, RichTextWritingMode::VerticalRl);
        assert_eq!(reversed.ruby_position, RichTextRubyPosition::Under);
        assert_eq!(reversed.column_gap, Milli(15_000));
    }
}
