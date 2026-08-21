# Complete version-1 persistence, replay and restore contract

## 1. Codec owner

`arcweft_runtime_scheduler::RuntimeTaskSnapshotCodecV1` is the sole scheduler
snapshot/replay codec. `RuntimeValueSnapshotV1` is core-owned and replaces the
AWBC-only duplicate value snapshot authority; AWBC save becomes one consumer.

The format is a private Arcweft v1 format, not generic Serde and not a public
AWBC opcode table. This correction does not alter AWBC opcode/function/flag
allocation.

## 2. Primitive byte grammar

```text
version        u8, exactly 1
enum tag       u8, closed by the owning enum's inherent encode/decode match
bool           u8 0 or 1
u16            two little-endian bytes
u32/u64/u128   fixed little-endian where the row says fixed
length/count   canonical shortest unsigned base-128 u32 varint
bytes          length || exact bytes
utf8           length || exact validated UTF-8 bytes
digest/id      exactly 32 bytes
Option<T>      0, or 1 || T
Vec<T>         count || elements in declared order
map            count || row projections in declared canonical key order
```

All counts are checked against `RuntimeSnapshotLimits` before allocation.
Unknown tags, invalid bool/Option bytes, nonminimal varints, overflow, duplicate
or out-of-order keys, invalid UTF-8 and trailing bytes reject.

All-zero is rejected only for the fixed identity owners:
`NeedProducerInstanceKey`, `NeedId`, `TaskKey`, `TaskId`. Semantic digests
accept all 32-byte outputs. Absence is always `Option`, never a zero sentinel.

## 3. Field and key order

Fields are emitted in Rust declaration order shown below and in
`machine/persistence_schemas.json`. Map-like collections are sorted by:

- generations: `GenerationId`;
- ordinal counters: producer-instance bytes;
- groups: `TaskKey`;
- launches/runtime rows: `TaskId`;
- Needs: `NeedId`;
- observers: `TaskObserverId`;
- child rows/outputs/specs: exact source index;
- events: `(generation, logical_epoch, sequence, TaskId)`;
- replacement View mappings: old program/site projection;
- replacement task mappings: old `TaskId`.

The decoder rejects unsorted input instead of sorting attacker-controlled
duplicates into an apparently valid map.

## 4. Complete row inventory

