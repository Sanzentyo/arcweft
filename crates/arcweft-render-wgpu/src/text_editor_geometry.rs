//! Renderer-owned conversion from Arcweft text layout to text-input geometry.
//!
//! This module is intentionally in `arcweft-render-wgpu`: it may depend on the
//! renderer-independent text layout crate and on presentation geometry, while
//! `arcweft-presentation` stays Sans I/O and renderer-agnostic.

use arcweft_presentation::hit::HitRect;
use arcweft_presentation::text_editor::{
    TextEditorGlyphGeometry, TextEditorLayout, TextEditorLayoutError, TextEditorLayoutParts,
    TextEditorLayoutSource,
};
use arcweft_presentation::text_input::{
    TextByteOffset, TextGeometryTransform, TextRange, TextWritingMode,
};
use arcweft_text_layout::{LaidOutText, LayoutRect};
use core::cmp::Ordering;

/// Renderer/player transform context for one focused text-control geometry pump.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextEditorGeometryContext {
    pub text_local_control_rect: HitRect,
    pub caret_width: f32,
    pub writing_mode: TextWritingMode,
    pub text_local_to_viewport: TextGeometryTransform,
    pub viewport_to_screen: TextGeometryTransform,
}

/// Stateless conversion entry point used after renderer text layout finishes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextEditorGeometryPump;

impl Default for TextEditorGeometryContext {
    fn default() -> Self {
        Self {
            text_local_control_rect: HitRect::new(0.0, 0.0, 320.0, 24.0),
            caret_width: 1.0,
            writing_mode: TextWritingMode::HorizontalTb,
            text_local_to_viewport: TextGeometryTransform::identity(),
            viewport_to_screen: TextGeometryTransform::identity(),
        }
    }
}

impl TextEditorGeometryContext {
    #[must_use]
    pub const fn with_text_local_control_rect(mut self, rect: HitRect) -> Self {
        self.text_local_control_rect = rect;
        self
    }

    #[must_use]
    pub const fn with_caret_width(mut self, width: f32) -> Self {
        self.caret_width = width;
        self
    }

    #[must_use]
    pub const fn with_writing_mode(mut self, writing_mode: TextWritingMode) -> Self {
        self.writing_mode = writing_mode;
        self
    }

    #[must_use]
    pub const fn with_text_local_to_viewport(mut self, transform: TextGeometryTransform) -> Self {
        self.text_local_to_viewport = transform;
        self
    }

    #[must_use]
    pub const fn with_viewport_to_screen(mut self, transform: TextGeometryTransform) -> Self {
        self.viewport_to_screen = transform;
        self
    }
}

impl TextEditorGeometryPump {
    /// Builds a renderer-backed editor layout from the latest text-layout output.
    pub fn layout_from_laid_out_text(
        text: &str,
        laid_out: &LaidOutText,
        context: TextEditorGeometryContext,
    ) -> Result<TextEditorLayout, TextEditorLayoutError> {
        let glyphs = laid_out
            .glyphs
            .iter()
            .map(|glyph| {
                TextEditorGlyphGeometry::new(
                    TextRange::new(
                        TextByteOffset(u32::try_from(glyph.range.start).unwrap_or(u32::MAX)),
                        TextByteOffset(u32::try_from(glyph.range.end).unwrap_or(u32::MAX)),
                    ),
                    layout_rect_to_hit_rect(glyph.bounds),
                )
            })
            .collect::<Vec<_>>();
        let glyphs = normalize_renderer_glyphs_for_editor(glyphs, context.writing_mode);

        TextEditorLayout::from_renderer_parts_for_text(
            text,
            TextEditorLayoutParts {
                source: TextEditorLayoutSource::Renderer,
                text_local_control_rect: context.text_local_control_rect,
                glyphs,
                caret_width: context.caret_width,
                writing_mode: context.writing_mode,
                text_local_to_viewport: context.text_local_to_viewport,
                viewport_to_screen: context.viewport_to_screen,
            },
        )
    }
}

fn normalize_renderer_glyphs_for_editor(
    mut glyphs: Vec<TextEditorGlyphGeometry>,
    writing_mode: TextWritingMode,
) -> Vec<TextEditorGlyphGeometry> {
    glyphs.sort_by(|left, right| compare_renderer_glyph_geometry(*left, *right, writing_mode));

    let non_empty_ranges = glyphs
        .iter()
        .map(|glyph| glyph.range())
        .filter(|range| !range_is_collapsed(*range))
        .collect::<Vec<_>>();
    let mut normalized = Vec::<TextEditorGlyphGeometry>::with_capacity(glyphs.len());

    for glyph in glyphs {
        let range = glyph.range();
        if range_is_collapsed(range)
            && collapsed_range_is_inside_non_empty_range(range, &non_empty_ranges)
        {
            continue;
        }
        match normalized.last_mut() {
            Some(previous) if previous.range() == range => {
                let bounds = union_hit_rect(previous.bounds(), glyph.bounds());
                *previous = TextEditorGlyphGeometry::new(range, bounds);
            }
            _ => normalized.push(glyph),
        }
    }

    normalized
}

