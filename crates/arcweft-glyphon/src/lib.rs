//! Shared project-font shaping, raster-key preparation, and prepared glyph data.

mod prepared_text;
mod text_engine;

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
