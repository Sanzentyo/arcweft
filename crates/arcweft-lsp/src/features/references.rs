use crate::documents::DocumentSnapshot;
use crate::profiles::LspProfile;
use lsp_types::{Location, Position, Uri};

/// Lists accepted semantic references for the symbol at the requested position.
pub fn references(
    profile: &LspProfile,
    _uri: &Uri,
    document: &DocumentSnapshot,
    position: Position,
) -> Vec<Location> {
    let Ok(offset) = document
        .line_index()
        .try_byte_offset_from_position(position)
    else {
        return Vec::new();
    };
    if let Some(locations) = crate::features::dialogue_lines::references(profile, document, offset)
    {
        return locations;
    }
    if let Some(locations) = crate::features::entry_roles::references(profile, document, offset) {
        return locations;
    }
    if let Some(locations) = crate::features::nominal_types::references(profile, document, offset) {
        return locations;
    }
    Vec::new()
}
