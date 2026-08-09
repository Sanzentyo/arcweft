use crate::documents::{DocumentSnapshot, DocumentStore};
use crate::features::character_definition::{
    CharacterDefinitionDispatch, CharacterDefinitionRequestError, character_definition,
};
use crate::profiles::LspProfile;
use lsp_types::{GotoDefinitionResponse, Position, Uri};

/// Computes definition locations for the effective presentation context.
pub fn definition(
    profile: &LspProfile,
    _uri: &Uri,
    documents: &DocumentStore,
    document: &DocumentSnapshot,
    position: Position,
) -> Result<Option<GotoDefinitionResponse>, CharacterDefinitionRequestError> {
    let offset = document
        .line_index()
        .try_byte_offset_from_position(position)?;
    if let Some(result) = crate::features::dialogue_lines::definition(profile, document, offset) {
        return Ok(Some(result));
    }
    if let Some(result) = crate::features::entry_roles::definition(profile, document, offset) {
        return Ok(Some(result));
    }
    if let Some(result) = crate::features::nominal_types::definition(profile, document, offset) {
        return Ok(Some(result));
    }
    match character_definition(profile, documents, document, offset)? {
        CharacterDefinitionDispatch::Character(result) => return Ok(result),
        CharacterDefinitionDispatch::NotCharacter => {}
    }
    Ok(None)
}
