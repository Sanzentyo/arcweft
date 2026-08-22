# Exact Rust-shaped schemas

The following shapes are normative changed/new seams. Existing accepted parent
types keep their fields and behavior unless this document explicitly evolves
them. Ellipses are used only for unchanged private fields already specified by
the accepted parent; they are not permission for an implementation-time
semantic choice.

## 1. Core adapter and driver protocols

`TaskLaunchAdapter` and every crossing batch remain in
`arcweft-core::task`. This preserves the current
`arcweft-runtime-scheduler -> arcweft-core` dependency and lets
`arcweft-host-adapter` implement the protocol without a reverse edge.

```rust
// crates/arcweft-core/src/task.rs

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
    ) -> Result<
        PreparedLaunchBatch<Self::PreparedLaunchToken>,
        Self::PrepareLaunchError,
    >;

    fn commit_launch(
        &mut self,
        prepared: PreparedLaunchBatch<Self::PreparedLaunchToken>,
    );

    fn rollback_launch(
        &mut self,
        prepared: PreparedLaunchBatch<Self::PreparedLaunchToken>,
    );

    fn prepare_restore(
        &mut self,
        batch: HostTaskRestoreBatch,
    ) -> Result<
        PreparedRestoreBatch<Self::PreparedRestoreToken>,
        Self::PrepareRestoreError,
    >;

    fn commit_restore(
        &mut self,
        prepared: PreparedRestoreBatch<Self::PreparedRestoreToken>,
    );

    fn rollback_restore(
        &mut self,
        prepared: PreparedRestoreBatch<Self::PreparedRestoreToken>,
    );

    fn prepare_rebind(
        &mut self,
        batch: HostTaskRebindBatch,
    ) -> Result<
        PreparedRebindBatch<Self::PreparedRebindToken>,
        Self::PrepareRebindError,
    >;

    fn commit_rebind(
        &mut self,
        prepared: PreparedRebindBatch<Self::PreparedRebindToken>,
    );

    fn rollback_rebind(
        &mut self,
        prepared: PreparedRebindBatch<Self::PreparedRebindToken>,
    );

    fn prepare_cancel(
        &mut self,
        batch: HostTaskCancelBatch,
    ) -> Result<
        PreparedCancelBatch<Self::PreparedCancelToken>,
        Self::PrepareCancelError,
    >;

    fn commit_cancel(
        &mut self,
        prepared: PreparedCancelBatch<Self::PreparedCancelToken>,
    );

    fn rollback_cancel(
        &mut self,
        prepared: PreparedCancelBatch<Self::PreparedCancelToken>,
    );
}
```

The current `TaskHost` is replaced in place by this narrow boundary. Its error
type may preserve host-specific diagnostics; it carries no adapter or token
type and does not make `BundleSession` generic.

```rust
// crates/arcweft-core/src/task.rs

pub trait TaskHost {
    type Error;

    fn ensure_task(
        &mut self,
        authority: &TaskValidationAuthority<'_>,
        spec: TaskSpec,
        observer: Option<TaskObserverKind>,
    ) -> Result<RuntimeNeedHandle, Self::Error>;

    fn observe_task(
        &mut self,
        handle: &RuntimeNeedHandle,
        kind: TaskObserverKind,
    ) -> Result<TaskObserverId, Self::Error>;

    fn cancel_tasks(
        &mut self,
        authority: &TaskValidationAuthority<'_>,
        request: RuntimeTaskCancelRequest,
    ) -> Result<RuntimeTaskCancelReceipt, Self::Error>;

    /// Host composition gathers concrete-adapter completions internally and
    /// advances its scheduler; the driver never reads an adapter directly.
    fn step_tasks(
        &mut self,
        input: TaskHostStepInput,
    ) -> Result<TaskHostStepOutput, Self::Error>;

    fn poll_frame(&mut self, budget: SchedulerBudget) -> Box<[TaskEvent]>;
}

pub struct TaskHostStepInput {
    logical_epoch: LogicalEpoch,
    elapsed: LogicalDuration,
    budget: SchedulerBudget,
}

impl TaskHostStepInput {
    pub const fn new(
        logical_epoch: LogicalEpoch,
        elapsed: LogicalDuration,
        budget: SchedulerBudget,
    ) -> Self;

    pub const fn logical_epoch(&self) -> LogicalEpoch;
    pub const fn elapsed(&self) -> LogicalDuration;
    pub const fn budget(&self) -> SchedulerBudget;
}

pub struct TaskHostStepOutput {
    accepted_adapter_events: u32,
    advanced_runtime_tasks: u32,
}

impl TaskHostStepOutput {
    pub const fn accepted_adapter_events(&self) -> u32;
    pub const fn advanced_runtime_tasks(&self) -> u32;
}
```

