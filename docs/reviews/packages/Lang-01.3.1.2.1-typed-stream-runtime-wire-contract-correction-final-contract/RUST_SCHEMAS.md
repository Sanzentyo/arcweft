# Exact Rust-shaped ownership and wire schemas

This file is normative for type ownership, field names/order, closed enum variants, and
cross-type invariants. It is Rust-shaped contract text, not a patch. Existing Arcweft
owners listed as **reused** remain the source of truth; implementation adds fields or
variants to the owning type instead of introducing extension traits or endpoint helpers.

## 1. Owning modules

| Module/crate owner | Contract-owned responsibility |
| --- | --- |
| `arcweft-lang-sema::callable` | Existing callable parameter semantics, source evidence, and the sole resolver. |
| `arcweft-lang-sema::effects` / `effect_row` | Existing accepted effect IDs/sets and closed effect-row report. |
| `arcweft-core::entry` | General runtime callable parameter/signature projection and general value type contracts. |
| `arcweft-core::plan` | `RuntimePlan`, canonical effect-set table, and Stream definition table. |
| `arcweft-core::stream` | Stream identity, handle, policy, instance table/state, replay, lifecycle, host events/requests, and snapshot types. |
| `arcweft-core::awbc::{schema,fiber,codec,verify,vm,product_step}` | Codec-8 Stream metadata/runtime operations and sole executor state. |
| `arcweft-manifest-model::stream_profile` | Authored fixed-width Stream profile tightening vocabulary only. |
| `arcweft-launch::{manifest,decode,source_map,accepted}` | Existing selected profile field and exact accepted source-span evidence; no policy resolution. |
| `arcweft-compiler::ProjectCompilationContext` | Sole typed cross-crate projection from accepted launch evidence and explicit target into runtime-plan input. |
| `arcweft-runtime-plan::stream_profile` | Built-in target baselines, monotonic profile acceptance, provenance, canonical profile hash, and policy validation input. |
| `arcweft-runtime-plan` | Checked projection from accepted sema/CFG into RuntimePlan and AWBC. |
| `arcweft-bundle` | Bundle schema 6, AWBC-v2 product discriminator, Stream fingerprints. |
| `arcweft-runtime-driver::session_save` | Save schema 2, blockers, generation pins, candidate restore. |
| Native/web/Agent adapters | Pass shared core host JSON bytes; no endpoint DTO ownership. |

## 2. Reused types; no Stream-local copies

```rust
// Existing owners, names illustrative of the already accepted public types.
use arcweft_source::SourceSpan;                    // diagnostics only
use arcweft_core::entry::{
    CallableContractHash,
    RuntimeCallableId,
    RuntimeCallableRole,
    RuntimeCallableExecutable,
    RuntimeFlowExecutable,
    RuntimeTypeSchema,
    TypeLayoutHash,
};
use arcweft_core::value::{
    RuntimeBinding,
    RuntimeExpr,
    RuntimePayload,
    RuntimePattern,
    RuntimeValue,
};
use arcweft_core::awbc::schema::{
    AwbcBlock,
    AwbcBlockId,
    AwbcFrameLayout,
    AwbcFrameLayoutId,
    AwbcFunction,
    AwbcFunctionId,
    AwbcInstruction,
    AwbcMatchArm,
    AwbcPattern,
    AwbcResumePoint,
    AwbcResumePointId,
    AwbcSourceMapId,
    AwbcTerminator,
};
```

`RuntimeSourceMapRef`, `RuntimeStreamFrameLayout`, `RuntimeStreamBinding`,
`RuntimeStreamExpression`, `RuntimeStreamMatchArm`, and a Stream-local CFG are absent.

## 3. Strict decimal JSON newtypes

All Stream-owned integer newtypes below implement the same private serde module:

```rust
// Owner: arcweft-core::stream::json_decimal
// Public types derive Serialize + Deserialize through #[serde(with = ...)] or manual impl.
// Accepted string grammar: "0" | [1-9][0-9]*
// JSON numeric tokens and every noncanonical string are errors.

#[repr(transparent)]
pub struct StreamJsonU32(u32);

#[repr(transparent)]
pub struct StreamJsonU64(u64);
```

The product API does not expose these generic wrappers. Domain newtypes delegate to the
same implementation. Binary AWBC uses fixed-width little-endian integers and does not use
this JSON representation.

## 4. General callable boundary projection

Owner: `arcweft-core::entry`; this extends the owning callable model, not a Stream-local
adapter.

```rust
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeParameterIndex(pub u32);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeParameterName(Box<str>);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeParameterPassing {
    PositionalOnly,
    PositionalOrNamed,
    NamedOnly,
    RestPositional,
    RestNamed,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeParameterPresence {
    Required,
    Optional,
    Defaulted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeValueTypeContract {
    pub layout: TypeLayoutHash,
    pub schema: RuntimeTypeSchema,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCallableParameter {
    pub index: RuntimeParameterIndex,
    pub name: Option<RuntimeParameterName>,
    pub ty: RuntimeValueTypeContract,
    pub passing: RuntimeParameterPassing,
    pub presence: RuntimeParameterPresence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCallableBoundarySignature {
    pub callable: RuntimeCallableId,
    pub contract: CallableContractHash,
    pub parameters: Vec<RuntimeCallableParameter>,
    pub result: RuntimeValueTypeContract,
    pub effects: RuntimeEffectSetId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeResolvedArguments<T> {
    pub values: Vec<RuntimeResolvedArgument<T>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeResolvedArgument<T> {
    Value { value: T },
    Omitted,
}

pub type RuntimePlanResolvedArguments = RuntimeResolvedArguments<RuntimeValue>;
pub type RuntimeHostResolvedArguments = RuntimeResolvedArguments<RuntimePayload>;
```

Invariants:

1. parameter indices are exactly `0..len`;
2. names are absent only where the accepted catalog has no name;
3. only `Optional` may map to `Omitted`;
4. `Defaulted` is always materialized as `Value` before RuntimePlan/AWBC lowering;
5. `RestPositional` has one sequence value; `RestNamed` has one canonical map value;
6. external-host arguments must be `RuntimePayload`-eligible and contain no affine handle;
7. call-site lowering has no name binding or default evaluator beyond the existing shared
   resolver/checker product.

Projection errors are owned by `arcweft-runtime-plan` because they retain `SourceSpan`:

```rust
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimeCallableProjectionError {
    #[error("callable projection does not match the accepted resolver identity")]
    ResolverIdentity { range: SourceSpan },
    #[error("callable projection parameter count does not match accepted evidence")]
    ParameterCount { range: SourceSpan },
    #[error("callable projection parameter index is not declaration-order canonical")]
    ParameterIndex { range: SourceSpan },
    #[error("callable projection metadata differs from accepted parameter evidence")]
    ParameterMetadata { range: SourceSpan },
    #[error("required parameter has no resolved value")]
    MissingRequired { range: SourceSpan },
    #[error("defaulted parameter was not materialized by accepted call lowering")]
    DefaultNotMaterialized { range: SourceSpan },
    #[error("only an optional parameter may be omitted")]
    InvalidOmitted { range: SourceSpan },
    #[error("rest parameter has the wrong canonical container")]
    RestShape { range: SourceSpan },
    #[error("external Stream argument is not host-payload eligible")]
    HostPayloadIneligible { range: SourceSpan },
}
```

