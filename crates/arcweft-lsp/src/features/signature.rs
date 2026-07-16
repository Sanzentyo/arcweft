use crate::documents::DocumentSnapshot;
use crate::features::hover::word_at_position;
use crate::profiles::LspProfile;
use arcweft_verify_lsp::rust_adapter_signature_help;
use lsp_types::{Position, SignatureHelp};

/// Computes signature help from Rust adapter metadata.
pub fn signature_help(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    position: Position,
) -> Option<SignatureHelp> {
    let offset = document
        .line_index()
        .try_byte_offset_from_position(position)
        .ok()?;
    if let Some(help) = crate::features::entry_roles::signature_help(profile, document, offset) {
        return Some(help);
    }
    let word = word_at_position(document, position)?;
    rust_adapter_signature_help(&profile.context(), &word)
}