The concrete host gathers adapter completions, converts them to core
`TaskEvent` rows, and constructs the scheduler-only input below. Its event
constructor enforces maintained order and duplicate rules before scheduler
mutation. There is no unchecked public field literal.

## 2. Host-owned generic scheduler

```rust
// crates/arcweft-runtime-scheduler/src/lib.rs

pub struct RuntimeTaskScheduler<A: TaskLaunchAdapter> {
    adapter: A,
    journal: RuntimeGenerationJournal,
    runtime: SchedulerRuntimeState,
    config: RuntimeTaskSchedulerConfig,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeTaskSchedulerConfig {
    default_budget: SchedulerBudget,
}

impl RuntimeTaskSchedulerConfig {
    pub const fn new(default_budget: SchedulerBudget) -> Self;

    pub const fn default_budget(self) -> SchedulerBudget;
}

struct SchedulerRuntimeState {
    tasks: BTreeMap<TaskId, RuntimeTask>,
    aggregates: BTreeMap<TaskId, RuntimeAwaitManyAggregateTask>,
    timeouts: BTreeMap<TaskId, RuntimeTimeoutTask>,
    pending_events: Vec<TaskEvent>,
}

struct SchedulerRuntimeAfterImage {
    state: SchedulerRuntimeState,
}

impl<A: TaskLaunchAdapter> RuntimeTaskScheduler<A> {
    pub fn new(
        generation: GenerationId,
        adapter: A,
        config: RuntimeTaskSchedulerConfig,
    ) -> Self;

    pub const fn generation(&self) -> GenerationId;
    pub const fn revision(&self) -> u64;

    pub fn ensure_task(
        &mut self,
        authority: &TaskValidationAuthority<'_>,
        spec: TaskSpec,
        observer: Option<TaskObserverKind>,
    ) -> Result<
        RuntimeNeedHandle,
        RuntimeTaskEnsureError<A::PrepareLaunchError>,
    >;

    pub fn observe_task(
        &mut self,
        handle: &RuntimeNeedHandle,
        kind: TaskObserverKind,
    ) -> Result<TaskObserverId, RuntimeTaskObserveError>;

    pub fn step(
        &mut self,
        input: RuntimeTaskSchedulerStepInput,
    ) -> Result<RuntimeTaskSchedulerStepOutput, RuntimeTaskStepError>;

    pub fn poll_frame(&mut self, budget: SchedulerBudget) -> Box<[TaskEvent]>;

    pub fn snapshot(
        &self,
        authority: &RuntimeSnapshotAuthority<'_>,
        limits: RuntimeSnapshotLimits,
    ) -> Result<RuntimeSchedulerSnapshotV1, RuntimeTaskSnapshotError>;

    pub fn prepare_restore<'scheduler>(
        &'scheduler mut self,
        decoded: DecodedRuntimeTaskSnapshotV1,
        authority: &RuntimeSnapshotAuthority<'_>,
        limits: RuntimeSnapshotLimits,
    ) -> Result<
        PreparedRuntimeTaskRestore<'scheduler, A>,
        RuntimeTaskRestorePrepareError<A::PrepareRestoreError>,
    >;

    pub fn apply_restore(
        prepared: PreparedRuntimeTaskRestore<'_, A>,
    ) -> Result<AppliedRuntimeTaskRestore, JournalApplyError>;

    pub fn prepare_rebind<'scheduler>(
        &'scheduler mut self,
        request: RuntimeTaskRebindRequest<'_>,
    ) -> Result<
        PreparedRuntimeTaskRebind<'scheduler, A>,
        RuntimeTaskRebindPrepareError<A::PrepareRebindError>,
    >;

    pub fn apply_rebind(
        prepared: PreparedRuntimeTaskRebind<'_, A>,
    ) -> Result<AppliedRuntimeTaskRebind, JournalApplyError>;

    pub fn prepare_cancel<'scheduler>(
        &'scheduler mut self,
        request: RuntimeTaskCancelRequest,
    ) -> Result<
        PreparedRuntimeTaskCancel<'scheduler, A>,
        RuntimeTaskCancelPrepareError<A::PrepareCancelError>,
    >;

    pub fn apply_cancel(
        prepared: PreparedRuntimeTaskCancel<'_, A>,
    ) -> Result<RuntimeTaskCancelReceipt, JournalApplyError>;

    fn apply_runtime_after_image(&mut self, after: SchedulerRuntimeAfterImage);
}

pub struct RuntimeTaskSchedulerStepInput {
    logical_epoch: LogicalEpoch,
    elapsed: LogicalDuration,
    adapter_events: Box<[TaskEvent]>,
    budget: SchedulerBudget,
}

impl RuntimeTaskSchedulerStepInput {
    pub fn try_new(
        logical_epoch: LogicalEpoch,
        elapsed: LogicalDuration,
        adapter_events: Box<[TaskEvent]>,
        budget: SchedulerBudget,
    ) -> Result<Self, TaskEventAdmissionError>;
}

pub struct RuntimeTaskSchedulerStepOutput {
    accepted_adapter_events: u32,
    advanced_runtime_tasks: u32,
}

impl RuntimeTaskSchedulerStepOutput {
    pub const fn accepted_adapter_events(&self) -> u32;
    pub const fn advanced_runtime_tasks(&self) -> u32;
}
```

