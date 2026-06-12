use crate::segmentation::OrientedCluster;
use crate::unicode_orientation::ResolvedOrientation;
use glyphon_layout_ext_api::{FontKey, GlyphId, Point, Rect, Size, TextGlyphCacheKey, Vector};

/// A shaping run after orientation/script/font segmentation.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapePlanRun<'a> {
    pub clusters: &'a [OrientedCluster],
    pub orientation: ResolvedOrientation,
    pub font_size: f32,
}

/// Renderer-independent shaped glyph.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapedGlyph {
    pub cache_key: TextGlyphCacheKey,
    pub cluster_index: u32,
    pub advance: Vector,
    pub offset: Vector,
    pub ink_bounds: Rect,
    pub orientation: ResolvedOrientation,
}

pub trait ShapingBackend {
    fn shape_run(&self, run: ShapePlanRun<'_>) -> Vec<ShapedGlyph>;
}

/// Deterministic placeholder shaper.
///
/// It assigns a stable pseudo glyph id per char and fixed advances. Production
/// code should call a real shaping backend after font fallback and feature planning.
#[derive(Clone, Copy, Debug, Default)]
pub struct MonospaceShaper;

impl ShapingBackend for MonospaceShaper {
    fn shape_run(&self, run: ShapePlanRun<'_>) -> Vec<ShapedGlyph> {
        run.clusters
            .iter()
            .map(|cluster| {
                let first = cluster.text.chars().next().unwrap_or('\u{FFFD}');
                let glyph_id = GlyphId((u32::from(first) & 0xFFFF) as u16);
                let advance = match cluster.resolved_orientation {
                    ResolvedOrientation::Upright => Vector::new(0.0, run.font_size),
                    ResolvedOrientation::SidewaysClockwise
                    | ResolvedOrientation::SidewaysCounterClockwise => {
                        Vector::new(0.0, run.font_size * 0.6)
                    }
                };
                ShapedGlyph {
                    cache_key: TextGlyphCacheKey {
                        font: FontKey(0),
                        glyph: glyph_id,
                        subpixel_x: 0,
                        subpixel_y: 0,
                    },
                    cluster_index: cluster.cluster_index,
                    advance,
                    offset: Vector::ZERO,
                    ink_bounds: Rect::from_min_size(
                        Point::new(-run.font_size * 0.5, 0.0),
                        Size::new(run.font_size, run.font_size),
                    ),
                    orientation: cluster.resolved_orientation,
                }
            })
            .collect()
    }
}

pub fn shape_clusters<B: ShapingBackend>(
    clusters: &[OrientedCluster],
    font_size: f32,
    backend: &B,
) -> Vec<ShapedGlyph> {
    let mut shaped = Vec::new();
    let mut start = 0_usize;

    while start < clusters.len() {
        let orientation = clusters[start].resolved_orientation;
        let end = clusters[start..]
            .iter()
            .position(|cluster| cluster.resolved_orientation != orientation)
            .map_or(clusters.len(), |offset| start + offset);
        shaped.extend(backend.shape_run(ShapePlanRun {
            clusters: &clusters[start..end],
            orientation,
            font_size,
        }));
        start = end;
    }

    shaped
}

#[cfg(test)]
mod tests {
    use super::{MonospaceShaper, shape_clusters};
    use crate::segmentation::segment_and_orient;
    use crate::style::{TextLayoutStyle, WritingMode};

    #[test]
    fn shapes_all_clusters() {
        let style = TextLayoutStyle {
            writing_mode: WritingMode::VerticalRl,
            ..TextLayoutStyle::default()
        };
        let clusters = segment_and_orient("あA", style);
        let shaped = shape_clusters(&clusters, 16.0, &MonospaceShaper);
        assert_eq!(shaped.len(), 2);
    }
}
