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
use std::ops::Range;
use thiserror::Error;

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
                );
            }
            RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr => {
                let context = RunLayoutContext {
                    run_index,
                    range_start: range.start,
                    presentation: &run.presentation,
                    config,
                };
                layout_vertical_run(&mut out.glyphs, text, writing_mode, vertical_latin, context);
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

fn layout_horizontal_run(
    glyphs: &mut Vec<LaidOutGlyph>,
    run_index: usize,
    range_start: usize,
    text: &str,
    presentation: &RichTextPresentation,
    config: TextLayoutConfig,
) {
    let mut x = config.origin.x;
    let mut y = config.origin.y;
    for (offset, ch) in text.char_indices() {
        if ch == '\n' {
            x = config.origin.x;
            y += config.line_advance;
            continue;
        }
        let width = horizontal_advance(ch, config.font_size);
        let start = range_start + offset;
        let end = start + ch.len_utf8();
        let bounds = LayoutRect::new(x, y, width.max(1.0), config.line_advance);
        glyphs.push(LaidOutGlyph {
            run_index,
            range: RichTextRange::new(start, end),
            text: ch.to_string(),
            origin: LayoutPoint::new(x, y),
            advance: LayoutSize::new(width, 0.0),
            bounds,
            writing_mode: RichTextWritingMode::HorizontalTb,
            orientation: GlyphOrientation::Upright,
            presentation: presentation.clone(),
        });
        x += width;
    }
}

fn layout_vertical_run(
    glyphs: &mut Vec<LaidOutGlyph>,
    text: &str,
    writing_mode: RichTextWritingMode,
    vertical_latin: RichTextVerticalLatinMode,
    context: RunLayoutContext<'_>,
) {
    let config = context.config;
    let mut x = vertical_column_start(writing_mode, config);
    let mut y = config.origin.y;
    let column_step = vertical_column_step(writing_mode, context.presentation, config);
    for cluster in vertical_clusters(text, vertical_latin) {
        if cluster.text == "\n" {
            x += column_step;
            y = config.origin.y;
            continue;
        }
        if y + config.line_advance > config.origin.y + config.size.height {
            x += column_step;
            y = config.origin.y;
        }
        let start = context.range_start + cluster.range.start;
        let end = context.range_start + cluster.range.end;
        let bounds = LayoutRect::new(x, y, config.line_advance, config.line_advance);
        glyphs.push(LaidOutGlyph {
            run_index: context.run_index,
            range: RichTextRange::new(start, end),
            text: cluster.text,
            origin: LayoutPoint::new(x, y),
            advance: LayoutSize::new(0.0, config.line_advance),
            bounds,
            writing_mode,
            orientation: cluster.orientation,
            presentation: context.presentation.clone(),
        });
        y += config.line_advance;
    }
}

#[derive(Clone, Copy)]
struct RunLayoutContext<'a> {
    run_index: usize,
    range_start: usize,
    presentation: &'a RichTextPresentation,
    config: TextLayoutConfig,
}

fn layout_ruby(
    frame: &LineDisplayFrame,
    glyphs: &[LaidOutGlyph],
    config: TextLayoutConfig,
) -> Vec<LaidOutRuby> {
    frame
        .display_map
        .ruby_annotations
        .iter()
        .enumerate()
        .filter_map(|(ruby_index, annotation)| {
            layout_one_ruby(ruby_index, annotation, glyphs, config)
        })
        .collect()
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
    let ruby_bounds = if vertical {
        let height = usize_to_f32(annotation.ruby.chars().count().max(1)) * config.ruby_font_size;
        LayoutRect::new(
            base_bounds.right() + config.ruby_font_size * 0.25,
            base_bounds.y,
            config.ruby_font_size,
            height.max(config.ruby_font_size),
        )
    } else {
        let width = usize_to_f32(annotation.ruby.chars().count().max(1)) * config.ruby_font_size;
        LayoutRect::new(
            base_bounds.x + (base_bounds.width - width).max(0.0) / 2.0,
            (base_bounds.y - config.ruby_font_size * 1.2).max(0.0),
            width.max(config.ruby_font_size),
            config.ruby_font_size,
        )
    };
    Some(LaidOutRuby {
        ruby_index,
        base_range: annotation.base_range,
        ruby: annotation.ruby.clone(),
        base_bounds,
        ruby_bounds,
        presentation: annotation.presentation.clone(),
    })
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
}

fn vertical_clusters(
    text: &str,
    vertical_latin: RichTextVerticalLatinMode,
) -> Vec<VerticalCluster> {
    let mut clusters = Vec::new();
    let mut iter = text.char_indices().peekable();
    while let Some((offset, ch)) = iter.next() {
        if ch.is_ascii_digit() {
            let mut end = offset + ch.len_utf8();
            let mut value = ch.to_string();
            while let Some((next_offset, next)) = iter.peek().copied() {
                if !next.is_ascii_digit() || value.chars().count() >= 4 {
                    break;
                }
                iter.next();
                value.push(next);
                end = next_offset + next.len_utf8();
            }
            if value.chars().count() >= 2 {
                clusters.push(VerticalCluster {
                    range: offset..end,
                    text: value,
                    orientation: GlyphOrientation::TextCombineUpright,
                });
                continue;
            }
            clusters.push(VerticalCluster {
                range: offset..end,
                text: value,
                orientation: vertical_orientation(ch, vertical_latin),
            });
            continue;
        }
        clusters.push(VerticalCluster {
            range: offset..offset + ch.len_utf8(),
            text: ch.to_string(),
            orientation: vertical_orientation(ch, vertical_latin),
        });
    }
    clusters
}

fn vertical_orientation(ch: char, vertical_latin: RichTextVerticalLatinMode) -> GlyphOrientation {
    match vertical_latin {
        RichTextVerticalLatinMode::Upright => GlyphOrientation::Upright,
        RichTextVerticalLatinMode::Sideways => GlyphOrientation::SidewaysCw,
        RichTextVerticalLatinMode::Mixed => {
            if ch.is_ascii_alphabetic() {
                GlyphOrientation::SidewaysCw
            } else {
                GlyphOrientation::Upright
            }
        }
    }
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

    fn assert_f32_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < f32::EPSILON,
            "expected {actual} to equal {expected}"
        );
    }
}
