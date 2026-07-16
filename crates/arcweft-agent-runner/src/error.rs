use arcweft_agent_protocol::{artifact::ProjectBindingMode, protocol::AgentAssertionKind};
use arcweft_core::{engine::EngineStartError, plan::RuntimePlanError};
use thiserror::Error;

use crate::effect_policy::AgentEffectPolicyError;

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
    #[error("Agent controller bytecode is invalid: {0}")]
    Bytecode(#[source] RuntimePlanError),
    #[error("Agent controller entry could not start: {0}")]
    ControllerEntryStart(#[source] EngineStartError),
    #[error("Agent controller entry is invalid: {detail}")]
    InvalidControllerEntry { detail: String },
    #[error("bundle is not an Agent controller bundle")]
    NotAgentControllerBundle,
    #[error("Agent controller bundle is missing its Agent artifact manifest")]
    MissingAgentManifest,
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
