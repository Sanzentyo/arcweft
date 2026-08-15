//! Closed runtime projection of semantically selected Agent Prelude calls.
//!
//! This vocabulary is intentionally independent of source spelling. The
//! compiler projects the selected semantic signature ID into this lower-layer
//! identity, and runtime-plan lowering decides whether it is a host operation
//! or a deterministic value constructor.

/// One semantically selected Agent Prelude operation at the runtime boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeAgentIntrinsic {
    Observe,
    Expect,
    Deny,
    Checkpoint,
    Note,
    Attach,
    ChoiceAction,
    Viewport,
    Layer,
    Object,
    Capture,
    ReadResource,
    EntityMeta,
    ProjectNeighbors,
    Signal,
    Metric,
    StatePath,
    ObservationPath,
    State,
    Observation,
    Diagnostics,
    Exists,
    ActionEnabled,
    All,
    Any,
    Not,
    Wait,
    AdvanceText,
    ViewportPoint,
    PointerClick,
    Invoke,
    RagQuery,
}

impl RuntimeAgentIntrinsic {
    /// Returns the canonical Agent host ABI operation for effectful calls.
    /// Deterministic value constructors return `None` and remain in expression
    /// lowering.
    pub const fn host_operation(self) -> Option<&'static str> {
        Some(match self {
            Self::Observe => "observe",
            Self::Expect => "expect",
            Self::Deny => "deny",
            Self::Checkpoint => "checkpoint",
            Self::Note => "note",
            Self::Attach => "attach",
            Self::Capture => "capture",
            Self::ReadResource => "read_resource",
            Self::EntityMeta => "entity_meta",
            Self::ProjectNeighbors => "project_neighbors",
            Self::Wait => "wait",
            Self::AdvanceText => "advance_text",
            Self::PointerClick => "pointer.click",
            Self::Invoke => "invoke",
            Self::RagQuery => "rag.query",
            Self::ChoiceAction
            | Self::Viewport
            | Self::Layer
            | Self::Object
            | Self::Signal
            | Self::Metric
            | Self::StatePath
            | Self::ObservationPath
            | Self::State
            | Self::Observation
            | Self::Diagnostics
            | Self::Exists
            | Self::ActionEnabled
            | Self::All
            | Self::Any
            | Self::Not
            | Self::ViewportPoint => return None,
        })
    }
}
