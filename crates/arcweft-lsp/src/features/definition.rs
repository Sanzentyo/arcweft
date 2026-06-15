use crate::documents::DocumentSnapshot;
use crate::features::cascade::{effective_dialogue_cascade_at, source_range};
use crate::positions::LineIndex;
use crate::profiles::LspProfile;
use arcweft_render_text::{RichTextCascadeLayer, RichTextStyleContribution};
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
    let selected_contributions = cascade.selected_contributions();
    let mut locations = selected_contributions
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
    if uses_profile_selected_dialogue_defaults(&selected_contributions)
        && let Some(location) = dialogue_defaults_selection_location(profile, document)
    {
        locations.push(location);
    }

    (!locations.is_empty()).then_some(GotoDefinitionResponse::Array(locations))
}

fn uses_profile_selected_dialogue_defaults(contributions: &[&RichTextStyleContribution]) -> bool {
    contributions
        .iter()
        .any(|contribution| contribution.layer == RichTextCascadeLayer::DialogueDefaults)
}

fn dialogue_defaults_selection_location(
    profile: &LspProfile,
    document: &DocumentSnapshot,
) -> Option<Location> {
    let selection = profile.dialogue_defaults_selection()?;
    let uri = selection.uri()?;
    let line_index = LineIndex::new(
        selection.source().to_owned(),
        document.line_index().position_encoding(),
    );
    let range = selection.value_range();
    Some(Location::new(
        uri,
        line_index.range_from_byte_span(range.start, range.end),
    ))
}
