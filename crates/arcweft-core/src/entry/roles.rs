//! Checked entry roles, executable bindings, and root execution policy.

use super::identity::{
    AgentPolicyHash, CallableContractHash, EntryBindingIdentity, FlowContractHash,
    RuntimeCallableId, RuntimeCommandConstructorId, RuntimeCommandTargetId, RuntimeNominalTypeId,
    TypeLayoutHash,
};
use super::schema::{RuntimeSchemaLimits, RuntimeTypeSchema};
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{EntryRuntimeId, FlowRuntimeId, RuntimePureHelperId};
use serde::{Deserialize, Serialize};
use thiserror::Error;
/// Effective hard limits included in an Agent entry binding.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentBudget {
    pub logical_timeout_millis: u64,
    pub max_vm_steps: u64,
    pub max_host_calls: u32,
    pub max_observations: u32,
    pub max_captures: u32,
    pub max_capture_bytes: u64,
    pub max_rag_queries: u32,
    pub max_context_bytes: u64,
}

impl Default for AgentBudget {
    fn default() -> Self {
        Self {
            logical_timeout_millis: 30_000,
            max_vm_steps: 100_000,
            max_host_calls: 256,
            max_observations: 256,
            max_captures: 16,
            max_capture_bytes: 64 * 1024 * 1024,
            max_rag_queries: 8,
            max_context_bytes: 1024 * 1024,
        }
    }
}

/// Exact stateful roles attached to one executable entry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeStatefulEntryRoles {
    pub binding: EntryBindingIdentity,
    pub state: RuntimeNominalRole,
    pub initializer: RuntimeCallableRole,
    pub event: RuntimeNominalRole,
    pub reducer: RuntimeCallableRole,
    pub initial_flow: RuntimeFlowRole,
    pub command_policy: RuntimeCommandPolicy,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeNominalRole {
    pub identity: RuntimeNominalTypeId,
    pub semantic_identity: RuntimeSemanticTypeId,
    pub layout: TypeLayoutHash,
    pub schema: RuntimeTypeSchema,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeCallableRole {
    pub callable: RuntimeCallableId,
    pub contract: CallableContractHash,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeFlowRole {
    pub flow: FlowRuntimeId,
    pub contract: FlowContractHash,
}

/// Executable pure-helper mapping for one checked callable contract.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeCallableExecutable {
    pub callable: RuntimeCallableId,
    pub contract: CallableContractHash,
    pub code: RuntimeCallableExecutableCode,
}

/// Existing executable substrate that owns one role callable body.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeCallableExecutableCode {
    PureHelper(RuntimePureHelperId),
    ControllerFlow(FlowRuntimeId),
}

/// Ownership mode of one executable flow input.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeFlowParameterMode {
    Owned,
    Shared,
    Mutable,
}

/// Stable zero-based coordinate of one Flow parameter.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct FlowParameterCoordinate(u32);

impl FlowParameterCoordinate {
    #[must_use]
    pub const fn from_position(position: u32) -> Self {
        Self(position)
    }

    pub fn try_from_index(index: usize) -> Result<Self, FlowParameterCoordinateError> {
        u32::try_from(index)
            .map(Self)
            .map_err(|_| FlowParameterCoordinateError::OutOfRange { index })
    }

    #[must_use]
    pub const fn position(self) -> u32 {
        self.0
    }

    pub fn index(self) -> Result<usize, FlowParameterCoordinateError> {
        usize::try_from(self.0)
            .map_err(|_| FlowParameterCoordinateError::InvalidPlatformWidth { position: self.0 })
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum FlowParameterCoordinateError {
    #[error("Flow parameter index {index} exceeds the coordinate domain")]
    OutOfRange { index: usize },
    #[error("Flow parameter coordinate {position} does not fit this platform")]
    InvalidPlatformWidth { position: u32 },
}

/// Exact executable flow parameter metadata used by entry verification.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeFlowExecutableParameter {
    pub coordinate: FlowParameterCoordinate,
    pub name: String,
    pub mode: RuntimeFlowParameterMode,
    pub semantic_identity: RuntimeSemanticTypeId,
}

/// Complete plan-owned invocation schema for one executable Flow.
///
/// This inventory exists for every lowered Flow, independently of whether an
/// Entry also assigns that Flow a launch role.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeFlowSchema {
    pub flow: FlowRuntimeId,
    pub parameters: Vec<RuntimeFlowExecutableParameter>,
}

