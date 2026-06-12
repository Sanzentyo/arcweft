//! Adapter from Arcweft Sans I/O text layout geometry to glyphon `GlyphArea`.

use arcweft_text_layout::{GlyphOrientation, LaidOutGlyph, LaidOutText};
use glyphon::{
    CacheKey, Color, GlyphArea, GlyphInstance, GlyphSource, GlyphTransform, Point, Rect,
    TextBounds, TextCluster, Vector,
};
use thiserror::Error;

/// Error raised while adapting laid-out text to glyphon glyph areas.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum GlyphonAdapterError {
    /// A laid-out glyph could not be mapped to a glyphon cache key.
    #[error("missing glyphon cache key for glyph at layout index {glyph_index}")]
    MissingCacheKey {
        /// Index in `LaidOutText::glyphs`.
        glyph_index: usize,
    },
}

/// Static options used to wrap one Arcweft layout in a glyphon area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphonAreaOptions {
    /// Area left offset.
    pub left: f32,
    /// Area top offset.
    pub top: f32,
    /// Area scale.
    pub scale: f32,
    /// Clip bounds.
    pub bounds: TextBounds,
    /// Default glyph color.
    pub default_color: Color,
    /// Whether missing cache keys should skip glyphs instead of erroring.
    pub skip_missing_glyphs: bool,
}

impl Default for GlyphonAreaOptions {
    fn default() -> Self {
        Self {
            left: 0.0,
            top: 0.0,
            scale: 1.0,
            bounds: TextBounds::default(),
            default_color: Color::rgb(245, 245, 245),
            skip_missing_glyphs: false,
        }
    }
}

/// Owned glyph area data whose borrowed view can be submitted to glyphon.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnedGlyphArea {
    glyphs: Vec<GlyphInstance>,
    left: f32,
    top: f32,
    scale: f32,
    bounds: TextBounds,
    default_color: Color,
    skipped_glyphs: usize,
}

impl OwnedGlyphArea {
    /// Returns a borrowed glyphon area view.
    pub fn as_glyph_area(&self) -> GlyphArea<'_> {
        GlyphArea {
            glyphs: &self.glyphs,
            left: self.left,
            top: self.top,
            scale: self.scale,
            bounds: self.bounds,
            default_color: self.default_color,
        }
    }

    /// Adapted glyph instances.
    pub fn glyphs(&self) -> &[GlyphInstance] {
        &self.glyphs
    }

    /// Number of laid-out glyphs skipped because cache keys were unavailable.
    pub const fn skipped_glyphs(&self) -> usize {
        self.skipped_glyphs
    }

    /// Number of adapted glyph instances.
    pub fn len(&self) -> usize {
        self.glyphs.len()
    }

    /// Whether the area contains no glyph instances.
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }
}

/// Adapts Arcweft layout geometry to a glyphon `GlyphArea`.
///
/// The resolver boundary keeps font shaping and cache-key ownership in the
/// renderer adapter. This crate only maps Arcweft geometry and orientation into
/// glyphon renderer instances.
pub fn glyph_area_from_layout(
    layout: &LaidOutText,
    options: GlyphonAreaOptions,
    mut resolve_cache_key: impl FnMut(usize, &LaidOutGlyph) -> Option<CacheKey>,
) -> Result<OwnedGlyphArea, GlyphonAdapterError> {
    let mut skipped_glyphs = 0;
    let mut glyphs = Vec::with_capacity(layout.glyphs.len());
    for (glyph_index, glyph) in layout.glyphs.iter().enumerate() {
        let Some(cache_key) = resolve_cache_key(glyph_index, glyph) else {
            if options.skip_missing_glyphs {
                skipped_glyphs += 1;
                continue;
            }
            return Err(GlyphonAdapterError::MissingCacheKey { glyph_index });
        };
        glyphs.push(glyph_instance(glyph_index, glyph, cache_key));
    }
    Ok(OwnedGlyphArea {
        glyphs,
        left: options.left,
        top: options.top,
        scale: options.scale,
        bounds: options.bounds,
        default_color: options.default_color,
        skipped_glyphs,
    })
}

fn glyph_instance(glyph_index: usize, glyph: &LaidOutGlyph, cache_key: CacheKey) -> GlyphInstance {
    GlyphInstance {
        source: GlyphSource::Text { cache_key },
        origin: Point::new(glyph.origin.x, glyph.origin.y),
        advance: Vector::new(glyph.advance.width, glyph.advance.height),
        ink_bounds: Rect::new(0.0, 0.0, glyph.bounds.width, glyph.bounds.height),
        transform: glyph_transform(glyph.orientation),
        color: None,
        metadata: glyph_index,
        cluster: Some(TextCluster {
            start: glyph.range.start,
            end: glyph.range.end,
            index: u32::try_from(glyph_index).unwrap_or(u32::MAX),
        }),
    }
}

