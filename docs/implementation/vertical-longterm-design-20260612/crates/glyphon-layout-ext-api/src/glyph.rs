use crate::color::Color;
use crate::geom::{Affine2, Point, Rect, Vector};

/// Renderer-independent font handle placeholder.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FontKey(pub u32);

/// Font-local glyph identifier.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GlyphId(pub u16);

/// Renderer-specific custom glyph identifier.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CustomGlyphId(pub u64);

/// Placeholder for the key glyphon would use to locate a cached text glyph.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TextGlyphCacheKey {
    pub font: FontKey,
    pub glyph: GlyphId,
    pub subpixel_x: u8,
    pub subpixel_y: u8,
}

/// Byte range in the flattened text stream.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

impl TextRange {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

/// Logical cluster metadata preserved for hit-test and Agent observation.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct TextCluster {
    pub logical_range: TextRange,
    pub cluster_index: u32,
}

/// Source of one renderable glyph.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GlyphSource {
    Text { cache_key: TextGlyphCacheKey },
    Custom { id: CustomGlyphId },
}

impl Default for GlyphSource {
    fn default() -> Self {
        Self::Text {
            cache_key: TextGlyphCacheKey::default(),
        }
    }
}

/// Per-glyph transform.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum GlyphTransform {
    #[default]
    Identity,
    Rotate90Cw,
    Rotate90Ccw,
    Affine(Affine2),
}

impl GlyphTransform {
    pub fn as_affine(self) -> Affine2 {
        match self {
            Self::Identity => Affine2::IDENTITY,
            Self::Rotate90Cw => Affine2::rotate_90_cw(),
            Self::Rotate90Ccw => Affine2::rotate_90_ccw(),
            Self::Affine(affine) => affine,
        }
    }
}

/// A fully positioned glyph quad request.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphInstance {
    pub source: GlyphSource,
    pub origin: Point,
    pub advance: Vector,
    pub ink_bounds: Rect,
    pub transform: GlyphTransform,
    pub color: Option<Color>,
    pub metadata: usize,
    pub cluster: Option<TextCluster>,
}

impl GlyphInstance {
    pub fn transformed_ink_bounds(&self) -> Rect {
        let translate = Affine2::translation(Vector::new(self.origin.x, self.origin.y));
        self.transform
            .as_affine()
            .then(translate)
            .transform_rect_aabb(self.ink_bounds)
    }
}

/// A renderer-facing batch of pre-laid glyphs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphArea<'a> {
    pub glyphs: &'a [GlyphInstance],
    pub left: f32,
    pub top: f32,
    pub scale: f32,
    pub bounds: Rect,
    pub default_color: Color,
}

impl<'a> GlyphArea<'a> {
    pub const fn new(glyphs: &'a [GlyphInstance], bounds: Rect) -> Self {
        Self {
            glyphs,
            left: 0.0,
            top: 0.0,
            scale: 1.0,
            bounds,
            default_color: Color::WHITE,
        }
    }

    pub fn visible_glyphs(self) -> impl Iterator<Item = &'a GlyphInstance> {
        self.glyphs
            .iter()
            .filter(move |glyph| glyph.transformed_ink_bounds().intersects(self.bounds))
    }
}

#[cfg(test)]
mod tests {
    use super::{GlyphArea, GlyphInstance, GlyphSource, GlyphTransform, TextGlyphCacheKey};
    use crate::geom::{Point, Rect, Size, Vector};

    fn glyph_at(origin: Point) -> GlyphInstance {
        GlyphInstance {
            source: GlyphSource::Text {
                cache_key: TextGlyphCacheKey::default(),
            },
            origin,
            advance: Vector::new(10.0, 0.0),
            ink_bounds: Rect::from_min_size(Point::new(0.0, 0.0), Size::new(8.0, 8.0)),
            transform: GlyphTransform::Identity,
            color: None,
            metadata: 0,
            cluster: None,
        }
    }

    #[test]
    fn visible_glyphs_filters_by_transformed_bounds() {
        let glyphs = [glyph_at(Point::new(0.0, 0.0)), glyph_at(Point::new(100.0, 100.0))];
        let area = GlyphArea::new(
            &glyphs,
            Rect::from_min_size(Point::new(-1.0, -1.0), Size::new(20.0, 20.0)),
        );
        assert_eq!(area.visible_glyphs().count(), 1);
    }
}
