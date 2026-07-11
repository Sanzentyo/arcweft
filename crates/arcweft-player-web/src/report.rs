use arcweft_presentation::fx::FxDiagnostic;
use arcweft_render_wgpu::geometry::{
    PreparedFrame, RenderFontFamily, RenderGlyphTransformKind, RenderImage, RenderStyledParagraph,
    RenderTextBlock, RenderTextSlant, RenderTextWeight,
};
use arcweft_render_wgpu::renderer::{
    StyledParagraphGlyphBounds, StyledParagraphGlyphTransformEvidence,
    StyledParagraphGlyphTransformRenderSupport, StyledParagraphLayoutEvidence,
    StyledParagraphLineBox, StyledParagraphRevealState, StyledParagraphSpanEvidence,
    StyledParagraphStyleEvidence, StyledParagraphTransformSupport,
};
use arcweft_runtime_driver::{dialogue::BundlePresentationTransition, session::BundleSessionStep};
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Path-free diagnostic/observation envelope emitted to JavaScript.
///
/// It is not a render protocol and contains no DOM construction instructions.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebObservationReport {
    pub schema_version: String,
    pub step_index: usize,
    pub logical_tick: u64,
    pub logical_dt_millis: u64,
    pub stop_reason: String,
    pub status: String,
    pub finished: bool,
    pub diagnostics: Vec<String>,
    pub fx_diagnostics: Vec<FxDiagnostic>,
    pub presentation_revision: u64,
    pub dialogue: Option<WebDialogueObservation>,
    pub presentation_transitions: Vec<BundlePresentationTransition>,
    pub choice_count: usize,
    pub image_count: usize,
    pub flow_event_count: usize,
    pub audio_commands: usize,
    pub requested_tasks: usize,
    pub queued_task_events: usize,
}

/// Input-gated dialogue state exposed for browser integration tests and hosts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebDialogueObservation {
    pub instance: u64,
    pub stage_index: u32,
    pub page_index: Option<u32>,
    pub page_count: usize,
    pub waiting_for_advance: bool,
}

impl WebObservationReport {
    pub fn from_step(step: &BundleSessionStep, queued_task_events: usize) -> Self {
        Self {
            schema_version: "arcweft.web_observation.v3".to_owned(),
            step_index: step.index,
            logical_tick: step.clock.tick().0,
            logical_dt_millis: step.clock.dt_millis(),
            stop_reason: step.stop_reason_label.clone(),
            status: step.status_label.clone(),
            finished: step.finished,
            diagnostics: step.diagnostics.clone(),
            fx_diagnostics: step.presentation.fx_diagnostics.clone(),
            presentation_revision: step.presentation.revision,
            dialogue: step
                .presentation
                .dialogue
                .as_ref()
                .map(|dialogue| WebDialogueObservation {
                    instance: dialogue.instance().get(),
                    stage_index: dialogue.stage_index().get(),
                    page_index: dialogue
                        .page_index()
                        .map(arcweft_runtime_driver::dialogue::DialoguePageIndex::get),
                    page_count: dialogue.page_count(),
                    waiting_for_advance: dialogue.is_waiting_for_advance(),
                }),
            presentation_transitions: step.presentation_transitions.clone(),
            choice_count: step.presentation.choices.len(),
            image_count: step.presentation.images.len(),
            flow_event_count: step.flow_events.len(),
            audio_commands: step.audio_commands.len(),
            requested_tasks: step.requested_tasks.len(),
            queued_task_events,
        }
    }
}

