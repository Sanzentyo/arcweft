use crate::model::LaidOutText;
use glyphon_layout_ext_api::{Point, TextCluster};

/// Hit-test result for one point in text-local coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HitTestResult {
    pub cluster: TextCluster,
    pub after: bool,
}

/// Derived hit-test map.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HitMap {
    pub cells: Vec<HitTestCell>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitTestCell {
    pub cluster: TextCluster,
    pub center: Point,
    pub inline_midpoint: f32,
}

impl HitMap {
    pub fn from_layout(layout: &LaidOutText) -> Self {
        let cells = layout
            .glyphs
            .iter()
            .filter_map(|placed| {
                let cluster = placed.glyph.cluster?;
                let bounds = placed.glyph.transformed_ink_bounds();
                Some(HitTestCell {
                    cluster,
                    center: Point::new(
                        (bounds.min.x + bounds.max.x) * 0.5,
                        (bounds.min.y + bounds.max.y) * 0.5,
                    ),
                    inline_midpoint: (bounds.min.y + bounds.max.y) * 0.5,
                })
            })
            .collect();
        Self { cells }
    }

    pub fn hit_test(&self, point: Point) -> Option<HitTestResult> {
        self.cells
            .iter()
            .min_by(|left, right| {
                squared_distance(left.center, point)
                    .total_cmp(&squared_distance(right.center, point))
            })
            .map(|cell| HitTestResult {
                cluster: cell.cluster,
                after: point.y >= cell.inline_midpoint,
            })
    }
}

fn squared_distance(a: Point, b: Point) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx.mul_add(dx, dy * dy)
}
