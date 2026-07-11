//! Dialogue-specific render input and paragraph projection.
//!
//! The parent geometry module owns whole-frame orchestration. This module owns
//! the dialogue panel, rich-text projection, and the renderer-facing paragraph
//! model; [`super::dialogue_timeline`] separately evaluates reveal timing.

use super::{
    PaintRect, Palette, RenderFontFamily, RenderScene, RenderTextBlock, RenderTextSelectionPolicy,
    RenderTextSlant, RenderTextStyle, RenderTextWeight, RenderViewport,
};
use arcweft_presentation::hit::HitRect;
use arcweft_render_text::{
    LineDisplayFrame, LineDisplayStage, RichTextColor, RichTextControlMarker,
    RichTextEffectDescriptor, RichTextEffectPhase, RichTextFontFamily, RichTextParam,
    RichTextRange, RichTextStyle, RichTextTextRun, RichTextTextSource, presentation_from_styles,
};
use num_traits::ToPrimitive;

/// Minimal dialogue data consumed by the shared renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderDialogue {
    pub speaker: String,
    pub text: String,
    pub base_styles: Vec<RichTextStyle>,
    pub text_runs: Vec<RichTextTextRun>,
    pub controls: Vec<RichTextControlMarker>,
    /// Page-local byte offset through which text was revealed by prior `[l]` stages.
    pub reveal_start: usize,
    /// Whether the current stage must be shown fully regardless of visual time.
    pub reveal_complete: bool,
}

impl RenderDialogue {
    pub fn plain(speaker: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            speaker: speaker.into(),
            text: text.into(),
            base_styles: Vec::new(),
            text_runs: Vec::new(),
            controls: Vec::new(),
            reveal_start: 0,
            reveal_complete: false,
        }
    }

    /// Chooses whether this stage bypasses the visual reveal timeline.
    #[must_use]
    pub fn with_reveal_complete(mut self, reveal_complete: bool) -> Self {
        self.reveal_complete = reveal_complete;
        self
    }

    /// Updates whether this stage bypasses the visual reveal timeline.
    pub fn set_reveal_complete(&mut self, reveal_complete: bool) {
        self.reveal_complete = reveal_complete;
    }

    pub fn from_display_frame(frame: &LineDisplayFrame) -> Self {
        Self {
            speaker: frame
                .speaker_label
                .clone()
                .unwrap_or_else(|| frame.callee.clone()),
            text: frame.text.clone(),
            base_styles: frame.base_styles.clone(),
            text_runs: frame.display_map.text_runs.clone(),
            controls: frame.display_map.controls.clone(),
            reveal_start: 0,
            reveal_complete: false,
        }
    }

    pub fn from_display_stage(stage: LineDisplayStage<'_>) -> Self {
        let frame = stage.frame();
        Self {
            speaker: frame
                .speaker_label
                .clone()
                .unwrap_or_else(|| frame.callee.clone()),
            text: stage.text().to_owned(),
            base_styles: frame.base_styles.clone(),
            text_runs: stage.text_runs(),
            controls: stage.controls(),
            reveal_start: stage.reveal_start(),
            reveal_complete: false,
        }
    }
}

/// One dialogue body laid out as a single rich-text paragraph.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderStyledParagraph {
    pub text: String,
    pub bounds: HitRect,
    pub default_style: RenderTextStyle,
    pub spans: Vec<RenderStyledTextSpan>,
    pub reveal: RenderTextReveal,
    pub glyph_transforms: Vec<RenderGlyphTransformSpan>,
    pub visual_time_millis: u64,
}

impl RenderStyledParagraph {
    #[must_use]
    pub fn reveal_complete(&self) -> bool {
        self.reveal.complete
    }
}

/// One typed style span inside a renderer-owned paragraph.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderStyledTextSpan {
    pub range: RichTextRange,
    pub style: RenderTextStyle,
    pub node_index: usize,
}

/// Source-range reveal mask for effects such as typewriter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderTextReveal {
    pub visible_end: usize,
    pub complete: bool,
}

/// One post-layout glyph transform span inside a renderer-owned paragraph.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderGlyphTransformSpan {
    pub range: RichTextRange,
    pub motion: RenderGlyphMotion,
    pub node_index: usize,
}

/// Supported deterministic glyph-position transform kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderGlyphTransformKind {
    Wave,
    Shake,
    Jitter,
}

