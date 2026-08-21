//! Design-only Rust-shaped contract.
//!
//! This file is not a production patch and is not a standalone crate. Existing
//! Arcweft types named here remain on their current owners. New behavior for an
//! Arcweft-owned enum/struct is an inherent implementation on that owner; this
//! design does not use an extension trait to avoid editing the real owner.

// crates/arcweft-core/src/task.rs

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

impl RuntimeNeedHandle {
    pub fn try_reusable_join(
        active_generation: GenerationId,
        producer: NeedProducerSpec,
        outcome: TaskOutcomeContract,
        spec: TaskSpec,
        catalog: &TaskContractCatalogV1,
    ) -> Result<Self, RuntimeNeedHandleConstructionError>;

    pub(crate) fn try_from_accepted_launch(
        accepted: AcceptedTaskLaunch<'_>,
    ) -> Result<Self, RuntimeNeedHandleConstructionError>;

    pub const fn correlation(&self) -> &TaskCorrelation;
    pub const fn producer(&self) -> &NeedProducerSpec;
    pub const fn outcome(&self) -> &TaskOutcomeContract;
    pub const fn state(&self) -> &RuntimeNeedHandleState;
    pub const fn need_id(&self) -> NeedId;

    pub fn validate_structure(
        &self,
        catalog: &TaskContractCatalogV1,
    ) -> Result<(), RuntimeNeedHandleConstructionError>;

    pub fn validate_use(
        &self,
        active_generation: GenerationId,
    ) -> Result<(), RuntimeNeedUseError>;
}

