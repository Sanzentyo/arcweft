# FINAL CONTRACT — Lang-01.3.1.2.1

`OPEN_QUESTIONS=0`

## 1. Binding scope and integration base

This document closes the contract defects listed by the sole request in `SOURCE_REQUEST.md`.
It is normative for the production implementation that follows Lang-01.3.1.2.1. The
integration base inspected for this package is `Sanzentyo/arcweft` `main` at
`23ed5d93824630d8ead9092d32f7fc70f0a8f314`. The implementation must re-read the then-current root `AGENTS.md` and
reconfirm the allocations in `WIRE_AND_VERSION_ALLOCATIONS.md` immediately before the
version-changing merge unit. If `main` has consumed an allocation, the implementer must
move only the collided new allocation to the next unused value, update every artifact and
test vector in one review, and must not preserve the collided value through an alias.

The following already accepted substrate remains authoritative and is not redesigned:

- the shared callable catalog/resolver, callable source evidence, accepted-HIR lifecycle,
  definition-source index, external binding publication, and query budgets;
- the ordinary `fn` direct-style suspension direction, typed `await` source node, direct
  call frames, CFG, resume points, safe points, and producer-child-fiber scheduler;
- the current executor-neutral `FiberState` exchange and AWBC verifier budget model;
- strict, single-version AWBC and save decoding patterns, canonical executable identity,
  and atomic hot-swap/restore transaction boundaries;
- `arcweft-source`, source documents, source spans, and AWBC debug source maps.

The concrete defects that justify narrow correction are the current Stream event's reuse
of `SourceEventKind`, unchecked Stream counters, queue-erasing close, duplicate compact
and facade Stream state, simultaneous Stream/Source AWBC tables, and the missing exact
owners/types named by the request. No unrelated proof, concurrency, presentation, style,
environment, view, rich-text, character, Need/task scheduling, CSS, or Takumi design is
changed.

The shared step boundary is exact: `RuntimeStepInput.stream_events` is the sole Stream
ingress, `RuntimeStepOutput.stream_event_outcomes` reports one disposition per ingress,
`RuntimeEffectBatch.stream_observations` carries non-item control/telemetry, and
`HostRequestBatch.stream_requests` carries both open and close requests. The old
`source_events`, old emitted `stream_events`, and `source_close` fields are removed.

## 2. Normative terms

- **MUST**, **MUST NOT**, **SHALL**, and **SHALL NOT** are requirements.
- **Accepted evidence** means typed output of the existing parser/HIR/sema/catalog/resolver
  pipeline, not source text, debug labels, or reconstructed names.
- **Canonical bytes** means the unique encoding defined by the owning shared codec.
- **Session** means one active runtime executor and its root/child fibers, Stream instance
  table, generation pins, and deterministic step boundary.
- **Live consumer** means the unique current affine `StreamHandle` lease.
- **Tombstone** means the sole post-terminal entry that replaces, rather than accompanies,
  a live instance entry.

## 3. Closed decision ledger

