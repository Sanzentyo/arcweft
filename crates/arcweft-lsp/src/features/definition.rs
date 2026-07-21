use crate::documents::{DocumentSnapshot, DocumentStore};
use crate::features::cascade::{effective_dialogue_cascade_at, source_range};
use crate::features::character_definition::{
    CharacterDefinitionDispatch, CharacterDefinitionRequestError, character_definition,
};
use crate::features::view_part_metadata::ViewPartMetadataIndex;
use crate::profiles::LspProfile;
use arcweft_verify_lsp::LspPositionMapper;
use lsp_types::{GotoDefinitionResponse, Location, Position, Uri};

/// Computes definition locations for the effective presentation context.
pub fn definition(
    profile: &LspProfile,
    uri: &Uri,
    documents: &DocumentStore,
    document: &DocumentSnapshot,
    position: Position,
) -> Result<Option<GotoDefinitionResponse>, CharacterDefinitionRequestError> {
    let offset = document
        .line_index()
        .try_byte_offset_from_position(position)?;
    if let Some(result) = crate::features::entry_roles::definition(profile, document, offset) {
        return Ok(Some(result));
    }
    match character_definition(profile, documents, document, offset)? {
        CharacterDefinitionDispatch::Character(result) => return Ok(result),
        CharacterDefinitionDispatch::NotCharacter => {}
    }
    Ok(presentation_definition(profile, uri, document, offset))
}

fn presentation_definition(
    profile: &LspProfile,
    uri: &Uri,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<GotoDefinitionResponse> {
    if let Some(metadata) = ViewPartMetadataIndex::for_document(profile, document) {
        let locations = metadata
            .definitions(offset)
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
            return Some(GotoDefinitionResponse::Array(locations));
        }
    }
    let cascade = effective_dialogue_cascade_at(profile, document, offset)?;
    let selected_contributions = cascade.selected_contributions();
    let locations = selected_contributions
        .iter()
        .copied()
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