// PartialEq/Eq/Hash/PartialOrd/Ord are manually implemented from NeedId only.
// Generation/correlation/spec/state remain structural and use-time evidence.

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeNeedHandleConstructionError {
    InactiveGeneration,
    NonJoinReusableSpec,
    NonZeroReusableOrdinal,
    ProducerMismatch,
    OutcomeMismatch,
    PolicyMismatch,
    ExecutionMismatch,
    RequestMismatch,
    CatalogMismatch,
    InvalidCorrelation,
    UncommittedLaunch,
    ReusableAlwaysStart,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeAwaitManyAggregateRequest {
    captured: Box<[RuntimeValue]>,
    source_items: Box<[RuntimeValue]>,
    child: Box<NeedProducerTemplate>,
    limit: NonZeroU32,
}

impl RuntimeAwaitManyAggregateRequest {
    pub fn try_new(
        captured: Box<[RuntimeValue]>,
        source_items: Box<[RuntimeValue]>,
        child: Box<NeedProducerTemplate>,
        limit: NonZeroU32,
        limits: RuntimeAwaitManyLimits,
    ) -> Result<Self, RuntimeAwaitManyRequestError>;

    pub fn child_argument(
        &self,
        source_index: u32,
    ) -> Result<RuntimeValue, RuntimeAwaitManyRequestError>;

    pub fn child_spec(
        &self,
        source_index: u32,
        generation: GenerationId,
        catalog: &TaskContractCatalogV1,
        limits: RuntimeValueCanonicalLimits,
    ) -> Result<TaskSpec, RuntimeAwaitManyRequestError>;

    pub fn aggregate_base_argument(
        &self,
    ) -> RuntimeValue;
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeedProducerTemplate {
    producer: NeedProducerTemplateIdentityV1,
    class: TaskClass,
    priority: TaskPriority,
    cancel_scope: CancelScopeId,
    policy: TaskPolicy,
    outcome: TaskOutcomeContract,
    execution: TaskExecutionTemplateV1,
    debug: TaskDebugMetadataTemplateV1,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaskExecutionTemplateV1 {
    Host(HostTaskRequestTemplateV1),
    Runtime(RuntimeTaskRequestTemplateV1),
}

impl NeedProducerTemplate {
    pub fn instantiate(
        &self,
        argument: &RuntimeValue,
        catalog: &TaskContractCatalogV1,
        limits: RuntimeValueCanonicalLimits,
    ) -> Result<TaskSpec, NeedProducerTemplateError>;
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskObserverId(NonZeroU64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskObserverKey {
    generation: GenerationId,
    id: TaskObserverId,
}

#[derive(Clone, Debug)]
pub struct RuntimeGenerationJournal {
    generation: GenerationId,
    next_always_start_ordinal: NonZeroU64,
    next_observer_id: NonZeroU64,
    // retained task, Need, observer, scope and publication rows
}

impl RuntimeGenerationJournal {
    pub fn plan_observer_ids(
        &self,
        count: usize,
    ) -> Result<PlannedObserverIds, TaskObserverAllocationError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskObserverAllocationError {
    ObserverIdOverflow,
    TooManyObservers,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeGenerationJournalSnapshotV1 {
    pub version: RuntimeGenerationJournalSnapshotVersion,
    pub generation: GenerationId,
    pub next_always_start_ordinal: NonZeroU64,
    pub next_observer_id: NonZeroU64,
    pub task_rows: Box<[TaskJournalRowSnapshotV1]>,
    pub need_rows: Box<[RuntimeNeedCellSnapshotV1]>,
    pub observer_rows: Box<[TaskObserverSnapshotV1]>,
    pub scope_rows: Box<[TaskScopeSnapshotV1]>,
}

// crates/arcweft-runtime-scheduler/src/lib.rs

struct EnsureBatchPlan {
    journal: RuntimeJournalBatchDelta,
    runtime: RuntimeTaskBatchDelta,
    observers: RuntimeObserverBatchDelta,
    prepared_host: Vec<PreparedLaunch>,
    results: Vec<(u32, RuntimeNeedHandle, TaskObserverId)>,
}

struct RuntimeJournalBatchDelta {
    tasks_after: BTreeMap<TaskId, TaskJournalRow>,
    needs_after: BTreeMap<NeedId, RuntimeNeedCell>,
    scopes_after: BTreeMap<CancelScopeId, RuntimeTaskScope>,
    next_always_start_ordinal_after: NonZeroU64,
}

struct RuntimeTaskBatchDelta {
    tasks_after: BTreeMap<TaskId, RuntimeTask>,
    aggregate_after: RuntimeAwaitManyAggregateTask,
}

struct RuntimeObserverBatchDelta {
    observers_after: BTreeMap<TaskObserverId, TaskObserver>,
    need_observer_sets_after: BTreeMap<NeedId, BTreeSet<TaskObserverId>>,
    next_observer_id_after: NonZeroU64,
}

impl<A: TaskLaunchAdapter> RuntimeTaskScheduler<A> {
    fn ensure_task_batch(
        &mut self,
        aggregate: TaskId,
        source_indices: Range<u32>,
    ) -> Result<(), TaskEnsureBatchError>;

    pub fn await_handle(
        &mut self,
        handle: &RuntimeNeedHandle,
        kind: TaskObserverKind,
    ) -> Result<TaskObserverId, RuntimeNeedAwaitError>;

    pub fn cancel_tasks(
        &mut self,
        request: RuntimeCancelRequest,
    ) -> Result<RuntimeCancelResult, TaskCancelTransactionError>;
}

struct CancelTransactionPlan {
    launch: RuntimeLaunchCancelDelta,
    needs: RuntimeNeedCancelDelta,
    observers: RuntimeObserverCancelDelta,
    runtime: RuntimeTaskCancelDelta,
    scopes: RuntimeScopeCancelDelta,
    pending_events: RuntimeEventCancelDelta,
    prepared_host: Vec<PreparedCancel>,
    results: Vec<TaskCancelDisposition>,
}

// crates/arcweft-core/src/task/adapter.rs

pub trait TaskLaunchAdapter {
    type PreparedLaunch;
    type PrepareLaunchError;
    type PreparedRestore;
    type PrepareRestoreError;
    type PreparedRebind;
    type PrepareRebindError;
    type PreparedCancel;
    type PrepareCancelError;

    fn prepare_launch(
        &mut self,
        batch: HostTaskLaunchBatch,
    ) -> Result<Self::PreparedLaunch, Self::PrepareLaunchError>;
    fn commit_launch(&mut self, prepared: Self::PreparedLaunch);
    fn rollback_launch(&mut self, prepared: Self::PreparedLaunch);

    fn prepare_restore(
        &mut self,
        batch: HostTaskRestoreBatch,
    ) -> Result<Self::PreparedRestore, Self::PrepareRestoreError>;
    fn commit_restore(&mut self, prepared: Self::PreparedRestore);
    fn rollback_restore(&mut self, prepared: Self::PreparedRestore);

    fn prepare_rebind(
        &mut self,
        batch: HostTaskRebindBatch,
    ) -> Result<Self::PreparedRebind, Self::PrepareRebindError>;
    fn commit_rebind(&mut self, prepared: Self::PreparedRebind);
    fn rollback_rebind(&mut self, prepared: Self::PreparedRebind);

    fn prepare_cancel(
        &mut self,
        batch: HostTaskCancelBatch,
    ) -> Result<Self::PreparedCancel, Self::PrepareCancelError>;
    fn commit_cancel(&mut self, prepared: Self::PreparedCancel);
    fn rollback_cancel(&mut self, prepared: Self::PreparedCancel);
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostOperationCatalogDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostOperationId(NonZeroU32);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostOperationIdentityV1 {
    Builtin(BuiltinHostOperationIdV1),
    Catalog {
        catalog_digest: HostOperationCatalogDigest,
        operation: HostOperationId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostOperationCatalogV1 {
    digest: HostOperationCatalogDigest,
    rows: Box<[HostOperationCatalogRowV1]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostOperationCatalogRowV1 {
    id: HostOperationId,
    capability: HostCapabilityId,
    request_contract: HostTaskRequestContractV1,
    restart: HostRestartPolicy,
    cancellation: HostCancellationContractV1,
    route: HostRouteId,
}

impl HostOperationCatalogV1 {
    pub fn try_new(
        rows: Box<[HostOperationCatalogRowV1]>,
    ) -> Result<Self, HostOperationCatalogError>;

    pub fn resolve(
        &self,
        identity: &HostOperationIdentityV1,
    ) -> Result<&HostOperationCatalogRowV1, HostOperationCatalogError>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskLaunchBatch {
    pub generation: GenerationId,
    pub rows: Box<[HostTaskLaunchRow]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskLaunchRow {
    pub source_index: u32,
    pub correlation: TaskCorrelation,
    pub operation: HostOperationIdentityV1,
    pub request: HostTaskRequest,
    pub outcome: TaskOutcomeContract,
    pub restart: HostRestartPolicy,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskRestoreBatch {
    pub generation: GenerationId,
    pub rows: Box<[HostTaskRestoreRow]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskRestoreRow {
    pub correlation: TaskCorrelation,
    pub complete_spec: TaskSpec,
    pub operation: HostOperationIdentityV1,
    pub prior_launch_capability: HostLaunchCapability,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskRebindBatch {
    pub old_generation: GenerationId,
    pub new_generation: GenerationId,
    pub rows: Box<[HostTaskRebindRow]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskRebindRow {
    pub old_correlation: TaskCorrelation,
    pub new_correlation: TaskCorrelation,
    pub operation: HostOperationIdentityV1,
    pub launch_capability: HostLaunchCapability,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskCancelBatch {
    pub generation: GenerationId,
    pub rows: Box<[HostTaskCancelRow]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskCancelRow {
    pub command: HostCancelCommandId,
    pub correlation: TaskCorrelation,
    pub operation: HostOperationIdentityV1,
    pub launch: HostLaunchCapability,
    pub cancel: HostCancellationCapability,
    pub reason: TaskCancelReason,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostCancelCommandId([u8; 32]);

impl HostCancelCommandId {
    pub fn derive(
        correlation: &TaskCorrelation,
    ) -> Result<Self, TaskIdentityError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostRestartPolicy {
    MustBeQuiescent,
    Restartable,
}

// crates/arcweft-core/src/value/awbc_save.rs
// Existing owner evolved in place; no second compatibility DTO/reader.

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
    Progress { ratio: f32, label: Option<String> },
    Range(RuntimeRange),
    Iterator(AwbcRuntimeIteratorSnapshot),
    EntityRef(String),
    Tuple(Box<[Self]>),
    Seq(AwbcRuntimeSeqSnapshot),
    Record(Box<[AwbcRuntimeFieldSnapshot]>),
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

#[derive(Clone, Debug, PartialEq)]
pub enum AwbcRuntimeIteratorSnapshot {
    Values {
        items: Box<[AwbcRuntimeValueSnapshot]>,
        index: u64,
    },
    Range(RuntimeRangeIterator),
    Witness {
        state: Box<AwbcRuntimeValueSnapshot>,
        next: RuntimeTraitMethodId,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum AwbcRuntimeSeqSnapshot {
    Values(Box<[AwbcRuntimeValueSnapshot]>),
    Dense(DenseSeq),
    TupleColumns {
        len: u64,
        columns: Box<[AwbcRuntimeSeqSnapshot]>,
    },
    RecordColumns {
        len: u64,
        fields: Box<[AwbcRuntimeRecordSeqFieldSnapshot]>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum DenseSeq {
    Units(DenseSeqStorage<()>),
    Bools(DenseSeqStorage<bool>),
    I8(DenseSeqStorage<i8>),
    I16(DenseSeqStorage<i16>),
    I32(DenseSeqStorage<i32>),
    I64(DenseSeqStorage<i64>),
    I128(DenseSeqStorage<i128>),
    ISize(DenseSeqStorage<RuntimeISizeValue>),
    U8(DenseSeqStorage<u8>),
    U16(DenseSeqStorage<u16>),
    U32(DenseSeqStorage<u32>),
    U64(DenseSeqStorage<u64>),
    U128(DenseSeqStorage<u128>),
    USize(DenseSeqStorage<RuntimeUSizeValue>),
    F32(DenseSeqStorage<f32>),
    F64(DenseSeqStorage<f64>),
    Strings(DenseSeqStorage<String>),
    Chars(DenseSeqStorage<char>),
    Durations(DenseSeqStorage<LogicalDuration>),
    EntityRefs(DenseSeqStorage<String>),
    Bytes(DenseSeqStorage<u8>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AwbcRuntimeRecordSeqFieldSnapshot {
    pub field: RuntimeRecordFieldId,
    pub name: String,
    pub values: AwbcRuntimeSeqSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AwbcRuntimeFieldSnapshot {
    pub field: RuntimeRecordFieldId,
    pub name: String,
    pub value: AwbcRuntimeValueSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AwbcRuntimeNominalRecordSnapshot {
    pub type_id: RuntimeNominalTypeId,
    pub layout: TypeLayoutHash,
    pub fields: Box<[AwbcRuntimeValueSnapshot]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AwbcRuntimeOpaqueSnapshot {
    pub producer: RuntimeOpaqueTypeProducerId,
    pub semantic_identity: RuntimeSemanticTypeId,
    pub value_class: RuntimeOpaqueValueClass,
    pub persistence: RuntimeOpaquePersistence,
    pub payload: Box<AwbcRuntimeValueSnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AwbcRuntimeReductionSnapshot {
    pub owner: RuntimeOpaqueTypeOwner,
    pub state: Box<AwbcRuntimeValueSnapshot>,
    pub commands: Box<[AwbcRuntimeCommandSnapshot]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AwbcRuntimeCommandSnapshot {
    pub constructor: RuntimeCommandConstructorId,
    pub target: RuntimeCommandTargetId,
    pub payload: Box<AwbcRuntimeValueSnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AwbcRuntimeAgentSnapshot {
    ActionTarget(RuntimeAgentActionTarget),
    CaptureTarget(RuntimeAgentCaptureTarget),
    DebugStatePath(RuntimeAgentPath),
    ObservationFieldPath(RuntimeAgentPath),
    Probe(RuntimeAgentProbe),
    Diagnostics,
    Predicate(AwbcRuntimeAgentPredicateSnapshot),
    ViewportPoint { x: u32, y: u32 },
}

#[derive(Clone, Debug, PartialEq)]
pub enum AwbcRuntimeAgentPredicateSnapshot {
    Compare {
        probe: RuntimeAgentProbe,
        op: RuntimeAgentCompareOp,
        value: Box<AwbcRuntimeValueSnapshot>,
    },
    Exists { probe: RuntimeAgentProbe },
    ActionEnabled { target: RuntimeCommandTargetId },
    DiagnosticsHasError,
    All { predicates: Box<[Self]> },
    Any { predicates: Box<[Self]> },
    Not { predicate: Box<Self> },
}

#[derive(Clone, Debug, PartialEq)]
pub enum AwbcRuntimeFunctionSnapshot {
    Awbc {
        function: AwbcFunctionId,
        remaining_params: Box<[String]>,
        captures: Box<[AwbcRuntimeBindingSnapshot]>,
        authority: AwbcExecutableAuthorityRefV1,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct AwbcRuntimeBindingSnapshot {
    pub name: String,
    pub value: Box<AwbcRuntimeValueSnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeNeedHandleSnapshotV1 {
    pub correlation: TaskCorrelation,
    pub producer: NeedProducerSpec,
    pub outcome: TaskOutcomeContract,
    pub state: RuntimeNeedHandleStateSnapshotV1,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeNeedHandleStateSnapshotV1 {
    ReusableJoin { spec: Box<TaskSpecSnapshotV1> },
    AcceptedLaunch,
}

#[derive(Clone, Debug, PartialEq)]
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

// crates/arcweft-lang-sema/src/final_analysis/match_transcript.rs

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedExpressionChildRole {
    Element { ordinal: u32 },
    RepeatedValue,
    RepeatLength,
    Callee,
    Argument { ordinal: u32 },
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
    Guard { arm: u32 },
    ArmValue { arm: u32 },
    IfLetGuard,
    DialogueTarget,
    DialogueCoordinate { ordinal: u32 },
    DialogueInterpolation { ordinal: u32 },
    DialogueTagPayload { ordinal: u32 },
    LinePlanOptionValue { path: CheckedNestedPathV1 },
    LinePlanLetValue { path: CheckedNestedPathV1 },
    LinePlanOut { path: CheckedNestedPathV1 },
    LinePlanTimelineAssert { path: CheckedNestedPathV1 },
    LinePlanExpression { path: CheckedNestedPathV1 },
    LinePlanTimedCueAnchor { path: CheckedNestedPathV1 },
    LinePlanTimedCueBody { path: CheckedNestedPathV1 },
    PostfixIndexCandidate,
    PostfixDialogueCandidate,
    ForInput,
    ChoiceIfCondition { path: CheckedNestedPathV1, branch: u32 },
    ChoiceForSource { path: CheckedNestedPathV1 },
    ChoiceMatchScrutinee { path: CheckedNestedPathV1 },
    ChoiceMatchGuard { path: CheckedNestedPathV1, arm: u32 },
    ChoiceOptionId { path: CheckedNestedPathV1 },
    ChoiceOptionForSource { path: CheckedNestedPathV1 },
    ChoiceCompactLabel { path: CheckedNestedPathV1 },
    ChoiceCompactCondition { path: CheckedNestedPathV1 },
    ChoiceCompactOut { path: CheckedNestedPathV1 },
    ChoiceOptionLabel { path: CheckedNestedPathV1, field: u32 },
    ChoiceOptionFieldId { path: CheckedNestedPathV1, field: u32 },
    ChoiceOptionValue { path: CheckedNestedPathV1, field: u32 },
    ChoiceOptionVisible { path: CheckedNestedPathV1, field: u32 },
    ChoiceOptionEnabled { path: CheckedNestedPathV1, field: u32 },
    ChoiceOptionOrder { path: CheckedNestedPathV1, field: u32 },
    ChoiceOptionHotkey { path: CheckedNestedPathV1, field: u32 },
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
    ChoicePlanAssignment { item: u32 },
    ChoicePlanTimeout { item: u32 },
    ChoicePlanCancelSignal { item: u32 },
    ChoicePlanCancelTimeout { item: u32 },
    ChoicePlanCancelExpr { item: u32 },
}

impl CheckedExpressionChildRole {
    pub const fn semantic_tag(&self) -> u16;
    pub fn write_payload(
        &self,
        sink: &mut CheckedMatchTranscriptSinkV1,
    ) -> Result<(), CheckedMatchTranscriptError>;
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskEventOrderKey {
    pub generation: GenerationId,
    pub logical_epoch: u64,
    pub task_id: TaskId,
    pub sequence: u64,
}