| Row | Kind | Exact fields/variants | Key/order | Main invariant |
|---|---|---|---|---|
| `RuntimeTaskSchedulerSnapshotV1` | struct | `version: RuntimeTaskSchedulerSnapshotVersion`<br>`journal: RuntimeTaskJournalSnapshotV1`<br>`runtime_tasks: RuntimeTaskStateSnapshotV1`<br>`pending_events: Vec<TaskEventSnapshotV1>`<br>`replacement: ReplacementStateSnapshotV1` | journal.active_generation | version exactly 1 |
| `RuntimeTaskJournalSnapshotV1` | struct | `version: RuntimeTaskJournalSnapshotVersion`<br>`active_generation: GenerationId`<br>`generations: Vec<RuntimeTaskGenerationSnapshotV1>` | active_generation | exactly one generation row matches active_generation |
| `RuntimeTaskGenerationSnapshotV1` | struct | `version: RuntimeTaskGenerationSnapshotVersion`<br>`generation: GenerationId`<br>`ordinal_counters: Vec<AlwaysStartOrdinalCounterSnapshotV1>`<br>`groups: Vec<TaskGroupSnapshotV1>`<br>`launches: Vec<TaskLaunchSnapshotV1>`<br>`needs: Vec<NeedCellSnapshotV1>`<br>`observers: Vec<TaskObserverSnapshotV1>`<br>`replay: TaskReplayStateSnapshotV1` | generation | all joins stay within this generation except NeedId identity |
| `AlwaysStartOrdinalCounterSnapshotV1` | struct | `version: AlwaysStartOrdinalCounterSnapshotVersion`<br>`producer: NeedProducerInstanceKey`<br>`next_ordinal: TaskLaunchOrdinal` | producer | next_ordinal is at least 1 |
| `TaskGroupSnapshotV1` | struct | `version: TaskGroupSnapshotVersion`<br>`task_key: TaskKey`<br>`producer: NeedProducerInstanceKey`<br>`policy: TaskPolicy`<br>`launches: Vec<TaskLaunchMappingSnapshotV1>` | task_key | TaskKey rederives from generation, producer and policy |
| `TaskLaunchMappingSnapshotV1` | struct | `version: TaskLaunchMappingSnapshotVersion`<br>`ordinal: TaskLaunchOrdinal`<br>`task_id: TaskId`<br>`need_id: NeedId` | ordinal | TaskId/NeedId rederive from group producer/policy/ordinal |
| `TaskLaunchSnapshotV1` | struct | `version: TaskLaunchSnapshotVersion`<br>`correlation: TaskCorrelationSnapshotV1`<br>`spec: TaskSpecSnapshotV1`<br>`lifecycle: TaskLifecycleSnapshotV1`<br>`last_publication: Option<TaskPublicationCursorSnapshotV1>`<br>`last_event_digest: Option<TaskEventDigest>`<br>`host_state: Option<HostTaskStateSnapshotV1>` | correlation.task_id | correlation rederives from spec and generation |
| `TaskSpecSnapshotV1` | struct | `version: TaskSpecSnapshotVersion`<br>`producer: NeedProducerSpecSnapshotV1`<br>`class: TaskClass`<br>`priority: TaskPriority`<br>`cancel_scope: CancelScopeId`<br>`policy: TaskPolicy`<br>`outcome: TaskOutcomeContractSnapshotV1`<br>`execution: TaskExecutionSnapshotV1`<br>`debug: TaskDebugMetadataSnapshotV1` | fields above | exactly one execution row |
| `NeedProducerSpecSnapshotV1` | struct | `version: NeedProducerSpecSnapshotVersion`<br>`family: NeedProducerFamily`<br>`contract: NeedProducerContractDigest`<br>`plan: TaskPlanSemanticDigest`<br>`producer_site: u32`<br>`payload_type: RuntimeTypeSemanticDigest`<br>`arguments: RuntimeValueDigest` | fields above | NeedProducerInstanceKey is recomputed; stored key is never authoritative |
| `TaskCorrelationSnapshotV1` | struct | `version: TaskCorrelationSnapshotVersion`<br>`generation: GenerationId`<br>`producer: NeedProducerInstanceKey`<br>`policy: TaskPolicy`<br>`ordinal: TaskLaunchOrdinal`<br>`need_id: NeedId`<br>`task_key: TaskKey`<br>`task_id: TaskId` | fields above | all fixed IDs nonzero |
| `TaskLifecycleSnapshotV1` | enum | `Accepted`<br>`Running`<br>`Ready`<br>`InfrastructureFailed`<br>`Cancelled` | declaration order | terminal variants agree with Need state |
| `TaskOutcomeContractSnapshotV1` | struct | `version: TaskOutcomeContractSnapshotVersion`<br>`payload_type: RuntimeTypeSemanticDigest`<br>`runtime_checked_type: RuntimeCheckedTypeSnapshotV1` | declaration order | runtime_checked_type semantic digest equals payload_type |
| `TaskDebugMetadataSnapshotV1` | struct | `version: TaskDebugMetadataSnapshotVersion`<br>`label: Option<BoundedUtf8>`<br>`origin: Option<BoundedUtf8>` | declaration order | diagnostic only; never read by identity/execution selection |
| `TaskExecutionSnapshotV1` | enum | `Host`(request: HostTaskRequestSnapshotV1)<br>`Runtime`(request: RuntimeTaskRequestSnapshotV1) | declaration order | one closed discriminant; no parallel Option fields |
| `HostTaskRequestSnapshotV1` | enum | `FileReadText`(path: BoundedUtf8)<br>`FileReadBytes`(path: BoundedUtf8)<br>`FileWriteText`(path: BoundedUtf8, text: BoundedUtf8)<br>`FileWriteBytes`(path: BoundedUtf8, bytes: BoundedBytes)<br>`HttpFetch`(url: BoundedUtf8, method: BoundedUtf8, headers: Vec<HeaderSnapshotV1>, body: Option<RuntimePayloadSnapshotV1>)<br>`HttpRespond`(request_id: BoundedUtf8, status: u16, headers: Vec<HeaderSnapshotV1>, body: Option<RuntimePayloadSnapshotV1>)<br>`ProcessRun`(program: BoundedUtf8, args: Vec<BoundedUtf8>, env: Vec<EnvPairSnapshotV1>)<br>`AssetLoad`(id: BoundedUtf8, kind: BoundedUtf8)<br>`ShaderCompile`(id: BoundedUtf8, entry: Option<BoundedUtf8>)<br>`AudioDecode`(id: BoundedUtf8)<br>`TtsSynthesis`(voice: Option<BoundedUtf8>, text: BoundedUtf8)<br>`WasmCall`(module: BoundedUtf8, function: BoundedUtf8, args: Vec<RuntimePayloadSnapshotV1>)<br>`SystemInfo`(kind: SystemInfoKind)<br>`Custom`(capability: HostCapabilityId, operation: RuntimeHostOperationId, args: Vec<RuntimePayloadSnapshotV1>, named_args: Vec<NamedRuntimePayloadSnapshotV1>) | declaration order | variant matches accepted HostOperation contract |
| `HeaderSnapshotV1` | struct | `name: BoundedUtf8`<br>`value: BoundedUtf8` | declaration order | strict row decode |
| `EnvPairSnapshotV1` | struct | `name: BoundedUtf8`<br>`value: BoundedUtf8` | declaration order | strict row decode |
| `NamedRuntimePayloadSnapshotV1` | struct | `name: BoundedUtf8`<br>`value: RuntimePayloadSnapshotV1` | declaration order | strict row decode |
| `HostTaskStateSnapshotV1` | struct | `version: HostTaskStateSnapshotVersion`<br>`correlation: TaskCorrelationSnapshotV1`<br>`request: HostTaskRequestSnapshotV1`<br>`phase: HostTaskPhaseSnapshotV1`<br>`restore_policy: HostTaskRestorePolicySnapshotV1` | correlation.task_id | request equals TaskSpec Host row |
| `HostTaskPhaseSnapshotV1` | enum | `Accepted`<br>`Dispatched`<br>`Terminal` | declaration order | strict row decode |
| `HostTaskRestorePolicySnapshotV1` | enum | `Restartable`<br>`MustBeQuiescent` | declaration order | strict row decode |
| `RuntimeTaskRequestSnapshotV1` | enum | `AwaitManyAggregate`(request: RuntimeAwaitManyAggregateRequestSnapshotV1)<br>`Timeout`(request: RuntimeTimeoutRequestSnapshotV1) | declaration order | strict row decode |
| `RuntimeAwaitManyAggregateRequestSnapshotV1` | struct | `version: RuntimeAwaitManyAggregateRequestSnapshotVersion`<br>`source_items: Vec<RuntimeValueSnapshotV1>`<br>`children: Vec<TaskSpecSnapshotV1>`<br>`limit: NonZeroU32` | declaration order | source_items.len equals children.len |
| `RuntimeTimeoutRequestSnapshotV1` | struct | `version: RuntimeTimeoutRequestSnapshotVersion`<br>`source: RuntimeNeedHandleSnapshotV1`<br>`requested_limit: LogicalDurationSnapshotV1`<br>`contract: NeedTimeoutContractDigest` | declaration order | source handle structurally validates |
| `RuntimeTaskStateSnapshotV1` | struct | `version: RuntimeTaskStateSnapshotVersion`<br>`tasks: Vec<RuntimeTaskRowSnapshotV1>` | tasks ascending TaskId | one runtime row for every active Runtime execution launch and no Host launch |
| `RuntimeTaskRowSnapshotV1` | struct | `version: RuntimeTaskRowSnapshotVersion`<br>`correlation: TaskCorrelationSnapshotV1`<br>`state: RuntimeTaskRequestStateSnapshotV1` | correlation.task_id | strict row decode |
| `RuntimeTaskRequestStateSnapshotV1` | enum | `AwaitManyAggregate`(state: RuntimeAwaitManyAggregateTaskSnapshotV1)<br>`Timeout`(state: RuntimeTimeoutNeedSnapshotV1) | declaration order | strict row decode |
| `RuntimeAwaitManyAggregateTaskSnapshotV1` | struct | `version: RuntimeAwaitManyAggregateTaskSnapshotVersion`<br>`aggregate: TaskCorrelationSnapshotV1`<br>`source_count: u32`<br>`source_items: Vec<RuntimeValueSnapshotV1>`<br>`child_specs: Vec<TaskSpecSnapshotV1>`<br>`children: Vec<RuntimeAwaitManyChildSnapshotV1>`<br>`limit: NonZeroU32`<br>`launch_cursor: u32`<br>`in_flight: u32`<br>`outputs: Vec<Option<RuntimePayloadSnapshotV1>>`<br>`publication_cursor: TaskPublicationCursorSnapshotV1`<br>`terminal: Option<RuntimeAwaitManyTerminalSnapshotV1>` | declaration order | all source-indexed vectors have source_count length |
| `RuntimeAwaitManyChildSnapshotV1` | struct | `version: RuntimeAwaitManyChildSnapshotVersion`<br>`source_index: u32`<br>`observer: Option<TaskObserverId>`<br>`handle: Option<RuntimeNeedHandleSnapshotV1>`<br>`status: RuntimeAwaitManyChildStatusSnapshotV1`<br>`last_cursor: Option<TaskPublicationCursorSnapshotV1>` | source_index | handle and observer absent only before launch |
| `RuntimeAwaitManyChildStatusSnapshotV1` | enum | `NotLaunched`<br>`Waiting`<br>`Ready`<br>`InfrastructureFailed`(failure: RuntimeTaskFailureSnapshotV1)<br>`Cancelled` | declaration order | strict row decode |
| `RuntimeAwaitManyTerminalSnapshotV1` | enum | `Ready`<br>`InfrastructureFailed`(failure: RuntimeTaskFailureSnapshotV1)<br>`Cancelled` | declaration order | strict row decode |
| `RuntimeTimeoutNeedSnapshotV1` | struct | `version: RuntimeTimeoutNeedSnapshotVersion`<br>`output: TaskCorrelationSnapshotV1`<br>`source: RuntimeNeedHandleSnapshotV1`<br>`requested_limit: LogicalDurationSnapshotV1`<br>`remaining: LogicalDurationSnapshotV1`<br>`phase: RuntimeTimeoutPhaseSnapshotV1`<br>`source_observer: Option<TaskObserverId>`<br>`source_cursor: Option<TaskPublicationCursorSnapshotV1>`<br>`publication_cursor: TaskPublicationCursorSnapshotV1`<br>`terminal: Option<RuntimeTimeoutTerminalSnapshotV1>` | declaration order | remaining <= requested_limit |
| `RuntimeTimeoutPhaseSnapshotV1` | enum | `NotStarted`<br>`Waiting`<br>`Resolved` | declaration order | strict row decode |
| `RuntimeTimeoutTerminalSnapshotV1` | enum | `SourceReady`(payload: RuntimePayloadSnapshotV1)<br>`SourceInfrastructureFailed`(failure: RuntimeTaskFailureSnapshotV1)<br>`Expired`<br>`Cancelled` | declaration order | strict row decode |
| `NeedCellSnapshotV1` | struct | `version: NeedCellSnapshotVersion`<br>`need_id: NeedId`<br>`producer: NeedProducerInstanceKey`<br>`outcome: TaskOutcomeContractSnapshotV1`<br>`state: NeedStateSnapshotV1`<br>`last_publication: Option<TaskPublicationCursorSnapshotV1>`<br>`observers: Vec<TaskObserverId>` | need_id | NeedId rederives from producer/policy/ordinal through owning launch |
| `NeedStateSnapshotV1` | enum | `NotStarted`<br>`Pending`(progress: ProgressSnapshotV1)<br>`Ready`(outcome: RuntimeNeedOutcomeSnapshotV1)<br>`Cancelled` | declaration order | strict row decode |
| `RuntimeNeedOutcomeSnapshotV1` | enum | `Payload`(payload: RuntimePayloadSnapshotV1)<br>`InfrastructureFailure`(failure: RuntimeTaskFailureSnapshotV1) | declaration order | strict row decode |
| `RuntimeTaskFailureSnapshotV1` | struct | `version: RuntimeTaskFailureSnapshotVersion`<br>`kind: RuntimeTaskFailureKind`<br>`diagnostic: BoundedUtf8` | declaration order | closed failure kind |
| `TaskObserverSnapshotV1` | struct | `version: TaskObserverSnapshotVersion`<br>`observer_id: TaskObserverId`<br>`generation: GenerationId`<br>`need_id: NeedId`<br>`kind: TaskObserverKindSnapshotV1`<br>`last_seen: Option<TaskPublicationCursorSnapshotV1>`<br>`active: bool` | observer_id | generation equals active use generation |
| `TaskObserverKindSnapshotV1` | enum | `Await`(fiber: RuntimeFiberId)<br>`AwaitManyChild`(aggregate_task: TaskId, source_index: u32)<br>`TimeoutSource`(timeout_task: TaskId)<br>`ViewMatch`(view_instance: ViewInstanceIdProjection, site: ViewMatchSiteIdProjection) | declaration order | strict row decode |
| `RuntimeNeedHandleSnapshotV1` | struct | `version: RuntimeNeedHandleSnapshotVersion`<br>`correlation: TaskCorrelationSnapshotV1`<br>`producer: NeedProducerSpecSnapshotV1`<br>`outcome: TaskOutcomeContractSnapshotV1`<br>`origin: NeedHandleOriginSnapshotV1` | declaration order | semantic Eq/Hash/Ord remains NeedId-only |
| `NeedHandleOriginSnapshotV1` | struct | `version: NeedHandleOriginSnapshotVersion`<br>`accepted_site: Option<StableProducerSiteProjection>`<br>`debug_label: Option<BoundedUtf8>` | declaration order | diagnostic/provenance only and excluded from canonical RuntimeValue identity |
| `TaskPublicationCursorSnapshotV1` | struct | `version: TaskPublicationCursorSnapshotVersion`<br>`logical_epoch: u64`<br>`sequence: u64` | declaration order | monotonic per task; gaps rejected according to replay policy |
| `TaskEventSnapshotV1` | struct | `version: TaskEventSnapshotVersion`<br>`correlation: TaskCorrelationSnapshotV1`<br>`cursor: TaskPublicationCursorSnapshotV1`<br>`kind: TaskEventKindSnapshotV1`<br>`digest: TaskEventDigest` | fields above | digest recomputed before state transition |
| `TaskEventKindSnapshotV1` | enum | `Progress`(progress: ProgressSnapshotV1)<br>`Ready`(payload: RuntimePayloadSnapshotV1)<br>`InfrastructureFailure`(failure: RuntimeTaskFailureSnapshotV1)<br>`Cancelled` | declaration order | strict row decode |
| `TaskReplayStateSnapshotV1` | struct | `version: TaskReplayStateSnapshotVersion`<br>`last_applied: Vec<TaskReplayCursorSnapshotV1>`<br>`accepted_event_digests: Vec<TaskReplayDigestSnapshotV1>` | both vectors ascending TaskId | keys unique; cursor/digest joins exact |
| `TaskReplayCursorSnapshotV1` | struct | `version: TaskReplayCursorSnapshotVersion`<br>`task_id: TaskId`<br>`cursor: TaskPublicationCursorSnapshotV1` | task_id | strict row decode |
| `TaskReplayDigestSnapshotV1` | struct | `version: TaskReplayDigestSnapshotVersion`<br>`task_id: TaskId`<br>`cursor: TaskPublicationCursorSnapshotV1`<br>`digest: TaskEventDigest` | task_id, cursor | strict row decode |
| `TaskReplayEnvelopeV1` | struct | `version: TaskReplayEnvelopeVersion`<br>`generation: GenerationId`<br>`events: Vec<TaskEventSnapshotV1>` | events normalized by event ordering key | generation matches every event |
| `ReplacementStateSnapshotV1` | enum | `Idle`<br>`Validated`(plan: ReplacementPlanSnapshotV1) | declaration order | Prepared adapter tokens are never persistable; snapshot fails while adapter prepare/commit is in flight |
| `ReplacementPlanSnapshotV1` | struct | `version: ReplacementPlanSnapshotVersion`<br>`from_generation: GenerationId`<br>`to_generation: GenerationId`<br>`view_mappings: Vec<ReplacementViewMappingSnapshotV1>`<br>`task_mappings: Vec<ReplacementTaskMappingSnapshotV1>` | view mappings by old site; task mappings by old TaskId | to_generation differs from from_generation |
| `ReplacementViewMappingSnapshotV1` | struct | `version: ReplacementViewMappingSnapshotVersion`<br>`old_program: ViewProgramIdProjection`<br>`old_revision: AcceptedViewProgramRevisionProjection`<br>`old_site: ViewMatchSiteIdProjection`<br>`new_program: ViewProgramIdProjection`<br>`new_revision: AcceptedViewProgramRevisionProjection`<br>`new_site: ViewMatchSiteIdProjection` | old_program, old_site | strict row decode |
| `ReplacementTaskMappingSnapshotV1` | struct | `version: ReplacementTaskMappingSnapshotVersion`<br>`old: TaskCorrelationSnapshotV1`<br>`new: TaskCorrelationSnapshotV1`<br>`need_handle: RuntimeNeedHandleSnapshotV1` | old.task_id | NeedId, producer, policy, ordinal equal old/new |
| `RuntimePayloadSnapshotV1` | struct | `version: RuntimePayloadSnapshotVersion`<br>`value: RuntimeValueSnapshotV1` | declaration order | strict row decode |
| `RuntimeValueSnapshotV1` | enum | `Unit`<br>`Bool`(value: bool)<br>`Int`(value: RuntimeIntSnapshotV1)<br>`UInt`(value: RuntimeUIntSnapshotV1)<br>`F32`(bits: u32)<br>`F64`(bits: u64)<br>`Matrix`(value: RuntimeMatrixSnapshotV1)<br>`Tensor`(value: RuntimeTensorSnapshotV1)<br>`String`(value: BoundedUtf8)<br>`Char`(scalar: u32)<br>`Duration`(value: LogicalDurationSnapshotV1)<br>`Progress`(value: ProgressSnapshotV1)<br>`Range`(value: RuntimeRangeSnapshotV1)<br>`Iterator`(value: RuntimeIteratorSnapshotV1)<br>`EntityRef`(value: BoundedUtf8)<br>`Tuple`(items: Vec<RuntimeValueSnapshotV1>)<br>`Seq`(value: RuntimeSeqSnapshotV1)<br>`Record`(fields: Vec<RuntimeFieldSnapshotV1>)<br>`NominalRecord`(value: RuntimeNominalRecordSnapshotV1)<br>`Opaque`(value: RuntimeOpaqueValueSnapshotV1)<br>`Reduction`(value: RuntimeReductionSnapshotV1)<br>`Agent`(value: RuntimeAgentSnapshotV1)<br>`Function`(value: RuntimeFunctionSnapshotV1)<br>`Variant`(value: RuntimeVariantSnapshotV1)<br>`NeedHandle`(value: RuntimeNeedHandleSnapshotV1) | declaration order | exhaustive over final public RuntimeValue in Cut 5 |
| `RuntimeCheckedTypeSnapshotV1` | struct | `version: RuntimeCheckedTypeSnapshotVersion`<br>`semantic_digest: RuntimeTypeSemanticDigest`<br>`projection: RuntimeCheckedTypeProjectionV1` | declaration order | projection encodes the closed runtime checked-type algebra; digest is recomputed |
| `ProgressSnapshotV1` | struct | `ratio_bits: u32`<br>`label: Option<BoundedUtf8>` | declaration order | strict row decode |
| `LogicalDurationSnapshotV1` | struct | `nanoseconds: u128` | declaration order | strict row decode |
| `RuntimeIntSnapshotV1` | struct | `width: RuntimeSignedIntWidth`<br>`bits: u128` | declaration order | strict row decode |
| `RuntimeUIntSnapshotV1` | struct | `width: RuntimeUnsignedIntWidth`<br>`bits: u128` | declaration order | strict row decode |
| `RuntimeMatrixSnapshotV1` | struct | `kind: RuntimeMatrixKind`<br>`dimensions: Vec<u32>`<br>`scalar_bits: BoundedBytes` | declaration order | strict row decode |
| `RuntimeTensorSnapshotV1` | struct | `kind: RuntimeTensorKind`<br>`shape: Vec<u32>`<br>`scalar_bits: BoundedBytes` | declaration order | strict row decode |
| `RuntimeRangeSnapshotV1` | struct | `start: Option<RuntimeValueSnapshotV1>`<br>`end: Option<RuntimeValueSnapshotV1>`<br>`inclusive: bool` | declaration order | strict row decode |
| `RuntimeIteratorSnapshotV1` | struct | `source: RuntimeValueSnapshotV1`<br>`cursor: u64` | declaration order | strict row decode |
| `RuntimeSeqSnapshotV1` | struct | `kind: RuntimeSeqKind`<br>`items: Vec<RuntimeValueSnapshotV1>` | declaration order | strict row decode |
| `RuntimeFieldSnapshotV1` | struct | `field: AcceptedFieldIdentityProjection`<br>`value: RuntimeValueSnapshotV1` | declaration order | strict row decode |
| `RuntimeNominalRecordSnapshotV1` | struct | `owner: RuntimeNominalTypeId`<br>`layout: TypeLayoutHash`<br>`fields: Vec<RuntimeFieldSnapshotV1>` | declaration order | strict row decode |
| `RuntimeOpaqueValueSnapshotV1` | struct | `producer: RuntimeOpaqueTypeProducerId`<br>`semantic_identity: AcceptedNominalSemanticIdentity`<br>`class: RuntimeOpaqueValueClass`<br>`persistence: RuntimeOpaquePersistence`<br>`payload: BoundedBytes` | declaration order | strict row decode |
| `RuntimeReductionSnapshotV1` | struct | `kind: RuntimeReductionKind`<br>`value: Option<RuntimeValueSnapshotV1>` | declaration order | strict row decode |
| `RuntimeAgentSnapshotV1` | struct | `variant: RuntimeAgentValueProjectionV1` | declaration order | strict row decode |
| `RuntimeFunctionSnapshotV1` | struct | `callable: RuntimeCallableId`<br>`contract: CallableContractHash`<br>`captures: Vec<RuntimeValueSnapshotV1>` | declaration order | strict row decode |
| `RuntimeVariantSnapshotV1` | struct | `owner: RuntimeVariantIdentity`<br>`case_ordinal: u32`<br>`payload: Option<RuntimeValueSnapshotV1>` | declaration order | strict row decode |

