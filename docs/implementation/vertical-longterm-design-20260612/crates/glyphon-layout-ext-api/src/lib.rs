#![forbid(unsafe_code)]
//! API sketch for a long-term glyphon extension that accepts pre-laid glyphs.
//!
//! This crate intentionally has no dependency on `glyphon` or `wgpu`. It captures
//! the public API shape that should be upstreamed or mirrored by an adapter crate.

pub mod color;
pub mod geom;
pub mod glyph;
pub mod renderer;

pub use color::Color;
pub use geom::{Affine2, Point, Px, Rect, Size, Vector};
pub use glyph::{
    CustomGlyphId, FontKey, GlyphArea, GlyphId, GlyphInstance, GlyphSource, GlyphTransform,
    TextCluster, TextGlyphCacheKey, TextRange,
};
pub use renderer::TextRendererGlyphAreaExt;