| Decision | Final rule |
| --- | --- |
| Callable projection | Preserve all five passing modes and all three presence modes. `Optional` is representable and is never rejected merely for being optional. |
| Argument resolution | The existing shared resolver/checker is the only resolver. Runtime lowering consumes its typed declaration-order binding; it never re-resolves names or source spellings. |
| Instance owner | One `StreamInstanceTable` owned by the active session/executor owns every live state and tombstone. Fibers own only affine handle values and reciprocal producer references. |
| Host lookup | `BTreeMap::get_mut(&StreamInstanceKey)` on the sole table after structural envelope validation. No scan registry, sidecar map, or rebuilt facade. |
| Replay owner | `StreamReplayStore` is embedded in the sole live entry and moved intact into the replacing tombstone. |
| Effect sets | RuntimePlan owns one canonical `RuntimeEffectSetTable`; AWBC owns its one-to-one projection in `AwbcProgram.effect_sets`. |
| RuntimePlan support types | Reuse the existing callable, flow, expression, pattern, CFG, frame, resume-point, and source-range owners. Remove the proposed Stream-local duplicates instead of defining parallel copies. |
| Policy profile | The existing selected `arcweft-launch` profile owns authored tightening and exact spans; an explicit core target plus compiler projection feeds the sole runtime-plan profile resolver. RuntimePlan carries only resolved policy and profile evidence. |
| Exhaustion | Invalid envelopes are rejected with no mutation. A structurally valid next envelope whose accepted body would exhaust a checked runtime counter is consumed as one atomic `RuntimeLimit` terminalization, with no item/queue partial commit. |
| Dropped consumer | The session registry, not a vanished consumer, synchronously disposes queued deliveries under the resolved drop-retention rule. A live consumer still drains queued deliveries before terminal observation. |
| Version migration | Lang-01.1.1's suspension/generator substrate and this sole Stream model land in one ABI 2 / codec 8 product format. Lang-01.1.1 provisional Stream shapes never land. |
| JSON integers | Every Stream-owned integer in shared host/save JSON is an ASCII canonical decimal string carried by a domain newtype. No Stream boundary exposes `usize`. |
| Compatibility | ABI 1, codec 7, bundle schema 5/AWBC-v1, save schema 1, Source tables/events, and provisional Stream forms are rejected directly. No migration dispatch exists. |

## 4. Lossless callable parameter projection

The owner of source callable semantics remains
`arcweft-lang-sema::callable::{CallableParameterPassing, CallableParameterPresence}`.
The RuntimePlan projection owned by `arcweft-core::entry` has the same closed variants:

- passing: `PositionalOnly`, `PositionalOrNamed`, `NamedOnly`, `RestPositional`,
  `RestNamed`;
- presence: `Required`, `Optional`, `Defaulted`.

`RuntimeResolvedArguments` contains exactly one declaration-order entry per non-curried
parameter group selected by the existing resolver. `Required` and `Defaulted` entries
MUST be `Value`; an `Optional` entry MAY be `Value` or `Omitted`. Defaults are evaluated
and materialized by the existing typed call-lowering path before Stream open lowering;
no default expression or source text is carried into RuntimePlan or host wire. A rest
positional entry is one canonical sequence value preserving source argument order. A rest
named entry is one canonical map value with unique normalized names in UTF-8 byte order;
named-rest iteration order is defined to be that canonical order.

The projection boundary validates resolver identity, parameter count/index, declared
name, type, passing mode, presence, and value/omission shape. Failure is a typed
`RuntimeCallableProjectionError` at the existing call span or
`CallableParameterSource.parameter` span. These failures indicate missing or inconsistent
accepted evidence; they do not trigger a fallback resolver. The exact error order is:
resolver identity, parameter count, parameter index, declaration metadata, required
value, default materialization, optional shape, rest container shape, payload eligibility.

## 5. Static definition identity and instance identity

`RuntimeStreamDefinitionId(u32)` is a canonical RuntimePlan table index. It is not stable
across independently built artifacts. `RuntimeStreamDefinitionKey([u8; 32])` is the stable
semantic key. It is BLAKE3-256 over the exact transcript specified in
`WIRE_AND_VERSION_ALLOCATIONS.md`: origin semantic identity, callable contract, item/error
layout hashes, canonical parameter contract, effect-set members, and provider ABI facts.
It excludes source files/ranges/maps, display labels, table indices, code bytes, resolved
capacity maxima, and generation.

`StreamInstanceKey` is ordered lexicographically by definition key, then generation, then
instance ordinal. It contains:

1. `definition_key: RuntimeStreamDefinitionKey`;
2. `generation: StreamGeneration`;
3. `ordinal: StreamInstanceOrdinal`.

Ordinals are allocated monotonically by the session table and never reused in the
session. Allocation at `u64::MAX` fails atomically with
`StreamCreateError::InstanceOrdinalExhausted`; no instance, handle, child fiber, request,
or generation pin is created. A host event whose generation or definition key does not
match the exact key is stale/wrong and cannot mutate state.