## 5. Effect-set RuntimePlan owner

Owner: `arcweft-core::plan`.

```rust
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeEffectSetId(pub u32);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeEffectId(Box<str>);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeEffectSet {
    pub effects: Vec<RuntimeEffectId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeEffectSetTable {
    pub sets: Vec<RuntimeEffectSet>,
}
```

`sets[0]` is empty. Member vectors and the table are canonical as defined in
`FINAL_CONTRACT.md`. There is no other RuntimePlan effect-set map.

## 6. Definition identity and origin

Owner: `arcweft-core::stream`.

```rust
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeStreamDefinitionId(pub u32);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeStreamDefinitionKey(pub [u8; 32]);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeStreamPublicId(Box<str>);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeExternalModuleId(Box<str>);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeExternalModuleAbiHash(pub [u8; 32]);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeCapabilityId(Box<str>);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeOperationId(Box<str>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeExternalStreamOrigin {
    pub module: RuntimeExternalModuleId,
    pub module_abi: RuntimeExternalModuleAbiHash,
    pub capability: RuntimeCapabilityId,
    pub operation: RuntimeOperationId,
    pub effects: RuntimeEffectSetId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeStreamOrigin {
    AuthoredGenerator {
        producer: RuntimeCallableRole,
    },
    External(RuntimeExternalStreamOrigin),
    Derived {
        producer: RuntimeCallableRole,
    },
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StreamHandleLayoutHash(pub [u8; 32]);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StreamStateLayoutHash(pub [u8; 32]);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StreamProducerCodeHash(pub [u8; 32]);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StreamGeneratorFrameContractHash(pub [u8; 32]);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStreamDefinition {
    pub id: RuntimeStreamDefinitionId,
    pub key: RuntimeStreamDefinitionKey,
    pub public_id: RuntimeStreamPublicId,
    pub item: RuntimeValueTypeContract,
    pub error: RuntimeValueTypeContract,
    pub callable: RuntimeCallableBoundarySignature,
    pub effects: RuntimeEffectSetId,
    pub origin: RuntimeStreamOrigin,
    pub policy: ResolvedStreamPolicy,
    pub handle_layout: StreamHandleLayoutHash,
    pub state_layout: StreamStateLayoutHash,
    pub producer_code: Option<StreamProducerCodeHash>,
    pub generator_frame: Option<StreamGeneratorFrameContractHash>,
}
```

Origin invariants:

- `AuthoredGenerator` and `Derived` reference an existing callable executable classified
  `Generator` and a codec-8 function kind `GeneratorProducer`;
- `External` references no producer function and its `effects` equals the definition and
  callable-signature effect ID;
- pass-through ordinary functions return an existing handle and do not create a definition;
- `public_id` and origin IDs are accepted typed IDs, never parsed debug strings;
- source maps/ranges and table indices do not participate in `key`.

## 7. RuntimePlan exact shape and ordering

Owner: existing `arcweft-core::plan::RuntimePlan`.

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimePlan {
    pub entries: Vec<RuntimeEntry>,                         // existing
    pub callable_executables: Vec<RuntimeCallableExecutable>, // existing
    pub flow_executables: Vec<RuntimeFlowExecutable>,      // existing
    pub flows: Vec<FlowPlan>,                              // existing
    pub pure_helpers: Vec<PureHelperPlan>,                 // existing
    pub trait_methods: Vec<TraitMethodPlan>,               // existing
    pub line_task_groups: Vec<LineTaskGroup>,              // existing
    pub stream_profile: RuntimeStreamProfileEvidence,       // new resolved evidence
    pub effect_sets: RuntimeEffectSetTable,                 // new sole owner
    pub stream_definitions: Vec<RuntimeStreamDefinition>,   // corrected sole table
}
```

There is no `stream_plans` or `source_plans`. Existing first-seven table ownership and
canonical rules remain. `stream_profile` is produced once by the typed compiler/runtime-
plan boundary and contains no manifest name or source range. Effect sets are canonicalized
before Stream definition IDs are assigned. Stream definitions are sorted by `key`; `id.0`
equals the resulting index. Every reference is checked before the plan is accepted.

## 8. Authored, target, and resolved policy types

Owner of authored/resolved per-definition policy: `arcweft-core::stream`. Authored
project-profile vocabulary, accepted source evidence, compiler projection, and runtime-plan
profile acceptance have the separate exact owners in section 21.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamBackpressure {
    LatestOnly,
    Bounded {
        capacity_items: StreamQueueItemLimit,
        capacity_bytes: StreamByteLimit,
        overflow: StreamOverflow,
    },
    ProviderBlocking {
        capacity_items: StreamQueueItemLimit,
        capacity_bytes: StreamByteLimit,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamOverflow {
    DropOldest,
    DropNewest,
    TerminalError,
    Coalesce { reducer: RuntimeCallableRole },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StreamReplayMode {
    Full,
    HashOnly,
    Summary,
    EventOnly,
    None,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StreamPrivacy {
    Transient,
    Redacted,
    Recordable,
    Private,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StreamPermissionRule {
    AtOpen,
    OnRestart,
    EachEvent,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StreamConsumerDropPolicy {
    DiscardQueued,
    DrainAndRetainReplay,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StreamReplayRetention {
    UntilConsumerDrop,
    ThroughTombstone,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StreamReplayDataClass {
    EventOnly,
    Summary,
    Digest,
    Payload,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StreamRestartRule {
    Deny,
    SameProvider,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StreamProviderReplacementRule {
    Deny,
    SameOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredStreamPolicy {
    pub backpressure: StreamBackpressure,
    pub replay: StreamReplayMode,
    pub privacy: StreamPrivacy,
    pub permission: StreamPermissionRule,
    pub consumer_drop: StreamConsumerDropPolicy,
    pub replay_retention: StreamReplayRetention,
    pub terminal_error_replay: StreamReplayDataClass,
    pub restart: StreamRestartRule,
    pub provider_replacement: StreamProviderReplacementRule,
    pub requested_limits: StreamRequestedLimits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamRequestedLimits {
    pub max_queue_items: Option<StreamQueueItemLimit>,
    pub max_queue_bytes: Option<StreamByteLimit>,
    pub max_replay_records: Option<StreamReplayRecordLimit>,
    pub max_replay_payload_bytes: Option<StreamByteLimit>,
    pub max_replay_total_bytes: Option<StreamByteLimit>,
    pub max_lifetime_events: Option<StreamCounterLimit>,
    pub max_lifetime_items: Option<StreamCounterLimit>,
    pub max_recoverable_errors: Option<StreamCounterLimit>,
    pub max_progress_events: Option<StreamCounterLimit>,
    pub max_deliveries: Option<StreamCounterLimit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedStreamPolicy {
    pub backpressure: StreamBackpressure,
    pub replay: StreamReplayMode,
    pub privacy: StreamPrivacy,
    pub permission: StreamPermissionRule,
    pub consumer_drop: StreamConsumerDropPolicy,
    pub replay_retention: StreamReplayRetention,
    pub terminal_error_replay: StreamReplayDataClass,
    pub restart: StreamRestartRule,
    pub provider_replacement: StreamProviderReplacementRule,
    pub limits: ResolvedStreamLimits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedStreamLimits {
    pub max_queue_items: StreamQueueItemLimit,
    pub max_queue_bytes: StreamByteLimit,
    pub max_item_bytes: StreamByteLimit,
    pub max_replay_records: StreamReplayRecordLimit,
    pub max_replay_payload_bytes: StreamByteLimit,
    pub max_replay_total_bytes: StreamByteLimit,
    pub max_lifetime_events: StreamCounterLimit,
    pub max_lifetime_items: StreamCounterLimit,
    pub max_recoverable_errors: StreamCounterLimit,
    pub max_progress_events: StreamCounterLimit,
    pub max_deliveries: StreamCounterLimit,
    pub max_restart_attempts: StreamCounterLimit,
    pub max_provider_replacements: StreamCounterLimit,
}
```

