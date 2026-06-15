use crate::documents::DocumentSnapshot;
use crate::features::cascade::{effective_dialogue_cascade_at, source_range};
use crate::profiles::LspProfile;
use arcweft_verify_lsp::LspPositionMapper;
use lsp_types::{GotoDefinitionResponse, Location, Position, Uri};

/// Computes definition locations for the effective presentation context.
pub fn definition(
    profile: &LspProfile,
    uri: &Uri,
    document: &DocumentSnapshot,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let offset = document.line_index().byte_offset_from_position(position);
    let cascade = effective_dialogue_cascade_at(document, offset, profile.dialogue_defaults())?;
    let locations = cascade
        .selected_contributions()
        .into_iter()
        .filter(|contribution| contribution.active)
        .filter_map(|contribution| source_range(&contribution.source))
        .map(|range| {
            Location::new(
                uri.clone(),
                document
                    .line_index()
                    .range_from_byte_span(range.start, range.end),
            )
        })
        .collect::<Vec<_>>();

    (!locations.is_empty()).then_some(GotoDefinitionResponse::Array(locations))
}