The diagnostic display form `stream:<64-lowercase-hex>@<generation>/<ordinal>` is never a
protocol identity and is never parsed by product code.

## 6. Sole instance and tombstone authority

The only mutable authority is:

```text
StreamInstanceTable.entries: BTreeMap<StreamInstanceKey, StreamInstanceEntry>
StreamInstanceEntry = Live(StreamInstanceState) | Tombstone(StreamTombstone)
```

The table is a field of the active executor/session exactly once. The structured executor
and AWBC product executor each embed that same core type when used as alternative
executors; no single session contains two tables. `FlowFiber` and `FiberState` contain no
`source_states`, `stream_states`, or Stream instance vectors/maps.

A consumer handle contains the key, item/error layout hashes, and current
`StreamConsumerLease`. The sole live/tombstone entry contains the matching consumer owner
and lease. Language-visible `Move` removes the source occurrence before installing the
destination occurrence and rotates the lease using the table's monotonic lease allocator.
Intra-fiber and cross-fiber moves use the same staged transfer. Cross-fiber moves also
change the authoritative owner `RuntimeFiberId`. Allocation/validation failure leaves the
source occurrence and instance entry unchanged. Copy, clone, use-after-move, stale-lease
use, and two current lease occurrences are runtime/verifier errors.

A producer child fiber contains only `ProducerStreamRef { key, lease }`. The live entry
contains the reciprocal `StreamProducerOwner::Fiber { fiber, lease }`. External
producers are represented by typed request/provider state in the live entry. Restore and
hot-swap validate both directions and reject an orphan, duplicate, wrong-key, or
wrong-lease producer.

Snapshot form is one sorted `Vec<StreamInstanceSnapshotEntry>` in the executor snapshot.
Using a vector makes duplicate JSON keys impossible to hide. Keys MUST be strictly
increasing. Root/child fibers serialize handles and producer references only. Restore
builds one candidate table, scans all candidate runtime values for current handle leases,
validates all reciprocal references, and commits the candidate executor only after every
check succeeds. There is no shadow registry, compact/facade rebuilding, or mutable
projection cache.

## 7. Queue, terminal, and lifecycle authority

A live instance owns one FIFO `VecDeque<StreamQueuedDelivery>`. Deliveries are closed
variants `Item(RuntimePayload)` and `RecoverableError(RuntimePayload)`; progress and
control events are not application items. Item/error payload type layout evidence is
checked against the definition before any sequence or counter mutation. Stream payload
schemas MUST be host/save-payload eligible and MUST NOT contain `StreamHandle` or another
affine runtime handle.

Terminal state is stored separately from the queue. For a live consumer, `NextStream`
always returns queued deliveries in FIFO commit order before returning the terminal
outcome. A terminal error payload has one live owner and is moved to the consumer exactly
once; later calls return a payload-free `Closed(reason_code)`. A tombstone is not created
until that payload has been consumed or dropped. Setting terminal state never clears or
reorders the live consumer's queue. Once the consumer is dropped, queued deliveries are
no longer observable and the session registry performs the synchronous cleanup in
section 12.

The external lifecycle is closed and typed: `OpenRequested`, `Open`, `Disconnected`,
`RestartRequested`, `Closing`, `Closed`. Local/derived producers are `Running` or
`Stopped`. The common close request state is `NotRequested`, `Requested`, `Acknowledged`, or
`NotApplicable`; request IDs are reserved during external instance creation so a terminal
transition cannot fail to allocate its one close request. Repeated close/drop/ack input is
idempotent when it exactly matches the stored request and is rejected without mutation
when it conflicts.

Inbound event kinds are: `Opened`, `Progress`, `Item`, `RecoverableError`, `End`,
`TerminalError`, `Disconnected`, `PermissionRevoked`, `CloseAcknowledged`, `Restarted`,
and `ProviderReplaced`. Progress is typed `phase/completed/total/unit` metadata and is
never a `T` item. Recoverable error is queued as an `E` delivery and does not close the
producer. End, terminal error, terminal disconnect, permission revocation, cancellation,
provider replacement failure, and runtime limit create one terminal state.

