use crate::entry::RuntimeValueDigest;
use crate::pattern::RuntimeCheckedType;
use crate::value::{RuntimeExpr, RuntimePayload, RuntimeValue};
use arcweft_need::{Need, Progress};
use serde::{Deserialize, Serialize};

use crate::runtime_id::RuntimeLocalDeclarationId;
use std::num::NonZeroU32;
use thiserror::Error;

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

/// Source-independent ordinal assigned to one launch of a producer instance.
/// Join uses the zero ordinal; positive `AlwaysStart` candidates remain journal
/// authority and are not exposed as raw constructors.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct TaskLaunchOrdinal(u64);

impl TaskLaunchOrdinal {
    pub const JOIN: Self = Self(0);

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

macro_rules! semantic_digest {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        #[repr(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            #[must_use]
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

semantic_digest!(NeedProducerContractDigest);
semantic_digest!(TaskPlanSemanticDigest);
semantic_digest!(RuntimeTypeSemanticDigest);
semantic_digest!(NeedTimeoutContractDigest);

/// Closed producer family used by the canonical instance-key transcript.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NeedProducerFamily {
    StructuredTaskPlan,
    AwbcTaskPlan,
    ViewMatchSubscription,
    AwaitManyBase,
    AwaitManyChild,
    Timeout,
    LineTask,
    HostAdapterTask,
    MakeNeedHandle,
}

impl NeedProducerFamily {
    const fn semantic_tag(self) -> u8 {
        match self {
            Self::StructuredTaskPlan => 0,
            Self::AwbcTaskPlan => 1,
            Self::ViewMatchSubscription => 2,
            Self::AwaitManyBase => 3,
            Self::AwaitManyChild => 4,
            Self::Timeout => 5,
            Self::LineTask => 6,
            Self::HostAdapterTask => 7,
            Self::MakeNeedHandle => 8,
        }
    }
}

/// Complete typed producer contract used as the sole source of its instance
/// identity.  The individual semantic fields are intentionally not exposed as
/// an alternate task/Need identity authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeedProducerSpec {
    family: NeedProducerFamily,
    contract: NeedProducerContractDigest,
    plan: TaskPlanSemanticDigest,
    producer_site: u32,
    payload_type: RuntimeTypeSemanticDigest,
    arguments: RuntimeValueDigest,
}

/// First-error identity failures shared by the standalone Cut 4 substrate.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum TaskIdentityError {
    #[error("a fixed runtime identity may not be all zero")]
    ZeroFixedIdentity,
}

impl NeedProducerSpec {
    #[must_use]
    pub const fn new(
        family: NeedProducerFamily,
        contract: NeedProducerContractDigest,
        plan: TaskPlanSemanticDigest,
        producer_site: u32,
        payload_type: RuntimeTypeSemanticDigest,
        arguments: RuntimeValueDigest,
    ) -> Self {
        Self {
            family,
            contract,
            plan,
            producer_site,
            payload_type,
            arguments,
        }
    }

    /// Derives the fixed producer-instance identity from this complete spec.
    pub fn instance_key(&self) -> Result<NeedProducerInstanceKey, TaskIdentityError> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.need.producer-instance.v1\0");
        hasher.update(&[self.family.semantic_tag()]);
        hasher.update(self.contract.as_bytes());
        hasher.update(self.plan.as_bytes());
        hasher.update(&self.producer_site.to_le_bytes());
        hasher.update(self.payload_type.as_bytes());
        hasher.update(self.arguments.as_bytes());
        let bytes = *hasher.finalize().as_bytes();
        if bytes == [0; 32] {
            return Err(TaskIdentityError::ZeroFixedIdentity);
        }
        Ok(NeedProducerInstanceKey(bytes))
    }

    pub const fn family(&self) -> NeedProducerFamily {
        self.family
    }

    pub const fn contract(&self) -> NeedProducerContractDigest {
        self.contract
    }

    pub const fn plan(&self) -> TaskPlanSemanticDigest {
        self.plan
    }

    pub const fn producer_site(&self) -> u32 {
        self.producer_site
    }

    pub const fn payload_type(&self) -> RuntimeTypeSemanticDigest {
        self.payload_type
    }

    pub const fn arguments(&self) -> RuntimeValueDigest {
        self.arguments
    }
}

/// Fixed identity of a complete producer spec.  It has no public raw-byte
/// constructor; only `NeedProducerSpec::instance_key` can issue it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct NeedProducerInstanceKey([u8; 32]);