/// Host-neutral summary of the canvas frame produced by the shared planner.
///
/// This is diagnostic evidence for native/Web parity. It is not a renderer
/// command stream and intentionally carries normalized scalar values instead of
/// platform resources.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameObservationReport {
    pub schema_version: String,
    pub viewport: WebFrameViewport,
    pub rectangle_count: usize,
    pub image_count: usize,
    pub text_count: usize,
    pub styled_paragraph_count: usize,
    pub choice_count: usize,
    pub images: Vec<WebFrameImage>,
    pub text: Vec<WebFrameText>,
    pub styled_paragraphs: Vec<WebFrameStyledParagraph>,
    pub choices: Vec<WebFrameChoice>,
    pub focus: arcweft_render_wgpu::geometry::FocusNavigationDebug,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WebFrameReportError {
    #[error(
        "styled paragraph evidence count mismatch: expected {expected} entries, found {actual}"
    )]
    StyledParagraphEvidenceCountMismatch { expected: usize, actual: usize },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameViewport {
    pub logical_width_milli: i64,
    pub logical_height_milli: i64,
    pub physical_width: u32,
    pub physical_height: u32,
    pub scale_factor_milli: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameBounds {
    pub x_milli: i64,
    pub y_milli: i64,
    pub width_milli: i64,
    pub height_milli: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameImage {
    pub id: String,
    pub bounds: WebFrameBounds,
    pub frame_width: u32,
    pub frame_height: u32,
    pub opacity_milli: u16,
    pub rgba_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameText {
    pub text: String,
    pub bounds: WebFrameBounds,
    pub font_size_milli: i64,
    pub line_height_milli: i64,
    pub rgba: [u8; 4],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameStyledParagraph {
    pub text: String,
    pub bounds: WebFrameBounds,
    pub text_len: usize,
    pub visible_end: usize,
    pub default_style: WebFrameTextStyle,
    pub span_count: usize,
    pub line_box_count: usize,
    pub glyph_count: usize,
    pub glyph_transform_count: usize,
    pub transform_support: String,
    pub spans: Vec<WebFrameStyledSpan>,
    pub line_boxes: Vec<WebFrameStyledLineBox>,
    pub glyph_bounds: Vec<WebFrameStyledGlyph>,
    pub glyph_transforms: Vec<WebFrameGlyphTransform>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameStyledSpan {
    pub start: usize,
    pub end: usize,
    pub node_index: usize,
    pub font_size_milli: i64,
    pub line_height_milli: i64,
    pub rgba: [u8; 4],
    pub style: WebFrameTextStyle,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameTextStyle {
    pub font_size_milli: i64,
    pub line_height_milli: i64,
    pub rgba: [u8; 4],
    pub font_family: String,
    pub weight: String,
    pub slant: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameStyledLineBox {
    pub line_index: usize,
    pub bounds: WebFrameBounds,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameStyledGlyph {
    pub source_start: usize,
    pub source_end: usize,
    pub line_index: usize,
    pub bounds: WebFrameBounds,
    pub visible: bool,
    pub reveal_state: String,
    pub style: WebFrameTextStyle,
    pub glyph_transform: Option<WebFrameGlyphTransform>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameGlyphTransform {
    pub source_start: usize,
    pub source_end: usize,
    pub node_index: usize,
    pub kind: String,
    pub amplitude_milli: i64,
    pub frequency_milli: i64,
    pub sampled_offset_y_milli: i64,
    pub rendered: bool,
    pub support: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameChoice {
    pub option_id: String,
    pub label: String,
    pub target: String,
    pub bounds: WebFrameBounds,
}

impl WebFrameObservationReport {
    pub fn from_prepared_frame(
        frame: &PreparedFrame,
        paragraph_evidence: &[StyledParagraphLayoutEvidence],
    ) -> Result<Self, WebFrameReportError> {
        let expected = frame.styled_paragraphs.len();
        let actual = paragraph_evidence.len();
        if expected != actual {
            return Err(WebFrameReportError::StyledParagraphEvidenceCountMismatch {
                expected,
                actual,
            });
        }

        Ok(Self {
            schema_version: "arcweft.web_frame_observation.v3".to_owned(),
            viewport: WebFrameViewport {
                logical_width_milli: f64_milli(f64::from(frame.viewport.logical_width)),
                logical_height_milli: f64_milli(f64::from(frame.viewport.logical_height)),
                physical_width: frame.viewport.physical_width,
                physical_height: frame.viewport.physical_height,
                scale_factor_milli: f64_milli(frame.viewport.scale_factor),
            },
            rectangle_count: frame.rectangles.len(),
            image_count: frame.images.len(),
            text_count: frame.prepared_text.len()
                + frame.text.len()
                + frame.styled_paragraphs.len(),
            styled_paragraph_count: frame.styled_paragraphs.len(),
            choice_count: frame.choices.len(),
            images: frame
                .images
                .iter()
                .map(WebFrameImage::from_render_image)
                .collect(),
            text: frame
                .text
                .iter()
                .map(WebFrameText::from_text_block)
                .collect(),
            styled_paragraphs: frame
                .styled_paragraphs
                .iter()
                .zip(paragraph_evidence)
                .map(|(paragraph, evidence)| {
                    WebFrameStyledParagraph::from_styled_paragraph(paragraph, evidence)
                })
                .collect(),
            choices: frame
                .choices
                .iter()
                .filter_map(|choice| {
                    frame
                        .semantics
                        .find(&choice.target)
                        .map(|node| WebFrameChoice {
                            option_id: choice.option_id.clone(),
                            label: choice.label.clone(),
                            target: choice.target.id().as_str().to_owned(),
                            bounds: WebFrameBounds::from_hit_rect(node.bounds()),
                        })
                })
                .collect(),
            focus: frame.focus_debug(),
        })
    }
}

impl WebFrameBounds {
    fn from_hit_rect(bounds: arcweft_presentation::hit::HitRect) -> Self {
        Self {
            x_milli: f32_milli(bounds.x),
            y_milli: f32_milli(bounds.y),
            width_milli: f32_milli(bounds.width),
            height_milli: f32_milli(bounds.height),
        }
    }
}

impl WebFrameImage {
    fn from_render_image(image: &RenderImage) -> Self {
        Self {
            id: image.id.clone(),
            bounds: WebFrameBounds::from_hit_rect(image.visible_bounds().unwrap_or(image.bounds)),
            frame_width: image.frame.width,
            frame_height: image.frame.height,
            opacity_milli: image.opacity_milli,
            rgba_hash: stable_hash(&image.frame.rgba),
        }
    }
}

impl WebFrameText {
    fn from_text_block(text: &RenderTextBlock) -> Self {
        Self {
            text: text.text.clone(),
            bounds: WebFrameBounds::from_hit_rect(text.bounds),
            font_size_milli: f32_milli(text.font_size),
            line_height_milli: f32_milli(text.line_height),
            rgba: text.rgba,
        }
    }
}

impl WebFrameStyledParagraph {
    fn from_styled_paragraph(
        paragraph: &RenderStyledParagraph,
        evidence: &StyledParagraphLayoutEvidence,
    ) -> Self {
        let line_boxes = evidence
            .line_boxes
            .iter()
            .map(WebFrameStyledLineBox::from_line_box)
            .collect::<Vec<_>>();
        let glyph_bounds = evidence
            .glyph_bounds
            .iter()
            .map(WebFrameStyledGlyph::from_glyph_bounds)
            .collect::<Vec<_>>();
        let glyph_transforms = evidence
            .glyph_transforms
            .iter()
            .map(WebFrameGlyphTransform::from_transform)
            .collect::<Vec<_>>();
        Self {
            text: paragraph.text.clone(),
            bounds: WebFrameBounds::from_hit_rect(evidence.bounds),
            text_len: evidence.text_len,
            visible_end: evidence.visible_end,
            default_style: WebFrameTextStyle::from_style_evidence(&evidence.default_style),
            span_count: evidence.spans.len(),
            line_box_count: line_boxes.len(),
            glyph_count: glyph_bounds.len(),
            glyph_transform_count: glyph_transforms.len(),
            transform_support: transform_support_label(evidence.transform_support).to_owned(),
            spans: evidence
                .spans
                .iter()
                .map(WebFrameStyledSpan::from_span_evidence)
                .collect(),
            line_boxes,
            glyph_bounds,
            glyph_transforms,
        }
    }
}

impl WebFrameTextStyle {
    fn from_style_evidence(style: &StyledParagraphStyleEvidence) -> Self {
        Self {
            font_size_milli: f32_milli(style.font_size),
            line_height_milli: f32_milli(style.line_height),
            rgba: style.rgba,
            font_family: font_family_label(&style.font_family),
            weight: text_weight_label(style.weight).to_owned(),
            slant: text_slant_label(style.slant).to_owned(),
        }
    }
}

impl WebFrameStyledSpan {
    fn from_span_evidence(span: &StyledParagraphSpanEvidence) -> Self {
        let style = WebFrameTextStyle::from_style_evidence(&span.style);
        Self {
            start: span.range.start,
            end: span.range.end,
            node_index: span.node_index,
            font_size_milli: style.font_size_milli,
            line_height_milli: style.line_height_milli,
            rgba: style.rgba,
            style,
        }
    }
}

impl WebFrameStyledLineBox {
    fn from_line_box(line: &StyledParagraphLineBox) -> Self {
        Self {
            line_index: line.line_index,
            bounds: WebFrameBounds::from_hit_rect(line.bounds),
        }
    }
}

impl WebFrameStyledGlyph {
    fn from_glyph_bounds(glyph: &StyledParagraphGlyphBounds) -> Self {
        Self {
            source_start: glyph.source_range.start,
            source_end: glyph.source_range.end,
            line_index: glyph.line_index,
            bounds: WebFrameBounds::from_hit_rect(glyph.bounds),
            visible: glyph.visible,
            reveal_state: reveal_state_label(glyph.reveal_state).to_owned(),
            style: WebFrameTextStyle::from_style_evidence(&glyph.style),
            glyph_transform: glyph
                .glyph_transform
                .as_ref()
                .map(WebFrameGlyphTransform::from_transform),
        }
    }
}

impl WebFrameGlyphTransform {
    fn from_transform(transform: &StyledParagraphGlyphTransformEvidence) -> Self {
        Self {
            source_start: transform.range.start,
            source_end: transform.range.end,
            node_index: transform.node_index,
            kind: glyph_transform_kind_label(transform.motion.kind).to_owned(),
            amplitude_milli: f32_milli(transform.motion.amplitude),
            frequency_milli: f32_milli(transform.motion.frequency),
            sampled_offset_y_milli: f32_milli(transform.sampled_offset_y),
            rendered: transform.rendered,
            support: glyph_transform_render_support_label(transform.render_support).to_owned(),
        }
    }
}

fn f32_milli(value: f32) -> i64 {
    f64_milli(f64::from(value))
}

fn f64_milli(value: f64) -> i64 {
    let scaled = (value * 1_000.0).round();
    if !scaled.is_finite() {
        return 0;
    }
    scaled.to_i64().unwrap_or_else(|| {
        if scaled.is_sign_negative() {
            i64::MIN
        } else {
            i64::MAX
        }
    })
}

fn font_family_label(family: &RenderFontFamily) -> String {
    match family {
        RenderFontFamily::Serif => "serif".to_owned(),
        RenderFontFamily::SansSerif => "sans_serif".to_owned(),
        RenderFontFamily::Monospace => "monospace".to_owned(),
        RenderFontFamily::Cursive => "cursive".to_owned(),
        RenderFontFamily::Fantasy => "fantasy".to_owned(),
        RenderFontFamily::Named(name) => name.clone(),
        RenderFontFamily::Stack(stack) => stack.join(", "),
    }
}

fn text_weight_label(weight: RenderTextWeight) -> &'static str {
    match weight {
        RenderTextWeight::Regular => "regular",
        RenderTextWeight::Bold => "bold",
    }
}

fn text_slant_label(slant: RenderTextSlant) -> &'static str {
    match slant {
        RenderTextSlant::Upright => "upright",
        RenderTextSlant::Italic => "italic",
    }
}

fn reveal_state_label(state: StyledParagraphRevealState) -> &'static str {
    match state {
        StyledParagraphRevealState::Visible => "visible",
        StyledParagraphRevealState::PartiallyVisible => "partially_visible",
        StyledParagraphRevealState::Hidden => "hidden",
    }
}

fn transform_support_label(support: StyledParagraphTransformSupport) -> &'static str {
    match support {
        StyledParagraphTransformSupport::NoTransforms => "no_transforms",
        StyledParagraphTransformSupport::MetadataOnlyUnsupported => "metadata_only_unsupported",
        StyledParagraphTransformSupport::Rendered => "rendered",
    }
}

fn glyph_transform_kind_label(kind: RenderGlyphTransformKind) -> &'static str {
    match kind {
        RenderGlyphTransformKind::Wave => "wave",
        RenderGlyphTransformKind::Shake => "shake",
        RenderGlyphTransformKind::Jitter => "jitter",
    }
}

fn glyph_transform_render_support_label(
    support: StyledParagraphGlyphTransformRenderSupport,
) -> &'static str {
    match support {
        StyledParagraphGlyphTransformRenderSupport::MetadataOnlyUnsupported => {
            "metadata_only_unsupported"
        }
        StyledParagraphGlyphTransformRenderSupport::Rendered => "rendered",
    }
}

fn stable_hash(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    format!("{hash:016x}")
}
