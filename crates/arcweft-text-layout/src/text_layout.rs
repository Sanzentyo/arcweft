//! Canonical font-shaped layout output and source mapping.

use std::{fmt, ops::Range};

use arcweft_render_text::ResolvedTextStyle;
use arcweft_text_model::{
    RichTextNodeIndex, RichTextNodeRange, RichTextPresentation, RichTextRange,
    RichTextTextRunRange, RichTextWritingMode,
};
use serde::{Deserialize, Serialize};

use crate::{
    FontInventoryHash, GlyphOrientation, GlyphVerticalForm, LayoutPoint, LayoutRect, LayoutSize,
    ShapedGlyphKey,
};

/// Stable geometry/cache identity of one complete text layout.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct TextLayoutHash([u8; 32]);

/// One visual line or vertical column.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextLayoutLine {
    pub source_range: RichTextRange,
    pub glyph_range: Range<u32>,
    pub bounds: LayoutRect,
    pub writing_mode: RichTextWritingMode,
}

/// Bounds and resolved metadata for one source run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextLayoutRun {
    pub run_index: u32,
    pub source_range: RichTextRange,
    pub glyph_range: Range<u32>,
    pub bounds: LayoutRect,
    pub writing_mode: RichTextWritingMode,
    pub style: ResolvedTextStyle,
    pub presentation: RichTextPresentation,
}

/// One shaped raster glyph in final visual order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextLayoutGlyph {
    pub run_index: u32,
    pub source_range: RichTextRange,
    pub line_index: u32,
    pub cluster_index: u32,
    pub logical_ordinal: u32,
    pub origin: LayoutPoint,
    pub advance: LayoutSize,
    /// Logical character/cell geometry used by hit testing and selection.
    pub layout_bounds: LayoutRect,
    pub ink_bounds: LayoutRect,
    pub orientation: GlyphOrientation,
    pub vertical_form: GlyphVerticalForm,
    /// Horizontal scale selected by vertical text-combine placement.
    pub inline_scale: f32,
    pub shape_key: ShapedGlyphKey,
}

/// One shaped glyph belonging to ruby annotation text rather than document
/// source text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextLayoutRubyGlyph {
    pub text_range: RichTextRange,
    pub cluster_index: u32,
    pub origin: LayoutPoint,
    pub advance: LayoutSize,
    pub layout_bounds: LayoutRect,
    pub ink_bounds: LayoutRect,
    pub orientation: GlyphOrientation,
    pub inline_scale: f32,
    pub shape_key: ShapedGlyphKey,
}

/// One allocated body-cluster interval, distinct from its physical glyph cells.
/// The interval retains the actual canonical body glyphs which own its paint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextLayoutBodyAllocation {
    pub glyph_range: Range<u32>,
    pub bounds: LayoutRect,
}

/// Ruby placement tied to one canonical base range.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextLayoutRuby {
    pub ruby_index: u32,
    pub owner_node: RichTextNodeIndex,
    pub body_nodes: RichTextNodeRange,
    pub base_runs: RichTextTextRunRange,
    pub base_range: RichTextRange,
    pub text: String,
    pub base_allocations: Vec<TextLayoutBodyAllocation>,
    pub base_bounds: LayoutRect,
    pub ruby_bounds: LayoutRect,
    pub glyphs: Vec<TextLayoutRubyGlyph>,
    pub writing_mode: RichTextWritingMode,
    pub style: ResolvedTextStyle,
    pub presentation: RichTextPresentation,
}

impl TextLayoutRuby {
    /// Allocation and actual ink are measured once in canonical allocation order.
    pub(crate) fn measure_base_bounds(
        allocations: &[TextLayoutBodyAllocation],
        body_glyphs: &[TextLayoutGlyph],
    ) -> Option<LayoutRect> {
        let mut bounds = allocations
            .iter()
            .map(|allocation| allocation.bounds)
            .reduce(LayoutRect::union)?;
        for allocation in allocations {
            let start = usize::try_from(allocation.glyph_range.start).ok()?;
            let end = usize::try_from(allocation.glyph_range.end).ok()?;
            for glyph in body_glyphs.get(start..end)? {
                bounds = bounds.union(glyph.ink_bounds);
            }
        }
        Some(bounds)
    }
}

