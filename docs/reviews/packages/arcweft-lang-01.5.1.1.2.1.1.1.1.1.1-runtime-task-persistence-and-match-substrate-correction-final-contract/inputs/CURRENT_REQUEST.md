# Lang-01.5.1.1.2.1.1.1.1.1.1 — runtime task persistence and Match substrate correction

## Sequence, inputs, and precedence

This is a narrow mandatory nonnumeric correction to the returned
Lang-01.5.1.1.2.1.1.1.1.1 runtime-Need/View-Match package. It does not reopen
the already selected Need/task identity roles or any numeric AWBC allocation.

Required retained inputs are:

- the parent
  [runtime Need instance and View Match admission correction](2026-08-22-lang-01.5.1.1.2.1.1.1.1.1-runtime-need-instance-view-match-admission-correction.md);
- the retained returned archive
  [`arcweft-lang-01.5.1.1.2.1.1.1.1.1-runtime-need-instance-view-match-admission-correction-final-contract.zip`](../packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1.1-runtime-need-instance-view-match-admission-correction-final-contract.zip),
  SHA-256
  `2B9B55043E8168D99838C81048E13F752A75B03F48293010BB36B5401043DB0B`;
- its searchable
  [frozen mirror](../packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1-runtime-need-instance-view-match-admission-correction-final-contract/README.md);
- the
  [repository intake and reconciliation](../../implementation/2026-08-22-lang-01-5-1-1-2-1-1-1-1-1-runtime-need-view-return-intake.md);
- maintained
  [AWBC runtime](../../02-runtime/executable-runtime-core.md),
  [Need timeout](../../02-runtime/need-timeout.md),
  [scheduler](../../02-runtime/async-scheduler.md), and
  [pattern runtime](../../02-runtime/control-flow-runtime.md) contracts; and
- current production at
  `17b384a36e1412cc7e7d9f13073d8dd33dcb5cbc`.

Current production, maintained stable documentation, this correction, and
later accepted contracts take precedence over stale package observations.
Every Arcweft-owned version marker remains exactly `1`. Do not add a
compatibility reader, legacy snapshot row, String fallback, second value
digest grammar, identity alias, or dual task carrier.

## Frozen decisions — out of scope

The return MUST preserve these decisions without reopening alternatives:

1. `NeedProducerInstanceKey` commits producer family, contract, plan, site,
   payload type, and the sole canonical runtime-value argument digest.
2. Join uses ordinal zero. AlwaysStart uses a journal-owned counter beginning
   at one and receives a distinct NeedId and TaskId per accepted launch.
3. TaskKey excludes ordinal; TaskId includes ordinal exactly once.
4. reusable handles are Join-only; AlwaysStart handles are accepted-launch
   outputs.
5. GenerationId is core-owned; correlation is derived by the sole task host.
6. fixed producer/Need/task IDs reject all-zero input without rehash; semantic
   digests accept their complete hash outputs and use `Option` for absence.
7. generic Match, Need producer admission, and retained View admission are
   separate products.
8. current `ViewProgramId` and `AcceptedViewProgramRevision([u8; 32])` keep
   their current roles; revision is not producer identity.
9. domain errors remain Ready payloads, infrastructure failures remain typed
   runtime failures, and cancellation remains nonreturning cancellation.
10. opaque value class/persistence is mandatory accepted-catalog evidence.
11. the maintained semantic-range opcode allocation, dense function kinds,
    function-flag bits, canonical u32 varint, final encoder buffer, direct
    borrowed reader, and no-tombstone policy remain final.

No numeric table is requested in this return.

## Why the returned contract is not implementable

The repository intake establishes nine blocking crossings:

1. Plain snapshot-only producer values are admitted but the sole canonical
   digest rejects them.
2. runtime-owned Timeout and AwaitMany aggregate work is forced through a
   `HostTaskRequest` and host adapter.
3. no concrete type owns journal, ordinal allocation, adapter preparation, and
   publication atomically.
4. an impossible `AdapterCommit` error branch remains despite infallible
   commit.
5. NeedHandle's declared NeedId-only value identity disagrees with derived
   structural equality.
6. required task/group/spec/Need/observer snapshot rows are undefined.
7. Match/site/admission digests depend on undefined or nonexistent semantic
   substrate types.
8. the bundle row embeds session-local `ExprId` through `CheckedMatchRef`.
9. ownership and publication cuts rely on missing carriers and reverse
   dependencies.

A validator that only confirms agreement among package files does not close
these repository crossings.

## Mandatory correction 1 — canonical value identity versus constant admission

Keep `arcweft_core::entry::RuntimeValueDigest` and the existing exhaustive
canonical RuntimeValue visitor as the sole identity grammar.

The corrected visitor MUST encode and directly hash an opaque value when:

