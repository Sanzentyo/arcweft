//! Semantic inventory for authored dialogue View input records.
//!
//! Dialogue presentation is selected by a View reference. The View receives a
//! nominal record whose `#[dialogue_view]` role identifies the projections the
//! runtime supplies. Lowering consults this inventory instead of recognizing a
//! particular type-name spelling.

use crate::types::TypeKind;
use std::collections::BTreeMap;

/// Attribute that assigns the dialogue View input role to a nominal record.
pub const DIALOGUE_VIEW_ATTRIBUTE: &str = "dialogue_view";

/// Canonical standard-prelude dialogue View input record.
pub const STANDARD_DIALOGUE_VIEW_TYPE: &str = "DialogueView";

/// Reserved engine-owned dialogue View resource.
pub const STANDARD_DIALOGUE_VIEW_RESOURCE: &str = "std.view.dialogue";

/// Canonical rich dialogue content value exposed by the standard prelude.
pub const DIALOGUE_CONTENT_TYPE: &str = "DialogueContent";

/// Stable identity of one active dialogue occurrence.
pub const DIALOGUE_OCCURRENCE_ID_TYPE: &str = "DialogueOccurrenceId";

/// Current lifecycle stage of one active dialogue occurrence.
pub const DIALOGUE_STAGE_TYPE: &str = "DialogueStage";

/// Current typed reveal state exposed to the authored View.
pub const DIALOGUE_REVEAL_TYPE: &str = "DialogueReveal";

/// Typed primary interaction supplied by the dialogue runtime.
pub const DIALOGUE_ACTION_TYPE: &str = "DialogueAction";

/// Runtime-supplied projection of one dialogue View input record.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DialogueViewProjection {
    CharacterDisplayName,
    Content,
    Occurrence,
    Stage,
    Reveal,
    PrimaryAction,
}

impl DialogueViewProjection {
    /// Canonical field that exposes this projection.
    pub const fn field(self) -> &'static str {
        match self {
            Self::CharacterDisplayName => "character_display_name",
            Self::Content => "content",
            Self::Occurrence => "occurrence",
            Self::Stage => "stage",
            Self::Reveal => "reveal",
            Self::PrimaryAction => "primary_action",
        }
    }

    /// Resolves a field through the closed dialogue View projection contract.
    pub fn from_field(field: &str) -> Option<Self> {
        match field {
            "character_display_name" => Some(Self::CharacterDisplayName),
            "content" => Some(Self::Content),
            "occurrence" => Some(Self::Occurrence),
            "stage" => Some(Self::Stage),
            "reveal" => Some(Self::Reveal),
            "primary_action" => Some(Self::PrimaryAction),
            _ => None,
        }
    }

    /// Semantic value type required for this projection.
    pub fn value_type(self) -> TypeKind {
        match self {
            Self::CharacterDisplayName => TypeKind::String,
            Self::Content => TypeKind::Named(DIALOGUE_CONTENT_TYPE.to_owned()),
            Self::Occurrence => TypeKind::Named(DIALOGUE_OCCURRENCE_ID_TYPE.to_owned()),
            Self::Stage => TypeKind::Named(DIALOGUE_STAGE_TYPE.to_owned()),
            Self::Reveal => TypeKind::Named(DIALOGUE_REVEAL_TYPE.to_owned()),
            Self::PrimaryAction => TypeKind::Named(DIALOGUE_ACTION_TYPE.to_owned()),
        }
    }
}

/// One nominal record registered for the dialogue View input role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueViewModel {
    type_name: String,
}

impl DialogueViewModel {
    fn new(type_name: impl Into<String>) -> Self {
        Self {
            type_name: type_name.into(),
        }
    }

    /// Nominal source type carrying this role.
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Resolves a field on this role-bearing record.
    pub fn projection(&self, field: &str) -> Option<DialogueViewProjection> {
        DialogueViewProjection::from_field(field)
    }
}

/// Closed inventory of standard and source-declared dialogue View models.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueViewModelRegistry {
    models: BTreeMap<String, DialogueViewModel>,
}

impl Default for DialogueViewModelRegistry {
    fn default() -> Self {
        Self::empty()
    }
}

impl DialogueViewModelRegistry {
    /// Creates an inventory without standard or source-declared models.
    pub fn empty() -> Self {
        Self {
            models: BTreeMap::new(),
        }
    }

    /// Creates the standard-prelude inventory.
    pub fn standard() -> Self {
        let model = DialogueViewModel::new(STANDARD_DIALOGUE_VIEW_TYPE);
        Self {
            models: BTreeMap::from([(STANDARD_DIALOGUE_VIEW_TYPE.to_owned(), model)]),
        }
    }

    /// Resolves the semantic role of one nominal source type.
    pub fn model(&self, type_name: &str) -> Option<&DialogueViewModel> {
        self.models.get(type_name)
    }

    /// Iterates role-bearing records in deterministic nominal-name order.
    pub fn models(&self) -> impl Iterator<Item = &DialogueViewModel> {
        self.models.values()
    }
}

#[cfg(test)]
mod tests {
    use super::{DialogueViewModelRegistry, DialogueViewProjection, STANDARD_DIALOGUE_VIEW_TYPE};

    #[test]
    fn standard_registry_exposes_the_closed_runtime_projection() {
        let registry = DialogueViewModelRegistry::standard();
        let model = registry
            .model(STANDARD_DIALOGUE_VIEW_TYPE)
            .expect("standard dialogue View model");
        assert_eq!(model.type_name(), STANDARD_DIALOGUE_VIEW_TYPE);
        assert_eq!(
            model.projection("content"),
            Some(DialogueViewProjection::Content)
        );
        assert_eq!(model.projection("unsupported"), None);
    }
}
