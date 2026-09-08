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
    /// Entity metadata response.
    EntityMetadata,
    /// Project graph neighborhood response.
    ProjectGraphNeighborhood,
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
            Self::EntityMetadata => "entity_metadata",
            Self::ProjectGraphNeighborhood => "project_graph_neighborhood",
        }
    }
}

/// Stable category for a rejected Agent host-response runtime projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentHostResponseAdmissionErrorKind {
    /// The response did not match the protocol-owned payload shape.
    InvalidShape,
    /// A response count cannot be represented by the runtime field type.
    CountOutOfRange,
    /// A protocol identity cannot be admitted by the runtime identity owner.
    InvalidIdentity,
    /// A JSON number cannot be represented by an Arcweft runtime number.
    NumberOutOfRange,
    /// A typed field value violates an Arcweft-owned protocol invariant.
    InvalidValue,
}

/// Failure while admitting a typed Agent host response into `RuntimeValue`.
#[derive(Debug, Error)]
pub enum AgentHostResponseAdmissionError {
    #[error("Agent {response} response at `{path}` has an invalid protocol shape: {source}")]
    InvalidShape {
        response: AgentHostResponseKind,
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error(
        "Agent {response} response count at `{path}` is outside the supported runtime range: {actual}"
    )]
    CountOutOfRange {
        response: AgentHostResponseKind,
        path: &'static str,
        actual: usize,
    },
    #[error("Agent {response} response identity at `{path}` has invalid value `{value}`: {detail}")]
    InvalidIdentity {
        response: AgentHostResponseKind,
        path: String,
        value: String,
        detail: String,
    },
    #[error(
        "Agent {response} response number at `{path}` is outside the supported runtime range: {value}"
    )]
    NumberOutOfRange {
        response: AgentHostResponseKind,
        path: String,
        value: String,
    },
    #[error(
        "Agent {response} response value at `{path}` is invalid: `{value}`; expected {expected}"
    )]
    InvalidValue {
        response: AgentHostResponseKind,
        path: String,
        value: String,
        expected: &'static str,
    },
}

impl AgentHostResponseAdmissionError {
    /// Returns the stable rejection category.
    #[must_use]
    pub const fn kind(&self) -> AgentHostResponseAdmissionErrorKind {
        match self {
            Self::InvalidShape { .. } => AgentHostResponseAdmissionErrorKind::InvalidShape,
            Self::CountOutOfRange { .. } => AgentHostResponseAdmissionErrorKind::CountOutOfRange,
            Self::InvalidIdentity { .. } => AgentHostResponseAdmissionErrorKind::InvalidIdentity,
            Self::NumberOutOfRange { .. } => AgentHostResponseAdmissionErrorKind::NumberOutOfRange,
            Self::InvalidValue { .. } => AgentHostResponseAdmissionErrorKind::InvalidValue,
        }
    }

    /// Returns the rejected response family.
    #[must_use]
    pub const fn response(&self) -> AgentHostResponseKind {
        match self {
            Self::InvalidShape { response, .. }
            | Self::CountOutOfRange { response, .. }
            | Self::InvalidIdentity { response, .. }
            | Self::NumberOutOfRange { response, .. }
            | Self::InvalidValue { response, .. } => *response,
        }
    }

    /// Returns the stable response path at which admission failed.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::InvalidShape { path, .. }
            | Self::InvalidIdentity { path, .. }
            | Self::NumberOutOfRange { path, .. } => path,
            Self::CountOutOfRange { path, .. } => path,
            Self::InvalidValue { path, .. } => path,
        }
    }
}

/// Stable category for runtime-value JSON projection failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRuntimeValueSerializationErrorKind {
    JsonSerialization,
    NonFiniteNumber,
    InvalidRuntimeState,
}

/// Failure while projecting one runtime value into Agent JSON.
#[derive(Debug, Error)]
pub enum AgentRuntimeValueSerializationError {
    #[error("runtime value at `{path}` could not be serialized as Agent JSON: {source}")]
    JsonSerialization {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("runtime value at `{path}` contains a non-finite JSON number: {value}")]
    NonFiniteNumber { path: String, value: String },
    #[error("runtime value at `{path}` cannot be projected from its current state: {detail}")]
    InvalidRuntimeState { path: String, detail: String },
}

