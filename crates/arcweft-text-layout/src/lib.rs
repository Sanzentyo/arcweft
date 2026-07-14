//! Sans I/O rich-text layout geometry for Arcweft players and agent debugging.
//!
//! This crate owns deterministic project-font text geometry. Renderer adapters
//! consume the canonical `TextLayout` without deriving bounds from pixels or
//! renderer-specific buffers.

mod config;
mod document_hash;
mod document_layout;
mod document_ruby;
mod geometry;
mod jlreq_punctuation;
mod jlreq_punctuation_data;
mod model;
mod shaping;
mod text_layout;
mod vertical_break;
mod vertical_clusters;
mod vertical_orientation;

pub use config::{HorizontalWrap, JlreqStrictness, TextLayoutError, TextLayoutRequest};
pub use document_layout::layout_document;
pub use geometry::{LayoutPoint, LayoutRect, LayoutSize};
pub use jlreq_punctuation_data::{
    JLREQ_PAIR_ADJUSTMENT_DATA_VERSION, JLREQ_PUNCTUATION_DATA_VERSION,
};
pub use model::{GlyphOrientation, GlyphVerticalForm};
pub use shaping::{
    FontFaceId, FontInventoryHash, ShapedGlyphKey, ShapedTextGlyph, ShapedTextRun,
    TextShapeRequest, TextShaper,
};
pub use text_layout::{
    TextLayout, TextLayoutGlyph, TextLayoutGlyphSource, TextLayoutHash, TextLayoutLine,
    TextLayoutRuby, TextLayoutRubyGlyph, TextLayoutRun, TextLayoutSourceMap,
};
pub use vertical_break::{
    MAX_VERTICAL_BREAK_CLUSTERS, VERTICAL_BREAK_UNITS_PER_EM, VerticalBreakCluster,
    VerticalBreakColumnExplain, VerticalBreakError, VerticalBreakExplain,
    VerticalBreakHardConstraint, VerticalBreakMetricRole, VerticalBreakPlan,
    VerticalBreakPlanStatus, VerticalBreakPolicy, VerticalBreakRejectionCounts, VerticalBreakScore,
    VerticalBreakTieBreakReason, plan_vertical_breaks,
};
pub use vertical_orientation::UNICODE_VERTICAL_ORIENTATION_VERSION;
