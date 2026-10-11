//! Admitted Agent object ownership projected from a canonical prepared-text owner.

use arcweft_render_wgpu::geometry::PreparedTextOwner;
use arcweft_text_model::{DialoguePreparedTextRole, PreparedTextOwnerKind};

use super::agent_uri_component;

#[cfg(test)]
mod tests;

/// Borrows the renderer's owner and applies the Agent object-identity contract.
/// Every rendered mount/text target has a distinct observation root. Selection
/// belongs to its issued context reference, not the descriptor's spelling.
#[derive(Debug)]
pub(in super::super) struct PreparedTextObservationOwner<'frame> {
    prepared: &'frame PreparedTextOwner,
    root_id: String,
}

impl<'frame> PreparedTextObservationOwner<'frame> {
    pub(in super::super) fn new(prepared: &'frame PreparedTextOwner) -> Self {
        let root_id = match prepared.kind {
            PreparedTextOwnerKind::DialogueView {
                dialogue,
                entry,
                role: DialoguePreparedTextRole::Content,
                mount,
            } => format!(
                "object.dialogue.{dialogue}.{entry}.mount.{mount}.text.{}.{}",
                prepared.text.index(),
                agent_uri_component(prepared.semantic_id.as_str())
            ),
            PreparedTextOwnerKind::DialogueView {
                role: DialoguePreparedTextRole::CharacterDisplayName,
                dialogue,
                entry,
                mount,
            } => format!(
                "object.dialogue.{dialogue}.{entry}.mount.{mount}.name.{}.{}",
                prepared.text.index(),
                agent_uri_component(prepared.semantic_id.as_str())
            ),
            PreparedTextOwnerKind::View { mount } => format!(
                "object.text.{}.mount.{mount}.text.{}",
                agent_uri_component(prepared.semantic_id.as_str()),
                prepared.text.index()
            ),
            PreparedTextOwnerKind::Control => format!(
                "object.control.text.{}.{}",
                prepared.text.index(),
                agent_uri_component(prepared.semantic_id.as_str())
            ),
        };
        Self { prepared, root_id }
    }

    pub(in super::super) const fn prepared(&self) -> &'frame PreparedTextOwner {
        self.prepared
    }

    pub(in super::super) fn root_id(&self) -> &str {
        &self.root_id
    }
}
