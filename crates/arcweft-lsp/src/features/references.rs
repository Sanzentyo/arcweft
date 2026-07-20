use crate::documents::DocumentSnapshot;
use crate::features::cascade::{effective_dialogue_cascade_at, source_range};
use crate::features::view_part_metadata::ViewPartMetadataIndex;
use crate::profiles::LspProfile;
use arcweft_verify_lsp::LspPositionMapper;
use lsp_types::{Location, Position, Uri};
use std::collections::BTreeSet;

/// Lists source ranges that contribute to the effective dialogue style cascade.
pub fn references(
    profile: &LspProfile,
    uri: &Uri,
    document: &DocumentSnapshot,
    position: Position,
) -> Vec<Location> {
    let Ok(offset) = document
        .line_index()
        .try_byte_offset_from_position(position)
    else {
        return Vec::new();
    };
    if let Some(locations) = crate::features::entry_roles::references(profile, document, offset) {
        return locations;
    }
    if let Some(metadata) = ViewPartMetadataIndex::for_document(profile, document) {
        let locations = metadata
            .references(offset)
            .into_iter()
            .map(|range| {
                Location::new(
                    uri.clone(),
                    document
                        .line_index()
                        .range_from_byte_span(range.start(), range.end()),
                )
            })
            .collect::<Vec<_>>();
        if !locations.is_empty() {
            return locations;
        }
    }
    let Some(cascade) = effective_dialogue_cascade_at(document, offset) else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    let selected_contributions = cascade.selected_contributions();
    selected_contributions
        .iter()
        .copied()
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