`step` validates and ingests adapter completions, advances internal runtime
tasks, and enqueues every normalized observer event into
`SchedulerRuntimeState::pending_events`. It never returns or drains an event.
`poll_frame` is the sole event drain and removes at most the selected budget in
maintained `(logical_epoch, task_id, sequence)` order. There is exactly one
event queue.

There is intentionally no `adapter_mut`, journal-row constructor, runtime-map
getter, pending-transaction slot, global coordinator ID, or persistence
parameter. An adapter is recovered only by consuming a fully shut-down
scheduler through a separately validated host-composition teardown; that
teardown is not a runtime task transaction API.

## 3. Untrusted decoded snapshot

Core owns the private Wire codec and the untrusted DTO. The existing
`RuntimeSchedulerSnapshotV1` is evolved in place; there is no V2 type or old
reader.

```rust
// crates/arcweft-core/src/task/snapshot.rs (a child of the task owner)

pub struct DecodedRuntimeTaskSnapshotV1 {
    version: RuntimeSnapshotVersion,
    snapshot: RuntimeSchedulerSnapshotV1,
}

impl DecodedRuntimeTaskSnapshotV1 {
    pub fn decode(
        bytes: &[u8],
        limits: RuntimeSnapshotLimits,
    ) -> Result<Self, RuntimeTaskSnapshotDecodeError>;

    pub const fn version(&self) -> RuntimeSnapshotVersion;

    // Only core transaction/scheduler preparation can consume rows. There is
    // no public row/handle/value accessor.
    pub(crate) fn into_snapshot(self) -> RuntimeSchedulerSnapshotV1;
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeTaskSnapshotDecodeError {
    #[error("runtime task snapshot exceeds the outer byte limit")]
    ByteLimit,
    #[error("runtime task snapshot version must be 1, found {found}")]
    Version { found: u64 },
    #[error("runtime task snapshot contains a noncanonical varint")]
    NonCanonicalVarint,
    #[error("runtime task snapshot contains an unknown field or tag")]
    UnknownField,
    #[error("runtime task snapshot contains a duplicate field")]
    DuplicateField,
    #[error("runtime task snapshot contains trailing bytes")]
    TrailingBytes,
    #[error("runtime task snapshot work limit exceeded for {kind:?}")]
    WorkLimit { kind: RuntimeSnapshotLimitKind },
    #[error("runtime task snapshot integer is out of range")]
    IntegerRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSnapshotLimitKind {
    Bytes,
    Rows,
    Nodes,
    Depth,
    TranscriptBytes,
    Fields,
    Observers,
    Events,
    CrossReferences,
}
```

All counts and ordinary private Wire integers use the existing shortest
base-128 varint owner. Fixed-width little-endian integers appear only in
semantic hash transcripts already assigned that grammar; they are not Wire.

## 4. Prepared guards and private transactions

```rust
// crates/arcweft-runtime-scheduler/src/transaction.rs

#[must_use = "apply the prepared restore or let it roll back before continuing"]
pub struct PreparedRuntimeTaskRestore<'scheduler, A: TaskLaunchAdapter> {
    scheduler: &'scheduler mut RuntimeTaskScheduler<A>,
    transaction: Option<PreparedRestoreTransaction<A::PreparedRestoreToken>>,
}

struct PreparedRestoreTransaction<T> {
    journal: SealedJournalAfterImage,
    runtime: SchedulerRuntimeAfterImage,
    prepared_host: Vec<PreparedRestoreBatch<T>>,
}

#[must_use = "apply the prepared rebind or let it roll back before continuing"]
pub struct PreparedRuntimeTaskRebind<'scheduler, A: TaskLaunchAdapter> {
    scheduler: &'scheduler mut RuntimeTaskScheduler<A>,
    transaction: Option<PreparedRebindTransaction<A::PreparedRebindToken>>,
}

struct PreparedRebindTransaction<T> {
    journal: SealedJournalAfterImage,
    runtime: SchedulerRuntimeAfterImage,
    prepared_host: Vec<PreparedRebindBatch<T>>,
}

#[must_use = "apply the prepared cancellation or let it roll back before continuing"]
pub struct PreparedRuntimeTaskCancel<'scheduler, A: TaskLaunchAdapter> {
    scheduler: &'scheduler mut RuntimeTaskScheduler<A>,
    transaction: Option<PreparedCancelTransaction<A::PreparedCancelToken>>,
}

struct PreparedCancelTransaction<T> {
    journal: SealedJournalAfterImage,
    runtime: SchedulerRuntimeAfterImage,
    prepared_host: Vec<PreparedCancelBatch<T>>,
}
```

