//! Canonical font-shaped layout output and source mapping.

use std::{fmt, ops::Range};

use arcweft_render_text::{
    ResolvedTextStyle, RichTextPresentation, RichTextRange, RichTextWritingMode,
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

/// Ruby placement tied to one canonical base range.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextLayoutRuby {
    pub ruby_index: u32,
    pub base_range: RichTextRange,
    pub text: String,
    pub base_bounds: LayoutRect,
    pub ruby_bounds: LayoutRect,
    pub glyphs: Vec<TextLayoutRubyGlyph>,
    pub writing_mode: RichTextWritingMode,
    pub style: ResolvedTextStyle,
    pub presentation: RichTextPresentation,
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
