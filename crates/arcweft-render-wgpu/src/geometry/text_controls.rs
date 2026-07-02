use super::{
    FramePlanError, PaintRect, Palette, PreparedTextInputTarget, RenderFontFamily, RenderTextBlock,
    RenderTextSlant, RenderTextWeight, RenderViewport,
};
use crate::text_editor_geometry::{TextEditorGeometryContext, TextEditorGeometryPump};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::layer::LayerId;
use arcweft_presentation::semantic::{SemanticNode, SemanticRole, SemanticTree};
use arcweft_presentation::text_editor::{TextEditorError, TextEditorState};
use arcweft_presentation::text_input::{
    TextByteOffset, TextGeometryTransform, TextInputOptions, TextInputSecurityPolicy,
    TextInputSessionId, TextRange, TextWritingMode,
};
use arcweft_render_text::{RichTextPresentation, RichTextRange, RichTextWritingMode};
use arcweft_text_layout::{
    GlyphOrientation, GlyphVerticalForm, LaidOutGlyph, LaidOutText, LayoutPoint, LayoutRect,
    LayoutSize,
};
use num_traits::ToPrimitive;

const TEXT_INSET_X: f32 = 8.0;
const TEXT_INSET_Y: f32 = 4.0;
const CARET_WIDTH: f32 = 2.0;

/// Real text-control input lowered from runtime/product UI state.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderTextInputControl {
    pub target: InteractionTarget,
    pub session: TextInputSessionId,
    pub value: String,
    pub selection: TextRange<TextByteOffset>,
    pub options: TextInputOptions,
    pub role: SemanticRole,
    pub bounds: HitRect,
    pub label: Option<String>,
}

impl RenderTextInputControl {
    pub fn new(
        target: InteractionTarget,
        session: TextInputSessionId,
        value: impl Into<String>,
        selection: TextRange<TextByteOffset>,
        options: TextInputOptions,
        role: SemanticRole,
        bounds: HitRect,
    ) -> Self {
        Self {
            target,
            session,
            value: value.into(),
            selection,
            options,
            role,
            bounds,
            label: None,
        }
    }

    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }

    #[must_use]
    pub const fn with_selection(mut self, selection: TextRange<TextByteOffset>) -> Self {
        self.selection = selection;
        self
    }

    #[must_use]
    pub fn with_options(mut self, options: TextInputOptions) -> Self {
        self.options = options;
        self
    }

    pub fn resolved_options(&self) -> Result<TextInputOptions, FramePlanError> {
        self.role
            .text_input_options(self.options.clone())
            .ok_or(FramePlanError::InvalidTextInputRole { role: self.role })
    }
}

pub(super) fn build_text_inputs(
    scene: &super::RenderScene,
    layer: &LayerId,
    semantics: &mut SemanticTree,
    rectangles: &mut Vec<PaintRect>,
    text: &mut Vec<RenderTextBlock>,
    palette: &Palette,
) -> Result<Option<PreparedTextInputTarget>, FramePlanError> {
    let mut focused = None;
    for control in &scene.text_inputs {
        let options = control.resolved_options()?;
        let is_focused = scene.interaction.focused.as_ref() == Some(&control.target);
        let layout = laid_out_text_for_control(control);
        rectangles.push(PaintRect {
            bounds: control.bounds,
            rgba: if is_focused {
                palette.choice_active
            } else {
                palette.choice_idle
            },
        });
        if is_focused {
            super::push_focus_ring(rectangles, control.bounds, palette.focus_ring);
            push_renderer_text_input_selection(rectangles, control, &layout, palette);
            push_renderer_text_input_caret(rectangles, control, &layout, palette);
        }

        let display_value = if options.is_secure() {
            mask_secure_text(&control.value)
        } else {
            control.value.clone()
        };
        text.push(RenderTextBlock {
            text: display_value,
            bounds: text_inner_bounds(control),
            font_size: text_control_font_size(control),
            line_height: text_control_line_height(control),
            font_family: RenderFontFamily::SansSerif,
            weight: RenderTextWeight::Regular,
            slant: RenderTextSlant::Upright,
            rgba: palette.choice_text,
        });

        let mut node = SemanticNode::new(
            layer.clone(),
            control.target.clone(),
            control.role,
            control.bounds,
        );
        if let Some(label) = &control.label {
            node = node.with_label(label.clone());
        }
        semantics.push(node);

        if is_focused {
            focused = Some(prepare_text_input_target(
                scene.viewport,
                control,
                &options,
                &layout,
            )?);
        }
    }
    Ok(focused)
}

