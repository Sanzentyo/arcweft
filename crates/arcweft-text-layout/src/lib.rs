//! Sans I/O rich-text layout geometry for Arcweft players and agent debugging.
//!
//! This crate owns deterministic text geometry. Renderer adapters consume the
//! resulting `LaidOutText` instead of deriving bounds from pixels or from
//! renderer-specific buffers.

use arcweft_render_text::{
    LineDisplayFrame, Milli, RichTextEffectDescriptor, RichTextEffectPhase,
    RichTextJlreqStrictness, RichTextParam, RichTextPresentation, RichTextRange,
    RichTextRubyAnnotation, RichTextRubyPosition, RichTextTextSource, RichTextVerticalLatinMode,
    RichTextWritingMode, parse_decimal_milli,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ops::Range;
use thiserror::Error;
use unicode_linebreak::{BreakOpportunity, linebreaks};
use unicode_segmentation::UnicodeSegmentation as _;

mod jlreq_punctuation;
mod jlreq_punctuation_data;
mod ruby;
mod vertical_orientation;
pub use jlreq_punctuation_data::{
    JLREQ_PAIR_ADJUSTMENT_DATA_VERSION, JLREQ_PUNCTUATION_DATA_VERSION,
};
use ruby::{horizontal_ruby_base_allocation_width, layout_ruby, ruby_text_extent};
#[cfg(test)]
use ruby::{horizontal_ruby_html_overlap, vertical_ruby_html_overlap};
pub use vertical_orientation::UNICODE_VERTICAL_ORIENTATION_VERSION;
use vertical_orientation::{UnicodeVerticalOrientation, unicode_vertical_orientation};

const DEFAULT_RUBY_GAP: f32 = 2.0;
const HORIZONTAL_RUBY_HTML_OVERLAP_EM: f32 = 0.36;
const VERTICAL_RUBY_HTML_OVERLAP_EM: f32 = 0.46;

/// Text layout failed before geometry could be produced.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TextLayoutError {
    /// A display-map range did not align with the resolved frame text.
    #[error("display range {range:?} is not valid for the resolved text")]
    InvalidRange {
        /// Invalid byte range.
        range: RichTextRange,
    },
}

/// Two-dimensional point in textbox-local pixels.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct LayoutPoint {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
}

impl LayoutPoint {
    /// Creates a point.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Two-dimensional size in textbox-local pixels.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct LayoutSize {
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl LayoutSize {
    /// Creates a size.
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// Axis-aligned rectangle in textbox-local pixels.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct LayoutRect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl LayoutRect {
    /// Creates a rectangle.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Right edge.
    pub fn right(self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge.
    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    /// Returns the union of two rectangles.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        let left = self.x.min(other.x);
        let top = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Self::new(left, top, (right - left).max(0.0), (bottom - top).max(0.0))
    }

    /// Returns whether two rectangles overlap with positive area.
    pub fn intersects(self, other: Self) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }
}

/// Static layout configuration supplied by the host textbox.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct TextLayoutConfig {
    /// Textbox-local origin.
    pub origin: LayoutPoint,
    /// Available layout size.
    pub size: LayoutSize,
    /// Base body font size.
    pub font_size: f32,
    /// Inline advance for body text.
    pub line_advance: f32,
    /// Ruby annotation font size.
    pub ruby_font_size: f32,
    /// Default writing mode when a run has no layout presentation.
    pub writing_mode: RichTextWritingMode,
    /// JLREQ punctuation pair strictness used by vertical column planning.
    pub jlreq_strictness: JlreqStrictness,
    /// Effect time used by layout-phase rich-text effects.
    pub effect_time_seconds: f32,
}

impl Default for TextLayoutConfig {
    fn default() -> Self {
        Self {
            origin: LayoutPoint::new(24.0, 24.0),
            size: LayoutSize::new(720.0, 360.0),
            font_size: 30.0,
            line_advance: 42.0,
            ruby_font_size: 14.0,
            writing_mode: RichTextWritingMode::HorizontalTb,
            jlreq_strictness: JlreqStrictness::Normal,
            effect_time_seconds: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RubyMetrics {
    font_size: f32,
    gap: f32,
    overhang: f32,
    collision_gap: f32,
}

fn ruby_metrics(annotation: &RichTextRubyAnnotation, config: TextLayoutConfig) -> RubyMetrics {
    ruby_metrics_from_presentation(&annotation.presentation, config)
}

fn ruby_metrics_from_presentation(
    presentation: &RichTextPresentation,
    config: TextLayoutConfig,
) -> RubyMetrics {
    let font_size = presentation
        .layout
        .as_ref()
        .and_then(|layout| positive_milli(layout.ruby_font_size))
        .unwrap_or(config.ruby_font_size.max(1.0));
    RubyMetrics {
        font_size,
        gap: presentation
            .layout
            .as_ref()
            .and_then(|layout| nonnegative_milli(layout.ruby_gap))
            .unwrap_or(DEFAULT_RUBY_GAP),
        overhang: presentation
            .layout
            .as_ref()
            .and_then(|layout| positive_milli(layout.ruby_overhang))
            .unwrap_or(font_size * 0.5),
        collision_gap: presentation
            .layout
            .as_ref()
            .and_then(|layout| positive_milli(layout.ruby_collision_gap))
            .unwrap_or(2.0),
    }
}

fn positive_milli(value: Option<arcweft_render_text::Milli>) -> Option<f32> {
    value
        .map(arcweft_render_text::Milli::as_f32)
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn nonnegative_milli(value: Option<arcweft_render_text::Milli>) -> Option<f32> {
    value
        .map(arcweft_render_text::Milli::as_f32)
        .filter(|value| value.is_finite() && *value >= 0.0)
}

/// Strictness preset for JLREQ punctuation pair planning.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JlreqStrictness {
    /// Prefer looser breaks, while still keeping non-separable repeat marks.
    Loose,
    /// Balanced default for narrative text.
    #[default]
    Normal,
    /// Prefer stricter Japanese composition around weak punctuation pairs.
    Strict,
}

/// Physical glyph orientation selected by layout.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlyphOrientation {
    /// Glyph is drawn upright.
    Upright,
    /// Glyph is drawn sideways by rotating clockwise in the renderer.
    SidewaysCw,
    /// A short digit cluster is compressed upright into one vertical cell.
    TextCombineUpright,
}

/// Vertical shaping form requested by UAX #50 orientation resolution.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlyphVerticalForm {
    /// No vertical alternate is requested.
    #[default]
    None,
    /// Use a vertical alternate when the font has one; fallback stays upright.
    UprightAlternate,
    /// Prefer a rotated vertical alternate when available; fallback is sideways.
    RotatedAlternate,
}

/// One source-cluster placement in layout order.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LaidOutGlyph {
    /// Source run index in `LineDisplayFrame::display_map.text_runs`.
    pub run_index: usize,
    /// Byte range in `LineDisplayFrame::text`.
    pub range: RichTextRange,
    /// Source text for the cluster.
    pub text: String,
    /// Origin of the glyph cluster.
    pub origin: LayoutPoint,
    /// Logical advance after the cluster.
    pub advance: LayoutSize,
    /// Ink bounds before renderer effects.
    pub bounds: LayoutRect,
    /// Writing mode that produced this placement.
    pub writing_mode: RichTextWritingMode,
    /// Physical glyph orientation.
    pub orientation: GlyphOrientation,
    /// Vertical shaping form requested before renderer fallback transforms.
    pub vertical_form: GlyphVerticalForm,
    /// Resolved presentation metadata for the source run.
    pub presentation: RichTextPresentation,
}

/// Bounds for one source text run.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LaidOutRun {
    /// Source run index.
    pub run_index: usize,
    /// Byte range in `LineDisplayFrame::text`.
    pub range: RichTextRange,
    /// Union of cluster bounds.
    pub bounds: LayoutRect,
    /// Writing mode used by the run.
    pub writing_mode: RichTextWritingMode,
    /// Resolved presentation metadata for the source run.
    pub presentation: RichTextPresentation,
}

/// Ruby placement tied to a base range.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LaidOutRuby {
    /// Source ruby annotation index.
    pub ruby_index: usize,
    /// Base text range.
    pub base_range: RichTextRange,
    /// Ruby annotation text.
    pub ruby: String,
    /// Base bounds used for placement.
    pub base_bounds: LayoutRect,
    /// Ruby annotation bounds.
    pub ruby_bounds: LayoutRect,
    /// Writing mode of the base text that produced this placement.
    pub writing_mode: RichTextWritingMode,
    /// Presentation metadata on the ruby annotation.
    pub presentation: RichTextPresentation,
}

/// Complete Sans I/O layout result.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct LaidOutText {
    /// Laid out glyph clusters in visual order.
    pub glyphs: Vec<LaidOutGlyph>,
    /// Source run bounds.
    pub runs: Vec<LaidOutRun>,
    /// Ruby annotation bounds.
    pub ruby: Vec<LaidOutRuby>,
    /// Union of all laid out content.
    pub bounds: Option<LayoutRect>,
}

