# Rust-shaped final schemas

These are normative API shapes, not production patches. Types already owned by
Arcweft remain on their original enum/struct owners; the design does not use
extension traits to avoid editing Arcweft-owned enums.

All fixed 32-byte identity newtypes have private fields. `NeedProducerInstanceKey`,
`NeedId`, `TaskKey`, and `TaskId` reject all-zero bytes. Semantic digests accept
all 32-byte outputs. Every `*Version` constructor accepts exactly `1`.

## 1. Canonical value identity and constant admission

```rust
// crates/arcweft-core/src/entry/schema.rs

pub(crate) trait CanonicalRuntimeValueSink {
    fn write(&mut self, bytes: &[u8]) -> Result<(), RuntimeValueCanonicalError>;
    fn bytes_written(&self) -> u64;
}

pub(crate) struct CanonicalBytesSink {
    bytes: Vec<u8>,
    limits: RuntimeValueCanonicalLimits,
}

pub(crate) struct CanonicalBlake3Sink {
    hasher: blake3::Hasher,
    bytes_written: u64,
    limits: RuntimeValueCanonicalLimits,
}

impl RuntimeValue {
    pub(crate) fn write_canonical(
        &self,
        sink: &mut impl CanonicalRuntimeValueSink,
        work: &mut RuntimeValueCanonicalWork,
    ) -> Result<(), RuntimeValueCanonicalError>;

    pub fn try_canonical_bytes(
        &self,
        limits: RuntimeValueCanonicalLimits,
    ) -> Result<Vec<u8>, RuntimeValueCanonicalError>;

    pub fn try_digest(
        &self,
        limits: RuntimeValueCanonicalLimits,
    ) -> Result<RuntimeValueDigest, RuntimeValueCanonicalError>;

    pub fn validate_constant_admission(
        &self,
        limits: RuntimeValueConstantAdmissionLimits,
    ) -> Result<(), RuntimeValueConstantAdmissionError>;

    pub fn try_constant_canonical_bytes(
        &self,
        canonical_limits: RuntimeValueCanonicalLimits,
        admission_limits: RuntimeValueConstantAdmissionLimits,
    ) -> Result<Vec<u8>, RuntimeValueConstantError>;
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeValueConstantAdmissionError {
    #[error("snapshot-only opaque value is not a publishable constant")]
    SnapshotOnlyOpaque,
    #[error("affine opaque handle is not a publishable constant")]
    AffineOpaque,
    #[error("runtime Need handle is not a publishable constant")]
    NeedHandle,
    #[error("runtime/frame-local value is not a publishable constant")]
    RuntimeLocal,
    #[error("constant-admission work limit exceeded")]
    WorkLimit,
}
```

The opaque arm stays in the original exhaustive visitor:

```rust
RuntimeValue::Opaque(value) => {
    value.validate_payload_and_semantics()?;
    match value.class() {
        RuntimeOpaqueValueClass::Plain => {}
        RuntimeOpaqueValueClass::AffineHandle(_) => {
            return Err(RuntimeValueCanonicalError::AffineOpaqueIdentity);
        }
    }
    match value.persistence() {
        RuntimeOpaquePersistence::ConstantAndSnapshot
        | RuntimeOpaquePersistence::SnapshotOnly => {}
    }
    // Existing opaque tag and exact existing transcript are emitted here.
    write_opaque_v1(value, sink, work)?;
}
```

There is no producer-only serializer. A non-producer diagnostic
`RuntimeValueDigest` also rejects affine handles because no stable value identity
exists for them. The snapshot codec may encode an affine handle only for a
typed, validated save/restore row; that row never mints `RuntimeValueDigest` and
does not alter View or producer admission.

## 2. Identity and producer owners

```rust
// crates/arcweft-core/src/task.rs

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GenerationId(u64); // zero is valid

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NeedProducerInstanceKey([u8; 32]); // all-zero rejected

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NeedId([u8; 32]); // all-zero rejected

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskKey([u8; 32]); // all-zero rejected

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskId([u8; 32]); // all-zero rejected

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskLaunchOrdinal(u64);

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
    pub fn try_new(
        family: NeedProducerFamily,
        contract: NeedProducerContractDigest,
        plan: TaskPlanSemanticDigest,
        producer_site: u32,
        payload_type: RuntimeTypeSemanticDigest,
        arguments: RuntimeValueDigest,
    ) -> Result<Self, NeedProducerSpecError>;

    pub fn instance_key(&self) -> Result<NeedProducerInstanceKey, NeedProducerIdentityError>;
}
```

The inherited v1 transcripts remain exactly:

```text
NeedProducerInstanceKey =
  BLAKE3("arcweft.need.producer-instance.v1\0"
       || family || contract || plan || producer_site
       || payload_type || arguments)

NeedId =
  BLAKE3("arcweft.need.id.v1\0"
       || producer_instance || policy || launch_ordinal)

TaskKey =
  BLAKE3("arcweft.task.key.v1\0"
       || generation || producer_instance || policy)

TaskId =
  BLAKE3("arcweft.task.id.v1\0"
       || task_key || launch_ordinal)
```

`TaskKey` excludes the ordinal. `TaskId` contains it exactly once. Join uses
ordinal zero. AlwaysStart uses a journal-owned positive counter beginning at
one.

## 3. One `TaskExecution` field

```rust
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
pub struct RuntimeAwaitManyAggregateRequest {
    source_items: Box<[RuntimeValue]>,
    children: Box<[TaskSpec]>,
    limit: NonZeroU32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeTimeoutRequest {
    source: RuntimeNeedHandle,
    requested_limit: LogicalDuration,
    contract: NeedTimeoutContractDigest,
}

impl TaskSpec {
    pub fn try_new(
        producer: NeedProducerSpec,
        class: TaskClass,
        priority: TaskPriority,
        cancel_scope: CancelScopeId,
        policy: TaskPolicy,
        outcome: TaskOutcomeContract,
        execution: TaskExecution,
        debug: TaskDebugMetadata,
    ) -> Result<Self, TaskSpecError>;

    pub fn validate_family_execution_policy(&self) -> Result<(), TaskSpecError>;

    pub fn structurally_eq_for_join(&self, other: &Self) -> bool;
}

impl TaskExecution {
    pub fn kind(&self) -> TaskExecutionKind;

    pub fn validate_for(
        &self,
        family: NeedProducerFamily,
        policy: TaskPolicy,
    ) -> Result<(), TaskExecutionPolicyError>;
}

impl RuntimeAwaitManyAggregateRequest {
    pub fn try_new(
        source_items: Box<[RuntimeValue]>,
        children: Box<[TaskSpec]>,
        limit: NonZeroU32,
        limits: &RuntimeSchedulerConfig,
    ) -> Result<Self, RuntimeAwaitManyRequestError>;
}
```

`source_items.len() == children.len()`. Every child has family
`AwaitManyChild`. Its producer arguments commit the captured argument tuple,
the exact `u32` source index and the exact item. A child may independently have
`TaskExecution::Host` or either explicit runtime variant. The aggregate itself
is `Runtime` and never reaches the adapter.

