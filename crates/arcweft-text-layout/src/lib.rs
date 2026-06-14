//! Sans I/O rich-text layout geometry for Arcweft players and agent debugging.
//!
//! This crate owns deterministic text geometry. Renderer adapters consume the
//! resulting `LaidOutText` instead of deriving bounds from pixels or from
//! renderer-specific buffers.

use arcweft_render_text::{
    LineDisplayFrame, RichTextJlreqStrictness, RichTextPresentation, RichTextRange,
    RichTextRubyAnnotation, RichTextVerticalLatinMode, RichTextWritingMode,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ops::Range;
use thiserror::Error;
use unicode_linebreak::{BreakOpportunity, linebreaks};
use unicode_segmentation::UnicodeSegmentation as _;

mod jlreq_punctuation;
mod jlreq_punctuation_data;
mod vertical_orientation;
pub use jlreq_punctuation_data::{
    JLREQ_PAIR_ADJUSTMENT_DATA_VERSION, JLREQ_PUNCTUATION_DATA_VERSION,
};
pub use vertical_orientation::UNICODE_VERTICAL_ORIENTATION_VERSION;
use vertical_orientation::{UnicodeVerticalOrientation, unicode_vertical_orientation};

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
        }
    }
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
                layout_horizontal_run(
                    &mut out.glyphs,
                    run_index,
                    range.start,
                    text,
                    &run.presentation,
                    run_config,
                    &mut state,
                );
            }
            RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr => {
                let context = RunLayoutContext {
                    run_index,
                    range_start: range.start,
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

#[derive(Clone, Copy, Debug)]
struct TextLayoutState {
    horizontal: LayoutCursor,
    vertical_rl: LayoutCursor,
    vertical_lr: LayoutCursor,
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
    let gap = config.ruby_font_size * 0.25;
    let segment_count = vertical_ruby_segment_count(annotation.ruby.chars().count(), config).max(1);
    gap + config.ruby_font_size
        + usize_to_f32(segment_count.saturating_sub(1))
            * vertical_ruby_continuation_track_step(config)
}

fn vertical_ruby_segment_count(char_count: usize, config: TextLayoutConfig) -> usize {
    let max_chars = max_ruby_chars_per_vertical_segment(config).max(1);
    char_count.max(1).div_ceil(max_chars)
}

fn vertical_ruby_continuation_track_step(config: TextLayoutConfig) -> f32 {
    const GAP: f32 = 2.0;
    config.ruby_font_size + GAP
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

fn layout_horizontal_run(
    glyphs: &mut Vec<LaidOutGlyph>,
    run_index: usize,
    range_start: usize,
    text: &str,
    presentation: &RichTextPresentation,
    config: TextLayoutConfig,
    state: &mut TextLayoutState,
) {
    let cursor = &mut state.horizontal;
    for (offset, ch) in text.char_indices() {
        if ch == '\n' {
            cursor.x = config.origin.x;
            cursor.y += config.line_advance;
            continue;
        }
        let width = horizontal_advance(ch, config.font_size);
        let start = range_start + offset;
        let end = start + ch.len_utf8();
        let bounds = LayoutRect::new(cursor.x, cursor.y, width.max(1.0), config.line_advance);
        glyphs.push(LaidOutGlyph {
            run_index,
            range: RichTextRange::new(start, end),
            text: ch.to_string(),
            origin: LayoutPoint::new(cursor.x, cursor.y),
            advance: LayoutSize::new(width, 0.0),
            bounds,
            writing_mode: RichTextWritingMode::HorizontalTb,
            orientation: GlyphOrientation::Upright,
            vertical_form: GlyphVerticalForm::None,
            presentation: presentation.clone(),
        });
        cursor.x += width;
    }
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
    let cursor = match writing_mode {
        RichTextWritingMode::VerticalRl => &mut state.vertical_rl,
        RichTextWritingMode::VerticalLr => &mut state.vertical_lr,
        RichTextWritingMode::HorizontalTb => {
            unreachable!("horizontal runs use layout_horizontal_run")
        }
    };
    let clusters = vertical_clusters(text, vertical_latin);
    let column_plan = plan_vertical_columns(&clusters, context, *cursor);
    for (cluster_index, cluster) in clusters.iter().enumerate() {
        if is_vertical_line_break_cluster(&cluster.text) {
            cursor.x += column_step;
            cursor.y = config.origin.y;
            continue;
        }
        let start = context.range_start + cluster.range.start;
        let end = context.range_start + cluster.range.end;
        let range = RichTextRange::new(start, end);
        if column_plan.breaks_before(cluster_index) {
            cursor.x += column_step;
            cursor.y = config.origin.y;
        }
        let advance = vertical_cluster_advance(&cluster.text, config);
        let glyph_y = vertical_cluster_origin_y(&cluster.text, cursor.y, advance, config);
        let bounds = LayoutRect::new(cursor.x, glyph_y, config.line_advance, config.line_advance);
        glyphs.push(LaidOutGlyph {
            run_index: context.run_index,
            range,
            text: cluster.text.clone(),
            origin: LayoutPoint::new(cursor.x, glyph_y),
            advance: LayoutSize::new(0.0, advance),
            bounds,
            writing_mode,
            orientation: cluster.orientation,
            vertical_form: cluster.vertical_form,
            presentation: context.presentation.clone(),
        });
        cursor.y += advance;
    }
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
) {
    if segment_start >= segment_end {
        return;
    }

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
    while cursor > 0 {
        let Some(state) = states[cursor] else {
            return;
        };
        if state.previous_break > 0 {
            plan.set_break_before(segment_start + state.previous_break);
        }
        cursor = state.previous_break;
    }
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

    let capacity = context.config.origin.y + context.config.size.height - column_start_y;
    let used = vertical_column_segment_required_extent(clusters, column_start, column_end, context);
    let overflow = (used - capacity).max(0.0);
    let allowed_overhang = vertical_column_segment_overhang_allowance(
        clusters,
        column_start,
        column_end,
        context.config,
    );
    if column_end < segment_end && overflow > allowed_overhang + f32::EPSILON {
        return None;
    }

    let remaining = (capacity - used).max(0.0);
    let capacity = capacity.max(context.config.line_advance);
    let badness = 100.0 * (remaining / capacity).powi(3);
    let overflow_penalty =
        ((overflow - allowed_overhang).max(0.0) / context.config.line_advance).powi(2) * 10_000.0;
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
    Some(badness + overflow_penalty + break_penalty)
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
    for cluster in &clusters[column_start..column_end] {
        let range = RichTextRange::new(
            context.range_start + cluster.range.start,
            context.range_start + cluster.range.end,
        );
        let required_inline_extent = vertical_cluster_required_inline_extent(
            range,
            context.range_start,
            clusters,
            context.ruby_annotations,
            context.config,
        );
        required = required.max(cursor + required_inline_extent);
        cursor += vertical_cluster_advance(&cluster.text, context.config);
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
        .map(|_| config.line_advance)
        .sum()
}

fn vertical_cluster_requires_previous_in_column(
    cluster_index: usize,
    clusters: &[VerticalCluster],
    config: TextLayoutConfig,
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    if jlreq_punctuation::is_line_head_prohibited_cluster(&cluster.text) {
        return true;
    }
    vertical_cluster_has_jlreq_separation_prohibited_before(
        cluster_index,
        clusters,
        config.jlreq_strictness,
    )
}

fn vertical_cluster_can_start_column(
    cluster_index: usize,
    clusters: &[VerticalCluster],
    strictness: JlreqStrictness,
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    cluster.break_allowed_before
        && !jlreq_punctuation::is_line_head_prohibited_cluster(&cluster.text)
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
        && cursor_y + config.line_advance > column_end
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
    presentation: &'a RichTextPresentation,
    ruby_annotations: &'a [RichTextRubyAnnotation],
    config: TextLayoutConfig,
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
        .map(|annotation| {
            let base_cluster_extent =
                vertical_ruby_base_cluster_count(annotation.base_range, range_start, clusters)
                    * config.line_advance;
            ruby_text_extent(&annotation.ruby, config.ruby_font_size).max(base_cluster_extent)
        })
        .fold(config.line_advance, f32::max)
        .min(config.size.height)
}

fn vertical_cluster_advance(grapheme: &str, config: TextLayoutConfig) -> f32 {
    if jlreq_punctuation::is_compressible_cluster(grapheme) {
        config.line_advance * 0.5
    } else {
        config.line_advance
    }
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

fn vertical_ruby_base_cluster_count(
    base_range: RichTextRange,
    range_start: usize,
    clusters: &[VerticalCluster],
) -> f32 {
    let count = clusters
        .iter()
        .filter(|cluster| {
            let start = range_start + cluster.range.start;
            let end = range_start + cluster.range.end;
            ranges_overlap(RichTextRange::new(start, end), base_range)
                && !is_vertical_line_break_cluster(&cluster.text)
        })
        .count()
        .max(1);
    usize_to_f32(count)
}

fn layout_ruby(
    frame: &LineDisplayFrame,
    glyphs: &[LaidOutGlyph],
    config: TextLayoutConfig,
) -> Vec<LaidOutRuby> {
    let mut ruby = frame
        .display_map
        .ruby_annotations
        .iter()
        .enumerate()
        .flat_map(|(ruby_index, annotation)| {
            layout_one_ruby(ruby_index, annotation, glyphs, config)
        })
        .collect::<Vec<_>>();
    resolve_ruby_collisions(&mut ruby, config);
    ruby
}

fn layout_one_ruby(
    ruby_index: usize,
    annotation: &RichTextRubyAnnotation,
    glyphs: &[LaidOutGlyph],
    config: TextLayoutConfig,
) -> Vec<LaidOutRuby> {
    let base_bounds = union_bounds(
        glyphs
            .iter()
            .filter(|glyph| ranges_overlap(glyph.range, annotation.base_range))
            .map(|glyph| glyph.bounds),
    );
    let Some(base_bounds) = base_bounds else {
        return Vec::new();
    };
    let vertical = glyphs
        .iter()
        .find(|glyph| ranges_overlap(glyph.range, annotation.base_range))
        .is_some_and(|glyph| !matches!(glyph.writing_mode, RichTextWritingMode::HorizontalTb));
    let writing_mode = glyphs
        .iter()
        .find(|glyph| ranges_overlap(glyph.range, annotation.base_range))
        .map_or(RichTextWritingMode::HorizontalTb, |glyph| {
            glyph.writing_mode
        });
    let ruby_extent = ruby_text_extent(&annotation.ruby, config.ruby_font_size);
    let base_bounds = if vertical {
        expand_vertical_ruby_base(base_bounds, ruby_extent, config)
    } else {
        expand_horizontal_ruby_base(base_bounds, ruby_extent, config)
    };
    if vertical && ruby_extent > config.size.height {
        layout_overheight_vertical_ruby(ruby_index, annotation, base_bounds, writing_mode, config)
    } else if vertical {
        vec![laid_out_ruby_segment(
            ruby_index,
            annotation,
            annotation.ruby.clone(),
            base_bounds,
            LayoutRect::new(
                vertical_ruby_track_x(base_bounds, writing_mode, config),
                ruby_annotation_start(
                    base_bounds.y,
                    base_bounds.height,
                    ruby_extent,
                    ruby_overhang_limit(config),
                ),
                config.ruby_font_size,
                ruby_extent,
            ),
            writing_mode,
        )]
    } else {
        vec![laid_out_ruby_segment(
            ruby_index,
            annotation,
            annotation.ruby.clone(),
            base_bounds,
            LayoutRect::new(
                ruby_annotation_start(
                    base_bounds.x,
                    base_bounds.width,
                    ruby_extent,
                    ruby_overhang_limit(config),
                ),
                (base_bounds.y - config.ruby_font_size * 1.2).max(0.0),
                ruby_extent,
                config.ruby_font_size,
            ),
            writing_mode,
        )]
    }
}

fn layout_overheight_vertical_ruby(
    ruby_index: usize,
    annotation: &RichTextRubyAnnotation,
    base_bounds: LayoutRect,
    writing_mode: RichTextWritingMode,
    config: TextLayoutConfig,
) -> Vec<LaidOutRuby> {
    let max_chars_per_segment = max_ruby_chars_per_vertical_segment(config);
    let track_x = vertical_ruby_track_x(base_bounds, writing_mode, config);
    split_ruby_text(&annotation.ruby, max_chars_per_segment)
        .into_iter()
        .enumerate()
        .map(|(segment_index, ruby)| {
            let ruby_extent = ruby_text_extent(&ruby, config.ruby_font_size);
            laid_out_ruby_segment(
                ruby_index,
                annotation,
                ruby,
                base_bounds,
                LayoutRect::new(
                    track_x
                        + vertical_ruby_continuation_step(writing_mode, config)
                            * usize_to_f32(segment_index),
                    config.origin.y,
                    config.ruby_font_size,
                    ruby_extent,
                ),
                writing_mode,
            )
        })
        .collect()
}

fn laid_out_ruby_segment(
    ruby_index: usize,
    annotation: &RichTextRubyAnnotation,
    ruby: String,
    base_bounds: LayoutRect,
    ruby_bounds: LayoutRect,
    writing_mode: RichTextWritingMode,
) -> LaidOutRuby {
    LaidOutRuby {
        ruby_index,
        base_range: annotation.base_range,
        ruby,
        base_bounds,
        ruby_bounds,
        writing_mode,
        presentation: annotation.presentation.clone(),
    }
}

fn split_ruby_text(ruby: &str, max_chars_per_segment: usize) -> Vec<String> {
    let mut segments = Vec::new();
    let mut segment = String::new();
    for ch in ruby.chars() {
        if segment.chars().count() >= max_chars_per_segment {
            segments.push(segment);
            segment = String::new();
        }
        segment.push(ch);
    }
    if !segment.is_empty() {
        segments.push(segment);
    }
    segments
}

fn max_ruby_chars_per_vertical_segment(config: TextLayoutConfig) -> usize {
    let mut count = 1usize;
    let mut extent = config.ruby_font_size.max(1.0);
    let max_extent = config.size.height.max(extent);
    while extent + config.ruby_font_size <= max_extent {
        count += 1;
        extent += config.ruby_font_size;
    }
    count
}

fn ruby_text_extent(ruby: &str, ruby_font_size: f32) -> f32 {
    usize_to_f32(ruby.chars().count().max(1)) * ruby_font_size
}

fn ruby_overhang_limit(config: TextLayoutConfig) -> f32 {
    config.ruby_font_size * 0.5
}

fn ruby_annotation_start(
    base_start: f32,
    base_extent: f32,
    ruby_extent: f32,
    overhang: f32,
) -> f32 {
    let ideal = base_start + (base_extent - ruby_extent) * 0.5;
    let min_start = base_start - overhang;
    let max_start = base_start + base_extent + overhang - ruby_extent;
    if min_start <= max_start {
        ideal.max(min_start).min(max_start)
    } else {
        ideal
    }
}

fn expand_horizontal_ruby_base(
    base_bounds: LayoutRect,
    ruby_width: f32,
    config: TextLayoutConfig,
) -> LayoutRect {
    let width = ruby_width.max(base_bounds.width).min(config.size.width);
    let max_right = config.origin.x + config.size.width;
    let centered_x = base_bounds.x + (base_bounds.width - width) * 0.5;
    let x = centered_x
        .max(config.origin.x)
        .min((max_right - width).max(config.origin.x));
    LayoutRect::new(x, base_bounds.y, width, base_bounds.height)
}

fn expand_vertical_ruby_base(
    base_bounds: LayoutRect,
    ruby_height: f32,
    config: TextLayoutConfig,
) -> LayoutRect {
    let height = ruby_height.max(base_bounds.height).min(config.size.height);
    let max_bottom = config.origin.y + config.size.height;
    let centered_y = base_bounds.y + (base_bounds.height - height) * 0.5;
    let y = centered_y
        .max(config.origin.y)
        .min((max_bottom - height).max(config.origin.y));
    LayoutRect::new(base_bounds.x, y, base_bounds.width, height)
}

fn vertical_ruby_track_x(
    base_bounds: LayoutRect,
    writing_mode: RichTextWritingMode,
    config: TextLayoutConfig,
) -> f32 {
    let gap = config.ruby_font_size * 0.25;
    match writing_mode {
        RichTextWritingMode::VerticalLr => base_bounds.x - config.ruby_font_size - gap,
        RichTextWritingMode::VerticalRl | RichTextWritingMode::HorizontalTb => {
            base_bounds.right() + gap
        }
    }
}

fn vertical_ruby_continuation_step(
    writing_mode: RichTextWritingMode,
    config: TextLayoutConfig,
) -> f32 {
    match writing_mode {
        RichTextWritingMode::VerticalRl => vertical_ruby_continuation_track_step(config),
        RichTextWritingMode::VerticalLr | RichTextWritingMode::HorizontalTb => {
            -vertical_ruby_continuation_track_step(config)
        }
    }
}

fn resolve_ruby_collisions(ruby: &mut [LaidOutRuby], config: TextLayoutConfig) {
    let mut placed = Vec::new();
    for annotation in ruby {
        let resolved = match annotation.writing_mode {
            RichTextWritingMode::HorizontalTb => {
                resolve_horizontal_ruby_collision(annotation.ruby_bounds, &placed, config)
            }
            RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr => {
                resolve_vertical_ruby_collision(
                    annotation.ruby_bounds,
                    annotation.writing_mode,
                    &placed,
                    config,
                )
            }
        };
        annotation.ruby_bounds = resolved;
        placed.push(RubyTrackPlacement {
            writing_mode: annotation.writing_mode,
            bounds: resolved,
        });
    }
}

#[derive(Clone, Copy, Debug)]
struct RubyTrackPlacement {
    writing_mode: RichTextWritingMode,
    bounds: LayoutRect,
}

fn resolve_horizontal_ruby_collision(
    mut bounds: LayoutRect,
    placed: &[RubyTrackPlacement],
    config: TextLayoutConfig,
) -> LayoutRect {
    const GAP: f32 = 2.0;
    for previous in placed
        .iter()
        .filter(|placement| matches!(placement.writing_mode, RichTextWritingMode::HorizontalTb))
    {
        if bounds.intersects(previous.bounds) {
            bounds.x = previous.bounds.right() + GAP;
        }
    }
    let overhang = ruby_overhang_limit(config);
    let min_left = config.origin.x - overhang;
    let max_right = config.origin.x + config.size.width + overhang;
    if bounds.x < min_left {
        bounds.x = min_left;
    }
    if bounds.right() > max_right {
        bounds.x = (max_right - bounds.width).max(min_left);
        bounds.y = (bounds.y - config.ruby_font_size - GAP).max(0.0);
    }
    bounds
}

fn resolve_vertical_ruby_collision(
    mut bounds: LayoutRect,
    writing_mode: RichTextWritingMode,
    placed: &[RubyTrackPlacement],
    config: TextLayoutConfig,
) -> LayoutRect {
    const GAP: f32 = 2.0;
    for previous in placed.iter().filter(|placement| {
        matches!(
            placement.writing_mode,
            RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr
        )
    }) {
        if bounds.intersects(previous.bounds) {
            bounds.y = previous.bounds.bottom() + GAP;
        }
    }
    let overhang = ruby_overhang_limit(config);
    let min_top = config.origin.y - overhang;
    let max_bottom = config.origin.y + config.size.height + overhang;
    if bounds.y < min_top {
        bounds.y = min_top;
    }
    if bounds.bottom() > max_bottom {
        bounds.y = min_top;
        bounds.x += match writing_mode {
            RichTextWritingMode::VerticalRl => vertical_ruby_continuation_track_step(config),
            RichTextWritingMode::VerticalLr | RichTextWritingMode::HorizontalTb => {
                -vertical_ruby_continuation_track_step(config)
            }
        };
    }
    bounds
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
    if ch.is_ascii() {
        font_size * 0.55
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
    grapheme
        .chars()
        .find(|ch| !is_grapheme_modifier_or_join_control(*ch))
        .or_else(|| grapheme.chars().next())
        .map_or(
            UnicodeVerticalOrientation::Rotated,
            unicode_vertical_orientation,
        )
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
mod tests {
    use super::*;
    use arcweft_render_text::{
        LineDisplayFrame, Milli, RichTextDisplayMap, RichTextEffectDescriptor, RichTextEffectPhase,
        RichTextEffectTarget, RichTextJlreqStrictness, RichTextLayout, RichTextParam,
        RichTextStateScope, RichTextTextRun, RichTextTextSource,
    };
    use std::collections::BTreeMap;

    fn frame_with_run(text: &str, presentation: RichTextPresentation) -> LineDisplayFrame {
        LineDisplayFrame {
            line: arcweft_core::plan::RuntimeLineId("say.test.001".to_owned()),
            callee: "alice.say".to_owned(),
            text: text.to_owned(),
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            nodes: Vec::new(),
            display_map: RichTextDisplayMap {
                text_runs: vec![RichTextTextRun {
                    range: RichTextRange::new(0, text.len()),
                    source: RichTextTextSource::Text,
                    node_index: 0,
                    styles: Vec::new(),
                    presentation,
                }],
                ruby_annotations: Vec::new(),
                controls: Vec::new(),
                host_events: Vec::new(),
            },
            host_events: Vec::new(),
            inline_failures: Vec::new(),
            unresolved: Vec::new(),
        }
    }

    fn vertical_presentation(writing_mode: RichTextWritingMode) -> RichTextPresentation {
        RichTextPresentation {
            layout: Some(RichTextLayout {
                writing_mode,
                ..RichTextLayout::default()
            }),
            ..RichTextPresentation::default()
        }
    }

    fn frame_with_split_runs(
        text: &str,
        split_at: usize,
        presentation: RichTextPresentation,
    ) -> LineDisplayFrame {
        LineDisplayFrame {
            line: arcweft_core::plan::RuntimeLineId("say.test.001".to_owned()),
            callee: "alice.say".to_owned(),
            text: text.to_owned(),
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            nodes: Vec::new(),
            display_map: RichTextDisplayMap {
                text_runs: vec![
                    RichTextTextRun {
                        range: RichTextRange::new(0, split_at),
                        source: RichTextTextSource::Text,
                        node_index: 0,
                        styles: Vec::new(),
                        presentation: presentation.clone(),
                    },
                    RichTextTextRun {
                        range: RichTextRange::new(split_at, text.len()),
                        source: RichTextTextSource::Text,
                        node_index: 1,
                        styles: Vec::new(),
                        presentation,
                    },
                ],
                ruby_annotations: Vec::new(),
                controls: Vec::new(),
                host_events: Vec::new(),
            },
            host_events: Vec::new(),
            inline_failures: Vec::new(),
            unresolved: Vec::new(),
        }
    }

    #[test]
    fn horizontal_layout_keeps_source_ranges() {
        let frame = frame_with_run("A夢", RichTextPresentation::default());
        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 2);
        assert_eq!(layout.glyphs[0].range, RichTextRange::new(0, 1));
        assert_eq!(layout.glyphs[1].range, RichTextRange::new(1, 4));
        assert_eq!(layout.glyphs[0].orientation, GlyphOrientation::Upright);
        assert_eq!(layout.runs.len(), 1);
    }

    #[test]
    fn horizontal_layout_keeps_cursor_across_style_runs() {
        let frame = frame_with_split_runs("AB", 1, RichTextPresentation::default());
        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 2);
        assert_f32_eq(layout.glyphs[0].origin.x, 24.0);
        assert!(layout.glyphs[1].origin.x > layout.glyphs[0].origin.x);
        assert_f32_eq(layout.glyphs[1].origin.y, layout.glyphs[0].origin.y);
    }

    #[test]
    fn vertical_rl_lays_out_top_to_bottom_then_right_to_left() {
        let frame = frame_with_run(
            "天地人",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let config = TextLayoutConfig {
            size: LayoutSize::new(120.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_f32_eq(layout.glyphs[0].origin.x, 102.0);
        assert_f32_eq(layout.glyphs[0].origin.y, 24.0);
        assert_f32_eq(layout.glyphs[1].origin.x, 102.0);
        assert_f32_eq(layout.glyphs[1].origin.y, 66.0);
        assert!(layout.glyphs[2].origin.x < layout.glyphs[1].origin.x);
        assert_f32_eq(layout.glyphs[2].origin.y, 24.0);
    }

    #[test]
    fn vertical_lr_lays_out_top_to_bottom_then_left_to_right() {
        let frame = frame_with_run(
            "天地人",
            vertical_presentation(RichTextWritingMode::VerticalLr),
        );
        let config = TextLayoutConfig {
            size: LayoutSize::new(120.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_f32_eq(layout.glyphs[0].origin.x, 24.0);
        assert_f32_eq(layout.glyphs[0].origin.y, 24.0);
        assert_f32_eq(layout.glyphs[1].origin.x, 24.0);
        assert_f32_eq(layout.glyphs[1].origin.y, 66.0);
        assert!(layout.glyphs[2].origin.x > layout.glyphs[1].origin.x);
        assert_f32_eq(layout.glyphs[2].origin.y, 24.0);
    }

    #[test]
    fn vertical_layout_keeps_cursor_across_style_runs() {
        let frame = frame_with_split_runs(
            "天地",
            "天".len(),
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 2);
        assert_f32_eq(layout.glyphs[1].origin.x, layout.glyphs[0].origin.x);
        assert!(layout.glyphs[1].origin.y > layout.glyphs[0].origin.y);
    }

    #[test]
    fn vertical_mixed_rotates_latin_and_combines_short_digits() {
        let frame = frame_with_run(
            "吾A12",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 3);
        assert_eq!(layout.glyphs[0].orientation, GlyphOrientation::Upright);
        assert_eq!(layout.glyphs[1].orientation, GlyphOrientation::SidewaysCw);
        assert_eq!(layout.glyphs[2].text, "12");
        assert_eq!(
            layout.glyphs[2].orientation,
            GlyphOrientation::TextCombineUpright
        );
    }

    #[test]
    fn vertical_layout_uses_grapheme_clusters_for_mixed_orientation() {
        let text = "e\u{301}👨‍👩‍👧‍👦A";
        let frame = frame_with_run(text, vertical_presentation(RichTextWritingMode::VerticalRl));
        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 3);
        assert_eq!(layout.glyphs[0].text, "e\u{301}");
        assert_eq!(
            layout.glyphs[0].range,
            RichTextRange::new(0, "e\u{301}".len())
        );
        assert_eq!(layout.glyphs[0].orientation, GlyphOrientation::SidewaysCw);
        assert_eq!(layout.glyphs[1].text, "👨‍👩‍👧‍👦");
        assert_eq!(layout.glyphs[1].orientation, GlyphOrientation::Upright);
        assert_eq!(layout.glyphs[2].text, "A");
        assert_eq!(layout.glyphs[2].orientation, GlyphOrientation::SidewaysCw);
    }

    #[test]
    fn vertical_mixed_orientation_uses_unicode_vertical_orientation_data() {
        let frame = frame_with_run(
            "AＡ。ー",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        assert_eq!(UNICODE_VERTICAL_ORIENTATION_VERSION, "17.0.0");
        assert_eq!(layout.glyphs.len(), 4);
        assert_eq!(layout.glyphs[0].text, "A");
        assert_eq!(layout.glyphs[0].orientation, GlyphOrientation::SidewaysCw);
        assert_eq!(layout.glyphs[1].text, "Ａ");
        assert_eq!(layout.glyphs[1].orientation, GlyphOrientation::Upright);
        assert_eq!(layout.glyphs[1].vertical_form, GlyphVerticalForm::None);
        assert_eq!(layout.glyphs[2].text, "。");
        assert_eq!(layout.glyphs[2].orientation, GlyphOrientation::Upright);
        assert_eq!(
            layout.glyphs[2].vertical_form,
            GlyphVerticalForm::UprightAlternate
        );
        assert_eq!(layout.glyphs[3].text, "ー");
        assert_eq!(layout.glyphs[3].orientation, GlyphOrientation::SidewaysCw);
        assert_eq!(
            layout.glyphs[3].vertical_form,
            GlyphVerticalForm::RotatedAlternate
        );
    }

    #[test]
    fn vertical_text_combine_uses_at_most_four_ascii_digits() {
        let frame = frame_with_run(
            "20265",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 2);
        assert_eq!(layout.glyphs[0].text, "2026");
        assert_eq!(
            layout.glyphs[0].orientation,
            GlyphOrientation::TextCombineUpright
        );
        assert_eq!(layout.glyphs[1].text, "5");
        assert_eq!(layout.glyphs[1].orientation, GlyphOrientation::SidewaysCw);
        assert_eq!(layout.glyphs[0].vertical_form, GlyphVerticalForm::None);
    }

    #[test]
    fn vertical_crlf_advances_to_next_column_without_emitting_glyph() {
        let frame = frame_with_run(
            "天\r\n地",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 2);
        assert_eq!(layout.glyphs[0].text, "天");
        assert_eq!(layout.glyphs[1].text, "地");
        assert!(layout.glyphs[1].origin.x < layout.glyphs[0].origin.x);
        assert_f32_eq(layout.glyphs[1].origin.y, layout.glyphs[0].origin.y);
    }

    #[test]
    fn vertical_column_breaks_use_uax14_opportunities() {
        let frame = frame_with_run(
            "天地。人",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 4);
        assert_eq!(layout.glyphs[2].text, "。");
        assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
        assert_f32_eq(
            layout.glyphs[2].origin.y,
            config.origin.y + config.size.height - config.line_advance * 0.5,
        );
        assert_f32_eq(
            layout.glyphs[2].bounds.bottom(),
            config.origin.y + config.size.height + config.line_advance * 0.5,
        );
        assert!(
            layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
            "closing punctuation may hang past the current column instead of violating kinsoku"
        );
        assert_eq!(layout.glyphs[3].text, "人");
        assert!(
            layout.glyphs[3].origin.x < layout.glyphs[2].origin.x,
            "the next breakable cluster should start the next vertical_rl column"
        );
        assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
    }

    #[test]
    fn vertical_column_plan_records_jlreq_break_decisions_before_placement() {
        let frame = frame_with_run(
            "天地。人",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let clusters = vertical_clusters(&frame.text, RichTextVerticalLatinMode::Mixed);
        let context = RunLayoutContext {
            run_index: 0,
            range_start: 0,
            presentation: &frame.display_map.text_runs[0].presentation,
            ruby_annotations: &frame.display_map.ruby_annotations,
            config,
        };
        let plan = plan_vertical_columns(
            &clusters,
            context,
            LayoutCursor::new(
                vertical_column_start(RichTextWritingMode::VerticalRl, config),
                config.origin.y,
            ),
        );

        assert_eq!(plan.break_before, vec![false, false, false, true]);
    }

    #[test]
    fn vertical_column_plan_pushes_line_end_prohibited_opening_punctuation() {
        let frame = frame_with_run(
            "天（地",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let clusters = vertical_clusters(&frame.text, RichTextVerticalLatinMode::Mixed);
        let context = RunLayoutContext {
            run_index: 0,
            range_start: 0,
            presentation: &frame.display_map.text_runs[0].presentation,
            ruby_annotations: &frame.display_map.ruby_annotations,
            config,
        };
        let plan = plan_vertical_columns(
            &clusters,
            context,
            LayoutCursor::new(
                vertical_column_start(RichTextWritingMode::VerticalRl, config),
                config.origin.y,
            ),
        );

        assert_eq!(plan.break_before, vec![false, true, false]);
    }

    #[test]
    fn vertical_column_keeps_vertical_presentation_bracket_pair_together() {
        for (writing_mode, next_column_moves_right) in [
            (RichTextWritingMode::VerticalRl, false),
            (RichTextWritingMode::VerticalLr, true),
        ] {
            let frame = frame_with_run("天︵︶人", vertical_presentation(writing_mode));
            let config = TextLayoutConfig {
                size: LayoutSize::new(160.0, 84.0),
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");
            let opening = nth_laid_out_glyph(&layout, "︵", 0);
            let closing = nth_laid_out_glyph(&layout, "︶", 0);
            let person = nth_laid_out_glyph(&layout, "人", 0);

            assert_vertical_layout_after(
                &layout.glyphs[0],
                opening,
                "vertical presentation opening bracket should sit after the previous cluster",
            );
            assert_vertical_layout_after(
                opening,
                closing,
                "vertical presentation compact bracket pair should stay together",
            );
            assert!(
                closing.bounds.bottom() > config.origin.y + config.size.height,
                "vertical presentation compact bracket pair may overhang as one suffix"
            );
            assert_next_vertical_layout_column(
                closing,
                person,
                next_column_moves_right,
                "ordinary text after vertical presentation compact bracket pair should start the next column",
            );
        }
    }

    #[test]
    fn vertical_column_plan_balances_paragraph_with_dp_cost() {
        let frame = frame_with_run(
            "天地玄黄宇宙",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 168.0),
            ..TextLayoutConfig::default()
        };
        let clusters = vertical_clusters(&frame.text, RichTextVerticalLatinMode::Mixed);
        let context = RunLayoutContext {
            run_index: 0,
            range_start: 0,
            presentation: &frame.display_map.text_runs[0].presentation,
            ruby_annotations: &frame.display_map.ruby_annotations,
            config,
        };
        let plan = plan_vertical_columns(
            &clusters,
            context,
            LayoutCursor::new(
                vertical_column_start(RichTextWritingMode::VerticalRl, config),
                config.origin.y,
            ),
        );

        assert_eq!(
            plan.break_before,
            vec![false, false, false, true, false, false]
        );
    }

    #[test]
    fn vertical_paragraph_plan_combines_published_jlreq_line_composition_classes() {
        // W3C JLREQ 3.1 groups these as line-head/line-end and
        // separation-prohibited punctuation classes; keep them together in one
        // paragraph plan instead of only proving isolated two-cluster cases.
        let text = "天地春夏秋冬月火、山々人「川」あっいおーえ―中・外………終";
        for (writing_mode, next_column_moves_right) in [
            (RichTextWritingMode::VerticalRl, false),
            (RichTextWritingMode::VerticalLr, true),
        ] {
            let frame = frame_with_run(text, vertical_presentation(writing_mode));
            let config = TextLayoutConfig {
                size: LayoutSize::new(210.0, 168.0),
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");

            assert!(
                vertical_layout_column_count(&layout) >= 7,
                "{writing_mode:?} JLREQ paragraph should require a multi-column plan: {layout:?}"
            );

            let fire = nth_laid_out_glyph(&layout, "火", 0);
            let comma = nth_laid_out_glyph(&layout, "、", 0);
            let mountain = nth_laid_out_glyph(&layout, "山", 0);
            assert_vertical_layout_after(fire, comma, "comma should follow body text");
            assert_f32_eq(comma.advance.height, config.line_advance * 0.5);
            assert_next_vertical_layout_column(
                comma,
                mountain,
                next_column_moves_right,
                "text after a column-end comma should continue in the next paragraph column",
            );

            let iteration = nth_laid_out_glyph(&layout, "々", 0);
            let person = nth_laid_out_glyph(&layout, "人", 0);
            assert_vertical_layout_after(
                mountain,
                iteration,
                "iteration mark should stay with the previous cluster in paragraph context",
            );
            assert_vertical_layout_after(
                iteration,
                person,
                "text after an iteration mark should continue in the same paragraph column when it fits",
            );

            let open = nth_laid_out_glyph(&layout, "「", 0);
            let river = nth_laid_out_glyph(&layout, "川", 0);
            let close = nth_laid_out_glyph(&layout, "」", 0);
            assert_vertical_layout_after(
                open,
                river,
                "opening bracket should not strand its base text",
            );
            assert_vertical_layout_after(
                river,
                close,
                "closing bracket should stay with its base text",
            );

            let large_kana = nth_laid_out_glyph(&layout, "あ", 0);
            let small_kana = nth_laid_out_glyph(&layout, "っ", 0);
            let next_kana = nth_laid_out_glyph(&layout, "い", 0);
            assert_vertical_layout_after(
                large_kana,
                small_kana,
                "small kana should stay out of a paragraph column head",
            );
            assert_next_vertical_layout_column(
                small_kana,
                next_kana,
                next_column_moves_right,
                "text after an overhanging small kana should continue in the next paragraph column",
            );

            assert_vertical_paragraph_dash_suffix(&layout, next_column_moves_right);

            let middle_dot = nth_laid_out_glyph(&layout, "・", 0);
            let outside = nth_laid_out_glyph(&layout, "外", 0);
            assert_same_vertical_layout_column(
                middle_dot,
                outside,
                "middle-dot compression should keep following paragraph text in the same column",
            );
            assert!(outside.origin.y > middle_dot.origin.y);

            let first_leader = nth_laid_out_glyph(&layout, "…", 0);
            let second_leader = nth_laid_out_glyph(&layout, "…", 1);
            let third_leader = nth_laid_out_glyph(&layout, "…", 2);
            let ending = nth_laid_out_glyph(&layout, "終", 0);
            assert_vertical_layout_after(
                first_leader,
                second_leader,
                "repeated leaders should stay together in paragraph context",
            );
            assert_vertical_layout_after(
                second_leader,
                third_leader,
                "the full leader chain should stay together in paragraph context",
            );
            assert!(
                third_leader.bounds.bottom() > config.origin.y + config.size.height,
                "leader chain should overhang as one paragraph suffix: {third_leader:?}"
            );
            assert_next_vertical_layout_column(
                third_leader,
                ending,
                next_column_moves_right,
                "text after an overhanging leader chain should continue in the next paragraph column",
            );
        }
    }

    #[test]
    fn vertical_paragraph_plan_keeps_strict_closing_opening_pair_inside_class_mix() {
        let text = "天地春夏秋冬月火、山々人。「川」あっいおーえ―中・外………終";
        for (writing_mode, next_column_moves_right) in [
            (RichTextWritingMode::VerticalRl, false),
            (RichTextWritingMode::VerticalLr, true),
        ] {
            let frame = frame_with_run(text, vertical_presentation(writing_mode));
            let loose_config = TextLayoutConfig {
                size: LayoutSize::new(210.0, 147.0),
                jlreq_strictness: JlreqStrictness::Loose,
                ..TextLayoutConfig::default()
            };
            let strict_config = TextLayoutConfig {
                jlreq_strictness: JlreqStrictness::Strict,
                ..loose_config
            };

            let loose = layout_frame(&frame, loose_config).expect("loose layout succeeds");
            let loose_full_stop = nth_laid_out_glyph(&loose, "。", 0);
            let loose_open = nth_laid_out_glyph(&loose, "「", 0);
            assert_next_vertical_layout_column(
                loose_full_stop,
                loose_open,
                next_column_moves_right,
                "loose paragraph class mix may break between closing and opening punctuation",
            );

            let strict = layout_frame(&frame, strict_config).expect("strict layout succeeds");
            assert!(
                vertical_layout_column_count(&strict) >= 7,
                "{writing_mode:?} strict JLREQ paragraph should still require a multi-column plan: {strict:?}"
            );
            let person = nth_laid_out_glyph(&strict, "人", 0);
            let strict_full_stop = nth_laid_out_glyph(&strict, "。", 0);
            let strict_open = nth_laid_out_glyph(&strict, "「", 0);
            let river = nth_laid_out_glyph(&strict, "川", 0);
            let close = nth_laid_out_glyph(&strict, "」", 0);
            assert_vertical_layout_after(
                person,
                strict_full_stop,
                "strict paragraph class mix should keep closing punctuation after its base",
            );
            assert_vertical_layout_after(
                strict_full_stop,
                strict_open,
                "strict paragraph class mix should keep adjacent closing/opening punctuation together",
            );
            assert_vertical_layout_after(
                strict_open,
                river,
                "strict opening punctuation should not strand its following base",
            );
            assert_vertical_layout_after(
                river,
                close,
                "strict paragraph class mix should keep closing bracket with its base",
            );
        }
    }

    #[test]
    fn vertical_hard_line_break_resets_strict_jlreq_paragraph_segment() {
        for (writing_mode, next_column_moves_right) in [
            (RichTextWritingMode::VerticalRl, false),
            (RichTextWritingMode::VerticalLr, true),
        ] {
            let frame = frame_with_run("天地。\n「人外", vertical_presentation(writing_mode));
            let config = TextLayoutConfig {
                size: LayoutSize::new(210.0, 147.0),
                jlreq_strictness: JlreqStrictness::Strict,
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");
            let full_stop = nth_laid_out_glyph(&layout, "。", 0);
            let opening = nth_laid_out_glyph(&layout, "「", 0);
            let person = nth_laid_out_glyph(&layout, "人", 0);

            assert_next_vertical_layout_column(
                full_stop,
                opening,
                next_column_moves_right,
                "explicit hard line break should start a new strict JLREQ paragraph segment",
            );
            assert_vertical_layout_after(
                opening,
                person,
                "text after the hard-break opening punctuation should stay in its new segment column",
            );
            assert_f32_eq(opening.origin.y, config.origin.y);
        }
    }

    fn assert_vertical_paragraph_dash_suffix(layout: &LaidOutText, next_column_moves_right: bool) {
        let syllable = nth_laid_out_glyph(layout, "え", 0);
        let dash = nth_laid_out_glyph(layout, "―", 0);
        let center = nth_laid_out_glyph(layout, "中", 0);
        assert_vertical_layout_after(
            syllable,
            dash,
            "dash mark should stay with the previous paragraph cluster",
        );
        assert_next_vertical_layout_column(
            dash,
            center,
            next_column_moves_right,
            "text after an overhanging dash-mark suffix should continue in the next paragraph column",
        );
    }

    #[test]
    fn vertical_column_plan_reads_generated_pair_break_penalty() {
        let clusters = vertical_clusters("天地。「人", RichTextVerticalLatinMode::Mixed);
        let penalty = vertical_column_pair_break_penalty(&clusters, 0, 3, JlreqStrictness::Normal);

        assert_f32_eq(penalty, 25.0);
    }

    #[test]
    fn vertical_column_pair_break_penalty_uses_jlreq_strictness() {
        let clusters = vertical_clusters("天地。「人", RichTextVerticalLatinMode::Mixed);

        assert_f32_eq(
            vertical_column_pair_break_penalty(&clusters, 0, 3, JlreqStrictness::Loose),
            5.0,
        );
        assert_f32_eq(
            vertical_column_pair_break_penalty(&clusters, 0, 3, JlreqStrictness::Strict),
            100.0,
        );
    }

    #[test]
    fn vertical_column_plan_applies_closing_opening_penalty_to_paragraph_dp() {
        let text = "天地。「人山川海";
        let presentation = vertical_presentation(RichTextWritingMode::VerticalRl);
        let frame = frame_with_run(text, presentation);
        let clusters = vertical_clusters(text, RichTextVerticalLatinMode::Mixed);
        let loose_config = TextLayoutConfig {
            size: LayoutSize::new(260.0, 147.0),
            jlreq_strictness: JlreqStrictness::Loose,
            ..TextLayoutConfig::default()
        };
        let strict_config = TextLayoutConfig {
            jlreq_strictness: JlreqStrictness::Strict,
            ..loose_config
        };
        let loose_context = RunLayoutContext {
            run_index: 0,
            range_start: 0,
            presentation: &frame.display_map.text_runs[0].presentation,
            ruby_annotations: &frame.display_map.ruby_annotations,
            config: loose_config,
        };
        let strict_context = RunLayoutContext {
            config: strict_config,
            ..loose_context
        };
        let start = LayoutCursor::new(
            vertical_column_start(RichTextWritingMode::VerticalRl, loose_config),
            loose_config.origin.y,
        );

        let loose_plan = plan_vertical_columns(&clusters, loose_context, start);
        let strict_plan = plan_vertical_columns(&clusters, strict_context, start);

        assert_eq!(
            loose_plan.break_before,
            vec![false, false, false, true, false, false, true, false],
            "loose composition may break between adjacent closing and opening punctuation"
        );
        assert_eq!(
            strict_plan.break_before,
            vec![false, true, false, false, false, true, false, false],
            "strict composition should choose a different paragraph plan to avoid the weak closing/opening break"
        );

        let loose_layout = layout_frame(&frame, loose_config).expect("loose layout succeeds");
        let loose_full_stop = nth_laid_out_glyph(&loose_layout, "。", 0);
        let loose_open = nth_laid_out_glyph(&loose_layout, "「", 0);
        assert_next_vertical_layout_column(
            loose_full_stop,
            loose_open,
            false,
            "loose layout should expose the weaker closing/opening break in geometry",
        );

        let strict_layout = layout_frame(&frame, strict_config).expect("strict layout succeeds");
        let strict_full_stop = nth_laid_out_glyph(&strict_layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&strict_layout, "「", 0);
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict layout should keep adjacent closing/opening punctuation in one column",
        );
    }

    #[test]
    fn vertical_column_plan_applies_middle_dot_opening_strict_pair_to_paragraph_dp() {
        let text = "天地・「人山川海";
        let presentation = vertical_presentation(RichTextWritingMode::VerticalRl);
        let frame = frame_with_run(text, presentation);
        let clusters = vertical_clusters(text, RichTextVerticalLatinMode::Mixed);
        let loose_config = TextLayoutConfig {
            size: LayoutSize::new(260.0, 147.0),
            jlreq_strictness: JlreqStrictness::Loose,
            ..TextLayoutConfig::default()
        };
        let strict_config = TextLayoutConfig {
            jlreq_strictness: JlreqStrictness::Strict,
            ..loose_config
        };
        let loose_context = RunLayoutContext {
            run_index: 0,
            range_start: 0,
            presentation: &frame.display_map.text_runs[0].presentation,
            ruby_annotations: &frame.display_map.ruby_annotations,
            config: loose_config,
        };
        let strict_context = RunLayoutContext {
            config: strict_config,
            ..loose_context
        };
        let start = LayoutCursor::new(
            vertical_column_start(RichTextWritingMode::VerticalRl, loose_config),
            loose_config.origin.y,
        );

        let loose_plan = plan_vertical_columns(&clusters, loose_context, start);
        let strict_plan = plan_vertical_columns(&clusters, strict_context, start);

        assert_eq!(
            loose_plan.break_before,
            vec![false, false, false, true, false, false, true, false],
            "loose composition may break between a middle dot and opening punctuation"
        );
        assert_eq!(
            strict_plan.break_before,
            vec![false, true, false, false, false, true, false, false],
            "strict composition should choose a different paragraph plan to keep the middle-dot/opening pair together"
        );

        let loose_layout = layout_frame(&frame, loose_config).expect("loose layout succeeds");
        let loose_middle_dot = nth_laid_out_glyph(&loose_layout, "・", 0);
        let loose_open = nth_laid_out_glyph(&loose_layout, "「", 0);
        assert_next_vertical_layout_column(
            loose_middle_dot,
            loose_open,
            false,
            "loose layout should expose the middle-dot/opening weak break in geometry",
        );

        let strict_layout = layout_frame(&frame, strict_config).expect("strict layout succeeds");
        let strict_middle_dot = nth_laid_out_glyph(&strict_layout, "・", 0);
        let strict_open = nth_laid_out_glyph(&strict_layout, "「", 0);
        assert_vertical_layout_after(
            strict_middle_dot,
            strict_open,
            "strict layout should keep middle-dot/opening punctuation in one column",
        );
    }

    #[test]
    fn vertical_column_pair_break_penalty_reads_expanded_jlreq_pairs() {
        let leader_clusters = vertical_clusters("天…人", RichTextVerticalLatinMode::Mixed);
        assert_f32_eq(
            vertical_column_pair_break_penalty(&leader_clusters, 0, 1, JlreqStrictness::Loose),
            50.0,
        );
        assert_f32_eq(
            vertical_column_pair_break_penalty(&leader_clusters, 0, 1, JlreqStrictness::Normal),
            500.0,
        );

        let middle_dot_clusters = vertical_clusters("天・人", RichTextVerticalLatinMode::Mixed);
        assert_f32_eq(
            vertical_column_pair_break_penalty(&middle_dot_clusters, 0, 1, JlreqStrictness::Strict),
            1000.0,
        );

        let middle_dot_open_clusters =
            vertical_clusters("天・「人", RichTextVerticalLatinMode::Mixed);
        assert_f32_eq(
            vertical_column_pair_break_penalty(
                &middle_dot_open_clusters,
                0,
                2,
                JlreqStrictness::Loose,
            ),
            0.0,
        );
        assert_f32_eq(
            vertical_column_pair_break_penalty(
                &middle_dot_open_clusters,
                0,
                2,
                JlreqStrictness::Normal,
            ),
            15.0,
        );
        assert_f32_eq(
            vertical_column_pair_break_penalty(
                &middle_dot_open_clusters,
                0,
                2,
                JlreqStrictness::Strict,
            ),
            1000.0,
        );

        let bracket_clusters = vertical_clusters("「」人", RichTextVerticalLatinMode::Mixed);
        assert_f32_eq(
            vertical_column_pair_break_penalty(&bracket_clusters, 0, 1, JlreqStrictness::Normal),
            1000.0,
        );
    }

    #[test]
    fn rich_text_layout_jlreq_strictness_overrides_host_config_when_explicit() {
        let mut presentation = vertical_presentation(RichTextWritingMode::VerticalRl);
        presentation
            .layout
            .as_mut()
            .expect("vertical presentation has layout")
            .jlreq_strictness = RichTextJlreqStrictness::Strict;
        let config = TextLayoutConfig {
            jlreq_strictness: JlreqStrictness::Loose,
            ..TextLayoutConfig::default()
        };

        let resolved = text_layout_config_for_presentation(config, &presentation);

        assert_eq!(resolved.jlreq_strictness, JlreqStrictness::Strict);
    }

    #[test]
    fn rich_text_layout_jlreq_auto_inherits_host_config() {
        let presentation = vertical_presentation(RichTextWritingMode::VerticalRl);
        let config = TextLayoutConfig {
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };

        let resolved = text_layout_config_for_presentation(config, &presentation);

        assert_eq!(resolved.jlreq_strictness, JlreqStrictness::Strict);
    }

    #[test]
    fn vertical_hanging_punctuation_limits_column_overhang_to_half_cell() {
        let frame = frame_with_run(
            "天地、人人",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        let column_end = config.origin.y + config.size.height;

        assert_eq!(layout.glyphs.len(), 5);
        assert_eq!(layout.glyphs[2].text, "、");
        assert_f32_eq(layout.glyphs[2].advance.height, config.line_advance * 0.5);
        assert_f32_eq(
            layout.glyphs[2].origin.y,
            column_end - config.line_advance * 0.5,
        );
        assert_f32_eq(
            layout.glyphs[2].bounds.bottom(),
            column_end + config.line_advance * 0.5,
        );
        assert_eq!(layout.glyphs[3].text, "人");
        assert!(
            layout.glyphs[3].origin.x < layout.glyphs[2].origin.x,
            "ordinary text after hanging punctuation should start the next column"
        );
        assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
    }

    #[test]
    fn vertical_fullwidth_and_halfwidth_closing_punctuation_hangs() {
        for (mark, label) in [
            ("？", "fullwidth question mark"),
            ("｡", "halfwidth full stop"),
        ] {
            for (writing_mode, next_column_moves_right) in [
                (RichTextWritingMode::VerticalRl, false),
                (RichTextWritingMode::VerticalLr, true),
            ] {
                let frame = frame_with_run(
                    &format!("天地{mark}人"),
                    vertical_presentation(writing_mode),
                );
                let config = TextLayoutConfig {
                    size: LayoutSize::new(160.0, 84.0),
                    ..TextLayoutConfig::default()
                };
                let layout = layout_frame(&frame, config).expect("layout succeeds");
                let punctuation = nth_laid_out_glyph(&layout, mark, 0);
                let person = nth_laid_out_glyph(&layout, "人", 0);

                assert_eq!(punctuation.text, mark);
                assert_f32_eq(punctuation.advance.height, config.line_advance * 0.5);
                assert_f32_eq(punctuation.origin.x, layout.glyphs[1].origin.x);
                assert!(
                    punctuation.bounds.bottom() > config.origin.y + config.size.height,
                    "{label} should hang past the {writing_mode:?} column end"
                );
                assert_next_vertical_layout_column(
                    punctuation,
                    person,
                    next_column_moves_right,
                    "ordinary text after closing punctuation should start the next column",
                );
            }
        }
    }

    #[test]
    fn vertical_punctuation_compression_keeps_following_text_in_column() {
        let frame = frame_with_run(
            "天、。人",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 126.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 4);
        assert_eq!(layout.glyphs[1].text, "、");
        assert_eq!(layout.glyphs[2].text, "。");
        assert_f32_eq(layout.glyphs[1].advance.height, config.line_advance * 0.5);
        assert_f32_eq(layout.glyphs[2].advance.height, config.line_advance * 0.5);
        assert_f32_eq(layout.glyphs[1].bounds.height, config.line_advance);
        assert_f32_eq(layout.glyphs[2].bounds.height, config.line_advance);
        assert_eq!(layout.glyphs[3].text, "人");
        assert_f32_eq(layout.glyphs[3].origin.x, layout.glyphs[0].origin.x);
        assert!(
            layout.glyphs[3].origin.y < config.origin.y + config.size.height,
            "compressed punctuation should leave room for the following cluster"
        );
    }

    #[test]
    fn vertical_consecutive_punctuation_compression_uses_half_cell_advances() {
        for writing_mode in [
            RichTextWritingMode::VerticalRl,
            RichTextWritingMode::VerticalLr,
        ] {
            let frame = frame_with_run("天、。・人", vertical_presentation(writing_mode));
            let config = TextLayoutConfig {
                size: LayoutSize::new(160.0, 168.0),
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");
            let body = nth_laid_out_glyph(&layout, "天", 0);
            let comma = nth_laid_out_glyph(&layout, "、", 0);
            let period = nth_laid_out_glyph(&layout, "。", 0);
            let middle_dot = nth_laid_out_glyph(&layout, "・", 0);
            let person = nth_laid_out_glyph(&layout, "人", 0);

            assert_same_vertical_layout_column(
                body,
                person,
                "consecutive compressed punctuation should leave the following text in the same column",
            );
            for punctuation in [comma, period, middle_dot] {
                assert_f32_eq(punctuation.advance.height, config.line_advance * 0.5);
                assert_f32_eq(punctuation.bounds.height, config.line_advance);
            }
            assert_vertical_layout_after(body, comma, "comma should follow body text");
            assert_vertical_layout_after(comma, period, "full stop should follow comma");
            assert_vertical_layout_after(period, middle_dot, "middle dot should follow full stop");
            assert_vertical_layout_after(
                middle_dot,
                person,
                "body text should follow the compressed punctuation chain",
            );
        }
    }

    #[test]
    fn vertical_column_keeps_small_kana_out_of_column_heads() {
        let frame = frame_with_run(
            "天地ぁ人",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 4);
        assert_eq!(layout.glyphs[2].text, "ぁ");
        assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
        assert!(
            layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
            "small kana may overhang the current column instead of starting the next column"
        );
        assert_eq!(layout.glyphs[3].text, "人");
        assert!(
            layout.glyphs[3].origin.x < layout.glyphs[2].origin.x,
            "the next ordinary cluster should start the next vertical_rl column"
        );
        assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
    }

    #[test]
    fn vertical_column_keeps_jlreq_leaders_together() {
        let frame = frame_with_run(
            "天……人",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 4);
        assert_eq!(layout.glyphs[1].text, "…");
        assert_eq!(layout.glyphs[2].text, "…");
        assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
        assert!(
            layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
            "the second leader mark should stay with the first instead of starting a new column"
        );
        assert_eq!(layout.glyphs[3].text, "人");
        assert!(layout.glyphs[3].origin.x < layout.glyphs[2].origin.x);
        assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
    }

    #[test]
    fn vertical_column_keeps_vertical_presentation_leaders_together() {
        for (writing_mode, next_column_moves_right) in [
            (RichTextWritingMode::VerticalRl, false),
            (RichTextWritingMode::VerticalLr, true),
        ] {
            let frame = frame_with_run("天︙︙人", vertical_presentation(writing_mode));
            let config = TextLayoutConfig {
                size: LayoutSize::new(160.0, 84.0),
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");

            assert_eq!(layout.glyphs.len(), 4);
            assert_eq!(layout.glyphs[1].text, "︙");
            assert_eq!(layout.glyphs[2].text, "︙");
            assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
            assert!(
                layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
                "second vertical presentation leader should stay with the first"
            );
            assert_eq!(layout.glyphs[3].text, "人");
            assert_next_vertical_layout_column(
                &layout.glyphs[2],
                &layout.glyphs[3],
                next_column_moves_right,
                "ordinary text after vertical presentation leaders should start the next column",
            );
        }
    }

    #[test]
    fn vertical_column_keeps_jlreq_leader_chain_together_before_next_column() {
        let frame = frame_with_run(
            "天………人",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let clusters = vertical_clusters(&frame.text, RichTextVerticalLatinMode::Mixed);
        let context = RunLayoutContext {
            run_index: 0,
            range_start: 0,
            presentation: &frame.display_map.text_runs[0].presentation,
            ruby_annotations: &frame.display_map.ruby_annotations,
            config,
        };
        let plan = plan_vertical_columns(
            &clusters,
            context,
            LayoutCursor::new(
                vertical_column_start(RichTextWritingMode::VerticalRl, config),
                config.origin.y,
            ),
        );
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(plan.break_before, vec![false, false, false, false, true]);
        assert_eq!(layout.glyphs.len(), 5);
        assert_eq!(layout.glyphs[1].text, "…");
        assert_eq!(layout.glyphs[2].text, "…");
        assert_eq!(layout.glyphs[3].text, "…");
        assert_f32_eq(layout.glyphs[1].origin.x, layout.glyphs[0].origin.x);
        assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[0].origin.x);
        assert_f32_eq(layout.glyphs[3].origin.x, layout.glyphs[0].origin.x);
        assert!(
            layout.glyphs[3].bounds.bottom() > config.origin.y + config.size.height,
            "the leader chain may overhang as one unbreakable trailing suffix"
        );
        assert_eq!(layout.glyphs[4].text, "人");
        assert!(
            layout.glyphs[4].origin.x < layout.glyphs[3].origin.x,
            "ordinary text after the leader chain should start the next vertical_rl column"
        );
        assert_f32_eq(layout.glyphs[4].origin.y, config.origin.y);
    }

    #[test]
    fn vertical_lr_column_keeps_jlreq_leader_chain_together_before_next_column() {
        let frame = frame_with_run(
            "天………人",
            vertical_presentation(RichTextWritingMode::VerticalLr),
        );
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let clusters = vertical_clusters(&frame.text, RichTextVerticalLatinMode::Mixed);
        let context = RunLayoutContext {
            run_index: 0,
            range_start: 0,
            presentation: &frame.display_map.text_runs[0].presentation,
            ruby_annotations: &frame.display_map.ruby_annotations,
            config,
        };
        let plan = plan_vertical_columns(
            &clusters,
            context,
            LayoutCursor::new(
                vertical_column_start(RichTextWritingMode::VerticalLr, config),
                config.origin.y,
            ),
        );
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(plan.break_before, vec![false, false, false, false, true]);
        assert_eq!(layout.glyphs.len(), 5);
        assert_eq!(layout.glyphs[1].text, "…");
        assert_eq!(layout.glyphs[2].text, "…");
        assert_eq!(layout.glyphs[3].text, "…");
        assert_f32_eq(layout.glyphs[1].origin.x, layout.glyphs[0].origin.x);
        assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[0].origin.x);
        assert_f32_eq(layout.glyphs[3].origin.x, layout.glyphs[0].origin.x);
        assert!(
            layout.glyphs[3].bounds.bottom() > config.origin.y + config.size.height,
            "the vertical_lr leader chain may overhang as one unbreakable trailing suffix"
        );
        assert_eq!(layout.glyphs[4].text, "人");
        assert!(
            layout.glyphs[4].origin.x > layout.glyphs[3].origin.x,
            "ordinary text after the leader chain should start the next vertical_lr column"
        );
        assert_f32_eq(layout.glyphs[4].origin.y, config.origin.y);
    }

    #[test]
    fn vertical_column_keeps_jlreq_dashes_together() {
        let frame = frame_with_run(
            "天――人",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 4);
        assert_eq!(layout.glyphs[1].text, "―");
        assert_eq!(layout.glyphs[2].text, "―");
        assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
        assert!(
            layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
            "the second dash should stay with the first instead of starting a new column"
        );
        assert_eq!(layout.glyphs[3].text, "人");
        assert!(layout.glyphs[3].origin.x < layout.glyphs[2].origin.x);
        assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
    }

    #[test]
    fn vertical_lr_column_keeps_jlreq_dashes_together() {
        let frame = frame_with_run(
            "天――人",
            vertical_presentation(RichTextWritingMode::VerticalLr),
        );
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 4);
        assert_eq!(layout.glyphs[1].text, "―");
        assert_eq!(layout.glyphs[2].text, "―");
        assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
        assert!(
            layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
            "the second vertical_lr dash should stay with the first instead of starting a new column"
        );
        assert_eq!(layout.glyphs[3].text, "人");
        assert!(layout.glyphs[3].origin.x > layout.glyphs[2].origin.x);
        assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
    }

    #[test]
    fn vertical_column_keeps_prolonged_sound_mark_out_of_column_heads() {
        let frame = frame_with_run(
            "天地ー人",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 4);
        assert_eq!(layout.glyphs[2].text, "ー");
        assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
        assert!(
            layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
            "prolonged sound marks should overhang instead of starting the next column"
        );
        assert_eq!(layout.glyphs[3].text, "人");
        assert!(layout.glyphs[3].origin.x < layout.glyphs[2].origin.x);
        assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
    }

    #[test]
    fn vertical_column_keeps_middle_dot_out_of_column_heads() {
        let frame = frame_with_run(
            "天地・人",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 4);
        assert_eq!(layout.glyphs[2].text, "・");
        assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
        assert!(
            layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
            "middle dots should overhang instead of starting the next column"
        );
        assert_eq!(layout.glyphs[3].text, "人");
        assert!(layout.glyphs[3].origin.x < layout.glyphs[2].origin.x);
        assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
    }

    #[test]
    fn vertical_column_keeps_jlreq_iteration_marks_with_previous_cluster() {
        let frame = frame_with_run(
            "天地々人",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 4);
        assert_eq!(layout.glyphs[2].text, "々");
        assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
        assert!(
            layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
            "iteration marks should stay with the previous cluster instead of starting a new column"
        );
        assert_eq!(layout.glyphs[3].text, "人");
        assert!(layout.glyphs[3].origin.x < layout.glyphs[2].origin.x);
        assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
    }

    #[test]
    fn vertical_lr_column_keeps_small_kana_out_of_column_heads() {
        assert_vertical_lr_no_column_head_mark("天地ぁ人", "ぁ");
    }

    #[test]
    fn vertical_lr_column_keeps_prolonged_sound_mark_out_of_column_heads() {
        assert_vertical_lr_no_column_head_mark("天地ー人", "ー");
    }

    #[test]
    fn vertical_lr_column_keeps_middle_dot_out_of_column_heads() {
        assert_vertical_lr_no_column_head_mark("天地・人", "・");
    }

    #[test]
    fn vertical_lr_column_keeps_jlreq_iteration_marks_with_previous_cluster() {
        assert_vertical_lr_no_column_head_mark("天地々人", "々");
    }

    fn assert_vertical_lr_no_column_head_mark(text: &str, mark: &str) {
        let frame = frame_with_run(text, vertical_presentation(RichTextWritingMode::VerticalLr));
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 4);
        assert_eq!(layout.glyphs[2].text, mark);
        assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
        assert!(
            layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
            "{mark} should overhang the current vertical_lr column instead of starting the next column"
        );
        assert_eq!(layout.glyphs[3].text, "人");
        assert!(
            layout.glyphs[3].origin.x > layout.glyphs[2].origin.x,
            "the next ordinary cluster should start the next vertical_lr column"
        );
        assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
    }

    #[test]
    fn vertical_column_breaks_before_jlreq_line_end_prohibited_opening_punctuation() {
        let frame = frame_with_run(
            "天（地",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 3);
        assert_eq!(layout.glyphs[1].text, "（");
        assert!(
            layout.glyphs[1].origin.x < layout.glyphs[0].origin.x,
            "opening punctuation should not remain at the previous column end"
        );
        assert_f32_eq(layout.glyphs[1].origin.y, config.origin.y);
        assert_eq!(layout.glyphs[2].text, "地");
        assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
        assert!(layout.glyphs[2].origin.y > layout.glyphs[1].origin.y);
    }

    #[test]
    fn ruby_uses_base_geometry() {
        let mut frame = frame_with_run("夢", RichTextPresentation::default());
        frame
            .display_map
            .ruby_annotations
            .push(RichTextRubyAnnotation {
                base_range: RichTextRange::new(0, "夢".len()),
                ruby: "ゆめ".to_owned(),
                node_index: 0,
                styles: Vec::new(),
                presentation: RichTextPresentation::default(),
            });
        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        assert_eq!(layout.ruby.len(), 1);
        assert_eq!(layout.ruby[0].base_bounds, layout.glyphs[0].bounds);
        assert!(layout.ruby[0].ruby_bounds.y < layout.glyphs[0].bounds.y);
    }

    #[test]
    fn horizontal_ruby_collision_shifts_adjacent_annotations() {
        let mut frame = frame_with_run("夢星", RichTextPresentation::default());
        push_ruby(&mut frame, 0, "夢".len(), "ながいよみ");
        push_ruby(&mut frame, "夢".len(), "夢星".len(), "ながいよみ");
        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        assert_eq!(layout.ruby.len(), 2);
        assert_eq!(
            layout.ruby[0].writing_mode,
            RichTextWritingMode::HorizontalTb
        );
        assert!(
            !layout.ruby[0]
                .ruby_bounds
                .intersects(layout.ruby[1].ruby_bounds)
        );
        assert!(
            layout.ruby[1].ruby_bounds.x >= layout.ruby[0].ruby_bounds.right(),
            "second horizontal ruby should move after the first annotation"
        );
    }

    #[test]
    fn long_horizontal_ruby_expands_base_allocation_before_overhang() {
        let mut frame = frame_with_run("夢", RichTextPresentation::default());
        push_ruby(&mut frame, 0, "夢".len(), "ながいよみ");
        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        assert_eq!(layout.ruby.len(), 1);
        assert!(
            layout.ruby[0].base_bounds.width > layout.glyphs[0].bounds.width,
            "long ruby should expand the base allocation before using overhang"
        );
        assert_f32_eq(
            layout.ruby[0].base_bounds.width,
            layout.ruby[0].ruby_bounds.width,
        );
        assert_f32_eq(layout.ruby[0].ruby_bounds.x, layout.ruby[0].base_bounds.x);
    }

    #[test]
    fn horizontal_ruby_uses_limited_overhang_after_base_expansion() {
        let mut frame = frame_with_run("夢", RichTextPresentation::default());
        push_ruby(&mut frame, 0, "夢".len(), "ながいよみか");
        let config = TextLayoutConfig {
            size: LayoutSize::new(60.0, 120.0),
            ruby_font_size: 12.0,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.ruby.len(), 1);
        assert_f32_eq(layout.ruby[0].base_bounds.width, 60.0);
        assert_f32_eq(
            layout.ruby[0].base_bounds.x - layout.ruby[0].ruby_bounds.x,
            config.ruby_font_size * 0.5,
        );
        assert_f32_eq(
            layout.ruby[0].ruby_bounds.right() - layout.ruby[0].base_bounds.right(),
            config.ruby_font_size * 0.5,
        );
        assert!(
            layout.ruby[0].base_bounds.x - layout.ruby[0].ruby_bounds.x
                <= config.ruby_font_size * 0.5
        );
    }

    #[test]
    fn vertical_ruby_collision_shifts_adjacent_annotations_inline() {
        for writing_mode in [
            RichTextWritingMode::VerticalRl,
            RichTextWritingMode::VerticalLr,
        ] {
            let mut frame = frame_with_run("夢星", vertical_presentation(writing_mode));
            push_ruby(&mut frame, 0, "夢".len(), "ながいよみ");
            push_ruby(&mut frame, "夢".len(), "夢星".len(), "ながいよみ");
            let layout =
                layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

            assert_eq!(layout.ruby.len(), 2);
            assert_eq!(layout.ruby[0].writing_mode, writing_mode);
            assert!(
                !layout.ruby[0]
                    .ruby_bounds
                    .intersects(layout.ruby[1].ruby_bounds)
            );
            assert!(
                layout.ruby[1].ruby_bounds.y >= layout.ruby[0].ruby_bounds.bottom(),
                "second {writing_mode:?} ruby should move below the first annotation"
            );
            assert_f32_eq(layout.ruby[1].ruby_bounds.x, layout.ruby[0].ruby_bounds.x);
        }
    }

    #[test]
    fn vertical_lr_ruby_uses_left_annotation_track_with_base_expansion() {
        let mut frame =
            frame_with_run("夢", vertical_presentation(RichTextWritingMode::VerticalLr));
        push_ruby(&mut frame, 0, "夢".len(), "ながいよみ");
        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        assert_eq!(layout.ruby.len(), 1);
        assert_eq!(layout.ruby[0].writing_mode, RichTextWritingMode::VerticalLr);
        assert!(
            layout.ruby[0].base_bounds.height > layout.glyphs[0].bounds.height,
            "long vertical ruby should expand the base allocation along inline progression"
        );
        assert!(
            layout.ruby[0].ruby_bounds.right() <= layout.ruby[0].base_bounds.x,
            "vertical_lr ruby annotation should be placed on the left side of the base"
        );
    }

    #[test]
    fn vertical_ruby_reserves_annotation_track_inside_layout_width() {
        for (writing_mode, annotation_on_right) in [
            (RichTextWritingMode::VerticalRl, true),
            (RichTextWritingMode::VerticalLr, false),
        ] {
            let mut frame = frame_with_run("夢", vertical_presentation(writing_mode));
            push_ruby(&mut frame, 0, "夢".len(), "ゆめ");
            let config = TextLayoutConfig {
                origin: LayoutPoint::new(0.0, 0.0),
                size: LayoutSize::new(84.0, 84.0),
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");

            assert_eq!(layout.ruby.len(), 1);
            let base = layout.ruby[0].base_bounds;
            let annotation = layout.ruby[0].ruby_bounds;
            assert!(
                annotation.x >= config.origin.x
                    && annotation.right() <= config.origin.x + config.size.width,
                "{writing_mode:?} ruby annotation should stay inside the layout width: {annotation:?}"
            );
            if annotation_on_right {
                assert!(annotation.x >= base.right());
            } else {
                assert!(annotation.right() <= base.x);
            }
        }
    }

    #[test]
    fn vertical_ruby_layout_survives_typewriter_visibility_effect() {
        for writing_mode in [
            RichTextWritingMode::VerticalRl,
            RichTextWritingMode::VerticalLr,
        ] {
            let mut presentation = vertical_presentation(writing_mode);
            presentation.effects.push(RichTextEffectDescriptor {
                id: "typewriter".to_owned(),
                params: BTreeMap::from([(
                    "cps".to_owned(),
                    RichTextParam::Milli { value: Milli::ONE },
                )]),
                target: RichTextEffectTarget::Run,
                phase: RichTextEffectPhase::GlyphMask,
                state_scope: RichTextStateScope::Run,
            });
            let mut frame = frame_with_run("夢", presentation);
            frame.display_map.text_runs[0].source = RichTextTextSource::RubyBase;
            push_ruby(&mut frame, 0, "夢".len(), "ゆめ");

            let layout =
                layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

            assert_eq!(layout.glyphs.len(), 1);
            assert_eq!(layout.ruby.len(), 1);
            assert_eq!(layout.ruby[0].base_range, RichTextRange::new(0, "夢".len()));
            assert_eq!(layout.ruby[0].writing_mode, writing_mode);
        }
    }

    #[test]
    fn vertical_ruby_base_expansion_feeds_back_into_column_breaks() {
        for (writing_mode, next_column_moves_right) in [
            (RichTextWritingMode::VerticalRl, false),
            (RichTextWritingMode::VerticalLr, true),
        ] {
            let mut frame = frame_with_run("天夢", vertical_presentation(writing_mode));
            push_ruby(&mut frame, "天".len(), "天夢".len(), "ながいよみ");
            let config = TextLayoutConfig {
                size: LayoutSize::new(160.0, 84.0),
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");

            assert_eq!(layout.glyphs.len(), 2);
            assert_eq!(layout.glyphs[0].text, "天");
            assert_eq!(layout.glyphs[1].text, "夢");
            assert_vertical_layout_column_restart(
                &layout.glyphs[0],
                &layout.glyphs[1],
                next_column_moves_right,
                "long ruby base allocation should force the annotated cluster to the next column",
            );
            assert_f32_eq(layout.glyphs[1].origin.y, config.origin.y);
            assert_eq!(layout.ruby.len(), 1);
            assert_f32_eq(layout.ruby[0].base_bounds.y, config.origin.y);
            assert!(
                layout.ruby[0].base_bounds.bottom() <= config.origin.y + config.size.height,
                "expanded {writing_mode:?} ruby base should fit inside the column after feedback"
            );
        }
    }

    #[test]
    fn vertical_ruby_multi_cluster_base_breaks_before_the_base_start() {
        for (writing_mode, next_column_moves_right) in [
            (RichTextWritingMode::VerticalRl, false),
            (RichTextWritingMode::VerticalLr, true),
        ] {
            let mut frame = frame_with_run("天夢星", vertical_presentation(writing_mode));
            push_ruby(&mut frame, "天".len(), "天夢星".len(), "ゆめ");
            let config = TextLayoutConfig {
                size: LayoutSize::new(160.0, 84.0),
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");

            assert_eq!(layout.glyphs.len(), 3);
            assert_eq!(layout.glyphs[1].text, "夢");
            assert_eq!(layout.glyphs[2].text, "星");
            assert_vertical_layout_column_restart(
                &layout.glyphs[0],
                &layout.glyphs[1],
                next_column_moves_right,
                "multi-cluster ruby base should move as a unit before it is split by overflow",
            );
            assert_f32_eq(layout.glyphs[1].origin.y, config.origin.y);
            assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
            assert!(layout.glyphs[2].origin.y > layout.glyphs[1].origin.y);
            assert_eq!(layout.ruby.len(), 1);
            assert_eq!(
                layout.ruby[0].base_range,
                RichTextRange::new("天".len(), "天夢星".len())
            );
            assert_f32_eq(layout.ruby[0].base_bounds.x, layout.glyphs[1].bounds.x);
        }
    }

    #[test]
    fn overheight_vertical_ruby_splits_into_column_segments() {
        for (writing_mode, continuation_moves_right) in [
            (RichTextWritingMode::VerticalRl, true),
            (RichTextWritingMode::VerticalLr, false),
        ] {
            let mut frame = frame_with_run("夢", vertical_presentation(writing_mode));
            push_ruby(&mut frame, 0, "夢".len(), "あいうえお");
            let config = TextLayoutConfig {
                size: LayoutSize::new(160.0, 42.0),
                ruby_font_size: 14.0,
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");

            assert_eq!(layout.ruby.len(), 2);
            assert_eq!(layout.ruby[0].writing_mode, writing_mode);
            assert_eq!(layout.ruby[0].ruby_index, layout.ruby[1].ruby_index);
            assert_eq!(layout.ruby[0].ruby, "あいう");
            assert_eq!(layout.ruby[1].ruby, "えお");
            assert!(layout.ruby[0].ruby_bounds.height <= config.size.height);
            assert!(layout.ruby[1].ruby_bounds.height <= config.size.height);
            if continuation_moves_right {
                assert!(layout.ruby[1].ruby_bounds.x > layout.ruby[0].ruby_bounds.x);
            } else {
                assert!(layout.ruby[1].ruby_bounds.x < layout.ruby[0].ruby_bounds.x);
            }
            assert_f32_eq(layout.ruby[0].ruby_bounds.y, config.origin.y);
            assert_f32_eq(layout.ruby[1].ruby_bounds.y, config.origin.y);
        }
    }

    fn push_ruby(frame: &mut LineDisplayFrame, start: usize, end: usize, ruby: &str) {
        frame
            .display_map
            .ruby_annotations
            .push(RichTextRubyAnnotation {
                base_range: RichTextRange::new(start, end),
                ruby: ruby.to_owned(),
                node_index: frame.display_map.ruby_annotations.len(),
                styles: Vec::new(),
                presentation: RichTextPresentation::default(),
            });
    }

    fn nth_laid_out_glyph<'layout>(
        layout: &'layout LaidOutText,
        text: &str,
        occurrence: usize,
    ) -> &'layout LaidOutGlyph {
        layout
            .glyphs
            .iter()
            .filter(|glyph| glyph.text == text)
            .nth(occurrence)
            .unwrap_or_else(|| panic!("missing laid-out glyph {text:?} occurrence {occurrence}"))
    }

    fn vertical_layout_column_count(layout: &LaidOutText) -> usize {
        layout
            .glyphs
            .iter()
            .map(|glyph| glyph.origin.x.to_bits())
            .collect::<HashSet<_>>()
            .len()
    }

    fn assert_same_vertical_layout_column(
        previous: &LaidOutGlyph,
        current: &LaidOutGlyph,
        message: &str,
    ) {
        assert_f32_eq(previous.origin.x, current.origin.x);
        assert!(
            current.origin.y > previous.origin.y,
            "{message}: expected {current:?} to advance after {previous:?}"
        );
    }

    fn assert_vertical_layout_after(
        previous: &LaidOutGlyph,
        current: &LaidOutGlyph,
        message: &str,
    ) {
        assert_same_vertical_layout_column(previous, current, message);
    }

    fn assert_next_vertical_layout_column(
        previous: &LaidOutGlyph,
        current: &LaidOutGlyph,
        next_column_moves_right: bool,
        message: &str,
    ) {
        if next_column_moves_right {
            assert!(
                current.origin.x > previous.origin.x,
                "{message}: expected {current:?} to move right after {previous:?}"
            );
        } else {
            assert!(
                current.origin.x < previous.origin.x,
                "{message}: expected {current:?} to move left after {previous:?}"
            );
        }
        assert!(
            current.origin.y < previous.origin.y,
            "{message}: expected {current:?} to restart above {previous:?}"
        );
    }

    fn assert_vertical_layout_column_restart(
        previous: &LaidOutGlyph,
        current: &LaidOutGlyph,
        next_column_moves_right: bool,
        message: &str,
    ) {
        if next_column_moves_right {
            assert!(
                current.origin.x > previous.origin.x,
                "{message}: expected {current:?} to move right after {previous:?}"
            );
        } else {
            assert!(
                current.origin.x < previous.origin.x,
                "{message}: expected {current:?} to move left after {previous:?}"
            );
        }
    }

    fn assert_f32_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < f32::EPSILON,
            "expected {actual} to equal {expected}"
        );
    }
}