## 8. Deterministic ingress normalization and event rejection

A RuntimeStep first checks the Stream batch count against the accepted profile. It then
sorts by `(StreamInstanceKey, StreamEventSequence, event-kind tag)` and partitions by key.
Duplicate `(key, sequence)` pairs or a non-contiguous sequence in one partition reject the
entire partition before that partition mutates; other key partitions remain independent
and are processed in key order. This preserves current deterministic scheduling without
making input vector order semantic.

Each partition is staged against one cloned/copy-on-write entry. Structural rejection
kinds are: malformed envelope, unknown instance, wrong definition, stale generation,
terminal/closed state, duplicate/retrograde/gap sequence, wrong lifecycle, wrong request
ID, wrong payload type, payload over profile limit, invalid progress, permission mismatch,
and forbidden restart/replacement. A rejection preserves the canonical bytes of the
entry, table allocation cursors, requests, observations, and counters.

A partition whose next valid envelope causes accepted runtime-limit terminalization is a
successful state transition, not a rejection. Later envelopes in that partition receive
`Terminal` rejection outcomes after the single terminal commit.

## 9. Replay record and store contract

`StreamReplayDecision` is either `Record(StreamReplayRecord)` or
`NoRecord(StreamReplayNoRecord)`. The stored record body is exactly one of:

- `Payload`: canonical shared runtime-value bytes and the typed payload;
- `Digest`: BLAKE3-256, explicit hash domain, input byte length, and digest;
- `Summary`: type-layout hash, canonical byte length, and closed shape/count metadata;
- `EventOnly`: event kind only.

`NoRecord` is a closed decision with reason `PolicyNone`, `Transient`, `Private`,
`LimitZero`, `RecordTooLarge`, or `TerminalCap`. It is not inserted into the store and has
no record ID. Stored record IDs are monotonically allocated, never reused, and strictly
increase in storage order. Every record also carries the instance commit sequence and
optional external ingress sequence. Stable order is record ID; event/commit fields are
validated as nondecreasing supporting evidence.

`StreamReplayStore` is embedded in the live entry and moved into its tombstone. It owns
`next_record_id`, the deque, retained payload bytes, retained total bytes, stored/evicted/
skipped counters, and no external index. The total-byte counter is the checked length of
the canonical binary replay-record transcript. Before commit, the store evicts oldest
records until record-count, payload-byte, per-instance total-byte, and global replay-byte
limits all hold. If one candidate cannot fit an empty store, the result is
`NoRecord::RecordTooLarge`; the Stream event still follows its ordinary queue/lifecycle
semantics. Replay retention limits do not themselves close a Stream. Arithmetic or
lifetime-counter exhaustion follows the RuntimeLimit rule in section 11.

The exact privacy/replay projection is:

| Privacy | Full | HashOnly | Summary | EventOnly | None |
| --- | --- | --- | --- | --- | --- |
| Recordable | Payload | Digest | Summary | EventOnly | NoRecord |
| Redacted | Summary | Digest | Summary | EventOnly | NoRecord |
| Transient | NoRecord | NoRecord | NoRecord | NoRecord | NoRecord |
| Private | NoRecord | NoRecord | NoRecord | NoRecord | NoRecord |

For events with no user payload, every non-None mode stores `EventOnly`. The resolved
terminal-error replay cap may further reduce `Payload -> Digest -> Summary -> EventOnly`;
it never upgrades. Unsupported requested replay modes are checked profile errors rather
than silent substitutions.

The digest transcript and domains are exact in `WIRE_AND_VERSION_ALLOCATIONS.md`.
Summary contains no string/byte content, field/case names, scalar values, entity IDs, or
source locations. It contains only layout hash, canonical length, event kind, and one
closed shape: unit, scalar bit width/class, UTF-8 byte/scalar counts, byte count,
sequence/tuple/record cardinality, variant ordinal plus payload-present bit, matrix
shape, or tensor rank/element count.