- `RuntimeOpaqueValueClass::Plain` is exact;
- persistence is either `ConstantAndSnapshot` or `SnapshotOnly`;
- producer, semantic type identity, class, persistence, and payload validate;
  and
- normal recursion/node/byte limits pass.

Both persistence modes use the existing opaque tag and transcript. Do not add
a producer-argument-only serializer or digest. Byte-sink and BLAKE3-sink paths
must have identical grammar, limits, ordering, and first errors.

Constant publication remains stricter. Every constant-lowering, runtime-plan
constant, dialogue/config constant, command constant, and equivalent current
caller MUST use an explicit constant-admission fence before or together with
canonical encoding. `SnapshotOnly` remains rejected there. The return must
enumerate and migrate every current caller that previously relied on the
canonical encoder's incidental persistence rejection.

Affine opaque handles remain rejected by retained View/producer admission.
The return must state separately whether a non-producer diagnostic/snapshot
digest may encode an affine handle; it may not change producer admissibility or
silently use constant admission as identity policy.

Required paired evidence using the same Plain+SnapshotOnly value:

- canonical bytes succeed;
- direct digest succeeds and equals `hash(canonical_bytes)`;
- Need producer argument admission and instance construction succeed;
- snapshot/save round-trip succeeds; and
- runtime-plan/constant publication fails at the explicit constant fence.

## Mandatory correction 2 — one typed execution owner

`TaskSpec` MUST replace its unconditional host request with one closed owner:

```rust
pub enum TaskExecution {
    Host(HostTaskRequest),
    Runtime(RuntimeTaskRequest),
}

pub enum RuntimeTaskRequest {
    AwaitManyAggregate(RuntimeAwaitManyAggregateTask),
    Timeout(RuntimeTimeoutNeed),
}
```

Names may be reconciled to an already accepted current owner, but there is one
enum field and no parallel `request: Option<_>` pair. The final returned
schemas must define every field, invariant, transition, event publication,
cancellation rule, work limit, snapshot row, and strict decoder for both
runtime variants.

`RuntimeTimeoutNeed` reconciles exactly with `docs/02-runtime/need-timeout.md`:
it owns output/source typed identities, requested limit, remaining duration,
and `NotStarted | Waiting | Resolved` phase; uses only `RuntimeStepInput.dt`;
and applies cancellation, normalized source terminal, expiration, then Pending
precedence. It is never sent to a host adapter.

The AwaitMany aggregate runtime row owns the aggregate correlation, exact
source length/order, child observer/correlation rows, bounded launch cursor,
outputs by source index, aggregate publication cursor, and terminal precedence.
Children may be host or runtime tasks according to their own final
`TaskExecution`; the aggregate itself is runtime-owned.

The return MUST provide a nine-family truth table mapping every
`NeedProducerFamily` to allowed `TaskExecution` rows and policy restrictions.
No family may be inferred from a debug label, host operation spelling, or
request variant at runtime.

`TaskLaunchAdapter::prepare_launch` and commit/rollback accept only the Host
row. Runtime tasks are staged and driven by the scheduler/journal itself.

## Mandatory correction 3 — one atomic scheduler/journal/adapter owner

Select one concrete final owner equivalent to:

```rust
pub struct RuntimeTaskScheduler<A: TaskLaunchAdapter> {
    config: RuntimeSchedulerConfig,
    journal: RuntimeTaskJournal,
    runtime_tasks: RuntimeTaskState,
    adapter: A,
    // deterministic queues and metrics
}
```

This type implements `TaskHost` and solely owns:

- generation journals and AlwaysStart ordinal counters;
- TaskKey groups, TaskId launches, Need cells, and observer rows;
- validation and identity derivation;
- host adapter prepare/commit/rollback;
- runtime-owned task staging and stepping;
- event normalization/application and terminal publication;
- cancellation transactions;
- save/restore/replay snapshots; and
- replacement rebind transactions.

The runtime driver is a consumer of this owner. It may supply step input and
request snapshot/replacement operations, but it must not own a second journal,
counter, or cross-object rollback protocol.

The returned API must show the exact borrow/ownership flow for `ensure_task`,
host event ingestion, runtime task stepping, observer registration, snapshot,
restore, and replacement. It must be implementable without `unsafe`, interior
global state, or a fallible action after irreversible commit.

## Mandatory correction 4 — prepare/commit error closure

Delete `TaskEnsureError::AdapterCommit`.

- `prepare_launch` performs every fallible reservation and returns an owned
  prepared token.
- journal insertion and counter updates are fully staged before commit.
- `commit_launch(prepared) -> ()` only exposes already prepared work and is
  infallible by trait contract.
- any precommit staging failure calls rollback and leaves the journal and
  ordinal counter unchanged.

