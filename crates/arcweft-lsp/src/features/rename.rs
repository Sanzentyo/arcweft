//! Rename support for ordinary declarations and entry identities.

use lsp_types::{Position, PrepareRenameResponse, WorkspaceEdit};

use crate::{
    documents::{DocumentSnapshot, DocumentStore},
    profiles::LspProfile,
};

pub(crate) fn prepare(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    position: Position,
) -> Option<PrepareRenameResponse> {
    let offset = document
        .line_index()
        .try_byte_offset_from_position(position)
        .ok()?;
    crate::features::entry_roles::prepare_rename(profile, document, offset)
}

pub(crate) fn rename(
    profile: &LspProfile,
    documents: &DocumentStore,
    document: &DocumentSnapshot,
    position: Position,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    let offset = document
        .line_index()
        .try_byte_offset_from_position(position)
        .ok()?;
    crate::features::entry_roles::rename(profile, documents, document, offset, new_name)
}