Each public guard is non-`Clone`, non-`Copy`, nonserialized, and private-field.
Its `Drop` takes the transaction when present and calls the operation-matched
adapter rollback in reverse order. `apply_*` takes the transaction first, so
the later `Drop` is a no-op. The guard retains `&mut RuntimeTaskScheduler<A>`;
another scheduler operation cannot interleave between prepare and apply.

The same private pattern is used transiently inside `ensure_task` with
`PreparedLaunchBatch<A::PreparedLaunchToken>`. It is not public because ensure
is one driver operation, not a persistence phase.

## 5. Applied core result and exposure

`SealedJournalAfterImage` stores only private construction inputs plus
preallocated backing storage. It contains no `RuntimeNeedHandle`, accepted
receipt, restored `RuntimeValue`, or `AppliedRuntimeTaskRestore`. After the
generation/revision precheck succeeds, core constructs the applied objects
during the successful journal swap without allocation or another failure
edge. No handle or receipt is reconstructed by the scheduler.

```rust
// crates/arcweft-core/src/task.rs

pub struct AppliedRuntimeTaskRestore {
    receipt: RuntimeTaskRestoreReceipt,
    handles: Box<[RuntimeNeedHandle]>,
    values: Box<[RuntimeValue]>,
}

impl AppliedRuntimeTaskRestore {
    pub const fn receipt(&self) -> &RuntimeTaskRestoreReceipt;
    pub fn handles(&self) -> &[RuntimeNeedHandle];
    pub fn values(&self) -> &[RuntimeValue];

    pub fn into_parts(
        self,
    ) -> (
        RuntimeTaskRestoreReceipt,
        Box<[RuntimeNeedHandle]>,
        Box<[RuntimeValue]>,
    );
}

pub struct RuntimeTaskRestoreReceipt {
    generation: GenerationId,
    revision: u64,
    restored: Box<[TaskCorrelation]>,
}

impl RuntimeTaskRestoreReceipt {
    pub const fn generation(&self) -> GenerationId;
    pub const fn revision(&self) -> u64;
    pub fn restored(&self) -> &[TaskCorrelation];
}

impl AppliedJournalBatch {
    // Operation kind is fixed in the sealed after-image. This is an
    // infallible move, not validation or receipt construction.
    pub(crate) fn into_restore(self) -> AppliedRuntimeTaskRestore;
}
```

`AppliedRuntimeTaskRestore` is core-owned and has no public/raw constructor.
Core creates it only during successful `apply_after_image`; the scheduler keeps it local
until after runtime swap and adapter commit. Only then does `apply_restore`
return it.

## 6. Rebind and cancellation requests

```rust
// crates/arcweft-core/src/task.rs

pub struct RuntimeTaskRebindRequest<'a> {
    old_authority: &'a TaskValidationAuthority<'a>,
    new_authority: &'a TaskValidationAuthority<'a>,
}

impl<'a> RuntimeTaskRebindRequest<'a> {
    pub fn try_new(
        old_authority: &'a TaskValidationAuthority<'a>,
        new_authority: &'a TaskValidationAuthority<'a>,
    ) -> Result<Self, RuntimeTaskRebindValidationError>;
}

pub struct RuntimeTaskCancelRequest {
    generation: GenerationId,
    correlations: Box<[TaskCorrelation]>,
    reason: TaskCancelReason,
}

impl RuntimeTaskCancelRequest {
    pub fn try_new(
        generation: GenerationId,
        correlations: Box<[TaskCorrelation]>,
        reason: TaskCancelReason,
    ) -> Result<Self, RuntimeTaskCancelValidationError>;
}

pub struct RuntimeTaskCancelReceipt {
    generation: GenerationId,
    revision: u64,
    dispositions: Box<[JournalCancelDisposition]>,
}

impl RuntimeTaskCancelReceipt {
    pub const fn generation(&self) -> GenerationId;
    pub const fn revision(&self) -> u64;
    pub fn dispositions(&self) -> &[JournalCancelDisposition];
}

pub struct AppliedRuntimeTaskRebind {
    old_generation: GenerationId,
    new_generation: GenerationId,
    revision: u64,
    handles: Box<[RuntimeNeedHandle]>,
}

impl AppliedRuntimeTaskRebind {
    pub const fn old_generation(&self) -> GenerationId;
    pub const fn new_generation(&self) -> GenerationId;
    pub const fn revision(&self) -> u64;
    pub fn handles(&self) -> &[RuntimeNeedHandle];
}
```

