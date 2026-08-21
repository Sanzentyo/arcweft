# Final Rust-shaped schemas

These schemas are normative API shapes. Field privacy may be tightened only
where shown private; owners, field sets, dependency direction, invariants, and
construction paths are fixed. New behavior is implemented on the Arcweft-owned
type itself, not through extension traits or feature-local helper tables.

## 1. Core identity and digest owners

```rust
// crates/arcweft-core/src/task/identity.rs

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct GenerationId(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct NeedProducerInstanceKey([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct NeedId([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct TaskKey([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct TaskId([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct TaskLaunchOrdinal(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct NeedProducerContractDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct TaskPlanSemanticDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct RuntimeTypeSemanticDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum TaskPolicy {
    JoinSameKey = 0,
    AlwaysStart = 1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum NeedProducerFamily {
    StructuredTaskPlan = 0,
    AwbcTaskPlan = 1,
    ViewMatchSubscription = 2,
    AwaitManyBase = 3,
    AwaitManyChild = 4,
    Timeout = 5,
    LineTask = 6,
    HostAdapterTask = 7,
    MakeNeedHandle = 8,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum FixedTaskIdentityError {
    #[error("{kind} must not be all zero")]
    Zero { kind: FixedTaskIdentityKind },
    #[error("task policy tag {0} is not assigned in version 1")]
    UnknownPolicy(u8),
    #[error("Need producer family tag {0} is not assigned in version 1")]
    UnknownProducerFamily(u8),
    #[error("JoinSameKey requires launch ordinal zero")]
    NonZeroJoinOrdinal,
    #[error("AlwaysStart requires a nonzero launch ordinal")]
    ZeroAlwaysStartOrdinal,
    #[error("task identity transcript exceeded its work limit")]
    WorkLimit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixedTaskIdentityKind {
    NeedProducerInstance,
    Need,
    TaskKey,
    Task,
}
```

The fixed identity types deliberately implement neither `Default` nor a public
`ZERO`. Their private wire `Deserialize` implementations call `try_from_bytes`.
Semantic digest types accept the complete hash output and therefore may expose
`from_bytes`; absence is always an `Option`.

```rust
impl GenerationId {
    pub const fn new(value: u64) -> Self;
    pub const fn get(self) -> u64;
}

impl TaskLaunchOrdinal {
    pub const JOIN: Self = Self(0);

    pub const fn get(self) -> u64;
    pub fn try_for_policy(
        policy: TaskPolicy,
        value: u64,
    ) -> Result<Self, FixedTaskIdentityError>;
}

impl TaskPolicy {
    pub const fn semantic_tag(self) -> u8;
    pub const fn from_semantic_tag(value: u8) -> Result<Self, FixedTaskIdentityError>;
}

impl NeedProducerFamily {
    pub const ALL: [Self; 9];
    pub const fn semantic_tag(self) -> u8;
    pub const fn from_semantic_tag(value: u8)
        -> Result<Self, FixedTaskIdentityError>;
}

impl NeedProducerInstanceKey {
    pub fn try_from_bytes(bytes: [u8; 32])
        -> Result<Self, FixedTaskIdentityError>;
    pub const fn as_bytes(&self) -> &[u8; 32];

    pub fn try_for(input: &NeedProducerInstanceInput<'_>)
        -> Result<Self, NeedProducerIdentityError>;
}

impl NeedId {
    pub fn try_from_bytes(bytes: [u8; 32])
        -> Result<Self, FixedTaskIdentityError>;
    pub const fn as_bytes(&self) -> &[u8; 32];

    pub fn try_for(
        producer: NeedProducerInstanceKey,
        policy: TaskPolicy,
        ordinal: TaskLaunchOrdinal,
    ) -> Result<Self, NeedProducerIdentityError>;
}

impl TaskKey {
    pub fn try_from_bytes(bytes: [u8; 32])
        -> Result<Self, FixedTaskIdentityError>;
    pub const fn as_bytes(&self) -> &[u8; 32];

    pub fn try_for(
        generation: GenerationId,
        producer: NeedProducerInstanceKey,
        policy: TaskPolicy,
    ) -> Result<Self, NeedProducerIdentityError>;
}

impl TaskId {
    pub fn try_from_bytes(bytes: [u8; 32])
        -> Result<Self, FixedTaskIdentityError>;
    pub const fn as_bytes(&self) -> &[u8; 32];

    pub fn try_for(
        task_key: TaskKey,
        ordinal: TaskLaunchOrdinal,
    ) -> Result<Self, NeedProducerIdentityError>;
}
```

