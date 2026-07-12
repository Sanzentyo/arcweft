//! Sans I/O rich-text layout geometry for Arcweft players and agent debugging.
//!
//! This crate owns deterministic text geometry. Renderer adapters consume the
//! resulting `LaidOutText` instead of deriving bounds from pixels or from
//! renderer-specific buffers.

mod config;
mod document_hash;
mod document_layout;
mod document_ruby;
mod document_vertical;
mod effects;
mod geometry;
mod horizontal;
mod jlreq_punctuation;
mod jlreq_punctuation_data;
mod layout;
mod model;
mod ruby;
mod ruby_metrics;
mod shaping;
mod text_layout;
mod vertical;
mod vertical_breaks;
mod vertical_clusters;
mod vertical_columns;
mod vertical_orientation;

pub use config::{
    HorizontalWrap, JlreqStrictness, TextLayoutConfig, TextLayoutError, TextLayoutRequest,
};
pub use document_layout::layout_document;
pub use geometry::{LayoutPoint, LayoutRect, LayoutSize};
pub use jlreq_punctuation_data::{
    JLREQ_PAIR_ADJUSTMENT_DATA_VERSION, JLREQ_PUNCTUATION_DATA_VERSION,
};
pub use layout::layout_frame;
pub use model::{
    GlyphOrientation, GlyphVerticalForm, LaidOutGlyph, LaidOutRuby, LaidOutRun, LaidOutText,
};
pub use shaping::{
    FontFaceId, FontInventoryHash, ShapedGlyphKey, ShapedTextGlyph, ShapedTextRun,
    TextShapeRequest, TextShaper,
};
pub use text_layout::{
    TextLayout, TextLayoutGlyph, TextLayoutGlyphSource, TextLayoutHash, TextLayoutLine,
    TextLayoutRuby, TextLayoutRubyGlyph, TextLayoutRun, TextLayoutSourceMap,
};
pub use vertical_orientation::UNICODE_VERTICAL_ORIENTATION_VERSION;

#[cfg(test)]
mod tests;