fn push_renderer_text_input_selection(
    rectangles: &mut Vec<PaintRect>,
    control: &RenderTextInputControl,
    layout: &LaidOutText,
    palette: &Palette,
) {
    let start = control.selection.start().get();
    let end = control.selection.end().get();
    if start == end {
        return;
    }
    rectangles.extend(
        text_range_rects(layout, start, end)
            .into_iter()
            .map(|bounds| text_local_to_viewport_rect(control, bounds))
            .map(|bounds| PaintRect {
                bounds,
                rgba: palette.choice_active,
            }),
    );
}

fn push_renderer_text_input_caret(
    rectangles: &mut Vec<PaintRect>,
    control: &RenderTextInputControl,
    layout: &LaidOutText,
    palette: &Palette,
) {
    let caret = control.selection.end().get();
    rectangles.push(PaintRect {
        bounds: text_local_to_viewport_rect(control, text_caret_rect(control, layout, caret)),
        rgba: palette.focus_ring,
    });
}

fn text_range_rects(layout: &LaidOutText, start: u32, end: u32) -> Vec<HitRect> {
    let range = RichTextRange::new(
        usize::try_from(start.min(end)).unwrap_or(usize::MAX),
        usize::try_from(start.max(end)).unwrap_or(usize::MAX),
    );
    layout
        .glyphs
        .iter()
        .filter(|glyph| rich_ranges_overlap(glyph.range, range))
        .map(|glyph| layout_rect_to_hit_rect(glyph.bounds))
        .collect()
}

fn text_caret_rect(control: &RenderTextInputControl, layout: &LaidOutText, offset: u32) -> HitRect {
    let offset = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(control.value.len());
    if let Some(glyph) = layout
        .glyphs
        .iter()
        .find(|glyph| offset <= glyph.range.start)
    {
        return HitRect::new(
            glyph.bounds.x,
            glyph.bounds.y,
            CARET_WIDTH,
            glyph.bounds.height.max(1.0),
        );
    }
    layout.glyphs.last().map_or_else(
        || {
            let inner = text_inner_bounds(control);
            HitRect::new(
                inner.x,
                inner.y,
                CARET_WIDTH,
                text_control_line_height(control),
            )
        },
        |glyph| {
            HitRect::new(
                glyph.bounds.x + glyph.bounds.width,
                glyph.bounds.y,
                CARET_WIDTH,
                glyph.bounds.height.max(1.0),
            )
        },
    )
}

fn text_inner_bounds(control: &RenderTextInputControl) -> HitRect {
    let inner = text_local_inner_bounds(control);
    HitRect::new(
        control.bounds.x + inner.x,
        control.bounds.y + inner.y,
        inner.width,
        inner.height,
    )
}

fn text_local_inner_bounds(control: &RenderTextInputControl) -> HitRect {
    HitRect::new(
        TEXT_INSET_X,
        TEXT_INSET_Y,
        (control.bounds.width - TEXT_INSET_X * 2.0).max(0.0),
        (control.bounds.height - TEXT_INSET_Y * 2.0).max(1.0),
    )
}

fn text_local_to_viewport_rect(control: &RenderTextInputControl, rect: HitRect) -> HitRect {
    HitRect::new(
        control.bounds.x + rect.x,
        control.bounds.y + rect.y,
        rect.width,
        rect.height,
    )
}

fn text_control_font_size(control: &RenderTextInputControl) -> f32 {
    (control.bounds.height * 0.55).clamp(12.0, 28.0)
}

fn text_control_line_height(control: &RenderTextInputControl) -> f32 {
    let inner_height = (control.bounds.height - TEXT_INSET_Y * 2.0).max(1.0);
    (text_control_font_size(control) * 1.25)
        .max(1.0)
        .min(inner_height)
}

fn prepare_text_input_target(
    viewport: RenderViewport,
    control: &RenderTextInputControl,
    options: &TextInputOptions,
    laid_out: &LaidOutText,
) -> Result<PreparedTextInputTarget, FramePlanError> {
    let editor = TextEditorState::from_text_control(
        control.session,
        control.target.clone(),
        control.value.clone(),
        control.selection,
        options.clone(),
    )?;
    let scale_factor = viewport.scale_factor.to_f32().unwrap_or(f32::MAX);
    let layout = TextEditorGeometryPump::layout_from_laid_out_text(
        editor.text(),
        laid_out,
        TextEditorGeometryContext::default()
            .with_text_local_control_rect(HitRect::new(
                0.0,
                0.0,
                control.bounds.width,
                control.bounds.height,
            ))
            .with_text_local_to_viewport(TextGeometryTransform::translation(
                control.bounds.x,
                control.bounds.y,
            ))
            .with_viewport_to_screen(TextGeometryTransform::scale(scale_factor, scale_factor))
            .with_writing_mode(TextWritingMode::HorizontalTb),
    )
    .map_err(TextEditorError::from)?;
    let (snapshot, geometry) = editor.snapshots(&layout)?.into_parts();
    let security = TextInputSecurityPolicy::from_options(options);
    Ok(PreparedTextInputTarget {
        snapshot: security.redact_snapshot(&snapshot),
        geometry: security.redact_geometry(&geometry),
    })
}

