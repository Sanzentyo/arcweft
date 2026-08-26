use arcweft_agent_protocol::{artifact::ProjectBindingMode, protocol::AgentAssertionKind};
use arcweft_bundle::BundleCodecError;
use arcweft_core::awbc::{product_step::AwbcProductStepBuildError, verify::AwbcVerifyError};
use std::fmt;
use thiserror::Error;

use crate::effect_policy::AgentEffectPolicyError;

/// Host response/event family whose JSON projection failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentHostResponseKind {
    /// Observation response or observation debug event.
    Observation,
    /// Action response or action debug event.
    Action,
    /// Capture response or capture debug event.
    Capture,
    /// Resource response or resource debug event.
    Resource,
    /// RAG context response or RAG debug event.
    RagContext,
}

impl AgentHostResponseKind {
    /// Stable diagnostic label for this response/event family.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observation => "observation",
            Self::Action => "action",
            Self::Capture => "capture",
            Self::Resource => "resource",
            Self::RagContext => "rag_context",
        }
    }
}

/// Failure while projecting one host response/event family to JSON.
#[derive(Debug, Error)]
#[error("Agent {kind} response/event serialization failed: {source}")]
pub struct AgentHostResponseSerializationError {
    kind: AgentHostResponseKind,
    #[source]
    source: serde_json::Error,
}

impl AgentHostResponseSerializationError {
    /// Returns the response/event family that failed to serialize.
    #[must_use]
    pub const fn kind(&self) -> AgentHostResponseKind {
        self.kind
    }

    /// Returns the underlying JSON serialization failure.
    #[must_use]
    pub fn source(&self) -> &serde_json::Error {
        &self.source
    }
}

impl AgentHostResponseKind {
    /// Serializes a response/event value while retaining its typed family.
    pub fn serialize<T: serde::Serialize>(
        self,
        value: &T,
    ) -> Result<serde_json::Value, AgentHostResponseSerializationError> {
        serde_json::to_value(value)
            .map_err(|source| AgentHostResponseSerializationError { kind: self, source })
    }
}

impl fmt::Display for AgentHostResponseKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Agent runner failure.
#[derive(Debug, Error)]
pub enum AgentRunError<SessionError, DebugError, RagError>
where
    SessionError: std::error::Error + Send + Sync + 'static,
    DebugError: std::error::Error + Send + Sync + 'static,
    RagError: std::error::Error + Send + Sync + 'static,
{
    #[error("Agent host request is denied by runtime policy: {0}")]
    PolicyDenied(&'static str),
    #[error("Agent host request is denied by verified effect policy: {0}")]
    EffectPolicy(#[source] AgentEffectPolicyError),
    #[error("Agent session failed: {0}")]
    Session(#[source] SessionError),
    #[error("Agent debug sink failed: {0}")]
    Debug(#[source] DebugError),
    #[error("Agent RAG service failed: {0}")]
    Rag(#[source] RagError),
    #[error(transparent)]
    HostResponseSerialization(#[from] AgentHostResponseSerializationError),
    #[error("Agent controller Product AWBC failed verification: {0}")]
    ProductAwbcVerification(#[source] AwbcVerifyError),
    #[error("Agent controller Product AWBC executor could not be built: {0}")]
    ProductAwbcExecutor(#[source] AwbcProductStepBuildError),
    #[error("Agent controller bundle Product AWBC is invalid: {0}")]
    BundleProductAwbc(#[source] BundleCodecError),
    #[error("Agent controller entry is invalid: {detail}")]
    InvalidControllerEntry { detail: String },
    #[error("bundle is not an Agent controller bundle")]
    NotAgentControllerBundle,
    #[error("Agent controller bundle is missing its Agent artifact manifest")]
    MissingAgentManifest,
    #[error("Agent controller bundle is missing its Product AWBC executable")]
    MissingProductAwbc,
    #[error("Agent controller artifact binding mismatch: {detail}")]
    AgentArtifactMismatch { detail: String },
    #[error(
        "Agent controller project binding mismatch: expected program hash {expected_program_hash}, actual {actual_program_hash}, mode {mode:?}: {detail}"
    )]
    ProjectBindingMismatch {
        expected_program_hash: String,
        actual_program_hash: String,
        mode: ProjectBindingMode,
        detail: String,
    },
    #[error("Agent project entity metadata is missing for {entity}")]
    ProjectEntityMetadataMissing { entity: String },
    #[error("Agent project graph is missing symbol for {entity}")]
    ProjectGraphSymbolMissing { entity: String },
    #[error("Agent controller emitted unsupported effect: {0}")]
    UnsupportedControllerEffect(String),
    #[error("Agent host response failed typed runtime admission: {0}")]
    InvalidHostResponse(String),
    #[error("Agent assertion failed ({kind:?}): {message}")]
    AssertionFailed {
        kind: AgentAssertionKind,
        message: String,
    },
    #[error("Agent controller failed: {0}")]
    ControllerFailed(String),
    #[error("Agent controller exceeded execution step budget of {max_steps}")]
    ControllerBudgetExceeded { max_steps: usize },
    #[error("Agent controller exceeded {kind} budget: attempted {attempted}, limit {limit}")]
    ControllerResourceBudgetExceeded {
        kind: &'static str,
        limit: u64,
        attempted: u64,
    },
    #[error("Agent wait timed out after {timeout_millis} ms")]
    WaitTimeout { timeout_millis: u64 },
}
