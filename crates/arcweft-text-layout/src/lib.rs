//! Sans I/O rich-text layout geometry for Arcweft players and agent debugging.
//!
//! This crate owns deterministic text geometry. Renderer adapters consume the
//! resulting `LaidOutText` instead of deriving bounds from pixels or from
//! renderer-specific buffers.

use arcweft_render_text::{
    LineDisplayFrame, RichTextPresentation, RichTextRange, RichTextRubyAnnotation,
    RichTextVerticalLatinMode, RichTextWritingMode,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ops::Range;
use thiserror::Error;
use unicode_linebreak::{BreakOpportunity, linebreaks};
use unicode_segmentation::UnicodeSegmentation as _;

mod vertical_orientation;
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
        }
    }
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
    let mut state = TextLayoutState::new(config);
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
        let glyph_start = out.glyphs.len();
        match writing_mode {
            RichTextWritingMode::HorizontalTb => {
                layout_horizontal_run(
                    &mut out.glyphs,
                    run_index,
                    range.start,
                    text,
                    &run.presentation,
                    config,
                    &mut state,
                );
            }
            RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr => {
                let context = RunLayoutContext {
                    run_index,
                    range_start: range.start,
                    presentation: &run.presentation,
                    ruby_annotations: &frame.display_map.ruby_annotations,
                    config,
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

#[derive(Clone, Copy, Debug)]
struct TextLayoutState {
    horizontal: LayoutCursor,
    vertical_rl: LayoutCursor,
    vertical_lr: LayoutCursor,
}

impl TextLayoutState {
    fn new(config: TextLayoutConfig) -> Self {
        Self {
            horizontal: LayoutCursor::new(config.origin.x, config.origin.y),
            vertical_rl: LayoutCursor::new(
                vertical_column_start(RichTextWritingMode::VerticalRl, config),
                config.origin.y,
            ),
            vertical_lr: LayoutCursor::new(
                vertical_column_start(RichTextWritingMode::VerticalLr, config),
                config.origin.y,
            ),
        }
    }
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
    for (cluster_index, cluster) in clusters.iter().enumerate() {
        if is_vertical_line_break_cluster(&cluster.text) {
            cursor.x += column_step;
            cursor.y = config.origin.y;
            continue;
        }
        let start = context.range_start + cluster.range.start;
        let end = context.range_start + cluster.range.end;
        let range = RichTextRange::new(start, end);
        let required_inline_extent = vertical_cluster_required_inline_extent(
            range,
            context.range_start,
            &clusters,
            context.ruby_annotations,
            config,
        );
        if cursor.y + required_inline_extent > config.origin.y + config.size.height
            && cursor.y > config.origin.y
            && cluster.break_allowed_before
            && !is_jlreq_line_head_prohibited_cluster(&cluster.text)
        {
            cursor.x += column_step;
            cursor.y = config.origin.y;
        }
        if vertical_cluster_should_break_before_line_end_prohibited(
            cluster_index,
            &clusters,
            required_inline_extent,
            context,
            *cursor,
        ) {
            cursor.x += column_step;
            cursor.y = config.origin.y;
        }
        let bounds = LayoutRect::new(cursor.x, cursor.y, config.line_advance, config.line_advance);
        glyphs.push(LaidOutGlyph {
            run_index: context.run_index,
            range,
            text: cluster.text.clone(),
            origin: LayoutPoint::new(cursor.x, cursor.y),
            advance: LayoutSize::new(0.0, config.line_advance),
            bounds,
            writing_mode,
            orientation: cluster.orientation,
            vertical_form: cluster.vertical_form,
            presentation: context.presentation.clone(),
        });
        cursor.y += config.line_advance;
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

fn vertical_cluster_should_break_before_line_end_prohibited(
    cluster_index: usize,
    clusters: &[VerticalCluster],
    required_inline_extent: f32,
    context: RunLayoutContext<'_>,
    cursor: LayoutCursor,
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    if cursor.y <= context.config.origin.y
        || !cluster.break_allowed_before
        || !is_jlreq_line_end_prohibited_cluster(&cluster.text)
    {
        return false;
    }
    let Some(next_cluster) = clusters[cluster_index + 1..]
        .iter()
        .find(|candidate| !is_vertical_line_break_cluster(&candidate.text))
    else {
        return false;
    };
    let next_range = RichTextRange::new(
        context.range_start + next_cluster.range.start,
        context.range_start + next_cluster.range.end,
    );
    let next_required_inline_extent = vertical_cluster_required_inline_extent(
        next_range,
        context.range_start,
        clusters,
        context.ruby_annotations,
        context.config,
    );
    cursor.y + required_inline_extent + next_required_inline_extent
        > context.config.origin.y + context.config.size.height
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
        .filter_map(|(ruby_index, annotation)| {
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
) -> Option<LaidOutRuby> {
    let base_bounds = union_bounds(
        glyphs
            .iter()
            .filter(|glyph| ranges_overlap(glyph.range, annotation.base_range))
            .map(|glyph| glyph.bounds),
    )?;
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
    let ruby_bounds = if vertical {
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
        )
    } else {
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
        )
    };
    Some(LaidOutRuby {
        ruby_index,
        base_range: annotation.base_range,
        ruby: annotation.ruby.clone(),
        base_bounds,
        ruby_bounds,
        writing_mode,
        presentation: annotation.presentation.clone(),
    })
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
            RichTextWritingMode::VerticalRl => config.ruby_font_size + GAP,
            RichTextWritingMode::VerticalLr | RichTextWritingMode::HorizontalTb => {
                -(config.ruby_font_size + GAP)
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

fn is_jlreq_line_end_prohibited_cluster(grapheme: &str) -> bool {
    grapheme
        .chars()
        .next()
        .is_some_and(is_jlreq_line_end_prohibited_char)
}

fn is_jlreq_line_head_prohibited_cluster(grapheme: &str) -> bool {
    grapheme
        .chars()
        .next()
        .is_some_and(is_jlreq_line_head_prohibited_char)
}

const fn is_jlreq_line_head_prohibited_char(ch: char) -> bool {
    is_jlreq_closing_punctuation_char(ch) || is_jlreq_small_kana_char(ch)
}

const fn is_jlreq_closing_punctuation_char(ch: char) -> bool {
    matches!(
        ch,
        ')' | ']'
            | '}'
            | '\u{2019}'
            | '\u{201d}'
            | '\u{3001}'
            | '\u{3002}'
            | '\u{3009}'
            | '\u{300b}'
            | '\u{300d}'
            | '\u{300f}'
            | '\u{3011}'
            | '\u{3015}'
            | '\u{3017}'
            | '\u{3019}'
            | '\u{301b}'
            | '\u{301e}'
            | '\u{301f}'
            | '\u{ff09}'
            | '\u{ff0c}'
            | '\u{ff0e}'
            | '\u{ff3d}'
            | '\u{ff5d}'
            | '\u{ff60}'
    )
}

const fn is_jlreq_small_kana_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{3041}'
            | '\u{3043}'
            | '\u{3045}'
            | '\u{3047}'
            | '\u{3049}'
            | '\u{3063}'
            | '\u{3083}'
            | '\u{3085}'
            | '\u{3087}'
            | '\u{308e}'
            | '\u{3095}'
            | '\u{3096}'
            | '\u{30a1}'
            | '\u{30a3}'
            | '\u{30a5}'
            | '\u{30a7}'
            | '\u{30a9}'
            | '\u{30c3}'
            | '\u{30e3}'
            | '\u{30e5}'
            | '\u{30e7}'
            | '\u{30ee}'
            | '\u{30f5}'
            | '\u{30f6}'
            | '\u{31f0}'..='\u{31ff}'
    )
}

const fn is_jlreq_line_end_prohibited_char(ch: char) -> bool {
    matches!(
        ch,
        '(' | '['
            | '{'
            | '\u{2018}'
            | '\u{201c}'
            | '\u{3008}'
            | '\u{300a}'
            | '\u{300c}'
            | '\u{300e}'
            | '\u{3010}'
            | '\u{3014}'
            | '\u{3016}'
            | '\u{3018}'
            | '\u{301a}'
            | '\u{301d}'
            | '\u{ff08}'
            | '\u{ff3b}'
            | '\u{ff5b}'
            | '\u{ff5f}'
    )
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
        LineDisplayFrame, RichTextDisplayMap, RichTextLayout, RichTextTextRun, RichTextTextSource,
    };

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
        assert!(
            layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
            "closing punctuation may overhang the current column instead of violating kinsoku"
        );
        assert_eq!(layout.glyphs[3].text, "人");
        assert!(
            layout.glyphs[3].origin.x < layout.glyphs[2].origin.x,
            "the next breakable cluster should start the next vertical_rl column"
        );
        assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
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
        let mut frame = frame_with_run(
            "夢星",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        push_ruby(&mut frame, 0, "夢".len(), "ながいよみ");
        push_ruby(&mut frame, "夢".len(), "夢星".len(), "ながいよみ");
        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        assert_eq!(layout.ruby.len(), 2);
        assert_eq!(layout.ruby[0].writing_mode, RichTextWritingMode::VerticalRl);
        assert!(
            !layout.ruby[0]
                .ruby_bounds
                .intersects(layout.ruby[1].ruby_bounds)
        );
        assert!(
            layout.ruby[1].ruby_bounds.y >= layout.ruby[0].ruby_bounds.bottom(),
            "second vertical ruby should move below the first annotation"
        );
        assert_f32_eq(layout.ruby[1].ruby_bounds.x, layout.ruby[0].ruby_bounds.x);
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
    fn vertical_ruby_base_expansion_feeds_back_into_column_breaks() {
        let mut frame = frame_with_run(
            "天夢",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        push_ruby(&mut frame, "天".len(), "天夢".len(), "ながいよみ");
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 2);
        assert_eq!(layout.glyphs[0].text, "天");
        assert_eq!(layout.glyphs[1].text, "夢");
        assert!(
            layout.glyphs[1].origin.x < layout.glyphs[0].origin.x,
            "long ruby base allocation should force the annotated cluster to the next column"
        );
        assert_f32_eq(layout.glyphs[1].origin.y, config.origin.y);
        assert_eq!(layout.ruby.len(), 1);
        assert_f32_eq(layout.ruby[0].base_bounds.y, config.origin.y);
        assert!(
            layout.ruby[0].base_bounds.bottom() <= config.origin.y + config.size.height,
            "expanded ruby base should fit inside the column after feedback"
        );
    }

    #[test]
    fn vertical_ruby_multi_cluster_base_breaks_before_the_base_start() {
        let mut frame = frame_with_run(
            "天夢星",
            vertical_presentation(RichTextWritingMode::VerticalRl),
        );
        push_ruby(&mut frame, "天".len(), "天夢星".len(), "ゆめ");
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 3);
        assert_eq!(layout.glyphs[1].text, "夢");
        assert_eq!(layout.glyphs[2].text, "星");
        assert!(
            layout.glyphs[1].origin.x < layout.glyphs[0].origin.x,
            "multi-cluster ruby base should move as a unit before it is split by overflow"
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

    fn assert_f32_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < f32::EPSILON,
            "expected {actual} to equal {expected}"
        );
    }
}