/// Logical source identity for one visual glyph.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextLayoutGlyphSource {
    pub run_index: u32,
    pub source_range: RichTextRange,
    pub line_index: u32,
    pub cluster_index: u32,
    pub logical_ordinal: u32,
}

/// Canonical source map kept independently from visual glyph ordering.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextLayoutSourceMap {
    glyphs: Vec<TextLayoutGlyphSource>,
}

/// Complete shared text layout consumed by preparation and interaction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextLayout {
    pub lines: Vec<TextLayoutLine>,
    pub runs: Vec<TextLayoutRun>,
    pub glyphs: Vec<TextLayoutGlyph>,
    pub ruby: Vec<TextLayoutRuby>,
    pub bounds: Option<LayoutRect>,
    pub source_map: TextLayoutSourceMap,
    pub hash: TextLayoutHash,
    pub font_inventory: FontInventoryHash,
}

impl TextLayoutHash {
    pub const fn from_bytes(value: [u8; 32]) -> Self {
        Self(value)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for TextLayoutHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TextLayoutHash(")?;
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        formatter.write_str(")")
    }
}

impl TextLayoutSourceMap {
    #[must_use]
    pub fn new(glyphs: Vec<TextLayoutGlyphSource>) -> Self {
        Self { glyphs }
    }

    pub fn glyphs(&self) -> &[TextLayoutGlyphSource] {
        &self.glyphs
    }

    pub fn glyphs_for_source_range(
        &self,
        source_range: RichTextRange,
    ) -> impl Iterator<Item = (usize, TextLayoutGlyphSource)> + '_ {
        self.glyphs
            .iter()
            .copied()
            .enumerate()
            .filter(move |(_, glyph)| ranges_overlap(glyph.source_range, source_range))
    }
}

fn ranges_overlap(left: RichTextRange, right: RichTextRange) -> bool {
    left.start < right.end && right.start < left.end
}

