//! Normative Rust-shaped contract for Lang-01.5.1.1.2.1.1.1.1.1.1.1.1.
//!
//! This is design material, not a standalone crate or a production patch.
//! Unqualified existing names stay on their current production owners. New
//! behavior for an Arcweft-owned type is implemented inherently on that owner.

use std::collections::{BTreeMap, BTreeSet};
use std::num::{NonZeroU32, NonZeroU64};

// -------------------------------------------------------------------------
// arcweft_core::task — final task, receipt, journal, and adapter seams
// -------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GenerationId(u64);

impl GenerationId {
    /// Generation zero is a valid first generation, not an absence sentinel.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskLaunchOrdinal(u64);

impl TaskLaunchOrdinal {
    pub const JOIN: Self = Self(0);

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NeedProducerInstanceKey([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NeedId([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskKey([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskId([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NeedProducerContractDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskPlanSemanticDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeTypeSemanticDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NeedTimeoutContractDigest([u8; 32]);

impl NeedProducerContractDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl TaskPlanSemanticDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl RuntimeTypeSemanticDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl NeedTimeoutContractDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TaskPolicy {
    JoinSameKey,
    AlwaysStart,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeedProducerSpec {
    family: NeedProducerFamily,
    contract: NeedProducerContractDigest,
    plan: TaskPlanSemanticDigest,
    producer_site: u32,
    payload_type: RuntimeTypeSemanticDigest,
    arguments: RuntimeValueDigest,
}

impl NeedProducerSpec {
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

    pub fn instance_key(&self) -> Result<NeedProducerInstanceKey, TaskIdentityError> {
        unimplemented!()
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

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskCorrelation {
    generation: GenerationId,
    producer: NeedProducerInstanceKey,
    policy: TaskPolicy,
    ordinal: TaskLaunchOrdinal,
    need_id: NeedId,
    task_key: TaskKey,
    task_id: TaskId,
}

impl TaskCorrelation {
    pub fn derive(
        generation: GenerationId,
        producer: NeedProducerInstanceKey,
        policy: TaskPolicy,
        ordinal: TaskLaunchOrdinal,
    ) -> Result<Self, TaskIdentityError> {
        unimplemented!()
    }

    pub fn validate(&self) -> Result<(), TaskIdentityError> {
        unimplemented!()
    }

    pub const fn generation(&self) -> GenerationId {
        self.generation
    }
    pub const fn producer(&self) -> NeedProducerInstanceKey {
        self.producer
    }
    pub const fn policy(&self) -> TaskPolicy {
        self.policy
    }
    pub const fn ordinal(&self) -> TaskLaunchOrdinal {
        self.ordinal
    }
    pub const fn need_id(&self) -> NeedId {
        self.need_id
    }
    pub const fn task_key(&self) -> TaskKey {
        self.task_key
    }
    pub const fn task_id(&self) -> TaskId {
        self.task_id
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskSpec {
    producer: NeedProducerSpec,
    class: TaskClass,
    priority: TaskPriority,
    cancel_scope: CancelScopeId,
    policy: TaskPolicy,
    outcome: TaskOutcomeContract,
    execution: TaskExecution,
    debug: TaskDebugMetadata,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaskExecution {
    Host(HostTaskRequest),
    Runtime(RuntimeTaskRequest),
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeTaskRequest {
    AwaitManyAggregate(RuntimeAwaitManyAggregateRequest),
    Timeout(RuntimeTimeoutRequest),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeTimeoutRequest {
    source: RuntimeNeedHandle,
    requested_limit: LogicalDuration,
    contract: NeedTimeoutContractDigest,
}

impl RuntimeTimeoutRequest {
    pub fn new(
        source: RuntimeNeedHandle,
        requested_limit: LogicalDuration,
        contract: NeedTimeoutContractDigest,
    ) -> Self {
        Self {
            source,
            requested_limit,
            contract,
        }
    }

    pub const fn source(&self) -> &RuntimeNeedHandle {
        &self.source
    }
    pub const fn requested_limit(&self) -> LogicalDuration {
        self.requested_limit
    }
    pub const fn contract(&self) -> NeedTimeoutContractDigest {
        self.contract
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskDebugMetadata {
    label: Option<String>,
    origin: Option<String>,
}

impl TaskDebugMetadata {
    pub fn new(label: Option<String>, origin: Option<String>) -> Self {
        Self { label, origin }
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
    pub fn origin(&self) -> Option<&str> {
        self.origin.as_deref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaskPriority(i32);

impl TaskPriority {
    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i32 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum HostTaskRequest {
    FileReadText {
        path: String,
    },
    FileReadBytes {
        path: String,
    },
    FileWriteText {
        path: String,
        text: String,
    },
    FileWriteBytes {
        path: String,
        bytes: Box<[u8]>,
    },
    HttpFetch {
        url: String,
        method: String,
        headers: Box<[(String, String)]>,
        body: Option<RuntimeValue>,
    },
    HttpRespond {
        request_id: String,
        status: u16,
        headers: Box<[(String, String)]>,
        body: Option<RuntimeValue>,
    },
    ProcessRun {
        program: String,
        args: Box<[String]>,
        env: Box<[(String, String)]>,
    },
    AssetLoad {
        id: String,
        kind: String,
    },
    ShaderCompile {
        id: String,
        entry: Option<String>,
    },
    AudioDecode {
        id: String,
    },
    TtsSynthesis {
        voice: Option<String>,
        text: String,
    },
    WasmCall {
        module: String,
        function: String,
        args: Box<[RuntimeValue]>,
    },
    SystemInfo {
        kind: SystemInfoKind,
    },
    Custom {
        operation: HostOperationIdentity,
        args: Box<[RuntimeValue]>,
        named_args: Box<[NamedRuntimeValue]>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NamedRuntimeValue {
    name: String,
    value: RuntimeValue,
}

impl NamedRuntimeValue {
    pub fn new(name: String, value: RuntimeValue) -> Self {
        Self { name, value }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub const fn value(&self) -> &RuntimeValue {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskIdentityError {
    ZeroFixedIdentity,
    JoinOrdinalNotZero,
    AlwaysStartOrdinalNotPositive,
    CorrelationMismatch,
    ArithmeticOverflow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeNeedHandleConstructionError {
    InactiveGeneration,
    MissingTaskRow,
    MissingNeedRow,
    UncommittedLaunch,
    CorrelationMismatch,
    ProducerMismatch,
    OutcomeMismatch,
    SpecMismatch,
    PolicyOrdinalMismatch,
    ReusableRequiresHostJoin,
    InvalidTaskSpec,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeNeedUseError {
    StaleGeneration,
    MissingCommittedLaunch,
    CorrelationMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskOrdinalAllocationError {
    Overflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskObserverAllocationError {
    Overflow,
    TooManyObservers,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedObserverIds {
    ids: Box<[TaskObserverId]>,
    next: NonZeroU64,
}

impl PlannedObserverIds {
    pub fn ids(&self) -> &[TaskObserverId] {
        &self.ids
    }
    pub const fn next(&self) -> NonZeroU64 {
        self.next
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostTaskReceiptError {
    GenerationMismatch,
    RowCountMismatch,
    DuplicateSourceIndex,
    NonCanonicalSourceOrder,
    MissingInputRow,
    CorrelationMismatch,
    OperationMismatch,
    ZeroCapability,
    CapabilityRouteMismatch,
    ActiveCapabilityReuse,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostTaskBatchError {
    Empty,
    GenerationMismatch,
    DuplicateSourceIndex,
    DuplicateCorrelation,
    NonCanonicalOrder,
    CorrelationMismatch,
    InvalidRebind,
    InvalidCancelCommand,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskValidationAuthorityError {
    GenerationMismatch,
    InvalidAwbcProgram,
    MissingStructuredTaskAuthority,
    MissingViewTaskAuthority,
    HostCatalogMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskSpecValidationError {
    ScalarOrLimit,
    ProducerIdentity,
    FamilyExecutionPolicy,
    PayloadOutcomeMismatch,
    MissingPlan,
    ProducerSiteMismatch,
    PlanDigestMismatch,
    HostOperationMismatch,
    HostRequestMismatch,
    RuntimeRequestMismatch,
    ArgumentDigestMismatch,
    ViewAuthority(ViewTaskPlanValidationError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewTaskPlanValidationError {
    MissingProgram,
    StaleRevision,
    MissingSite,
    ProducerMismatch,
    OutcomeMismatch,
    RequestMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeSnapshotAuthorityError {
    GenerationMismatch,
    InvalidTaskAuthority,
    InvalidJournal,
    AwbcIdentityMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostOperationCatalogError {
    Empty,
    NonCanonicalOrder,
    DuplicateIdentity,
    InvalidRoute,
    InvalidRequestContract,
    DigestMismatch,
    MissingOperation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NeedProducerTemplateError {
    InvalidCombinedArgument,
    MissingCapturedValue,
    SourceIndexMismatch,
    SourceItemMismatch,
    InvalidConstant,
    HostMaterialization,
    RuntimeMaterialization,
    InvalidTaskSpec,
    WorkLimit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AwaitManyError {
    SourceIndexOutOfRange,
    SourceCountOverflow,
    InvalidTemplate,
    InvalidChildSpec,
    WorkLimit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeValueCanonicalLimits {
    pub max_depth: u32,
    pub max_nodes: u64,
    pub max_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskCancelReason {
    Explicit,
    Scope,
    Parent,
    GenerationReplacement,
}

#[derive(Clone, Debug)]
pub struct RuntimeNeedHandle {
    correlation: TaskCorrelation,
    producer: NeedProducerSpec,
    outcome: TaskOutcomeContract,
    state: RuntimeNeedHandleState,
}

#[derive(Clone, Debug)]
pub enum RuntimeNeedHandleState {
    ReusableJoin { spec: Box<TaskSpec> },
    AcceptedLaunch,
}

/// Public cross-crate proof with non-public construction fields.
pub struct AcceptedTaskLaunchReceipt<'a> {
    journal: &'a TaskJournalRow,
    need: &'a RuntimeNeedCell,
}

impl RuntimeNeedHandle {
    pub fn try_reusable_join(
        active_generation: GenerationId,
        spec: TaskSpec,
        authority: &TaskValidationAuthority<'_>,
    ) -> Result<Self, RuntimeNeedHandleConstructionError> {
        unimplemented!()
    }

    pub fn try_from_accepted_launch(
        accepted: AcceptedTaskLaunchReceipt<'_>,
    ) -> Result<Self, RuntimeNeedHandleConstructionError> {
        unimplemented!()
    }

    pub fn validate_use(&self, active_generation: GenerationId) -> Result<(), RuntimeNeedUseError> {
        unimplemented!()
    }

    pub const fn correlation(&self) -> &TaskCorrelation {
        &self.correlation
    }

    pub const fn producer(&self) -> &NeedProducerSpec {
        &self.producer
    }

    pub const fn outcome(&self) -> &TaskOutcomeContract {
        &self.outcome
    }

    pub const fn state(&self) -> &RuntimeNeedHandleState {
        &self.state
    }

    pub const fn need_id(&self) -> NeedId {
        unimplemented!()
    }
}

/// Manual PartialEq/Eq/Hash/PartialOrd/Ord use `correlation.need_id()` only.

#[derive(Clone, Debug)]
pub struct RuntimeGenerationJournal {
    generation: GenerationId,
    revision: u64,
    next_always_start_ordinals: BTreeMap<NeedProducerInstanceKey, NonZeroU64>,
    next_observer_id: NonZeroU64,
    task_rows: BTreeMap<TaskId, TaskJournalRow>,
    need_rows: BTreeMap<NeedId, RuntimeNeedCell>,
    observer_rows: BTreeMap<TaskObserverId, TaskObserver>,
    scope_rows: BTreeMap<CancelScopeId, RuntimeTaskScope>,
}

impl RuntimeGenerationJournal {
    pub fn new(generation: GenerationId) -> Self {
        unimplemented!()
    }

    pub const fn generation(&self) -> GenerationId {
        self.generation
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn task(&self, id: &TaskId) -> Option<&TaskJournalRow> {
        self.task_rows.get(id)
    }

    pub fn need(&self, id: &NeedId) -> Option<&RuntimeNeedCell> {
        self.need_rows.get(id)
    }

    pub fn observer(&self, id: &TaskObserverId) -> Option<&TaskObserver> {
        self.observer_rows.get(id)
    }

    pub fn scope(&self, id: &CancelScopeId) -> Option<&RuntimeTaskScope> {
        self.scope_rows.get(id)
    }

    pub fn next_always_start_ordinal(
        &self,
        producer: &NeedProducerInstanceKey,
    ) -> Result<TaskLaunchOrdinal, TaskOrdinalAllocationError> {
        unimplemented!()
    }

    pub fn accepted_launch_receipt(
        &self,
        correlation: &TaskCorrelation,
    ) -> Result<AcceptedTaskLaunchReceipt<'_>, RuntimeNeedHandleConstructionError> {
        unimplemented!()
    }

    pub fn plan_observer_ids(
        &self,
        count: usize,
    ) -> Result<PlannedObserverIds, TaskObserverAllocationError> {
        unimplemented!()
    }

    pub fn begin_transaction<'j, 'a>(
        &'j self,
        authority: &'a TaskValidationAuthority<'a>,
    ) -> Result<JournalTransaction<'j, 'a>, JournalTransactionError> {
        unimplemented!()
    }

    /// Checks generation and base revision before mutation, then swaps the
    /// complete core-owned after-image without a fallible intermediate step.
    pub fn apply_after_image(
        &mut self,
        after: SealedJournalAfterImage,
    ) -> Result<AppliedJournalBatch, JournalApplyError> {
        unimplemented!()
    }
}

/// Core-owned staging transaction. Scheduler code can request changes and
/// inspect typed work, but cannot construct journal rows or committed proofs.
pub struct JournalTransaction<'j, 'a> {
    base: &'j RuntimeGenerationJournal,
    authority: &'a TaskValidationAuthority<'a>,
    after: RuntimeGenerationJournalAfterImage,
}

impl<'j, 'a> JournalTransaction<'j, 'a> {
    pub fn ensure_task(
        &mut self,
        source_index: u32,
        spec: TaskSpec,
        observer: Option<TaskObserverKind>,
    ) -> Result<JournalEnsureResult, JournalTransactionError> {
        unimplemented!()
    }

    pub fn plan_restore(&mut self) -> Result<(), JournalTransactionError> {
        unimplemented!()
    }

    pub fn plan_rebind(
        &mut self,
        new_authority: &TaskValidationAuthority<'_>,
    ) -> Result<(), JournalTransactionError> {
        unimplemented!()
    }

    pub fn plan_cancel(
        &mut self,
        correlations: Box<[TaskCorrelation]>,
        reason: TaskCancelReason,
    ) -> Result<Box<[JournalCancelDisposition]>, JournalTransactionError> {
        unimplemented!()
    }

    pub fn ensure_results(&self) -> &[JournalEnsureResult] {
        unimplemented!()
    }

    pub const fn host_launch_batch(&self) -> Option<&HostTaskLaunchBatch> {
        unimplemented!()
    }
    pub const fn host_restore_batch(&self) -> Option<&HostTaskRestoreBatch> {
        unimplemented!()
    }
    pub const fn host_rebind_batch(&self) -> Option<&HostTaskRebindBatch> {
        unimplemented!()
    }
    pub const fn host_cancel_batch(&self) -> Option<&HostTaskCancelBatch> {
        unimplemented!()
    }

    pub fn accept_launch_receipt(
        &mut self,
        receipt: &HostTaskLaunchReceipt,
    ) -> Result<(), JournalTransactionError> {
        unimplemented!()
    }
    pub fn accept_restore_receipt(
        &mut self,
        receipt: &HostTaskRestoreReceipt,
    ) -> Result<(), JournalTransactionError> {
        unimplemented!()
    }
    pub fn accept_rebind_receipt(
        &mut self,
        receipt: &HostTaskRebindReceipt,
    ) -> Result<(), JournalTransactionError> {
        unimplemented!()
    }

    pub fn seal(self) -> Result<SealedJournalAfterImage, JournalTransactionError> {
        unimplemented!()
    }
}

struct RuntimeGenerationJournalAfterImage {
    generation: GenerationId,
    base_revision: u64,
    next_revision: u64,
    next_always_start_ordinals: BTreeMap<NeedProducerInstanceKey, NonZeroU64>,
    next_observer_id: NonZeroU64,
    task_rows: BTreeMap<TaskId, TaskJournalRow>,
    need_rows: BTreeMap<NeedId, RuntimeNeedCell>,
    observer_rows: BTreeMap<TaskObserverId, TaskObserver>,
    scope_rows: BTreeMap<CancelScopeId, RuntimeTaskScope>,
    ensure_results: Box<[JournalEnsureResult]>,
    host_launch: Option<HostTaskLaunchBatch>,
    host_restore: Option<HostTaskRestoreBatch>,
    host_rebind: Option<HostTaskRebindBatch>,
    host_cancel: Option<HostTaskCancelBatch>,
}

/// Only `JournalTransaction::seal` constructs this private-field apply proof.
pub struct SealedJournalAfterImage {
    after: RuntimeGenerationJournalAfterImage,
}

/// Returned only after the journal swap. It contains keys, never row authority.
pub struct AppliedJournalBatch {
    generation: GenerationId,
    revision: u64,
    ensure_results: Box<[AppliedEnsureResult]>,
}

impl AppliedJournalBatch {
    pub const fn generation(&self) -> GenerationId {
        self.generation
    }
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    pub fn ensure_results(&self) -> &[AppliedEnsureResult] {
        &self.ensure_results
    }

    pub fn into_results(self) -> Box<[AppliedEnsureResult]> {
        self.ensure_results
    }
}

#[derive(Clone, Debug)]
pub struct JournalEnsureResult {
    source_index: u32,
    correlation: TaskCorrelation,
    disposition: JournalEnsureDisposition,
    observer: Option<TaskObserverId>,
}

impl JournalEnsureResult {
    pub const fn source_index(&self) -> u32 {
        self.source_index
    }
    pub const fn correlation(&self) -> &TaskCorrelation {
        &self.correlation
    }
    pub const fn disposition(&self) -> JournalEnsureDisposition {
        self.disposition
    }
    pub const fn observer(&self) -> Option<TaskObserverId> {
        self.observer
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalEnsureDisposition {
    Existing,
    PlannedHostLaunch,
    PlannedRuntimeLaunch,
}

/// Core constructs the live handle while applying the sealed after-image.
/// Scheduler code can only read or return it after its own infallible swap and
/// adapter commit have completed.
pub struct AppliedEnsureResult {
    source_index: u32,
    handle: RuntimeNeedHandle,
    observer: Option<TaskObserverId>,
}

impl AppliedEnsureResult {
    pub const fn source_index(&self) -> u32 {
        self.source_index
    }
    pub const fn handle(&self) -> &RuntimeNeedHandle {
        &self.handle
    }
    pub const fn observer(&self) -> Option<TaskObserverId> {
        self.observer
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalCancelDisposition {
    Planned,
    Absent,
    Terminal,
    AlreadyRequested,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JournalTransactionError {
    GenerationMismatch,
    MixedMode,
    DuplicateSourceIndex,
    DuplicateCorrelation,
    InvalidTaskSpec(TaskSpecValidationError),
    Identity(TaskIdentityError),
    Ordinal(TaskOrdinalAllocationError),
    Observer(TaskObserverAllocationError),
    MissingTask,
    MissingNeed,
    Receipt(HostTaskReceiptError),
    MissingAdapterReceipt,
    UnexpectedAdapterReceipt,
    RevisionOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalApplyError {
    GenerationMismatch,
    StaleRevision,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskObserverId(NonZeroU64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskObserverKey {
    generation: GenerationId,
    id: TaskObserverId,
}

#[derive(Clone, Debug)]
pub struct TaskJournalRow {
    correlation: TaskCorrelation,
    spec: TaskSpec,
    lifecycle: TaskLifecycle,
    host: Option<AcceptedHostLaunch>,
    last_publication: Option<TaskPublicationCursor>,
}

impl TaskJournalRow {
    pub const fn correlation(&self) -> &TaskCorrelation {
        &self.correlation
    }
    pub const fn spec(&self) -> &TaskSpec {
        &self.spec
    }
    pub const fn lifecycle(&self) -> TaskLifecycle {
        self.lifecycle
    }
    pub const fn host(&self) -> Option<&AcceptedHostLaunch> {
        self.host.as_ref()
    }
    pub const fn last_publication(&self) -> Option<&TaskPublicationCursor> {
        self.last_publication.as_ref()
    }
}

#[derive(Clone, Debug)]
pub struct AcceptedHostLaunch {
    operation: HostOperationIdentity,
    launch: HostLaunchCapability,
    cancellation: HostCancellationCapability,
    restart: HostRestartPolicy,
}

impl AcceptedHostLaunch {
    pub const fn operation(&self) -> &HostOperationIdentity {
        &self.operation
    }
    pub const fn launch(&self) -> HostLaunchCapability {
        self.launch
    }
    pub const fn cancellation(&self) -> HostCancellationCapability {
        self.cancellation
    }
    pub const fn restart(&self) -> HostRestartPolicy {
        self.restart
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeNeedCell {
    correlation: TaskCorrelation,
    producer: NeedProducerSpec,
    outcome: TaskOutcomeContract,
    state: RuntimeNeedCellState,
    observers: BTreeSet<TaskObserverId>,
}

impl RuntimeNeedCell {
    pub const fn correlation(&self) -> &TaskCorrelation {
        &self.correlation
    }
    pub const fn producer(&self) -> &NeedProducerSpec {
        &self.producer
    }
    pub const fn outcome(&self) -> &TaskOutcomeContract {
        &self.outcome
    }
    pub const fn state(&self) -> &RuntimeNeedCellState {
        &self.state
    }
    pub fn observers(&self) -> impl ExactSizeIterator<Item = TaskObserverId> + '_ {
        self.observers.iter().copied()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeNeedCellState {
    Pending,
    Ready {
        cursor: TaskPublicationCursor,
        value: RuntimeValue,
    },
    InfrastructureFailed {
        cursor: TaskPublicationCursor,
        failure: RuntimeTaskFailure,
    },
    CancellationRequested,
    Cancelled {
        cursor: TaskPublicationCursor,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskLifecycle {
    Accepted,
    Running,
    Ready,
    InfrastructureFailed,
    CancellationRequested,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTaskFailure {
    kind: RuntimeTaskFailureKind,
    code: u32,
    detail_digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeTaskFailureKind {
    AdapterUnavailable,
    WorkerFailure,
    ProtocolViolation,
    RestoreFailure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskObserver {
    id: TaskObserverId,
    need: NeedId,
    kind: TaskObserverKind,
    state: TaskObserverState,
    last_cursor: Option<TaskPublicationCursor>,
}

impl TaskObserver {
    pub const fn id(&self) -> TaskObserverId {
        self.id
    }
    pub const fn need(&self) -> NeedId {
        self.need
    }
    pub const fn kind(&self) -> TaskObserverKind {
        self.kind
    }
    pub const fn state(&self) -> TaskObserverState {
        self.state
    }
    pub const fn last_cursor(&self) -> Option<&TaskPublicationCursor> {
        self.last_cursor.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskObserverKind {
    DirectAwait,
    AwaitManyChild,
    TimeoutSource,
    ViewSubscription,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskObserverState {
    Active,
    Detached,
    Terminal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTaskScope {
    id: CancelScopeId,
    tasks: BTreeSet<TaskId>,
    cancellation_requested: bool,
}

impl RuntimeTaskScope {
    pub const fn id(&self) -> &CancelScopeId {
        &self.id
    }
    pub fn tasks(&self) -> impl ExactSizeIterator<Item = TaskId> + '_ {
        self.tasks.iter().copied()
    }
    pub const fn cancellation_requested(&self) -> bool {
        self.cancellation_requested
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostRouteId(NonZeroU32);

impl HostRouteId {
    pub const fn new(id: NonZeroU32) -> Self {
        Self(id)
    }

    pub const fn get(self) -> NonZeroU32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostLaunchCapability {
    route: HostRouteId,
    id: NonZeroU64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostCancellationCapability {
    route: HostRouteId,
    id: NonZeroU64,
}

impl HostLaunchCapability {
    pub const fn new(route: HostRouteId, id: NonZeroU64) -> Self {
        Self { route, id }
    }

    pub const fn route(self) -> HostRouteId {
        self.route
    }
    pub const fn id(self) -> NonZeroU64 {
        self.id
    }
}

impl HostCancellationCapability {
    pub const fn new(route: HostRouteId, id: NonZeroU64) -> Self {
        Self { route, id }
    }

    pub const fn route(self) -> HostRouteId {
        self.route
    }
    pub const fn id(self) -> NonZeroU64 {
        self.id
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskLaunchBatch {
    generation: GenerationId,
    rows: Box<[HostTaskLaunchRow]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskLaunchRow {
    source_index: u32,
    correlation: TaskCorrelation,
    operation: HostOperationIdentity,
    request: HostTaskRequest,
    outcome: TaskOutcomeContract,
    restart: HostRestartPolicy,
}

impl HostTaskLaunchBatch {
    pub fn try_new(
        generation: GenerationId,
        rows: Box<[HostTaskLaunchRow]>,
    ) -> Result<Self, HostTaskBatchError> {
        unimplemented!()
    }

    pub const fn generation(&self) -> GenerationId {
        self.generation
    }
    pub fn rows(&self) -> &[HostTaskLaunchRow] {
        &self.rows
    }
}

impl HostTaskLaunchRow {
    pub fn new(
        source_index: u32,
        correlation: TaskCorrelation,
        operation: HostOperationIdentity,
        request: HostTaskRequest,
        outcome: TaskOutcomeContract,
        restart: HostRestartPolicy,
    ) -> Self {
        Self {
            source_index,
            correlation,
            operation,
            request,
            outcome,
            restart,
        }
    }

    pub const fn source_index(&self) -> u32 {
        self.source_index
    }
    pub const fn correlation(&self) -> &TaskCorrelation {
        &self.correlation
    }
    pub const fn operation(&self) -> &HostOperationIdentity {
        &self.operation
    }
    pub const fn request(&self) -> &HostTaskRequest {
        &self.request
    }
    pub const fn outcome(&self) -> &TaskOutcomeContract {
        &self.outcome
    }
    pub const fn restart(&self) -> HostRestartPolicy {
        self.restart
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostTaskLaunchReceipt {
    generation: GenerationId,
    rows: Box<[HostTaskLaunchReceiptRow]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostTaskLaunchReceiptRow {
    source_index: u32,
    correlation: TaskCorrelation,
    operation: HostOperationIdentity,
    launch: HostLaunchCapability,
    cancellation: HostCancellationCapability,
}

impl HostTaskLaunchReceiptRow {
    pub fn new(
        source_index: u32,
        correlation: TaskCorrelation,
        operation: HostOperationIdentity,
        launch: HostLaunchCapability,
        cancellation: HostCancellationCapability,
    ) -> Self {
        Self {
            source_index,
            correlation,
            operation,
            launch,
            cancellation,
        }
    }

    pub const fn source_index(&self) -> u32 {
        self.source_index
    }
    pub const fn correlation(&self) -> &TaskCorrelation {
        &self.correlation
    }
    pub const fn operation(&self) -> &HostOperationIdentity {
        &self.operation
    }
    pub const fn launch(&self) -> HostLaunchCapability {
        self.launch
    }
    pub const fn cancellation(&self) -> HostCancellationCapability {
        self.cancellation
    }
}

impl HostTaskLaunchReceipt {
    pub fn try_for_batch(
        batch: &HostTaskLaunchBatch,
        rows: Box<[HostTaskLaunchReceiptRow]>,
    ) -> Result<Self, HostTaskReceiptError> {
        unimplemented!()
    }

    pub const fn generation(&self) -> GenerationId {
        self.generation
    }

    pub fn rows(&self) -> &[HostTaskLaunchReceiptRow] {
        &self.rows
    }
}

/// The token remains opaque because only the adapter can construct its `T`.
pub struct PreparedLaunchBatch<T> {
    receipt: HostTaskLaunchReceipt,
    token: T,
}

impl<T> PreparedLaunchBatch<T> {
    pub fn try_new(
        batch: &HostTaskLaunchBatch,
        receipt_rows: Box<[HostTaskLaunchReceiptRow]>,
        token: T,
    ) -> Result<Self, HostTaskReceiptError> {
        unimplemented!()
    }

    pub const fn receipt(&self) -> &HostTaskLaunchReceipt {
        &self.receipt
    }

    pub fn into_parts(self) -> (HostTaskLaunchReceipt, T) {
        (self.receipt, self.token)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskRestoreBatch {
    generation: GenerationId,
    rows: Box<[HostTaskRestoreRow]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskRestoreRow {
    correlation: TaskCorrelation,
    complete_spec: TaskSpec,
    operation: HostOperationIdentity,
    launch: HostLaunchCapability,
    cancellation: HostCancellationCapability,
}

impl HostTaskRestoreBatch {
    pub fn try_new(
        generation: GenerationId,
        rows: Box<[HostTaskRestoreRow]>,
    ) -> Result<Self, HostTaskBatchError> {
        unimplemented!()
    }

    pub const fn generation(&self) -> GenerationId {
        self.generation
    }
    pub fn rows(&self) -> &[HostTaskRestoreRow] {
        &self.rows
    }
}

impl HostTaskRestoreRow {
    pub fn new(
        correlation: TaskCorrelation,
        complete_spec: TaskSpec,
        operation: HostOperationIdentity,
        launch: HostLaunchCapability,
        cancellation: HostCancellationCapability,
    ) -> Self {
        Self {
            correlation,
            complete_spec,
            operation,
            launch,
            cancellation,
        }
    }

    pub const fn correlation(&self) -> &TaskCorrelation {
        &self.correlation
    }
    pub const fn complete_spec(&self) -> &TaskSpec {
        &self.complete_spec
    }
    pub const fn operation(&self) -> &HostOperationIdentity {
        &self.operation
    }
    pub const fn launch(&self) -> HostLaunchCapability {
        self.launch
    }
    pub const fn cancellation(&self) -> HostCancellationCapability {
        self.cancellation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostTaskRestoreReceipt {
    generation: GenerationId,
    rows: Box<[HostTaskRestoreReceiptRow]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostTaskRestoreReceiptRow {
    correlation: TaskCorrelation,
    operation: HostOperationIdentity,
    launch: HostLaunchCapability,
    cancellation: HostCancellationCapability,
}

impl HostTaskRestoreReceiptRow {
    pub fn new(
        correlation: TaskCorrelation,
        operation: HostOperationIdentity,
        launch: HostLaunchCapability,
        cancellation: HostCancellationCapability,
    ) -> Self {
        Self {
            correlation,
            operation,
            launch,
            cancellation,
        }
    }

    pub const fn correlation(&self) -> &TaskCorrelation {
        &self.correlation
    }
    pub const fn operation(&self) -> &HostOperationIdentity {
        &self.operation
    }
    pub const fn launch(&self) -> HostLaunchCapability {
        self.launch
    }
    pub const fn cancellation(&self) -> HostCancellationCapability {
        self.cancellation
    }
}

pub struct PreparedRestoreBatch<T> {
    receipt: HostTaskRestoreReceipt,
    token: T,
}

impl HostTaskRestoreReceipt {
    pub fn try_for_batch(
        batch: &HostTaskRestoreBatch,
        rows: Box<[HostTaskRestoreReceiptRow]>,
    ) -> Result<Self, HostTaskReceiptError> {
        unimplemented!()
    }

    pub const fn generation(&self) -> GenerationId {
        self.generation
    }

    pub fn rows(&self) -> &[HostTaskRestoreReceiptRow] {
        &self.rows
    }
}

impl<T> PreparedRestoreBatch<T> {
    pub fn try_new(
        batch: &HostTaskRestoreBatch,
        receipt_rows: Box<[HostTaskRestoreReceiptRow]>,
        token: T,
    ) -> Result<Self, HostTaskReceiptError> {
        unimplemented!()
    }

    pub const fn receipt(&self) -> &HostTaskRestoreReceipt {
        &self.receipt
    }

    pub fn into_parts(self) -> (HostTaskRestoreReceipt, T) {
        (self.receipt, self.token)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskRebindBatch {
    old_generation: GenerationId,
    new_generation: GenerationId,
    rows: Box<[HostTaskRebindRow]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskRebindRow {
    old_correlation: TaskCorrelation,
    new_correlation: TaskCorrelation,
    operation: HostOperationIdentity,
    launch: HostLaunchCapability,
    cancellation: HostCancellationCapability,
}

impl HostTaskRebindBatch {
    pub fn try_new(
        old_generation: GenerationId,
        new_generation: GenerationId,
        rows: Box<[HostTaskRebindRow]>,
    ) -> Result<Self, HostTaskBatchError> {
        unimplemented!()
    }

    pub const fn old_generation(&self) -> GenerationId {
        self.old_generation
    }
    pub const fn new_generation(&self) -> GenerationId {
        self.new_generation
    }
    pub fn rows(&self) -> &[HostTaskRebindRow] {
        &self.rows
    }
}

impl HostTaskRebindRow {
    pub fn new(
        old_correlation: TaskCorrelation,
        new_correlation: TaskCorrelation,
        operation: HostOperationIdentity,
        launch: HostLaunchCapability,
        cancellation: HostCancellationCapability,
    ) -> Self {
        Self {
            old_correlation,
            new_correlation,
            operation,
            launch,
            cancellation,
        }
    }

    pub const fn old_correlation(&self) -> &TaskCorrelation {
        &self.old_correlation
    }
    pub const fn new_correlation(&self) -> &TaskCorrelation {
        &self.new_correlation
    }
    pub const fn operation(&self) -> &HostOperationIdentity {
        &self.operation
    }
    pub const fn launch(&self) -> HostLaunchCapability {
        self.launch
    }
    pub const fn cancellation(&self) -> HostCancellationCapability {
        self.cancellation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostTaskRebindReceipt {
    old_generation: GenerationId,
    new_generation: GenerationId,
    rows: Box<[HostTaskRebindReceiptRow]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostTaskRebindReceiptRow {
    old_correlation: TaskCorrelation,
    new_correlation: TaskCorrelation,
    operation: HostOperationIdentity,
    launch: HostLaunchCapability,
    cancellation: HostCancellationCapability,
}

impl HostTaskRebindReceiptRow {
    pub fn new(
        old_correlation: TaskCorrelation,
        new_correlation: TaskCorrelation,
        operation: HostOperationIdentity,
        launch: HostLaunchCapability,
        cancellation: HostCancellationCapability,
    ) -> Self {
        Self {
            old_correlation,
            new_correlation,
            operation,
            launch,
            cancellation,
        }
    }

    pub const fn old_correlation(&self) -> &TaskCorrelation {
        &self.old_correlation
    }
    pub const fn new_correlation(&self) -> &TaskCorrelation {
        &self.new_correlation
    }
    pub const fn operation(&self) -> &HostOperationIdentity {
        &self.operation
    }
    pub const fn launch(&self) -> HostLaunchCapability {
        self.launch
    }
    pub const fn cancellation(&self) -> HostCancellationCapability {
        self.cancellation
    }
}

pub struct PreparedRebindBatch<T> {
    receipt: HostTaskRebindReceipt,
    token: T,
}

impl HostTaskRebindReceipt {
    pub fn try_for_batch(
        batch: &HostTaskRebindBatch,
        rows: Box<[HostTaskRebindReceiptRow]>,
    ) -> Result<Self, HostTaskReceiptError> {
        unimplemented!()
    }

    pub const fn old_generation(&self) -> GenerationId {
        self.old_generation
    }

    pub const fn new_generation(&self) -> GenerationId {
        self.new_generation
    }

    pub fn rows(&self) -> &[HostTaskRebindReceiptRow] {
        &self.rows
    }
}

impl<T> PreparedRebindBatch<T> {
    pub fn try_new(
        batch: &HostTaskRebindBatch,
        receipt_rows: Box<[HostTaskRebindReceiptRow]>,
        token: T,
    ) -> Result<Self, HostTaskReceiptError> {
        unimplemented!()
    }

    pub const fn receipt(&self) -> &HostTaskRebindReceipt {
        &self.receipt
    }

    pub fn into_parts(self) -> (HostTaskRebindReceipt, T) {
        (self.receipt, self.token)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskCancelBatch {
    generation: GenerationId,
    rows: Box<[HostTaskCancelRow]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskCancelRow {
    command: HostCancelCommandId,
    correlation: TaskCorrelation,
    operation: HostOperationIdentity,
    launch: HostLaunchCapability,
    cancellation: HostCancellationCapability,
    reason: TaskCancelReason,
}

impl HostTaskCancelBatch {
    pub fn try_new(
        generation: GenerationId,
        rows: Box<[HostTaskCancelRow]>,
    ) -> Result<Self, HostTaskBatchError> {
        unimplemented!()
    }

    pub const fn generation(&self) -> GenerationId {
        self.generation
    }
    pub fn rows(&self) -> &[HostTaskCancelRow] {
        &self.rows
    }
}

impl HostTaskCancelRow {
    pub fn new(
        command: HostCancelCommandId,
        correlation: TaskCorrelation,
        operation: HostOperationIdentity,
        launch: HostLaunchCapability,
        cancellation: HostCancellationCapability,
        reason: TaskCancelReason,
    ) -> Self {
        Self {
            command,
            correlation,
            operation,
            launch,
            cancellation,
            reason,
        }
    }

    pub const fn command(&self) -> HostCancelCommandId {
        self.command
    }
    pub const fn correlation(&self) -> &TaskCorrelation {
        &self.correlation
    }
    pub const fn operation(&self) -> &HostOperationIdentity {
        &self.operation
    }
    pub const fn launch(&self) -> HostLaunchCapability {
        self.launch
    }
    pub const fn cancellation(&self) -> HostCancellationCapability {
        self.cancellation
    }
    pub const fn reason(&self) -> TaskCancelReason {
        self.reason
    }
}

pub struct PreparedCancelBatch<T> {
    batch: HostTaskCancelBatch,
    token: T,
}

impl<T> PreparedCancelBatch<T> {
    pub fn new(batch: HostTaskCancelBatch, token: T) -> Self {
        Self { batch, token }
    }

    pub const fn batch(&self) -> &HostTaskCancelBatch {
        &self.batch
    }

    pub fn into_parts(self) -> (HostTaskCancelBatch, T) {
        (self.batch, self.token)
    }
}

pub trait TaskLaunchAdapter {
    type PreparedLaunchToken;
    type PrepareLaunchError;
    type PreparedRestoreToken;
    type PrepareRestoreError;
    type PreparedRebindToken;
    type PrepareRebindError;
    type PreparedCancelToken;
    type PrepareCancelError;

    fn prepare_launch(
        &mut self,
        batch: HostTaskLaunchBatch,
    ) -> Result<PreparedLaunchBatch<Self::PreparedLaunchToken>, Self::PrepareLaunchError>;

    fn commit_launch(&mut self, prepared: PreparedLaunchBatch<Self::PreparedLaunchToken>);
    fn rollback_launch(&mut self, prepared: PreparedLaunchBatch<Self::PreparedLaunchToken>);

    fn prepare_restore(
        &mut self,
        batch: HostTaskRestoreBatch,
    ) -> Result<PreparedRestoreBatch<Self::PreparedRestoreToken>, Self::PrepareRestoreError>;

    fn commit_restore(&mut self, prepared: PreparedRestoreBatch<Self::PreparedRestoreToken>);
    fn rollback_restore(&mut self, prepared: PreparedRestoreBatch<Self::PreparedRestoreToken>);

    fn prepare_rebind(
        &mut self,
        batch: HostTaskRebindBatch,
    ) -> Result<PreparedRebindBatch<Self::PreparedRebindToken>, Self::PrepareRebindError>;

    fn commit_rebind(&mut self, prepared: PreparedRebindBatch<Self::PreparedRebindToken>);
    fn rollback_rebind(&mut self, prepared: PreparedRebindBatch<Self::PreparedRebindToken>);

    fn prepare_cancel(
        &mut self,
        batch: HostTaskCancelBatch,
    ) -> Result<PreparedCancelBatch<Self::PreparedCancelToken>, Self::PrepareCancelError>;

    fn commit_cancel(&mut self, prepared: PreparedCancelBatch<Self::PreparedCancelToken>);
    fn rollback_cancel(&mut self, prepared: PreparedCancelBatch<Self::PreparedCancelToken>);
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostOperationCatalog {
    digest: HostOperationCatalogDigest,
    rows: Box<[HostOperationCatalogRow]>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostOperationCatalogDigest([u8; 32]);

impl HostOperationCatalogDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostOperationId(NonZeroU32);

impl HostOperationId {
    pub const fn new(id: NonZeroU32) -> Self {
        Self(id)
    }

    pub const fn get(self) -> NonZeroU32 {
        self.0
    }
}

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

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostOperationIdentity {
    Builtin(BuiltinHostOperationId),
    Catalog {
        catalog: HostOperationCatalogDigest,
        operation: HostOperationId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostTaskRequestContract {
    kind: HostTaskRequestKind,
    positional: Box<[RuntimeCheckedTypeProjectionV1]>,
    named: Box<[HostNamedArgumentContract]>,
    spread: HostSpreadContract,
}

impl HostTaskRequestContract {
    pub fn try_new(
        kind: HostTaskRequestKind,
        positional: Box<[RuntimeCheckedTypeProjectionV1]>,
        named: Box<[HostNamedArgumentContract]>,
        spread: HostSpreadContract,
    ) -> Result<Self, HostOperationCatalogError> {
        unimplemented!()
    }

    pub const fn kind(&self) -> HostTaskRequestKind {
        self.kind
    }
    pub fn positional(&self) -> &[RuntimeCheckedTypeProjectionV1] {
        &self.positional
    }
    pub fn named(&self) -> &[HostNamedArgumentContract] {
        &self.named
    }
    pub const fn spread(&self) -> HostSpreadContract {
        self.spread
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostNamedArgumentContract {
    name: String,
    ty: RuntimeCheckedTypeProjectionV1,
    required: bool,
}

impl HostNamedArgumentContract {
    pub fn new(name: String, ty: RuntimeCheckedTypeProjectionV1, required: bool) -> Self {
        Self { name, ty, required }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub const fn ty(&self) -> &RuntimeCheckedTypeProjectionV1 {
        &self.ty
    }
    pub const fn required(&self) -> bool {
        self.required
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostSpreadContract {
    Forbidden,
    PositionalTail,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostRestartPolicy {
    MustBeQuiescent,
    Restartable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostCancellationContract {
    RequiredIdempotent,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostCancelCommandId([u8; 32]);

impl HostCancelCommandId {
    pub fn derive(correlation: &TaskCorrelation) -> Result<Self, TaskIdentityError> {
        unimplemented!()
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
    pub fn try_new(
        identity: HostOperationIdentity,
        capability: HostCapabilityId,
        request: HostTaskRequestContract,
        route: HostRouteId,
        restart: HostRestartPolicy,
        cancellation: HostCancellationContract,
    ) -> Result<Self, HostOperationCatalogError> {
        unimplemented!()
    }

    pub const fn identity(&self) -> &HostOperationIdentity {
        &self.identity
    }
    pub const fn capability(&self) -> &HostCapabilityId {
        &self.capability
    }
    pub const fn request(&self) -> &HostTaskRequestContract {
        &self.request
    }
    pub const fn route(&self) -> HostRouteId {
        self.route
    }
    pub const fn restart(&self) -> HostRestartPolicy {
        self.restart
    }
    pub const fn cancellation(&self) -> HostCancellationContract {
        self.cancellation
    }
}

impl HostOperationCatalog {
    pub fn try_new(
        rows: Box<[HostOperationCatalogRow]>,
    ) -> Result<Self, HostOperationCatalogError> {
        unimplemented!()
    }

    pub const fn digest(&self) -> HostOperationCatalogDigest {
        self.digest
    }

    pub fn rows(&self) -> &[HostOperationCatalogRow] {
        &self.rows
    }

    pub fn resolve(
        &self,
        operation: &HostOperationIdentity,
    ) -> Result<&HostOperationCatalogRow, HostOperationCatalogError> {
        unimplemented!()
    }

    pub fn validate_launch_receipt(
        &self,
        input: &HostTaskLaunchBatch,
        receipt: &HostTaskLaunchReceipt,
        journal: &RuntimeGenerationJournal,
    ) -> Result<(), HostTaskReceiptError> {
        unimplemented!()
    }
}

/// Core protocol implemented by the actual accepted upper View product owner.
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

/// Sole borrowed TaskSpec admission context. It copies no catalog rows.
pub struct TaskValidationAuthority<'a> {
    generation: GenerationId,
    structured: &'a RuntimePlan,
    awbc: &'a AwbcProgram,
    awbc_identity: AwbcDigest,
    host_operations: &'a HostOperationCatalog,
    view: &'a dyn ViewTaskPlanAuthority,
}

impl<'a> TaskValidationAuthority<'a> {
    pub fn try_new(
        generation: GenerationId,
        structured: &'a RuntimePlan,
        awbc: &'a AwbcProgram,
        host_operations: &'a HostOperationCatalog,
        view: &'a dyn ViewTaskPlanAuthority,
        verify_budget: AwbcVerifyBudget,
        verify_context: AwbcVerifyContext<'_>,
    ) -> Result<Self, TaskValidationAuthorityError> {
        unimplemented!()
    }

    pub const fn generation(&self) -> GenerationId {
        self.generation
    }
}

impl TaskSpec {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        producer: NeedProducerSpec,
        class: TaskClass,
        priority: TaskPriority,
        cancel_scope: CancelScopeId,
        policy: TaskPolicy,
        outcome: TaskOutcomeContract,
        execution: TaskExecution,
        debug: TaskDebugMetadata,
        authority: &TaskValidationAuthority<'_>,
    ) -> Result<Self, TaskSpecValidationError> {
        let spec = Self {
            producer,
            class,
            priority,
            cancel_scope,
            policy,
            outcome,
            execution,
            debug,
        };
        spec.validate(authority)?;
        Ok(spec)
    }

    pub fn validate(
        &self,
        authority: &TaskValidationAuthority<'_>,
    ) -> Result<(), TaskSpecValidationError> {
        unimplemented!()
    }

    pub const fn producer(&self) -> &NeedProducerSpec {
        &self.producer
    }
    pub const fn class(&self) -> TaskClass {
        self.class
    }
    pub const fn priority(&self) -> TaskPriority {
        self.priority
    }
    pub const fn cancel_scope(&self) -> &CancelScopeId {
        &self.cancel_scope
    }
    pub const fn policy(&self) -> TaskPolicy {
        self.policy
    }
    pub const fn outcome(&self) -> &TaskOutcomeContract {
        &self.outcome
    }
    pub const fn execution(&self) -> &TaskExecution {
        &self.execution
    }
    pub const fn debug(&self) -> &TaskDebugMetadata {
        &self.debug
    }
}

// Existing RuntimePlan owner gains one table; it is not a parallel catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTaskPlan {
    semantic_digest: TaskPlanSemanticDigest,
    producer_contract: NeedProducerContractDigest,
    producer_site: u32,
    payload_type: RuntimeTypeSemanticDigest,
    operation: HostOperationIdentity,
    request_contract: HostTaskRequestContract,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeTaskPlanTable {
    rows: BTreeMap<TaskPlanSemanticDigest, RuntimeTaskPlan>,
}

impl RuntimePlan {
    pub fn task_plan(&self, digest: &TaskPlanSemanticDigest) -> Option<&RuntimeTaskPlan> {
        unimplemented!()
    }
}

impl AwbcProgram {
    pub fn validate_task_plan(
        &self,
        producer: &NeedProducerSpec,
        outcome: &TaskOutcomeContract,
        request: &HostTaskRequest,
    ) -> Result<(), TaskSpecValidationError> {
        unimplemented!()
    }
}

// -------------------------------------------------------------------------
// arcweft_runtime_scheduler — private complete after-image and coordinator
// -------------------------------------------------------------------------

struct SchedulerRuntimeAfterImage {
    tasks_after: BTreeMap<TaskId, RuntimeTask>,
    aggregates_after: BTreeMap<TaskId, RuntimeAwaitManyAggregateTask>,
    timeouts_after: BTreeMap<TaskId, RuntimeTimeoutTask>,
}

impl RuntimeScheduler {
    /// Infallible private swap. The public coordinator invokes it only after
    /// core JournalTransaction sealing and every adapter prepare succeeds.
    fn apply_runtime_after_image(&mut self, after: SchedulerRuntimeAfterImage) {
        unimplemented!()
    }

    /// Exact post-prepare coordinator. The only fallible operation is the
    /// journal base-generation/revision check. That failure rolls adapter
    /// reservations back in reverse order. After a successful journal swap,
    /// scheduler swap and adapter commit are both infallible; only then are
    /// live handles returned to the caller.
    fn apply_ensure_plan<A: TaskLaunchAdapter>(
        &mut self,
        journal: &mut RuntimeGenerationJournal,
        adapter: &mut A,
        plan: EnsureBatchPlan<A::PreparedLaunchToken>,
    ) -> Result<Box<[AppliedEnsureResult]>, JournalApplyError> {
        let EnsureBatchPlan {
            journal: journal_after,
            runtime,
            mut prepared_host,
        } = plan;
        let applied = match journal.apply_after_image(journal_after) {
            Ok(applied) => applied,
            Err(error) => {
                while let Some(prepared) = prepared_host.pop() {
                    adapter.rollback_launch(prepared);
                }
                return Err(error);
            }
        };
        self.apply_runtime_after_image(runtime);
        for prepared in prepared_host {
            adapter.commit_launch(prepared);
        }
        Ok(applied.into_results())
    }

    fn apply_restore_plan<A: TaskLaunchAdapter>(
        &mut self,
        journal: &mut RuntimeGenerationJournal,
        adapter: &mut A,
        plan: RestoreBatchPlan<A::PreparedRestoreToken>,
    ) -> Result<AppliedJournalBatch, JournalApplyError> {
        let RestoreBatchPlan {
            journal: journal_after,
            runtime,
            mut prepared_host,
        } = plan;
        let applied = match journal.apply_after_image(journal_after) {
            Ok(applied) => applied,
            Err(error) => {
                while let Some(prepared) = prepared_host.pop() {
                    adapter.rollback_restore(prepared);
                }
                return Err(error);
            }
        };
        self.apply_runtime_after_image(runtime);
        for prepared in prepared_host {
            adapter.commit_restore(prepared);
        }
        Ok(applied)
    }

    fn apply_rebind_plan<A: TaskLaunchAdapter>(
        &mut self,
        journal: &mut RuntimeGenerationJournal,
        adapter: &mut A,
        plan: RebindBatchPlan<A::PreparedRebindToken>,
    ) -> Result<AppliedJournalBatch, JournalApplyError> {
        let RebindBatchPlan {
            journal: journal_after,
            runtime,
            mut prepared_host,
        } = plan;
        let applied = match journal.apply_after_image(journal_after) {
            Ok(applied) => applied,
            Err(error) => {
                while let Some(prepared) = prepared_host.pop() {
                    adapter.rollback_rebind(prepared);
                }
                return Err(error);
            }
        };
        self.apply_runtime_after_image(runtime);
        for prepared in prepared_host {
            adapter.commit_rebind(prepared);
        }
        Ok(applied)
    }

    fn apply_cancel_plan<A: TaskLaunchAdapter>(
        &mut self,
        journal: &mut RuntimeGenerationJournal,
        adapter: &mut A,
        plan: CancelBatchPlan<A::PreparedCancelToken>,
    ) -> Result<AppliedJournalBatch, JournalApplyError> {
        let CancelBatchPlan {
            journal: journal_after,
            runtime,
            mut prepared_host,
        } = plan;
        let applied = match journal.apply_after_image(journal_after) {
            Ok(applied) => applied,
            Err(error) => {
                while let Some(prepared) = prepared_host.pop() {
                    adapter.rollback_cancel(prepared);
                }
                return Err(error);
            }
        };
        self.apply_runtime_after_image(runtime);
        for prepared in prepared_host {
            adapter.commit_cancel(prepared);
        }
        Ok(applied)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeTask {
    correlation: TaskCorrelation,
    request: RuntimeTaskRequest,
    state: RuntimeTaskState,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeTaskState {
    AwaitManyAggregate(RuntimeAwaitManyAggregateTask),
    Timeout(RuntimeTimeoutTask),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeAwaitManyAggregateTask {
    aggregate: TaskCorrelation,
    request: RuntimeAwaitManyAggregateRequest,
    launch_cursor: u32,
    children: Box<[RuntimeAwaitManyChildState]>,
    outputs: Box<[Option<RuntimeValue>]>,
    terminal: Option<RuntimeAwaitManyTerminal>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeAwaitManyChildState {
    source_index: u32,
    observer: Option<TaskObserverId>,
    handle: Option<RuntimeNeedHandle>,
    status: RuntimeAwaitManyChildStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeAwaitManyChildStatus {
    NotLaunched,
    Waiting,
    Ready,
    InfrastructureFailed(RuntimeTaskFailure),
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeAwaitManyTerminal {
    Ready,
    InfrastructureFailed(RuntimeTaskFailure),
    Cancelled,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeTimeoutTask {
    output: TaskCorrelation,
    request: RuntimeTimeoutRequest,
    remaining: LogicalDuration,
    source_observer: Option<TaskObserverId>,
    terminal: Option<RuntimeTimeoutTerminal>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeTimeoutTerminal {
    SourceReady(RuntimeValue),
    SourceInfrastructureFailed(RuntimeTaskFailure),
    Expired,
    Cancelled,
}

struct EnsureBatchPlan<T> {
    journal: SealedJournalAfterImage,
    runtime: SchedulerRuntimeAfterImage,
    prepared_host: Vec<PreparedLaunchBatch<T>>,
}

struct RestoreBatchPlan<T> {
    journal: SealedJournalAfterImage,
    runtime: SchedulerRuntimeAfterImage,
    prepared_host: Vec<PreparedRestoreBatch<T>>,
}

struct RebindBatchPlan<T> {
    journal: SealedJournalAfterImage,
    runtime: SchedulerRuntimeAfterImage,
    prepared_host: Vec<PreparedRebindBatch<T>>,
}

struct CancelBatchPlan<T> {
    journal: SealedJournalAfterImage,
    runtime: SchedulerRuntimeAfterImage,
    prepared_host: Vec<PreparedCancelBatch<T>>,
}

// -------------------------------------------------------------------------
// AwaitMany — retained corrected request and template
// -------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeAwaitManyAggregateRequest {
    captured: Box<[RuntimeValue]>,
    source_items: Box<[RuntimeValue]>,
    child: Box<NeedProducerTemplate>,
    limit: NonZeroU32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeedProducerTemplate {
    producer: NeedProducerTemplateIdentity,
    class: TaskClass,
    priority: TaskPriority,
    cancel_scope: CancelScopeId,
    policy: TaskPolicy,
    outcome: TaskOutcomeContract,
    execution: TaskExecutionTemplate,
    debug: TaskDebugMetadataTemplate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeedProducerTemplateIdentity {
    family: NeedProducerFamily,
    contract: NeedProducerContractDigest,
    plan: TaskPlanSemanticDigest,
    producer_site: u32,
    payload_type: RuntimeTypeSemanticDigest,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaskExecutionTemplate {
    Host(HostTaskRequestTemplate),
    Runtime(RuntimeTaskRequestTemplate),
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskRequestTemplate {
    operation: HostOperationIdentity,
    arguments: Box<[HostTaskArgumentTemplate]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskArgumentTemplate {
    name: Option<String>,
    spread: bool,
    source: NeedTemplateValueSource,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NeedTemplateValueSource {
    WholeArgument,
    Captured { ordinal: u32 },
    SourceIndex,
    SourceItem,
    Constant(RuntimeValue),
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeTaskRequestTemplate {
    AwaitManyAggregate {
        captured: Box<[NeedTemplateValueSource]>,
        source_items: NeedTemplateValueSource,
        child: Box<NeedProducerTemplate>,
        limit: NonZeroU32,
    },
    Timeout {
        source: NeedTemplateValueSource,
        requested_limit: NeedTemplateValueSource,
        contract: NeedTimeoutContractDigest,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskDebugMetadataTemplate {
    label: Option<String>,
    origin: Option<String>,
}

impl NeedProducerTemplate {
    pub fn instantiate(
        &self,
        argument: &RuntimeValue,
        authority: &TaskValidationAuthority<'_>,
        limits: RuntimeValueCanonicalLimits,
    ) -> Result<TaskSpec, NeedProducerTemplateError> {
        unimplemented!()
    }
}

impl RuntimeAwaitManyAggregateRequest {
    pub fn new(
        captured: Box<[RuntimeValue]>,
        source_items: Box<[RuntimeValue]>,
        child: NeedProducerTemplate,
        limit: NonZeroU32,
    ) -> Self {
        Self {
            captured,
            source_items,
            child: Box::new(child),
            limit,
        }
    }

    pub fn captured(&self) -> &[RuntimeValue] {
        &self.captured
    }
    pub fn source_items(&self) -> &[RuntimeValue] {
        &self.source_items
    }
    pub const fn child(&self) -> &NeedProducerTemplate {
        &self.child
    }
    pub const fn limit(&self) -> NonZeroU32 {
        self.limit
    }

    pub fn child_argument(&self, index: u32) -> Result<RuntimeValue, AwaitManyError> {
        unimplemented!()
    }

    pub fn child_spec(
        &self,
        index: u32,
        authority: &TaskValidationAuthority<'_>,
        limits: RuntimeValueCanonicalLimits,
    ) -> Result<TaskSpec, AwaitManyError> {
        unimplemented!()
    }

    pub fn aggregate_base_argument(&self) -> RuntimeValue {
        unimplemented!()
    }
}

// -------------------------------------------------------------------------
// One outer snapshot authority and in-place RuntimeValue snapshot owner
// -------------------------------------------------------------------------

/// Sole borrowed, nonserialized snapshot admission authority.
pub struct RuntimeSnapshotAuthority<'a> {
    task: TaskValidationAuthority<'a>,
    journal: &'a RuntimeGenerationJournal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeSnapshotLimits {
    pub max_depth: u32,
    pub max_nodes: u64,
    pub max_collection_len: u64,
    pub max_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AwbcRuntimeValueSnapshotError {
    WorkLimit,
    UnknownTag,
    TrailingBytes,
    NonCanonicalLength,
    InvalidFieldIdentity,
    DuplicateField,
    InvalidNominalJoin,
    InvalidOpaqueJoin,
    InvalidVariantJoin,
    InvalidNeedHandle,
    InvalidTaskSpec,
    MissingAcceptedLaunch,
    UnrebindableStructuredFunction,
    MissingAwbcExecutableAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeSnapshotVersion;

impl RuntimeSnapshotVersion {
    pub const ENCODED: u8 = 1;
}

impl<'a> RuntimeSnapshotAuthority<'a> {
    pub fn try_new(
        task: TaskValidationAuthority<'a>,
        journal: &'a RuntimeGenerationJournal,
    ) -> Result<Self, RuntimeSnapshotAuthorityError> {
        unimplemented!()
    }

    pub const fn task_validation(&self) -> &TaskValidationAuthority<'a> {
        &self.task
    }

    pub const fn journal(&self) -> &'a RuntimeGenerationJournal {
        self.journal
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AwbcRuntimeValueSnapshot {
    Unit,
    Bool(bool),
    Int(RuntimeInt),
    UInt(RuntimeUInt),
    F32(f32),
    F64(f64),
    MatrixF32(DenseMatrixF32),
    MatrixF64(DenseMatrixF64),
    TensorF32(DenseTensorF32),
    TensorF64(DenseTensorF64),
    String(String),
    Char(char),
    Duration(LogicalDuration),
    Progress {
        ratio: f32,
        label: Option<String>,
    },
    Range(RuntimeRange),
    Iterator(AwbcRuntimeIteratorSnapshot),
    EntityRef(String),
    Tuple(Vec<Self>),
    Seq(AwbcRuntimeSeqSnapshot),
    Record(Vec<AwbcRuntimeFieldSnapshot>),
    NominalRecord(AwbcRuntimeNominalRecordSnapshot),
    Opaque(AwbcRuntimeOpaqueSnapshot),
    Reduction(AwbcRuntimeReductionSnapshot),
    Agent(AwbcRuntimeAgentSnapshot),
    Function(AwbcRuntimeFunctionSnapshot),
    Variant {
        owner: RuntimeVariantIdentity,
        ordinal: u32,
        name: String,
        payload: Option<Box<Self>>,
    },
    NeedHandle(RuntimeNeedHandleSnapshotV1),
}

impl AwbcRuntimeValueSnapshot {
    pub fn from_runtime_value(
        value: &RuntimeValue,
        authority: &RuntimeSnapshotAuthority<'_>,
        limits: RuntimeSnapshotLimits,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        unimplemented!()
    }

    pub fn into_runtime_value(
        self,
        authority: &RuntimeSnapshotAuthority<'_>,
        limits: RuntimeSnapshotLimits,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        unimplemented!()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AwbcRuntimeSeqSnapshot {
    Values(Vec<AwbcRuntimeValueSnapshot>),
    Dense(DenseSeq), // Reuses current owner: Units(usize), Bool(...).
    TupleColumns {
        len: u64,
        columns: Vec<AwbcRuntimeSeqSnapshot>,
    },
    RecordColumns {
        len: u64,
        fields: Vec<AwbcRuntimeRecordSeqFieldSnapshot>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct AwbcRuntimeFunctionSnapshot {
    pub function: AwbcFunctionId,
    pub remaining_params: Box<[String]>,
    pub captures: Box<[AwbcRuntimeBindingSnapshot]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeNeedHandleSnapshotV1 {
    pub version: RuntimeSnapshotVersion,
    pub correlation: TaskCorrelationSnapshotV1,
    pub producer: NeedProducerSpecSnapshotV1,
    pub outcome: TaskOutcomeContractSnapshotV1,
    pub state: RuntimeNeedHandleStateSnapshotV1,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeNeedHandleStateSnapshotV1 {
    ReusableJoin { spec: Box<TaskSpecSnapshotV1> },
    AcceptedLaunch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeGenerationJournalSnapshotV1 {
    pub version: RuntimeSnapshotVersion,
    pub generation: GenerationId,
    pub ordinal_counters: Box<[AlwaysStartOrdinalCounterSnapshotV1]>,
    pub next_observer_id: NonZeroU64,
    pub tasks: Box<[TaskJournalRowSnapshotV1]>,
    pub needs: Box<[RuntimeNeedCellSnapshotV1]>,
    pub observers: Box<[TaskObserverSnapshotV1]>,
    pub scopes: Box<[RuntimeTaskScopeSnapshotV1]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeSchedulerSnapshotV1 {
    pub version: RuntimeSnapshotVersion,
    pub journal: RuntimeGenerationJournalSnapshotV1,
    pub runtime: RuntimeTaskStateSnapshotV1,
    pub pending_events: Box<[TaskEventSnapshotV1]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeTaskStateSnapshotV1 {
    pub version: RuntimeSnapshotVersion,
    pub tasks: Box<[RuntimeTaskSnapshotV1]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeTaskSnapshotV1 {
    pub correlation: TaskCorrelationSnapshotV1,
    pub state: RuntimeTaskKindSnapshotV1,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeTaskKindSnapshotV1 {
    AwaitManyAggregate(RuntimeAwaitManyAggregateTaskSnapshotV1),
    Timeout(RuntimeTimeoutTaskSnapshotV1),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeAwaitManyAggregateTaskSnapshotV1 {
    pub request: RuntimeAwaitManyAggregateRequestSnapshotV1,
    pub launch_cursor: u32,
    pub children: Box<[RuntimeAwaitManyChildSnapshotV1]>,
    pub outputs: Box<[Option<AwbcRuntimeValueSnapshot>]>,
    pub terminal: Option<RuntimeAwaitManyTerminal>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeAwaitManyChildSnapshotV1 {
    pub source_index: u32,
    pub observer: Option<TaskObserverId>,
    pub handle: Option<RuntimeNeedHandleSnapshotV1>,
    pub status: RuntimeAwaitManyChildStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeTimeoutTaskSnapshotV1 {
    pub request: RuntimeTimeoutRequestSnapshotV1,
    pub remaining: LogicalDuration,
    pub source_observer: Option<TaskObserverId>,
    pub terminal: Option<RuntimeTimeoutTerminalSnapshotV1>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeTimeoutTerminalSnapshotV1 {
    SourceReady(AwbcRuntimeValueSnapshot),
    SourceInfrastructureFailed(RuntimeTaskFailure),
    Expired,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskEventSnapshotV1 {
    pub generation: GenerationId,
    pub logical_epoch: LogicalEpoch,
    pub task_id: TaskId,
    pub sequence: TaskSequence,
    pub kind: TaskEventKindSnapshotV1,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaskEventKindSnapshotV1 {
    Accepted,
    Running,
    Ready(AwbcRuntimeValueSnapshot),
    InfrastructureFailed(RuntimeTaskFailure),
    CancellationRequested,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeedProducerSpecSnapshotV1 {
    pub version: RuntimeSnapshotVersion,
    pub family: NeedProducerFamily,
    pub contract: NeedProducerContractDigest,
    pub plan: TaskPlanSemanticDigest,
    pub producer_site: u32,
    pub payload_type: RuntimeTypeSemanticDigest,
    pub arguments: RuntimeValueDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskCorrelationSnapshotV1 {
    pub version: RuntimeSnapshotVersion,
    pub generation: GenerationId,
    pub producer: NeedProducerInstanceKey,
    pub policy: TaskPolicy,
    pub ordinal: TaskLaunchOrdinal,
    pub need_id: NeedId,
    pub task_key: TaskKey,
    pub task_id: TaskId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskOutcomeContractSnapshotV1 {
    pub version: RuntimeSnapshotVersion,
    pub payload_type: RuntimeTypeSemanticDigest,
    pub checked: RuntimeCheckedTypeProjectionV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskDebugMetadataSnapshotV1 {
    pub version: RuntimeSnapshotVersion,
    pub label: Option<String>,
    pub origin: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskJournalRowSnapshotV1 {
    pub version: RuntimeSnapshotVersion,
    pub correlation: TaskCorrelationSnapshotV1,
    pub spec: TaskSpecSnapshotV1,
    pub lifecycle: TaskLifecycle,
    pub host: Option<AcceptedHostLaunchSnapshotV1>,
    pub last_publication: Option<TaskPublicationCursor>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeNeedCellSnapshotV1 {
    pub version: RuntimeSnapshotVersion,
    pub correlation: TaskCorrelationSnapshotV1,
    pub producer: NeedProducerSpecSnapshotV1,
    pub outcome: TaskOutcomeContractSnapshotV1,
    pub state: RuntimeNeedCellStateSnapshotV1,
    pub observers: Box<[TaskObserverId]>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeNeedCellStateSnapshotV1 {
    Pending,
    Ready {
        cursor: TaskPublicationCursor,
        value: AwbcRuntimeValueSnapshot,
    },
    InfrastructureFailed {
        cursor: TaskPublicationCursor,
        failure: RuntimeTaskFailure,
    },
    CancellationRequested,
    Cancelled {
        cursor: TaskPublicationCursor,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskObserverSnapshotV1 {
    pub version: RuntimeSnapshotVersion,
    pub id: TaskObserverId,
    pub need: NeedId,
    pub kind: TaskObserverKind,
    pub state: TaskObserverState,
    pub last_cursor: Option<TaskPublicationCursor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTaskScopeSnapshotV1 {
    pub version: RuntimeSnapshotVersion,
    pub id: CancelScopeId,
    pub tasks: Box<[TaskId]>,
    pub cancellation_requested: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlwaysStartOrdinalCounterSnapshotV1 {
    pub version: RuntimeSnapshotVersion,
    pub producer: NeedProducerInstanceKey,
    pub next: NonZeroU64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AcceptedHostLaunchSnapshotV1 {
    pub version: RuntimeSnapshotVersion,
    pub operation: HostOperationIdentity,
    pub launch: HostLaunchCapability,
    pub cancellation: HostCancellationCapability,
    pub restart: HostRestartPolicy,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskSpecSnapshotV1 {
    pub version: RuntimeSnapshotVersion,
    pub producer: NeedProducerSpecSnapshotV1,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub policy: TaskPolicy,
    pub outcome: TaskOutcomeContractSnapshotV1,
    pub execution: TaskExecutionSnapshotV1,
    pub debug: TaskDebugMetadataSnapshotV1,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeedProducerTemplateSnapshotV1 {
    pub version: RuntimeSnapshotVersion,
    pub producer: NeedProducerTemplateIdentitySnapshotV1,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub policy: TaskPolicy,
    pub outcome: TaskOutcomeContractSnapshotV1,
    pub execution: TaskExecutionTemplateSnapshotV1,
    pub debug: TaskDebugMetadataTemplateSnapshotV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeedProducerTemplateIdentitySnapshotV1 {
    pub family: NeedProducerFamily,
    pub contract: NeedProducerContractDigest,
    pub plan: TaskPlanSemanticDigest,
    pub producer_site: u32,
    pub payload_type: RuntimeTypeSemanticDigest,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaskExecutionTemplateSnapshotV1 {
    Host(HostTaskRequestTemplateSnapshotV1),
    Runtime(RuntimeTaskRequestTemplateSnapshotV1),
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskRequestTemplateSnapshotV1 {
    pub operation: HostOperationIdentity,
    pub arguments: Box<[HostTaskArgumentTemplateSnapshotV1]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskArgumentTemplateSnapshotV1 {
    pub name: Option<String>,
    pub spread: bool,
    pub source: NeedTemplateValueSourceSnapshotV1,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NeedTemplateValueSourceSnapshotV1 {
    WholeArgument,
    Captured { ordinal: u32 },
    SourceIndex,
    SourceItem,
    Constant(AwbcRuntimeValueSnapshot),
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeTaskRequestTemplateSnapshotV1 {
    AwaitManyAggregate {
        captured: Box<[NeedTemplateValueSourceSnapshotV1]>,
        source_items: NeedTemplateValueSourceSnapshotV1,
        child: Box<NeedProducerTemplateSnapshotV1>,
        limit: NonZeroU32,
    },
    Timeout {
        source: NeedTemplateValueSourceSnapshotV1,
        requested_limit: NeedTemplateValueSourceSnapshotV1,
        contract: NeedTimeoutContractDigest,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskDebugMetadataTemplateSnapshotV1 {
    pub label: Option<String>,
    pub origin: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaskExecutionSnapshotV1 {
    Host(HostTaskRequestSnapshotV1),
    Runtime(RuntimeTaskRequestSnapshotV1),
}

#[derive(Clone, Debug, PartialEq)]
pub enum HostTaskRequestSnapshotV1 {
    FileReadText {
        path: String,
    },
    FileReadBytes {
        path: String,
    },
    FileWriteText {
        path: String,
        text: String,
    },
    FileWriteBytes {
        path: String,
        bytes: Box<[u8]>,
    },
    HttpFetch {
        url: String,
        method: String,
        headers: Box<[(String, String)]>,
        body: Option<Box<AwbcRuntimeValueSnapshot>>,
    },
    HttpRespond {
        request_id: String,
        status: u16,
        headers: Box<[(String, String)]>,
        body: Option<Box<AwbcRuntimeValueSnapshot>>,
    },
    ProcessRun {
        program: String,
        args: Box<[String]>,
        env: Box<[(String, String)]>,
    },
    AssetLoad {
        id: String,
        kind: String,
    },
    ShaderCompile {
        id: String,
        entry: Option<String>,
    },
    AudioDecode {
        id: String,
    },
    TtsSynthesis {
        voice: Option<String>,
        text: String,
    },
    WasmCall {
        module: String,
        function: String,
        args: Box<[AwbcRuntimeValueSnapshot]>,
    },
    SystemInfo {
        kind: SystemInfoKind,
    },
    Custom {
        operation: HostOperationIdentity,
        args: Box<[AwbcRuntimeValueSnapshot]>,
        named_args: Box<[NamedRuntimeValueSnapshotV1]>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NamedRuntimeValueSnapshotV1 {
    pub name: String,
    pub value: AwbcRuntimeValueSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeTaskRequestSnapshotV1 {
    AwaitManyAggregate(RuntimeAwaitManyAggregateRequestSnapshotV1),
    Timeout(RuntimeTimeoutRequestSnapshotV1),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeTimeoutRequestSnapshotV1 {
    pub version: RuntimeSnapshotVersion,
    pub source: RuntimeNeedHandleSnapshotV1,
    pub requested_limit: LogicalDuration,
    pub contract: NeedTimeoutContractDigest,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeAwaitManyAggregateRequestSnapshotV1 {
    pub version: RuntimeSnapshotVersion,
    pub captured: Box<[AwbcRuntimeValueSnapshot]>,
    pub source_items: Box<[AwbcRuntimeValueSnapshot]>,
    pub child: Box<NeedProducerTemplateSnapshotV1>,
    pub limit: NonZeroU32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeSequenceProjectionV1 {
    Vec,
    Array { length: u64 },
    Slice,
    Seq,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeOwnershipTypeProjectionV1 {
    Range,
    IteratorState(IteratorStateKind),
    Map(MapKind),
    Need,
    Stream,
    ThreadHandle,
    Shared,
    Function,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeNominalRecordProjectionV1 {
    pub nominal: RuntimeNominalTypeId,
    pub semantic_identity: RuntimeSemanticTypeId,
    pub layout: TypeLayoutHash,
    pub fields: Box<[RuntimeNominalRecordFieldProjectionV1]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeNominalRecordFieldProjectionV1 {
    pub field: RuntimeRecordFieldId,
    pub ty: RuntimeCheckedTypeProjectionV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeProjectNominalProjectionV1 {
    ExactRecord(RuntimeNominalRecordProjectionV1),
    ExactVariant(RuntimeCheckedTypeProjectionV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeAcceptedNominalProjectionV1 {
    ExactRecord(RuntimeNominalRecordProjectionV1),
    ExactVariant(RuntimeCheckedTypeProjectionV1),
    ExactOpaque(RuntimeOpaqueTypeOwner),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeCheckedTypeProjectionV1 {
    Never,
    Unit,
    Bool,
    Signed(RuntimeSignedIntWidth),
    Unsigned(RuntimeUnsignedIntWidth),
    F32,
    F64,
    String,
    Char,
    Duration,
    Progress,
    EntityReference,
    Bytes,
    Sequence(Box<Self>),
    Tuple(Box<[Self]>),
    Choice(Box<[Self]>),
    Nominal {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
    },
    Opaque {
        owner: RuntimeOpaqueTypeOwner,
    },
    Variant {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        cases: Box<[RuntimeCheckedVariantCaseProjectionV1]>,
    },
    Result {
        ok: Box<Self>,
        error: Box<Self>,
    },
    Option(Box<Self>),
    Agent(RuntimeAgentOperationalType),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCheckedVariantCaseProjectionV1 {
    pub ordinal: u32,
    pub name: String,
    pub payload: Option<Box<RuntimeCheckedTypeProjectionV1>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeTextProjectionV1 {
    TextClusterUtf8 {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
    },
    DisplayTextUtf8 {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
    },
}

// There is deliberately no RuntimeDenseSeqProjectionV1.

// -------------------------------------------------------------------------
// arcweft_lang_hir — HIR-only child edge authority
// -------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirExpressionChildEdge {
    child: ExprId,
    role: HirExpressionChildRole,
}

impl HirExpressionChildEdge {
    pub const fn child(&self) -> ExprId {
        self.child
    }

    pub const fn role(&self) -> &HirExpressionChildRole {
        &self.role
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirNestedExpressionPath(Box<[HirNestedExpressionPathSegment]>);

impl HirNestedExpressionPath {
    pub fn try_from_segments(
        segments: Box<[HirNestedExpressionPathSegment]>,
    ) -> Result<Self, HirNestedExpressionPathError> {
        (!segments.is_empty())
            .then_some(Self(segments))
            .ok_or(HirNestedExpressionPathError::Empty)
    }

    pub fn segments(&self) -> &[HirNestedExpressionPathSegment] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirNestedExpressionPathError {
    Empty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirNestedExpressionPathSegment {
    ChoiceBodyItem { ordinal: u32 },
    ChoiceIfBranch { ordinal: u32 },
    ChoiceIfElse,
    ChoiceForBody,
    ChoiceMatchArm { ordinal: u32 },
    ChoiceOptionBody,
    ChoiceOptionField { ordinal: u32 },
    ChoiceViewEntry { ordinal: u32 },
    ChoicePlanItem { ordinal: u32 },
    LinePlanItem { ordinal: u32 },
    LinePlanStartGroupItem { ordinal: u32 },
    LinePlanTogetherGroupItem { ordinal: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirExpressionChildRole {
    Element {
        ordinal: u32,
    },
    RepeatedValue,
    RepeatLength,
    Callee,
    Argument {
        ordinal: u32,
    },
    Target,
    Index,
    PipeLeft,
    PipeRight,
    Operand,
    RangeStart,
    RangeEnd,
    RecordField {
        source_ordinal: u32,
    },
    BinaryLeft,
    BinaryRight,
    ClosureBody,
    BlockTail,
    LoopTail,
    Condition,
    ThenBranch,
    ElseBranch,
    Scrutinee,
    Guard {
        arm: u32,
    },
    ArmValue {
        arm: u32,
    },
    IfLetGuard,
    DialogueTarget,
    DialogueCoordinate {
        ordinal: u32,
    },
    DialogueInterpolation {
        ordinal: u32,
    },
    DialogueTagPayload {
        ordinal: u32,
    },
    LinePlanOptionValue {
        path: HirNestedExpressionPath,
    },
    LinePlanLetValue {
        path: HirNestedExpressionPath,
    },
    LinePlanOut {
        path: HirNestedExpressionPath,
    },
    LinePlanTimelineAssert {
        path: HirNestedExpressionPath,
    },
    LinePlanExpression {
        path: HirNestedExpressionPath,
    },
    LinePlanTimedCueAnchor {
        path: HirNestedExpressionPath,
    },
    LinePlanTimedCueBody {
        path: HirNestedExpressionPath,
    },
    PostfixIndexCandidate,
    PostfixDialogueCandidate,
    ForInput,
    ChoiceIfCondition {
        path: HirNestedExpressionPath,
        branch: u32,
    },
    ChoiceForSource {
        path: HirNestedExpressionPath,
    },
    ChoiceMatchScrutinee {
        path: HirNestedExpressionPath,
    },
    ChoiceMatchGuard {
        path: HirNestedExpressionPath,
        arm: u32,
    },
    ChoiceOptionId {
        path: HirNestedExpressionPath,
    },
    ChoiceOptionForSource {
        path: HirNestedExpressionPath,
    },
    ChoiceCompactLabel {
        path: HirNestedExpressionPath,
    },
    ChoiceCompactCondition {
        path: HirNestedExpressionPath,
    },
    ChoiceCompactOut {
        path: HirNestedExpressionPath,
    },
    ChoiceOptionLabel {
        path: HirNestedExpressionPath,
        field: u32,
    },
    ChoiceOptionFieldId {
        path: HirNestedExpressionPath,
        field: u32,
    },
    ChoiceOptionValue {
        path: HirNestedExpressionPath,
        field: u32,
    },
    ChoiceOptionVisible {
        path: HirNestedExpressionPath,
        field: u32,
    },
    ChoiceOptionEnabled {
        path: HirNestedExpressionPath,
        field: u32,
    },
    ChoiceOptionOrder {
        path: HirNestedExpressionPath,
        field: u32,
    },
    ChoiceOptionHotkey {
        path: HirNestedExpressionPath,
        field: u32,
    },
    ChoiceOptionViewKey {
        path: HirNestedExpressionPath,
        field: u32,
        entry: u32,
    },
    ChoiceOptionViewValue {
        path: HirNestedExpressionPath,
        field: u32,
        entry: u32,
    },
    ChoicePlanAssignment {
        item: u32,
    },
    ChoicePlanTimeout {
        item: u32,
    },
    ChoicePlanCancelSignal {
        item: u32,
    },
    ChoicePlanCancelTimeout {
        item: u32,
    },
    ChoicePlanCancelExpr {
        item: u32,
    },
}

impl HirExprKind {
    pub fn child_edges(&self) -> Vec<HirExpressionChildEdge> {
        unimplemented!()
    }

    pub fn direct_expression_children(&self) -> Vec<ExprId> {
        self.child_edges()
            .into_iter()
            .map(|edge| edge.child)
            .collect()
    }
}

// -------------------------------------------------------------------------
// arcweft_lang_sema::final_analysis — checked enrichment
// -------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedNestedPathV1(Box<[CheckedNestedPathSegmentV1]>);

impl CheckedNestedPathV1 {
    pub fn try_from_segments(
        segments: Box<[CheckedNestedPathSegmentV1]>,
    ) -> Result<Self, CheckedNestedPathError> {
        (!segments.is_empty())
            .then_some(Self(segments))
            .ok_or(CheckedNestedPathError::Empty)
    }

    pub fn segments(&self) -> &[CheckedNestedPathSegmentV1] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedNestedPathError {
    Empty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedNestedPathSegmentV1 {
    ChoiceBodyItem { ordinal: u32 },
    ChoiceIfBranch { ordinal: u32 },
    ChoiceIfElse,
    ChoiceForBody,
    ChoiceMatchArm { ordinal: u32 },
    ChoiceOptionBody,
    ChoiceOptionField { ordinal: u32 },
    ChoiceViewEntry { ordinal: u32 },
    ChoicePlanItem { ordinal: u32 },
    LinePlanItem { ordinal: u32 },
    LinePlanStartGroupItem { ordinal: u32 },
    LinePlanTogetherGroupItem { ordinal: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedExpressionChildRole {
    Element {
        ordinal: u32,
    },
    RepeatedValue,
    RepeatLength,
    Callee,
    Argument {
        ordinal: u32,
    },
    Target,
    Index,
    PipeLeft,
    PipeRight,
    Operand,
    RangeStart,
    RangeEnd,
    RecordField {
        source_ordinal: u32,
        accepted_field: RuntimeRecordFieldId,
    },
    BinaryLeft,
    BinaryRight,
    ClosureBody,
    BlockTail,
    LoopTail,
    Condition,
    ThenBranch,
    ElseBranch,
    Scrutinee,
    Guard {
        arm: u32,
    },
    ArmValue {
        arm: u32,
    },
    IfLetGuard,
    DialogueTarget,
    DialogueCoordinate {
        ordinal: u32,
    },
    DialogueInterpolation {
        ordinal: u32,
    },
    DialogueTagPayload {
        ordinal: u32,
    },
    LinePlanOptionValue {
        path: CheckedNestedPathV1,
    },
    LinePlanLetValue {
        path: CheckedNestedPathV1,
    },
    LinePlanOut {
        path: CheckedNestedPathV1,
    },
    LinePlanTimelineAssert {
        path: CheckedNestedPathV1,
    },
    LinePlanExpression {
        path: CheckedNestedPathV1,
    },
    LinePlanTimedCueAnchor {
        path: CheckedNestedPathV1,
    },
    LinePlanTimedCueBody {
        path: CheckedNestedPathV1,
    },
    PostfixIndexCandidate,
    PostfixDialogueCandidate,
    ForInput,
    ChoiceIfCondition {
        path: CheckedNestedPathV1,
        branch: u32,
    },
    ChoiceForSource {
        path: CheckedNestedPathV1,
    },
    ChoiceMatchScrutinee {
        path: CheckedNestedPathV1,
    },
    ChoiceMatchGuard {
        path: CheckedNestedPathV1,
        arm: u32,
    },
    ChoiceOptionId {
        path: CheckedNestedPathV1,
    },
    ChoiceOptionForSource {
        path: CheckedNestedPathV1,
    },
    ChoiceCompactLabel {
        path: CheckedNestedPathV1,
    },
    ChoiceCompactCondition {
        path: CheckedNestedPathV1,
    },
    ChoiceCompactOut {
        path: CheckedNestedPathV1,
    },
    ChoiceOptionLabel {
        path: CheckedNestedPathV1,
        field: u32,
    },
    ChoiceOptionFieldId {
        path: CheckedNestedPathV1,
        field: u32,
    },
    ChoiceOptionValue {
        path: CheckedNestedPathV1,
        field: u32,
    },
    ChoiceOptionVisible {
        path: CheckedNestedPathV1,
        field: u32,
    },
    ChoiceOptionEnabled {
        path: CheckedNestedPathV1,
        field: u32,
    },
    ChoiceOptionOrder {
        path: CheckedNestedPathV1,
        field: u32,
    },
    ChoiceOptionHotkey {
        path: CheckedNestedPathV1,
        field: u32,
    },
    ChoiceOptionViewKey {
        path: CheckedNestedPathV1,
        field: u32,
        entry: u32,
    },
    ChoiceOptionViewValue {
        path: CheckedNestedPathV1,
        field: u32,
        entry: u32,
    },
    ChoicePlanAssignment {
        item: u32,
    },
    ChoicePlanTimeout {
        item: u32,
    },
    ChoicePlanCancelSignal {
        item: u32,
    },
    ChoicePlanCancelTimeout {
        item: u32,
    },
    ChoicePlanCancelExpr {
        item: u32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedChildEdgeError {
    MissingExpression,
    ChildCountMismatch,
    ChildIdentityMismatch,
    MissingCheckedRecordField,
    MissingCallFacts,
    CallSlotMismatch,
    MissingNestedPath,
    StaleNestedPath,
    WorkLimit,
}

impl FinalSemanticAnalysis {
    pub fn checked_child_edges(
        &self,
        owner: ExprId,
    ) -> Result<Vec<(ExprId, CheckedExpressionChildRole)>, CheckedChildEdgeError> {
        unimplemented!()
    }
}

// -------------------------------------------------------------------------
// Ownership result vocabulary — exhaustive TypeKind implementation in sema
// -------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeProducerArgumentAdmission {
    Copy(RuntimeOwnershipProjection),
    SnapshotClone(RuntimeOwnershipProjection),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeOwnershipProjection {
    Checked(RuntimeCheckedTypeProjectionV1),
    Sequence(RuntimeSequenceProjectionV1),
    Operational(RuntimeOwnershipTypeProjectionV1),
    Nominal(RuntimeNominalRecordProjectionV1),
    ProjectNominal(RuntimeProjectNominalProjectionV1),
    AcceptedNominal(RuntimeAcceptedNominalProjectionV1),
    Agent(RuntimeAgentValueProjectionV1),
    Text(RuntimeTextProjectionV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskEventOrderKey {
    pub logical_epoch: LogicalEpoch,
    pub task_id: TaskId,
    pub sequence: TaskSequence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedTaskEventOrderKey {
    pub generation: GenerationId,
    pub event: TaskEventOrderKey,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAgentValueProjectionV1 {
    ActionTarget,
    CaptureTarget,
    DebugStatePath,
    ObservationFieldPath,
    Probe,
    Diagnostics,
    Predicate,
    ViewportPoint,
}

// Unqualified types not defined in this excerpt are unchanged existing owners
// or accepted parent definitions. Rust name resolution during implementation,
// not a duplicate placeholder declaration here, binds them to their owners.