fn laid_out_text_for_control(control: &RenderTextInputControl) -> LaidOutText {
    let inner = text_local_inner_bounds(control);
    let line_height = text_control_line_height(control);
    let font_size = text_control_font_size(control);
    let mut x = inner.x;
    let mut y = inner.y;
    let mut glyphs = Vec::new();
    for (start, ch) in control.value.char_indices() {
        let end = start.saturating_add(ch.len_utf8());
        if ch == '\n' {
            glyphs.push(LaidOutGlyph {
                run_index: 0,
                range: RichTextRange::new(start, end),
                text: String::new(),
                origin: LayoutPoint::new(inner.x, y + line_height),
                advance: LayoutSize::new(0.0, line_height),
                bounds: LayoutRect::new(inner.x, y + line_height, 0.0, line_height),
                writing_mode: RichTextWritingMode::HorizontalTb,
                orientation: GlyphOrientation::Upright,
                vertical_form: GlyphVerticalForm::None,
                presentation: RichTextPresentation::default(),
            });
            x = inner.x;
            y += line_height;
            continue;
        }
        let width = estimated_text_input_glyph_width(ch, font_size);
        glyphs.push(LaidOutGlyph {
            run_index: 0,
            range: RichTextRange::new(start, end),
            text: ch.to_string(),
            origin: LayoutPoint::new(x, y),
            advance: LayoutSize::new(width, 0.0),
            bounds: LayoutRect::new(x, y, width, line_height),
            writing_mode: RichTextWritingMode::HorizontalTb,
            orientation: GlyphOrientation::Upright,
            vertical_form: GlyphVerticalForm::None,
            presentation: RichTextPresentation::default(),
        });
        x += width;
    }
    LaidOutText {
        glyphs,
        runs: Vec::new(),
        ruby: Vec::new(),
        bounds: Some(LayoutRect::new(
            0.0,
            0.0,
            control.bounds.width.max(1.0),
            control.bounds.height.max(1.0),
        )),
    }
}

fn estimated_text_input_glyph_width(ch: char, font_size: f32) -> f32 {
    if ch.is_ascii_whitespace() {
        (font_size * 0.35).max(4.0)
    } else if ch.is_ascii() {
        (font_size * 0.55).max(7.0)
    } else {
        font_size.max(10.0)
    }
}

fn layout_rect_to_hit_rect(rect: LayoutRect) -> HitRect {
    HitRect::new(rect.x, rect.y, rect.width, rect.height)
}

fn rich_ranges_overlap(left: RichTextRange, right: RichTextRange) -> bool {
    left.start < right.end && right.start < left.end
}

fn mask_secure_text(value: &str) -> String {
    value.chars().map(|_| '*').collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_id::PublicId;

    fn target() -> InteractionTarget {
        InteractionTarget::new(PublicId::try_new("input.test").unwrap())
    }

    fn control(value: &str, selection: u32, height: f32) -> RenderTextInputControl {
        RenderTextInputControl::new(
            target(),
            TextInputSessionId(1),
            value,
            TextRange::new(TextByteOffset(selection), TextByteOffset(selection)),
            TextInputOptions::default(),
            SemanticRole::TextField,
            HitRect::new(40.0, 30.0, 420.0, height),
        )
    }

    #[test]
    fn single_line_caret_uses_font_size_not_full_line_box_width() {
        let control = control("Tokyo", 5, 48.0);
        let layout = laid_out_text_for_control(&control);
        let caret = text_local_to_viewport_rect(&control, text_caret_rect(&control, &layout, 5));

        assert!(caret.x > 110.0);
        assert!(
            caret.x < 140.0,
            "caret should stay near rendered Latin text, got {}",
            caret.x
        );
    }

    #[test]
    fn multiline_caret_moves_to_following_visual_line_after_newline() {
        let value = "line one\nTokyo";
        let caret_offset = u32::try_from(value.len()).unwrap();
        let control = control(value, caret_offset, 136.0);
        let layout = laid_out_text_for_control(&control);
        let first = text_local_to_viewport_rect(&control, text_caret_rect(&control, &layout, 0));
        let caret =
            text_local_to_viewport_rect(&control, text_caret_rect(&control, &layout, caret_offset));

        assert!(
            caret.y > first.y + 20.0,
            "caret did not move down: {caret:?}"
        );
        assert!(
            caret.x < first.x + 120.0,
            "caret should be relative to the second line, got {caret:?}"
        );
    }

    #[test]
    fn text_area_inner_bounds_allow_multiple_lines() {
        let control = control("line one\nTokyo", 14, 136.0);
        let inner = text_inner_bounds(&control);

        assert!(inner.height > text_control_line_height(&control) * 2.0);
    }
}