Apply the same rule to replacement rebind. The returned error enums and
failure-precedence tables must contain only reachable branches.

## Mandatory correction 5 — NeedHandle semantic equality and generation use

Retain canonical `RuntimeValue::NeedHandle` identity as exactly tag 20 plus
NeedId. Do not add generation or the complete spec to canonical value identity.

Implement semantic equality/hash/order for `RuntimeNeedHandle` from NeedId only
or use a private semantic-key newtype consistently inside RuntimeValue. Do not
derive a conflicting structural `PartialEq` for the public value carrier.
Debug/spec/origin differences do not change value identity.

Structural validation remains strict at construction, snapshot restore, Await,
timeout, and replacement boundaries. Ordinary Await/timeout requires the
handle correlation generation to equal the active scheduler generation.
Cross-generation use fails with a typed error before observer or task mutation.
Only explicit replacement rebind may change the active generation while
preserving NeedId and ordinal and rederiving TaskKey/TaskId.

Required tests distinguish:

- same NeedId with diagnostic/spec-label differences: equal semantic values;
- same NeedId from a stale generation: equal value identity but rejected use;
- explicit valid replacement rebind: accepted with complete new correlation;
- tampered correlation/spec: rejected at construction/restore.

## Mandatory correction 6 — complete version-1 persistence schemas

Define, do not merely reference, every version-1 snapshot and replay row needed
by the final runtime, including at least:

- `RuntimeTaskJournalSnapshotV1` and generation rows;
- AlwaysStart ordinal counters;
- Task group rows and ordered launch mappings;
- complete TaskSpec/producer/execution rows;
- complete Task correlation and lifecycle rows;
- Need state and `Need<RuntimeNeedOutcome>` rows;
- host and runtime task request/state rows;
- observer rows;
- RuntimeNeedHandle rows;
- event, event-digest, and replay envelopes;
- AwaitMany aggregate/child state;
- Timeout remaining/phase/source subscription state; and
- replacement mapping/prepared rebind state where persistence is legitimate.

For each row specify exact fields, key order, Option representation, bounded
lengths, strict unknown/duplicate rejection, identity/digest rederivation, and
first-error precedence. Restore constructs private temporary maps, validates
all joins and runtime task invariants, and publishes the complete scheduler
state atomically. Every marker is exactly `1`; no `V2`, old reader, translation
table, or zero sentinel is allowed.

## Mandatory correction 7 — constructible Match and View semantic substrate

Use current semantic authority rather than invented generation types.

`CheckedMatchRef` is compiler-local lookup evidence bound to the exact current
`HirSnapshotId` plus `ExprId`, or to an already existing final-analysis key that
is proved equivalent. Do not introduce `AcceptedSemanticGeneration` unless the
same return defines its sole current owner, construction, and deletion of the
superseded lookup owner; the preferred correction is current `HirSnapshotId`.

The return MUST provide exhaustive, version-1 semantic transcripts for:

- every current `CheckedExpressionResolution` variant used by Match
  scrutinee, guards, and bodies;
- every current `CheckedPatternResolution`/pattern-family variant;
- literal payloads with exact integer/float/text rules;
- local and pattern bindings through stable pattern/declaration coordinates;
- callable references through accepted runtime callable/contract identity;
- project and accepted nominal references through their existing semantic
  identities and layouts;
- variant/record field identities and source-order child roles;
- guard class, coverage constructors, unreachable evidence, and bodies; and
- `AcceptedDeclarationSemanticId`, `StableCheckedValueCoordinate`, and
  `CheckedExpressionChildRolePath` construction.

No transcript may use raw HIR allocation numbers, source spans, source spelling,
debug names, hash-map order, or generic Serde. The returned test corpus must
include exhaustive enum-variant coverage and differential tests that vary HIR
allocation/source spans while preserving semantic meaning.

## Mandatory correction 8 — compiler-local rows versus persistent bundle rows

Keep a compiler-local catalog row containing `CheckedMatchRef` for exact lookup.
Define a separate persistent projection; do not embed the local row:

```rust
pub struct AcceptedViewMatchBundleRowV1 {
    pub version: ViewMatchBundleRowVersion, // exactly 1
    pub program: ViewProgramIdProjection,
    pub accepted_revision: AcceptedViewProgramRevisionProjection,
    pub site: ViewMatchSiteIdProjection,
    pub checked_match: CheckedMatchSemanticDigestProjection,
    pub view_admission: CheckedViewMatchAdmissionDigestProjection,
    pub need_admission: CheckedNeedProducerAdmissionDigestProjection,
    pub ownership: OwnershipEvidenceDigestProjection,
    pub producer_contract: NeedProducerContractDigest,
    pub payload_type: RuntimeTypeSemanticDigest,
    pub plan: TaskPlanSemanticDigest,
    pub arguments: RuntimeValueDigest,
    pub resource_dependency: Option<ResourceDependencyDigestProjection>,
}
```

