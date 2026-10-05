use crate::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use crate::value::{RuntimeExpr, RuntimePayload, RuntimeValue};
use arcweft_need::Need;
pub use arcweft_need::Progress;
use serde::{Deserialize, Serialize};

use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::runtime_id::RuntimePersistentFiberId;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::num::{NonZeroU32, NonZeroU64};
use thiserror::Error;

pub mod outcome;
pub use outcome::{
    BoundTaskOutcome, BoundTaskSpec, RuntimeProgramOwner, TaskOutcomeBindingError,
    TaskOutcomeValueError,
};

/// Host-local generation slot used to qualify live runtime state.
///
/// Zero is a valid first slot; absence is represented by `Option`, never by a
/// sentinel generation value.  The semantic generation contract is a separate
/// digest owned by the plan boundary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct GenerationId(u64);

impl GenerationId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

mod producer;
pub use producer::*;
pub(crate) mod semantic;

// Private final identity preparation. Publish only with the atomic task/Need
// carrier, journal, host and persistence migration; no String conversion exists.
mod identity;
pub use identity::{
    NeedId, TaskCorrelation, TaskCorrelationError, TaskId, TaskIdentityError, TaskIdentityKind,
    TaskKey,
};
mod need_handle;
mod specification;
pub use need_handle::{RuntimeNeedHandle, RuntimeNeedHandleError, RuntimeNeedHandleSaveSnapshot};
pub use specification::{NeedProducerInstance, TaskHandle, TaskSpec};
mod journal;
pub use journal::{TaskAdmissionJournal, TaskSubmission};
mod snapshot;
pub use snapshot::{HostTaskRequestSnapshot, TaskSpecSnapshot, TaskSubmissionSaveSnapshot};
mod failure;
pub use failure::{
    BoundedRuntimeDiagnostic, RuntimeNeedOutcome, RuntimeTaskFailure, RuntimeTaskFailureKind,
};
mod template;
pub use template::{HostCallProducerDefinition, NeedProducerTemplate};

#[cfg(test)]
mod identity_tests {
    use super::*;
    #[test]
    fn host_catalog_owns_canonical_order_and_lookup() {
        let contract = HostTaskRequestContract::try_new(
            HostTaskRequestKind::FileReadText,
            Box::new([]),
            Box::new([]),
            HostSpreadContract::Forbidden,
        )
        .expect("request contract");
        let row = HostOperationCatalogRowInput::try_new(
            HostOperationCatalogOperation::Builtin(BuiltinHostOperationId::FileReadText),
            HostCapabilityId("fs".to_owned()),
            contract,
            HostRouteId::new(NonZeroU32::new(1).expect("route")),
            HostRestartPolicy::Restartable,
            HostCancellationContract::RequiredIdempotent,
        )
        .expect("host operation row");
        let catalog = HostOperationCatalog::try_new(Box::new([row])).expect("catalog");
        let operation = HostOperationIdentity::Builtin(BuiltinHostOperationId::FileReadText);
        assert_eq!(
            catalog
                .resolve(&operation)
                .expect("lookup")
                .route()
                .get()
                .get(),
            1
        );
        assert_ne!(
            catalog.digest(),
            HostOperationCatalogDigest::from_bytes([0; 32])
        );
    }

    #[test]
    fn host_catalog_retains_and_resolves_catalog_bound_identity() {
        let operation = HostOperationId::new(NonZeroU32::new(1).expect("operation"));
        let contract = HostTaskRequestContract::try_new(
            HostTaskRequestKind::Custom,
            Box::new([]),
            Box::new([]),
            HostSpreadContract::Forbidden,
        )
        .expect("request contract");
        let row = HostOperationCatalogRowInput::try_new(
            HostOperationCatalogOperation::Custom(operation),
            HostCapabilityId("custom".to_owned()),
            contract,
            HostRouteId::new(NonZeroU32::new(2).expect("route")),
            HostRestartPolicy::MustBeQuiescent,
            HostCancellationContract::RequiredIdempotent,
        )
        .expect("catalog input");
        let catalog = HostOperationCatalog::try_new(Box::new([row])).expect("catalog");
        let identity = HostOperationIdentity::Catalog {
            catalog: catalog.digest(),
            operation,
        };

        assert_eq!(
            catalog.resolve(&identity).expect("lookup").identity(),
            &identity
        );
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct CancelScopeId(pub String);

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct LogicalEpoch(pub u64);

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct TaskSequence(pub u64);

/// Monotone publication revision within one task dispatch, issued by the
/// runtime or host adapter boundary. Revision one is first; zero is invalid.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct TaskPublicationRevision(NonZeroU64);

impl TaskPublicationRevision {
    pub const FIRST: Self = Self(NonZeroU64::MIN);

    #[must_use]
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    #[must_use]
    pub fn checked_next(self) -> Option<Self> {
        self.get()
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .map(Self)
    }
}

/// Exact host dispatch identity carried back with every publication. Request
/// sequence is never reused as the within-dispatch publication revision.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TaskDispatchIdentity {
    pub correlation: TaskCorrelation,
    pub logical_epoch: LogicalEpoch,
    /// Physical dispatch attempt fence, separate from publication ordering.
    pub sequence: TaskSequence,
}

impl TaskDispatchIdentity {
    pub const fn new(
        correlation: TaskCorrelation,
        logical_epoch: LogicalEpoch,
        sequence: TaskSequence,
    ) -> Self {
        Self {
            correlation,
            logical_epoch,
            sequence,
        }
    }
}
/// Starting point for one task dispatch and its publication journal. Restored
/// re-ensure supplies the last accepted revision so the adapter continues the
/// same dispatch at its checked successor instead of restarting at revision 1.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TaskDispatchStart {
    identity: TaskDispatchIdentity,
    last_publication_revision: Option<TaskPublicationRevision>,
}

impl TaskDispatchStart {
    #[must_use]
    pub const fn new(
        identity: TaskDispatchIdentity,
        last_publication_revision: Option<TaskPublicationRevision>,
    ) -> Self {
        Self {
            identity,
            last_publication_revision,
        }
    }

    #[must_use]
    pub const fn identity(&self) -> &TaskDispatchIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn last_publication_revision(&self) -> Option<TaskPublicationRevision> {
        self.last_publication_revision
    }