No free function or extension trait may reproduce these transcripts.

## 2. Producer contract and instance input

```rust
use arcweft_core::entry::{CallableContractHash, RuntimeCallableId, RuntimeValueDigest};
use arcweft_core::task::{HostCapabilityId, RuntimeHostOperationId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NeedProducerContractOwner {
    CheckedCallable {
        callable: RuntimeCallableId,
        contract: CallableContractHash,
    },
    HostOperation {
        capability: HostCapabilityId,
        operation: RuntimeHostOperationId,
        contract: CallableContractHash,
    },
    BuiltinTimeout {
        timeout_contract: NeedTimeoutContractDigest,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeedProducerContractInput {
    pub owner: NeedProducerContractOwner,
}

#[derive(Clone, Copy, Debug)]
pub struct NeedProducerInstanceInput<'a> {
    pub family: NeedProducerFamily,
    pub contract: &'a NeedProducerContractDigest,
    pub plan: &'a TaskPlanSemanticDigest,
    pub producer_site: u32,
    pub payload_type: &'a RuntimeTypeSemanticDigest,
    pub arguments: &'a RuntimeValueDigest,
}

impl NeedProducerContractDigest {
    pub fn for_input(
        input: &NeedProducerContractInput,
    ) -> Result<Self, NeedProducerContractError>;

    pub const fn from_bytes(bytes: [u8; 32]) -> Self;
    pub const fn as_bytes(&self) -> &[u8; 32];
}
```

`RuntimeHostOperationId` is a validated typed accepted-catalog coordinate; it
is not a producer identity and cannot be created from source spelling during
runtime identity construction.

## 3. Final task, correlation, event, and Need schemas

```rust
use arcweft_need::Need;
use arcweft_core::entry::RuntimeValueDigest;
use arcweft_core::value::{Progress, RuntimePayload};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeedProducerInstance {
    key: NeedProducerInstanceKey,
    contract: NeedProducerContractDigest,
    plan: TaskPlanSemanticDigest,
    payload_type: RuntimeTypeSemanticDigest,
    arguments: RuntimeValueDigest,
}

impl NeedProducerInstance {
    pub fn try_new(
        family: NeedProducerFamily,
        contract: NeedProducerContractDigest,
        plan: TaskPlanSemanticDigest,
        producer_site: u32,
        payload_type: RuntimeTypeSemanticDigest,
        arguments: RuntimeValueDigest,
    ) -> Result<Self, NeedProducerIdentityError>;

    pub const fn key(&self) -> NeedProducerInstanceKey;
    pub const fn contract(&self) -> NeedProducerContractDigest;
    pub const fn plan(&self) -> TaskPlanSemanticDigest;
    pub const fn payload_type(&self) -> RuntimeTypeSemanticDigest;
    pub const fn arguments(&self) -> RuntimeValueDigest;
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskSpec {
    pub generation: GenerationId,
    pub producer: NeedProducerInstance,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub policy: TaskPolicy,
    pub outcome: TaskOutcomeContract,
    pub request: HostTaskRequest,
    pub debug_label: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskCorrelation {
    pub generation: GenerationId,
    pub producer: NeedProducerInstanceKey,
    pub producer_contract: NeedProducerContractDigest,
    pub need: NeedId,
    pub task_key: TaskKey,
    pub task_id: TaskId,
    pub launch_ordinal: TaskLaunchOrdinal,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskHandle {
    pub correlation: TaskCorrelation,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskPublicationCursor {
    pub logical_epoch: LogicalEpoch,
    pub sequence: TaskSequence,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskEvent {
    pub correlation: TaskCorrelation,
    pub cursor: TaskPublicationCursor,
    pub kind: TaskEventKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaskEventKind {
    Progress(Progress),
    Ready(RuntimePayload),
    InfrastructureFailure(RuntimeTaskFailure),
    Cancelled,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeNeedState {
    pub correlation: TaskCorrelation,
    pub cursor: Option<TaskPublicationCursor>,
    pub state: Need<RuntimeNeedOutcome>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeNeedOutcome {
    Value(RuntimePayload),
    InfrastructureFailure(RuntimeTaskFailure),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTaskFailure {
    pub kind: RuntimeTaskFailureKind,
    pub diagnostic: BoundedRuntimeDiagnostic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeTaskFailureKind {
    AdapterUnavailable,
    AdapterProtocolViolation,
    WorkerFailure,
    RuntimeInvariant,
    RestoreFailure,
}
```

The `TaskCorrelation` field set is exact. Additional correlation DTOs or
conversion wrappers are forbidden.