The Rust-shaped declarations and all remaining invariants are in
`RUST_SCHEMAS.md`; the JSON is the machine authority used by the package
validator. Prose and machine inventories must have exactly the same row names.

## 5. Save admission

Snapshot is rejected before projection when:

- an adapter launch/restore/rebind token is prepared;
- replacement is in its nonpersistable Prepared phase;
- a Host task with `MustBeQuiescent` policy is in flight;
- any RuntimeValue lacks a complete snapshot projection;
- any row/list/byte/depth limit would be exceeded.

A restartable Host task persists its complete request, correlation, lifecycle
phase and restore policy. Adapter-private handles/tokens are not bytes in the
snapshot. Restore calls `prepare_restore` with the complete typed rows.

## 6. Identity/digest rederivation

The decoder treats stored digests and IDs as claims:

1. decode fixed fields;
2. reconstruct `NeedProducerSpec`;
3. recompute `NeedProducerInstanceKey`;
4. rederive NeedId/TaskKey/TaskId from generation/policy/ordinal;
5. recompute RuntimeValue argument digest from restored canonical value where a
   value row is present;
6. recompute runtime type, plan, event and bundle digests from their semantic
   owners;
7. compare every stored claim.

No field is accepted merely because its length is 32 bytes.

## 7. Private restore construction