fn compare_renderer_glyph_geometry(
    left: TextEditorGlyphGeometry,
    right: TextEditorGlyphGeometry,
    writing_mode: TextWritingMode,
) -> Ordering {
    left.range()
        .start()
        .0
        .cmp(&right.range().start().0)
        .then_with(|| left.range().end().0.cmp(&right.range().end().0))
        .then_with(|| compare_visual_position(left.bounds(), right.bounds(), writing_mode))
        .then_with(|| left.bounds().width.total_cmp(&right.bounds().width))
        .then_with(|| left.bounds().height.total_cmp(&right.bounds().height))
}

fn compare_visual_position(
    left: HitRect,
    right: HitRect,
    writing_mode: TextWritingMode,
) -> Ordering {
    match writing_mode {
        TextWritingMode::HorizontalTb => left
            .y
            .total_cmp(&right.y)
            .then_with(|| left.x.total_cmp(&right.x)),
        TextWritingMode::VerticalRl => right
            .x
            .total_cmp(&left.x)
            .then_with(|| left.y.total_cmp(&right.y)),
        TextWritingMode::VerticalLr => left
            .x
            .total_cmp(&right.x)
            .then_with(|| left.y.total_cmp(&right.y)),
    }
}

fn range_is_collapsed(range: TextRange<TextByteOffset>) -> bool {
    range.start() == range.end()
}

fn collapsed_range_is_inside_non_empty_range(
    range: TextRange<TextByteOffset>,
    non_empty_ranges: &[TextRange<TextByteOffset>],
) -> bool {
    let offset = range.start().0;
    non_empty_ranges
        .iter()
        .any(|non_empty| non_empty.start().0 < offset && offset < non_empty.end().0)
}

fn union_hit_rect(left: HitRect, right: HitRect) -> HitRect {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    let right_edge = (left.x + left.width).max(right.x + right.width);
    let bottom_edge = (left.y + left.height).max(right.y + right.height);
    HitRect::new(
        x,
        y,
        (right_edge - x).max(0.0),
        (bottom_edge - y).max(0.0),
    )
}

