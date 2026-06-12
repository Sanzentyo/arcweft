//! Glyphon provides a simple way to render 2D text with [wgpu], [cosmic-text] and [etagere].
//!
//! [wpgu]: https://github.com/gfx-rs/wgpu
//! [cosmic-text]: https://github.com/pop-os/cosmic-text
//! [etagere]: https://github.com/nical/etagere

mod cache;
mod custom_glyph;
mod error;
mod text_atlas;
mod text_render;
mod viewport;

pub use cache::Cache;
pub use custom_glyph::{
    ContentType, CustomGlyph, CustomGlyphId, RasterizeCustomGlyphRequest, RasterizedCustomGlyph,
};
pub use error::{PrepareError, RenderError};
pub use text_atlas::{ColorMode, TextAtlas};
pub use text_render::TextRenderer;
pub use viewport::Viewport;

// Re-export all top-level types from `cosmic-text` for convenience.
#[doc(no_inline)]
pub use cosmic_text::{
    self, fontdb, Action, Affinity, Attrs, AttrsList, AttrsOwned, Buffer, BufferLine, CacheKey,
    Color, Command, Cursor, Edit, Editor, Family, FamilyOwned, Font, FontSystem, LayoutCursor,
    LayoutGlyph, LayoutLine, LayoutRun, LayoutRunIter, Metrics, ShapeGlyph, ShapeLine, ShapeSpan,
    ShapeWord, Shaping, Stretch, Style, SubpixelBin, SwashCache, SwashContent, SwashImage, Weight,
    Wrap,
};

use etagere::AllocId;
use wgpu::{Device, Queue};

pub(crate) enum GpuCacheStatus {
    InAtlas {
        x: u16,
        y: u16,
        content_type: ContentType,
    },
    SkipRasterization,
}

pub(crate) struct GlyphDetails {
    width: u16,
    height: u16,
    gpu_cache: GpuCacheStatus,
    atlas_id: Option<AllocId>,
    top: i16,
    left: i16,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct GlyphToRender {
    pos: [i32; 2],
    dim: [u16; 2],
    uv: [u16; 2],
    color: u32,
    content_type_with_srgb: [u16; 2],
    depth: f32,
    transform: [f32; 6],
    clip_bounds: [f32; 4],
}

/// The screen resolution to use when rendering text.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Resolution {
    /// The width of the screen in pixels.
    pub width: u32,
    /// The height of the screen in pixels.
    pub height: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Params {
    screen_resolution: Resolution,
    _pad: [u32; 2],
}

/// Controls the visible area of the text. Any text outside of the visible area will be clipped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextBounds {
    /// The position of the left edge of the visible area.
    pub left: i32,
    /// The position of the top edge of the visible area.
    pub top: i32,
    /// The position of the right edge of the visible area.
    pub right: i32,
    /// The position of the bottom edge of the visible area.
    pub bottom: i32,
}

/// The default visible area doesn't clip any text.
impl Default for TextBounds {
    fn default() -> Self {
        Self {
            left: i32::MIN,
            top: i32::MIN,
            right: i32::MAX,
            bottom: i32::MAX,
        }
    }
}

/// Two-dimensional point in glyph area coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
}

impl Point {
    /// Creates a point.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Two-dimensional vector in glyph area coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
}

impl Vector {
    /// Creates a vector.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Axis-aligned rectangle in glyph-local coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub left: f32,
    /// Top edge.
    pub top: f32,
    /// Right edge.
    pub right: f32,
    /// Bottom edge.
    pub bottom: f32,
}

impl Rect {
    /// Creates a rectangle from edges.
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// Rectangle width.
    pub fn width(self) -> f32 {
        (self.right - self.left).max(0.0)
    }

    /// Rectangle height.
    pub fn height(self) -> f32 {
        (self.bottom - self.top).max(0.0)
    }
}

/// Affine glyph-local transform represented as `[a, b, c, d, e, f]`.
///
/// A local point `(x, y)` maps to `(a*x + b*y + e, c*x + d*y + f)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine2 {
    /// Matrix and translation values.
    pub values: [f32; 6],
}

impl Affine2 {
    /// Identity transform.
    pub const IDENTITY: Self = Self {
        values: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    };

    /// Creates an affine transform from values.
    pub const fn new(values: [f32; 6]) -> Self {
        Self { values }
    }
}

