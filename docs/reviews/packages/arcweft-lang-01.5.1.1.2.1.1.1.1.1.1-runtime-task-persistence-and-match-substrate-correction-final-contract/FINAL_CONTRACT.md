# Final contract

## 0. Scope and precedence

This package is the mandatory nonnumeric correction to the predecessor
runtime-Need/View-Match return. It preserves the predecessor's exact identity
transcripts, producer-family set, policy ordinals, View identity roles and
maintained AWBC allocation. It changes only the repository crossings identified
by the current request.

The source basis is `Sanzentyo/arcweft` at
`3670625a02b9e7e8578b57fc7b148a1758a17dba`. This is one documentation-only commit after the request's
`17b384a36e1412cc7e7d9f13073d8dd33dcb5cbc` production observation; no production crate changed in
that delta.

Every Arcweft-owned version marker is exactly `1`. There is no V2, old reader,
translation table, String fallback, zero sentinel, identity alias or dual task
carrier.

## 1. Canonical value identity is not constant admission

`arcweft_core::entry::RuntimeValueDigest` and the existing exhaustive
RuntimeValue visitor remain the sole value-identity grammar. The visitor is
made sink-parametric so canonical bytes and direct BLAKE3 hashing execute the
same variant match, recursion accounting, byte limits, order and first errors.

The existing opaque transcript is emitted for both:

- `RuntimeOpaqueValueClass::Plain +
  RuntimeOpaquePersistence::ConstantAndSnapshot`;
- `RuntimeOpaqueValueClass::Plain +
  RuntimeOpaquePersistence::SnapshotOnly`.

Producer/type/class/persistence/payload validation still precedes emission.
Affine handles fail canonical value identity. A typed snapshot may preserve an
affine runtime handle for save/restore, but it cannot mint a
`RuntimeValueDigest` and does not change producer/View admission.

Constant publication is a distinct recursive fence. Runtime-plan expression
constants, dialogue/config constants, command constants and equivalent
publishers call `RuntimeValue::validate_constant_admission` before publication.
`SnapshotOnly`, affine, NeedHandle and frame-local values fail there.

The required single-value evidence is closed:

| Operation on the same Plain+SnapshotOnly value | Result |
|---|---|
| canonical bytes | success |
| direct digest | success and equals BLAKE3(canonical bytes) |
| Need argument admission/instance construction | success |
| snapshot/save round trip | success |
| runtime-plan/dialogue/config/command constant publication | explicit constant-admission failure |

## 2. One closed task execution owner

`TaskSpec` has exactly one `execution: TaskExecution` field. It has no
unconditional `HostTaskRequest`, no `Option<HostTaskRequest>`, no
`Option<RuntimeTaskRequest>` and no inferred route.

`TaskExecution::Host` contains the current closed `HostTaskRequest`.
`TaskExecution::Runtime` contains exactly `AwaitManyAggregate` or `Timeout`.

The complete nine-family mapping is normative in
`EXECUTION_TRUTH_TABLE.md` and
`machine/producer_execution_truth_table.json`. Runtime selection reads
`NeedProducerFamily` from the producer descriptor and validates the enum row
through an inherent method. Debug labels, host operation spelling and request
variant spelling never select a family.

AwaitMany aggregate requests contain exact ordered source values, one complete
child `TaskSpec` per source index and the bounded concurrency limit. Thus every
child already has an explicit Host/Runtime route. The aggregate is always
scheduler-owned.

Timeout requests contain the source `RuntimeNeedHandle`, exact requested
logical duration and retained timeout contract digest. The accepted output
correlation is scheduler-derived.

## 3. One atomic scheduler/journal/adapter owner

The final concrete owner is:

```rust
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
```

It implements `TaskHost` and solely owns:

- active/retained generation journals and AlwaysStart counters;
- TaskKey groups, TaskId launches, Need cells and observers;
- spec validation and all identity/correlation derivation;
- host adapter prepare/commit/rollback;
- runtime task staging/stepping;
- host/runtime event normalization and terminal publication;
- cancellation transactions;
- save/restore/replay; and
- replacement validation and rebind.

The driver supplies step input, typed events and operation requests. The old
driver registry, generation owner, counters and rollback protocol are deleted.

Every mutation is an `&mut self` transaction. Runtime task stepping first
selects stable keys/indices, releases the task-map borrow, performs bounded
internal ensure/event transactions, then reapplies results. No `unsafe`,
global interior state or cross-object rollback protocol is required.

## 4. Prepare/commit error closure

Only adapter preparation is fallible. `prepare_launch` receives a
`HostTaskLaunchRequest` that can only be built from `TaskExecution::Host` and
returns an owned prepared token.

The scheduler performs this order:

1. validate spec, family/policy/execution and producer instance;
2. inspect existing Join state or choose the next AlwaysStart ordinal without
   changing the counter;
3. derive all fixed identities and a private journal/runtime delta;
4. for Host only, prepare the adapter token;
5. validate the complete staged delta and all cross-references;
6. if any precommit step fails, rollback the token and discard the delta;
7. apply the delta with infallible map operations;
8. commit the prepared token through an infallible `-> ()` method.

No typed error is reachable after step 6. `TaskEnsureError::AdapterCommit` is
deleted. Restore and replacement use the same prepare/stage/commit rule.

## 5. NeedHandle identity and use