impl NeedProducerInstanceKey {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Sans-I/O protocol implemented by the accepted upper View product owner.
/// Core validates only the typed request projection and never copies View
/// catalog rows or depends on `arcweft-view`.
pub trait ViewTaskPlanAuthority {
    fn validate_view_task_plan(
        &self,
        request: ViewTaskPlanValidation<'_>,
    ) -> Result<(), ViewTaskPlanValidationError>;
}

#[derive(Clone, Copy, Debug)]
pub struct ViewTaskPlanValidation<'a> {
    pub generation: GenerationId,
    pub producer: &'a NeedProducerSpec,
    pub outcome: &'a TaskOutcomeContract,
    pub request: &'a HostTaskRequest,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ViewTaskPlanValidationError {
    #[error("View task-plan validation rejected the generation")]
    GenerationMismatch,
    #[error("View task-plan validation rejected the producer")]
    ProducerMismatch,
    #[error("View task-plan validation rejected the outcome")]
    OutcomeMismatch,
    #[error("View task-plan validation rejected the Host request")]
    RequestMismatch,
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[test]
    fn generation_zero_and_join_ordinal_are_valid_values() {
        assert_eq!(GenerationId::new(0).get(), 0);
        assert_eq!(TaskLaunchOrdinal::JOIN.get(), 0);
    }

    #[test]
    fn producer_instance_key_commits_every_typed_spec_field() {
        let base = NeedProducerSpec::new(
            NeedProducerFamily::StructuredTaskPlan,
            NeedProducerContractDigest::from_bytes([1; 32]),
            TaskPlanSemanticDigest::from_bytes([2; 32]),
            7,
            RuntimeTypeSemanticDigest::from_bytes([3; 32]),
            RuntimeValueDigest::from_bytes([4; 32]),
        );
        let changed = NeedProducerSpec::new(
            NeedProducerFamily::StructuredTaskPlan,
            NeedProducerContractDigest::from_bytes([1; 32]),
            TaskPlanSemanticDigest::from_bytes([2; 32]),
            8,
            RuntimeTypeSemanticDigest::from_bytes([3; 32]),
            RuntimeValueDigest::from_bytes([4; 32]),
        );
        assert_ne!(
            base.instance_key().expect("base key").as_bytes(),
            changed.instance_key().expect("changed key").as_bytes()
        );
    }

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
pub struct TaskId(pub String);

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TaskKey(pub String);

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct NeedId(pub String);

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

/// Replay-stable position of one publication from a single task.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct TaskPublicationCursor {
    pub logical_epoch: LogicalEpoch,
    pub sequence: TaskSequence,
}

impl TaskPublicationCursor {
    #[must_use]
    pub const fn from_event(event: &TaskEvent) -> Self {
        Self {
            logical_epoch: event.logical_epoch,
            sequence: event.sequence,
        }
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
    logical_epoch: LogicalEpoch,
    need: NeedId,
    sequence: TaskSequence,
    state: Need<RuntimePayload>,
}

/// The exact payload type a host task may publish through temporal `Ready`.
///
/// Fallible producers admit a `Result<T, E>` payload here. Infrastructure
/// failures and cancellation are control outcomes and are not alternate typed
/// payload coordinates.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TaskOutcomeContract {
    payload: RuntimeCheckedType,
}

impl TaskOutcomeContract {
    #[must_use]
    pub const fn new(payload: RuntimeCheckedType) -> Self {
        Self { payload }
    }

    #[must_use]
    pub const fn payload(&self) -> &RuntimeCheckedType {
        &self.payload
    }

    pub fn try_payload(&self, value: RuntimeValue) -> Result<RuntimePayload, String> {
        if self.payload.accepts_value(&value) {
            Ok(RuntimePayload::new(value))
        } else {
            Err("host task payload does not satisfy its checked outcome contract".to_owned())
        }
    }

    pub fn try_result_ok(&self, value: RuntimeValue) -> Result<RuntimePayload, String> {
        let RuntimeCheckedType::Result { ok, .. } = &self.payload else {
            return Err("host task outcome is not an admitted Result payload".to_owned());
        };
        if !ok.accepts_value(&value) {
            return Err(
                "host task Result::Ok payload does not satisfy its checked type".to_owned(),
            );
        }
        self.try_payload(RuntimeValue::result_ok(value))
    }

    pub fn try_result_err(&self, value: RuntimeValue) -> Result<RuntimePayload, String> {
        let RuntimeCheckedType::Result { error, .. } = &self.payload else {
            return Err("host task outcome is not an admitted Result payload".to_owned());
        };
        if !error.accepts_value(&value) {
            return Err(
                "host task Result::Err payload does not satisfy its checked type".to_owned(),
            );
        }
        self.try_payload(RuntimeValue::result_err(value))
    }