const fn glyph_transform(orientation: GlyphOrientation) -> GlyphTransform {
    match orientation {
        GlyphOrientation::Upright | GlyphOrientation::TextCombineUpright => {
            GlyphTransform::Identity
        }
        GlyphOrientation::SidewaysCw => GlyphTransform::Rotate90Cw,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_text_layout::{
        LaidOutGlyph, LayoutPoint, LayoutRect, LayoutSize, TextLayoutConfig, layout_frame,
    };
    use glyphon::{Weight, cosmic_text::CacheKeyFlags, fontdb};

    fn fake_cache_key(glyph_id: u16) -> CacheKey {
        let (key, _, _) = CacheKey::new(
            fontdb::ID::dummy(),
            glyph_id,
            30.0,
            (0.0, 0.0),
            Weight::NORMAL,
            CacheKeyFlags::empty(),
        );
        key
    }

    #[test]
    fn maps_layout_glyphs_to_glyphon_instances() {
        let layout = LaidOutText {
            glyphs: vec![
                LaidOutGlyph {
                    run_index: 0,
                    range: arcweft_render_text::RichTextRange::new(0, 3),
                    text: "夢".to_owned(),
                    origin: LayoutPoint::new(10.0, 20.0),
                    advance: LayoutSize::new(0.0, 42.0),
                    bounds: LayoutRect::new(10.0, 20.0, 42.0, 42.0),
                    writing_mode: arcweft_render_text::RichTextWritingMode::VerticalRl,
                    orientation: GlyphOrientation::Upright,
                    presentation: arcweft_render_text::RichTextPresentation::default(),
                },
                LaidOutGlyph {
                    run_index: 0,
                    range: arcweft_render_text::RichTextRange::new(3, 4),
                    text: "A".to_owned(),
                    origin: LayoutPoint::new(10.0, 62.0),
                    advance: LayoutSize::new(0.0, 42.0),
                    bounds: LayoutRect::new(10.0, 62.0, 42.0, 42.0),
                    writing_mode: arcweft_render_text::RichTextWritingMode::VerticalRl,
                    orientation: GlyphOrientation::SidewaysCw,
                    presentation: arcweft_render_text::RichTextPresentation::default(),
                },
            ],
            runs: Vec::new(),
            ruby: Vec::new(),
            bounds: None,
        };

        let area =
            glyph_area_from_layout(&layout, GlyphonAreaOptions::default(), |index, _glyph| {
                Some(fake_cache_key(u16::try_from(index).unwrap_or(u16::MAX)))
            })
            .expect("area adapts");

        assert_eq!(area.len(), 2);
        assert_eq!(area.glyphs()[0].transform, GlyphTransform::Identity);
        assert_eq!(area.glyphs()[1].transform, GlyphTransform::Rotate90Cw);
        assert_eq!(
            area.glyphs()[1].cluster,
            Some(TextCluster {
                start: 3,
                end: 4,
                index: 1
            })
        );
    }

    #[test]
    fn missing_cache_key_can_skip_or_error() {
        let frame = arcweft_render_text::LineDisplayFrame {
            line: arcweft_core::plan::RuntimeLineId("say.test.001".to_owned()),
            callee: "alice.say".to_owned(),
            text: "A".to_owned(),
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            nodes: Vec::new(),
            display_map: arcweft_render_text::RichTextDisplayMap {
                text_runs: vec![arcweft_render_text::RichTextTextRun {
                    range: arcweft_render_text::RichTextRange::new(0, 1),
                    source: arcweft_render_text::RichTextTextSource::Text,
                    node_index: 0,
                    styles: Vec::new(),
                    presentation: arcweft_render_text::RichTextPresentation::default(),
                }],
                ruby_annotations: Vec::new(),
                controls: Vec::new(),
                host_events: Vec::new(),
            },
            host_events: Vec::new(),
            inline_failures: Vec::new(),
            unresolved: Vec::new(),
        };
        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        assert_eq!(
            glyph_area_from_layout(&layout, GlyphonAreaOptions::default(), |_index, _glyph| {
                None
            })
            .expect_err("missing key errors"),
            GlyphonAdapterError::MissingCacheKey { glyph_index: 0 }
        );

        let area = glyph_area_from_layout(
            &layout,
            GlyphonAreaOptions {
                skip_missing_glyphs: true,
                ..GlyphonAreaOptions::default()
            },
            |_index, _glyph| None,
        )
        .expect("missing key skips");
        assert_eq!(area.skipped_glyphs(), 1);
        assert!(area.is_empty());
    }
}