`RuntimeValue::NeedHandle` retains canonical `tag 20 || NeedId`. Public
`RuntimeNeedHandle` equality, hashing and ordering are manually implemented
from `NeedId` only. Origin, debug metadata, full producer spec and generation
do not change value identity.

Those fields remain mandatory structural evidence. Construction and restore
rederive the producer instance, NeedId, TaskKey and TaskId. Ordinary Await and
timeout call `validate_use(active_generation)` before observer/task mutation.
A stale-generation handle remains semantically equal as a value but returns a
typed `StaleGeneration` use error.

Only a validated replacement transaction may rebind generation. It preserves
NeedId, producer instance, policy and ordinal and rederives TaskKey/TaskId for
the new generation.

## 6. Complete version-1 persistence and replay

`machine/persistence_schemas.json` contains 72 exact row definitions, mirrored
in `RUST_SCHEMAS.md` and `PERSISTENCE_AND_REPLAY.md`. They include scheduler,
journal/generation, ordinal counter, task group/mapping/spec/correlation/
lifecycle, Need/outcome, host state, runtime requests/state, observers,
NeedHandle, events/digests/replay, AwaitMany, timeout, replacement and complete
RuntimeValue/type projections.

The purpose-built codec uses:

- version byte exactly `1`;
- fixed IDs as raw 32 bytes;
- fixed little-endian generation/ordinal/cursor integers;
- canonical shortest `u32` varints for lengths;
- `Option<T> = 0 | 1 || T`;
- declared field order;
- canonical sorted map rows;
- bounded allocation before construction.

Unknown tags/fields, duplicate or out-of-order keys, nonminimal varints,
trailing bytes and all-zero fixed identities reject. No generic Serde format is
normative.

Restore decodes to private rows, rederives all identity/digest values, builds
temporary BTreeMaps, validates all joins and runtime-task invariants, prepares
restorable Host rows, and publishes one complete scheduler state. The old live
state remains untouched on any failure.

Prepared adapter tokens are never serialized. Snapshot fails while such a
transaction is in flight. Host operations marked `MustBeQuiescent` also block a
mid-flight snapshot; restartable rows restore by adapter preparation from the
complete original request/correlation.

## 7. Constructible Match substrate

The current owner is `FinalSemanticAnalysis`, which already records the exact
`HirSnapshotId` per module. The compiler-local reference is:

```rust
pub struct CheckedMatchRef {
    snapshot: HirSnapshotId,
    expression: ExprId,
}
```

No `AcceptedSemanticGeneration` is introduced.

Stable semantic identity is built with:

- `AcceptedDeclarationSemanticId`;
- `CheckedExpressionChildRolePath`;
- `StableCheckedValueCoordinate`;
- stable pattern coordinates;
- accepted callable/contract, nominal/layout, field/case and project identities;
- exact semantic literal payloads; and
- bounded coverage/unreachable evidence.

The transcript tables cover all 27 current
`CheckedExpressionResolution` variants, 8 `CheckedValueResolution` variants, 7
`CheckedSelectResolution` variants, 5 `CheckedPatternResolution` variants, 13
`HirPatternKind` families and 7 literal families.

Raw arena IDs are lookup-only. No emitted transcript contains HIR allocation
numbers, source spans, source spelling, debug names, map iteration order or
generic Serde bytes. Differential tests renumber arenas and move spans while
preserving semantic meaning.

## 8. Compiler-local and persistent View rows are separate

Cut 3 publishes `CompilerLocalViewMatchCatalogRow`, which contains
`CheckedMatchRef` for exact lookup and only Cut 1/2/3 semantic/admission
products. It has no task/Need digest dependency.

Cut 5 publishes `AcceptedViewMatchBundleRowV1`, which contains exactly the 13
closed field roles named by the request. It contains no `CheckedMatchRef`,
`ExprId`, `HirSnapshotId`, `SourceSpan` or compiler certificate object.

Bundle validation joins the projections against current compiler, AWBC,
ownership, producer and revision products. It cannot mint a replacement
semantic identity from the joined bytes.

## 9. Carrier-backed ownership

The current 85-variant `TypeKind` matrix is exhaustive. Each successful
`SnapshotClone` row names:

1. exact current/same-cut runtime projection;
2. exact live `RuntimeValue` carrier;
3. exact canonical identity transcript;
4. exact `RuntimeValueSnapshotV1` row.

`Predicate` is a TypeKind leaf. Its runtime predicate value may recursively
contain RuntimeValue operands, but the type classifier has no Predicate child
edge.

`Shared<T>` is rejected with `MissingRuntimeSnapshotOwner`. This correction
does not create a core Shared carrier, opaque-name encoding, extension trait or
side table.

Opaque accepted nominals require exact catalog evidence for producer, semantic
nominal identity, value class and persistence. Plain SnapshotOnly is
snapshot-clone admissible; affine is not.

## 10. Five compile-clean cuts

The implementation sequence is exactly:

1. Generic Match;
2. Ownership;
3. compiler-local View admission;
4. private task identity/digest and sink preparation;
5. atomic public switch.

Cut 3 has no Cut 4 type dependency. Cut 4 does not claim a private public-enum
variant and does not publish final TaskSpec/runtime schemas. Cut 5 changes
`RuntimeValue` and every exhaustive consumer in one protected commit, publishes
all final task/runtime/bundle/snapshot/adapter surfaces, updates generated
artifacts/fixtures and deletes every old String/dual route.

The exact crates, feature gates, APIs, deletions and cargo gates are in
`COMPILE_CLEAN_SEQUENCE.md`.