/// Lays out one resolved rich-text frame into renderer-independent geometry.
pub fn layout_frame(
    frame: &LineDisplayFrame,
    config: TextLayoutConfig,
) -> Result<LaidOutText, TextLayoutError> {
    let mut out = LaidOutText::default();
    let mut state = TextLayoutState::new(config, vertical_ruby_track_reservation(frame, config));
    for (run_index, run) in frame.display_map.text_runs.iter().enumerate() {
        let range = valid_range(frame, run.range)?;
        let text = frame
            .text
            .get(range.clone())
            .ok_or(TextLayoutError::InvalidRange { range: run.range })?;
        let writing_mode = run
            .presentation
            .layout
            .as_ref()
            .map_or(config.writing_mode, |layout| layout.writing_mode);
        let vertical_latin = run
            .presentation
            .layout
            .as_ref()
            .map_or(RichTextVerticalLatinMode::Mixed, |layout| {
                layout.vertical_latin
            });
        let run_config = text_layout_config_for_presentation(config, &run.presentation);
        let glyph_start = out.glyphs.len();
        match writing_mode {
            RichTextWritingMode::HorizontalTb => {
                let context = HorizontalRunLayoutContext {
                    run_index,
                    range_start: range.start,
                    presentation: &run.presentation,
                    ruby_annotations: &frame.display_map.ruby_annotations,
                    config: run_config,
                };
                layout_horizontal_run(&mut out.glyphs, text, context, &mut state);
            }
            RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr => {
                let context = RunLayoutContext {
                    run_index,
                    range_start: range.start,
                    source: run.source,
                    presentation: &run.presentation,
                    ruby_annotations: &frame.display_map.ruby_annotations,
                    config: run_config,
                };
                layout_vertical_run(
                    &mut out.glyphs,
                    text,
                    writing_mode,
                    vertical_latin,
                    context,
                    &mut state,
                );
            }
        }
        if let Some(bounds) =
            union_bounds(out.glyphs[glyph_start..].iter().map(|glyph| glyph.bounds))
        {
            out.runs.push(LaidOutRun {
                run_index,
                range: run.range,
                bounds,
                writing_mode,
                presentation: run.presentation.clone(),
            });
            out.bounds = Some(out.bounds.map_or(bounds, |existing| existing.union(bounds)));
        }
    }
    out.ruby = layout_ruby(frame, &out.glyphs, config);
    for ruby in &out.ruby {
        out.bounds = Some(
            out.bounds
                .map_or(ruby.ruby_bounds, |bounds| bounds.union(ruby.ruby_bounds)),
        );
    }
    Ok(out)
}

fn text_layout_config_for_presentation(
    config: TextLayoutConfig,
    presentation: &RichTextPresentation,
) -> TextLayoutConfig {
    let Some(layout) = &presentation.layout else {
        return config;
    };
    let jlreq_strictness = match layout.jlreq_strictness {
        RichTextJlreqStrictness::Auto => config.jlreq_strictness,
        RichTextJlreqStrictness::Loose => JlreqStrictness::Loose,
        RichTextJlreqStrictness::Normal => JlreqStrictness::Normal,
        RichTextJlreqStrictness::Strict => JlreqStrictness::Strict,
    };
    TextLayoutConfig {
        jlreq_strictness,
        ..config
    }
}

#[derive(Clone, Debug)]
struct TextLayoutState {
    horizontal: LayoutCursor,
    vertical_rl: LayoutCursor,
    vertical_lr: LayoutCursor,
    vertical_rl_previous_cluster: Option<String>,
    vertical_lr_previous_cluster: Option<String>,
}