Restore never incrementally mutates the live scheduler:

```rust
struct ValidatedSchedulerRestore<A: TaskLaunchAdapter> {
    journal: RuntimeTaskJournal,
    runtime_tasks: RuntimeTaskState,
    pending_events: BTreeMap<TaskEventOrderKey, TaskEvent>,
    replacement: ReplacementState,
    host_restore: HostTaskRestoreBatch,
    _adapter: PhantomData<A>,
}
```

Construction order:

1. strict decode to row vectors;
2. validate row-local invariants;
3. build private BTreeMaps with duplicate checks;
4. validate generation/group/launch/Need/observer joins;
5. validate TaskSpec/family/execution/policy and correlations;
6. validate Host state and runtime state one-to-one;
7. validate AwaitMany lengths/cursors/in-flight/output joins;
8. validate Timeout phase/remaining/source observer joins;
9. validate replay cursors/digests;
10. validate replacement mappings;
11. prepare Host restore;
12. run final whole-state invariants;
13. swap complete scheduler state;
14. infallibly commit Host restore.

Any error before step 13 rolls back a prepared Host token and leaves the old
scheduler untouched.

## 8. RuntimeValue snapshot authority

`RuntimeValueSnapshotV1` exhaustively mirrors the final public RuntimeValue enum,
including NeedHandle. It stores exact float bits, integer widths, nominal
owner/layout, opaque class/persistence/payload, Agent variant data and complete
NeedHandle structural evidence.