    #[must_use]
    pub fn next_publication_revision(&self) -> Option<TaskPublicationRevision> {
        self.last_publication_revision.map_or(
            Some(TaskPublicationRevision::FIRST),
            TaskPublicationRevision::checked_next,
        )
    }
}

/// Monotone publication position within one complete task correlation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskPublicationCursor {
    pub logical_epoch: LogicalEpoch,
    pub sequence: TaskSequence,
}

impl TaskPublicationCursor {
    pub const fn from_event(event: &TaskEvent) -> Self {
        event.cursor
    }
    pub fn compare_same_source(self, other: Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}
/// One producer-owned, in-memory state publication for a typed `Need<T>`.
///
/// This boundary deliberately does not add a `RuntimeValue` or AWBC wire
/// surrogate. The handle carried by a verified `NeedHandle` register names the
/// `NeedId`; the producer publishes the typed success/error payload here for
/// the current deterministic runtime step. Fallible producers publish a
/// `Result<T, E>` as this single payload.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeNeedState {
    pub correlation: TaskCorrelation,
    pub cursor: Option<TaskPublicationCursor>,
    pub state: Need<RuntimeNeedOutcome>,
}
/// Owned publication queued for one exact correlation. Local Ready payloads
/// remain in their producer owner until the consuming Await takes them.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeNeedPublication {
    State {
        correlation: TaskCorrelation,
        state: Need<RuntimeNeedOutcome>,
        cursor: TaskPublicationCursor,
    },
    Producer {
        correlation: TaskCorrelation,
        cursor: TaskPublicationCursor,
    },
    InfrastructureFailure {
        correlation: TaskCorrelation,
        cursor: TaskPublicationCursor,
        failure: RuntimeTaskFailure,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RuntimeNeedPublicationRollbackImage {
    State {
        correlation: TaskCorrelation,
        state: RuntimeNeedStateRollbackImage,
        cursor: TaskPublicationCursor,
    },
    Producer {
        correlation: TaskCorrelation,
        cursor: TaskPublicationCursor,
    },
    InfrastructureFailure {
        correlation: TaskCorrelation,
        cursor: TaskPublicationCursor,
        failure: RuntimeTaskFailure,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RuntimeNeedStateRollbackImage {
    NotStarted,
    Pending(Progress),
    Ready(crate::value::AwbcRuntimeValueSnapshot),
    InfrastructureFailure(RuntimeTaskFailure),
    Cancelled,
}

impl RuntimeNeedPublication {
    pub(crate) fn inert_rollback_image(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> Result<RuntimeNeedPublicationRollbackImage, String> {
        Ok(match self {
            Self::State {
                correlation,
                state,
                cursor,
            } => RuntimeNeedPublicationRollbackImage::State {
                correlation: *correlation,
                cursor: *cursor,
                state: match state {
                    Need::NotStarted => RuntimeNeedStateRollbackImage::NotStarted,
                    Need::Pending(progress) => {
                        RuntimeNeedStateRollbackImage::Pending(progress.clone())
                    }
                    Need::Ready(RuntimeNeedOutcome::Value(value)) => {
                        RuntimeNeedStateRollbackImage::Ready(
                            crate::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(
                                value.value(),
                                owner,
                            )
                            .map_err(|error| error.to_string())?,
                        )
                    }
                    Need::Ready(RuntimeNeedOutcome::InfrastructureFailure(failure)) => {
                        RuntimeNeedStateRollbackImage::InfrastructureFailure(failure.clone())
                    }
                    Need::Cancelled => RuntimeNeedStateRollbackImage::Cancelled,
                },
            },
            Self::Producer {
                correlation,
                cursor,
            } => RuntimeNeedPublicationRollbackImage::Producer {
                correlation: *correlation,
                cursor: *cursor,
            },
            Self::InfrastructureFailure {
                correlation,
                cursor,
                failure,
            } => RuntimeNeedPublicationRollbackImage::InfrastructureFailure {
                correlation: *correlation,
                cursor: *cursor,
                failure: failure.clone(),
            },
        })
    }

    pub(crate) fn from_rollback_image(
        image: RuntimeNeedPublicationRollbackImage,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, String> {
        Ok(match image {
            RuntimeNeedPublicationRollbackImage::State {
                correlation,
                state,
                cursor,
            } => Self::State {
                correlation,
                cursor,
                state: match state {
                    RuntimeNeedStateRollbackImage::NotStarted => Need::NotStarted,
                    RuntimeNeedStateRollbackImage::Pending(progress) => Need::Pending(progress),
                    RuntimeNeedStateRollbackImage::Ready(saved) => {
                        Need::Ready(RuntimeNeedOutcome::Value(RuntimePayload(
                            saved
                                .into_runtime_value_for_program(owner)
                                .map_err(|error| error.to_string())?,
                        )))
                    }
                    RuntimeNeedStateRollbackImage::InfrastructureFailure(failure) => {
                        Need::Ready(RuntimeNeedOutcome::InfrastructureFailure(failure))
                    }
                    RuntimeNeedStateRollbackImage::Cancelled => Need::Cancelled,
                },
            },
            RuntimeNeedPublicationRollbackImage::Producer {
                correlation,
                cursor,
            } => Self::Producer {
                correlation,
                cursor,
            },
            RuntimeNeedPublicationRollbackImage::InfrastructureFailure {
                correlation,
                cursor,
                failure,
            } => Self::InfrastructureFailure {
                correlation,
                cursor,
                failure,
            },
        })
    }

    pub const fn correlation(&self) -> TaskCorrelation {
        match self {
            Self::State { correlation, .. }
            | Self::Producer { correlation, .. }
            | Self::InfrastructureFailure { correlation, .. } => *correlation,
        }
    }
    pub const fn need(&self) -> &NeedId {
        match self {
            Self::State { correlation, .. }
            | Self::Producer { correlation, .. }
            | Self::InfrastructureFailure { correlation, .. } => &correlation.need,
        }
    }
    pub const fn cursor(&self) -> TaskPublicationCursor {
        match self {
            Self::State { cursor, .. }
            | Self::Producer { cursor, .. }
            | Self::InfrastructureFailure { cursor, .. } => *cursor,
        }
    }
}
/// The exact payload type a host task may publish through temporal `Ready`.
///
/// Fallible producers admit a `Result<T, E>` payload here. Infrastructure
/// failures and cancellation are control outcomes and are not alternate typed
/// payload coordinates.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum TaskOutcomeContract {
    /// An explicit finite contract owned by a producer without an executable.
    Standalone { payload: RuntimeCheckedType },
    /// The result row owned by the selected native or AWBC executable.
    Program { payload: RuntimeSemanticTypeId },
}

impl TaskOutcomeContract {
    pub fn payload_semantic_identity(&self) -> RuntimeSemanticTypeId {
        match self {
            Self::Standalone { payload } => payload.semantic_identity_digest(),
            Self::Program { payload } => *payload,
        }
    }
    #[must_use]
    pub const fn new(payload: RuntimeCheckedType) -> Self {
        Self::Standalone { payload }
    }

    #[must_use]
    pub const fn program(payload: RuntimeSemanticTypeId) -> Self {
        Self::Program { payload }
    }

    #[must_use]
    pub const fn standalone_payload(&self) -> Option<&RuntimeCheckedType> {
        match self {
            Self::Standalone { payload } => Some(payload),
            Self::Program { .. } => None,
        }
    }

    #[must_use]
    pub const fn program_payload(&self) -> Option<RuntimeSemanticTypeId> {
        match self {
            Self::Standalone { .. } => None,
            Self::Program { payload } => Some(*payload),
        }
    }

    pub fn try_payload(&self, value: RuntimeValue) -> Result<RuntimePayload, String> {
        match self {
            Self::Standalone { payload } if self.standalone_contract_is_valid() => {
                payload.try_payload(value)
            }
            Self::Standalone { .. } => {
                Err("standalone task outcome needs program authority".into())
            }
            Self::Program { .. } => Err("program task result requires its bound executable".into()),
        }
    }

    pub fn try_result_ok(&self, value: RuntimeValue) -> Result<RuntimePayload, String> {
        match self {
            Self::Standalone { payload } if self.standalone_contract_is_valid() => {
                payload.try_result_payload(Ok(value))
            }
            Self::Standalone { .. } => {
                Err("standalone task outcome needs program authority".into())
            }
            Self::Program { .. } => Err("program task result requires its bound executable".into()),
        }
    }

    pub fn try_result_err(&self, value: RuntimeValue) -> Result<RuntimePayload, String> {
        match self {
            Self::Standalone { payload } if self.standalone_contract_is_valid() => {
                payload.try_result_payload(Err(value))
            }
            Self::Standalone { .. } => {
                Err("standalone task outcome needs program authority".into())
            }
            Self::Program { .. } => Err("program task result requires its bound executable".into()),
        }
    }

    #[must_use]
    pub fn result_error(&self) -> Option<&RuntimeCheckedType> {
        if !self.standalone_contract_is_valid() {
            return None;
        }
        match self {
            Self::Standalone {
                payload: RuntimeCheckedType::Result { error, .. },
            } => Some(error),
            _ => None,
        }
    }
}

impl Default for TaskOutcomeContract {
    fn default() -> Self {
        Self::new(RuntimeCheckedType::Unit)
    }
}

impl RuntimeNeedState {
    pub const fn new(
        correlation: TaskCorrelation,
        cursor: Option<TaskPublicationCursor>,
        state: Need<RuntimeNeedOutcome>,
    ) -> Self {
        Self {
            correlation,
            cursor,
            state,
        }
    }
    pub const fn need(&self) -> &NeedId {
        &self.correlation.need
    }
    pub const fn state(&self) -> &Need<RuntimeNeedOutcome> {
        &self.state
    }
    pub fn into_parts(
        self,
    ) -> (
        TaskCorrelation,
        Option<TaskPublicationCursor>,
        Need<RuntimeNeedOutcome>,
    ) {
        (self.correlation, self.cursor, self.state)
    }
    pub fn inspect_host_ready_ownership(&self) -> Result<(), RuntimeHostPayloadOwnershipError> {
        match &self.state {
            Need::Ready(RuntimeNeedOutcome::Value(value)) => inspect_host_payload_ownership(value),
            _ => Ok(()),
        }
    }
}
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct TaskPriority(pub i32);

#[derive(Clone, Debug, PartialEq)]
pub struct AwaitTarget {
    pub need: NeedId,
    pub task: TaskId,
    pub outcome: TaskOutcomeContract,
    pub request: HostTaskRequestTemplate,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AwaitManyTarget {
    pub source: RuntimeExpr,
    pub item_binding: RuntimeLocalDeclarationId,
    pub limit: u32,
    pub base: NeedProducerTemplate,
    pub child: NeedProducerTemplate,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskRequestTemplate {
    pub capability: HostCapabilityId,
    pub operation: String,
    pub args: Vec<RuntimeHostArgumentTemplate>,
    pub(crate) captures: Vec<RuntimeLocalDeclarationId>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeHostArgumentTemplate {
    Positional(RuntimeExpr),
    Named(NamedHostArg<RuntimeExpr>),
    Spread(RuntimeExpr),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NamedHostArg<T> {
    pub name: String,
    pub value: T,
}

/// A task identifier was reused with a different accepted specification, or a
/// same-key join requested work that does not share the owner's complete
/// scheduling and outcome contract.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TaskEnsureError {
    #[error("task identity derivation failed: {0}")]
    Identity(#[from] TaskIdentityError),
    #[error("task launch ordinal is exhausted")]
    LaunchOrdinalExhausted,
    #[error("task outcome differs from its issued producer payload type")]
    OutcomeContractMismatch,
    #[error("task identifier {task_id:?} was reused with a different specification")]
    TaskIdSpecificationConflict { task_id: TaskId },
    #[error("task {task_id:?} conflicts with owner {owner_id:?} for same-key join {key:?}")]
    JoinSpecificationConflict {
        task_id: TaskId,
        owner_id: TaskId,
        key: TaskKey,
    },
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SchedulerBudget {
    pub max_events: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TaskClass {
    LocalView,
    Io,
    Cpu,
    GpuPrepare,
    ShaderCompile,
    WasmCall,
    AssetDecode,
    AudioDecode,
    AudioRender,
    TtsSynthesis,
    BgmPrecompose,
    Lsp,
    Background,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum TaskPolicy {
    JoinSameKey,
    AlwaysStart,
}

impl TaskPolicy {
    pub(crate) const fn semantic_tag(self) -> u8 {
        match self {
            Self::JoinSameKey => 0,
            Self::AlwaysStart => 1,
        }
    }

    pub(crate) const fn from_semantic_tag(value: u8) -> Result<Self, TaskIdentityError> {
        match value {
            0 => Ok(Self::JoinSameKey),
            1 => Ok(Self::AlwaysStart),
            other => Err(TaskIdentityError::UnknownPolicy(other)),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct HostCapabilityId(pub String);

/// Host route assigned by the adapter catalog.  Zero is rejected by the
/// `NonZeroU32` boundary and therefore cannot be confused with absence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct HostRouteId(NonZeroU32);

impl HostRouteId {
    #[must_use]
    pub const fn new(value: NonZeroU32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> NonZeroU32 {
        self.0
    }
}

/// Canonical operation ordinal within one host catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct HostOperationId(NonZeroU32);

impl HostOperationId {
    #[must_use]
    pub const fn new(value: NonZeroU32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> NonZeroU32 {
        self.0
    }
}

/// Digest of the canonical host-operation catalog transcript.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct HostOperationCatalogDigest([u8; 32]);

impl HostOperationCatalogDigest {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Closed built-in host operation vocabulary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BuiltinHostOperationId {
    FileReadText,
    FileReadBytes,
    FileWriteText,
    FileWriteBytes,
    HttpFetch,
    HttpRespond,
    ProcessRun,
    AssetLoad,
    ShaderCompile,
    AudioDecode,
    TtsSynthesis,
    WasmCall,
    SystemInfo,
}

impl BuiltinHostOperationId {
    pub(crate) const fn semantic_tag(self) -> u8 {
        match self {
            Self::FileReadText => 0,
            Self::FileReadBytes => 1,
            Self::FileWriteText => 2,
            Self::FileWriteBytes => 3,
            Self::HttpFetch => 4,
            Self::HttpRespond => 5,
            Self::ProcessRun => 6,
            Self::AssetLoad => 7,
            Self::ShaderCompile => 8,
            Self::AudioDecode => 9,
            Self::TtsSynthesis => 10,
            Self::WasmCall => 11,
            Self::SystemInfo => 12,
        }
    }
}

/// Typed host-operation identity; custom operations are catalog-bound.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostOperationIdentity {
    Builtin(BuiltinHostOperationId),
    Catalog {
        catalog: HostOperationCatalogDigest,
        operation: HostOperationId,
    },
}

/// Construction-only operation coordinate consumed when sealing one catalog.
/// Custom rows gain the computed catalog digest only inside
/// `HostOperationCatalog::try_new`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostOperationCatalogOperation {
    Builtin(BuiltinHostOperationId),
    Custom(HostOperationId),
}

impl HostOperationCatalogOperation {
    fn write_semantic(self, hasher: &mut blake3::Hasher) {
        match self {
            Self::Builtin(operation) => {
                hasher.update(&[0, operation.semantic_tag()]);
            }
            Self::Custom(operation) => {
                hasher.update(&[1]);
                hasher.update(&operation.get().get().to_le_bytes());
            }
        }
    }

    fn seal(self, catalog: HostOperationCatalogDigest) -> HostOperationIdentity {
        match self {
            Self::Builtin(operation) => HostOperationIdentity::Builtin(operation),
            Self::Custom(operation) => HostOperationIdentity::Catalog { catalog, operation },
        }
    }
}

/// Closed request-shape family used by host catalog contracts.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostTaskRequestKind {
    FileReadText,
    FileReadBytes,
    FileWriteText,
    FileWriteBytes,
    HttpFetch,
    HttpRespond,
    ProcessRun,
    AssetLoad,
    ShaderCompile,
    AudioDecode,
    TtsSynthesis,
    WasmCall,
    SystemInfo,
    Custom,
}

impl HostTaskRequestKind {
    const fn semantic_tag(self) -> u8 {
        match self {
            Self::FileReadText => 0,
            Self::FileReadBytes => 1,
            Self::FileWriteText => 2,
            Self::FileWriteBytes => 3,
            Self::HttpFetch => 4,
            Self::HttpRespond => 5,
            Self::ProcessRun => 6,
            Self::AssetLoad => 7,
            Self::ShaderCompile => 8,
            Self::AudioDecode => 9,
            Self::TtsSynthesis => 10,
            Self::WasmCall => 11,
            Self::SystemInfo => 12,
            Self::Custom => 13,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostSpreadContract {
    Forbidden,
    PositionalTail,
}

impl HostSpreadContract {
    const fn semantic_tag(self) -> u8 {
        match self {
            Self::Forbidden => 0,
            Self::PositionalTail => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum HostRestartPolicy {
    MustBeQuiescent,
    Restartable,
}

impl HostRestartPolicy {
    const fn semantic_tag(self) -> u8 {
        match self {
            Self::MustBeQuiescent => 0,
            Self::Restartable => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostCancellationContract {
    RequiredIdempotent,
}

impl HostCancellationContract {
    const fn semantic_tag(self) -> u8 {
        match self {
            Self::RequiredIdempotent => 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostNamedArgumentContract {
    name: String,
    ty: RuntimeCheckedType,
    required: bool,
}

impl HostNamedArgumentContract {
    #[must_use]
    pub fn new(name: String, ty: RuntimeCheckedType, required: bool) -> Self {
        Self { name, ty, required }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn ty(&self) -> &RuntimeCheckedType {
        &self.ty
    }

    #[must_use]
    pub const fn required(&self) -> bool {
        self.required
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostTaskRequestContract {
    kind: HostTaskRequestKind,
    positional: Box<[RuntimeCheckedType]>,
    named: Box<[HostNamedArgumentContract]>,
    spread: HostSpreadContract,
}

impl HostTaskRequestContract {
    pub fn try_new(
        kind: HostTaskRequestKind,
        positional: Box<[RuntimeCheckedType]>,
        named: Box<[HostNamedArgumentContract]>,
        spread: HostSpreadContract,
    ) -> Result<Self, HostOperationCatalogError> {
        if u32::try_from(positional.len()).is_err() || u32::try_from(named.len()).is_err() {
            return Err(HostOperationCatalogError::InvalidRequestContract);
        }
        if named
            .windows(2)
            .any(|pair| pair[0].name() >= pair[1].name())
        {
            return Err(HostOperationCatalogError::InvalidRequestContract);
        }
        Ok(Self {
            kind,
            positional,
            named,
            spread,
        })
    }

    #[must_use]
    pub const fn kind(&self) -> HostTaskRequestKind {
        self.kind
    }

    #[must_use]
    pub fn positional(&self) -> &[RuntimeCheckedType] {
        &self.positional
    }

    #[must_use]
    pub fn named(&self) -> &[HostNamedArgumentContract] {
        &self.named
    }

    #[must_use]
    pub const fn spread(&self) -> HostSpreadContract {
        self.spread
    }

    pub(crate) fn write_semantic(&self, hasher: &mut blake3::Hasher) {
        hasher.update(&[self.kind.semantic_tag()]);
        hasher.update(
            &u32::try_from(self.positional.len())
                .expect("validated positional request count fits u32")
                .to_le_bytes(),
        );
        for ty in &self.positional {
            hasher.update(ty.semantic_identity_digest().as_bytes());
        }
        hasher.update(
            &u32::try_from(self.named.len())
                .expect("validated named request count fits u32")
                .to_le_bytes(),
        );
        for named in &self.named {
            write_host_string(hasher, &named.name);
            hasher.update(named.ty.semantic_identity_digest().as_bytes());
            hasher.update(&[u8::from(named.required)]);
        }
        hasher.update(&[self.spread.semantic_tag()]);
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HostOperationCatalogError {
    #[error("host operation catalog is empty")]
    Empty,
    #[error("host operation rows are not in canonical order")]
    NonCanonicalOrder,
    #[error("host operation catalog contains a duplicate identity")]
    DuplicateIdentity,
    #[error("host operation route is invalid")]
    InvalidRoute,
    #[error("host operation request contract is invalid")]
    InvalidRequestContract,
    #[error("host operation catalog identity does not match its rows")]
    DigestMismatch,
    #[error("host operation is missing from the catalog")]
    MissingOperation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostOperationCatalogRowInput {
    operation: HostOperationCatalogOperation,
    capability: HostCapabilityId,
    request: HostTaskRequestContract,
    route: HostRouteId,
    restart: HostRestartPolicy,
    cancellation: HostCancellationContract,
}

impl HostOperationCatalogRowInput {
    pub fn try_new(
        operation: HostOperationCatalogOperation,
        capability: HostCapabilityId,
        request: HostTaskRequestContract,
        route: HostRouteId,
        restart: HostRestartPolicy,
        cancellation: HostCancellationContract,
    ) -> Result<Self, HostOperationCatalogError> {
        if capability.0.is_empty() {
            return Err(HostOperationCatalogError::InvalidRequestContract);
        }
        Ok(Self {
            operation,
            capability,
            request,
            route,
            restart,
            cancellation,
        })
    }

    #[must_use]
    pub const fn operation(&self) -> HostOperationCatalogOperation {
        self.operation
    }

    #[must_use]
    pub const fn capability(&self) -> &HostCapabilityId {
        &self.capability
    }

    #[must_use]
    pub const fn request(&self) -> &HostTaskRequestContract {
        &self.request
    }

    #[must_use]
    pub const fn route(&self) -> HostRouteId {
        self.route
    }

    #[must_use]
    pub const fn restart(&self) -> HostRestartPolicy {
        self.restart
    }

    #[must_use]
    pub const fn cancellation(&self) -> HostCancellationContract {
        self.cancellation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostOperationCatalogRow {
    identity: HostOperationIdentity,
    capability: HostCapabilityId,
    request: HostTaskRequestContract,
    route: HostRouteId,
    restart: HostRestartPolicy,
    cancellation: HostCancellationContract,
}

impl HostOperationCatalogRow {
    fn seal(input: HostOperationCatalogRowInput, catalog: HostOperationCatalogDigest) -> Self {
        Self {
            identity: input.operation.seal(catalog),
            capability: input.capability,
            request: input.request,
            route: input.route,
            restart: input.restart,
            cancellation: input.cancellation,
        }
    }

    #[must_use]
    pub const fn identity(&self) -> &HostOperationIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn capability(&self) -> &HostCapabilityId {
        &self.capability
    }

    #[must_use]
    pub const fn request(&self) -> &HostTaskRequestContract {
        &self.request
    }

    #[must_use]
    pub const fn route(&self) -> HostRouteId {
        self.route
    }

    #[must_use]
    pub const fn restart(&self) -> HostRestartPolicy {
        self.restart
    }

    #[must_use]
    pub const fn cancellation(&self) -> HostCancellationContract {
        self.cancellation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostOperationCatalog {
    digest: HostOperationCatalogDigest,
    rows: Box<[HostOperationCatalogRow]>,
}

impl HostOperationCatalog {
    pub fn try_new(
        rows: Box<[HostOperationCatalogRowInput]>,
    ) -> Result<Self, HostOperationCatalogError> {
        if rows.is_empty() {
            return Err(HostOperationCatalogError::Empty);
        }
        if rows
            .windows(2)
            .any(|pair| pair[0].operation > pair[1].operation)
        {
            return Err(HostOperationCatalogError::NonCanonicalOrder);
        }
        if rows
            .windows(2)
            .any(|pair| pair[0].operation == pair[1].operation)
        {
            return Err(HostOperationCatalogError::DuplicateIdentity);
        }
        let digest = HostOperationCatalogDigest::from_bytes(host_catalog_digest(&rows));
        let rows = rows
            .into_vec()
            .into_iter()
            .map(|row| HostOperationCatalogRow::seal(row, digest))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(Self { digest, rows })
    }

    #[must_use]
    pub const fn digest(&self) -> HostOperationCatalogDigest {
        self.digest
    }

    #[must_use]
    pub fn rows(&self) -> &[HostOperationCatalogRow] {
        &self.rows
    }

    pub fn resolve(
        &self,
        operation: &HostOperationIdentity,
    ) -> Result<&HostOperationCatalogRow, HostOperationCatalogError> {
        if let HostOperationIdentity::Catalog { catalog, .. } = operation
            && *catalog != self.digest
        {
            return Err(HostOperationCatalogError::DigestMismatch);
        }
        self.rows
            .binary_search_by(|row| row.identity.cmp(operation))
            .ok()
            .and_then(|index| self.rows.get(index))
            .ok_or(HostOperationCatalogError::MissingOperation)
    }
}

fn host_catalog_digest(rows: &[HostOperationCatalogRowInput]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.host-operation-catalog.v1\0");
    hasher.update(&(u32::try_from(rows.len()).unwrap_or(u32::MAX)).to_le_bytes());
    for row in rows {
        row.operation.write_semantic(&mut hasher);
        write_host_string(&mut hasher, &row.capability.0);
        row.request.write_semantic(&mut hasher);
        hasher.update(&row.route.get().get().to_le_bytes());
        hasher.update(&[row.restart.semantic_tag(), row.cancellation.semantic_tag()]);
    }
    *hasher.finalize().as_bytes()
}

fn write_host_string(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(u32::try_from(value.len()).unwrap_or(u32::MAX)).to_le_bytes());
    hasher.update(value.as_bytes());
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum HostTaskRequest {
    FileReadText(FileReadTextRequest),
    FileReadBytes(FileReadBytesRequest),
    FileWriteText(FileWriteTextRequest),
    FileWriteBytes(FileWriteBytesRequest),
    HttpFetch(HttpFetchRequest),
    HttpRespond(HttpRespondRequest),
    ProcessRun(ProcessRunRequest),
    AssetLoad(AssetRequest),
    ShaderCompile(ShaderRequest),
    AudioDecode(AudioDecodeRequest),
    TtsSynthesis(TtsRequest),
    WasmCall(WasmCallRequest),
    SystemInfo(SystemInfoRequest),
    Custom {
        capability: HostCapabilityId,
        operation: String,
        args: Vec<RuntimePayload>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        named_args: Vec<NamedHostArg<RuntimePayload>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        manifest_contract: Option<crate::step::HostCallContractDigest>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileReadTextRequest {
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileReadBytesRequest {
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileWriteTextRequest {
    pub path: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileWriteBytesRequest {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HttpFetchRequest {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<RuntimePayload>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HttpRespondRequest {
    pub request_id: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Option<RuntimePayload>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessRunRequest {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetRequest {
    pub id: String,
    pub kind: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ShaderRequest {
    pub id: String,
    pub entry: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AudioDecodeRequest {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TtsRequest {
    pub voice: Option<String>,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WasmCallRequest {
    pub module: String,
    pub function: String,
    pub args: Vec<RuntimePayload>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SystemInfoRequest {
    pub kind: SystemInfoKind,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum SystemInfoKind {
    CoreCount,
    ThreadCount,
    AvailableParallelism,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TaskEvent {
    pub correlation: TaskCorrelation,
    pub cursor: TaskPublicationCursor,
    pub kind: TaskEventKind,
}

impl TaskEvent {
    pub fn from_dispatch(
        dispatch: TaskDispatchIdentity,
        publication_revision: TaskPublicationRevision,
        kind: TaskEventKind,
    ) -> Self {
        Self {
            correlation: dispatch.correlation,
            cursor: TaskPublicationCursor {
                logical_epoch: dispatch.logical_epoch,
                sequence: TaskSequence(publication_revision.get()),
            },
            kind,
        }
    }
    /// Host task completion may introduce a new affine value, but it cannot
    /// claim a dialogue line lease: those tokens are issued and transferred
    /// only by the runtime line ledger, and host requests cannot carry them.
    pub fn inspect_host_ready_ownership(&self) -> Result<(), RuntimeHostPayloadOwnershipError> {
        let TaskEventKind::Ready(value) = &self.kind else {
            return Ok(());
        };
        inspect_host_payload_ownership(value)
    }
}

pub(crate) fn inspect_host_payload_ownership(
    value: &RuntimePayload,
) -> Result<(), RuntimeHostPayloadOwnershipError> {
    let handles = value.value().affine_line_handles().map_err(|error| {
        RuntimeHostPayloadOwnershipError::InvalidValueGraph {
            message: error.to_string(),
        }
    })?;
    if let Some(handle) = handles.first() {
        return Err(RuntimeHostPayloadOwnershipError::ForeignLineHandle {
            token: handle.token().clone(),
        });
    }
    Ok(())
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeHostPayloadOwnershipError {
    #[error("host result contains a dialogue line handle not issued to the host: {token:?}")]
    ForeignLineHandle {
        token: crate::runtime_id::RuntimeLineHandleToken,
    },
    #[error("host result has an invalid affine value graph: {message}")]
    InvalidValueGraph { message: String },
}

#[cfg(test)]
mod host_ready_ownership_tests {
    use super::*;
    use crate::pattern::{
        RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId,
    };
    use crate::runtime_id::{
        DialogueActivationId, RuntimeDialogueContentPlanId, RuntimeLineHandleSiteId,
        RuntimeLineHandleToken,
    };
    use crate::value::{RuntimeHandleKind, RuntimeOpaquePersistence, RuntimeOpaqueValueClass};

    fn ready(value: RuntimeValue) -> TaskEvent {
        TaskEvent {
            correlation: crate::tests::reusable_need("ready argument").correlation(),
            cursor: TaskPublicationCursor {
                logical_epoch: LogicalEpoch(0),
                sequence: TaskSequence(1),
            },
            kind: TaskEventKind::Ready(RuntimePayload(value)),
        }
    }

    #[test]
    fn host_ready_rejects_nested_line_lease_but_accepts_other_affine_payload() {
        let token = RuntimeLineHandleToken::new(
            DialogueActivationId::new(
                crate::effect::RuntimeArtifactFingerprint::try_from_bytes([7; 32]).unwrap(),
                RuntimePersistentFiberId::from_allocated(11),
                RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::new(3).unwrap()),
                17,
            ),
            RuntimeLineHandleSiteId::from_zero_based(3),
            23,
        );
        let kind = RuntimeHandleKind::Voice;
        let owner = RuntimeOpaqueTypeOwner::exact_with(
            RuntimeOpaqueTypeProducerId::try_new("std.line.voice_handle").unwrap(),
            RuntimeSemanticTypeId::from_bytes([9; 32]),
            RuntimeOpaqueValueClass::AffineHandle(kind),
            RuntimeOpaquePersistence::SnapshotOnly,
        );
        let handle = owner.try_wrap(token.encode_payload()).unwrap();
        let event = ready(RuntimeValue::Tuple(vec![RuntimeValue::Unit, handle]));
        assert_eq!(
            event.inspect_host_ready_ownership(),
            Err(RuntimeHostPayloadOwnershipError::ForeignLineHandle { token })
        );

        let other_affine = ready(RuntimeValue::NeedHandle(crate::tests::reusable_need(
            "need.ready",
        )));
        assert!(other_affine.inspect_host_ready_ownership().is_ok());
    }
}

/// A completion publication does not belong to one live scheduler owner.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TaskCompletionError {
    #[error("completion references unknown task {task_id:?}")]
    UnknownTask { task_id: TaskId },
    #[error("task {task_id:?} completion does not match its registered dispatch identity")]
    DispatchMismatch { task_id: TaskId },
    #[error("task {task_id:?} publication revision is stale or duplicated")]
    StalePublication { task_id: TaskId },
    #[error("task {task_id:?} cannot accept a nonterminal publication at the maximum revision")]
    PublicationRevisionExhausted { task_id: TaskId },
    #[error("task {task_id:?} received more than one terminal completion")]
    DuplicateTerminalEvent { task_id: TaskId },
    #[error("task {task_id:?} received a publication after its terminal completion")]
    EventAfterTerminal { task_id: TaskId },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum TaskEventKind {
    Ready(RuntimePayload),
    InfrastructureFailure(RuntimeTaskFailure),
    Cancelled,
    Progress(Progress),
}

pub trait TaskHost {
    fn ensure_task(&mut self, spec: TaskSpec) -> Result<TaskHandle, TaskEnsureError>;
    fn cancel_scope(&mut self, scope: CancelScopeId);
    fn poll_frame(&mut self, budget: SchedulerBudget) -> Vec<TaskEvent>;
}

impl AwaitTarget {
    pub fn new(need: NeedId, task: TaskId, request: HostTaskRequestTemplate) -> Self {
        Self {
            need,
            task,
            outcome: TaskOutcomeContract::default(),
            request,
        }
    }

    pub fn with_outcome(
        need: NeedId,
        task: TaskId,
        outcome: TaskOutcomeContract,
        request: HostTaskRequestTemplate,
    ) -> Self {
        Self {
            need,
            task,
            outcome,
            request,
        }
    }
}

impl HostTaskRequestTemplate {
    pub fn captures(&self) -> &[RuntimeLocalDeclarationId] {
        &self.captures
    }
}

impl RuntimeHostArgumentTemplate {
    pub fn positional(value: RuntimeExpr) -> Self {
        Self::Positional(value)
    }

    pub fn named(name: impl Into<String>, value: RuntimeExpr) -> Self {
        Self::Named(NamedHostArg {
            name: name.into(),
            value,
        })
    }

    pub fn spread(value: RuntimeExpr) -> Self {
        Self::Spread(value)
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Named(argument) => Some(&argument.name),
            Self::Positional(_) | Self::Spread(_) => None,
        }
    }

    pub fn value(&self) -> &RuntimeExpr {
        match self {
            Self::Positional(value) | Self::Spread(value) => value,
            Self::Named(argument) => &argument.value,
        }
    }

    pub const fn is_spread(&self) -> bool {
        matches!(self, Self::Spread(_))
    }
}

impl HostTaskRequest {
    pub(crate) fn runtime_values(&self) -> impl Iterator<Item = &RuntimeValue> {
        let (body, args, named): (
            Option<&RuntimePayload>,
            &[RuntimePayload],
            &[NamedHostArg<RuntimePayload>],
        ) = match self {
            Self::HttpFetch(request) => (request.body.as_ref(), &[], &[]),
            Self::HttpRespond(request) => (request.body.as_ref(), &[], &[]),
            Self::WasmCall(request) => (None, &request.args, &[]),
            Self::Custom {
                args, named_args, ..
            } => (None, args, named_args),
            Self::FileReadText(_)
            | Self::FileReadBytes(_)
            | Self::FileWriteText(_)
            | Self::FileWriteBytes(_)
            | Self::ProcessRun(_)
            | Self::AssetLoad(_)
            | Self::ShaderCompile(_)
            | Self::AudioDecode(_)
            | Self::TtsSynthesis(_)
            | Self::SystemInfo(_) => (None, &[], &[]),
        };
        body.into_iter()
            .chain(args.iter())
            .chain(named.iter().map(|arg| &arg.value))
            .map(RuntimePayload::value)
    }

    pub fn custom(
        capability: impl Into<String>,
        operation: impl Into<String>,
        args: impl IntoIterator<Item = RuntimePayload>,
    ) -> Self {
        Self::Custom {
            capability: HostCapabilityId(capability.into()),
            operation: operation.into(),
            args: args.into_iter().collect(),
            named_args: Vec::new(),
            manifest_contract: None,
        }
    }

    pub fn custom_with_named_args(
        capability: impl Into<String>,
        operation: impl Into<String>,
        args: impl IntoIterator<Item = RuntimePayload>,
        named_args: impl IntoIterator<Item = (String, RuntimePayload)>,
    ) -> Self {
        Self::Custom {
            capability: HostCapabilityId(capability.into()),
            operation: operation.into(),
            args: args.into_iter().collect(),
            named_args: named_args
                .into_iter()
                .map(|(name, value)| NamedHostArg { name, value })
                .collect(),
            manifest_contract: None,
        }
    }

    pub fn custom_with_named_args_and_manifest_contract(
        capability: impl Into<String>,
        operation: impl Into<String>,
        args: impl IntoIterator<Item = RuntimePayload>,
        named_args: impl IntoIterator<Item = (String, RuntimePayload)>,
        manifest_contract: crate::step::HostCallContractDigest,
    ) -> Self {
        let mut request = Self::custom_with_named_args(capability, operation, args, named_args);
        if let Self::Custom {
            manifest_contract: selected,
            ..
        } = &mut request
        {
            *selected = Some(manifest_contract);
        }
        request
    }

    pub fn debug_label(&self) -> String {
        match self {
            Self::FileReadText(request) => format!("file.read_text {}", request.path),
            Self::FileReadBytes(request) => format!("file.read_bytes {}", request.path),
            Self::FileWriteText(request) => format!("file.write_text {}", request.path),
            Self::FileWriteBytes(request) => format!("file.write_bytes {}", request.path),
            Self::HttpFetch(request) => format!("http.fetch {} {}", request.method, request.url),
            Self::HttpRespond(request) => {
                format!("http.respond {} {}", request.request_id, request.status)
            }
            Self::ProcessRun(request) => format!("process.run {}", request.program),
            Self::AssetLoad(request) => format!("asset.load {} {}", request.kind, request.id),
            Self::ShaderCompile(request) => format!("shader.compile {}", request.id),
            Self::AudioDecode(request) => format!("audio.decode {}", request.id),
            Self::TtsSynthesis(request) => {
                format!(
                    "tts.synthesis {}",
                    request.voice.as_deref().unwrap_or("default")
                )
            }
            Self::WasmCall(request) => {
                format!("wasm.call {}::{}", request.module, request.function)
            }
            Self::SystemInfo(request) => format!("system.{}", request.kind.as_str()),
            Self::Custom {
                capability,
                operation,
                ..
            } => format!("{}.{}", capability.0, operation),
        }
    }

    pub fn host_call_id(&self) -> String {
        match self {
            Self::FileReadText(_) => "fs.read_text".to_owned(),
            Self::FileReadBytes(_) => "fs.read_bytes".to_owned(),
            Self::FileWriteText(_) => "fs.write_text".to_owned(),
            Self::FileWriteBytes(_) => "fs.write_bytes".to_owned(),
            Self::HttpFetch(_) => "http.fetch".to_owned(),
            Self::HttpRespond(_) => "http.respond".to_owned(),
            Self::ProcessRun(_) => "process.run".to_owned(),
            Self::AssetLoad(request) => format!("asset.{}", request.kind),
            Self::ShaderCompile(_) => "shader.compile".to_owned(),
            Self::AudioDecode(_) => "audio.decode".to_owned(),
            Self::TtsSynthesis(_) => "tts.synthesize".to_owned(),
            Self::WasmCall(_) => "wasm.call".to_owned(),
            Self::SystemInfo(request) => format!("system.{}", request.kind.as_str()),
            Self::Custom {
                capability,
                operation,
                ..
            } => format!("{}.{}", capability.0, operation),
        }
    }

    pub const fn task_class(&self) -> TaskClass {
        match self {
            Self::FileReadText(_)
            | Self::FileReadBytes(_)
            | Self::FileWriteText(_)
            | Self::FileWriteBytes(_)
            | Self::HttpFetch(_)
            | Self::HttpRespond(_)
            | Self::ProcessRun(_) => TaskClass::Io,
            Self::AssetLoad(_) => TaskClass::AssetDecode,
            Self::ShaderCompile(_) => TaskClass::ShaderCompile,
            Self::AudioDecode(_) => TaskClass::AudioDecode,
            Self::TtsSynthesis(_) => TaskClass::TtsSynthesis,
            Self::WasmCall(_) => TaskClass::WasmCall,
            Self::SystemInfo(_) => TaskClass::Cpu,
            Self::Custom { .. } => TaskClass::Background,
        }
    }
}

impl SystemInfoKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CoreCount => "core_count",
            Self::ThreadCount => "thread_count",
            Self::AvailableParallelism => "available_parallelism",
        }
    }
}

impl From<&str> for HostCapabilityId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for HostCapabilityId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Returns task events in replay-stable completion order.
pub fn normalize_task_events(mut events: Vec<TaskEvent>) -> Vec<TaskEvent> {
    if events.len() > 1 && !task_events_are_normalized(&events) {
        events.sort_by(compare_task_events);
    }
    events
}

/// Returns true when task events are already in replay-stable completion order.
pub fn task_events_are_normalized(events: &[TaskEvent]) -> bool {
    events
        .windows(2)
        .all(|pair| compare_task_events(&pair[0], &pair[1]).is_le())
}

/// Compares task events by replay-stable completion order.
pub fn compare_task_events(left: &TaskEvent, right: &TaskEvent) -> std::cmp::Ordering {
    left.cursor
        .logical_epoch
        .cmp(&right.cursor.logical_epoch)
        .then_with(|| left.correlation.cmp(&right.correlation))
        .then_with(|| left.cursor.sequence.cmp(&right.cursor.sequence))
}

/// Returns producer-owned Need states in replay-stable publication order.
pub fn normalize_runtime_need_states(mut states: Vec<RuntimeNeedState>) -> Vec<RuntimeNeedState> {
    if states.len() > 1 && !runtime_need_states_are_normalized(&states) {
        states.sort_by(compare_runtime_need_states);
    }
    states
}

/// Returns true when Need states are already in replay-stable order.
pub fn runtime_need_states_are_normalized(states: &[RuntimeNeedState]) -> bool {
    states
        .windows(2)
        .all(|pair| compare_runtime_need_states(&pair[0], &pair[1]).is_le())
}

/// Compares Need states by the same deterministic epoch/identity/sequence
/// vocabulary used by task events.
pub fn compare_runtime_need_states(
    left: &RuntimeNeedState,
    right: &RuntimeNeedState,
) -> std::cmp::Ordering {
    left.cursor
        .cmp(&right.cursor)
        .then_with(|| left.correlation.cmp(&right.correlation))
}

/// Selects the current state for one Need from a normalized publication list.
///
/// Progress and `NotStarted` publications may advance until the first terminal
/// publication. Once Ready or Cancelled is committed, later publications for
/// the same identity cannot replace it.
pub fn resolved_runtime_need_state<'a>(
    states: &'a [RuntimeNeedState],
    correlation: &TaskCorrelation,
) -> Option<&'a RuntimeNeedState> {
    let mut current = None;
    for candidate in states
        .iter()
        .filter(|candidate| &candidate.correlation == correlation)
    {
        current = Some(candidate);
        if candidate.state().is_terminal() {
            break;
        }
    }
    current
}