impl TextLayoutState {
    fn new(config: TextLayoutConfig, ruby_track: VerticalRubyTrackReservation) -> Self {
        Self {
            horizontal: LayoutCursor::new(config.origin.x, config.origin.y),
            vertical_rl: LayoutCursor::new(
                vertical_column_start(RichTextWritingMode::VerticalRl, config)
                    - ruby_track.vertical_rl,
                config.origin.y,
            ),
            vertical_lr: LayoutCursor::new(
                vertical_column_start(RichTextWritingMode::VerticalLr, config)
                    + ruby_track.vertical_lr,
                config.origin.y,
            ),
            vertical_rl_previous_cluster: None,
            vertical_lr_previous_cluster: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct VerticalRubyTrackReservation {
    vertical_rl: f32,
    vertical_lr: f32,
}

fn vertical_ruby_track_reservation(
    frame: &LineDisplayFrame,
    config: TextLayoutConfig,
) -> VerticalRubyTrackReservation {
    frame.display_map.ruby_annotations.iter().fold(
        VerticalRubyTrackReservation::default(),
        |mut reservation, annotation| {
            if matches!(
                ruby_position(annotation),
                RichTextRubyPosition::InterCharacter
            ) {
                return reservation;
            }
            let track_width = vertical_ruby_track_reservation_width(annotation, config);
            match ruby_annotation_writing_mode(frame, annotation, config) {
                RichTextWritingMode::VerticalRl => {
                    reservation.vertical_rl = reservation.vertical_rl.max(track_width);
                }
                RichTextWritingMode::VerticalLr => {
                    reservation.vertical_lr = reservation.vertical_lr.max(track_width);
                }
                RichTextWritingMode::HorizontalTb => {}
            }
            reservation
        },
    )
}

fn ruby_annotation_writing_mode(
    frame: &LineDisplayFrame,
    annotation: &RichTextRubyAnnotation,
    config: TextLayoutConfig,
) -> RichTextWritingMode {
    frame
        .display_map
        .text_runs
        .iter()
        .find(|run| ranges_overlap(run.range, annotation.base_range))
        .and_then(|run| run.presentation.layout.as_ref())
        .map_or(config.writing_mode, |layout| layout.writing_mode)
}

fn vertical_ruby_track_reservation_width(
    annotation: &RichTextRubyAnnotation,
    config: TextLayoutConfig,
) -> f32 {
    let metrics = ruby_metrics(annotation, config);
    let segment_count =
        vertical_ruby_segment_count(annotation.ruby.chars().count(), config, metrics).max(1);
    metrics.gap
        + metrics.font_size
        + usize_to_f32(segment_count.saturating_sub(1))
            * vertical_ruby_continuation_track_step(metrics)
}

fn vertical_ruby_segment_count(
    char_count: usize,
    config: TextLayoutConfig,
    metrics: RubyMetrics,
) -> usize {
    let max_chars = max_ruby_chars_per_vertical_segment(config, metrics).max(1);
    char_count.max(1).div_ceil(max_chars)
}

fn vertical_ruby_continuation_track_step(metrics: RubyMetrics) -> f32 {
    metrics.font_size + metrics.collision_gap
}

#[derive(Clone, Copy, Debug)]
struct LayoutCursor {
    x: f32,
    y: f32,
}

impl LayoutCursor {
    const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct LayoutEffectReserve {
    x: f32,
    y: f32,
}

impl LayoutEffectReserve {
    fn union(self, other: Self) -> Self {
        Self {
            x: self.x.max(other.x),
            y: self.y.max(other.y),
        }
    }
}

fn layout_phase_effect_reserve(presentation: &RichTextPresentation) -> LayoutEffectReserve {
    presentation
        .effects
        .iter()
        .filter(|effect| {
            matches!(
                effect.phase,
                RichTextEffectPhase::BeforeLayout | RichTextEffectPhase::LayoutTransform
            )
        })
        .map(layout_builtin_effect_reserve)
        .fold(LayoutEffectReserve::default(), LayoutEffectReserve::union)
}

fn layout_builtin_effect_reserve(effect: &RichTextEffectDescriptor) -> LayoutEffectReserve {
    match effect.id.as_str() {
        "wave" => {
            let amplitude = effect_param_milli(effect, "amp")
                .unwrap_or(Milli(4000))
                .as_f32()
                .abs();
            let direction = effect_param_vec2(effect, "dir")
                .or_else(|| effect_axis_direction(effect))
                .unwrap_or([0.0, 1.0]);
            LayoutEffectReserve {
                x: amplitude * direction[0].abs(),
                y: amplitude * direction[1].abs(),
            }
        }
        "shake" | "jitter" => {
            let amplitude = effect_param_milli(effect, "amp")
                .unwrap_or(Milli(2000))
                .as_f32()
                .abs();
            LayoutEffectReserve {
                x: amplitude,
                y: amplitude,
            }
        }
        "arc" => {
            let radius = effect_param_milli(effect, "radius")
                .unwrap_or(Milli(120_000))
                .as_f32()
                .abs();
            LayoutEffectReserve {
                x: radius,
                y: radius,
            }
        }
        _ => LayoutEffectReserve::default(),
    }
}

fn effect_param_milli(effect: &RichTextEffectDescriptor, name: &str) -> Option<Milli> {
    effect_param_as_milli(effect.params.get(name)?)
}

fn effect_param_vec2(effect: &RichTextEffectDescriptor, name: &str) -> Option<[f32; 2]> {
    effect_param_as_vec2(effect.params.get(name)?)
}

fn effect_param_as_milli(param: &RichTextParam) -> Option<Milli> {
    match param {
        RichTextParam::Milli { value } => Some(*value),
        RichTextParam::Int { value } => {
            Some(Milli(i32::try_from(*value).ok()?.saturating_mul(1000)))
        }
        RichTextParam::Raw { value } | RichTextParam::Text { value } => {
            parse_raw_effect_milli(value)
        }
        _ => None,
    }
}

fn effect_param_as_vec2(param: &RichTextParam) -> Option<[f32; 2]> {
    match param {
        RichTextParam::Vec2 { value } => Some([value.x.as_f32(), value.y.as_f32()]),
        RichTextParam::Raw { value } | RichTextParam::Text { value } => {
            parse_raw_effect_vec2(value)
        }
        _ => None,
    }
}

fn parse_raw_effect_milli(value: &str) -> Option<Milli> {
    let trimmed = value.trim();
    let numeric = trimmed
        .strip_suffix("px")
        .or_else(|| trimmed.strip_suffix("deg"))
        .or_else(|| trimmed.strip_suffix("ch"))
        .unwrap_or(trimmed)
        .trim();
    parse_decimal_milli(numeric)
}

fn parse_raw_effect_vec2(value: &str) -> Option<[f32; 2]> {
    let (x, y) = value.split_once(',')?;
    Some([
        parse_raw_effect_milli(x)?.as_f32(),
        parse_raw_effect_milli(y)?.as_f32(),
    ])
}

fn effect_axis_direction(effect: &RichTextEffectDescriptor) -> Option<[f32; 2]> {
    match effect.params.get("axis")? {
        RichTextParam::Raw { value }
        | RichTextParam::Text { value }
        | RichTextParam::Selector { value } => match value.as_str() {
            "x" | ".x" => Some([1.0, 0.0]),
            "y" | ".y" => Some([0.0, 1.0]),
            _ => None,
        },
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct HorizontalRunLayoutContext<'a> {
    run_index: usize,
    range_start: usize,
    presentation: &'a RichTextPresentation,
    ruby_annotations: &'a [RichTextRubyAnnotation],
    config: TextLayoutConfig,
}

fn layout_horizontal_run(
    glyphs: &mut Vec<LaidOutGlyph>,
    text: &str,
    context: HorizontalRunLayoutContext<'_>,
    state: &mut TextLayoutState,
) {
    let cursor = &mut state.horizontal;
    let char_indices = text.char_indices().collect::<Vec<_>>();
    let mut char_index = 0usize;
    while let Some((offset, ch)) = char_indices.get(char_index).copied() {
        if ch == '\n' {
            cursor.x = context.config.origin.x;
            cursor.y += context.config.line_advance;
            char_index += 1;
            continue;
        }
        let absolute_start = context.range_start + offset;
        if let Some(annotation) = horizontal_ruby_annotation_starting_at(
            context.ruby_annotations,
            context.range_start,
            text,
            absolute_start,
        ) {
            let base_end = annotation.base_range.end;
            let next_index = layout_horizontal_ruby_base(glyphs, text, annotation, context, cursor);
            char_index = char_indices
                .iter()
                .position(|(candidate_offset, _)| {
                    context.range_start + *candidate_offset >= base_end
                })
                .unwrap_or(next_index);
            continue;
        }
        let reserve = layout_phase_effect_reserve(context.presentation);
        let width = horizontal_advance(ch, context.config.font_size);
        let allocation_width = horizontal_layout_advance(width, reserve);
        if horizontal_cluster_should_wrap(cursor.x, allocation_width, context.config) {
            cursor.x = context.config.origin.x;
            cursor.y += context.config.line_advance;
        }
        let start = context.range_start + offset;
        let end = start + ch.len_utf8();
        let origin_x = cursor.x + reserve.x;
        let bounds = horizontal_glyph_bounds_with_reserve(
            origin_x,
            cursor.y,
            width,
            context.config,
            reserve,
        );
        glyphs.push(LaidOutGlyph {
            run_index: context.run_index,
            range: RichTextRange::new(start, end),
            text: ch.to_string(),
            origin: LayoutPoint::new(origin_x, cursor.y),
            advance: LayoutSize::new(allocation_width, 0.0),
            bounds,
            writing_mode: RichTextWritingMode::HorizontalTb,
            orientation: GlyphOrientation::Upright,
            vertical_form: GlyphVerticalForm::None,
            presentation: context.presentation.clone(),
        });
        cursor.x += allocation_width;
        char_index += 1;
    }
}

fn horizontal_ruby_annotation_starting_at<'a>(
    annotations: &'a [RichTextRubyAnnotation],
    range_start: usize,
    text: &str,
    absolute_start: usize,
) -> Option<&'a RichTextRubyAnnotation> {
    let range_end = range_start + text.len();
    annotations.iter().find(|annotation| {
        annotation.base_range.start == absolute_start
            && annotation.base_range.start < annotation.base_range.end
            && annotation.base_range.end <= range_end
            && text
                .get(
                    (annotation.base_range.start - range_start)
                        ..(annotation.base_range.end - range_start),
                )
                .is_some_and(|base| !base.contains('\n'))
    })
}

fn layout_horizontal_ruby_base(
    glyphs: &mut Vec<LaidOutGlyph>,
    text: &str,
    annotation: &RichTextRubyAnnotation,
    context: HorizontalRunLayoutContext<'_>,
    cursor: &mut LayoutCursor,
) -> usize {
    let base_start = annotation.base_range.start - context.range_start;
    let base_end = annotation.base_range.end - context.range_start;
    let Some(base_text) = text.get(base_start..base_end) else {
        return 0;
    };
    let reserve = layout_phase_effect_reserve(context.presentation);
    let base_width = horizontal_text_layout_advance(base_text, context.config.font_size, reserve);
    let metrics = ruby_metrics(annotation, context.config);
    let allocation_width = horizontal_ruby_base_allocation_width(
        base_width,
        ruby_text_extent(&annotation.ruby, metrics.font_size),
        context.config,
    );
    if horizontal_cluster_should_wrap(cursor.x, allocation_width, context.config) {
        cursor.x = context.config.origin.x;
        cursor.y += context.config.line_advance;
    }
    let mut glyph_x = cursor.x + (allocation_width - base_width).max(0.0) * 0.5;
    for (offset, ch) in base_text.char_indices() {
        let width = horizontal_advance(ch, context.config.font_size);
        let allocation_width = horizontal_layout_advance(width, reserve);
        let start = context.range_start + base_start + offset;
        let end = start + ch.len_utf8();
        let origin_x = glyph_x + reserve.x;
        glyphs.push(LaidOutGlyph {
            run_index: context.run_index,
            range: RichTextRange::new(start, end),
            text: ch.to_string(),
            origin: LayoutPoint::new(origin_x, cursor.y),
            advance: LayoutSize::new(allocation_width, 0.0),
            bounds: horizontal_glyph_bounds_with_reserve(
                origin_x,
                cursor.y,
                width,
                context.config,
                reserve,
            ),
            writing_mode: RichTextWritingMode::HorizontalTb,
            orientation: GlyphOrientation::Upright,
            vertical_form: GlyphVerticalForm::None,
            presentation: context.presentation.clone(),
        });
        glyph_x += allocation_width;
    }
    cursor.x += allocation_width;
    text[..base_end].chars().count()
}

fn horizontal_glyph_bounds(
    x: f32,
    line_y: f32,
    width: f32,
    config: TextLayoutConfig,
) -> LayoutRect {
    let height = config.font_size.max(1.0).min(config.line_advance.max(1.0));
    let y = line_y + (config.line_advance - height).max(0.0) * 0.5;
    LayoutRect::new(x, y, width.max(1.0), height)
}

fn horizontal_glyph_bounds_with_reserve(
    x: f32,
    line_y: f32,
    width: f32,
    config: TextLayoutConfig,
    reserve: LayoutEffectReserve,
) -> LayoutRect {
    let mut bounds = horizontal_glyph_bounds(x, line_y, width, config);
    bounds.x -= reserve.x;
    bounds.y -= reserve.y;
    bounds.width += reserve.x * 2.0;
    bounds.height += reserve.y * 2.0;
    bounds
}

fn vertical_glyph_bounds(
    column_x: f32,
    glyph_y: f32,
    cluster: &VerticalCluster,
    config: TextLayoutConfig,
) -> LayoutRect {
    let width = config.font_size.max(1.0).min(config.line_advance.max(1.0));
    let height = if cluster_is_sideways_latin_run(cluster) {
        vertical_cluster_advance(cluster, config)
    } else {
        config.font_size.max(1.0)
    };
    LayoutRect::new(column_x, glyph_y, width, height)
}

fn vertical_glyph_bounds_with_reserve(
    column_x: f32,
    glyph_y: f32,
    cluster: &VerticalCluster,
    config: TextLayoutConfig,
    reserve: LayoutEffectReserve,
) -> LayoutRect {
    let mut bounds = vertical_glyph_bounds(column_x, glyph_y, cluster, config);
    bounds.x -= reserve.x;
    bounds.y -= reserve.y;
    bounds.width += reserve.x * 2.0;
    bounds.height += reserve.y * 2.0;
    bounds
}

fn horizontal_layout_advance(width: f32, reserve: LayoutEffectReserve) -> f32 {
    width + reserve.x * 2.0
}

fn horizontal_text_layout_advance(text: &str, font_size: f32, reserve: LayoutEffectReserve) -> f32 {
    text.chars()
        .filter(|ch| *ch != '\n')
        .map(|ch| horizontal_layout_advance(horizontal_advance(ch, font_size), reserve))
        .sum()
}

fn horizontal_cluster_should_wrap(cursor_x: f32, width: f32, config: TextLayoutConfig) -> bool {
    let line_start = config.origin.x;
    let line_end = config.origin.x + config.size.width.max(1.0);
    cursor_x > line_start + f32::EPSILON && cursor_x + width > line_end + f32::EPSILON
}

fn layout_vertical_run(
    glyphs: &mut Vec<LaidOutGlyph>,
    text: &str,
    writing_mode: RichTextWritingMode,
    vertical_latin: RichTextVerticalLatinMode,
    context: RunLayoutContext<'_>,
    state: &mut TextLayoutState,
) {
    let config = context.config;
    let column_step = vertical_column_step(writing_mode, context.presentation, config);
    let previous_cluster = match writing_mode {
        RichTextWritingMode::VerticalRl => state.vertical_rl_previous_cluster.clone(),
        RichTextWritingMode::VerticalLr => state.vertical_lr_previous_cluster.clone(),
        RichTextWritingMode::HorizontalTb => {
            unreachable!("horizontal runs use layout_horizontal_run")
        }
    };
    let cursor = match writing_mode {
        RichTextWritingMode::VerticalRl => &mut state.vertical_rl,
        RichTextWritingMode::VerticalLr => &mut state.vertical_lr,
        RichTextWritingMode::HorizontalTb => {
            unreachable!("horizontal runs use layout_horizontal_run")
        }
    };
    let clusters = vertical_clusters(text, vertical_latin);
    let column_plan =
        plan_vertical_columns(&clusters, context, *cursor, previous_cluster.as_deref());
    let mut next_previous_cluster = previous_cluster;
    let mut cluster_index = 0usize;
    while cluster_index < clusters.len() {
        let cluster = &clusters[cluster_index];
        if is_vertical_line_break_cluster(&cluster.text) {
            cursor.x += column_step;
            cursor.y = config.origin.y;
            next_previous_cluster = None;
            cluster_index += 1;
            continue;
        }
        let start = context.range_start + cluster.range.start;
        let end = context.range_start + cluster.range.end;
        let range = RichTextRange::new(start, end);
        if column_plan.breaks_before(cluster_index) {
            cursor.x += column_step;
            cursor.y = config.origin.y;
        }
        if let Some((next_cluster_index, previous_cluster)) = layout_vertical_side_ruby_base(
            glyphs,
            &clusters,
            cluster_index,
            cursor,
            writing_mode,
            context,
        ) {
            next_previous_cluster = previous_cluster;
            cluster_index = next_cluster_index;
            continue;
        }
        let reserve = layout_phase_effect_reserve(context.presentation);
        let advance = vertical_cluster_layout_advance(cluster, config, reserve);
        let glyph_y =
            vertical_cluster_origin_y(&cluster.text, cursor.y + reserve.y, advance, config);
        push_vertical_glyph(
            glyphs,
            cluster,
            range,
            cursor.x,
            glyph_y,
            writing_mode,
            context,
        );
        cursor.y += advance;
        cursor.y += vertical_inter_character_ruby_extent_after(range, context);
        next_previous_cluster = Some(cluster.text.clone());
        cluster_index += 1;
    }
    match writing_mode {
        RichTextWritingMode::VerticalRl => {
            state.vertical_rl_previous_cluster = next_previous_cluster;
        }
        RichTextWritingMode::VerticalLr => {
            state.vertical_lr_previous_cluster = next_previous_cluster;
        }
        RichTextWritingMode::HorizontalTb => {
            unreachable!("horizontal runs use layout_horizontal_run")
        }
    }
}

fn layout_vertical_side_ruby_base(
    glyphs: &mut Vec<LaidOutGlyph>,
    clusters: &[VerticalCluster],
    cluster_index: usize,
    cursor: &mut LayoutCursor,
    writing_mode: RichTextWritingMode,
    context: RunLayoutContext<'_>,
) -> Option<(usize, Option<String>)> {
    let annotation = vertical_side_ruby_annotation_starting_at(
        context.ruby_annotations,
        context.range_start,
        clusters,
        cluster_index,
    )?;
    let base_span =
        vertical_ruby_base_cluster_span(annotation, context.range_start, clusters, cluster_index);
    let reserve = layout_phase_effect_reserve(context.presentation);
    let base_extent =
        vertical_cluster_span_layout_advance(&clusters[base_span.clone()], context.config, reserve);
    let metrics = ruby_metrics(annotation, context.config);
    let allocation_extent = vertical_ruby_base_allocation_height(
        base_extent,
        ruby_text_extent(&annotation.ruby, metrics.font_size),
        context.config,
    );
    let mut previous_cluster = None;
    let mut glyph_y = cursor.y;
    for base_cluster in &clusters[base_span.clone()] {
        let base_range = RichTextRange::new(
            context.range_start + base_cluster.range.start,
            context.range_start + base_cluster.range.end,
        );
        push_vertical_glyph(
            glyphs,
            base_cluster,
            base_range,
            cursor.x,
            glyph_y + reserve.y,
            writing_mode,
            context,
        );
        glyph_y += vertical_cluster_layout_advance(base_cluster, context.config, reserve);
        previous_cluster = Some(base_cluster.text.clone());
    }
    cursor.y += allocation_extent;
    Some((base_span.end, previous_cluster))
}

fn push_vertical_glyph(
    glyphs: &mut Vec<LaidOutGlyph>,
    cluster: &VerticalCluster,
    range: RichTextRange,
    column_x: f32,
    glyph_y: f32,
    writing_mode: RichTextWritingMode,
    context: RunLayoutContext<'_>,
) {
    let reserve = layout_phase_effect_reserve(context.presentation);
    let advance = vertical_cluster_layout_advance(cluster, context.config, reserve);
    glyphs.push(LaidOutGlyph {
        run_index: context.run_index,
        range,
        text: cluster.text.clone(),
        origin: LayoutPoint::new(column_x, glyph_y),
        advance: LayoutSize::new(0.0, advance),
        bounds: vertical_glyph_bounds_with_reserve(
            column_x,
            glyph_y,
            cluster,
            context.config,
            reserve,
        ),
        writing_mode,
        orientation: cluster.orientation,
        vertical_form: cluster.vertical_form,
        presentation: context.presentation.clone(),
    });
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VerticalColumnPlan {
    break_before: Vec<bool>,
}

impl VerticalColumnPlan {
    fn new(cluster_count: usize) -> Self {
        Self {
            break_before: vec![false; cluster_count],
        }
    }

    fn set_break_before(&mut self, cluster_index: usize) {
        if let Some(value) = self.break_before.get_mut(cluster_index) {
            *value = true;
        }
    }

    fn breaks_before(&self, cluster_index: usize) -> bool {
        self.break_before
            .get(cluster_index)
            .copied()
            .unwrap_or_default()
    }
}

fn plan_vertical_columns(
    clusters: &[VerticalCluster],
    context: RunLayoutContext<'_>,
    initial_cursor: LayoutCursor,
    previous_cluster_text: Option<&str>,
) -> VerticalColumnPlan {
    let mut plan = VerticalColumnPlan::new(clusters.len());
    let mut segment_start = 0;
    let mut segment_initial_y = initial_cursor.y;
    for (cluster_index, cluster) in clusters.iter().enumerate() {
        if is_vertical_line_break_cluster(&cluster.text) {
            plan_vertical_column_segment(
                &mut plan,
                clusters,
                segment_start,
                cluster_index,
                context,
                segment_initial_y,
                previous_cluster_text,
            );
            segment_start = cluster_index + 1;
            segment_initial_y = context.config.origin.y;
        }
    }
    plan_vertical_column_segment(
        &mut plan,
        clusters,
        segment_start,
        clusters.len(),
        context,
        segment_initial_y,
        previous_cluster_text,
    );
    plan
}

#[derive(Clone, Copy, Debug)]
struct VerticalColumnDpState {
    cost: f32,
    previous_break: usize,
}

fn plan_vertical_column_segment(
    plan: &mut VerticalColumnPlan,
    clusters: &[VerticalCluster],
    segment_start: usize,
    segment_end: usize,
    context: RunLayoutContext<'_>,
    initial_cursor_y: f32,
    previous_cluster_text: Option<&str>,
) {
    if segment_start >= segment_end {
        return;
    }

    let continued = solve_vertical_column_segment(
        clusters,
        segment_start,
        segment_end,
        context,
        initial_cursor_y,
    );
    let restarted = (vertical_run_can_restart_at_boundary(
        context.source,
        clusters,
        segment_start,
        context.config.jlreq_strictness,
        previous_cluster_text,
    ) && initial_cursor_y > context.config.origin.y + f32::EPSILON)
        .then(|| {
            solve_vertical_column_segment(
                clusters,
                segment_start,
                segment_end,
                context,
                context.config.origin.y,
            )
            .map(|mut candidate| {
                candidate.cost += 25.0;
                candidate.break_offsets.push(0);
                candidate
            })
        })
        .flatten();
    let candidate = match (continued, restarted) {
        (Some(continued), Some(restarted)) if restarted.cost < continued.cost => restarted,
        (Some(continued), _) => continued,
        (None, Some(restarted)) => restarted,
        (None, None) => return,
    };
    for offset in candidate.break_offsets {
        plan.set_break_before(segment_start + offset);
    }
}

#[derive(Clone, Debug, PartialEq)]
struct VerticalColumnSegmentPlan {
    cost: f32,
    break_offsets: Vec<usize>,
}

fn solve_vertical_column_segment(
    clusters: &[VerticalCluster],
    segment_start: usize,
    segment_end: usize,
    context: RunLayoutContext<'_>,
    initial_cursor_y: f32,
) -> Option<VerticalColumnSegmentPlan> {
    let segment_len = segment_end - segment_start;
    let mut states = vec![None; segment_len + 1];
    states[0] = Some(VerticalColumnDpState {
        cost: 0.0,
        previous_break: 0,
    });

    for relative_start in 0..segment_len {
        let Some(start_state) = states[relative_start] else {
            continue;
        };
        let column_start_y = if relative_start == 0 {
            initial_cursor_y
        } else {
            context.config.origin.y
        };
        let mut relative_end = relative_start + 1;
        while relative_end <= segment_len {
            let absolute_start = segment_start + relative_start;
            let absolute_end = segment_start + relative_end;
            if let Some(column_cost) = vertical_column_segment_cost(
                clusters,
                absolute_start,
                absolute_end,
                segment_end,
                context,
                column_start_y,
            ) {
                let cost = start_state.cost + column_cost;
                if vertical_column_dp_candidate_is_better(
                    states[relative_end],
                    cost,
                    relative_start,
                ) {
                    states[relative_end] = Some(VerticalColumnDpState {
                        cost,
                        previous_break: relative_start,
                    });
                }
            }
            relative_end += 1;
        }
    }

    let mut cursor = segment_len;
    let mut break_offsets = Vec::new();
    while cursor > 0 {
        let state = states[cursor]?;
        if state.previous_break > 0 {
            break_offsets.push(state.previous_break);
        }
        cursor = state.previous_break;
    }
    break_offsets.reverse();
    Some(VerticalColumnSegmentPlan {
        cost: states[segment_len]?.cost,
        break_offsets,
    })
}

fn vertical_column_dp_candidate_is_better(
    current: Option<VerticalColumnDpState>,
    cost: f32,
    previous_break: usize,
) -> bool {
    let Some(current) = current else {
        return true;
    };
    cost < current.cost
        || ((cost - current.cost).abs() <= f32::EPSILON && previous_break > current.previous_break)
}

fn vertical_column_segment_cost(
    clusters: &[VerticalCluster],
    column_start: usize,
    column_end: usize,
    segment_end: usize,
    context: RunLayoutContext<'_>,
    column_start_y: f32,
) -> Option<f32> {
    if column_start >= column_end {
        return None;
    }
    if column_end < segment_end
        && !vertical_cluster_can_start_column(column_end, clusters, context.config.jlreq_strictness)
    {
        return None;
    }
    if column_end < segment_end
        && vertical_column_ends_with_line_end_prohibited(column_start, column_end, clusters)
    {
        return None;
    }
    if vertical_column_splits_side_ruby_base(clusters, column_start, column_end, context) {
        return None;
    }

    let capacity = context.config.origin.y + context.config.size.height - column_start_y;
    let used = vertical_column_segment_required_extent(clusters, column_start, column_end, context);
    let overflow = (used - capacity).max(0.0);
    let allowed_overhang = vertical_column_segment_overhang_allowance(
        clusters,
        column_start,
        column_end,
        context.config,
    );
    let overhang_uses_linebreak_continuation =
        vertical_column_segment_overhang_uses_linebreak_continuation(
            clusters,
            column_start,
            column_end,
            context.config,
        );
    if column_end < segment_end && overflow > allowed_overhang + f32::EPSILON {
        return None;
    }
    if column_end == segment_end
        && overflow > allowed_overhang + f32::EPSILON
        && vertical_column_segment_has_valid_overflow_avoiding_break(
            clusters,
            column_start,
            column_end,
            context,
            column_start_y,
        )
    {
        return None;
    }

    let remaining = (capacity - used).max(0.0);
    let capacity = capacity.max(context.config.font_size);
    let badness = 100.0 * (remaining / capacity).powi(3);
    let overflow_penalty =
        ((overflow - allowed_overhang).max(0.0) / context.config.font_size).powi(2) * 10_000.0;
    let allowed_overhang_penalty = if overhang_uses_linebreak_continuation {
        (overflow.min(allowed_overhang) / context.config.font_size).powi(2) * 50.0
    } else {
        0.0
    };
    let break_penalty = if column_end < segment_end {
        5.0 + vertical_column_pair_break_penalty(
            clusters,
            column_start,
            column_end,
            context.config.jlreq_strictness,
        )
    } else {
        0.0
    };
    Some(badness + overflow_penalty + allowed_overhang_penalty + break_penalty)
}

fn vertical_column_segment_has_valid_overflow_avoiding_break(
    clusters: &[VerticalCluster],
    column_start: usize,
    column_end: usize,
    context: RunLayoutContext<'_>,
    column_start_y: f32,
) -> bool {
    let capacity = context.config.origin.y + context.config.size.height - column_start_y;
    (column_start + 1..column_end).any(|break_index| {
        if !vertical_cluster_can_start_column(
            break_index,
            clusters,
            context.config.jlreq_strictness,
        ) || vertical_column_ends_with_line_end_prohibited(column_start, break_index, clusters)
            || vertical_column_splits_side_ruby_base(clusters, column_start, break_index, context)
        {
            return false;
        }
        let used =
            vertical_column_segment_required_extent(clusters, column_start, break_index, context);
        let allowed_overhang = vertical_column_segment_overhang_allowance(
            clusters,
            column_start,
            break_index,
            context.config,
        );
        (used - capacity).max(0.0) <= allowed_overhang + f32::EPSILON
    })
}

fn vertical_column_splits_side_ruby_base(
    clusters: &[VerticalCluster],
    column_start: usize,
    column_end: usize,
    context: RunLayoutContext<'_>,
) -> bool {
    (0..clusters.len()).any(|cluster_index| {
        let Some(annotation) = vertical_side_ruby_annotation_starting_at(
            context.ruby_annotations,
            context.range_start,
            clusters,
            cluster_index,
        ) else {
            return false;
        };
        let span = vertical_ruby_base_cluster_span(
            annotation,
            context.range_start,
            clusters,
            cluster_index,
        );
        span.start < column_end
            && column_start < span.end
            && (column_start > span.start || column_end < span.end)
    })
}

fn vertical_column_pair_break_penalty(
    clusters: &[VerticalCluster],
    column_start: usize,
    column_end: usize,
    strictness: JlreqStrictness,
) -> f32 {
    let Some(left) = clusters[column_start..column_end]
        .iter()
        .rev()
        .find(|cluster| !is_vertical_line_break_cluster(&cluster.text))
    else {
        return 0.0;
    };
    let Some(right) = clusters[column_end..]
        .iter()
        .find(|cluster| !is_vertical_line_break_cluster(&cluster.text))
    else {
        return 0.0;
    };
    f32::from(
        jlreq_punctuation::pair_adjustment_for_clusters(&left.text, &right.text, strictness)
            .break_penalty,
    )
}

fn vertical_column_segment_required_extent(
    clusters: &[VerticalCluster],
    column_start: usize,
    column_end: usize,
    context: RunLayoutContext<'_>,
) -> f32 {
    let mut cursor = 0.0f32;
    let mut required = 0.0f32;
    let mut cluster_index = column_start;
    let reserve = layout_phase_effect_reserve(context.presentation);
    while cluster_index < column_end {
        let cluster = &clusters[cluster_index];
        let range = RichTextRange::new(
            context.range_start + cluster.range.start,
            context.range_start + cluster.range.end,
        );
        if let Some(annotation) = vertical_side_ruby_annotation_starting_at(
            context.ruby_annotations,
            context.range_start,
            clusters,
            cluster_index,
        ) {
            let span = vertical_ruby_base_cluster_span(
                annotation,
                context.range_start,
                clusters,
                cluster_index,
            );
            let allocation_extent = vertical_ruby_base_allocation_height(
                vertical_cluster_span_layout_advance(
                    &clusters[span.clone()],
                    context.config,
                    reserve,
                ),
                ruby_text_extent(
                    &annotation.ruby,
                    ruby_metrics(annotation, context.config).font_size,
                ),
                context.config,
            );
            required = required.max(cursor + allocation_extent);
            cursor += allocation_extent;
            cluster_index = span.end.min(column_end);
            continue;
        }
        let required_inline_extent = vertical_cluster_required_inline_extent(
            range,
            context.range_start,
            clusters,
            context.ruby_annotations,
            context.config,
        );
        required = required.max(cursor + required_inline_extent);
        cursor += vertical_cluster_layout_advance(cluster, context.config, reserve);
        cursor += vertical_inter_character_ruby_extent_after(range, context);
        cluster_index += 1;
    }
    required.max(cursor)
}

fn vertical_column_segment_overhang_allowance(
    clusters: &[VerticalCluster],
    column_start: usize,
    column_end: usize,
    config: TextLayoutConfig,
) -> f32 {
    let Some(last_cluster_index) = (column_start..column_end)
        .rfind(|index| !is_vertical_line_break_cluster(&clusters[*index].text))
    else {
        return 0.0;
    };

    let mut suffix_start = last_cluster_index + 1;
    for cluster_index in (column_start.saturating_add(1)..=last_cluster_index).rev() {
        if vertical_cluster_requires_previous_as_latin_word_or_unit(cluster_index, clusters) {
            return 0.0;
        }
        if vertical_cluster_requires_previous_in_column(cluster_index, clusters, config) {
            suffix_start = cluster_index;
        } else {
            break;
        }
    }
    if suffix_start > last_cluster_index {
        return 0.0;
    }

    clusters[suffix_start..=last_cluster_index]
        .iter()
        .filter(|cluster| !is_vertical_line_break_cluster(&cluster.text))
        .map(|cluster| vertical_cluster_advance(cluster, config))
        .sum()
}

fn vertical_column_segment_overhang_uses_linebreak_continuation(
    clusters: &[VerticalCluster],
    column_start: usize,
    column_end: usize,
    config: TextLayoutConfig,
) -> bool {
    let Some(last_cluster_index) = (column_start..column_end)
        .rfind(|index| !is_vertical_line_break_cluster(&clusters[*index].text))
    else {
        return false;
    };

    for cluster_index in (column_start.saturating_add(1)..=last_cluster_index).rev() {
        if !vertical_cluster_requires_previous_in_column(cluster_index, clusters, config) {
            return false;
        }
        if vertical_cluster_requires_previous_by_linebreak_only(cluster_index, clusters, config) {
            return true;
        }
    }
    false
}

fn vertical_cluster_requires_previous_in_column(
    cluster_index: usize,
    clusters: &[VerticalCluster],
    config: TextLayoutConfig,
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    if reference_mark_sequence_requires_previous(cluster_index, clusters) {
        return true;
    }
    if vertical_cluster_requires_previous_by_linebreak_only(cluster_index, clusters, config) {
        return true;
    }
    if jlreq_punctuation::is_line_head_prohibited_cluster(&cluster.text) {
        return true;
    }
    vertical_cluster_has_jlreq_separation_prohibited_before(
        cluster_index,
        clusters,
        config.jlreq_strictness,
    )
}

fn vertical_cluster_requires_previous_as_latin_word_or_unit(
    cluster_index: usize,
    clusters: &[VerticalCluster],
) -> bool {
    latin_word_sequence_requires_previous(cluster_index, clusters)
        || numeric_unit_symbol_sequence_requires_previous(cluster_index, clusters)
}

fn vertical_cluster_requires_previous_by_linebreak_only(
    cluster_index: usize,
    clusters: &[VerticalCluster],
    config: TextLayoutConfig,
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    let Some(previous) = cluster_index
        .checked_sub(1)
        .and_then(|previous_index| clusters.get(previous_index))
    else {
        return false;
    };
    let requires_ascii_digit_sequence = !cluster.break_allowed_before
        && is_ascii_digit_cluster_text(&previous.text)
        && is_ascii_digit_cluster_text(&cluster.text);
    let requires_ascii_number_separator_sequence =
        ascii_number_separator_sequence_requires_previous(cluster_index, clusters);
    let requires_numeric_abbreviation_sequence =
        numeric_abbreviation_sequence_requires_previous(cluster_index, clusters);
    let requires_latin_word_sequence =
        latin_word_sequence_requires_previous(cluster_index, clusters);
    let requires_numeric_unit_symbol_sequence =
        numeric_unit_symbol_sequence_requires_previous(cluster_index, clusters);
    let requires_sub_superscript_object_sequence =
        sub_superscript_object_sequence_requires_previous(cluster_index, clusters);
    !jlreq_punctuation::is_line_end_prohibited_cluster(&cluster.text)
        && !jlreq_punctuation::is_line_head_prohibited_cluster(&cluster.text)
        && (requires_ascii_digit_sequence
            || requires_ascii_number_separator_sequence
            || requires_numeric_abbreviation_sequence
            || requires_latin_word_sequence
            || requires_numeric_unit_symbol_sequence
            || requires_sub_superscript_object_sequence)
        && !vertical_cluster_has_jlreq_separation_prohibited_before(
            cluster_index,
            clusters,
            config.jlreq_strictness,
        )
}

fn is_ascii_digit_cluster_text(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

fn ascii_number_separator_sequence_requires_previous(
    cluster_index: usize,
    clusters: &[VerticalCluster],
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    if is_ascii_number_separator_cluster_text(&cluster.text) {
        return cluster_index
            .checked_sub(1)
            .and_then(|previous_index| clusters.get(previous_index))
            .is_some_and(|previous| is_ascii_digit_cluster_text(&previous.text))
            && clusters
                .get(cluster_index + 1)
                .is_some_and(|next| is_ascii_digit_cluster_text(&next.text));
    }
    is_ascii_digit_cluster_text(&cluster.text)
        && cluster_index
            .checked_sub(1)
            .and_then(|previous_index| clusters.get(previous_index))
            .is_some_and(|previous| is_ascii_number_separator_cluster_text(&previous.text))
        && cluster_index
            .checked_sub(2)
            .and_then(|before_separator_index| clusters.get(before_separator_index))
            .is_some_and(|before_separator| is_ascii_digit_cluster_text(&before_separator.text))
}

fn is_ascii_number_separator_cluster_text(text: &str) -> bool {
    matches!(text, "," | "." | " ")
}

fn numeric_abbreviation_sequence_requires_previous(
    cluster_index: usize,
    clusters: &[VerticalCluster],
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    if is_numeric_suffix_abbreviation_cluster_text(&cluster.text) {
        return cluster_index
            .checked_sub(1)
            .and_then(|previous_index| clusters.get(previous_index))
            .is_some_and(|previous| is_jlreq_numeric_cluster_text(&previous.text));
    }
    if postfixed_abbreviation_unit_tail_requires_previous(cluster_index, clusters) {
        return true;
    }
    is_jlreq_numeric_cluster_text(&cluster.text)
        && cluster_index
            .checked_sub(1)
            .and_then(|previous_index| clusters.get(previous_index))
            .is_some_and(|previous| is_numeric_prefix_abbreviation_cluster_text(&previous.text))
}

fn is_jlreq_numeric_cluster_text(text: &str) -> bool {
    is_ascii_digit_cluster_text(text) || is_ideographic_numeral_cluster_text(text)
}

fn is_ideographic_numeral_cluster_text(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(ch) = chars.next() else {
        return false;
    };
    chars.next().is_none()
        && matches!(
            ch,
            '〇' | '零'
                | '一'
                | '二'
                | '三'
                | '四'
                | '五'
                | '六'
                | '七'
                | '八'
                | '九'
                | '十'
                | '百'
                | '千'
                | '万'
                | '億'
                | '兆'
        )
}

fn is_numeric_prefix_abbreviation_cluster_text(text: &str) -> bool {
    matches!(text, "$" | "¢" | "¥" | "￥")
}

fn is_numeric_suffix_abbreviation_cluster_text(text: &str) -> bool {
    matches!(text, "%" | "％" | "‰" | "°" | "′" | "″" | "℃")
}

fn postfixed_abbreviation_unit_tail_requires_previous(
    cluster_index: usize,
    clusters: &[VerticalCluster],
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    is_latin_or_greek_alphabetic_cluster_text(&cluster.text)
        && cluster_index
            .checked_sub(1)
            .and_then(|previous_index| clusters.get(previous_index))
            .is_some_and(|previous| is_postfixed_abbreviation_unit_leader(&previous.text))
        && cluster_index
            .checked_sub(2)
            .and_then(|numeric_index| clusters.get(numeric_index))
            .is_some_and(|numeric| is_jlreq_numeric_cluster_text(&numeric.text))
}

fn is_postfixed_abbreviation_unit_leader(text: &str) -> bool {
    matches!(text, "°" | "′" | "″")
}

fn numeric_unit_symbol_sequence_requires_previous(
    cluster_index: usize,
    clusters: &[VerticalCluster],
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    is_latin_or_greek_alphabetic_cluster_text(&cluster.text)
        && cluster_index
            .checked_sub(1)
            .and_then(|previous_index| clusters.get(previous_index))
            .is_some_and(|previous| is_jlreq_numeric_cluster_text(&previous.text))
}

fn latin_word_sequence_requires_previous(
    cluster_index: usize,
    clusters: &[VerticalCluster],
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    let previous = cluster_index
        .checked_sub(1)
        .and_then(|previous_index| clusters.get(previous_index));
    let next = clusters.get(cluster_index + 1);
    if is_latin_word_joiner_cluster_text(&cluster.text) {
        return previous
            .is_some_and(|previous| is_latin_or_greek_alphabetic_cluster_text(&previous.text))
            && next.is_some_and(|next| is_latin_or_greek_alphabetic_cluster_text(&next.text));
    }
    if !is_latin_or_greek_alphabetic_cluster_text(&cluster.text) {
        return false;
    }
    previous.is_some_and(|previous| {
        (!cluster.break_allowed_before && is_latin_or_greek_alphabetic_cluster_text(&previous.text))
            || (is_latin_word_joiner_cluster_text(&previous.text)
                && cluster_index
                    .checked_sub(2)
                    .and_then(|before_joiner_index| clusters.get(before_joiner_index))
                    .is_some_and(|before_joiner| {
                        is_latin_or_greek_alphabetic_cluster_text(&before_joiner.text)
                    }))
    })
}

fn is_latin_or_greek_alphabetic_cluster_text(text: &str) -> bool {
    let mut has_script_letter = false;
    for ch in text.chars() {
        if is_latin_or_greek_alphabetic_char(ch) {
            has_script_letter = true;
        } else if !(is_combining_mark(ch) || is_variation_selector(ch)) {
            return false;
        }
    }
    has_script_letter
}

const fn is_latin_or_greek_alphabetic_char(ch: char) -> bool {
    matches!(
        ch,
        'A'..='Z'
            | 'a'..='z'
            | '\u{00b5}'
            | '\u{00c0}'..='\u{00ff}'
            | '\u{0100}'..='\u{024f}'
            | '\u{0370}'..='\u{03ff}'
            | '\u{1f00}'..='\u{1fff}'
            | '\u{1e00}'..='\u{1eff}'
            | '\u{ff21}'..='\u{ff3a}'
            | '\u{ff41}'..='\u{ff5a}'
    )
}

fn is_latin_word_joiner_cluster_text(text: &str) -> bool {
    matches!(text, "'" | "\u{2019}" | "-" | "\u{2010}" | "\u{2011}")
}

fn sub_superscript_object_sequence_requires_previous(
    cluster_index: usize,
    clusters: &[VerticalCluster],
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    let Some(previous) = cluster_index
        .checked_sub(1)
        .and_then(|previous_index| clusters.get(previous_index))
    else {
        return false;
    };
    (is_sub_superscript_cluster_text(&cluster.text)
        && (is_sub_superscript_base_cluster_text(&previous.text)
            || is_sub_superscript_cluster_text(&previous.text)))
        || (is_sub_superscript_base_cluster_text(&cluster.text)
            && is_sub_superscript_cluster_text(&previous.text))
}

fn is_sub_superscript_base_cluster_text(text: &str) -> bool {
    is_ascii_digit_cluster_text(text) || is_latin_or_greek_alphabetic_cluster_text(text)
}

fn is_sub_superscript_cluster_text(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(ch) = chars.next() else {
        return false;
    };
    chars.next().is_none()
        && (matches!(ch, '\u{00b2}' | '\u{00b3}' | '\u{00b9}')
            || matches!(ch, '\u{2070}'..='\u{209f}'))
}

fn reference_mark_sequence_requires_previous(
    cluster_index: usize,
    clusters: &[VerticalCluster],
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    let Some(previous) = cluster_index
        .checked_sub(1)
        .and_then(|previous_index| clusters.get(previous_index))
    else {
        return false;
    };
    if is_reference_mark_part_cluster_text(&cluster.text) {
        return true;
    }
    is_reference_mark_following_full_stop_cluster_text(&cluster.text)
        && is_reference_mark_part_cluster_text(&previous.text)
}

fn is_reference_mark_part_cluster_text(text: &str) -> bool {
    matches!(
        text,
        "¹" | "²" | "³" | "⁰" | "⁴" | "⁵" | "⁶" | "⁷" | "⁸" | "⁹" | "⁽" | "⁾"
    )
}

fn is_reference_mark_following_full_stop_cluster_text(text: &str) -> bool {
    matches!(text, "。" | "．" | ".")
}

fn vertical_cluster_can_start_column(
    cluster_index: usize,
    clusters: &[VerticalCluster],
    strictness: JlreqStrictness,
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    let can_break_before = cluster.break_allowed_before
        || jlreq_punctuation::is_line_end_prohibited_cluster(&cluster.text);
    can_break_before
        && !jlreq_punctuation::is_line_head_prohibited_cluster(&cluster.text)
        && !ascii_number_separator_sequence_requires_previous(cluster_index, clusters)
        && !numeric_abbreviation_sequence_requires_previous(cluster_index, clusters)
        && !numeric_unit_symbol_sequence_requires_previous(cluster_index, clusters)
        && !latin_word_sequence_requires_previous(cluster_index, clusters)
        && !sub_superscript_object_sequence_requires_previous(cluster_index, clusters)
        && !reference_mark_sequence_requires_previous(cluster_index, clusters)
        && !vertical_cluster_has_jlreq_separation_prohibited_before(
            cluster_index,
            clusters,
            strictness,
        )
}

fn vertical_column_ends_with_line_end_prohibited(
    column_start: usize,
    column_end: usize,
    clusters: &[VerticalCluster],
) -> bool {
    clusters[column_start..column_end]
        .iter()
        .rev()
        .find(|cluster| !is_vertical_line_break_cluster(&cluster.text))
        .is_some_and(|cluster| jlreq_punctuation::is_line_end_prohibited_cluster(&cluster.text))
}

fn vertical_cluster_origin_y(
    grapheme: &str,
    cursor_y: f32,
    advance: f32,
    config: TextLayoutConfig,
) -> f32 {
    let column_end = config.origin.y + config.size.height;
    if jlreq_punctuation::is_hanging_cluster(grapheme)
        && cursor_y + config.font_size > column_end
        && cursor_y > config.origin.y
    {
        (column_end - advance).max(config.origin.y)
    } else {
        cursor_y
    }
}

#[derive(Clone, Copy)]
struct RunLayoutContext<'a> {
    run_index: usize,
    range_start: usize,
    source: RichTextTextSource,
    presentation: &'a RichTextPresentation,
    ruby_annotations: &'a [RichTextRubyAnnotation],
    config: TextLayoutConfig,
}

fn vertical_run_can_restart_at_boundary(
    source: RichTextTextSource,
    clusters: &[VerticalCluster],
    segment_start: usize,
    strictness: JlreqStrictness,
    previous_cluster_text: Option<&str>,
) -> bool {
    if !matches!(
        source,
        RichTextTextSource::Text
            | RichTextTextSource::Interpolation
            | RichTextTextSource::InterpolationFallback
            | RichTextTextSource::ControlRaw
    ) {
        return false;
    }
    if segment_start > 0 {
        return true;
    }
    let Some(first_cluster) = clusters[segment_start..]
        .iter()
        .find(|cluster| !is_vertical_line_break_cluster(&cluster.text))
    else {
        return true;
    };
    if jlreq_punctuation::is_line_head_prohibited_cluster(&first_cluster.text) {
        return false;
    }
    if let Some(previous) = previous_cluster_text {
        let rule = jlreq_punctuation::pair_adjustment_for_clusters(
            previous,
            &first_cluster.text,
            strictness,
        );
        if rule.keep_together || rule.break_penalty > 0 {
            return false;
        }
    }
    true
}

fn vertical_cluster_required_inline_extent(
    range: RichTextRange,
    range_start: usize,
    clusters: &[VerticalCluster],
    ruby_annotations: &[RichTextRubyAnnotation],
    config: TextLayoutConfig,
) -> f32 {
    ruby_annotations
        .iter()
        .filter(|annotation| annotation.base_range.start == range.start)
        .filter(|annotation| ranges_overlap(annotation.base_range, range))
        .filter(|annotation| {
            !matches!(
                ruby_position(annotation),
                RichTextRubyPosition::InterCharacter
            )
        })
        .map(|annotation| {
            let base_cluster_extent = vertical_ruby_base_cluster_extent(
                annotation.base_range,
                range_start,
                clusters,
                config,
            );
            ruby_text_extent(&annotation.ruby, ruby_metrics(annotation, config).font_size)
                .max(base_cluster_extent)
        })
        .fold(config.font_size, f32::max)
        .min(config.size.height)
}

fn vertical_side_ruby_annotation_starting_at<'a>(
    ruby_annotations: &'a [RichTextRubyAnnotation],
    range_start: usize,
    clusters: &[VerticalCluster],
    cluster_index: usize,
) -> Option<&'a RichTextRubyAnnotation> {
    let cluster = clusters.get(cluster_index)?;
    let absolute_start = range_start + cluster.range.start;
    let range_end = range_start + clusters.last().map_or(0, |cluster| cluster.range.end);
    ruby_annotations.iter().find(|annotation| {
        annotation.base_range.start == absolute_start
            && annotation.base_range.start < annotation.base_range.end
            && annotation.base_range.end <= range_end
            && !matches!(
                ruby_position(annotation),
                RichTextRubyPosition::InterCharacter
            )
    })
}

fn vertical_ruby_base_cluster_span(
    annotation: &RichTextRubyAnnotation,
    range_start: usize,
    clusters: &[VerticalCluster],
    cluster_index: usize,
) -> Range<usize> {
    let mut end = cluster_index;
    while let Some(cluster) = clusters.get(end) {
        if is_vertical_line_break_cluster(&cluster.text) {
            break;
        }
        end += 1;
        if range_start + cluster.range.end >= annotation.base_range.end {
            break;
        }
    }
    cluster_index..end.max(cluster_index + 1).min(clusters.len())
}

fn vertical_cluster_span_layout_advance(
    clusters: &[VerticalCluster],
    config: TextLayoutConfig,
    reserve: LayoutEffectReserve,
) -> f32 {
    clusters
        .iter()
        .filter(|cluster| !is_vertical_line_break_cluster(&cluster.text))
        .map(|cluster| vertical_cluster_layout_advance(cluster, config, reserve))
        .sum::<f32>()
        .max(config.font_size)
}

fn vertical_ruby_base_allocation_height(
    base_extent: f32,
    ruby_extent: f32,
    config: TextLayoutConfig,
) -> f32 {
    ruby_extent.max(base_extent).min(config.size.height)
}

fn vertical_inter_character_ruby_extent_after(
    range: RichTextRange,
    context: RunLayoutContext<'_>,
) -> f32 {
    context
        .ruby_annotations
        .iter()
        .filter(|annotation| annotation.base_range.start == range.start)
        .filter(|annotation| ranges_overlap(annotation.base_range, range))
        .filter(|annotation| {
            matches!(
                ruby_position(annotation),
                RichTextRubyPosition::InterCharacter
            )
        })
        .map(|annotation| ruby_text_extent(&annotation.ruby, context.config.ruby_font_size))
        .sum()
}

fn vertical_cluster_advance(cluster: &VerticalCluster, config: TextLayoutConfig) -> f32 {
    if jlreq_punctuation::is_compressible_cluster(&cluster.text) {
        config.font_size * 0.5
    } else if cluster_is_sideways_latin_run(cluster) {
        horizontal_text_layout_advance(
            &cluster.text,
            config.font_size,
            LayoutEffectReserve::default(),
        )
        .max(config.font_size)
    } else {
        config.font_size
    }
}

fn vertical_cluster_layout_advance(
    cluster: &VerticalCluster,
    config: TextLayoutConfig,
    reserve: LayoutEffectReserve,
) -> f32 {
    vertical_cluster_advance(cluster, config) + reserve.y * 2.0
}

fn vertical_cluster_has_jlreq_separation_prohibited_before(
    cluster_index: usize,
    clusters: &[VerticalCluster],
    strictness: JlreqStrictness,
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    clusters[..cluster_index]
        .iter()
        .rev()
        .find(|candidate| !is_vertical_line_break_cluster(&candidate.text))
        .is_some_and(|previous| {
            jlreq_punctuation::pair_adjustment_for_clusters(
                &previous.text,
                &cluster.text,
                strictness,
            )
            .keep_together
        })
}

fn vertical_ruby_base_cluster_extent(
    base_range: RichTextRange,
    range_start: usize,
    clusters: &[VerticalCluster],
    config: TextLayoutConfig,
) -> f32 {
    clusters
        .iter()
        .filter(|cluster| {
            let start = range_start + cluster.range.start;
            let end = range_start + cluster.range.end;
            ranges_overlap(RichTextRange::new(start, end), base_range)
                && !is_vertical_line_break_cluster(&cluster.text)
        })
        .map(|cluster| vertical_cluster_advance(cluster, config))
        .sum::<f32>()
        .max(config.font_size)
}

fn max_ruby_chars_per_vertical_segment(config: TextLayoutConfig, metrics: RubyMetrics) -> usize {
    let mut count = 1usize;
    let mut extent = metrics.font_size.max(1.0);
    let max_extent = config.size.height.max(extent);
    while extent + metrics.font_size <= max_extent {
        count += 1;
        extent += metrics.font_size;
    }
    count
}

fn ruby_position(annotation: &RichTextRubyAnnotation) -> RichTextRubyPosition {
    annotation
        .presentation
        .layout
        .as_ref()
        .map_or(RichTextRubyPosition::Auto, |layout| layout.ruby_position)
}

fn valid_range(
    frame: &LineDisplayFrame,
    range: RichTextRange,
) -> Result<Range<usize>, TextLayoutError> {
    let range = range.start..range.end;
    if frame.text.get(range.clone()).is_some() {
        Ok(range)
    } else {
        Err(TextLayoutError::InvalidRange {
            range: RichTextRange::new(range.start, range.end),
        })
    }
}

fn horizontal_advance(ch: char, font_size: f32) -> f32 {
    if ch.is_ascii_whitespace() {
        font_size * 0.33
    } else if matches!(
        ch,
        'i' | 'j' | 'l' | 'I' | '!' | '|' | '.' | ',' | ':' | ';'
    ) {
        font_size * 0.28
    } else if matches!(ch, 'm' | 'w' | 'M' | 'W') {
        font_size * 0.82
    } else if ch.is_ascii_alphanumeric() {
        font_size * 0.54
    } else if ch.is_ascii_punctuation() {
        font_size * 0.36
    } else {
        font_size
    }
}

fn vertical_column_start(writing_mode: RichTextWritingMode, config: TextLayoutConfig) -> f32 {
    match writing_mode {
        RichTextWritingMode::VerticalRl => {
            config.origin.x + config.size.width - config.line_advance
        }
        RichTextWritingMode::VerticalLr | RichTextWritingMode::HorizontalTb => config.origin.x,
    }
}

fn vertical_column_step(
    writing_mode: RichTextWritingMode,
    presentation: &RichTextPresentation,
    config: TextLayoutConfig,
) -> f32 {
    let gap = presentation
        .layout
        .as_ref()
        .map_or(8.0, |layout| layout.column_gap.as_f32());
    let step = config.line_advance + gap;
    match writing_mode {
        RichTextWritingMode::VerticalRl => -step,
        RichTextWritingMode::VerticalLr | RichTextWritingMode::HorizontalTb => step,
    }
}

#[derive(Clone, Debug)]
struct VerticalCluster {
    range: Range<usize>,
    text: String,
    orientation: GlyphOrientation,
    vertical_form: GlyphVerticalForm,
    break_allowed_before: bool,
}

const MAX_TEXT_COMBINE_DIGITS: usize = 4;

fn vertical_clusters(
    text: &str,
    vertical_latin: RichTextVerticalLatinMode,
) -> Vec<VerticalCluster> {
    let mut clusters = Vec::new();
    let graphemes: Vec<(usize, &str)> = text.grapheme_indices(true).collect();
    let break_offsets = line_break_offsets(text);
    let mut index = 0;
    while let Some((offset, grapheme)) = graphemes.get(index).copied() {
        if is_ascii_digit_grapheme(grapheme) {
            let mut end = offset + grapheme.len();
            let mut value = grapheme.to_owned();
            let mut digit_count = 1;
            let break_allowed_before = break_offsets.contains(&offset);
            index += 1;
            while let Some((next_offset, next)) = graphemes.get(index).copied() {
                if !is_ascii_digit_grapheme(next) || digit_count >= MAX_TEXT_COMBINE_DIGITS {
                    break;
                }
                value.push_str(next);
                end = next_offset + next.len();
                digit_count += 1;
                index += 1;
            }
            if digit_count >= 2 {
                clusters.push(VerticalCluster {
                    range: offset..end,
                    text: value,
                    orientation: GlyphOrientation::TextCombineUpright,
                    vertical_form: GlyphVerticalForm::None,
                    break_allowed_before,
                });
                continue;
            }
            let orientation = vertical_orientation(grapheme, vertical_latin);
            clusters.push(VerticalCluster {
                range: offset..end,
                text: value,
                orientation,
                vertical_form: vertical_form(grapheme, vertical_latin),
                break_allowed_before,
            });
            continue;
        }
        if is_sideways_latin_run_grapheme(grapheme, vertical_latin) {
            let mut end = offset + grapheme.len();
            let mut value = grapheme.to_owned();
            let break_allowed_before = break_offsets.contains(&offset);
            index += 1;
            while let Some((next_offset, next)) = graphemes.get(index).copied() {
                if !is_sideways_latin_run_grapheme(next, vertical_latin) {
                    break;
                }
                value.push_str(next);
                end = next_offset + next.len();
                index += 1;
            }
            clusters.push(VerticalCluster {
                range: offset..end,
                text: value,
                orientation: GlyphOrientation::SidewaysCw,
                vertical_form: GlyphVerticalForm::None,
                break_allowed_before,
            });
            continue;
        }
        index += 1;
        let orientation = vertical_orientation(grapheme, vertical_latin);
        clusters.push(VerticalCluster {
            range: offset..offset + grapheme.len(),
            text: grapheme.to_owned(),
            orientation,
            vertical_form: vertical_form(grapheme, vertical_latin),
            break_allowed_before: break_offsets.contains(&offset),
        });
    }
    clusters
}

fn is_sideways_latin_run_grapheme(
    grapheme: &str,
    vertical_latin: RichTextVerticalLatinMode,
) -> bool {
    is_latin_or_greek_alphabetic_cluster_text(grapheme)
        && matches!(
            vertical_orientation(grapheme, vertical_latin),
            GlyphOrientation::SidewaysCw
        )
}

fn cluster_is_sideways_latin_run(cluster: &VerticalCluster) -> bool {
    cluster.orientation == GlyphOrientation::SidewaysCw
        && cluster.text.graphemes(true).count() > 1
        && cluster
            .text
            .graphemes(true)
            .all(is_latin_or_greek_alphabetic_cluster_text)
}

fn line_break_offsets(text: &str) -> HashSet<usize> {
    linebreaks(text)
        .filter_map(|(offset, opportunity)| match opportunity {
            BreakOpportunity::Allowed | BreakOpportunity::Mandatory if offset < text.len() => {
                Some(offset)
            }
            BreakOpportunity::Allowed | BreakOpportunity::Mandatory => None,
        })
        .collect()
}

fn is_ascii_digit_grapheme(grapheme: &str) -> bool {
    matches!(grapheme.as_bytes(), [b'0'..=b'9'])
}

fn is_vertical_line_break_cluster(grapheme: &str) -> bool {
    matches!(grapheme, "\n" | "\r\n")
}

fn vertical_orientation(
    grapheme: &str,
    vertical_latin: RichTextVerticalLatinMode,
) -> GlyphOrientation {
    match vertical_latin {
        RichTextVerticalLatinMode::Upright => GlyphOrientation::Upright,
        RichTextVerticalLatinMode::Sideways => GlyphOrientation::SidewaysCw,
        RichTextVerticalLatinMode::Mixed => {
            match unicode_vertical_orientation_for_grapheme(grapheme) {
                UnicodeVerticalOrientation::Upright
                | UnicodeVerticalOrientation::TransformedUpright => GlyphOrientation::Upright,
                UnicodeVerticalOrientation::Rotated
                | UnicodeVerticalOrientation::TransformedRotated => GlyphOrientation::SidewaysCw,
            }
        }
    }
}

fn vertical_form(grapheme: &str, vertical_latin: RichTextVerticalLatinMode) -> GlyphVerticalForm {
    if !matches!(vertical_latin, RichTextVerticalLatinMode::Mixed) {
        return GlyphVerticalForm::None;
    }
    match unicode_vertical_orientation_for_grapheme(grapheme) {
        UnicodeVerticalOrientation::Upright | UnicodeVerticalOrientation::Rotated => {
            GlyphVerticalForm::None
        }
        UnicodeVerticalOrientation::TransformedUpright => GlyphVerticalForm::UprightAlternate,
        UnicodeVerticalOrientation::TransformedRotated => GlyphVerticalForm::RotatedAlternate,
    }
}

fn unicode_vertical_orientation_for_grapheme(grapheme: &str) -> UnicodeVerticalOrientation {
    if is_keycap_grapheme(grapheme) {
        return UnicodeVerticalOrientation::Upright;
    }
    grapheme
        .chars()
        .find(|ch| !is_grapheme_modifier_or_join_control(*ch))
        .or_else(|| grapheme.chars().next())
        .map_or(
            UnicodeVerticalOrientation::Rotated,
            unicode_vertical_orientation,
        )
}

fn is_keycap_grapheme(grapheme: &str) -> bool {
    let Some(head) = grapheme.chars().next() else {
        return false;
    };
    matches!(head, '#' | '*' | '0'..='9') && grapheme.chars().any(|ch| ch == '\u{20e3}')
}

const fn is_grapheme_modifier_or_join_control(ch: char) -> bool {
    is_combining_mark(ch) || is_variation_selector(ch) || matches!(ch, '\u{200c}' | '\u{200d}')
}

const fn is_combining_mark(ch: char) -> bool {
    matches!(
        ch,
        '\u{0300}'..='\u{036f}'
            | '\u{1ab0}'..='\u{1aff}'
            | '\u{1dc0}'..='\u{1dff}'
            | '\u{20d0}'..='\u{20ff}'
            | '\u{fe20}'..='\u{fe2f}'
    )
}

const fn is_variation_selector(ch: char) -> bool {
    matches!(ch, '\u{fe00}'..='\u{fe0f}' | '\u{e0100}'..='\u{e01ef}')
}

fn union_bounds(rects: impl IntoIterator<Item = LayoutRect>) -> Option<LayoutRect> {
    rects.into_iter().reduce(LayoutRect::union)
}

fn ranges_overlap(left: RichTextRange, right: RichTextRange) -> bool {
    left.start < right.end && right.start < left.end
}

fn usize_to_f32(value: usize) -> f32 {
    let value = u16::try_from(value).unwrap_or(u16::MAX);
    f32::from(value)
}

#[cfg(test)]
mod tests;