Payload erasure is a logical ownership guarantee: after an erasing transition, no owning
`RuntimePayload` clone remains in queue, replay, tombstone, snapshot candidate, request,
or observation state. This contract does not claim physical zeroization of allocator
memory and introduces no unsafe code or new zeroization dependency.

## 10. Canonical effect-set ownership

The accepted semantic inventory remains `arcweft-lang-sema::effects::EffectSet`, backed
by the existing ordered `EffectId` set and closed effect-row report. RuntimePlan lowering
converts accepted IDs once into `RuntimeEffectId`; `arcweft-core` does not depend on sema.

`RuntimePlan.effect_sets` is the sole RuntimePlan table. ID 0 is the empty set. Members in
each set are sorted by canonical UTF-8 bytes and unique. The table is sorted
lexicographically by member vectors with the empty set forced first; duplicate sets are
forbidden. All `RuntimeEffectSetId` references are bounds checked. RuntimePlan lookup is
only `effect_sets.get(id.index())`.

AWBC projects the table one-to-one and in the same order into its existing
`AwbcProgram.effect_sets`; effect names use the canonical string table. Codec 8 verifier
adds whole-table duplicate rejection and the ID-0-empty invariant in addition to member
ordering/bounds/allowed-effect checks. Effect-set bytes participate in executable and
bundle fingerprints. A tampered reference is rejected before execution/restore.

## 11. Complete RuntimePlan support ownership

The corrected RuntimePlan retains its unrelated accepted tables and canonical ordering,
then owns `effect_sets` and `stream_definitions`. It has no `stream_plans` or
`source_plans` fields.

A Stream definition contains typed item/error value contracts, one general callable
boundary signature, one effect-set ID, one origin, one resolved policy, and the separate
layout/code/provider hashes used for bundle/hot-swap classification. Closed origins are
`AuthoredGenerator`, `External`, and `Derived`. An ordinary pass-through function that
returns an existing handle creates no definition and no instance facade.

The following proposed support names are eliminated rather than duplicated:

- `RuntimeSourceMapRef`: use existing diagnostic `SourceSpan` while lowering and existing
  `AwbcSourceMapId` after AWBC emission; neither is semantic identity;
- `RuntimeStreamFrameLayout`: use existing `AwbcFrameLayout`/`AwbcFrameLayoutId`;
- Stream-local bindings/expressions/patterns/match arms: use the existing
  `RuntimeBinding`, `RuntimeExpr`, `RuntimePattern`, and accepted CFG/match structures;
- Stream-local program/branch tree: use existing `AwbcFunction`, `AwbcBlock`,
  `AwbcInstruction`, `AwbcTerminator`, and `AwbcResumePoint`.

Branch and match lowering continues to use existing CFG branch selection, so only one
selected path executes. `for await` lowers to codec-8 `NextStream`; empty/open queues
suspend at the existing resume-point mechanism and resume in FIFO item order. Generator
`yield` lowers to codec-8 `YieldStream` and suspends the producer child at a typed safe
point. No behavior is recovered from source text.

## 12. Typed profile, exhaustion, and dropped-consumer resolution

The exact profile owner, dependency-preserving projection, constants, target/launch-kind
matrix, monotonic resolution, canonical hash, and first-error order are in
`POLICY_PROFILE.md` and `RUST_SCHEMAS.md` §21. The accepted single-decode launch profile
owns authored values/spans; the compiler owns the cross-crate projection; runtime-plan is
the sole baseline/resolution owner; RuntimePlan stores only core-owned profile evidence.
All current `Native`, `Web`, and `Agent` target baselines set
`supports_provider_blocking=false`; a blocking definition is rejected before RuntimePlan
emission. There is no runtime fallback to nonblocking behavior.

Every Stream arithmetic operation uses checked arithmetic. External sequence state is
`Next(n)` or `Exhausted { last }`. At `Next(u64::MAX)`, a matching structurally valid
envelope is consumed as `RuntimeLimit::EventSequence`, the cursor becomes `Exhausted`,
no body/queue item is committed, one terminal observation/result is emitted, and one
external close request is emitted if applicable. After exhaustion/terminal, repeated
input is rejected without mutation.