This snapshot authority is different from canonical value identity:

- Plain SnapshotOnly opaque: both snapshot and canonical identity succeed;
- affine opaque: snapshot may succeed with exact restore owner, canonical
  identity fails;
- NeedHandle: snapshot contains complete structure, canonical identity contains
  NeedId only;
- runtime-local Range/Iterator rows may be saved for runtime continuation but
  producer ownership can still reject them.

## 9. Replay

`TaskReplayEnvelopeV1` stores one generation and an ordered event list. Each
event includes its stored digest. Apply order is:

```text
version/generation
< event digest recomputation
< complete correlation
< normal cursor relation
< normal lifecycle/Need/observer transition
```

Live and replay use the same `EventApplyDelta` implementation. There is no
replay-only relaxed correlation, stale or terminal rule.

## 10. Replacement persistence

Only `ReplacementStateSnapshotV1::Idle` and
`ReplacementStateSnapshotV1::Validated(plan)` persist. A prepared adapter token
is process-local and makes snapshot fail.

Validated task mappings persist old/new complete correlations plus the rebound
NeedHandle. Restore revalidates preservation of NeedId/producer/policy/ordinal
and exact rederivation of the new TaskKey/TaskId.

## 11. First-error precedence

```text
version
< byte/count/depth/canonical-varint
< tag/field form
< key order/duplicate
< fixed identity all-zero
< row-local structure
< digest/identity rederivation
< cross-row joins
< runtime-state invariants
< replay/replacement invariants
< Host quiescence/restore prepare
< final whole-state invariant
< atomic publish
```