The request constructors sort nothing. Inputs must already be in canonical
correlation order; out-of-order and duplicate rows reject, preserving the
caller's semantic/source order contract.

## 7. Lifecycle and event types

The current `TaskEventKind::Failed(String)` is replaced rather than retained.
Snapshot event variants evolve in place under version `1`.

```rust
// crates/arcweft-core/src/task.rs

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskLifecycleStage {
    Accepted,
    Running,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskLifecycleTransition {
    LaunchAccepted,
    ExecutionStarted,
    CancellationRequested,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaskEventKind {
    Progress(Progress),
    Ready(RuntimePayload),
    InfrastructureFailure(RuntimeTaskFailure),
    Cancelled,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeNeedCellState {
    Pending,
    Progress {
        cursor: TaskPublicationCursor,
        progress: Progress,
    },
    Ready {
        cursor: TaskPublicationCursor,
        value: RuntimeValue,
    },
    InfrastructureFailure {
        cursor: TaskPublicationCursor,
        failure: RuntimeTaskFailure,
    },
    CancellationRequested,
    Cancelled {
        cursor: TaskPublicationCursor,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaskEventKindSnapshotV1 {
    Progress(Progress),
    Ready(AwbcRuntimeValueSnapshot),
    InfrastructureFailure(RuntimeTaskFailure),
    Cancelled,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeNeedCellStateSnapshotV1 {
    Pending,
    Progress {
        cursor: TaskPublicationCursor,
        progress: Progress,
    },
    Ready {
        cursor: TaskPublicationCursor,
        value: AwbcRuntimeValueSnapshot,
    },
    InfrastructureFailure {
        cursor: TaskPublicationCursor,
        failure: RuntimeTaskFailure,
    },
    CancellationRequested,
    Cancelled {
        cursor: TaskPublicationCursor,
    },
}
```

`TaskJournalRowSnapshotV1` stores `TaskLifecycleStage` separately. Launch
acceptance and execution start never appear in `TaskEventKindSnapshotV1`.

## 8. Error surfaces

```rust
// crates/arcweft-runtime-scheduler/src/error.rs

pub enum TaskEventAdmissionError {
    NonCanonicalOrder { source_index: u32 },
    DuplicateKey { source_index: u32 },
    UnknownTask { source_index: u32 },
    CursorRegression { source_index: u32 },
    InvalidTransition { source_index: u32 },
}

pub enum RuntimeSchedulerAfterImageError {
    TaskLimit,
    AggregateLimit,
    TimeoutLimit,
    EventLimit,
    DuplicateTask { source_index: u32 },
    MissingRuntimeTask { source_index: u32 },
    InvalidRuntimeCrossReference { source_index: u32 },
}

pub enum RuntimeTaskSnapshotAdmissionError {
    TaskSpec {
        source_index: u32,
        source: TaskSpecValidationError,
    },
    RuntimeValue {
        source_index: u32,
        source: AwbcRuntimeValueSnapshotError,
    },
    NeedHandle {
        source_index: u32,
        source: RuntimeNeedHandleConstructionError,
    },
    View {
        source_index: u32,
        source: ViewTaskPlanValidationError,
    },
    Outcome { source_index: u32 },
}

pub enum RuntimeTaskSnapshotError {
    Authority(RuntimeSnapshotAuthorityError),
    NonQuiescent { source_index: u32 },
    IncompleteRestartableRow { source_index: u32 },
    RuntimeValue(AwbcRuntimeValueSnapshotError),
    WorkLimit { kind: RuntimeSnapshotLimitKind },
}

pub enum RuntimeTaskObserveError {
    GenerationMismatch,
    UnknownHandle,
    TerminalObserverRejected,
    Journal(JournalTransactionError),
    Apply(JournalApplyError),
}

pub enum RuntimeTaskStepError {
    Event(TaskEventAdmissionError),
    GenerationMismatch,
    WorkLimit,
    InvalidRuntimeTransition { source_index: u32 },
}

pub enum RuntimeTaskRebindValidationError {
    OldAuthorityGeneration,
    NewAuthorityGeneration,
    SameGeneration,
    ReplacementRejected { source_index: u32 },
}

pub enum RuntimeTaskCancelValidationError {
    Empty,
    GenerationMismatch,
    NonCanonicalOrder { source_index: u32 },
    DuplicateCorrelation { source_index: u32 },
}

pub enum RuntimeTaskRestorePrepareError<E> {
    GenerationMismatch,
    SnapshotAdmission(RuntimeTaskSnapshotAdmissionError),
    DuplicateIdentity { source_index: u32 },
    InvalidCrossReference { source_index: u32 },
    RestartPolicy { source_index: u32 },
    SchedulerAfterImage(RuntimeSchedulerAfterImageError),
    AdapterPrepare { source_index: u32, source: E },
    AdapterReceipt { source_index: u32, source: HostTaskReceiptError },
    Journal(JournalTransactionError),
}

pub enum RuntimeTaskEnsureError<E> {
    Validation(TaskSpecValidationError),
    Journal(JournalTransactionError),
    SchedulerAfterImage(RuntimeSchedulerAfterImageError),
    AdapterPrepare { source_index: u32, source: E },
    AdapterReceipt { source_index: u32, source: HostTaskReceiptError },
    Apply(JournalApplyError),
}

pub enum RuntimeTaskRebindPrepareError<E> {
    Validation(RuntimeTaskRebindValidationError),
    Journal(JournalTransactionError),
    SchedulerAfterImage(RuntimeSchedulerAfterImageError),
    AdapterPrepare { source_index: u32, source: E },
    AdapterReceipt { source_index: u32, source: HostTaskReceiptError },
}

pub enum RuntimeTaskCancelPrepareError<E> {
    Validation(RuntimeTaskCancelValidationError),
    Journal(JournalTransactionError),
    SchedulerAfterImage(RuntimeSchedulerAfterImageError),
    AdapterPrepare { source_index: u32, source: E },
}
```