impl AgentRuntimeValueSerializationError {
    /// Returns the stable failure category.
    #[must_use]
    pub const fn kind(&self) -> AgentRuntimeValueSerializationErrorKind {
        match self {
            Self::JsonSerialization { .. } => {
                AgentRuntimeValueSerializationErrorKind::JsonSerialization
            }
            Self::NonFiniteNumber { .. } => {
                AgentRuntimeValueSerializationErrorKind::NonFiniteNumber
            }
            Self::InvalidRuntimeState { .. } => {
                AgentRuntimeValueSerializationErrorKind::InvalidRuntimeState
            }
        }
    }

    /// Returns the stable runtime-value path at which projection failed.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::JsonSerialization { path, .. }
            | Self::NonFiniteNumber { path, .. }
            | Self::InvalidRuntimeState { path, .. } => path,
        }
    }
}

/// Stable category for a rejected controller host request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentHostRequestAdmissionErrorKind {
    UnsupportedEffect,
    UnsupportedCapability,
    UnsupportedOperation,
    MissingArgument,
    InvalidArguments,
    RuntimeValueSerialization,
}

/// Failure while admitting one controller-emitted Agent host request.
#[derive(Debug, Error)]
pub enum AgentHostRequestAdmissionError {
    #[error("unsupported Agent controller effect `{effect}`")]
    UnsupportedEffect { effect: String },
    #[error("unsupported Agent task capability `{capability}`")]
    UnsupportedCapability { capability: String },
    #[error("unsupported Agent operation `{operation}`")]
    UnsupportedOperation { operation: String },
    #[error("Agent operation `{operation}` requires argument `{argument}`")]
    MissingArgument {
        operation: &'static str,
        argument: &'static str,
    },
    #[error("Agent operation `{operation}` has invalid arguments: {detail}")]
    InvalidArguments { operation: String, detail: String },
    #[error(
        "Agent operation `{operation}` argument `{argument}` cannot be projected to JSON: {source}"
    )]
    RuntimeValueSerialization {
        operation: &'static str,
        argument: &'static str,
        #[source]
        source: AgentRuntimeValueSerializationError,
    },
}

impl AgentHostRequestAdmissionError {
    pub(crate) fn invalid_arguments(
        operation: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self::InvalidArguments {
            operation: operation.into(),
            detail: detail.into(),
        }
    }

    /// Returns the stable rejection category.
    #[must_use]
    pub const fn kind(&self) -> AgentHostRequestAdmissionErrorKind {
        match self {
            Self::UnsupportedEffect { .. } => AgentHostRequestAdmissionErrorKind::UnsupportedEffect,
            Self::UnsupportedCapability { .. } => {
                AgentHostRequestAdmissionErrorKind::UnsupportedCapability
            }
            Self::UnsupportedOperation { .. } => {
                AgentHostRequestAdmissionErrorKind::UnsupportedOperation
            }
            Self::MissingArgument { .. } => AgentHostRequestAdmissionErrorKind::MissingArgument,
            Self::InvalidArguments { .. } => AgentHostRequestAdmissionErrorKind::InvalidArguments,
            Self::RuntimeValueSerialization { .. } => {
                AgentHostRequestAdmissionErrorKind::RuntimeValueSerialization
            }
        }
    }
}

/// Stable category for a rejected controller task outcome carrier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentControllerOutcomeAdmissionErrorKind {
    ResultContractRejected,
}

/// Failure while wrapping one admitted host response in its checked task result.
#[derive(Debug, Error)]
#[error(
    "Agent controller task `{task_id}` response failed checked result admission at `{path}`: {detail}"
)]
pub struct AgentControllerOutcomeAdmissionError {
    task_id: String,
    path: &'static str,
    detail: String,
}

impl AgentControllerOutcomeAdmissionError {
    pub(crate) fn result_contract_rejected(task_id: impl Into<String>, detail: String) -> Self {
        Self {
            task_id: task_id.into(),
            path: "task.outcome.result.ok",
            detail,
        }
    }

    /// Returns the stable rejection category.
    #[must_use]
    pub const fn kind(&self) -> AgentControllerOutcomeAdmissionErrorKind {
        AgentControllerOutcomeAdmissionErrorKind::ResultContractRejected
    }

    /// Returns the controller task identity.
    #[must_use]
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// Returns the stable carrier path at which admission failed.
    #[must_use]
    pub const fn path(&self) -> &'static str {
        self.path
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
    #[error("Agent controller host request failed typed admission: {0}")]
    InvalidControllerRequest(#[source] AgentHostRequestAdmissionError),
    #[error("Agent host response failed typed runtime admission: {0}")]
    InvalidHostResponse(#[source] AgentHostResponseAdmissionError),
    #[error("Agent controller result carrier rejected a host response: {0}")]
    InvalidControllerOutcome(#[source] AgentControllerOutcomeAdmissionError),
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