At a configured item/event/progress lifetime maximum, the event that reaches the maximum
is accepted. The next structurally valid matching event consumes its sequence but not its
body and atomically creates `RuntimeLimit` terminal state. At a local generator yield,
the same rule is applied at the yield safe point without a host sequence: no item is
queued, the producer child stops, and the terminal/result/observation is committed once.
Delivery count cannot exhaust before accepted item/error count because validation
requires `max_deliveries >= max_items + max_recoverable_errors`; a snapshot violating
that or containing a queued delivery with an already exhausted delivery counter is
rejected. Tests still cover `MAX-1`, `MAX`, terminal observation, and tampered impossible
states.

On sole-consumer drop, the table owner immediately changes consumer state to `Dropped`
and processes the queue synchronously:

- `DiscardQueued`: drop all queued delivery payloads; erase queued-associated replay only
  when replay retention is `UntilConsumerDrop`;
- `DrainAndRetainReplay`: walk queued deliveries in FIFO order, retain or attempt exactly
  one policy projection for each within replay limits, then drop all queued payloads.
  This mode requires non-`None` replay and `ThroughTombstone` retention.

An external producer receives exactly one typed close request; a local/derived producer
child receives one cancellation transition. Repeated cleanup is idempotent. Once the
consumer is dropped, the queue is unobservable, so terminal transition no longer waits
for an unreachable drain owner. Live consumers retain the queue-before-terminal rule.

A live entry is replaced atomically by a tombstone when producer/close state is settled,
its queue is empty, terminal metadata is available, and any terminal error payload has
been consumed by the live consumer or dropped by consumer cleanup. The tombstone remains the sole
entry and may retain bounded replay plus a closed consumer lease. It is released only
when the consumer lease is gone, close is settled, replay retention permits erasure or
deterministic tombstone eviction, and its generation pin can be released. Total entries,
tombstones, queue bytes, and replay bytes are profile bounded. When tombstone capacity is
full, oldest releasable tombstones are erased first; otherwise a terminal live entry
remains `TerminalPendingTombstone` and blocks new creation rather than violating a limit.

## 13. Reconciliation with Lang-01.1.1 and Lang-01.3.1.1

Authoritative Lang-01.1.1 substrate:

- one ordinary `fn` spelling;
- direct-call frames and ordinary-function suspension;
- typed `await`, existing CFG, resume points, safe points, and child-fiber scheduler;
- own-scope `yield` plus `Stream<T,E>` return classifies a callable as a generator;
- a Stream-returning callable with no own-scope yield remains immediate pass-through.

Authoritative Lang-01.3.1.1 evidence:

- an external operation is an ordinary bodyless callable in an `extern capability`;
- its `Stream<T,E>` result, capability/operation identity, effects, parameter schema, and
  source evidence come from the shared resolver/catalog.

Superseded design-only material from Lang-01.1.1: any provisional `StreamPlan`, Stream
handle/state/event, Source-based producer, provisional opcode/table, ABI/codec writer, or
save shape. None may enter product code even temporarily. This corrected contract is the
sole Stream public/runtime/wire model.

One atomic AWBC merge unit owned by `arcweft-core::awbc` changes ABI to 2 and codec to 8
and includes both the direct-suspension/generator requirements and the sole corrected
Stream definition/runtime operations. `arcweft-runtime-plan::awbc_lower` is the producer;
`arcweft-bundle::product_awbc` is the product wrapper consumer. There is one codec-8
reader/writer/verifier/VM path and no ABI-1/codec-7 dispatch.

One later atomic persistence merge unit owned by
`arcweft-runtime-driver::session_save` with `arcweft-save` changes session save schema to
2. It includes the sole table snapshot, generator safe points, replay/tombstones,
generation pins, restore validation, and old-schema rejection. No schema-1 migration is
registered.