impl RenderGlyphTransformKind {
    fn from_effect_id(id: &str) -> Option<Self> {
        match id {
            "wave" => Some(Self::Wave),
            "shake" => Some(Self::Shake),
            "jitter" => Some(Self::Jitter),
            _ => None,
        }
    }
}

/// Deterministic glyph-position transform parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderGlyphMotion {
    pub kind: RenderGlyphTransformKind,
    pub amplitude: f32,
    pub frequency: f32,
}

impl RenderGlyphMotion {
    pub fn offset_y(self, seconds: f32, source_byte: usize) -> f32 {
        let source_phase = source_byte.to_f32().unwrap_or(f32::MAX) * 0.58;
        let phase = seconds.mul_add(self.frequency, source_phase);
        match self.kind {
            RenderGlyphTransformKind::Wave => phase.sin() * self.amplitude,
            RenderGlyphTransformKind::Shake => {
                ((phase * 1.7).sin() * 0.6 + (phase * 2.3).cos() * 0.4) * self.amplitude
            }
            RenderGlyphTransformKind::Jitter => (phase.sin() * 12_989.0).sin() * self.amplitude,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct DialogueTextLayout {
    bounds: HitRect,
    style: RenderTextStyle,
    visual_time_millis: u64,
    reduce_motion: bool,
}

/// Adds the dialogue panel, speaker label, and rich-text paragraph to a frame.
pub(super) fn push_panel(
    scene: &RenderScene,
    rectangles: &mut Vec<PaintRect>,
    text: &mut Vec<RenderTextBlock>,
    styled_paragraphs: &mut Vec<RenderStyledParagraph>,
    palette: &Palette,
) {
    let Some(dialogue) = &scene.dialogue else {
        return;
    };
    let panel = panel_bounds(scene.viewport);
    rectangles.push(PaintRect::new(panel, palette.dialogue_panel));
    let inset = 28.0;
    let scale = f32::from(scene.preferences.text_scale_milli) / 1_000.0;
    let base_style = text_style_from_styles(
        &dialogue.base_styles,
        RenderTextStyle::new(
            25.0 * scale,
            34.0 * scale,
            palette.dialogue_text,
            RenderFontFamily::SansSerif,
        ),
    );
    let speaker_style = RenderTextStyle {
        font_size: (base_style.font_size * 0.8).max(16.0 * scale),
        line_height: (base_style.line_height * 0.78).max(24.0 * scale),
        color: base_style.color,
        font_family: base_style.font_family.clone(),
        weight: RenderTextWeight::Bold,
        slant: base_style.slant,
    };
    text.push(RenderTextBlock {
        target: None,
        text: dialogue.speaker.clone(),
        bounds: HitRect::new(
            panel.x + inset,
            panel.y + 20.0,
            panel.width - inset * 2.0,
            28.0 * scale,
        ),
        clip_bounds: None,
        buffer_width: None,
        buffer_height: None,
        font_size: speaker_style.font_size,
        line_height: speaker_style.line_height,
        font_family: speaker_style.font_family,
        weight: speaker_style.weight,
        slant: speaker_style.slant,
        rgba: if dialogue.base_styles.is_empty() {
            palette.speaker_text
        } else {
            speaker_style.color
        },
        selection_policy: RenderTextSelectionPolicy::Disabled,
        selection: None,
        selection_rgba: palette.choice_active,
    });
    push_styled_paragraph(
        styled_paragraphs,
        dialogue,
        &DialogueTextLayout {
            bounds: HitRect::new(
                panel.x + inset,
                panel.y + 58.0,
                panel.width - inset * 2.0,
                panel.height - 76.0,
            ),
            style: base_style,
            visual_time_millis: scene.visual_time_millis,
            reduce_motion: scene.preferences.reduce_motion,
        },
    );
}

/// Returns the viewport-relative panel used by dialogue and choice placement.
pub(super) fn panel_bounds(viewport: RenderViewport) -> HitRect {
    let margin = (viewport.logical_width * 0.045).max(24.0);
    let height = (viewport.logical_height * 0.28).clamp(180.0, 320.0);
    HitRect::new(
        margin,
        viewport.logical_height - height - margin,
        viewport.logical_width - margin * 2.0,
        height,
    )
}

fn push_styled_paragraph(
    styled_paragraphs: &mut Vec<RenderStyledParagraph>,
    dialogue: &RenderDialogue,
    layout: &DialogueTextLayout,
) {
    let runs = dialogue_text_runs(dialogue);
    let reveal = super::dialogue_timeline::evaluate_dialogue_reveal(
        &dialogue.text,
        &runs,
        &dialogue.controls,
        dialogue.reveal_start,
        super::dialogue_timeline::DialogueRevealPolicy {
            complete_stage: dialogue.reveal_complete,
            instant_characters: layout.reduce_motion,
        },
        layout.visual_time_millis,
    );
    let displayed = dialogue_from_offset(dialogue, &runs, reveal.display_start);
    let spans = displayed
        .text_runs
        .iter()
        .filter_map(|run| render_styled_text_span(&displayed, layout, run))
        .collect();
    let glyph_transforms = if layout.reduce_motion {
        Vec::new()
    } else {
        displayed
            .text_runs
            .iter()
            .filter_map(|run| render_glyph_transform_span(&displayed, run))
            .collect()
    };
    styled_paragraphs.push(RenderStyledParagraph {
        text: displayed.text.clone(),
        bounds: layout.bounds,
        default_style: layout.style.clone(),
        spans,
        reveal: RenderTextReveal {
            visible_end: reveal
                .visible_end
                .saturating_sub(reveal.display_start)
                .min(displayed.text.len()),
            complete: reveal.complete,
        },
        glyph_transforms,
        visual_time_millis: layout.visual_time_millis,
    });
}

fn dialogue_from_offset(
    dialogue: &RenderDialogue,
    runs: &[RichTextTextRun],
    display_start: usize,
) -> RenderDialogue {
    let display_start = display_start.min(dialogue.text.len());
    let text = dialogue
        .text
        .get(display_start..)
        .unwrap_or_default()
        .to_owned();
    let text_runs = runs
        .iter()
        .filter_map(|run| {
            let start = run.range.start.max(display_start);
            let end = run.range.end.min(dialogue.text.len());
            (start < end).then(|| {
                let mut run = run.clone();
                run.range = RichTextRange::new(start - display_start, end - display_start);
                run
            })
        })
        .collect();
    let controls = dialogue
        .controls
        .iter()
        .filter(|marker| marker.text_offset >= display_start)
        .cloned()
        .map(|mut marker| {
            marker.text_offset -= display_start;
            marker.range = marker.range.and_then(|range| {
                let start = range.start.max(display_start);
                let end = range.end.min(dialogue.text.len());
                (start < end)
                    .then(|| RichTextRange::new(start - display_start, end - display_start))
            });
            marker
        })
        .collect();
    RenderDialogue {
        speaker: dialogue.speaker.clone(),
        text,
        base_styles: dialogue.base_styles.clone(),
        text_runs,
        controls,
        reveal_start: dialogue.reveal_start.saturating_sub(display_start),
        reveal_complete: dialogue.reveal_complete,
    }
}

fn dialogue_text_runs(dialogue: &RenderDialogue) -> Vec<RichTextTextRun> {
    if !dialogue.text_runs.is_empty() {
        return dialogue.text_runs.clone();
    }
    let styles = dialogue.base_styles.clone();
    vec![RichTextTextRun {
        range: RichTextRange::new(0, dialogue.text.len()),
        source: RichTextTextSource::Text,
        node_index: 0,
        presentation: presentation_from_styles(&styles),
        styles,
    }]
}

fn render_styled_text_span(
    dialogue: &RenderDialogue,
    layout: &DialogueTextLayout,
    run: &RichTextTextRun,
) -> Option<RenderStyledTextSpan> {
    let range = valid_text_range(&dialogue.text, run.range)?;
    Some(RenderStyledTextSpan {
        range,
        style: text_style_from_styles(&run.styles, layout.style.clone()),
        node_index: run.node_index,
    })
}

fn render_glyph_transform_span(
    dialogue: &RenderDialogue,
    run: &RichTextTextRun,
) -> Option<RenderGlyphTransformSpan> {
    let range = valid_text_range(&dialogue.text, run.range)?;
    Some(RenderGlyphTransformSpan {
        range,
        motion: glyph_motion(&run.presentation.effects)?,
        node_index: run.node_index,
    })
}

fn valid_text_range(text: &str, range: RichTextRange) -> Option<RichTextRange> {
    let start = range.start.min(text.len());
    let end = range.end.min(text.len());
    if start >= end || text.get(start..end).is_none() {
        return None;
    }
    Some(RichTextRange::new(start, end))
}

fn glyph_motion(effects: &[RichTextEffectDescriptor]) -> Option<RenderGlyphMotion> {
    effects
        .iter()
        .find(|effect| {
            RenderGlyphTransformKind::from_effect_id(&effect.id).is_some()
                && effect.phase == RichTextEffectPhase::GlyphTransform
        })
        .map(|effect| RenderGlyphMotion {
            kind: RenderGlyphTransformKind::from_effect_id(&effect.id)
                .expect("effect id was matched above"),
            amplitude: effect
                .params
                .get("amp")
                .or_else(|| effect.params.get("amplitude"))
                .and_then(param_f32)
                .unwrap_or(4.0)
                .clamp(0.0, 24.0),
            frequency: effect
                .params
                .get("freq")
                .or_else(|| effect.params.get("frequency"))
                .and_then(param_f32)
                .unwrap_or(7.0)
                .clamp(0.1, 24.0),
        })
}

fn text_style_from_styles(styles: &[RichTextStyle], fallback: RenderTextStyle) -> RenderTextStyle {
    styles.iter().fold(fallback, apply_text_style)
}

fn apply_text_style(mut style: RenderTextStyle, rich_style: &RichTextStyle) -> RenderTextStyle {
    match rich_style {
        RichTextStyle::Em { .. } | RichTextStyle::Italic { .. } | RichTextStyle::Oblique { .. } => {
            style.slant = RenderTextSlant::Italic;
        }
        RichTextStyle::Strong { .. } => style.weight = RenderTextWeight::Bold,
        RichTextStyle::Color { value } => style.color = rich_text_color(value),
        RichTextStyle::Font { family } => {
            style.font_family = RenderFontFamily::from_rich_text(family);
        }
        RichTextStyle::Size {
            points: Some(points),
            ..
        } => {
            style.font_size = f32::from(*points);
            style.line_height = style.font_size * 1.35;
        }
        RichTextStyle::Size { points: None, .. }
        | RichTextStyle::Speed { .. }
        | RichTextStyle::Layout { .. }
        | RichTextStyle::Transform { .. }
        | RichTextStyle::Presentation { .. }
        | RichTextStyle::Effect { .. }
        | RichTextStyle::Shader { .. }
        | RichTextStyle::Fx { .. }
        | RichTextStyle::Object { .. }
        | RichTextStyle::Unknown { .. } => {}
    }
    style
}

impl RenderFontFamily {
    fn from_rich_text(family: &RichTextFontFamily) -> Self {
        match family {
            RichTextFontFamily::Serif => Self::Serif,
            RichTextFontFamily::SansSerif => Self::SansSerif,
            RichTextFontFamily::Monospace => Self::Monospace,
            RichTextFontFamily::Cursive => Self::Cursive,
            RichTextFontFamily::Fantasy => Self::Fantasy,
            RichTextFontFamily::Named { name } => Self::Named(name.clone()),
        }
    }
}

fn rich_text_color(color: &RichTextColor) -> [u8; 4] {
    match color {
        RichTextColor::Rgb { red, green, blue } => [*red, *green, *blue, 255],
        RichTextColor::Named { name } => match name.as_str() {
            "red" => [240, 110, 110, 255],
            "green" => [120, 220, 150, 255],
            "blue" => [130, 180, 255, 255],
            "yellow" => [240, 220, 120, 255],
            "muted" | "quiet" => [170, 170, 170, 255],
            _ => [245, 245, 245, 255],
        },
    }
}

fn param_f32(param: &RichTextParam) -> Option<f32> {
    match param {
        RichTextParam::Int { value } => value.to_f32(),
        RichTextParam::Milli { value } => Some(value.as_f32()),
        RichTextParam::Text { value } | RichTextParam::Raw { value } => {
            value.trim().trim_end_matches("px").parse().ok()
        }
        RichTextParam::Bool { .. }
        | RichTextParam::Vec2 { .. }
        | RichTextParam::Selector { .. }
        | RichTextParam::Expr { .. } => None,
    }
}