    #[must_use]
    pub const fn result_error(&self) -> Option<&RuntimeCheckedType> {
        match &self.payload {
            RuntimeCheckedType::Result { error, .. } => Some(error),
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
        logical_epoch: LogicalEpoch,
        need: NeedId,
        sequence: TaskSequence,
        state: Need<RuntimePayload>,
    ) -> Self {
        Self {
            logical_epoch,
            need,
            sequence,
            state,
        }
    }

    pub const fn logical_epoch(&self) -> LogicalEpoch {
        self.logical_epoch
    }

    pub const fn need(&self) -> &NeedId {
        &self.need
    }

    pub const fn sequence(&self) -> TaskSequence {
        self.sequence
    }

    pub const fn state(&self) -> &Need<RuntimePayload> {
        &self.state
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
    pub need: NeedId,
    pub task: TaskId,
    pub outcome: TaskOutcomeContract,
    pub source: RuntimeExpr,
    pub item_binding: RuntimeLocalDeclarationId,
    pub limit: usize,
    pub request: HostTaskRequestTemplate,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskRequestTemplate {
    pub capability: HostCapabilityId,
    pub operation: String,
    pub args: Vec<RuntimeHostArgumentTemplate>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeHostArgumentTemplate {
    Positional(RuntimeExpr),
    Named(NamedHostArg<RuntimeExpr>),
    Spread(RuntimeExpr),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct NamedHostArg<T> {
    pub name: String,
    pub value: T,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TaskSpec {
    pub id: TaskId,
    pub key: TaskKey,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub policy: TaskPolicy,
    pub outcome: TaskOutcomeContract,
    pub request: HostTaskRequest,
    pub debug_label: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TaskHandle {
    pub id: TaskId,
    pub key: TaskKey,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SchedulerBudget {
    pub max_events: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum TaskPolicy {
    JoinSameKey,
    AlwaysStart,
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FileReadTextRequest {
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FileReadBytesRequest {
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FileWriteTextRequest {
    pub path: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FileWriteBytesRequest {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HttpFetchRequest {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<RuntimePayload>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HttpRespondRequest {
    pub request_id: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Option<RuntimePayload>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProcessRunRequest {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AssetRequest {
    pub id: String,
    pub kind: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ShaderRequest {
    pub id: String,
    pub entry: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AudioDecodeRequest {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TtsRequest {
    pub voice: Option<String>,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WasmCallRequest {
    pub module: String,
    pub function: String,
    pub args: Vec<RuntimePayload>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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
    pub logical_epoch: LogicalEpoch,
    pub task_id: TaskId,
    pub sequence: TaskSequence,
    pub kind: TaskEventKind,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum TaskEventKind {
    Ready(RuntimePayload),
    Failed(String),
    Cancelled,
    Progress(Progress),
}

pub trait TaskHost {
    fn ensure_task(&mut self, spec: TaskSpec) -> TaskHandle;
    fn cancel_scope(&mut self, scope: CancelScopeId);
    fn poll_frame(&mut self, budget: SchedulerBudget) -> Vec<TaskEvent>;
}

impl TaskSpec {
    pub fn new(
        id: TaskId,
        key: TaskKey,
        class: TaskClass,
        priority: TaskPriority,
        cancel_scope: CancelScopeId,
        policy: TaskPolicy,
        request: HostTaskRequest,
    ) -> Self {
        let debug_label = request.debug_label();
        Self {
            id,
            key,
            class,
            priority,
            cancel_scope,
            policy,
            outcome: TaskOutcomeContract::default(),
            request,
            debug_label,
        }
    }

    #[must_use]
    pub fn with_outcome(mut self, outcome: TaskOutcomeContract) -> Self {
        self.outcome = outcome;
        self
    }
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

impl AwaitManyTarget {
    pub fn new(
        need: NeedId,
        task: TaskId,
        source: RuntimeExpr,
        item_binding: RuntimeLocalDeclarationId,
        limit: usize,
        request: HostTaskRequestTemplate,
    ) -> Self {
        Self {
            need,
            task,
            outcome: TaskOutcomeContract::default(),
            source,
            item_binding,
            limit,
            request,
        }
    }
}

impl HostTaskRequestTemplate {
    pub fn new(
        capability: impl Into<String>,
        operation: impl Into<String>,
        args: impl IntoIterator<Item = RuntimeHostArgumentTemplate>,
    ) -> Self {
        Self {
            capability: HostCapabilityId(capability.into()),
            operation: operation.into(),
            args: args.into_iter().collect(),
        }
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
        }
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
    left.logical_epoch
        .cmp(&right.logical_epoch)
        .then_with(|| left.task_id.cmp(&right.task_id))
        .then_with(|| left.sequence.cmp(&right.sequence))
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
    left.logical_epoch()
        .cmp(&right.logical_epoch())
        .then_with(|| left.need().cmp(right.need()))
        .then_with(|| left.sequence().cmp(&right.sequence()))
}

/// Selects the current state for one Need from a normalized publication list.
///
/// Progress and `NotStarted` publications may advance until the first terminal
/// publication. Once Ready or Cancelled is committed, later publications for
/// the same identity cannot replace it.
pub fn resolved_runtime_need_state<'a>(
    states: &'a [RuntimeNeedState],
    need: &NeedId,
) -> Option<&'a RuntimeNeedState> {
    let mut current = None;
    for candidate in states.iter().filter(|candidate| candidate.need() == need) {
        current = Some(candidate);
        if candidate.state().is_terminal() {
            break;
        }
    }
    current
}