## 14. Shared host and save JSON

Every Stream-owned integer uses a domain newtype whose JSON `Serialize`/`Deserialize`
accepts exactly one canonical decimal string. Accepted grammar is `"0"` or
`"[1-9][0-9]*"`. Plus/minus signs, whitespace, leading zeros, exponent/decimal notation,
JSON numeric tokens, overflow, empty strings, non-ASCII digits, and embedded NUL are
rejected. `u32` and `u64` use their platform-independent maxima. No Stream host/save type
contains `usize` or `isize`.

All Stream structs/enums use `deny_unknown_fields`; known duplicate fields are errors.
Input bytes must be UTF-8 without BOM; trailing non-whitespace bytes are errors. Canonical
output is compact UTF-8 with struct declaration order, internally tagged enum tag first,
explicit `null` for optional fields, lower-snake-case enum names, arrays already in their
normative order, and 64-lowercase-hex field encoding for every new Stream-owned digest
or reused digest field inside a Stream host/save record. That field codec does not change
the reused digest type globally. Native, web, and Agent adapters pass the same encoded
bytes through the shared core codec and do not deserialize into endpoint-local DTOs or
JavaScript-number intermediates.

Shared `RuntimePayload` contents retain their existing canonical codec. This contract's
decimal-string rule applies to all Stream envelope, progress, policy, counter, statistic,
replay, instance, tombstone, and snapshot integers surrounding those payloads.

## 15. Bundle, fingerprint, save, restore, and hot reload

The product bundle outer schema becomes 6 solely because the AWBC discriminator and
runtime summary shape change. `BundleAwbcEncoding` has only `AwbcV2`/`"awbc_v2"`.
`BundleRuntimeSummary` has `stream_definitions` and no Source/old Stream plan counts.
Schema 5 and `awbc_v1` are rejected; no second reader is kept.

The Stream bundle fingerprint is BLAKE3-256 over the domain, AWBC executable identity,
canonical Stream definition records, resolved profile bytes, provider ABI hashes, handle
and state layout hashes, generator code/frame compatibility hashes, and adapter
requirements. Existing debug source/display maps remain excluded. Worked bytes and a
verified digest are in `WORKED_EXAMPLES.md`.

Hot reload uses closed classifications:

1. `ContentOnly`: Stream semantic/code/profile fingerprints identical;
2. `CodeCompatible`: semantic/provider/layout/policy contract identical and generator
   frame/safe-point compatibility hash identical; safe-point frames may rebind;
3. `CodeGenerational`: semantic/provider/layout contract identical but producer code/frame
   incompatible; existing instances stay pinned, new instances use the new generation;
4. `RestartRequired`: same external origin/provider ABI and resolved policy explicitly
   allows same-provider restart/replacement;
5. `Incompatible`: any item/error/signature/effect/origin/provider/layout incompatibility
   or policy loosening; reject the whole swap while an affected live/tombstoned pin exists.

No instance state is rebuilt from another representation. Save blockers are typed and
include every live/opening/restarting external producer, pending close acknowledgement,
local producer not at an accepted safe point, and a non-Recordable instance with a
non-empty payload queue. Closed external state, safe-point local generators, bounded
replay, and tombstones serialize in schema 2. Restore validates the complete candidate in
the fixed order in `RUST_SCHEMAS.md` and commits once.

## 16. Implementation and acceptance

The only allowed implementation order is `IMPLEMENTATION_ORDER.md`. The complete direct,
negative, boundary, tampering, persistence, cross-host, and structural matrix is
`TEST_MATRIX.md`. Source/provisional deletion is `DELETION_INVENTORY.md`. No source-text
search is acceptance evidence; tests exercise public typed APIs, verifier/codec behavior,
canonical bytes, restore atomicity, compile-fail visibility, Cargo metadata, and the
structured audit.

Production implementation may resume from this package. No implementer choice remains at
the callable, RuntimePlan, lifecycle, ownership, policy, replay, host, AWBC, bundle, save,
restore, or compatibility boundaries.