/// Entry/controller role metadata for a Flow that has an externally selected
/// launch role. Parameter ABI belongs solely to [`RuntimeFlowSchema`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeFlowExecutable {
    pub flow: FlowRuntimeId,
    pub contract: FlowContractHash,
    pub controller: Option<RuntimeCallableRole>,
}

/// Entry-local command surface and root limits selected by the verified
/// runtime/adapter policy.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeCommandPolicy {
    pub admitted: Vec<RuntimeCommandContract>,
    pub root_limits: RootExecutionLimits,
}

/// Exact constructor/target/payload contract accepted after reducer evaluation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeCommandContract {
    pub constructor: RuntimeCommandConstructorId,
    pub target: RuntimeCommandTargetId,
    pub payload_layout: TypeLayoutHash,
    pub payload_schema: RuntimeTypeSchema,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeAgentEntryRoles {
    pub binding: EntryBindingIdentity,
    pub controller: RuntimeCallableRole,
    pub policy: AgentPolicyHash,
    pub budget: AgentBudget,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeEntryRoles {
    None,
    Stateful(Box<RuntimeStatefulEntryRoles>),
    Agent(Box<RuntimeAgentEntryRoles>),
}

/// Explicit limits for one selected stateful entry's durable root runtime.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RootExecutionLimits {
    pub schema: RuntimeSchemaLimits,
    pub max_commands_per_transition: usize,
    pub max_command_bytes_per_transition: usize,
    pub max_pending_events: usize,
    pub max_pending_commands: usize,
}

impl RootExecutionLimits {
    /// Intentional engine policy for ordinary interactive sessions.
    ///
    /// The selected runtime/adapter owner must opt into this policy; it is not
    /// an implicit root-runtime fallback.
    #[must_use]
    pub const fn engine_default() -> Self {
        Self {
            schema: RuntimeSchemaLimits::engine_default(),
            max_commands_per_transition: 4_096,
            max_command_bytes_per_transition: 8_388_608,
            max_pending_events: 65_536,
            max_pending_commands: 65_536,
        }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.schema.max_depth > 0
            && self.schema.max_nodes > 0
            && self.schema.max_sequence_items > 0
            && self.schema.max_string_bytes > 0
            && self.schema.max_encoded_bytes > 0
            && self.max_commands_per_transition > 0
            && self.max_command_bytes_per_transition > 0
            && self.max_pending_events > 0
            && self.max_pending_commands > 0
            && self.max_commands_per_transition <= self.max_pending_commands
            && self.max_commands_per_transition <= u32::MAX as usize
            && self.max_pending_events <= u32::MAX as usize
            && self.max_pending_commands <= u32::MAX as usize
    }
}

impl RuntimeCommandPolicy {
    /// Creates an explicit adapter command policy and root execution limit set.
    #[must_use]
    pub fn new(
        admitted: impl IntoIterator<Item = RuntimeCommandContract>,
        root_limits: RootExecutionLimits,
    ) -> Self {
        Self {
            admitted: admitted.into_iter().collect(),
            root_limits,
        }
    }

    /// Creates an adapter policy that admits no root commands.
    #[must_use]
    pub const fn deny_all(root_limits: RootExecutionLimits) -> Self {
        Self {
            admitted: Vec::new(),
            root_limits,
        }
    }
}

impl RuntimeEntryRoles {
    #[must_use]
    pub const fn binding(&self) -> Option<EntryBindingIdentity> {
        match self {
            Self::None => None,
            Self::Stateful(roles) => Some(roles.binding),
            Self::Agent(roles) => Some(roles.binding),
        }
    }

    #[must_use]
    pub const fn stateful(&self) -> Option<&RuntimeStatefulEntryRoles> {
        match self {
            Self::Stateful(roles) => Some(roles),
            Self::None | Self::Agent(_) => None,
        }
    }

    #[must_use]
    pub const fn agent(&self) -> Option<&RuntimeAgentEntryRoles> {
        match self {
            Self::Agent(roles) => Some(roles),
            Self::None | Self::Stateful(_) => None,
        }
    }
}

/// Active entry identity shared by save, replay, and hot-reload checks.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ActiveEntrySnapshotV1 {
    pub id: EntryRuntimeId,
    pub kind: crate::plan::RuntimeEntryKind,
    pub binding: EntryBindingIdentity,
}