`TaskEventKind::Ready` validates through `TaskOutcomeContract` before
publication. A domain error remains an ordinary Result payload. Adapters may
not turn `RuntimeTaskFailure` into a domain `Result::Err`.

## 4. One RuntimeNeedHandle carrier

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeNeedHandle {
    correlation: TaskCorrelation,
    spec: Box<TaskSpec>,
    origin: RuntimeNeedHandleOrigin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeNeedHandleOrigin {
    ReusableJoin,
    AcceptedLaunch,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeNeedHandleError {
    #[error("reusable Need handle requires JoinSameKey")]
    ReusableAlwaysStart,
    #[error("handle correlation does not match its TaskSpec")]
    CorrelationMismatch,
    #[error("handle producer contract does not match its TaskSpec")]
    ProducerContractMismatch,
    #[error("handle identity derivation failed")]
    Identity(#[from] NeedProducerIdentityError),
}

impl RuntimeNeedHandle {
    pub(crate) fn try_reusable_join(
        spec: TaskSpec,
    ) -> Result<Self, RuntimeNeedHandleError>;

    pub(crate) fn try_from_accepted_launch(
        spec: TaskSpec,
        handle: TaskHandle,
    ) -> Result<Self, RuntimeNeedHandleError>;

    pub const fn correlation(&self) -> TaskCorrelation;
    pub const fn need_id(&self) -> NeedId;
    pub const fn outcome(&self) -> &TaskOutcomeContract;

    /// Returns the start specification only for a reusable Join handle.
    pub const fn reusable_spec(&self) -> Option<&TaskSpec>;

    /// Direct Await consumes this concrete correlation and never rederives it.
    pub fn await_target(
        &self,
        observer: NeedObserverKey,
    ) -> AwaitTarget;
}
```

`RuntimeValue` receives one new closed variant in its original enum:

```rust
pub enum RuntimeValue {
    // existing variants remain in their existing owner
    NeedHandle(RuntimeNeedHandle),
}
```

This is not a second carrier. The existing enum's inherent methods are extended
exhaustively: canonical encoding, shape checks, function/opaque traversal,
snapshot admission, display label, and any closed visitor all gain the
`NeedHandle` row.

Canonical runtime-value grammar for this variant is:

```text
u8:20  // existing canonical RuntimeValue owner
NeedId:[u8;32]
```

The private snapshot codec carries `correlation + spec + origin` and validates
that the rederived NeedId, TaskKey, TaskId, contract, generation, policy, and
ordinal exactly agree before constructing the value.

## 5. Await and AwaitMany

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct AwaitTarget {
    pub handle: RuntimeNeedHandle,
    pub observer: NeedObserverKey,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeedProducerTemplate {
    pub family: NeedProducerFamily,
    pub contract: NeedProducerContractDigest,
    pub plan: TaskPlanSemanticDigest,
    pub producer_site: u32,
    pub payload_type: RuntimeTypeSemanticDigest,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub policy: TaskPolicy,
    pub outcome: TaskOutcomeContract,
    pub request: HostTaskRequestTemplate,
    pub debug_label: String,
}

impl NeedProducerTemplate {
    pub fn instantiate(
        &self,
        generation: GenerationId,
        arguments: RuntimeValue,
        request: HostTaskRequest,
        limits: RuntimeValueDigestLimits,
    ) -> Result<TaskSpec, NeedProducerInstantiationError>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct AwaitManyTarget {
    pub source: RuntimeExpr,
    pub item_binding: RuntimeLocalDeclarationId,
    pub limit: u32,
    pub base: NeedProducerTemplate,
    pub child: NeedProducerTemplate,
    pub observer: NeedObserverKey,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FiberAwaitManyInFlight {
    pub generation: GenerationId,
    pub base: RuntimeNeedHandle,
    pub source_len: u32,
    pub next_source_index: u32,
    pub limit: u32,
    pub children: BTreeMap<u32, AwaitManyChildInFlight>,
    pub outputs: Vec<Option<RuntimePayload>>,
    pub last_aggregate_cursor: Option<TaskPublicationCursor>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AwaitManyChildInFlight {
    pub source_index: u32,
    pub handle: RuntimeNeedHandle,
    pub observer: NeedObserverKey,
    pub cursor: Option<TaskPublicationCursor>,
}
```

Normative argument construction:

```rust
fn await_many_base_arguments(
    captured: Vec<RuntimeValue>,
    source_items: Vec<RuntimeValue>,
) -> RuntimeValue {
    RuntimeValue::Tuple(vec![
        RuntimeValue::Tuple(captured),
        RuntimeValue::Tuple(source_items),
    ])
}

fn await_many_child_arguments(
    captured: Vec<RuntimeValue>,
    source_index: u32,
    item: RuntimeValue,
) -> RuntimeValue {
    RuntimeValue::Tuple(vec![
        RuntimeValue::Tuple(captured),
        RuntimeValue::u32(source_index),
        item,
    ])
}
```

These snippets state shape; production must place them as inherent constructors
on the owning AwaitMany plan/input type rather than as free ad hoc helpers.

The child `producer_site` is the accepted child site, not the index. Index is
committed only through the canonical argument tuple. A source length larger
than `u32::MAX` is rejected before any launch or ordinal allocation.

## 6. Timeout

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct NeedTimeoutTarget {
    pub source: RuntimeNeedHandle,
    pub limit: RuntimeValue,
    pub producer: NeedProducerTemplate, // family is Timeout; policy is JoinSameKey
}

impl NeedTimeoutTarget {
    pub fn try_task_spec(
        &self,
        generation: GenerationId,
        limits: RuntimeValueDigestLimits,
    ) -> Result<TaskSpec, NeedTimeoutConstructionError>;
}
```

The inherent constructor hashes exactly:

```rust
RuntimeValue::Tuple(vec![
    RuntimeValue::NeedHandle(self.source.clone()),
    self.limit.clone(),
])
```

It never parses the source `NeedId`, changes the source state, or cancels the
source producer.

## 7. Observer key and journal schemas

The parent observer lifecycle remains one authority. Its final typed key is:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NeedObserverKey {
    Fiber {
        generation: GenerationId,
        fiber: FiberId,
        await_site: RuntimeAwaitSiteId,
    },
    View {
        mount: ViewMountId,
        subscription: ViewNeedSubscriptionId,
    },
    AwaitManyChild {
        generation: GenerationId,
        fiber: FiberId,
        await_site: RuntimeAwaitSiteId,
        source_index: u32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeedObserverState {
    pub key: NeedObserverKey,
    pub correlation: TaskCorrelation,
    pub last_cursor: Option<TaskPublicationCursor>,
    pub queued_invalidation: bool,
    pub status: NeedObserverStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NeedObserverStatus {
    Subscribed,
    Cancelled,
    Detached,
}
```

```rust
#[derive(Clone, Debug)]
pub struct RuntimeTaskJournal {
    generations: BTreeMap<GenerationId, GenerationTaskJournal>,
    limits: RuntimeTaskJournalLimits,
}

#[derive(Clone, Debug)]
pub struct GenerationTaskJournal {
    last_always_start_ordinal:
        BTreeMap<NeedProducerInstanceKey, TaskLaunchOrdinal>,
    groups: BTreeMap<TaskKey, TaskGroupJournalEntry>,
    tasks: BTreeMap<TaskId, TaskJournalEntry>,
    needs: BTreeMap<NeedId, RuntimeNeedState>,
    observers: BTreeMap<NeedObserverKey, NeedObserverState>,
}

#[derive(Clone, Debug)]
pub struct TaskGroupJournalEntry {
    pub task_key: TaskKey,
    pub producer: NeedProducerInstanceKey,
    pub policy: TaskPolicy,
    pub launches: BTreeMap<TaskLaunchOrdinal, TaskId>,
}

#[derive(Clone, Debug)]
pub struct TaskJournalEntry {
    pub correlation: TaskCorrelation,
    pub spec: TaskSpec,
    pub lifecycle: TaskLifecycleState,
    pub last_event: Option<TaskEvent>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskLifecycleState {
    Prepared,
    Accepted,
    Running,
    Terminal,
    Cancelled,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TaskEnsureError {
    #[error("task plan policy is not permitted for this producer")]
    InvalidPolicy,
    #[error("task identity derivation failed")]
    Identity,
    #[error("task journal work limit exceeded")]
    JournalLimit,
    #[error("host adapter rejected task preparation")]
    AdapterPrepare,
    #[error("host adapter could not commit the accepted task")]
    AdapterCommit,
    #[error("existing Join task does not match the submitted final TaskSpec")]
    JoinSpecificationConflict,
}

pub trait TaskHost {
    fn ensure_task(
        &mut self,
        spec: TaskSpec,
    ) -> Result<TaskHandle, TaskEnsureError>;

    fn cancel_scope(
        &mut self,
        scope: CancelScopeId,
    ) -> Result<(), TaskCancellationError>;

    fn poll_frame(
        &mut self,
        budget: SchedulerBudget,
    ) -> Vec<TaskEvent>;
}
```

## 8. Host/adaptor transaction envelopes

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskLaunchRequest {
    pub correlation: TaskCorrelation,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub outcome: TaskOutcomeContract,
    pub request: HostTaskRequest,
    pub debug_label: String,
}

#[derive(Debug)]
pub struct PreparedHostTaskLaunch {
    correlation: TaskCorrelation,
    adapter_token: AdapterPreparedTaskToken,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskPublicationEnvelope {
    pub event: TaskEvent,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskRebindRequest {
    pub old: TaskCorrelation,
    pub new: TaskCorrelation,
}

#[derive(Debug)]
pub struct PreparedHostTaskRebind {
    old: TaskCorrelation,
    new: TaskCorrelation,
    adapter_token: AdapterPreparedRebindToken,
}

pub trait TaskLaunchAdapter {
    fn prepare_launch(
        &mut self,
        request: HostTaskLaunchRequest,
    ) -> Result<PreparedHostTaskLaunch, HostTaskPrepareError>;

    /// Commit is infallible after a successful prepare token.
    fn commit_launch(&mut self, prepared: PreparedHostTaskLaunch);

    fn rollback_launch(&mut self, prepared: PreparedHostTaskLaunch);

    fn prepare_rebind(
        &mut self,
        request: HostTaskRebindRequest,
    ) -> Result<PreparedHostTaskRebind, HostTaskRebindError>;

    fn commit_rebind(&mut self, prepared: PreparedHostTaskRebind);

    fn rollback_rebind(&mut self, prepared: PreparedHostTaskRebind);
}
```

`TaskHost::ensure_task` owns the journal transaction. Adapters see a derived
correlation and cannot supply or replace it.

## 9. Save, replay, and replacement

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeTaskJournalSnapshotV1 {
    pub version: RuntimeTaskJournalVersion,
    pub generations: Vec<GenerationTaskJournalSnapshotV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct RuntimeTaskJournalVersion(u8);

impl RuntimeTaskJournalVersion {
    pub const V1: Self = Self(1);
}

#[derive(Clone, Debug, PartialEq)]
pub struct GenerationTaskJournalSnapshotV1 {
    pub generation: GenerationId,
    pub ordinal_counters: Vec<AlwaysStartOrdinalSnapshotV1>,
    pub groups: Vec<TaskGroupSnapshotV1>,
    pub tasks: Vec<TaskSnapshotV1>,
    pub needs: Vec<RuntimeNeedStateSnapshotV1>,
    pub observers: Vec<NeedObserverSnapshotV1>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AlwaysStartOrdinalSnapshotV1 {
    pub producer: NeedProducerInstanceKey,
    pub last_allocated: TaskLaunchOrdinal,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeNeedStateSnapshotV1 {
    pub correlation: TaskCorrelation,
    pub cursor: Option<TaskPublicationCursor>,
    pub state: NeedSnapshotV1<RuntimeNeedOutcomeSnapshotV1>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeNeedHandleSnapshotV1 {
    pub correlation: TaskCorrelation,
    pub spec: TaskSpecSnapshotV1,
    pub origin: RuntimeNeedHandleOriginSnapshotV1,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeTaskReplayEnvelopeV1 {
    pub generation: GenerationId,
    pub event: TaskEvent,
    pub event_digest: RuntimeTaskEventDigest,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewNeedReplacementMappingV1 {
    pub old: CheckedViewMatchCoordinate,
    pub new: CheckedViewMatchCoordinate,
    pub old_revision: AcceptedViewProgramRevision,
    pub new_revision: AcceptedViewProgramRevision,
    pub expected: ViewNeedRebindEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewNeedRebindEvidence {
    pub checked_match: CheckedMatchSemanticDigest,
    pub view_admission: CheckedViewMatchAdmissionDigest,
    pub producer_admission: CheckedNeedProducerAdmissionDigest,
    pub producer_contract: NeedProducerContractDigest,
    pub payload_type: RuntimeTypeSemanticDigest,
    pub plan: TaskPlanSemanticDigest,
    pub ownership: OwnershipEvidenceDigest,
    pub resource_dependency: Option<ResourceDependencyDigest>,
    pub arguments: RuntimeValueDigest,
}
```

Snapshot vectors are encoded in deterministic key order. Restore first validates
the envelope/version and bounded counts, then reconstructs private maps, then
rederives every identity and event digest, then atomically publishes the
journal. No partially restored generation is observable.

## 10. AWBC task-plan row

```rust
// crates/arcweft-core/src/awbc/schema.rs

#[derive(Clone, Debug, PartialEq)]
pub struct AwbcTaskPlan {
    pub producer: AwbcNeedProducerRow,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub policy: TaskPolicy,
    pub outcome: TaskOutcomeContract,
    pub request: AwbcHostTaskRequestTemplate,
    pub debug_label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AwbcNeedProducerRow {
    pub family: NeedProducerFamily,
    pub contract: NeedProducerContractDigest,
    pub producer_site: u32,
    pub payload_type: RuntimeTypeSemanticDigest,
    pub semantic_binding: AwbcTaskSemanticBinding,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AwbcTaskSemanticBinding {
    Ordinary,
    View {
        program: ViewProgramIdProjection,
        site: ViewMatchSiteIdProjection,
        admission: CheckedViewMatchAdmissionDigestProjection,
    },
    AwaitManyBase,
    AwaitManyChild,
    Timeout {
        timeout_contract: NeedTimeoutContractDigest,
    },
    Line {
        line_plan: LinePlanSemanticDigest,
    },
}

impl AwbcTaskPlan {
    pub fn semantic_digest(
        &self,
        program: &AwbcProgram,
    ) -> Result<TaskPlanSemanticDigest, AwbcTaskPlanDigestError>;
}
```

The projection wrappers in core are private fixed-byte/string projections
validated against the compiler/bundle join; they expose no independent digest
constructor and are not second semantic authorities. `AwbcTaskPlan` contains no
`need_id` and no `plan_digest`.

Runtime-plan and bundle rows may store an expected
`TaskPlanSemanticDigest`; verifier and restore always recompute it from the
final owning program and compare.

The verifier adds an inherent policy check on `AwbcTaskPlan`:
`NeedProducerFamily::MakeNeedHandle` requires `TaskPolicy::JoinSameKey`.

## 11. Sink-parametric canonical RuntimeValue visitor

```rust
// crates/arcweft-core/src/entry/runtime_value.rs

trait CanonicalRuntimeValueSink {
    fn write(&mut self, bytes: &[u8]) -> Result<(), RuntimeSchemaError>;
    fn encoded_len(&self) -> usize;
}

struct CanonicalBytesSink {
    bytes: Vec<u8>,
    limit: usize,
}

struct CanonicalBlake3Sink {
    hasher: blake3::Hasher,
    encoded_len: usize,
    limit: usize,
}

impl RuntimeValue {
    pub fn try_canonical_bytes(
        &self,
        limits: RuntimeValueDigestLimits,
    ) -> Result<Vec<u8>, RuntimeSchemaError> {
        let mut sink = CanonicalBytesSink::new(limits.max_encoded_bytes);
        self.write_canonical(&mut sink, limits)?;
        Ok(sink.finish())
    }

    pub fn try_digest(
        &self,
        limits: RuntimeValueDigestLimits,
    ) -> Result<RuntimeValueDigest, RuntimeSchemaError> {
        let mut sink = CanonicalBlake3Sink::new(limits.max_encoded_bytes);
        self.write_canonical(&mut sink, limits)?;
        Ok(RuntimeValueDigest::from_bytes(sink.finish()))
    }

    fn write_canonical<S: CanonicalRuntimeValueSink>(
        &self,
        sink: &mut S,
        limits: RuntimeValueDigestLimits,
    ) -> Result<(), RuntimeSchemaError>;
}
```

The trait is private implementation plumbing of the existing owner, not an
extension trait. The exhaustive `RuntimeValue::write_canonical` method owns the
grammar. Both sinks charge the same byte count before writing. Recursion/node
limits and first errors are identical.

## 12. Generic Match owners

```rust
// crates/arcweft-lang-sema/src/final_analysis/checked_match.rs

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedMatchSemanticDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedMatchRef {
    pub generation: AcceptedSemanticGeneration,
    pub expression: ExprId, // session lookup only; excluded from digest/product identity
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatch {
    owner: ExprId,
    scrutinee: CheckedExpressionRef,
    arms: Box<[CheckedMatchArm]>,
    coverage: CheckedMatchCoverage,
    semantic_digest: CheckedMatchSemanticDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatchArm {
    pub ordinal: u32,
    pub scope: ScopeId,
    pub pattern: CheckedPatternRef,
    pub bindings: Box<[CheckedMatchBindingRef]>,
    pub guard: Option<CheckedExpressionRef>,
    pub guard_class: CheckedGuardClass,
    pub body: CheckedExpressionRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedGuardClass {
    ConstantTrue,
    ConstantFalse,
    Dynamic,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatchCoverage {
    exhaustive: bool,
    unreachable: Box<[CheckedUnreachableArm]>,
}

impl CheckedMatch {
    pub(crate) fn try_from_hir(
        module: &HirModule,
        owner: ExprId,
        expressions: &BTreeMap<ExprId, CheckedExpression>,
        patterns: &BTreeMap<PatternId, CheckedPattern>,
        bindings: &BTreeMap<LocalId, CheckedBinding>,
        symbols: &ProjectSymbolTable,
        world: &RegisteredSemanticWorld,
        limits: CheckedMatchLimits,
    ) -> Result<Self, CheckedMatchConstructionError>;

    pub const fn semantic_digest(&self) -> CheckedMatchSemanticDigest;
    pub const fn coverage(&self) -> &CheckedMatchCoverage;
}

impl CheckedGuardClass {
    pub fn from_checked_expression(
        expression: &CheckedExpression,
    ) -> Self {
        match expression.resolution() {
            CheckedExpressionResolution::Literal(HirLiteral::Boolean(true)) =>
                Self::ConstantTrue,
            CheckedExpressionResolution::Literal(HirLiteral::Boolean(false)) =>
                Self::ConstantFalse,
            _ => Self::Dynamic,
        }
    }
}
```

No ownership context is accepted by `try_from_hir`. `CheckedMatchCoverage` has
no public constructor; only the private bounded analyzer constructs it.

## 13. View admission owners

```rust
// crates/arcweft-lang-sema or compiler projection boundary

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedViewMatchAdmissionDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedNeedProducerAdmissionDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OwnershipEvidenceDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewMatchSiteId([u8; 32]);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedViewMatchCoordinate {
    pub program: ViewProgramId,
    pub site: ViewMatchSiteId,
    pub admission: CheckedViewMatchAdmissionDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedViewMatchAdmission {
    pub checked_match: CheckedMatchRef,
    pub retained_outputs: Box<[CheckedRetainedViewValue]>,
    pub retained_captures: Box<[CheckedRetainedViewValue]>,
    pub ownership: CheckedOwnershipCertificate,
    pub need_producer: CheckedNeedProducerAdmission,
    pub digest: CheckedViewMatchAdmissionDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRetainedViewValue {
    pub coordinate: StableCheckedValueCoordinate,
    pub ty: SemanticTypeDigest,
    pub disposition: RetainedValueDisposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainedValueDisposition {
    Copy,
    SnapshotClone,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedNeedProducerAdmission {
    pub arguments: Box<[CheckedProducerArgumentAdmission]>,
    pub ownership: CheckedOwnershipCertificate,
    pub digest: CheckedNeedProducerAdmissionDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProducerArgumentAdmission {
    pub coordinate: StableCheckedValueCoordinate,
    pub ty: SemanticTypeDigest,
    pub disposition: RetainedValueDisposition,
}

pub struct CheckedOwnershipContext<'a> {
    pub symbols: &'a ProjectSymbolTable,
    pub world: &'a RegisteredSemanticWorld,
}

impl CheckedViewMatchAdmission {
    pub fn try_new(
        checked_match: CheckedMatchRef,
        outputs: ExactRetainedViewOutputs<'_>,
        captures: ExactRetainedViewCaptures<'_>,
        producer: CheckedNeedProducerAdmission,
        context: CheckedOwnershipContext<'_>,
        limits: CheckedOwnershipLimits,
    ) -> Result<Self, CheckedViewMatchAdmissionError>;
}
```

`ResourceTypeRegistry` is absent from this API.

## 14. Stable View site

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedExpressionChildRolePath(
    Box<[CheckedExpressionChildRole]>,
);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedExpressionChildRole {
    Root,
    CallCallee,
    CallArgument(u32),
    MatchScrutinee,
    MatchArmPattern(u32),
    MatchArmGuard(u32),
    MatchArmBody(u32),
    TupleItem(u32),
    RecordField(RuntimeRecordFieldId),
    BlockStatement(u32),
    BlockTail,
    ConditionalCondition,
    ConditionalThen,
    ConditionalElse,
    ViewProperty(ViewPropertyId),
}

impl ViewMatchSiteId {
    pub fn for_checked_path(
        program: &ViewProgramId,
        declaration: &AcceptedDeclarationSemanticId,
        path: &CheckedExpressionChildRolePath,
    ) -> Self;

    pub const fn as_bytes(&self) -> &[u8; 32];
}
```

HIR IDs and `SourceSpan` are not parameters. The site type is a semantic
coordinate, so it accepts the complete BLAKE3 output and uses `Option` for
absence.

## 15. Opaque evidence chain

```rust
// existing Arcweft-owned enums gain inherent semantic encoding where absent
impl RuntimeOpaqueValueClass {
    pub const fn semantic_tag(&self) -> u8;
    pub fn write_semantic_payload(
        &self,
        sink: &mut impl SemanticSink,
    ) -> Result<(), AcceptedNominalCatalogError>;
}

impl RuntimeOpaquePersistence {
    pub const fn semantic_tag(self) -> u8;
}

pub struct AcceptedNominalInventoryInput {
    id: AcceptedNominalId,
    arity: u16,
    runtime_producer: RuntimeOpaqueTypeProducerId,
    value_class: RuntimeOpaqueValueClass,
    persistence: RuntimeOpaquePersistence,
    visibility: AcceptedNominalInputVisibility,
    origin: AcceptedNominalOrigin,
    source: SourceSpan,
    item: EnvironmentPublicationItemId,
}

impl AcceptedNominalInventoryInput {
    pub fn new(
        id: AcceptedNominalId,
        arity: u16,
        runtime_producer: RuntimeOpaqueTypeProducerId,
        value_class: RuntimeOpaqueValueClass,
        persistence: RuntimeOpaquePersistence,
        visibility: AcceptedNominalInputVisibility,
        origin: AcceptedNominalOrigin,
        source: SourceSpan,
        item: EnvironmentPublicationItemId,
    ) -> Self;
}

pub enum AcceptedNominalSemantics {
    Exact(TypeKind),
    Opaque {
        producer: RuntimeOpaqueTypeProducerId,
        value_class: RuntimeOpaqueValueClass,
        persistence: RuntimeOpaquePersistence,
    },
    Character(CharacterNominalType),
}

impl AcceptedNominalRecord {
    pub fn try_new_opaque(
        id: AcceptedNominalId,
        arity: u16,
        producer: RuntimeOpaqueTypeProducerId,
        value_class: RuntimeOpaqueValueClass,
        persistence: RuntimeOpaquePersistence,
        origin: AcceptedNominalOrigin,
        source: Option<SourceSpan>,
    ) -> Result<Self, AcceptedNominalCatalogError>;
}
```

The registrar has no overload that omits the two evidence fields.

## 16. Ownership classifier and value-level certificates

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedOwnershipCertificate {
    pub disposition: RetainedValueDisposition,
    pub evidence: OwnershipEvidenceDigest,
    consulted: Box<[ConsultedOwnershipEvidence]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedValueOwnershipCertificate {
    TypeDirected(CheckedOwnershipCertificate),
    CaptureFreeStableCallable {
        callable: RuntimeCallableId,
        contract: CallableContractHash,
        evidence: OwnershipEvidenceDigest,
    },
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CheckedOwnershipError {
    #[error("affine value cannot be retained")]
    AffineValue,
    #[error("Stream value cannot be retained")]
    StreamValue,
    #[error("borrowed value cannot be retained")]
    BorrowedValue,
    #[error("frame-local value cannot be retained")]
    FrameLocalValue,
    #[error("Function requires an exact value-level certificate")]
    FunctionValueRequiresCertificate,
    #[error("ViewValue has no published persistence owner")]
    MissingViewPersistenceEvidence,
    #[error("runtime snapshot owner is missing")]
    MissingRuntimeSnapshotOwner,
    #[error("opaque persistence evidence is missing")]
    MissingOpaquePersistenceEvidence,
    #[error("recursive retained type cycle is unsupported")]
    RecursiveRetentionCycle,
    #[error("generic/open/projected/error type is not closed")]
    UnresolvedType,
    #[error("ownership work limit exceeded")]
    WorkLimit,
}

impl RegisteredSemanticWorld {
    pub fn checked_ownership(
        &self,
        ty: &TypeKind,
        symbols: &ProjectSymbolTable,
        limits: CheckedOwnershipLimits,
    ) -> Result<CheckedOwnershipCertificate, CheckedOwnershipError>;
}
```

The classifier is an inherent method on the accepted semantic-world owner.
No extension trait or feature-local helper supplies missing enum behavior.

## 17. Serialization policy

Public task/Need types use the private version-1 Wire owner for canonical
persistence. Serde, where retained for diagnostics or noncanonical structured
data, serializes fixed identities as exactly 32 bytes and validates on
deserialize. It never emits or accepts hex/String identity.

All ordinary `u32` fields in private Wire continue to use the maintained
canonical shortest base-128 varint. Hash transcripts use the explicit
little-endian widths written in `IDENTITY_AND_DIGESTS.md`; those are hash
grammar fields, not a second AWBC wire allocation.
