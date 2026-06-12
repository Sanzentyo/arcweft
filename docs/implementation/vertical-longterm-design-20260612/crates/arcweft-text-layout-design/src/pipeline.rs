use crate::line_break::{LayoutItem, break_lines_dp};
use crate::model::{LaidOutText, LineBox, PlacedGlyph};
use crate::segmentation::segment_and_orient;
use crate::shaping::{MonospaceShaper, shape_clusters};
use crate::style::{TextLayoutStyle, WritingMode};
use crate::unicode_orientation::ResolvedOrientation;
use glyphon_layout_ext_api::{
    GlyphInstance, GlyphSource, GlyphTransform, Point, Rect, Size, TextCluster, Vector,
};

/// Input for one paragraph of text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParagraphLayoutInput<'a> {
    pub text: &'a str,
    pub style: TextLayoutStyle,
}

/// End-to-end design skeleton layout.
pub fn layout_paragraph(input: ParagraphLayoutInput<'_>) -> LaidOutText {
    let clusters = segment_and_orient(input.text, input.style);
    let shaped = shape_clusters(&clusters, input.style.font_size, &MonospaceShaper);
    let items = shaped
        .iter()
        .map(|glyph| LayoutItem::glyph(glyph.cluster_index, glyph.advance.y.abs().max(glyph.advance.x.abs())))
        .collect::<Vec<_>>();
    let breaks = break_lines_dp(&items, input.style.max_inline);

    let mut placed = Vec::new();
    let mut lines = Vec::new();
    let block_advance = input.style.font_size + input.style.column_gap;

    for (line_index, line) in breaks.iter().enumerate() {
        let glyph_start = placed.len();
        let block_start = line_index as f32 * block_advance;
        let mut inline_cursor = 0.0;

        for glyph in &shaped[line.start..line.end] {
            let cluster = clusters
                .iter()
                .find(|cluster| cluster.cluster_index == glyph.cluster_index)
                .expect("shaped glyph cluster exists");
            let origin = physical_origin(input.style, block_start, inline_cursor);
            let transform = match glyph.orientation {
                ResolvedOrientation::Upright => GlyphTransform::Identity,
                ResolvedOrientation::SidewaysClockwise => GlyphTransform::Rotate90Cw,
                ResolvedOrientation::SidewaysCounterClockwise => GlyphTransform::Rotate90Ccw,
            };
            placed.push(PlacedGlyph {
                glyph: GlyphInstance {
                    source: GlyphSource::Text {
                        cache_key: glyph.cache_key,
                    },
                    origin,
                    advance: glyph.advance,
                    ink_bounds: glyph.ink_bounds,
                    transform,
                    color: None,
                    metadata: glyph.cluster_index as usize,
                    cluster: Some(TextCluster {
                        logical_range: cluster.range,
                        cluster_index: glyph.cluster_index,
                    }),
                },
                logical_origin: Point::new(block_start, inline_cursor),
            });
            inline_cursor += glyph.advance.y.abs().max(glyph.advance.x.abs());
        }

        lines.push(LineBox {
            glyph_range_start: glyph_start,
            glyph_range_end: placed.len(),
            inline_start: 0.0,
            inline_size: line.used_inline,
            block_start,
            block_size: input.style.font_size,
        });
    }

    LaidOutText { glyphs: placed, lines }
}

fn physical_origin(style: TextLayoutStyle, block_start: f32, inline_start: f32) -> Point {
    match style.writing_mode {
        WritingMode::HorizontalTb => Point::new(inline_start, block_start),
        WritingMode::VerticalRl => Point::new(-block_start, inline_start),
        WritingMode::VerticalLr => Point::new(block_start, inline_start),
    }
}

/// Converts a layout result into a standalone glyph vector suitable for a `GlyphArea`.
pub fn glyph_area_bounds(layout: &LaidOutText) -> Rect {
    layout
        .glyphs
        .iter()
        .map(|placed| placed.glyph.transformed_ink_bounds())
        .reduce(Rect::union)
        .unwrap_or_else(|| Rect::from_min_size(Point::ZERO, Size::ZERO))
}

#[cfg(test)]
mod tests {
    use super::{ParagraphLayoutInput, layout_paragraph};
    use crate::style::{TextLayoutStyle, WritingMode};
    use glyphon_layout_ext_api::GlyphTransform;

    #[test]
    fn vertical_mixed_places_ascii_sideways() {
        let style = TextLayoutStyle {
            writing_mode: WritingMode::VerticalRl,
            max_inline: 200.0,
            ..TextLayoutStyle::default()
        };
        let layout = layout_paragraph(ParagraphLayoutInput { text: "あA", style });
        assert_eq!(layout.glyphs.len(), 2);
        assert_eq!(layout.glyphs[1].glyph.transform, GlyphTransform::Rotate90Cw);
    }
}