fn layout_rect_to_hit_rect(rect: LayoutRect) -> HitRect {
    HitRect::new(rect.x, rect.y, rect.width, rect.height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_render_text::{RichTextPresentation, RichTextRange, RichTextWritingMode};
    use arcweft_text_layout::{
        GlyphOrientation, GlyphVerticalForm, LaidOutGlyph, LayoutPoint, LayoutSize,
    };

    fn glyph(start: usize, end: usize, x: f32, width: f32) -> LaidOutGlyph {
        glyph_at(start, end, x, 4.0, width)
    }

    fn glyph_at(start: usize, end: usize, x: f32, y: f32, width: f32) -> LaidOutGlyph {
        LaidOutGlyph {
            run_index: 0,
            range: RichTextRange::new(start, end),
            text: String::new(),
            origin: LayoutPoint::new(x, y),
            advance: LayoutSize::new(width, 0.0),
            bounds: LayoutRect::new(x, y, width, 18.0),
            writing_mode: RichTextWritingMode::HorizontalTb,
            orientation: GlyphOrientation::Upright,
            vertical_form: GlyphVerticalForm::None,
            presentation: RichTextPresentation::default(),
        }
    }

    #[test]
    fn converts_real_layout_glyphs_to_renderer_backed_editor_layout() {
        let text = "A日本🦀";
        let laid_out = LaidOutText {
            glyphs: vec![
                glyph(0, 1, 8.0, 9.0),
                glyph(1, 4, 17.0, 18.0),
                glyph(4, 7, 35.0, 18.0),
                glyph(7, 11, 53.0, 22.0),
            ],
            ..LaidOutText::default()
        };

        let layout = TextEditorGeometryPump::layout_from_laid_out_text(
            text,
            &laid_out,
            TextEditorGeometryContext::default()
                .with_text_local_control_rect(HitRect::new(0.0, 0.0, 120.0, 28.0))
                .with_viewport_to_screen(TextGeometryTransform::translation(200.0, 100.0)),
        )
        .unwrap();

        assert!(layout.is_renderer_backed());
        assert_eq!(layout.glyphs().len(), 4);
        assert!(layout.glyphs()[1].bounds().width > layout.glyphs()[0].bounds().width);
    }

    #[test]
    fn merges_identical_non_empty_renderer_ranges_into_one_editor_cluster() {
        let text = "abcde";
        let laid_out = LaidOutText {
            glyphs: vec![
                glyph(0, 1, 0.0, 8.0),
                glyph(1, 4, 8.0, 5.0),
                glyph(1, 4, 13.0, 7.0),
                glyph(4, 5, 20.0, 8.0),
            ],
            ..LaidOutText::default()
        };

        let layout = TextEditorGeometryPump::layout_from_laid_out_text(
            text,
            &laid_out,
            TextEditorGeometryContext::default(),
        )
        .unwrap();

        assert_eq!(layout.glyphs().len(), 3);
        assert_eq!(layout.glyphs()[1].range().start().0, 1);
        assert_eq!(layout.glyphs()[1].range().end().0, 4);
        assert_eq!(
            layout.glyphs()[1].bounds(),
            HitRect::new(8.0, 4.0, 12.0, 18.0)
        );
    }

    #[test]
    fn mixed_script_fallback_ranges_do_not_reach_editor_layout_as_duplicates() {
        let text = "( ﾟДﾟ)";
        let laid_out = LaidOutText {
            glyphs: vec![
                glyph(0, 1, 0.0, 8.0),
                glyph(1, 2, 8.0, 5.0),
                glyph(2, 5, 13.0, 4.0),
                glyph(2, 5, 17.0, 6.0),
                glyph(5, 7, 23.0, 11.0),
                glyph(7, 10, 34.0, 4.0),
                glyph(7, 10, 38.0, 6.0),
                glyph(10, 11, 44.0, 8.0),
            ],
            ..LaidOutText::default()
        };

        let layout = TextEditorGeometryPump::layout_from_laid_out_text(
            text,
            &laid_out,
            TextEditorGeometryContext::default(),
        )
        .unwrap();

        assert_eq!(layout.glyphs().len(), 6);
        assert_eq!(layout.glyphs()[2].range().start().0, 2);
        assert_eq!(layout.glyphs()[2].range().end().0, 5);
        assert_eq!(layout.glyphs()[4].range().start().0, 7);
        assert_eq!(layout.glyphs()[4].range().end().0, 10);
    }

    #[test]
    fn collapsed_range_inside_non_empty_cluster_is_dropped_before_validation() {
        let text = "abcd";
        let laid_out = LaidOutText {
            glyphs: vec![
                glyph(0, 3, 0.0, 30.0),
                glyph(1, 1, 10.0, 0.0),
                glyph(3, 4, 30.0, 10.0),
            ],
            ..LaidOutText::default()
        };

        let layout = TextEditorGeometryPump::layout_from_laid_out_text(
            text,
            &laid_out,
            TextEditorGeometryContext::default(),
        )
        .unwrap();

        assert_eq!(layout.glyphs().len(), 2);
        assert!(layout
            .glyphs()
            .iter()
            .all(|glyph| glyph.range().start().0 != 1 || glyph.range().end().0 != 1));
    }

    #[test]
    fn collapsed_range_at_distinct_caret_stop_is_preserved() {
        let text = "abcd";
        let laid_out = LaidOutText {
            glyphs: vec![
                glyph(0, 1, 0.0, 10.0),
                glyph(1, 2, 10.0, 10.0),
                glyph_at(2, 2, 0.0, 26.0, 0.0),
                glyph_at(2, 3, 0.0, 26.0, 10.0),
                glyph_at(3, 4, 10.0, 26.0, 10.0),
            ],
            ..LaidOutText::default()
        };

        let layout = TextEditorGeometryPump::layout_from_laid_out_text(
            text,
            &laid_out,
            TextEditorGeometryContext::default(),
        )
        .unwrap();

        assert!(layout
            .glyphs()
            .iter()
            .any(|glyph| glyph.range().start().0 == 2 && glyph.range().end().0 == 2));
    }

    #[test]
    fn partial_renderer_range_overlap_remains_a_layout_error() {
        let text = "abcdef";
        let laid_out = LaidOutText {
            glyphs: vec![glyph(0, 4, 0.0, 30.0), glyph(2, 5, 30.0, 30.0)],
            ..LaidOutText::default()
        };

        let error = TextEditorGeometryPump::layout_from_laid_out_text(
            text,
            &laid_out,
            TextEditorGeometryContext::default(),
        )
        .unwrap_err();

        assert_eq!(
            error,
            TextEditorLayoutError::OverlappingGlyphRange {
                previous: TextRange::new(TextByteOffset(0), TextByteOffset(4)),
                next: TextRange::new(TextByteOffset(2), TextByteOffset(5)),
            }
        );
    }
}