`apply_restore`, `apply_rebind`, and `apply_cancel` return only
`JournalApplyError`. Adapter commit/rollback have no error type. Exact
first-error precedence is normative in
[TRANSACTION_AND_STATE_PROJECTION.md](TRANSACTION_AND_STATE_PROJECTION.md).

## 9. Runtime-driver step

```rust
// crates/arcweft-runtime-driver/src/session.rs

impl BundleSession {
    pub fn step_with_task_host<H: TaskHost>(
        &mut self,
        host: &mut H,
        authority: &TaskValidationAuthority<'_>,
        clock: RuntimeClockStep,
        input: BundleStepInput,
    ) -> Result<BundleSessionStep, BundleSessionTaskHostError<H::Error>>;
}

pub enum BundleSessionTaskHostError<E> {
    Step(E),
    Ensure { source_index: u32, source: E },
    Observe { source_index: u32, source: E },
    Cancel { source_index: u32, source: E },
}
```

The method is generic; `BundleSession` is not. The old `step_with_clock` task
path, `requested_tasks`, `cancel_scopes`, `HostTaskDispatch`, and
`RuntimeTaskRegistry` do not remain as a fallback overload.

`step_with_task_host` first calls `host.step_tasks`, then calls
`host.poll_frame` exactly once and supplies those drained events to the runtime
step. It never reads events from `TaskHostStepOutput`. Ensure/observe/cancel
calls happen after the runtime step in accepted source order.

## 10. Concrete host composition

The concrete adapter facades own external reservation behavior. Their
completion sources are receive-only endpoints for already typed `TaskEvent`
rows; they are not task identity, lifecycle, or journal authorities.

