use crate::style::TextLayoutStyle;
use crate::unicode_orientation::{ResolvedOrientation, VerticalOrientation, resolve_for_char, vertical_orientation};
use glyphon_layout_ext_api::TextRange;

/// One oriented text cluster.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrientedCluster {
    pub range: TextRange,
    pub cluster_index: u32,
    pub text: String,
    pub vertical_orientation: VerticalOrientation,
    pub resolved_orientation: ResolvedOrientation,
    pub requires_vertical_alternate: bool,
}

/// Design skeleton segmentation.
///
/// Production must use UAX #29 grapheme cluster segmentation. This function uses
/// Rust `char_indices` so tests can exercise later pipeline stages without a data dependency.
pub fn segment_and_orient(text: &str, style: TextLayoutStyle) -> Vec<OrientedCluster> {
    let mut indices = text.char_indices().peekable();
    let mut cluster_index = 0_u32;
    let mut clusters = Vec::new();

    while let Some((start, ch)) = indices.next() {
        let end = indices.peek().map_or(text.len(), |(next, _)| *next);
        let vertical_orientation = vertical_orientation(ch);
        clusters.push(OrientedCluster {
            range: TextRange::new(start, end),
            cluster_index,
            text: text[start..end].to_owned(),
            vertical_orientation,
            resolved_orientation: resolve_for_char(ch, style),
            requires_vertical_alternate: matches!(
                vertical_orientation,
                VerticalOrientation::TransformedUpright | VerticalOrientation::TransformedRotated
            ),
        });
        cluster_index = cluster_index.saturating_add(1);
    }

    clusters
}

#[cfg(test)]
mod tests {
    use super::segment_and_orient;
    use crate::style::{TextLayoutStyle, WritingMode};
    use crate::unicode_orientation::ResolvedOrientation;

    #[test]
    fn produces_byte_ranges() {
        let style = TextLayoutStyle {
            writing_mode: WritingMode::VerticalRl,
            ..TextLayoutStyle::default()
        };
        let clusters = segment_and_orient("あA", style);
        assert_eq!(clusters[0].range.start, 0);
        assert_eq!(clusters[0].range.end, "あ".len());
        assert_eq!(clusters[1].resolved_orientation, ResolvedOrientation::SidewaysClockwise);
    }
}
