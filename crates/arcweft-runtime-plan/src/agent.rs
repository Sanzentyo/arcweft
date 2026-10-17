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

impl From<arcweft_lang_sema::callable::AgentIntrinsicSignatureId> for RuntimeAgentIntrinsic {
    fn from(intrinsic: arcweft_lang_sema::callable::AgentIntrinsicSignatureId) -> Self {
        use arcweft_lang_sema::callable::AgentIntrinsicSignatureId;
        match intrinsic {
            AgentIntrinsicSignatureId::Observe => Self::Observe,
            AgentIntrinsicSignatureId::Expect => Self::Expect,
            AgentIntrinsicSignatureId::Deny => Self::Deny,
            AgentIntrinsicSignatureId::Checkpoint => Self::Checkpoint,
            AgentIntrinsicSignatureId::Note => Self::Note,
            AgentIntrinsicSignatureId::Attach => Self::Attach,
            AgentIntrinsicSignatureId::ChoiceAction => Self::ChoiceAction,
            AgentIntrinsicSignatureId::Viewport => Self::Viewport,
            AgentIntrinsicSignatureId::Layer => Self::Layer,
            AgentIntrinsicSignatureId::Object => Self::Object,
            AgentIntrinsicSignatureId::Capture => Self::Capture,
            AgentIntrinsicSignatureId::ReadResource => Self::ReadResource,
            AgentIntrinsicSignatureId::EntityMeta => Self::EntityMeta,
            AgentIntrinsicSignatureId::ProjectNeighbors => Self::ProjectNeighbors,
            AgentIntrinsicSignatureId::Signal => Self::Signal,
            AgentIntrinsicSignatureId::Metric => Self::Metric,
            AgentIntrinsicSignatureId::StatePath => Self::StatePath,
            AgentIntrinsicSignatureId::ObservationPath => Self::ObservationPath,
            AgentIntrinsicSignatureId::State => Self::State,
            AgentIntrinsicSignatureId::Observation => Self::Observation,
            AgentIntrinsicSignatureId::Diagnostics => Self::Diagnostics,
            AgentIntrinsicSignatureId::Exists => Self::Exists,
            AgentIntrinsicSignatureId::ActionEnabled => Self::ActionEnabled,
            AgentIntrinsicSignatureId::All => Self::All,
            AgentIntrinsicSignatureId::Any => Self::Any,
            AgentIntrinsicSignatureId::Not => Self::Not,
            AgentIntrinsicSignatureId::Wait => Self::Wait,
            AgentIntrinsicSignatureId::AdvanceText => Self::AdvanceText,
            AgentIntrinsicSignatureId::ViewportPoint => Self::ViewportPoint,
            AgentIntrinsicSignatureId::PointerClick => Self::PointerClick,
            AgentIntrinsicSignatureId::Invoke => Self::Invoke,
            AgentIntrinsicSignatureId::RagQuery => Self::RagQuery,
        }
    }
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
