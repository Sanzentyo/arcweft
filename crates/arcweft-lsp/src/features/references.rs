use crate::documents::DocumentSnapshot;
use crate::features::cascade::{effective_dialogue_cascade_at, source_range};
use arcweft_verify_lsp::LspPositionMapper;
use lsp_types::{Location, Position, Uri};
use std::collections::BTreeSet;

/// Lists source ranges that contribute to the effective dialogue style cascade.
pub fn references(uri: &Uri, document: &DocumentSnapshot, position: Position) -> Vec<Location> {
    let offset = document.line_index().byte_offset_from_position(position);
    let Some(cascade) = effective_dialogue_cascade_at(document, offset) else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    cascade
        .spec
        .style_contributions
        .iter()
        .filter_map(|contribution| source_range(&contribution.source))
        .filter(|range| seen.insert((range.start, range.end)))
        .map(|range| {
            Location::new(
                uri.clone(),
                document
                    .line_index()
                    .range_from_byte_span(range.start, range.end),
            )
        })
        .collect()
}