## 4. Complete correlation and Need handle semantics

```rust
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
    ) -> Result<Self, TaskIdentityError>;

    pub fn validate(&self) -> Result<(), TaskIdentityError>;
}

#[derive(Clone, Debug)]
pub struct RuntimeNeedHandle {
    correlation: TaskCorrelation,
    producer: NeedProducerSpec,
    outcome: TaskOutcomeContract,
    origin: NeedHandleOrigin,
}

impl RuntimeNeedHandle {
    pub fn try_new(
        correlation: TaskCorrelation,
        producer: NeedProducerSpec,
        outcome: TaskOutcomeContract,
        origin: NeedHandleOrigin,
    ) -> Result<Self, RuntimeNeedHandleError>;

    pub const fn need_id(&self) -> NeedId;
    pub const fn correlation(&self) -> &TaskCorrelation;
    pub const fn producer(&self) -> &NeedProducerSpec;
    pub const fn outcome(&self) -> &TaskOutcomeContract;

    pub fn validate_structure(&self) -> Result<(), RuntimeNeedHandleError>;

    pub fn validate_use(
        &self,
        active_generation: GenerationId,
    ) -> Result<(), RuntimeNeedUseError>;

    pub fn rebind_for_replacement(
        &self,
        new_generation: GenerationId,
        validated: &ValidatedReplacementMapping,
    ) -> Result<Self, RuntimeNeedRebindError>;
}

impl PartialEq for RuntimeNeedHandle {
    fn eq(&self, other: &Self) -> bool {
        self.need_id() == other.need_id()
    }
}
impl Eq for RuntimeNeedHandle {}
impl Hash for RuntimeNeedHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.need_id().hash(state);
    }
}
impl PartialOrd for RuntimeNeedHandle {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for RuntimeNeedHandle {
    fn cmp(&self, other: &Self) -> Ordering {
        self.need_id().cmp(&other.need_id())
    }
}
```

The final original enum arm is:

```rust
pub enum RuntimeValue {
    // every existing arm remains in its current order
    NeedHandle(RuntimeNeedHandle), // canonical tag 20, payload NeedId only
}
```

Generation, complete correlation, producer spec, outcome contract, origin and
debug metadata are structural/use evidence, not value identity.