Default authored policy preserves current accepted Source-policy behavior without keeping
Source types: LatestOnly, EventOnly, Transient, AtOpen, DiscardQueued,
UntilConsumerDrop, terminal EventOnly, restart Deny, replacement Deny, capacity one.

## 9. Domain integer and digest newtypes

All integer types below are transparent over `u32` or `u64`, derive only the traits needed
by their owner, use checked constructors/advancement, and use canonical decimal-string
JSON at host/save boundaries.

```rust
pub struct StreamGeneration(pub u64);
pub struct StreamInstanceOrdinal(pub u64);
pub struct StreamConsumerLease(pub u64);
pub struct StreamProducerLease(pub u64);
pub struct StreamEventSequence(pub u64);
pub struct StreamCommitSequence(pub u64);
pub struct StreamDeliveryId(pub u64);
pub struct StreamReplayRecordId(pub u64);
pub struct StreamTombstoneOrdinal(pub u64);
pub struct StreamRequestId(pub u64);
pub struct StreamProviderAttempt(pub u64);
pub struct StreamCount(pub u64);
pub struct StreamByteCount(pub u64);
pub struct StreamCounterLimit(pub u64);
pub struct StreamByteLimit(pub u64);
pub struct StreamQueueItemLimit(pub u32);
pub struct StreamReplayRecordLimit(pub u32);
pub struct RuntimeFiberId(pub u64); // owner: existing awbc::fiber module, not Stream-local
```

Limits are inclusive lifetime maxima. Queue/replay limits may be zero only where explicitly
permitted: replay records/bytes may be zero; queue item/byte and item byte limits must be
nonzero. `RuntimeFiberId(0)` is root; child IDs are monotonically allocated and never
reused within the executor snapshot.

## 10. Instance key and affine handle

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StreamInstanceKey {
    pub definition_key: RuntimeStreamDefinitionKey,
    pub generation: StreamGeneration,
    pub ordinal: StreamInstanceOrdinal,
}

#[derive(Debug, Eq, PartialEq)]
pub struct StreamHandle {
    pub key: StreamInstanceKey,
    pub item_layout: TypeLayoutHash,
    pub error_layout: TypeLayoutHash,
    pub lease: StreamConsumerLease,
}
```

`StreamHandle` has no language-visible `Clone` or `Copy`. Internal transaction/snapshot
cloning of an enclosing `RuntimeValue` does not create a second runnable owner: original
and candidate executor states are mutually exclusive, and candidate validation requires
one current lease occurrence before commit. VM `Move`, aggregate construction/destruction,
call argument transfer, return, child-fiber transfer, and `Drop` all use affine-aware
move APIs on the owning `RuntimeValue` enum.

## 11. Sole table and allocation cursors

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct StreamInstanceTable {
    pub next_instance_ordinal: StreamInstanceOrdinal,
    pub next_consumer_lease: StreamConsumerLease,
    pub next_producer_lease: StreamProducerLease,
    pub next_request_id: StreamRequestId,
    pub next_tombstone_ordinal: StreamTombstoneOrdinal,
    pub total_queue_bytes: StreamByteCount,
    pub total_replay_bytes: StreamByteCount,
    pub entries: BTreeMap<StreamInstanceKey, StreamInstanceEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StreamInstanceEntry {
    Live(StreamInstanceState),
    Tombstone(StreamTombstone),
}
```

External instance creation reserves two consecutive request IDs: one open ID and one close
ID. If either ID, the instance ordinal, consumer lease, producer lease, profile capacity,
or generation pin cannot be allocated, creation fails before inserting an entry or
spawning a fiber.