/// Per-glyph transform applied by the renderer vertex path.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum GlyphTransform {
    /// No transform.
    #[default]
    Identity,
    /// Rotate local glyph quad 90 degrees clockwise.
    Rotate90Cw,
    /// Rotate local glyph quad 90 degrees counter-clockwise.
    Rotate90Ccw,
    /// Full affine glyph-local transform.
    Affine(Affine2),
}

impl GlyphTransform {
    pub(crate) fn matrix_for_size(self, width: f32, height: f32) -> [f32; 6] {
        match self {
            Self::Identity => Affine2::IDENTITY.values,
            Self::Rotate90Cw => [0.0, 1.0, -1.0, 0.0, 0.0, width],
            Self::Rotate90Ccw => [0.0, -1.0, 1.0, 0.0, height, 0.0],
            Self::Affine(affine) => affine.values,
        }
    }
}

/// Logical cluster metadata retained for renderer consumers.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct TextCluster {
    /// Byte start in the flattened text stream.
    pub start: usize,
    /// Byte end in the flattened text stream.
    pub end: usize,
    /// Stable cluster index in layout order.
    pub index: u32,
}

/// Source of one pre-laid glyph.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GlyphSource {
    /// A glyph already shaped by cosmic-text/swash.
    Text { cache_key: CacheKey },
    /// A renderer-specific custom glyph.
    Custom { id: CustomGlyphId },
}

/// One pre-laid glyph submitted to `GlyphArea`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphInstance {
    /// Glyph raster source.
    pub source: GlyphSource,
    /// Glyph origin in area-local coordinates.
    pub origin: Point,
    /// Logical advance retained for hit testing and observations.
    pub advance: Vector,
    /// Glyph-local ink bounds.
    pub ink_bounds: Rect,
    /// Glyph-local transform.
    pub transform: GlyphTransform,
    /// Optional glyph color overriding the area default.
    pub color: Option<Color>,
    /// Renderer-opaque metadata forwarded to depth/object-id callbacks.
    pub metadata: usize,
    /// Optional logical cluster metadata.
    pub cluster: Option<TextCluster>,
}

/// A batch of pre-laid glyphs rendered by glyphon without using `Buffer` layout.
#[derive(Clone, Copy)]
pub struct GlyphArea<'a> {
    /// Pre-laid glyph instances.
    pub glyphs: &'a [GlyphInstance],
    /// Left edge of the area.
    pub left: f32,
    /// Top edge of the area.
    pub top: f32,
    /// Scale applied to glyph positions and glyph quads.
    pub scale: f32,
    /// Visible bounds for clipping.
    pub bounds: TextBounds,
    /// Default color of the glyph area.
    pub default_color: Color,
}

/// A text area containing text to be rendered along with its overflow behavior.
#[derive(Clone)]
pub struct TextArea<'a> {
    /// The buffer containing the text to be rendered.
    pub buffer: &'a Buffer,
    /// The left edge of the buffer.
    pub left: f32,
    /// The top edge of the buffer.
    pub top: f32,
    /// The scaling to apply to the buffer.
    pub scale: f32,
    /// The visible bounds of the text area. This is used to clip the text and doesn't have to
    /// match the `left` and `top` values.
    pub bounds: TextBounds,
    /// The default color of the text area.
    pub default_color: Color,
    /// Additional custom glyphs to render.
    pub custom_glyphs: &'a [CustomGlyph],
}

pub(crate) struct State<'a> {
    pub(crate) device: &'a Device,
    pub(crate) queue: &'a Queue,
}

#[cfg(test)]
mod tests {
    use super::{Affine2, GlyphTransform};

    #[test]
    fn glyph_transform_matrices_cover_identity_and_quarter_turns() {
        assert_eq!(
            GlyphTransform::Identity.matrix_for_size(10.0, 20.0),
            Affine2::IDENTITY.values
        );
        assert_eq!(
            GlyphTransform::Rotate90Cw.matrix_for_size(10.0, 20.0),
            [0.0, 1.0, -1.0, 0.0, 0.0, 10.0]
        );
        assert_eq!(
            GlyphTransform::Rotate90Ccw.matrix_for_size(10.0, 20.0),
            [0.0, -1.0, 1.0, 0.0, 20.0, 0.0]
        );
    }
}