```rust
// crates/arcweft-host-adapter/src/lib.rs

pub struct RegistryTaskLaunchAdapter {
    registry: HostAdapterRegistry,
    completion_sink: RegistryTaskCompletionSink,
    route_allocators: BTreeMap<HostRouteId, RegistryRouteCapabilityAllocator>,
}

pub struct RegistryTaskCompletionSource {
    receiver: RegistryTaskCompletionReceiver,
}

impl RegistryTaskCompletionSource {
    pub fn drain(&mut self) -> Box<[TaskEvent]>;
}

pub fn registry_task_adapter(
    registry: HostAdapterRegistry,
) -> (RegistryTaskLaunchAdapter, RegistryTaskCompletionSource);

struct RegistryPreparedBatchState {
    base_allocators: Box<[(HostRouteId, RegistryRouteCapabilityAllocator)]>,
    after_allocators: Box<[(HostRouteId, RegistryRouteCapabilityAllocator)]>,
    reservations: Vec<Box<dyn PreparedHostAdapterReservation>>,
}

pub trait PreparedHostAdapterReservation: Debug + Send {
    /// Publishes a reservation which was made infallible by `prepare_*`.
    fn commit(self: Box<Self>);

    /// Releases an unpublished reservation. Implementations must be
    /// idempotent with respect to external work which was not published.
    fn rollback(self: Box<Self>);
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HostAdapterReservationError {
    #[error("the accepted operation is unavailable")]
    OperationUnavailable,
    #[error("the adapter cannot reserve the requested restart contract")]
    RestartContract,
    #[error("the adapter cannot reserve idempotent cancellation")]
    CancellationContract,
    #[error("the adapter reservation limit is exhausted")]
    ReservationLimit,
    #[error("adapter reservation failed with code {code}")]
    Infrastructure {
        code: u32,
        detail_digest: [u8; 32],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RegistryRouteCapabilityAllocator {
    next_launch: NonZeroU64,
    next_cancellation: NonZeroU64,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RegistryTaskPrepareError {
    #[error("host route is absent at source row {source_index}")]
    MissingRoute { source_index: u32 },
    #[error("host route capability space is exhausted at source row {source_index}")]
    CapabilityExhausted { source_index: u32 },
    #[error("host adapter rejected an unpublished reservation at source row {source_index}")]
    Reservation {
        source_index: u32,
        source: HostAdapterReservationError,
    },
    #[error("host receipt is invalid at source row {source_index}")]
    Receipt {
        source_index: u32,
        source: HostTaskReceiptError,
    },
}

// Public because each is a public associated type; fields remain private.
// Every token owns the exact route-allocation after-image and opaque
// unpublished reservations for its operation.
pub struct RegistryPreparedLaunchToken {
    state: RegistryPreparedBatchState,
}
pub struct RegistryPreparedRestoreToken {
    state: RegistryPreparedBatchState,
}
pub struct RegistryPreparedRebindToken {
    state: RegistryPreparedBatchState,
}
pub struct RegistryPreparedCancelToken {
    state: RegistryPreparedBatchState,
}

impl TaskLaunchAdapter for RegistryTaskLaunchAdapter {
    type PreparedLaunchToken = RegistryPreparedLaunchToken;
    type PrepareLaunchError = RegistryTaskPrepareError;
    type PreparedRestoreToken = RegistryPreparedRestoreToken;
    type PrepareRestoreError = RegistryTaskPrepareError;
    type PreparedRebindToken = RegistryPreparedRebindToken;
    type PrepareRebindError = RegistryTaskPrepareError;
    type PreparedCancelToken = RegistryPreparedCancelToken;
    type PrepareCancelError = RegistryTaskPrepareError;

    // Exact twelve methods are the core protocol from section 1.
}

// crates/arcweft-runtime-host/src/native_task.rs

pub struct NativeTaskBridge {
    scheduler: RuntimeTaskScheduler<RegistryTaskLaunchAdapter>,
    completions: RegistryTaskCompletionSource,
    stats: NativeTaskStats,
}

// crates/arcweft-runtime-host/src/bundle_runner/session.rs
// The existing private `BundleRunnerSession::host: NativeTaskBridge` field
// remains the sole native/headless task composition. No HeadlessTaskHost type
// is added.

// crates/arcweft-player-web/src/host.rs

pub struct BrowserTaskLaunchAdapter {
    resources: BrowserTaskResources,
    work_queue: BrowserWorkQueue,
    completion_sink: BrowserTaskCompletionSink,
    route_allocators: BTreeMap<HostRouteId, BrowserRouteCapabilityAllocator>,
}

pub struct BrowserTaskResources {
    allowed_calls: BTreeSet<String>,
    files: BTreeMap<String, Vec<u8>>,
    asset_files: BTreeMap<String, String>,
}

pub struct BrowserTaskCompletionSource {
    receiver: BrowserTaskCompletionReceiver,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BrowserRouteCapabilityAllocator {
    next_launch: NonZeroU64,
    next_cancellation: NonZeroU64,
}

struct BrowserPreparedBatchState {
    base_allocators: Box<[(HostRouteId, BrowserRouteCapabilityAllocator)]>,
    after_allocators: Box<[(HostRouteId, BrowserRouteCapabilityAllocator)]>,
    work: BrowserWorkQueueReservation,
}

struct BrowserWorkQueueReservation {
    operations: Box<[BrowserPreparedOperation]>,
}

struct BrowserWorkQueue {
    batches: VecDeque<Box<[BrowserPreparedOperation]>>,
}

enum BrowserPreparedOperation {
    Launch(HostTaskLaunchRow),
    Restore(HostTaskRestoreRow),
    Rebind(HostTaskRebindRow),
    Cancel(HostTaskCancelRow),
}

pub struct BrowserPreparedLaunchToken {
    state: BrowserPreparedBatchState,
}
pub struct BrowserPreparedRestoreToken {
    state: BrowserPreparedBatchState,
}
pub struct BrowserPreparedRebindToken {
    state: BrowserPreparedBatchState,
}
pub struct BrowserPreparedCancelToken {
    state: BrowserPreparedBatchState,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum BrowserTaskPrepareError {
    #[error("browser host route is absent at source row {source_index}")]
    MissingRoute { source_index: u32 },
    #[error("browser route capability space is exhausted at source row {source_index}")]
    CapabilityExhausted { source_index: u32 },
    #[error("browser work queue capacity cannot be reserved")]
    QueueCapacity,
    #[error("browser operation rejected at source row {source_index}")]
    Operation {
        source_index: u32,
        source: BrowserHostTaskError,
    },
    #[error("browser host receipt is invalid at source row {source_index}")]
    Receipt {
        source_index: u32,
        source: HostTaskReceiptError,
    },
}

impl TaskLaunchAdapter for BrowserTaskLaunchAdapter {
    type PreparedLaunchToken = BrowserPreparedLaunchToken;
    type PrepareLaunchError = BrowserTaskPrepareError;
    type PreparedRestoreToken = BrowserPreparedRestoreToken;
    type PrepareRestoreError = BrowserTaskPrepareError;
    type PreparedRebindToken = BrowserPreparedRebindToken;
    type PrepareRebindError = BrowserTaskPrepareError;
    type PreparedCancelToken = BrowserPreparedCancelToken;
    type PrepareCancelError = BrowserTaskPrepareError;

    // Exact twelve methods are the core protocol from section 1.
}

pub struct BrowserTaskBroker {
    scheduler: RuntimeTaskScheduler<BrowserTaskLaunchAdapter>,
    completions: BrowserTaskCompletionSource,
}
```

