//! Shared project-font shaping, raster-key preparation, and prepared glyph data.

mod physical_bounds;
mod prepared_text;
mod text_engine;

pub use physical_bounds::{
    PreparedTextBoundsComponent, PreparedTextBoundsEdge, PreparedTextPhysicalBounds,
    PreparedTextPhysicalBoundsError,
};
pub use prepared_text::{
    PreparedGlyph, PreparedGlyphSource, PreparedTextAffine, PreparedTextBatch, PreparedTextError,
    PreparedTextId, PreparedTextItem, PreparedTextSubmission, TextCaretPaint, TextCharacterBounds,
    TextCompositionUnderline, TextGlyphPaint, TextGlyphTransform, TextInteractionPlan,
    TextPaintPlan,
};
pub use text_engine::{
    GlyphRasterKey, GlyphonTextEngine, GlyphonTextEngineError, TextShapeCacheLimits,
    TextShapeCacheStats,
};
