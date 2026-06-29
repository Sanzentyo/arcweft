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
            push_renderer_text_input_selection(rectangles, control, palette);
            push_renderer_text_input_caret(rectangles, control, palette);
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
            )?);
        }
    }
    Ok(focused)
}

fn push_renderer_text_input_selection(
    rectangles: &mut Vec<PaintRect>,
    control: &RenderTextInputControl,
    palette: &Palette,
) {
    let start = control.selection.start().get();
    let end = control.selection.end().get();
    if start == end {
        return;
    }
    rectangles.push(PaintRect {
        bounds: text_range_rect(control, start, end),
        rgba: palette.choice_active,
    });
}

fn push_renderer_text_input_caret(
    rectangles: &mut Vec<PaintRect>,
    control: &RenderTextInputControl,
    palette: &Palette,
) {
    let caret = control.selection.end().get();
    let inner = text_inner_bounds(control);
    let x = inner.x + text_advance_to_byte(control, caret);
    rectangles.push(PaintRect {
        bounds: HitRect::new(x, inner.y, CARET_WIDTH, inner.height),
        rgba: palette.focus_ring,
    });
}

fn text_range_rect(control: &RenderTextInputControl, start: u32, end: u32) -> HitRect {
    let inner = text_inner_bounds(control);
    let x0 = inner.x + text_advance_to_byte(control, start);
    let x1 = inner.x + text_advance_to_byte(control, end);
    HitRect::new(x0.min(x1), inner.y, (x1 - x0).abs().max(1.0), inner.height)
}

fn text_inner_bounds(control: &RenderTextInputControl) -> HitRect {
    HitRect::new(
        control.bounds.x + TEXT_INSET_X,
        control.bounds.y + TEXT_INSET_Y,
        (control.bounds.width - TEXT_INSET_X * 2.0).max(0.0),
        text_control_line_height(control),
    )
}

fn text_control_font_size(control: &RenderTextInputControl) -> f32 {
    (control.bounds.height * 0.55).clamp(12.0, 28.0)
}

fn text_control_line_height(control: &RenderTextInputControl) -> f32 {
    (control.bounds.height - TEXT_INSET_Y * 2.0).max(1.0)
}

fn prepare_text_input_target(
    viewport: RenderViewport,
    control: &RenderTextInputControl,
    options: &TextInputOptions,
) -> Result<PreparedTextInputTarget, FramePlanError> {
    let editor = TextEditorState::from_text_control(
        control.session,
        control.target.clone(),
        control.value.clone(),
        control.selection,
        options.clone(),
    )?;
    let laid_out = laid_out_text_for_control(control);
    let scale_factor = viewport.scale_factor.to_f32().unwrap_or(f32::MAX);
    let layout = TextEditorGeometryPump::layout_from_laid_out_text(
        editor.text(),
        &laid_out,
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
    let line_height = text_control_line_height(control);
    let mut x = TEXT_INSET_X;
    let mut glyphs = Vec::new();
    for (start, ch) in control.value.char_indices() {
        let end = start.saturating_add(ch.len_utf8());
        let width = estimated_text_input_glyph_width(ch, line_height);
        glyphs.push(LaidOutGlyph {
            run_index: 0,
            range: RichTextRange::new(start, end),
            text: ch.to_string(),
            origin: LayoutPoint::new(x, 4.0),
            advance: LayoutSize::new(width, 0.0),
            bounds: LayoutRect::new(x, 4.0, width, line_height),
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
        bounds: Some(LayoutRect::new(0.0, 0.0, x, control.bounds.height.max(1.0))),
    }
}

fn estimated_text_input_glyph_width(ch: char, line_height: f32) -> f32 {
    if ch.is_ascii() {
        (line_height * 0.55).max(7.0)
    } else {
        (line_height * 0.9).max(10.0)
    }
}

fn text_advance_to_byte(control: &RenderTextInputControl, byte_offset: u32) -> f32 {
    let limit = usize::try_from(byte_offset)
        .unwrap_or(usize::MAX)
        .min(control.value.len());
    control
        .value
        .char_indices()
        .take_while(|(index, _)| *index < limit)
        .map(|(_, ch)| estimated_text_input_glyph_width(ch, text_control_line_height(control)))
        .sum::<f32>()
        .min((control.bounds.width - TEXT_INSET_X * 2.0).max(0.0))
}

fn mask_secure_text(value: &str) -> String {
    value.chars().map(|_| '*').collect()
}