Each composition implements `TaskHost`. `step_tasks` drains its sole completion
source, calls `RuntimeTaskSchedulerStepInput::try_new`, then delegates to
`RuntimeTaskScheduler::step`, which only enqueues normalized events and returns
counts. `poll_frame` delegates to the scheduler's sole queue drain.
Ensure/observe/cancel delegate directly and map
the generic scheduler error into the composition's typed public error. The
completion endpoint cannot launch, cancel, restore, or inspect a task.

For the registry implementation, every `prepare_*` compares the live route
allocator with `base_allocators`, reserves capabilities into
`after_allocators`, and constructs all dynamic reservations before returning a
core batch. A partial failure calls `rollback` for already-created
reservations in reverse order and leaves the live allocators unchanged.
`commit_*` first swaps the matching allocator after-image and then commits
reservations in canonical order; both operations are infallible. `rollback_*`
only rolls reservations back in reverse order. The scheduler guard prevents a
second registry transaction from invalidating the recorded base while one is
prepared.

The boxed reservation is confined inside the one concrete
`RegistryPrepared*Token` implementation because `HostAdapterRegistry` is the
existing heterogeneous plugin-dispatch owner. Core and scheduler still retain
the statically known associated type; they never store `Box<dyn Any>`, erase
`A::Prepared*Token`, downcast a token, or accept a registry reservation as the
transaction protocol. Thus this does not create the rejected cross-boundary
trait-object token model.

The browser implementation uses the same rule without a dynamic registry.
`prepare_*` validates the operation, calls `VecDeque::try_reserve(1)` on the
work queue, and owns the complete typed operation batch in that reservation.
It neither executes the operation nor creates a
`TaskEvent`, output `RuntimeValue`, or resource after-image. The scheduler
guard prevents queue pumping or another reservation before consumption.
`commit_*` therefore moves the boxed batch with `push_back` into already
reserved capacity without allocation; ordinary later host pumping executes it
and reports any infrastructure failure through the normal completion path.
`rollback_*` drops the unpublished work reservation and need not mutate the
queue. Browser I/O remains the existing embedded-VFS operation set; this
protocol does not grant network, IndexedDB, or ambient filesystem authority.

## 11. Explicitly absent APIs

The final schema contains none of:

```text
TaskPersistence
TaskRestoreJournal
RestorePreparedRecord
RestoreCommittedRecord
PendingTaskPublication
RuntimeTaskCoordinator
PublishedRuntimeTaskBatch
append_restore_prepared
append_restore_committed
commit_restore(... persistence ...)
legacy snapshot reader
V2/V3 restore or snapshot type
adapter commit Result
```

Outer code may use `arcweft-save` to read/write the complete snapshot DTO. It
does not pass an I/O trait into core or scheduler preparation/application.