## 5. Events, Need cells and observers

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeNeedOutcome {
    Payload(RuntimePayload),
    InfrastructureFailure(RuntimeTaskFailure),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeNeedCell {
    correlation: TaskCorrelation,
    outcome: TaskOutcomeContract,
    state: Need<RuntimeNeedOutcome>,
    last_publication: Option<TaskPublicationCursor>,
    observers: BTreeSet<TaskObserverId>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskPublicationCursor {
    logical_epoch: u64,
    sequence: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskEvent {
    correlation: TaskCorrelation,
    cursor: TaskPublicationCursor,
    kind: TaskEventKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaskEventKind {
    Progress(Progress),
    Ready(RuntimePayload),
    InfrastructureFailure(RuntimeTaskFailure),
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTaskFailure {
    kind: RuntimeTaskFailureKind,
    diagnostic: BoundedDiagnostic,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskObserver {
    id: TaskObserverId,
    generation: GenerationId,
    need_id: NeedId,
    kind: TaskObserverKind,
    last_seen: Option<TaskPublicationCursor>,
    active: bool,
}
```

A domain `Result` or `Option` error stays inside `RuntimePayload`.
Infrastructure failure is a typed runtime outcome. Cancellation is
`Need::Cancelled` and does not return a payload.

## 6. Runtime-owned task state

```rust
#[derive(Clone, Debug, Default)]
pub struct RuntimeTaskState {
    tasks: BTreeMap<TaskId, RuntimeTask>,
}

#[derive(Clone, Debug)]
pub enum RuntimeTask {
    AwaitManyAggregate(RuntimeAwaitManyAggregateTask),
    Timeout(RuntimeTimeoutNeed),
}

#[derive(Clone, Debug)]
pub struct RuntimeAwaitManyAggregateTask {
    aggregate: TaskCorrelation,
    source_items: Box<[RuntimeValue]>,
    child_specs: Box<[TaskSpec]>,
    children: Box<[RuntimeAwaitManyChild]>,
    limit: NonZeroU32,
    launch_cursor: u32,
    in_flight: u32,
    outputs: Box<[Option<RuntimePayload>]>,
    publication_cursor: TaskPublicationCursor,
    terminal: Option<RuntimeAwaitManyTerminal>,
}

#[derive(Clone, Debug)]
pub struct RuntimeAwaitManyChild {
    source_index: u32,
    observer: Option<TaskObserverId>,
    handle: Option<RuntimeNeedHandle>,
    status: RuntimeAwaitManyChildStatus,
    last_cursor: Option<TaskPublicationCursor>,
}

#[derive(Clone, Debug)]
pub struct RuntimeTimeoutNeed {
    output: TaskCorrelation,
    source: RuntimeNeedHandle,
    requested_limit: LogicalDuration,
    remaining: LogicalDuration,
    phase: RuntimeTimeoutPhase,
    source_observer: Option<TaskObserverId>,
    source_cursor: Option<TaskPublicationCursor>,
    publication_cursor: TaskPublicationCursor,
    terminal: Option<RuntimeTimeoutTerminal>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeTimeoutPhase {
    NotStarted,
    Waiting,
    Resolved,
}
```

`RuntimeTimeoutNeed::step` receives only `RuntimeStepInput.dt`. Its decision
order is cancellation, normalized source terminal, expiration, then Pending.
Cancellation of the wrapper detaches its observer and does not cancel the
source.

AwaitMany chooses a bounded source-index batch without holding a mutable borrow
of its task row. It then calls the scheduler's internal ensure transaction for
those complete child specs and finally reapplies results by source index. This
is implementable with ordinary `&mut self` borrowing and no `unsafe`.

## 7. One atomic scheduler/journal/adapter owner

```rust
// crates/arcweft-runtime-scheduler/src/lib.rs

pub struct RuntimeTaskScheduler<A: TaskLaunchAdapter> {
    config: RuntimeSchedulerConfig,
    journal: RuntimeTaskJournal,
    runtime_tasks: RuntimeTaskState,
    adapter: A,
    pending_events: BTreeMap<TaskEventOrderKey, TaskEvent>,
    ready_runtime_tasks: BTreeSet<TaskId>,
    metrics: RuntimeSchedulerMetrics,
    replacement: ReplacementState,
}

pub trait TaskHost {
    fn ensure_task(
        &mut self,
        spec: TaskSpec,
    ) -> Result<RuntimeNeedHandle, TaskEnsureError>;

    fn register_observer(
        &mut self,
        handle: &RuntimeNeedHandle,
        kind: TaskObserverKind,
    ) -> Result<TaskObserverId, TaskObserverError>;

    fn remove_observer(
        &mut self,
        observer: TaskObserverId,
    ) -> Result<(), TaskObserverError>;
}

impl<A: TaskLaunchAdapter> RuntimeTaskScheduler<A> {
    pub fn ensure_task(
        &mut self,
        spec: TaskSpec,
    ) -> Result<RuntimeNeedHandle, TaskEnsureError>;

    pub fn ingest_host_events<I>(
        &mut self,
        events: I,
    ) -> Result<RuntimeEventApplyReport, RuntimeEventApplyError>
    where
        I: IntoIterator<Item = TaskEvent>;

    pub fn step_runtime_tasks(
        &mut self,
        input: RuntimeStepInput,
    ) -> Result<RuntimeTaskStepReport, RuntimeTaskStepError>;

    pub fn cancel_scope(
        &mut self,
        generation: GenerationId,
        scope: &CancelScopeId,
    ) -> Result<RuntimeCancelReport, RuntimeCancelError>;

    pub fn snapshot(
        &self,
    ) -> Result<RuntimeTaskSchedulerSnapshotV1, RuntimeSnapshotError>;

    pub fn restore(
        &mut self,
        snapshot: RuntimeTaskSchedulerSnapshotV1,
    ) -> Result<(), RuntimeRestoreError>;

    pub fn replay(
        &mut self,
        replay: TaskReplayEnvelopeV1,
    ) -> Result<RuntimeReplayReport, RuntimeReplayError>;

    pub fn prepare_replacement(
        &mut self,
        request: RuntimeReplacementRequest,
    ) -> Result<ValidatedReplacementMapping, RuntimeReplacementError>;

    pub fn commit_replacement(
        &mut self,
        mapping: ValidatedReplacementMapping,
    ) -> Result<RuntimeReplacementReport, RuntimeReplacementError>;
}
```

The driver may pass `RuntimeStepInput`, submit typed host events and request
snapshot/replacement operations. It owns no second journal, ordinal counter,
Need map or rollback protocol.

## 8. Adapter transaction closure

```rust
pub trait TaskLaunchAdapter {
    type PreparedLaunch;
    type PreparedRestore;
    type PreparedRebind;
    type PrepareLaunchError: Error + Send + Sync + 'static;
    type PrepareRestoreError: Error + Send + Sync + 'static;
    type PrepareRebindError: Error + Send + Sync + 'static;

    fn prepare_launch(
        &mut self,
        launch: HostTaskLaunchRequest,
    ) -> Result<Self::PreparedLaunch, Self::PrepareLaunchError>;

    fn commit_launch(&mut self, prepared: Self::PreparedLaunch);
    fn rollback_launch(&mut self, prepared: Self::PreparedLaunch);

    fn prepare_restore(
        &mut self,
        restore: HostTaskRestoreBatch,
    ) -> Result<Self::PreparedRestore, Self::PrepareRestoreError>;

    fn commit_restore(&mut self, prepared: Self::PreparedRestore);
    fn rollback_restore(&mut self, prepared: Self::PreparedRestore);

    fn prepare_rebind(
        &mut self,
        rebind: HostTaskRebindBatch,
    ) -> Result<Self::PreparedRebind, Self::PrepareRebindError>;

    fn commit_rebind(&mut self, prepared: Self::PreparedRebind);
    fn rollback_rebind(&mut self, prepared: Self::PreparedRebind);
}
```

`HostTaskLaunchRequest` can only be constructed from `TaskExecution::Host`.
Runtime rows have no conversion into it. `TaskEnsureError` has no
`AdapterCommit` branch:

```rust
pub enum TaskEnsureError {
    InvalidSpec(TaskSpecError),
    InvalidProducer(NeedProducerSpecError),
    FamilyExecutionMismatch(TaskExecutionPolicyError),
    PolicyMismatch(TaskPolicyError),
    JournalLimit(RuntimeJournalLimitError),
    JoinSpecConflict,
    OrdinalOverflow,
    IdentityDerivation(TaskIdentityError),
    AdapterPrepare(Box<dyn Error + Send + Sync>),
    StagingInvariant(RuntimeJournalInvariantError),
}
```

All fallible work ends before the in-memory delta and prepared token are
committed. Commit is infallible by trait contract. Any failure after prepare
calls rollback and leaves the journal/counter unchanged. Replacement uses the
same rule.

## 9. Match semantic substrate

```rust
// crates/arcweft-lang-sema/src/final_analysis/model.rs

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CheckedMatchRef {
    snapshot: HirSnapshotId,
    expression: ExprId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceptedDeclarationSemanticId([u8; 32]);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedExpressionChildRolePath {
    declaration: AcceptedDeclarationSemanticId,
    steps: Box<[CheckedExpressionChildRoleStep]>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedExpressionChildRoleStep {
    Field {
        role: CheckedExpressionChildRole,
        field: Option<AcceptedFieldIdentityProjection>,
    },
    Indexed {
        role: CheckedExpressionChildRole,
        ordinal: u32,
    },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StableCheckedValueCoordinate {
    Expression {
        declaration: AcceptedDeclarationSemanticId,
        path: CheckedExpressionChildRolePath,
    },
    PatternBinding {
        declaration: AcceptedDeclarationSemanticId,
        match_path: CheckedExpressionChildRolePath,
        arm_ordinal: u32,
        pattern: StablePatternCoordinate,
        binding_ordinal: u32,
    },
    Capture {
        callable: AcceptedDeclarationSemanticId,
        capture_ordinal: u32,
        origin: Box<StableCheckedValueCoordinate>,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedExpressionSemanticDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedPatternSemanticDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedMatchSemanticDigest([u8; 32]);
```

Construction APIs remain on the final-analysis owner:

```rust
impl FinalSemanticAnalysis {
    pub fn checked_match_ref(
        &self,
        module: &HirModule,
        symbols: &ProjectSymbolTable,
        expression: ExprId,
    ) -> Result<CheckedMatchRef, CheckedMatchLookupError>;

    pub fn build_checked_match(
        &self,
        project: HirExecutableProjectView<'_>,
        symbols: &ProjectSymbolTable,
        reference: CheckedMatchRef,
        limits: CheckedMatchLimits,
    ) -> Result<CheckedMatch, CheckedMatchError>;
}
```

Raw `ExprId`, `PatternId`, `LocalId`, `ItemId`, `TypeId`, `SourceSpan` and
`HirName` may be used only to query current accepted facts. Emitted transcripts
contain accepted identities, declaration-rooted role paths, source-order
ordinals, exact checked types/layouts/contracts and semantic literal payloads.

## 10. Compiler-local and persistent View rows

```rust
// Cut 3: compiler-local only
pub struct CompilerLocalViewMatchCatalogRow {
    checked_match: CheckedMatchRef,
    program: ViewProgramId,
    accepted_revision: AcceptedViewProgramRevision,
    site: ViewMatchSiteId,
    checked_match_semantic: CheckedMatchSemanticDigest,
    view_admission: CheckedViewMatchAdmissionDigest,
    need_admission: CheckedNeedProducerAdmissionDigest,
    ownership: OwnershipEvidenceDigest,
    resource_dependency: Option<ResourceDependencyDigest>,
}

// Cut 5: persistent projection only
pub struct AcceptedViewMatchBundleRowV1 {
    version: ViewMatchBundleRowVersion, // exactly one
    program: ViewProgramIdProjection,
    accepted_revision: AcceptedViewProgramRevisionProjection,
    site: ViewMatchSiteIdProjection,
    checked_match: CheckedMatchSemanticDigestProjection,
    view_admission: CheckedViewMatchAdmissionDigestProjection,
    need_admission: CheckedNeedProducerAdmissionDigestProjection,
    ownership: OwnershipEvidenceDigestProjection,
    producer_contract: NeedProducerContractDigest,
    payload_type: RuntimeTypeSemanticDigest,
    plan: TaskPlanSemanticDigest,
    arguments: RuntimeValueDigest,
    resource_dependency: Option<ResourceDependencyDigestProjection>,
}
```

The persistent row cannot contain `CheckedMatchRef`, `ExprId`, `HirSnapshotId`,
`SourceSpan`, a compiler certificate object or a copied compiler-local catalog
row.

## 11. Snapshot codec primitives

```rust
pub struct RuntimeTaskSnapshotCodecV1 {
    limits: RuntimeSnapshotLimits,
}

impl RuntimeTaskSnapshotCodecV1 {
    pub fn encode(
        &self,
        snapshot: &RuntimeTaskSchedulerSnapshotV1,
    ) -> Result<Vec<u8>, RuntimeSnapshotEncodeError>;

    pub fn decode<'a>(
        &self,
        bytes: &'a [u8],
    ) -> Result<RuntimeTaskSchedulerSnapshotV1, RuntimeSnapshotDecodeError>;
}
```

The codec is purpose-built. Version markers are one byte and exactly `1`.
Fixed IDs are raw 32-byte payloads. `GenerationId`, ordinals and cursors are
fixed little-endian. Lengths are canonical shortest `u32` varints. `Option<T>`
is byte `0`, or byte `1` followed by `T`. Lists are bounded before allocation.
Maps are encoded as sorted row lists and reject duplicate/out-of-order keys.
Unknown discriminants, duplicate fields, nonminimal varints and trailing bytes
are hard errors. Generic Serde is not the normative wire authority.

## 12. Complete version-1 snapshot and replay row inventory

The following declarations are generated from `machine/persistence_schemas.json` and are normative for field presence and order.

### `RuntimeTaskSchedulerSnapshotV1`

```rust
pub struct RuntimeTaskSchedulerSnapshotV1 {
    pub version: RuntimeTaskSchedulerSnapshotVersion,
    pub journal: RuntimeTaskJournalSnapshotV1,
    pub runtime_tasks: RuntimeTaskStateSnapshotV1,
    pub pending_events: Vec<TaskEventSnapshotV1>,
    pub replacement: ReplacementStateSnapshotV1,
}
```
Key: `journal.active_generation`.  
Encoding order: fields above; pending_events sorted by event ordering key.  
Bound: RuntimeSnapshotLimits owner.
Invariants: version exactly 1; no adapter transaction may be prepared while snapshotting; pending events normalized and unique.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeTaskJournalSnapshotV1`

```rust
pub struct RuntimeTaskJournalSnapshotV1 {
    pub version: RuntimeTaskJournalSnapshotVersion,
    pub active_generation: GenerationId,
    pub generations: Vec<RuntimeTaskGenerationSnapshotV1>,
}
```
Key: `active_generation`.  
Encoding order: generations ascending GenerationId.  
Bound: RuntimeSnapshotLimits owner.
Invariants: exactly one generation row matches active_generation; generation keys unique.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeTaskGenerationSnapshotV1`

```rust
pub struct RuntimeTaskGenerationSnapshotV1 {
    pub version: RuntimeTaskGenerationSnapshotVersion,
    pub generation: GenerationId,
    pub ordinal_counters: Vec<AlwaysStartOrdinalCounterSnapshotV1>,
    pub groups: Vec<TaskGroupSnapshotV1>,
    pub launches: Vec<TaskLaunchSnapshotV1>,
    pub needs: Vec<NeedCellSnapshotV1>,
    pub observers: Vec<TaskObserverSnapshotV1>,
    pub replay: TaskReplayStateSnapshotV1,
}
```
Key: `generation`.  
Encoding order: counters by producer; groups by TaskKey; launches by TaskId; needs by NeedId; observers by ObserverId.  
Bound: RuntimeSnapshotLimits owner.
Invariants: all joins stay within this generation except NeedId identity; every cross-reference resolves exactly once.
Unknown/duplicate fields and trailing bytes reject.

### `AlwaysStartOrdinalCounterSnapshotV1`

```rust
pub struct AlwaysStartOrdinalCounterSnapshotV1 {
    pub version: AlwaysStartOrdinalCounterSnapshotVersion,
    pub producer: NeedProducerInstanceKey,
    pub next_ordinal: TaskLaunchOrdinal,
}
```
Key: `producer`.  
Encoding order: producer byte order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: next_ordinal is at least 1; next_ordinal is strictly greater than every persisted launch ordinal for this producer; Join rows have no counter.
Unknown/duplicate fields and trailing bytes reject.

### `TaskGroupSnapshotV1`

```rust
pub struct TaskGroupSnapshotV1 {
    pub version: TaskGroupSnapshotVersion,
    pub task_key: TaskKey,
    pub producer: NeedProducerInstanceKey,
    pub policy: TaskPolicy,
    pub launches: Vec<TaskLaunchMappingSnapshotV1>,
}
```
Key: `task_key`.  
Encoding order: launches by ascending ordinal.  
Bound: RuntimeSnapshotLimits owner.
Invariants: TaskKey rederives from generation, producer and policy; Join has exactly ordinal 0 and at most one launch; AlwaysStart has no ordinal 0 and unique positive ordinals.
Unknown/duplicate fields and trailing bytes reject.

### `TaskLaunchMappingSnapshotV1`

```rust
pub struct TaskLaunchMappingSnapshotV1 {
    pub version: TaskLaunchMappingSnapshotVersion,
    pub ordinal: TaskLaunchOrdinal,
    pub task_id: TaskId,
    pub need_id: NeedId,
}
```
Key: `ordinal`.  
Encoding order: ascending ordinal.  
Bound: RuntimeSnapshotLimits owner.
Invariants: TaskId/NeedId rederive from group producer/policy/ordinal; task and need rows exist.
Unknown/duplicate fields and trailing bytes reject.

### `TaskLaunchSnapshotV1`

```rust
pub struct TaskLaunchSnapshotV1 {
    pub version: TaskLaunchSnapshotVersion,
    pub correlation: TaskCorrelationSnapshotV1,
    pub spec: TaskSpecSnapshotV1,
    pub lifecycle: TaskLifecycleSnapshotV1,
    pub last_publication: Option<TaskPublicationCursorSnapshotV1>,
    pub last_event_digest: Option<TaskEventDigest>,
    pub host_state: Option<HostTaskStateSnapshotV1>,
}
```
Key: `correlation.task_id`.  
Encoding order: TaskId byte order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: correlation rederives from spec and generation; host_state present iff TaskExecution::Host; runtime state is stored only in RuntimeTaskStateSnapshotV1.
Unknown/duplicate fields and trailing bytes reject.

### `TaskSpecSnapshotV1`

```rust
pub struct TaskSpecSnapshotV1 {
    pub version: TaskSpecSnapshotVersion,
    pub producer: NeedProducerSpecSnapshotV1,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub policy: TaskPolicy,
    pub outcome: TaskOutcomeContractSnapshotV1,
    pub execution: TaskExecutionSnapshotV1,
    pub debug: TaskDebugMetadataSnapshotV1,
}
```
Encoding order: fields above.  
Bound: RuntimeSnapshotLimits owner.
Invariants: exactly one execution row; family/execution/policy truth table validates; debug is excluded from identity.
Unknown/duplicate fields and trailing bytes reject.

### `NeedProducerSpecSnapshotV1`

```rust
pub struct NeedProducerSpecSnapshotV1 {
    pub version: NeedProducerSpecSnapshotVersion,
    pub family: NeedProducerFamily,
    pub contract: NeedProducerContractDigest,
    pub plan: TaskPlanSemanticDigest,
    pub producer_site: u32,
    pub payload_type: RuntimeTypeSemanticDigest,
    pub arguments: RuntimeValueDigest,
}
```
Encoding order: fields above.  
Bound: RuntimeSnapshotLimits owner.
Invariants: NeedProducerInstanceKey is recomputed; stored key is never authoritative; arguments is never RuntimeValueDigest::ZERO as an absence sentinel.
Unknown/duplicate fields and trailing bytes reject.

### `TaskCorrelationSnapshotV1`

```rust
pub struct TaskCorrelationSnapshotV1 {
    pub version: TaskCorrelationSnapshotVersion,
    pub generation: GenerationId,
    pub producer: NeedProducerInstanceKey,
    pub policy: TaskPolicy,
    pub ordinal: TaskLaunchOrdinal,
    pub need_id: NeedId,
    pub task_key: TaskKey,
    pub task_id: TaskId,
}
```
Encoding order: fields above.  
Bound: RuntimeSnapshotLimits owner.
Invariants: all fixed IDs nonzero; Join ordinal exactly 0; AlwaysStart ordinal at least 1; all three IDs are rederived and compared.
Unknown/duplicate fields and trailing bytes reject.

### `TaskLifecycleSnapshotV1`

```rust
pub enum TaskLifecycleSnapshotV1 {
    Accepted,
    Running,
    Ready,
    InfrastructureFailed,
    Cancelled,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: terminal variants agree with Need state.
Unknown/duplicate fields and trailing bytes reject.

### `TaskOutcomeContractSnapshotV1`

```rust
pub struct TaskOutcomeContractSnapshotV1 {
    pub version: TaskOutcomeContractSnapshotVersion,
    pub payload_type: RuntimeTypeSemanticDigest,
    pub runtime_checked_type: RuntimeCheckedTypeSnapshotV1,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: runtime_checked_type semantic digest equals payload_type.
Unknown/duplicate fields and trailing bytes reject.

### `TaskDebugMetadataSnapshotV1`

```rust
pub struct TaskDebugMetadataSnapshotV1 {
    pub version: TaskDebugMetadataSnapshotVersion,
    pub label: Option<BoundedUtf8>,
    pub origin: Option<BoundedUtf8>,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: diagnostic only; never read by identity/execution selection.
Unknown/duplicate fields and trailing bytes reject.

### `TaskExecutionSnapshotV1`

```rust
pub enum TaskExecutionSnapshotV1 {
    Host(HostTaskRequestSnapshotV1),
    Runtime(RuntimeTaskRequestSnapshotV1),
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: one closed discriminant; no parallel Option fields.
Unknown/duplicate fields and trailing bytes reject.

### `HostTaskRequestSnapshotV1`

```rust
pub enum HostTaskRequestSnapshotV1 {
    FileReadText {
        path: BoundedUtf8,
    },
    FileReadBytes {
        path: BoundedUtf8,
    },
    FileWriteText {
        path: BoundedUtf8,
        text: BoundedUtf8,
    },
    FileWriteBytes {
        path: BoundedUtf8,
        bytes: BoundedBytes,
    },
    HttpFetch {
        url: BoundedUtf8,
        method: BoundedUtf8,
        headers: Vec<HeaderSnapshotV1>,
        body: Option<RuntimePayloadSnapshotV1>,
    },
    HttpRespond {
        request_id: BoundedUtf8,
        status: u16,
        headers: Vec<HeaderSnapshotV1>,
        body: Option<RuntimePayloadSnapshotV1>,
    },
    ProcessRun {
        program: BoundedUtf8,
        args: Vec<BoundedUtf8>,
        env: Vec<EnvPairSnapshotV1>,
    },
    AssetLoad {
        id: BoundedUtf8,
        kind: BoundedUtf8,
    },
    ShaderCompile {
        id: BoundedUtf8,
        entry: Option<BoundedUtf8>,
    },
    AudioDecode {
        id: BoundedUtf8,
    },
    TtsSynthesis {
        voice: Option<BoundedUtf8>,
        text: BoundedUtf8,
    },
    WasmCall {
        module: BoundedUtf8,
        function: BoundedUtf8,
        args: Vec<RuntimePayloadSnapshotV1>,
    },
    SystemInfo {
        kind: SystemInfoKind,
    },
    Custom {
        capability: HostCapabilityId,
        operation: RuntimeHostOperationId,
        args: Vec<RuntimePayloadSnapshotV1>,
        named_args: Vec<NamedRuntimePayloadSnapshotV1>,
    },
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: variant matches accepted HostOperation contract; all lists bounded; headers/env/named args retain source order unless owner contract requires canonical unique order.
Unknown/duplicate fields and trailing bytes reject.

### `HeaderSnapshotV1`

```rust
pub struct HeaderSnapshotV1 {
    pub name: BoundedUtf8,
    pub value: BoundedUtf8,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `EnvPairSnapshotV1`

```rust
pub struct EnvPairSnapshotV1 {
    pub name: BoundedUtf8,
    pub value: BoundedUtf8,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `NamedRuntimePayloadSnapshotV1`

```rust
pub struct NamedRuntimePayloadSnapshotV1 {
    pub name: BoundedUtf8,
    pub value: RuntimePayloadSnapshotV1,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `HostTaskStateSnapshotV1`

```rust
pub struct HostTaskStateSnapshotV1 {
    pub version: HostTaskStateSnapshotVersion,
    pub correlation: TaskCorrelationSnapshotV1,
    pub request: HostTaskRequestSnapshotV1,
    pub phase: HostTaskPhaseSnapshotV1,
    pub restore_policy: HostTaskRestorePolicySnapshotV1,
}
```
Key: `correlation.task_id`.  
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: request equals TaskSpec Host row; in-flight MustBeQuiescent rows make snapshot fail; Restartable rows are re-prepared by adapter on restore; no adapter-private token is serialized.
Unknown/duplicate fields and trailing bytes reject.

### `HostTaskPhaseSnapshotV1`

```rust
pub enum HostTaskPhaseSnapshotV1 {
    Accepted,
    Dispatched,
    Terminal,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `HostTaskRestorePolicySnapshotV1`

```rust
pub enum HostTaskRestorePolicySnapshotV1 {
    Restartable,
    MustBeQuiescent,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeTaskRequestSnapshotV1`

```rust
pub enum RuntimeTaskRequestSnapshotV1 {
    AwaitManyAggregate(RuntimeAwaitManyAggregateRequestSnapshotV1),
    Timeout(RuntimeTimeoutRequestSnapshotV1),
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeAwaitManyAggregateRequestSnapshotV1`

```rust
pub struct RuntimeAwaitManyAggregateRequestSnapshotV1 {
    pub version: RuntimeAwaitManyAggregateRequestSnapshotVersion,
    pub source_items: Vec<RuntimeValueSnapshotV1>,
    pub children: Vec<TaskSpecSnapshotV1>,
    pub limit: NonZeroU32,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: source_items.len equals children.len; each child family is AwaitManyChild; each child producer arguments rehash from captured arguments, exact source index and item; limit bounded.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeTimeoutRequestSnapshotV1`

```rust
pub struct RuntimeTimeoutRequestSnapshotV1 {
    pub version: RuntimeTimeoutRequestSnapshotVersion,
    pub source: RuntimeNeedHandleSnapshotV1,
    pub requested_limit: LogicalDurationSnapshotV1,
    pub contract: NeedTimeoutContractDigest,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: source handle structurally validates; requested limit is the exact accepted duration; family is Timeout and policy JoinSameKey.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeTaskStateSnapshotV1`

```rust
pub struct RuntimeTaskStateSnapshotV1 {
    pub version: RuntimeTaskStateSnapshotVersion,
    pub tasks: Vec<RuntimeTaskRowSnapshotV1>,
}
```
Encoding order: tasks ascending TaskId.  
Bound: RuntimeSnapshotLimits owner.
Invariants: one runtime row for every active Runtime execution launch and no Host launch.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeTaskRowSnapshotV1`

```rust
pub struct RuntimeTaskRowSnapshotV1 {
    pub version: RuntimeTaskRowSnapshotVersion,
    pub correlation: TaskCorrelationSnapshotV1,
    pub state: RuntimeTaskRequestStateSnapshotV1,
}
```
Key: `correlation.task_id`.  
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeTaskRequestStateSnapshotV1`

```rust
pub enum RuntimeTaskRequestStateSnapshotV1 {
    AwaitManyAggregate(RuntimeAwaitManyAggregateTaskSnapshotV1),
    Timeout(RuntimeTimeoutNeedSnapshotV1),
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeAwaitManyAggregateTaskSnapshotV1`

```rust
pub struct RuntimeAwaitManyAggregateTaskSnapshotV1 {
    pub version: RuntimeAwaitManyAggregateTaskSnapshotVersion,
    pub aggregate: TaskCorrelationSnapshotV1,
    pub source_count: u32,
    pub source_items: Vec<RuntimeValueSnapshotV1>,
    pub child_specs: Vec<TaskSpecSnapshotV1>,
    pub children: Vec<RuntimeAwaitManyChildSnapshotV1>,
    pub limit: NonZeroU32,
    pub launch_cursor: u32,
    pub in_flight: u32,
    pub outputs: Vec<Option<RuntimePayloadSnapshotV1>>,
    pub publication_cursor: TaskPublicationCursorSnapshotV1,
    pub terminal: Option<RuntimeAwaitManyTerminalSnapshotV1>,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: all source-indexed vectors have source_count length; children keyed by exact source index and ordered ascending; launch_cursor <= source_count; in_flight equals active child count and <= limit; outputs only for Ready child rows; terminal only after precedence rules.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeAwaitManyChildSnapshotV1`

```rust
pub struct RuntimeAwaitManyChildSnapshotV1 {
    pub version: RuntimeAwaitManyChildSnapshotVersion,
    pub source_index: u32,
    pub observer: Option<TaskObserverId>,
    pub handle: Option<RuntimeNeedHandleSnapshotV1>,
    pub status: RuntimeAwaitManyChildStatusSnapshotV1,
    pub last_cursor: Option<TaskPublicationCursorSnapshotV1>,
}
```
Key: `source_index`.  
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: handle and observer absent only before launch; Ready status has corresponding aggregate output; source index is unique and in range.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeAwaitManyChildStatusSnapshotV1`

```rust
pub enum RuntimeAwaitManyChildStatusSnapshotV1 {
    NotLaunched,
    Waiting,
    Ready,
    InfrastructureFailed(RuntimeTaskFailureSnapshotV1),
    Cancelled,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeAwaitManyTerminalSnapshotV1`

```rust
pub enum RuntimeAwaitManyTerminalSnapshotV1 {
    Ready,
    InfrastructureFailed(RuntimeTaskFailureSnapshotV1),
    Cancelled,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeTimeoutNeedSnapshotV1`

```rust
pub struct RuntimeTimeoutNeedSnapshotV1 {
    pub version: RuntimeTimeoutNeedSnapshotVersion,
    pub output: TaskCorrelationSnapshotV1,
    pub source: RuntimeNeedHandleSnapshotV1,
    pub requested_limit: LogicalDurationSnapshotV1,
    pub remaining: LogicalDurationSnapshotV1,
    pub phase: RuntimeTimeoutPhaseSnapshotV1,
    pub source_observer: Option<TaskObserverId>,
    pub source_cursor: Option<TaskPublicationCursorSnapshotV1>,
    pub publication_cursor: TaskPublicationCursorSnapshotV1,
    pub terminal: Option<RuntimeTimeoutTerminalSnapshotV1>,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: remaining <= requested_limit; NotStarted has no observer/source cursor/terminal; Waiting has observer and no terminal; Resolved has terminal; output/source identities and payload types validate.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeTimeoutPhaseSnapshotV1`

```rust
pub enum RuntimeTimeoutPhaseSnapshotV1 {
    NotStarted,
    Waiting,
    Resolved,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeTimeoutTerminalSnapshotV1`

```rust
pub enum RuntimeTimeoutTerminalSnapshotV1 {
    SourceReady(RuntimePayloadSnapshotV1),
    SourceInfrastructureFailed(RuntimeTaskFailureSnapshotV1),
    Expired,
    Cancelled,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `NeedCellSnapshotV1`

```rust
pub struct NeedCellSnapshotV1 {
    pub version: NeedCellSnapshotVersion,
    pub need_id: NeedId,
    pub producer: NeedProducerInstanceKey,
    pub outcome: TaskOutcomeContractSnapshotV1,
    pub state: NeedStateSnapshotV1,
    pub last_publication: Option<TaskPublicationCursorSnapshotV1>,
    pub observers: Vec<TaskObserverId>,
}
```
Key: `need_id`.  
Encoding order: observers ascending ObserverId.  
Bound: RuntimeSnapshotLimits owner.
Invariants: NeedId rederives from producer/policy/ordinal through owning launch; observer ids unique and reverse links exact; terminal state agrees with launch lifecycle.
Unknown/duplicate fields and trailing bytes reject.

### `NeedStateSnapshotV1`

```rust
pub enum NeedStateSnapshotV1 {
    NotStarted,
    Pending(ProgressSnapshotV1),
    Ready {
        outcome: RuntimeNeedOutcomeSnapshotV1,
    },
    Cancelled,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeNeedOutcomeSnapshotV1`

```rust
pub enum RuntimeNeedOutcomeSnapshotV1 {
    Payload(RuntimePayloadSnapshotV1),
    InfrastructureFailure(RuntimeTaskFailureSnapshotV1),
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeTaskFailureSnapshotV1`

```rust
pub struct RuntimeTaskFailureSnapshotV1 {
    pub version: RuntimeTaskFailureSnapshotVersion,
    pub kind: RuntimeTaskFailureKind,
    pub diagnostic: BoundedUtf8,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: closed failure kind; diagnostic bounded and nonidentity.
Unknown/duplicate fields and trailing bytes reject.

### `TaskObserverSnapshotV1`

```rust
pub struct TaskObserverSnapshotV1 {
    pub version: TaskObserverSnapshotVersion,
    pub observer_id: TaskObserverId,
    pub generation: GenerationId,
    pub need_id: NeedId,
    pub kind: TaskObserverKindSnapshotV1,
    pub last_seen: Option<TaskPublicationCursorSnapshotV1>,
    pub active: bool,
}
```
Key: `observer_id`.  
Encoding order: ObserverId byte order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: generation equals active use generation; need exists; reverse Need observer list contains id; inactive observer cannot advance.
Unknown/duplicate fields and trailing bytes reject.

### `TaskObserverKindSnapshotV1`

```rust
pub enum TaskObserverKindSnapshotV1 {
    Await {
        fiber: RuntimeFiberId,
    },
    AwaitManyChild {
        aggregate_task: TaskId,
        source_index: u32,
    },
    TimeoutSource {
        timeout_task: TaskId,
    },
    ViewMatch {
        view_instance: ViewInstanceIdProjection,
        site: ViewMatchSiteIdProjection,
    },
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeNeedHandleSnapshotV1`

```rust
pub struct RuntimeNeedHandleSnapshotV1 {
    pub version: RuntimeNeedHandleSnapshotVersion,
    pub correlation: TaskCorrelationSnapshotV1,
    pub producer: NeedProducerSpecSnapshotV1,
    pub outcome: TaskOutcomeContractSnapshotV1,
    pub origin: NeedHandleOriginSnapshotV1,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: semantic Eq/Hash/Ord remains NeedId-only; full structure validates at construction/restore/use boundaries; ordinary use generation must match active scheduler generation.
Unknown/duplicate fields and trailing bytes reject.

### `NeedHandleOriginSnapshotV1`

```rust
pub struct NeedHandleOriginSnapshotV1 {
    pub version: NeedHandleOriginSnapshotVersion,
    pub accepted_site: Option<StableProducerSiteProjection>,
    pub debug_label: Option<BoundedUtf8>,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: diagnostic/provenance only and excluded from canonical RuntimeValue identity.
Unknown/duplicate fields and trailing bytes reject.

### `TaskPublicationCursorSnapshotV1`

```rust
pub struct TaskPublicationCursorSnapshotV1 {
    pub version: TaskPublicationCursorSnapshotVersion,
    pub logical_epoch: u64,
    pub sequence: u64,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: monotonic per task; gaps rejected according to replay policy.
Unknown/duplicate fields and trailing bytes reject.

### `TaskEventSnapshotV1`

```rust
pub struct TaskEventSnapshotV1 {
    pub version: TaskEventSnapshotVersion,
    pub correlation: TaskCorrelationSnapshotV1,
    pub cursor: TaskPublicationCursorSnapshotV1,
    pub kind: TaskEventKindSnapshotV1,
    pub digest: TaskEventDigest,
}
```
Encoding order: fields above.  
Bound: RuntimeSnapshotLimits owner.
Invariants: digest recomputed before state transition; event ordering key is generation, logical_epoch, sequence, task_id.
Unknown/duplicate fields and trailing bytes reject.

### `TaskEventKindSnapshotV1`

```rust
pub enum TaskEventKindSnapshotV1 {
    Progress(ProgressSnapshotV1),
    Ready(RuntimePayloadSnapshotV1),
    InfrastructureFailure(RuntimeTaskFailureSnapshotV1),
    Cancelled,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `TaskReplayStateSnapshotV1`

```rust
pub struct TaskReplayStateSnapshotV1 {
    pub version: TaskReplayStateSnapshotVersion,
    pub last_applied: Vec<TaskReplayCursorSnapshotV1>,
    pub accepted_event_digests: Vec<TaskReplayDigestSnapshotV1>,
}
```
Encoding order: both vectors ascending TaskId.  
Bound: RuntimeSnapshotLimits owner.
Invariants: keys unique; cursor/digest joins exact.
Unknown/duplicate fields and trailing bytes reject.

### `TaskReplayCursorSnapshotV1`

```rust
pub struct TaskReplayCursorSnapshotV1 {
    pub version: TaskReplayCursorSnapshotVersion,
    pub task_id: TaskId,
    pub cursor: TaskPublicationCursorSnapshotV1,
}
```
Key: `task_id`.  
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `TaskReplayDigestSnapshotV1`

```rust
pub struct TaskReplayDigestSnapshotV1 {
    pub version: TaskReplayDigestSnapshotVersion,
    pub task_id: TaskId,
    pub cursor: TaskPublicationCursorSnapshotV1,
    pub digest: TaskEventDigest,
}
```
Key: `task_id, cursor`.  
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `TaskReplayEnvelopeV1`

```rust
pub struct TaskReplayEnvelopeV1 {
    pub version: TaskReplayEnvelopeVersion,
    pub generation: GenerationId,
    pub events: Vec<TaskEventSnapshotV1>,
}
```
Encoding order: events normalized by event ordering key.  
Bound: RuntimeSnapshotLimits owner.
Invariants: generation matches every event; duplicates only allowed if exact digest matches.
Unknown/duplicate fields and trailing bytes reject.

### `ReplacementStateSnapshotV1`

```rust
pub enum ReplacementStateSnapshotV1 {
    Idle,
    Validated(ReplacementPlanSnapshotV1),
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: Prepared adapter tokens are never persistable; snapshot fails while adapter prepare/commit is in flight.
Unknown/duplicate fields and trailing bytes reject.

### `ReplacementPlanSnapshotV1`

```rust
pub struct ReplacementPlanSnapshotV1 {
    pub version: ReplacementPlanSnapshotVersion,
    pub from_generation: GenerationId,
    pub to_generation: GenerationId,
    pub view_mappings: Vec<ReplacementViewMappingSnapshotV1>,
    pub task_mappings: Vec<ReplacementTaskMappingSnapshotV1>,
}
```
Encoding order: view mappings by old site; task mappings by old TaskId.  
Bound: RuntimeSnapshotLimits owner.
Invariants: to_generation differs from from_generation; mappings unique; NeedId and ordinal preserved; TaskKey/TaskId rederived for to_generation.
Unknown/duplicate fields and trailing bytes reject.

### `ReplacementViewMappingSnapshotV1`

```rust
pub struct ReplacementViewMappingSnapshotV1 {
    pub version: ReplacementViewMappingSnapshotVersion,
    pub old_program: ViewProgramIdProjection,
    pub old_revision: AcceptedViewProgramRevisionProjection,
    pub old_site: ViewMatchSiteIdProjection,
    pub new_program: ViewProgramIdProjection,
    pub new_revision: AcceptedViewProgramRevisionProjection,
    pub new_site: ViewMatchSiteIdProjection,
}
```
Key: `old_program, old_site`.  
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `ReplacementTaskMappingSnapshotV1`

```rust
pub struct ReplacementTaskMappingSnapshotV1 {
    pub version: ReplacementTaskMappingSnapshotVersion,
    pub old: TaskCorrelationSnapshotV1,
    pub new: TaskCorrelationSnapshotV1,
    pub need_handle: RuntimeNeedHandleSnapshotV1,
}
```
Key: `old.task_id`.  
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: NeedId, producer, policy, ordinal equal old/new; generation, TaskKey and TaskId change consistently; handle rebound only by explicit replacement.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimePayloadSnapshotV1`

```rust
pub struct RuntimePayloadSnapshotV1 {
    pub version: RuntimePayloadSnapshotVersion,
    pub value: RuntimeValueSnapshotV1,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeValueSnapshotV1`

```rust
pub enum RuntimeValueSnapshotV1 {
    Unit,
    Bool(bool),
    Int(RuntimeIntSnapshotV1),
    UInt(RuntimeUIntSnapshotV1),
    F32 {
        bits: u32,
    },
    F64 {
        bits: u64,
    },
    Matrix(RuntimeMatrixSnapshotV1),
    Tensor(RuntimeTensorSnapshotV1),
    String(BoundedUtf8),
    Char {
        scalar: u32,
    },
    Duration(LogicalDurationSnapshotV1),
    Progress(ProgressSnapshotV1),
    Range(RuntimeRangeSnapshotV1),
    Iterator(RuntimeIteratorSnapshotV1),
    EntityRef(BoundedUtf8),
    Tuple {
        items: Vec<RuntimeValueSnapshotV1>,
    },
    Seq(RuntimeSeqSnapshotV1),
    Record {
        fields: Vec<RuntimeFieldSnapshotV1>,
    },
    NominalRecord(RuntimeNominalRecordSnapshotV1),
    Opaque(RuntimeOpaqueValueSnapshotV1),
    Reduction(RuntimeReductionSnapshotV1),
    Agent(RuntimeAgentSnapshotV1),
    Function(RuntimeFunctionSnapshotV1),
    Variant(RuntimeVariantSnapshotV1),
    NeedHandle(RuntimeNeedHandleSnapshotV1),
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: exhaustive over final public RuntimeValue in Cut 5; NeedHandle is published only in the atomic public switch.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeCheckedTypeSnapshotV1`

```rust
pub struct RuntimeCheckedTypeSnapshotV1 {
    pub version: RuntimeCheckedTypeSnapshotVersion,
    pub semantic_digest: RuntimeTypeSemanticDigest,
    pub projection: RuntimeCheckedTypeProjectionV1,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Invariants: projection encodes the closed runtime checked-type algebra; digest is recomputed.
Unknown/duplicate fields and trailing bytes reject.

### `ProgressSnapshotV1`

```rust
pub struct ProgressSnapshotV1 {
    pub ratio_bits: u32,
    pub label: Option<BoundedUtf8>,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `LogicalDurationSnapshotV1`

```rust
pub struct LogicalDurationSnapshotV1 {
    pub nanoseconds: u128,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeIntSnapshotV1`

```rust
pub struct RuntimeIntSnapshotV1 {
    pub width: RuntimeSignedIntWidth,
    pub bits: u128,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeUIntSnapshotV1`

```rust
pub struct RuntimeUIntSnapshotV1 {
    pub width: RuntimeUnsignedIntWidth,
    pub bits: u128,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeMatrixSnapshotV1`

```rust
pub struct RuntimeMatrixSnapshotV1 {
    pub kind: RuntimeMatrixKind,
    pub dimensions: Vec<u32>,
    pub scalar_bits: BoundedBytes,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeTensorSnapshotV1`

```rust
pub struct RuntimeTensorSnapshotV1 {
    pub kind: RuntimeTensorKind,
    pub shape: Vec<u32>,
    pub scalar_bits: BoundedBytes,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeRangeSnapshotV1`

```rust
pub struct RuntimeRangeSnapshotV1 {
    pub start: Option<RuntimeValueSnapshotV1>,
    pub end: Option<RuntimeValueSnapshotV1>,
    pub inclusive: bool,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeIteratorSnapshotV1`

```rust
pub struct RuntimeIteratorSnapshotV1 {
    pub source: RuntimeValueSnapshotV1,
    pub cursor: u64,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeSeqSnapshotV1`

```rust
pub struct RuntimeSeqSnapshotV1 {
    pub kind: RuntimeSeqKind,
    pub items: Vec<RuntimeValueSnapshotV1>,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeFieldSnapshotV1`

```rust
pub struct RuntimeFieldSnapshotV1 {
    pub field: AcceptedFieldIdentityProjection,
    pub value: RuntimeValueSnapshotV1,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeNominalRecordSnapshotV1`

```rust
pub struct RuntimeNominalRecordSnapshotV1 {
    pub owner: RuntimeNominalTypeId,
    pub layout: TypeLayoutHash,
    pub fields: Vec<RuntimeFieldSnapshotV1>,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeOpaqueValueSnapshotV1`

```rust
pub struct RuntimeOpaqueValueSnapshotV1 {
    pub producer: RuntimeOpaqueTypeProducerId,
    pub semantic_identity: AcceptedNominalSemanticIdentity,
    pub class: RuntimeOpaqueValueClass,
    pub persistence: RuntimeOpaquePersistence,
    pub payload: BoundedBytes,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeReductionSnapshotV1`

```rust
pub struct RuntimeReductionSnapshotV1 {
    pub kind: RuntimeReductionKind,
    pub value: Option<RuntimeValueSnapshotV1>,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeAgentSnapshotV1`

```rust
pub struct RuntimeAgentSnapshotV1 {
    pub variant: RuntimeAgentValueProjectionV1,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeFunctionSnapshotV1`

```rust
pub struct RuntimeFunctionSnapshotV1 {
    pub callable: RuntimeCallableId,
    pub contract: CallableContractHash,
    pub captures: Vec<RuntimeValueSnapshotV1>,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.

### `RuntimeVariantSnapshotV1`

```rust
pub struct RuntimeVariantSnapshotV1 {
    pub owner: RuntimeVariantIdentity,
    pub case_ordinal: u32,
    pub payload: Option<RuntimeValueSnapshotV1>,
}
```
Encoding order: declaration order.  
Bound: RuntimeSnapshotLimits owner.
Unknown/duplicate fields and trailing bytes reject.