## 12. Consumer and producer ownership

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamConsumerState {
    Owned {
        fiber: RuntimeFiberId,
        lease: StreamConsumerLease,
    },
    Dropped {
        final_lease: StreamConsumerLease,
        cleanup: StreamConsumerCleanupState,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamConsumerCleanupState {
    Pending,
    Complete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProducerStreamRef {
    pub key: StreamInstanceKey,
    pub lease: StreamProducerLease,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamProducerOwner {
    Fiber {
        fiber: RuntimeFiberId,
        lease: StreamProducerLease,
    },
    External {
        open_request: StreamRequestId,
        reserved_close_request: StreamRequestId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamProducerState {
    LocalRunning,
    External(StreamExternalLifecycle),
    Stopped(StreamProducerStopReason),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamExternalLifecycle {
    OpenRequested,
    Open {
        provider: StreamProviderInstanceId,
    },
    Disconnected {
        provider: StreamProviderInstanceId,
        next_attempt: StreamProviderAttempt,
    },
    RestartRequested {
        previous: StreamProviderInstanceId,
        attempt: StreamProviderAttempt,
    },
    Closing {
        provider: Option<StreamProviderInstanceId>,
    },
    Closed,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StreamProviderInstanceId(Box<str>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamProducerStopReason {
    Completed,
    Failed,
    Cancelled,
    RuntimeLimit(StreamRuntimeLimit),
    PermissionRevoked,
    Disconnected,
    ProviderReplaced,
}
```

`FiberState` receives exactly these narrow additions/removals:

```rust
pub struct FiberState {
    pub id: RuntimeFiberId,                    // new general fiber identity
    // existing generation, entry, cursor, frames, status, suspension,
    // terminal, budget, line cursor remain authoritative
    pub producer_stream: Option<ProducerStreamRef>, // new reciprocal reference
    // REMOVE sources: Vec<FiberSourceState>
    // REMOVE streams: Vec<FiberStreamState>
}
```

## 13. Sequence, queue, counters, close, and terminal

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamSequenceCursor {
    Next(StreamEventSequence),
    Exhausted { last: StreamEventSequence },
}

#[derive(Clone, Debug, PartialEq)]
pub struct StreamQueuedDelivery {
    pub id: StreamDeliveryId,
    pub commit: StreamCommitSequence,
    pub ingress: Option<StreamEventSequence>,
    pub body: StreamQueuedDeliveryBody,
    pub canonical_bytes: StreamByteCount,
    pub replay_record: Option<StreamReplayRecordId>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StreamQueuedDeliveryBody {
    Item(RuntimePayload),
    RecoverableError(RuntimePayload),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamLifetimeCounters {
    pub consumed_envelopes: StreamCount,
    pub accepted_items: StreamCount,
    pub accepted_recoverable_errors: StreamCount,
    pub progress_events: StreamCount,
    pub delivered_items: StreamCount,
    pub delivered_recoverable_errors: StreamCount,
    pub overflow_drop_oldest: StreamCount,
    pub overflow_drop_newest: StreamCount,
    pub overflow_coalesced: StreamCount,
    pub restarts: StreamCount,
    pub provider_replacements: StreamCount,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamCloseRequestState {
    NotRequested { reserved: Option<StreamRequestId> },
    Requested {
        id: StreamRequestId,
        reason: StreamCloseReason,
    },
    Acknowledged {
        id: StreamRequestId,
    },
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamCloseReason {
    ConsumerDropped,
    Completed,
    TerminalError,
    RuntimeLimit,
    Cancelled,
    PermissionRevoked,
    Disconnected,
    ProviderReplacement,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StreamTerminalState {
    pub commit: StreamCommitSequence,
    pub ingress: Option<StreamEventSequence>,
    pub reason: StreamTerminalReason,
    pub observed_by_consumer: bool,
    pub result_emitted: bool,
    pub observation_emitted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StreamTerminalReason {
    End,
    Error {
        payload: Option<RuntimePayload>,
        marker: StreamErrorMarker,
    },
    RuntimeLimit(StreamRuntimeLimit),
    Cancelled,
    Disconnected,
    PermissionRevoked,
    ProviderReplaced,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamTombstoneTerminal {
    pub commit: StreamCommitSequence,
    pub ingress: Option<StreamEventSequence>,
    pub reason: StreamTerminalReasonCode,
    pub result_emitted: bool,
    pub observation_emitted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamTerminalReasonCode {
    End,
    Error,
    RuntimeLimit(StreamRuntimeLimit),
    Cancelled,
    Disconnected,
    PermissionRevoked,
    ProviderReplaced,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamRuntimeLimit {
    EventSequence,
    CommitSequence,
    LifetimeEvents,
    LifetimeItems,
    RecoverableErrors,
    ProgressEvents,
    ReplayRecordId,
    ReplayStatistics,
    QueueBytes,
    DeliveryInvariant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamErrorMarker {
    pub commit: StreamCommitSequence,
    pub class: StreamErrorClass,
    pub replay_record: Option<StreamReplayRecordId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamErrorClass {
    Recoverable,
    Terminal,
}
```

A live terminal error owns its payload only in `StreamTerminalReason::Error.payload`.
The first live-consumer terminal observation moves that payload into the returned
`TerminalError(E)` value, sets the field to `None`, and marks result emission. Dropped-
consumer cleanup drops it. A payload-retaining replay record is created independently
only when Recordable policy permits. Tombstoning is legal only after the live error
payload is `None`; the tombstone stores the payload-free reason code and error marker/
replay reference, never the terminal payload.

## 14. Replay exact types

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct StreamReplayStore {
    pub next_record_id: StreamReplayRecordId,
    pub records: VecDeque<StreamReplayRecord>,
    pub retained_payload_bytes: StreamByteCount,
    pub retained_total_bytes: StreamByteCount,
    pub stored_records: StreamCount,
    pub evicted_records: StreamCount,
    pub skipped_records: StreamCount,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StreamReplayDecision {
    Record(StreamReplayRecord),
    NoRecord(StreamReplayNoRecord),
}

#[derive(Clone, Debug, PartialEq)]
pub struct StreamReplayRecord {
    pub id: StreamReplayRecordId,
    pub commit: StreamCommitSequence,
    pub ingress: Option<StreamEventSequence>,
    pub event: StreamReplayEventKind,
    pub body: StreamReplayRecordBody,
    pub accounted_bytes: StreamByteCount,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StreamReplayRecordBody {
    Payload {
        payload: RuntimePayload,
        canonical_bytes: StreamByteCount,
    },
    Digest {
        algorithm: StreamReplayHashAlgorithm,
        domain: StreamReplayHashDomain,
        canonical_bytes: StreamByteCount,
        digest: [u8; 32],
    },
    Summary(StreamReplaySummary),
    EventOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamReplayHashAlgorithm {
    Blake3_256,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamReplayHashDomain {
    Item,
    RecoverableError,
    Progress,
    TerminalError,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamReplaySummary {
    pub type_layout: TypeLayoutHash,
    pub canonical_bytes: StreamByteCount,
    pub shape: StreamReplayShape,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamReplayShape {
    Unit,
    Scalar { class: StreamScalarClass, bits: u16 },
    Utf8 { bytes: StreamByteCount, scalars: StreamCount },
    Bytes { bytes: StreamByteCount },
    Tuple { arity: StreamCount },
    Sequence { items: StreamCount },
    Record { fields: StreamCount },
    Variant { ordinal: u32, has_payload: bool },
    Matrix { rows: StreamCount, columns: StreamCount },
    Tensor { rank: StreamCount, elements: StreamCount },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamScalarClass {
    Bool,
    Signed,
    Unsigned,
    Float,
    Char,
    Duration,
    EntityRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamReplayEventKind {
    Opened,
    Progress,
    Item,
    RecoverableError,
    End,
    TerminalError,
    Disconnected,
    PermissionRevoked,
    CloseAcknowledged,
    Restarted,
    ProviderReplaced,
    RuntimeLimit,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamReplayNoRecord {
    pub event: StreamReplayEventKind,
    pub reason: StreamReplayNoRecordReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamReplayNoRecordReason {
    PolicyNone,
    Transient,
    Private,
    LimitZero,
    RecordTooLarge,
    TerminalCap,
}
```

The summary's `u16` and `u32` fields also serialize as decimal strings in Stream host/save
JSON; they are fixed integers in the replay binary transcript.

## 15. Sole live state and tombstone

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct StreamInstanceState {
    pub key: StreamInstanceKey,
    pub definition: RuntimeStreamDefinitionId,
    pub item_layout: TypeLayoutHash,
    pub error_layout: TypeLayoutHash,
    pub policy: ResolvedStreamPolicy,
    pub producer_owner: StreamProducerOwner,
    pub producer: StreamProducerState,
    pub consumer: StreamConsumerState,
    pub external_sequence: Option<StreamSequenceCursor>,
    pub next_commit: StreamCommitSequence,
    pub next_delivery_id: StreamDeliveryId,
    pub queue: VecDeque<StreamQueuedDelivery>,
    pub queue_bytes: StreamByteCount,
    pub replay: StreamReplayStore,
    pub counters: StreamLifetimeCounters,
    pub terminal: Option<StreamTerminalState>,
    pub close: StreamCloseRequestState,
    pub last_error: Option<StreamErrorMarker>,
    pub terminal_pending_tombstone: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StreamTombstone {
    pub key: StreamInstanceKey,
    pub definition: RuntimeStreamDefinitionId,
    pub item_layout: TypeLayoutHash,
    pub error_layout: TypeLayoutHash,
    pub consumer: StreamTombstoneConsumer,
    pub terminal: StreamTombstoneTerminal,
    pub close: StreamCloseRequestState,
    pub replay: StreamReplayStore,
    pub counters: StreamLifetimeCounters,
    pub last_error: Option<StreamErrorMarker>,
    pub ordinal: StreamTombstoneOrdinal,
    pub pins_generation: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamTombstoneConsumer {
    ClosedHandle {
        fiber: RuntimeFiberId,
        lease: StreamConsumerLease,
    },
    Dropped {
        final_lease: StreamConsumerLease,
    },
}
```

Live-state invariants include: key/definition-key match; queue byte sum; strictly increasing
delivery IDs/commit order; queue payload type match; replay counters/limits; sequence and
counter bounds; one current consumer; reciprocal producer; terminal/producer/close
compatibility; queue-before-terminal for a live consumer; and no terminal payload retained
against privacy/cap.

Tombstones contain no delivery queue, live producer, or terminal payload. A closed-handle
tombstone is legal only after the live consumer has received the one terminal outcome;
subsequent `NextStream` calls return `Closed(StreamTerminalReasonCode)` idempotently until
the affine handle is dropped. A dropped tombstone records the payload-free reason and is
releasable when close is settled and replay retention/eviction releases its pin.

## 16. Shared host request, event, outcome, and observation schema

Owner: `arcweft-core::stream`; the existing `RuntimeStepInput`, `RuntimeStepOutput`,
`RuntimeEffectBatch`, and `HostRequestBatch` are changed in place. There is no endpoint
DTO or adapter-specific representation.

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeStreamEvent {
    pub instance: StreamInstanceKey,
    pub sequence: StreamEventSequence,
    pub kind: RuntimeStreamEventKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeStreamEventKind {
    Opened {
        open_request: StreamRequestId,
        provider: StreamProviderInstanceId,
    },
    Progress { progress: StreamProgress },
    Item {
        type_layout: TypeLayoutHash,
        payload: RuntimePayload,
    },
    RecoverableError {
        type_layout: TypeLayoutHash,
        error: RuntimePayload,
    },
    End,
    TerminalError {
        type_layout: TypeLayoutHash,
        error: RuntimePayload,
    },
    Disconnected {
        provider: StreamProviderInstanceId,
        reason: StreamDisconnectReason,
    },
    PermissionRevoked { permission: RuntimePermissionId },
    CloseAcknowledged { close_request: StreamRequestId },
    Restarted {
        open_request: StreamRequestId,
        provider: StreamProviderInstanceId,
        attempt: StreamProviderAttempt,
    },
    ProviderReplaced {
        previous: StreamProviderInstanceId,
        replacement: StreamProviderInstanceId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamProgress {
    pub phase: StreamProgressPhase,
    pub completed: Option<StreamCount>,
    pub total: Option<StreamCount>,
    pub unit: StreamProgressUnit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamProgressPhase { Starting, Running, Paused, Retrying }

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamProgressUnit { Items, Bytes, Steps }

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamDisconnectReason { Provider, Transport, Shutdown }

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuntimePermissionId(Box<str>);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeStreamRequest {
    Open {
        request: StreamRequestId,
        instance: StreamInstanceKey,
        definition: RuntimeStreamDefinitionKey,
        module: RuntimeExternalModuleId,
        module_abi: RuntimeExternalModuleAbiHash,
        capability: RuntimeCapabilityId,
        operation: RuntimeOperationId,
        arguments: RuntimeHostResolvedArguments,
        item_layout: TypeLayoutHash,
        error_layout: TypeLayoutHash,
        policy: ResolvedExternalStreamPolicy,
    },
    Close {
        request: StreamRequestId,
        instance: StreamInstanceKey,
        reason: StreamCloseReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedExternalStreamPolicy {
    pub backpressure: StreamBackpressure,
    pub replay: StreamReplayMode,
    pub privacy: StreamPrivacy,
    pub permission: StreamPermissionRule,
    pub consumer_drop: StreamConsumerDropPolicy,
    pub replay_retention: StreamReplayRetention,
    pub terminal_error_replay: StreamReplayDataClass,
    pub restart: StreamRestartRule,
    pub provider_replacement: StreamProviderReplacementRule,
    pub limits: ResolvedStreamLimits,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeStreamEventOutcome {
    pub instance: StreamInstanceKey,
    pub sequence: StreamEventSequence,
    pub disposition: RuntimeStreamEventDisposition,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeStreamEventDisposition {
    Accepted { commit: StreamCommitSequence },
    Terminalized {
        commit: StreamCommitSequence,
        limit: StreamRuntimeLimit,
    },
    Rejected { reason: StreamEventRejection },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamEventRejection {
    Malformed,
    BatchLimit,
    BatchDuplicate,
    UnknownInstance,
    WrongDefinition,
    StaleGeneration,
    Terminal,
    DuplicateSequence,
    RetrogradeSequence,
    SequenceGap,
    WrongLifecycle,
    WrongRequest,
    WrongType,
    PayloadTooLarge,
    InvalidProgress,
    PermissionMismatch,
    RestartForbidden,
    ProviderReplacementForbidden,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeStreamObservation {
    pub instance: StreamInstanceKey,
    pub commit: Option<StreamCommitSequence>,
    pub kind: RuntimeStreamObservationKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeStreamObservationKind {
    Opened { provider: StreamProviderInstanceId },
    Progress { progress: StreamProgress },
    Overflow {
        action: StreamOverflowAction,
        deliveries: StreamCount,
    },
    Restarted {
        provider: StreamProviderInstanceId,
        attempt: StreamProviderAttempt,
    },
    ProviderReplaced {
        previous: StreamProviderInstanceId,
        replacement: StreamProviderInstanceId,
    },
    Terminal { reason: StreamTerminalObservationReason },
    ReplayEvicted {
        records: StreamCount,
        bytes: StreamByteCount,
    },
    ConsumerDropped {
        discarded_deliveries: StreamCount,
        replay_records_added: StreamCount,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamOverflowAction { DroppedOldest, DroppedNewest, Coalesced, Terminalized }

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamTerminalObservationReason {
    End,
    Error,
    RuntimeLimit,
    Cancelled,
    Disconnected,
    PermissionRevoked,
    ProviderReplaced,
}
```

The exact existing boundary changes are:

```rust
pub struct RuntimeStepInput {
    // tick, dt, bindings, input_events, task_events, audio_events unchanged
    pub stream_events: Vec<RuntimeStreamEvent>, // replaces source_events
    // host_call_results, root_events, deferred_root_events unchanged
}

pub struct RuntimeStepOutput {
    // diagnostics, flow_events, effects, requests, root fields unchanged
    pub stream_event_outcomes: Vec<RuntimeStreamEventOutcome>,
}

pub struct RuntimeEffectBatch {
    pub line: Vec<LineEffectRequest>,
    pub stream_observations: Vec<RuntimeStreamObservation>,
    // remove source_events and old stream_events
}

pub struct HostRequestBatch {
    // tasks, audio, cancel_scopes, content, host calls, root events unchanged
    pub stream_requests: Vec<RuntimeStreamRequest>,
    // remove source_close
}
```

`stream_event_outcomes` contains exactly one result for each supplied event, in normalized
partition order and original-event identity. `stream_observations` contains runtime
control/telemetry observations only; it is not application iteration data and never
repeats an item/error payload. Open/close requests share one vector and one JSON codec.

New Stream-owned digest fields in these host/save structs use the shared
`arcweft-core::stream::json::digest32_hex` field codec over the existing digest newtypes;
this does not change their internal owner or global serde. It accepts/emits exactly 64
lowercase hexadecimal characters. Every Stream-owned integer uses the decimal-string
codec from section 3.

Step statistics add one nested `RuntimeStreamStepStats`; all fields are domain newtypes,
not `usize`:

```rust
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeStreamStepStats {
    pub events_in: StreamCount,
    pub events_accepted: StreamCount,
    pub events_rejected: StreamCount,
    pub items_enqueued: StreamCount,
    pub recoverable_errors_enqueued: StreamCount,
    pub deliveries: StreamCount,
    pub progress_observations: StreamCount,
    pub instances_created: StreamCount,
    pub instances_tombstoned: StreamCount,
    pub instances_released: StreamCount,
    pub queue_bytes_added: StreamByteCount,
    pub queue_bytes_released: StreamByteCount,
    pub replay_records_added: StreamCount,
    pub replay_records_evicted: StreamCount,
    pub replay_bytes_retained: StreamByteCount,
    pub close_requests_emitted: StreamCount,
}
```

## 17. Transition staging

`StagedStreamPartitionTransition` is an internal owner type in
`arcweft-core::engine::stream`. It contains the candidate entry/table-cursor deltas,
emitted requests, outcomes, observations, statistics, and optional producer-child-fiber
transition. Construction performs all checked arithmetic and validation; commit performs
one replacement of the sole table entry plus the already staged scheduler/output deltas.
A rejected partition contains no candidate delta. The type is not serialized and is not a
sidecar authority.

## 18. AWBC codec-8 schemas

Owner: existing `arcweft-core::awbc::schema` types are changed in place.

```rust
pub struct AwbcParameter {
    pub name: Option<AwbcStringId>,
    pub ty: AwbcTypeId,
    pub passing: AwbcParameterPassing,
    pub presence: AwbcParameterPresence,
}

pub enum AwbcParameterPassing {
    PositionalOnly,
    PositionalOrNamed,
    NamedOnly,
    RestPositional,
    RestNamed,
}

pub enum AwbcParameterPresence { Required, Optional, Defaulted }

pub struct AwbcSignature {
    pub params: Vec<AwbcParameter>, // replaces Vec<AwbcTypeId>
    pub result: Option<AwbcTypeId>,
    pub effects: AwbcEffectSetId,
}

pub enum AwbcResolvedArgument {
    Value(AwbcRegisterId),
    Omitted,
}

pub enum AwbcRuntimeType {
    // existing variants and tags unchanged
    StreamHandle { item: AwbcTypeId, error: AwbcTypeId }, // tag 21
}

pub struct AwbcStreamDefinition {
    pub public_id: AwbcStringId,
    pub semantic_key: AwbcDigest,
    pub item_type: AwbcTypeId,
    pub error_type: AwbcTypeId,
    pub signature: AwbcSignatureId,
    pub effects: AwbcEffectSetId,
    pub origin: AwbcStreamOrigin,
    pub policy: AwbcStreamPolicy,
    pub handle_layout: AwbcDigest,
    pub state_layout: AwbcDigest,
    pub producer_code: Option<AwbcDigest>,
    pub generator_frame: Option<AwbcDigest>,
}

pub enum AwbcStreamOrigin {
    External {
        module: AwbcStringId,
        module_abi: AwbcDigest,
        capability: AwbcStringId,
        operation: AwbcStringId,
    },
    AuthoredGenerator { producer: AwbcFunctionId },
    Derived { producer: AwbcFunctionId },
}

pub struct AwbcStreamPolicy {
    pub backpressure: AwbcStreamBackpressure,
    pub replay: AwbcStreamReplayMode,
    pub privacy: AwbcStreamPrivacy,
    pub permission: AwbcStreamPermissionRule,
    pub consumer_drop: AwbcStreamConsumerDropPolicy,
    pub replay_retention: AwbcStreamReplayRetention,
    pub terminal_error_replay: AwbcStreamReplayDataClass,
    pub restart: AwbcStreamRestartRule,
    pub provider_replacement: AwbcStreamProviderReplacementRule,
    pub limits: AwbcResolvedStreamLimits,
}

pub enum AwbcInstruction {
    // existing nonremoved instructions
    OpenStream {
        dst: AwbcRegisterId,
        definition: AwbcStreamDefinitionId,
        args: Vec<AwbcResolvedArgument>,
    },
    FinishStream {
        stream: AwbcRegisterId,
        outcome: AwbcStreamProducerOutcome,
    },
}

pub enum AwbcStreamProducerOutcome {
    Complete,
    Fail { error: AwbcRegisterId },
    Cancelled,
}

pub enum AwbcTerminator {
    // existing terminators
    NextStream {
        stream: AwbcRegisterId,
        dst: AwbcRegisterId,
        resume: AwbcResumePointId,
        ready: AwbcBlockId,
    },
    YieldStream {
        stream: AwbcRegisterId,
        value: AwbcRegisterId,
        resume: AwbcResumePointId,
        continuation: AwbcBlockId,
    },
}
```

`NextStream` writes the existing-owner compiler-created ordinary variant type:

```rust
pub enum RuntimeStreamNextOutcome<T, E> {
    Item(T),
    RecoverableError(E),
    End,
    TerminalError(E),
    RuntimeLimit(StreamRuntimeLimit),
    Cancelled,
    Disconnected,
    PermissionRevoked,
    ProviderReplaced,
    Closed(StreamTerminalReasonCode),
}
```

Queued Item/RecoverableError values are returned first. The first terminal observation
returns the exact non-Closed terminal case and consumes any terminal error payload; every
later call through a retained closed handle returns `Closed(reason_code)`. Existing
pattern/match CFG selects one path. If no delivery/terminal is ready, `NextStream`
suspends at `StreamNext` and retries on resume. `YieldStream` stages local item acceptance
and suspends the producer at `StreamYield`; its continuation runs only after the scheduler
resumes that producer. Existing `Move` and `Drop` own affine transfer/drop behavior; no
redundant Stream move/drop opcode exists.

`AwbcFunctionKind` keeps existing unrelated tags 0,1,2,6,7 and adds `Ordinary=8`,
`GeneratorProducer=9`; codec 8 treats 3,4,5 as unknown. `GeneratorProducer` requires
`MAY_SUSPEND` and the next unused `OWNS_STREAM_PRODUCER` flag bit. Exact numeric tags are
in `WIRE_AND_VERSION_ALLOCATIONS.md`.

## 19. Executor and snapshot shape

The product executor snapshot replaces `stream_sequences` and every compact/facade Stream
shape with one table snapshot:

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductExecutorSnapshot {
    pub fiber: FiberState,
    pub child_fibers: Vec<FiberState>,
    pub next_fiber_id: RuntimeFiberId,
    pub streams: StreamInstanceTableSnapshot,
    // Existing non-Stream fields remain in their current declaration order:
    pub entry_bound: bool,
    pub active_dialogue: Option<AwbcProductActiveDialogueSnapshot>,
    pub active_choice: Option<AwbcProductActiveChoiceSnapshot>,
    pub pending_host_call: Option<AwbcProductPendingHostCallSnapshot>,
    pub started_tasks: BTreeSet<TaskId>,
    pub emitted_content: BTreeSet<AwbcContentUnitId>,
    pub next_generation: StreamGeneration,
    pub next_host_call_sequence: u64, // non-Stream existing field; current codec retained
    pub next_audio_sequence: u64,     // non-Stream existing field; current codec retained
    pub compact_pure_stats: RuntimePureCallStats,
    pub observations: RuntimeObservationState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamInstanceTableSnapshot {
    pub next_instance_ordinal: StreamInstanceOrdinal,
    pub next_consumer_lease: StreamConsumerLease,
    pub next_producer_lease: StreamProducerLease,
    pub next_request_id: StreamRequestId,
    pub next_tombstone_ordinal: StreamTombstoneOrdinal,
    pub total_queue_bytes: StreamByteCount,
    pub total_replay_bytes: StreamByteCount,
    pub entries: Vec<StreamInstanceSnapshotEntry>, // strictly key-sorted
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "entry", rename_all = "snake_case", deny_unknown_fields)]
pub enum StreamInstanceSnapshotEntry {
    Live(StreamInstanceState),
    Tombstone(StreamTombstone),
}
```

The snapshot's Stream-owned fields all use decimal-string JSON. Existing non-Stream
snapshot integers are not changed by this narrow contract unless their owning schema
already changes them independently.

## 20. Save blockers and restore order

```rust
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum StreamSaveBlocker {
    ExternalOpening { instance: StreamInstanceKey },
    ExternalLive { instance: StreamInstanceKey },
    ExternalRestarting { instance: StreamInstanceKey },
    CloseAcknowledgementPending { instance: StreamInstanceKey },
    ProducerNotAtSafePoint {
        instance: StreamInstanceKey,
        fiber: RuntimeFiberId,
    },
    NonRecordableQueuedPayload { instance: StreamInstanceKey },
    ConsumerCleanupPending { instance: StreamInstanceKey },
}
```

Restore validation order is fixed:

1. strict save envelope/checksum/schema ID/schema version/UTF-8/BOM/trailing data;
2. JSON duplicate/unknown fields and canonical Stream integer/digest strings;
3. unchanged non-Stream snapshot validation;
4. artifact identity, AWBC ABI 2/codec 8 executable fingerprint, generation availability;
5. Stream table sortedness, duplicate keys, allocation cursors, global limits;
6. definition ID/key/item/error/layout/profile references;
7. each live/tombstone local invariant, queue/replay accounting and privacy;
8. scan all root/child runtime values for exactly one current consumer lease or zero if
   dropped; reject stale/current duplicates and payload-contained handles;
9. reciprocal producer-fiber key/lease and generator safe-point validation;
10. external state must be closed with no pending open/restart/close acknowledgement;
11. generation pins and hot-reload compatibility;
12. build candidate executor, recompute canonical snapshot bytes, and commit one swap.

Any failure destroys the candidate and leaves the active executor unchanged.

## 21. Typed profile input, projection, and accepted evidence

### 21.1 Authored schema owner

Owner: `arcweft-manifest-model::stream_profile`. These are authored manifest values, not
runtime policy or endpoint DTOs. They use platform-independent `u32`/`u64`; TOML/manifest
syntax follows the existing strict manifest decoder and is not the host/save JSON wire.

```rust
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct StreamRuntimeProfileSpec {
    pub max_live_instances: Option<u64>,
    pub max_tombstones: Option<u64>,
    pub max_events_per_step: Option<u64>,
    pub max_queue_items_per_instance: Option<u32>,
    pub max_queue_bytes_per_instance: Option<u64>,
    pub max_total_queue_bytes: Option<u64>,
    pub max_item_bytes: Option<u64>,
    pub max_replay_records_per_instance: Option<u32>,
    pub max_replay_payload_bytes_per_instance: Option<u64>,
    pub max_replay_total_bytes_per_instance: Option<u64>,
    pub max_total_replay_bytes: Option<u64>,
    pub max_open_arguments: Option<u32>,
    pub max_open_argument_bytes: Option<u64>,
    pub max_lifetime_events: Option<u64>,
    pub max_lifetime_items: Option<u64>,
    pub max_recoverable_errors: Option<u64>,
    pub max_progress_events: Option<u64>,
    pub max_deliveries: Option<u64>,
    pub max_restart_attempts: Option<u64>,
    pub max_provider_replacements: Option<u64>,
    pub supports_full_replay: Option<bool>,
    pub supports_hash_replay: Option<bool>,
    pub supports_summary_replay: Option<bool>,
    pub supports_event_replay: Option<bool>,
    pub supports_coalesce: Option<bool>,
    pub supports_restart: Option<bool>,
    pub supports_provider_replacement: Option<bool>,
    pub minimum_privacy: Option<StreamProfilePrivacyFloor>,
    pub minimum_permission: Option<StreamProfilePermissionFloor>,
    pub maximum_terminal_error_replay: Option<StreamProfileTerminalReplayCap>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StreamProfilePrivacyFloor { Recordable, Redacted, Transient, Private }

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StreamProfilePermissionFloor { AtOpen, OnRestart, EachEvent }

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StreamProfileTerminalReplayCap { EventOnly, Summary, Digest, Payload }
```

`arcweft-launch::manifest::ProfileSpec` adds exactly
`stream: StreamRuntimeProfileSpec` with `#[serde(default)]`. The strict decoder adds
`ProfileField::Stream`, `ManifestPathSegment::StreamProfile(StreamProfileField)`, and this
closed field enum in declaration order:

```rust
pub enum StreamProfileField {
    MaxLiveInstances,
    MaxTombstones,
    MaxEventsPerStep,
    MaxQueueItemsPerInstance,
    MaxQueueBytesPerInstance,
    MaxTotalQueueBytes,
    MaxItemBytes,
    MaxReplayRecordsPerInstance,
    MaxReplayPayloadBytesPerInstance,
    MaxReplayTotalBytesPerInstance,
    MaxTotalReplayBytes,
    MaxOpenArguments,
    MaxOpenArgumentBytes,
    MaxLifetimeEvents,
    MaxLifetimeItems,
    MaxRecoverableErrors,
    MaxProgressEvents,
    MaxDeliveries,
    MaxRestartAttempts,
    MaxProviderReplacements,
    SupportsFullReplay,
    SupportsHashReplay,
    SupportsSummaryReplay,
    SupportsEventReplay,
    SupportsCoalesce,
    SupportsRestart,
    SupportsProviderReplacement,
    MinimumPrivacy,
    MinimumPermission,
    MaximumTerminalErrorReplay,
}
```

The existing one-pass decoder records the key and scalar-value `SourceSpan` for each
present field. `SourceBackedManifest` exposes those spans through a typed
`profile_stream_field_span(profile, field)` accessor. It never reparses text.

### 21.2 Runtime target and compiler projection

Owner of the cross-product target and persisted evidence: `arcweft-core::stream`.

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StreamRuntimeTarget { Native, Web, Agent }

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StreamRuntimeProfileHash(pub [u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeStreamProfileEvidence {
    pub target: StreamRuntimeTarget,
    pub canonical_hash: StreamRuntimeProfileHash,
}
```

The typed target is an explicit `ProjectCompilationContext` input. It is not derived from
profile ID, adapter name, Cargo feature, file path, or source spelling. Agent pairing is
validated before baseline resolution: `LaunchKind::Agent` pairs only with `Agent`, and
non-Agent kinds pair only with explicitly supplied `Native` or `Web`.

Owner of the source-backed handoff construction:
`arcweft-compiler::ProjectCompilationContext`. It performs one exhaustive field-by-field
match from the manifest-model enums into the runtime-plan input below. This is the
necessary compile-time projection between sibling crates, not a compatibility shim,
extension trait, endpoint DTO, or second policy resolver.

### 21.3 Runtime-plan acceptance input and result

Owner: `arcweft-runtime-plan::stream_profile`.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcedProfileValue<T> {
    pub value: T,
    pub span: SourceSpan, // existing arcweft-source type/revision
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AuthoredStreamRuntimeProfileInput {
    pub max_live_instances: Option<SourcedProfileValue<u64>>,
    pub max_tombstones: Option<SourcedProfileValue<u64>>,
    pub max_events_per_step: Option<SourcedProfileValue<u64>>,
    pub max_queue_items_per_instance: Option<SourcedProfileValue<u32>>,
    pub max_queue_bytes_per_instance: Option<SourcedProfileValue<u64>>,
    pub max_total_queue_bytes: Option<SourcedProfileValue<u64>>,
    pub max_item_bytes: Option<SourcedProfileValue<u64>>,
    pub max_replay_records_per_instance: Option<SourcedProfileValue<u32>>,
    pub max_replay_payload_bytes_per_instance: Option<SourcedProfileValue<u64>>,
    pub max_replay_total_bytes_per_instance: Option<SourcedProfileValue<u64>>,
    pub max_total_replay_bytes: Option<SourcedProfileValue<u64>>,
    pub max_open_arguments: Option<SourcedProfileValue<u32>>,
    pub max_open_argument_bytes: Option<SourcedProfileValue<u64>>,
    pub max_lifetime_events: Option<SourcedProfileValue<u64>>,
    pub max_lifetime_items: Option<SourcedProfileValue<u64>>,
    pub max_recoverable_errors: Option<SourcedProfileValue<u64>>,
    pub max_progress_events: Option<SourcedProfileValue<u64>>,
    pub max_deliveries: Option<SourcedProfileValue<u64>>,
    pub max_restart_attempts: Option<SourcedProfileValue<u64>>,
    pub max_provider_replacements: Option<SourcedProfileValue<u64>>,
    pub supports_full_replay: Option<SourcedProfileValue<bool>>,
    pub supports_hash_replay: Option<SourcedProfileValue<bool>>,
    pub supports_summary_replay: Option<SourcedProfileValue<bool>>,
    pub supports_event_replay: Option<SourcedProfileValue<bool>>,
    pub supports_coalesce: Option<SourcedProfileValue<bool>>,
    pub supports_restart: Option<SourcedProfileValue<bool>>,
    pub supports_provider_replacement: Option<SourcedProfileValue<bool>>,
    pub minimum_privacy: Option<SourcedProfileValue<StreamPrivacy>>,
    pub minimum_permission: Option<SourcedProfileValue<StreamPermissionRule>>,
    pub maximum_terminal_error_replay: Option<SourcedProfileValue<StreamReplayDataClass>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamProfileProvenance {
    TargetBaseline,
    Authored(SourceSpan),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamRuntimeProfileLimits {
    pub max_live_instances: StreamCount,
    pub max_tombstones: StreamCount,
    pub max_events_per_step: StreamCount,
    pub max_queue_items_per_instance: StreamQueueItemLimit,
    pub max_queue_bytes_per_instance: StreamByteLimit,
    pub max_total_queue_bytes: StreamByteLimit,
    pub max_item_bytes: StreamByteLimit,
    pub max_replay_records_per_instance: StreamReplayRecordLimit,
    pub max_replay_payload_bytes_per_instance: StreamByteLimit,
    pub max_replay_total_bytes_per_instance: StreamByteLimit,
    pub max_total_replay_bytes: StreamByteLimit,
    pub max_open_arguments: StreamQueueItemLimit,
    pub max_open_argument_bytes: StreamByteLimit,
    pub max_lifetime_events: StreamCounterLimit,
    pub max_lifetime_items: StreamCounterLimit,
    pub max_recoverable_errors: StreamCounterLimit,
    pub max_progress_events: StreamCounterLimit,
    pub max_deliveries: StreamCounterLimit,
    pub max_restart_attempts: StreamCounterLimit,
    pub max_provider_replacements: StreamCounterLimit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamRuntimeSupport {
    pub supports_provider_blocking: bool,
    pub supports_full_replay: bool,
    pub supports_hash_replay: bool,
    pub supports_summary_replay: bool,
    pub supports_event_replay: bool,
    pub supports_coalesce: bool,
    pub supports_restart: bool,
    pub supports_provider_replacement: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedStreamRuntimeProfile {
    pub target: StreamRuntimeTarget,
    pub limits: StreamRuntimeProfileLimits,
    pub support: StreamRuntimeSupport,
    pub minimum_privacy: StreamPrivacy,
    pub minimum_permission: StreamPermissionRule,
    pub maximum_terminal_error_replay: StreamReplayDataClass,
    // Exactly 30 entries in the declaration order above.
    pub provenance: [StreamProfileProvenance; 30],
    pub canonical_hash: StreamRuntimeProfileHash,
}
```

`supports_provider_blocking` is a non-overridable built-in field and is `false` for all
three targets. It is intentionally absent from both authored input shapes. Runtime-plan
computes the accepted result and first error in `POLICY_PROFILE.md`; no later owner clamps,
loosens, or re-resolves it.

The canonical profile hash is BLAKE3-256 over
`b"arcweft.stream.profile.v1\\0"`, target tag `0 Native / 1 Web / 2 Agent`, every resolved
limit as fixed-width little-endian in `StreamRuntimeProfileLimits` declaration order,
every support flag as `0/1` in declaration order, then the explicit privacy, permission,
and terminal-replay rank bytes. Provenance, profile ID, and source ranges are excluded.
The exact same hash becomes `RuntimePlan.stream_profile.canonical_hash`, participates in
the bundle Stream fingerprint, and is checked on restore/hot reload.

### 21.4 Serde reachability rule

Every type reachable from `StreamInstanceTableSnapshot`, a Stream host request/event,
Stream observation/outcome, or a runtime value containing `StreamHandle` MUST implement
the owning strict codec. Save/host-reachable structs and enums in this file derive or
manually implement `Serialize`/`Deserialize` with `deny_unknown_fields`, exact closed tags,
and the decimal-string newtype codec. Repetitive derive tokens omitted from an in-memory
excerpt do not permit an alternative field, tag, numeric representation, or permissive
decoder.
