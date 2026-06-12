use glyphon_layout_ext_api::{GlyphInstance, Point, Rect, TextCluster, Vector};

/// A glyph after shaping and layout, before renderer-specific packing.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedGlyph {
    pub glyph: GlyphInstance,
    pub logical_origin: Point,
}

/// One logical line/column box.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LineBox {
    pub glyph_range_start: usize,
    pub glyph_range_end: usize,
    pub inline_start: f32,
    pub inline_size: f32,
    pub block_start: f32,
    pub block_size: f32,
}

/// Layout result consumed by renderer adapters and input systems.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaidOutText {
    pub glyphs: Vec<PlacedGlyph>,
    pub lines: Vec<LineBox>,
}

impl LaidOutText {
    pub fn glyph_instances(&self) -> Vec<GlyphInstance> {
        self.glyphs.iter().map(|placed| placed.glyph.clone()).collect()
    }

    pub fn cluster_bounds(&self, cluster: TextCluster) -> Option<Rect> {
        self.glyphs
            .iter()
            .filter(|placed| placed.glyph.cluster == Some(cluster))
            .map(|placed| placed.glyph.transformed_ink_bounds())
            .reduce(Rect::union)
    }

    pub fn advance_for_cluster(&self, cluster: TextCluster) -> Vector {
        self.glyphs
            .iter()
            .filter(|placed| placed.glyph.cluster == Some(cluster))
            .fold(Vector::ZERO, |acc, placed| Vector {
                x: acc.x + placed.glyph.advance.x,
                y: acc.y + placed.glyph.advance.y,
            })
    }
}