/// Structural validation for a transported prepared document layout.
#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum TextLayoutContextError {
    #[error(transparent)]
    Range(#[from] arcweft_text_model::ProjectedTextRangeError),
    #[error("{kind} {index} references an absent layout element")]
    MissingElement { kind: &'static str, index: usize },
    #[error("{kind} {index} has invalid finite layout geometry")]
    Geometry { kind: &'static str, index: usize },
    #[error("glyph source map differs from shaped glyph {index}")]
    SourceMap { index: usize },
    #[error("Ruby annotation {index} has an invalid text range")]
    RubyText { index: usize },
    #[error("layout {kind} identity/order differs at {index}")]
    Identity { kind: &'static str, index: usize },
    #[error("Ruby annotation {index} does not allocate its exact owned body clusters")]
    RubyAllocation { index: usize },
}

impl TextLayout {
    /// Body glyphs are emitted in allocated cluster order, even when source
    /// glyph order inside one shaped cluster differs from logical text order.
    pub fn cluster_glyphs(&self, index: u32) -> Option<&[TextLayoutGlyph]> {
        let start = self
            .glyphs
            .partition_point(|glyph| glyph.cluster_index < index);
        let end =
            start + self.glyphs[start..].partition_point(|glyph| glyph.cluster_index == index);
        (start != end).then(|| &self.glyphs[start..end])
    }

    /// Validate owned layout/source-map data against its checked text context.
    pub fn validate_text_context(
        &self,
        ranges: arcweft_text_model::ProjectedTextRangeDomain,
        text: &str,
    ) -> Result<(), TextLayoutContextError> {
        ranges.validate_prepared_range(ranges.prepared_range(), text)?;
        for (index, line) in self.lines.iter().enumerate() {
            validate_layout_source_range(ranges, text, line.source_range)?;
            validate_glyph_interval("line", index, &line.glyph_range, self.glyphs.len())?;
            validate_context_rect("line", index, line.bounds)?;
        }
        for (index, run) in self.runs.iter().enumerate() {
            if usize::try_from(run.run_index).ok() != Some(index)
                || index.checked_sub(1).is_some_and(|previous| {
                    self.runs[previous].source_range.end > run.source_range.start
                })
            {
                return Err(TextLayoutContextError::Identity { kind: "run", index });
            }
            validate_layout_source_range(ranges, text, run.source_range)?;
            validate_glyph_interval("run", index, &run.glyph_range, self.glyphs.len())?;
            validate_context_rect("run", index, run.bounds)?;
        }
        if self.source_map.glyphs.len() != self.glyphs.len() {
            return Err(TextLayoutContextError::SourceMap {
                index: self.source_map.glyphs.len().min(self.glyphs.len()),
            });
        }
        for (index, glyph) in self.glyphs.iter().enumerate() {
            if index
                .checked_sub(1)
                .is_some_and(|previous| self.glyphs[previous].cluster_index > glyph.cluster_index)
            {
                return Err(TextLayoutContextError::Identity {
                    kind: "cluster",
                    index,
                });
            }
            validate_layout_source_range(ranges, text, glyph.source_range)?;
            if usize::try_from(glyph.run_index).map_or(true, |n| n >= self.runs.len())
                || usize::try_from(glyph.line_index).map_or(true, |n| n >= self.lines.len())
            {
                return Err(TextLayoutContextError::MissingElement {
                    kind: "glyph",
                    index,
                });
            }
            validate_context_rect("glyph", index, glyph.layout_bounds)?;
            validate_context_rect("glyph ink", index, glyph.ink_bounds)?;
            if !glyph.origin.x.is_finite()
                || !glyph.origin.y.is_finite()
                || !glyph.advance.width.is_finite()
                || !glyph.advance.height.is_finite()
                || !glyph.inline_scale.is_finite()
                || glyph.inline_scale <= 0.0
            {
                return Err(TextLayoutContextError::Geometry {
                    kind: "glyph",
                    index,
                });
            }
            let source = &self.source_map.glyphs[index];
            if source.run_index != glyph.run_index
                || source.source_range != glyph.source_range
                || source.line_index != glyph.line_index
                || source.cluster_index != glyph.cluster_index
                || source.logical_ordinal != glyph.logical_ordinal
            {
                return Err(TextLayoutContextError::SourceMap { index });
            }
        }
        for (index, ruby) in self.ruby.iter().enumerate() {
            validate_layout_source_range(ranges, text, ruby.base_range)?;
            let base_runs = ruby.base_runs.as_usize_range();
            if base_runs.start > base_runs.end || base_runs.end > self.runs.len() {
                return Err(TextLayoutContextError::MissingElement {
                    kind: "Ruby base runs",
                    index,
                });
            }
            validate_ruby_base_allocations(self, ruby, index)?;
            validate_context_rect("Ruby base", index, ruby.base_bounds)?;
            validate_context_rect("Ruby annotation", index, ruby.ruby_bounds)?;
            for glyph in &ruby.glyphs {
                let range = glyph.text_range;
                if range.start > range.end || ruby.text.get(range.start..range.end).is_none() {
                    return Err(TextLayoutContextError::RubyText { index });
                }
                validate_context_rect("Ruby glyph", index, glyph.layout_bounds)?;
                validate_context_rect("Ruby glyph ink", index, glyph.ink_bounds)?;
                if !glyph.origin.x.is_finite()
                    || !glyph.origin.y.is_finite()
                    || !glyph.advance.width.is_finite()
                    || !glyph.advance.height.is_finite()
                    || !glyph.inline_scale.is_finite()
                    || glyph.inline_scale <= 0.0
                {
                    return Err(TextLayoutContextError::Geometry {
                        kind: "Ruby glyph",
                        index,
                    });
                }
            }
        }
        if let Some(bounds) = self.bounds {
            validate_context_rect("layout", 0, bounds)?;
        }
        Ok(())
    }
}

fn validate_ruby_base_allocations(
    layout: &TextLayout,
    ruby: &TextLayoutRuby,
    index: usize,
) -> Result<(), TextLayoutContextError> {
    let failure = || TextLayoutContextError::RubyAllocation { index };
    let runs = ruby.base_runs.as_usize_range();
    let mut expected = layout.runs[runs.clone()]
        .iter()
        .flat_map(|run| run.glyph_range.clone())
        .filter(|glyph_index| {
            usize::try_from(*glyph_index)
                .ok()
                .and_then(|glyph_index| layout.glyphs.get(glyph_index))
                .is_some_and(|glyph| {
                    ruby.base_range.start <= glyph.source_range.start
                        && glyph.source_range.end <= ruby.base_range.end
                })
        });
    for allocation in &ruby.base_allocations {
        validate_glyph_interval(
            "Ruby body allocation",
            index,
            &allocation.glyph_range,
            layout.glyphs.len(),
        )?;
        validate_context_rect("Ruby body allocation", index, allocation.bounds)?;
        if allocation.glyph_range.start == allocation.glyph_range.end {
            return Err(failure());
        }
        let first = usize::try_from(allocation.glyph_range.start).map_err(|_| failure())?;
        let end = usize::try_from(allocation.glyph_range.end).map_err(|_| failure())?;
        let selected_cluster = layout.glyphs[first].cluster_index;
        if first
            .checked_sub(1)
            .and_then(|index| layout.glyphs.get(index))
            .is_some_and(|glyph| glyph.cluster_index == selected_cluster)
            || layout
                .glyphs
                .get(end)
                .is_some_and(|glyph| glyph.cluster_index == selected_cluster)
        {
            return Err(failure());
        }
        let mut cluster = None;
        let mut line = None;
        for glyph_index in allocation.glyph_range.clone() {
            if expected.next() != Some(glyph_index) {
                return Err(failure());
            }
            let glyph = &layout.glyphs[usize::try_from(glyph_index).map_err(|_| failure())?];
            if usize::try_from(glyph.run_index).map_or(true, |run| !runs.contains(&run))
                || cluster.is_some_and(|cluster| cluster != glyph.cluster_index)
                || line.is_some_and(|line| line != glyph.line_index)
                || allocation.bounds.x > glyph.layout_bounds.x
                || allocation.bounds.y > glyph.layout_bounds.y
                || allocation.bounds.right() < glyph.layout_bounds.right()
                || allocation.bounds.bottom() < glyph.layout_bounds.bottom()
            {
                return Err(failure());
            }
            cluster = Some(glyph.cluster_index);
            line = Some(glyph.line_index);
        }
    }
    if expected.next().is_some()
        || TextLayoutRuby::measure_base_bounds(&ruby.base_allocations, &layout.glyphs)
            != Some(ruby.base_bounds)
    {
        return Err(failure());
    }
    Ok(())
}

fn validate_layout_source_range(
    ranges: arcweft_text_model::ProjectedTextRangeDomain,
    text: &str,
    source: RichTextRange,
) -> Result<(), TextLayoutContextError> {
    let prepared = ranges.source_to_prepared(source)?;
    ranges.validate_prepared_range(prepared, text)?;
    Ok(())
}

fn validate_glyph_interval(
    kind: &'static str,
    index: usize,
    interval: &Range<u32>,
    glyphs: usize,
) -> Result<(), TextLayoutContextError> {
    if interval.start > interval.end || usize::try_from(interval.end).map_or(true, |n| n > glyphs) {
        Err(TextLayoutContextError::MissingElement { kind, index })
    } else {
        Ok(())
    }
}

fn validate_context_rect(
    kind: &'static str,
    index: usize,
    rect: LayoutRect,
) -> Result<(), TextLayoutContextError> {
    if !rect.x.is_finite()
        || !rect.y.is_finite()
        || !rect.width.is_finite()
        || !rect.height.is_finite()
        || !rect.right().is_finite()
        || !rect.bottom().is_finite()
        || rect.width < 0.0
        || rect.height < 0.0
    {
        Err(TextLayoutContextError::Geometry { kind, index })
    } else {
        Ok(())
    }
}
