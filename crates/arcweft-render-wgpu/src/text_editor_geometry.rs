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
use arcweft_presentation::text_input::{TextByteOffset, TextGeometryTransform, TextWritingMode};
use arcweft_text_layout::{LaidOutText, LayoutRect};

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
                    arcweft_presentation::text_input::TextRange::new(
                        TextByteOffset(u32::try_from(glyph.range.start).unwrap_or(u32::MAX)),
                        TextByteOffset(u32::try_from(glyph.range.end).unwrap_or(u32::MAX)),
                    ),
                    layout_rect_to_hit_rect(glyph.bounds),
                )
            })
            .collect::<Vec<_>>();

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
        LaidOutGlyph {
            run_index: 0,
            range: RichTextRange::new(start, end),
            text: String::new(),
            origin: LayoutPoint::new(x, 4.0),
            advance: LayoutSize::new(width, 0.0),
            bounds: LayoutRect::new(x, 4.0, width, 18.0),
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
}
