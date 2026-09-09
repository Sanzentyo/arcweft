//! Admitted Agent object ownership projected from a canonical prepared-text owner.

use arcweft_render_wgpu::geometry::{
    DialoguePreparedTextRole, PreparedTextOwner, PreparedTextOwnerKind,
};
use thiserror::Error;

use super::agent_uri_component;

#[cfg(test)]
mod tests;

/// Borrows the renderer's owner and applies the Agent object-identity contract.
/// Character display names remain semantic View text; dialogue child objects
/// belong to the Content role. No independent renderer owner is created here.
#[derive(Debug)]
pub(in super::super) struct PreparedTextObservationOwner<'frame> {
    prepared: &'frame PreparedTextOwner,
    root_id: String,
}

#[derive(Debug, Error)]
pub(in super::super) enum PreparedTextOwnerSelectionError {
    #[error("no matching prepared-text owner")]
    Missing,
    #[error("matches {count} prepared-text owners")]
    Ambiguous { count: usize },
}

impl<'frame> PreparedTextObservationOwner<'frame> {
    pub(in super::super) fn select(
        owners: &'frame [PreparedTextOwner],
        matches: impl Fn(&Self) -> bool,
    ) -> Result<Self, PreparedTextOwnerSelectionError> {
        let mut candidates = owners.iter().filter_map(Self::new).filter(matches);
        let first = candidates
            .next()
            .ok_or(PreparedTextOwnerSelectionError::Missing)?;
        let remaining = candidates.count();
        if remaining == 0 {
            Ok(first)
        } else {
            Err(PreparedTextOwnerSelectionError::Ambiguous {
                count: remaining + 1,
            })
        }
    }

    pub(in super::super) fn new(prepared: &'frame PreparedTextOwner) -> Option<Self> {
        let root_id = match prepared.kind {
            PreparedTextOwnerKind::DialogueView {
                dialogue,
                entry,
                role: DialoguePreparedTextRole::Content,
                ..
            } => format!("object.dialogue.{dialogue}.{entry}"),
            PreparedTextOwnerKind::DialogueView {
                role: DialoguePreparedTextRole::CharacterDisplayName,
                ..
            } => return None,
            PreparedTextOwnerKind::View { mount } => format!(
                "object.text.{}.mount.{mount}",
                agent_uri_component(prepared.semantic_id.as_str())
            ),
            PreparedTextOwnerKind::Control => prepared.semantic_id.to_string(),
        };
        Some(Self { prepared, root_id })
    }

    pub(in super::super) const fn prepared(&self) -> &'frame PreparedTextOwner {
        self.prepared
    }

    pub(in super::super) fn root_id(&self) -> &str {
        &self.root_id
    }

    pub(in super::super) fn contains_object(&self, object_id: &str) -> bool {
        object_id == self.root_id
            || (!matches!(self.prepared.kind, PreparedTextOwnerKind::Control)
                && object_id
                    .strip_prefix(&self.root_id)
                    .is_some_and(|suffix| suffix.starts_with('.')))
    }
}
