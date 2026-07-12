use arcweft_glyphon::{PreparedTextItem, TextGlyphPaint};
use arcweft_presentation::fx::FxDiagnostic;
use arcweft_render_text::{
    ResolvedTextStyle, RichTextInlineDirection, RichTextRange, RichTextWritingMode, TextFontFamily,
    TextSlant, TextWeight,
};
use arcweft_render_wgpu::geometry::{
    PreparedFrame, PreparedTextOwner, PreparedTextOwnerKind, RenderImage,
};
use arcweft_runtime_driver::{dialogue::BundlePresentationTransition, session::BundleSessionStep};
use arcweft_text_layout::{GlyphOrientation, GlyphVerticalForm, LayoutRect};
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};

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
    pub dialogue: u64,
    pub entry: u64,
    pub revision: u64,
    pub view: String,
    pub view_mount: Option<u64>,
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
                .latest_active()
                .map(|(dialogue, entry)| WebDialogueObservation {
                    dialogue: dialogue.id().get(),
                    entry: entry.id().get(),
                    revision: dialogue.revision().get(),
                    view: dialogue.view().as_str().to_owned(),
                    view_mount: step
                        .presentation
                        .view
                        .mounts
                        .iter()
                        .find(|mount| {
                            mount.handle == entry.view_handle_id()
                                && mount.path.segments().is_empty()
                        })
                        .map(|mount| mount.mount.get()),
                    instance: entry.instance().get(),
                    stage_index: entry.stage_index().get(),
                    page_index: entry
                        .page_index()
                        .map(arcweft_runtime_driver::dialogue::DialoguePageIndex::get),
                    page_count: entry.page_count(),
                    waiting_for_advance: entry.is_waiting_for_advance(),
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

/// Host-neutral summary of the exact frame consumed by the shared renderer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameObservationReport {
    pub schema_version: String,
    pub fx_diagnostics: Vec<FxDiagnostic>,
    pub viewport: WebFrameViewport,
    pub rectangle_count: usize,
    pub image_count: usize,
    pub text_count: usize,
    pub choice_count: usize,
    pub images: Vec<WebFrameImage>,
    pub text: Vec<WebFramePreparedText>,
    pub choices: Vec<WebFrameChoice>,
    pub focus: arcweft_render_wgpu::geometry::FocusNavigationDebug,
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

/// Canonical layout and paint evidence for one frame-local prepared item.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFramePreparedText {
    pub id: u32,
    pub text: String,
    pub visible_text: String,
    pub bounds: Option<WebFrameBounds>,
    pub layout_hash: String,
    pub source_origin: usize,
    pub owner: Option<WebFramePreparedTextOwner>,
    pub line_count: usize,
    pub run_count: usize,
    pub glyph_count: usize,
    pub visible_glyph_count: usize,
    pub ruby_count: usize,
    pub runs: Vec<WebFramePreparedRun>,
    pub lines: Vec<WebFramePreparedLine>,
    pub glyphs: Vec<WebFramePreparedGlyph>,
    pub ruby: Vec<WebFramePreparedRuby>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFramePreparedTextOwner {
    pub semantic_id: String,
    pub parent_id: Option<String>,
    pub kind: String,
    pub object_bounds: WebFrameBounds,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFramePreparedRun {
    pub run_index: u32,
    pub source_start: usize,
    pub source_end: usize,
    pub glyph_start: u32,
    pub glyph_end: u32,
    pub bounds: WebFrameBounds,
    pub style: WebFrameTextStyle,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFramePreparedLine {
    pub source_start: usize,
    pub source_end: usize,
    pub glyph_start: u32,
    pub glyph_end: u32,
    pub bounds: WebFrameBounds,
    pub writing_mode: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFramePreparedGlyph {
    pub source_start: usize,
    pub source_end: usize,
    pub line_index: u32,
    pub cluster_index: u32,
    pub logical_ordinal: u32,
    pub bounds: WebFrameBounds,
    pub ink_bounds: WebFrameBounds,
    pub visible: bool,
    pub opacity_milli: u16,
    pub rgba: [u8; 4],
    pub orientation: String,
    pub vertical_form: String,
    pub inline_scale_milli: i64,
    pub transform: WebFrameGlyphTransform,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameGlyphTransform {
    pub matrix_milli: [i64; 4],
    pub translate_milli: [i64; 2],
    pub opacity_milli: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFramePreparedRuby {
    pub base_start: usize,
    pub base_end: usize,
    pub text: String,
    pub base_bounds: WebFrameBounds,
    pub ruby_bounds: WebFrameBounds,
    pub glyph_count: usize,
    pub writing_mode: String,
    pub style: WebFrameTextStyle,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameTextStyle {
    pub font_size_milli: u32,
    pub line_height_milli: u32,
    pub rgba: [u8; 4],
    pub font_families: Vec<String>,
    pub weight: String,
    pub slant: String,
    pub writing_mode: String,
    pub direction: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameChoice {
    pub option_id: String,
    pub label: String,
    pub target: String,
    pub bounds: WebFrameBounds,
}

impl WebFrameObservationReport {
    #[must_use]
    pub fn from_prepared_frame(frame: &PreparedFrame) -> Self {
        Self {
            schema_version: "arcweft.web_frame_observation.v3".to_owned(),
            fx_diagnostics: frame.fx_diagnostics.clone(),
            viewport: WebFrameViewport {
                logical_width_milli: f64_milli(f64::from(frame.viewport.logical_width)),
                logical_height_milli: f64_milli(f64::from(frame.viewport.logical_height)),
                physical_width: frame.viewport.physical_width,
                physical_height: frame.viewport.physical_height,
                scale_factor_milli: f64_milli(frame.viewport.scale_factor),
            },
            rectangle_count: frame.rectangles.len(),
            image_count: frame.images.len(),
            text_count: frame.text.len(),
            choice_count: frame.choices.len(),
            images: frame
                .images
                .iter()
                .map(WebFrameImage::from_render_image)
                .collect(),
            text: frame
                .text
                .iter()
                .map(|(id, item)| {
                    let owner = frame
                        .prepared_text_owners()
                        .iter()
                        .find(|owner| owner.text == id);
                    WebFramePreparedText::new(id.index(), item, owner)
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
        }
    }
}

impl WebFramePreparedText {
    fn new(id: u32, item: &PreparedTextItem, owner: Option<&PreparedTextOwner>) -> Self {
        let visible_ranges = visible_body_ranges(item);
        let glyphs = item
            .layout
            .glyphs
            .iter()
            .zip(&item.paint.glyphs)
            .map(|(glyph, paint)| {
                let opacity_milli = paint.effective_opacity_milli();
                WebFramePreparedGlyph {
                    source_start: glyph.source_range.start,
                    source_end: glyph.source_range.end,
                    line_index: glyph.line_index,
                    cluster_index: glyph.cluster_index,
                    logical_ordinal: glyph.logical_ordinal,
                    bounds: WebFrameBounds::from_layout_rect(glyph.layout_bounds),
                    ink_bounds: WebFrameBounds::from_layout_rect(glyph.ink_bounds),
                    visible: paint.visible && opacity_milli > 0,
                    opacity_milli,
                    rgba: paint.color.channels(),
                    orientation: orientation_label(glyph.orientation).to_owned(),
                    vertical_form: vertical_form_label(glyph.vertical_form).to_owned(),
                    inline_scale_milli: f32_milli(glyph.inline_scale),
                    transform: WebFrameGlyphTransform::from_paint(paint),
                }
            })
            .collect::<Vec<_>>();
        Self {
            id,
            text: item.interaction.text.clone(),
            visible_text: text_for_ranges(&item.interaction.text, &visible_ranges),
            bounds: item
                .interaction
                .container_bounds
                .or(item.layout.bounds)
                .map(WebFrameBounds::from_layout_rect),
            layout_hash: hex_bytes(&item.layout.hash.as_bytes()),
            source_origin: owner.map_or(0, |owner| owner.source_origin),
            owner: owner.map(WebFramePreparedTextOwner::from_owner),
            line_count: item.layout.lines.len(),
            run_count: item.layout.runs.len(),
            glyph_count: item.layout.glyphs.len(),
            visible_glyph_count: glyphs.iter().filter(|glyph| glyph.visible).count(),
            ruby_count: item.layout.ruby.len(),
            runs: item
                .layout
                .runs
                .iter()
                .map(|run| WebFramePreparedRun {
                    run_index: run.run_index,
                    source_start: run.source_range.start,
                    source_end: run.source_range.end,
                    glyph_start: run.glyph_range.start,
                    glyph_end: run.glyph_range.end,
                    bounds: WebFrameBounds::from_layout_rect(run.bounds),
                    style: WebFrameTextStyle::from_resolved(&run.style),
                })
                .collect(),
            lines: item
                .layout
                .lines
                .iter()
                .map(|line| WebFramePreparedLine {
                    source_start: line.source_range.start,
                    source_end: line.source_range.end,
                    glyph_start: line.glyph_range.start,
                    glyph_end: line.glyph_range.end,
                    bounds: WebFrameBounds::from_layout_rect(line.bounds),
                    writing_mode: writing_mode_label(line.writing_mode).to_owned(),
                })
                .collect(),
            glyphs,
            ruby: item
                .layout
                .ruby
                .iter()
                .map(|ruby| WebFramePreparedRuby {
                    base_start: ruby.base_range.start,
                    base_end: ruby.base_range.end,
                    text: ruby.text.clone(),
                    base_bounds: WebFrameBounds::from_layout_rect(ruby.base_bounds),
                    ruby_bounds: WebFrameBounds::from_layout_rect(ruby.ruby_bounds),
                    glyph_count: ruby.glyphs.len(),
                    writing_mode: writing_mode_label(ruby.writing_mode).to_owned(),
                    style: WebFrameTextStyle::from_resolved(&ruby.style),
                })
                .collect(),
        }
    }
}

impl WebFramePreparedTextOwner {
    fn from_owner(owner: &PreparedTextOwner) -> Self {
        Self {
            semantic_id: owner.semantic_id.as_str().to_owned(),
            parent_id: owner
                .parent_id
                .as_ref()
                .map(|parent| parent.as_str().to_owned()),
            kind: owner_kind_label(owner.kind),
            object_bounds: WebFrameBounds::from_hit_rect(owner.object_bounds),
        }
    }
}

impl WebFrameGlyphTransform {
    fn from_paint(paint: &TextGlyphPaint) -> Self {
        let resolved = paint.transform.resolved();
        let matrix = resolved.matrix();
        let translation = resolved.translation();
        Self {
            matrix_milli: matrix.map(|value| f32_milli(value.get())),
            translate_milli: translation.map(|value| f32_milli(value.pixels())),
            opacity_milli: f32_milli(resolved.opacity().value().get()),
        }
    }
}

impl WebFrameTextStyle {
    fn from_resolved(style: &ResolvedTextStyle) -> Self {
        Self {
            font_size_milli: style.font_size_milli(),
            line_height_milli: style.line_height_milli(),
            rgba: style.color().channels(),
            font_families: style
                .font_families()
                .iter()
                .map(font_family_label)
                .collect(),
            weight: weight_label(style.weight()).to_owned(),
            slant: slant_label(style.slant()).to_owned(),
            writing_mode: writing_mode_label(style.writing_mode()).to_owned(),
            direction: direction_label(style.direction()).to_owned(),
        }
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

    fn from_layout_rect(bounds: LayoutRect) -> Self {
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

fn visible_body_ranges(item: &PreparedTextItem) -> Vec<RichTextRange> {
    let mut ranges = item
        .layout
        .glyphs
        .iter()
        .zip(&item.paint.glyphs)
        .filter(|(_, paint)| paint.visible && paint.effective_opacity_milli() > 0)
        .map(|(glyph, _)| glyph.source_range)
        .collect::<Vec<_>>();
    ranges.sort_by_key(|range| (range.start, range.end));
    ranges.into_iter().fold(Vec::new(), |mut merged, range| {
        if let Some(last) = merged.last_mut()
            && range.start <= last.end
        {
            last.end = last.end.max(range.end);
        } else {
            merged.push(range);
        }
        merged
    })
}

fn text_for_ranges(text: &str, ranges: &[RichTextRange]) -> String {
    ranges
        .iter()
        .filter_map(|range| text.get(range.start..range.end))
        .collect()
}

fn owner_kind_label(kind: PreparedTextOwnerKind) -> String {
    match kind {
        PreparedTextOwnerKind::DialogueView {
            dialogue,
            entry,
            mount,
        } => format!("dialogue:{dialogue}:{entry}:{mount}"),
        PreparedTextOwnerKind::View { mount } => format!("view:{mount}"),
        PreparedTextOwnerKind::Control => "control".to_owned(),
    }
}

fn font_family_label(family: &TextFontFamily) -> String {
    match family {
        TextFontFamily::Serif => "serif".to_owned(),
        TextFontFamily::SansSerif => "sans_serif".to_owned(),
        TextFontFamily::Monospace => "monospace".to_owned(),
        TextFontFamily::Cursive => "cursive".to_owned(),
        TextFontFamily::Fantasy => "fantasy".to_owned(),
        TextFontFamily::Named(name) => name.clone(),
    }
}

const fn weight_label(weight: TextWeight) -> &'static str {
    match weight {
        TextWeight::Thin => "thin",
        TextWeight::ExtraLight => "extra_light",
        TextWeight::Light => "light",
        TextWeight::Normal => "normal",
        TextWeight::Medium => "medium",
        TextWeight::SemiBold => "semi_bold",
        TextWeight::Bold => "bold",
        TextWeight::ExtraBold => "extra_bold",
        TextWeight::Black => "black",
    }
}

const fn slant_label(slant: TextSlant) -> &'static str {
    match slant {
        TextSlant::Upright => "upright",
        TextSlant::Italic => "italic",
        TextSlant::Oblique { .. } => "oblique",
    }
}

const fn writing_mode_label(mode: RichTextWritingMode) -> &'static str {
    match mode {
        RichTextWritingMode::HorizontalTb => "horizontal_tb",
        RichTextWritingMode::VerticalRl => "vertical_rl",
        RichTextWritingMode::VerticalLr => "vertical_lr",
    }
}

const fn direction_label(direction: RichTextInlineDirection) -> &'static str {
    match direction {
        RichTextInlineDirection::Auto => "auto",
        RichTextInlineDirection::Ltr => "ltr",
        RichTextInlineDirection::Rtl => "rtl",
    }
}

const fn orientation_label(orientation: GlyphOrientation) -> &'static str {
    match orientation {
        GlyphOrientation::Upright => "upright",
        GlyphOrientation::SidewaysCw => "sideways_cw",
        GlyphOrientation::TextCombineUpright => "text_combine_upright",
    }
}

const fn vertical_form_label(form: GlyphVerticalForm) -> &'static str {
    match form {
        GlyphVerticalForm::None => "none",
        GlyphVerticalForm::UprightAlternate => "upright_alternate",
        GlyphVerticalForm::RotatedAlternate => "rotated_alternate",
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

fn hex_bytes(bytes: &[u8]) -> String {
    use core::fmt::Write;

    bytes.iter().fold(
        String::with_capacity(bytes.len().saturating_mul(2)),
        |mut output, byte| {
            write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
            output
        },
    )
}

fn stable_hash(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    format!("{hash:016x}")
}