Exact projection names may follow legitimate dependency direction, but the
field roles are closed and the row contains no `CheckedMatchRef`, `ExprId`,
`HirSnapshotId`, `SourceSpan`, or compiler-only certificate object. Bundle
validation joins these digests against compiler/AWBC products and current
accepted revision without minting semantic identity.

## Mandatory correction 9 — carrier-backed ownership matrix

Regenerate the exhaustive current `TypeKind` matrix from real current owners.

- `Predicate` is a leaf; it has no child recursion.
- a successful `SnapshotClone` row must name a current or fully specified
  same-cut runtime projection, live RuntimeValue carrier, canonical identity,
  and snapshot codec.
- `Shared<T>` is `MissingRuntimeSnapshotOwner` unless the return defines all
  four owners plus construction/restore invariants in the same contract.
- future or planned carriers are not evidence.
- all resource/Agent/opaque/project nominal rows must cite the exact current
  carrier shape rather than a family name.

If Shared is introduced, use one closed core-owned carrier and one snapshot
row; do not represent it as an opaque name, extension trait, or side table. If
that full design is not required for the current feature, reject it and leave
Shared carrier work outside this correction.

The machine matrix, prose matrix, classifier match, tests, and deletion matrix
must agree on every current variant and recursion edge.

## Mandatory correction 10 — compile-clean deletion sequence

Publish only already-constructible final owners. The required sequence is:

1. **Generic Match:** define the exhaustive semantic substrate, bounded
   coverage, current snapshot-bound lookup, and generic Match fact only.
2. **Ownership:** publish mandatory opaque evidence and only carrier-backed
   ownership/producer-admission rows. No View or task carrier publication.
3. **Compiler-local View admission:** publish stable site/admission facts and
   compiler-local catalog rows only. No bundle/runtime row that depends on
   unpublished task digest types.
4. **Private preparation:** add standalone task identity/digest types and
   private sink implementation plumbing. Do not add a public RuntimeValue enum
   variant or public final task schema in this cut.
5. **Atomic public switch:** together publish `RuntimeValue::NeedHandle`, final
   TaskSpec/TaskExecution/correlation/events, the concrete scheduler/journal/
   adapter owner, runtime tasks, bundle projection, View runtime, Await/
   AwaitMany/timeout, every snapshot/replay/replacement row, adapters,
   generated artifacts, fixtures, and delete every old String/dual route.

A Rust enum variant is never "private" relative to exhaustive consumers. Cut 5
must update every exhaustive RuntimeValue visitor in the same protected commit.
Each published cut must list exact crate gates and compile using only types
introduced in that or earlier cuts.

## Source reconciliation and deletion inventory

The return MUST regenerate its source evidence and deletion matrix from the
current tree. It must not name nonexistent adapter crates as implementation
targets. At minimum audit current String task/Need consumers in core engine,
suspension/flow, AWBC VM/product-step, runtime plan/final flow, line task,
scheduler, runtime driver, bundle, save/snapshot, tests, and generated fixtures.

Every obsolete constructor, String identity, suffix helper, partial event DTO,
old snapshot field, direct-Await surrogate, plan self-digest, and copied View
row must have one real current path and one deletion cut. Unknown or absent
paths are evidence failures, not placeholder rows.

## Required returned package

Return one independently throwable ZIP containing at least:

1. final contract and decision register;
2. exact Rust-shaped schemas for every named owner;
3. canonical identity and semantic transcript specification;
4. host/runtime producer-family execution truth table;
5. lifecycle, event, timeout, AwaitMany, cancellation, and replacement state
   machines;
6. complete persistence/replay schemas and strict decode rules;
7. exhaustive Match/pattern/value-coordinate transcript tables;
8. carrier-backed ownership matrix;
9. corrected dependency and owner/API map;
10. real source evidence and deletion matrix;
11. corrected five-cut compile-clean sequence;
12. focused, property, differential, tamper, rollback, snapshot, replay,
    replacement, and structural-absence test matrix;
13. machine-readable equivalents validated against prose; and
14. a read-only package validator with negative self-tests for every crossing
    identified by this request.

The validator MUST fail if it reintroduces `AdapterCommit`, an unconditional
HostTaskRequest field, undefined snapshot types, compiler-local IDs in bundle
rows, Shared without a carrier, Predicate child recursion, a private
RuntimeValue variant claim, or a Cut 3 dependency on Cut 4 types.

`READY_FOR_IMPLEMENTATION` is valid only when every normative type and
transcript is constructible from current or same-cut defined owners, every
runtime family has an execution route, every persisted row has a complete
strict schema, and every public cut is compile-clean. Open result-changing
choices must be returned as explicit blockers rather than guessed.
