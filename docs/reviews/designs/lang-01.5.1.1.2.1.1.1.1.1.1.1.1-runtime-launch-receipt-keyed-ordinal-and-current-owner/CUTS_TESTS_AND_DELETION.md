# Compile-clean cuts, deletion, and acceptance tests

## Cut graph

Each numbered cut is committed only after its stated compile/test gate passes.
Subcuts are ordered within one reviewable semantic cut; no downstream public
API is published between them.

### Cut 1 — generic Match identity and child edges

Dependencies: current HIR and sema only. No task, Need, snapshot, or View
runtime type may appear.

1. In `arcweft-lang-hir`, add HIR-only nested paths, roles, and
   `HirExpressionChildEdge`; implement `child_edges`; make
   `direct_expression_children` and recovery operand indexing projections of
   that owner. Delete the old duplicate child-order switch in the same subcut.
2. In `arcweft-lang-sema`, add checked path/role enrichment, retained explicit
   tags, Match/pattern/guard/coverage transcripts, and current
   `CheckedCallableCatalog` joins. No `CheckedCallableCatalogV1`, runtime
   callable fallback, or source-spelling identity exists.

Gate: changed-crate checks/tests, 38-family edge differential, callable
catalog differential, missing-evidence negatives, and dependency graph proving
HIR has no core/sema edge.

### Cut 2 — ownership projections

Dependencies: Cut 1 plus current core value/type owners. No public task type.

Add complete projection structs/enums and the exhaustive TypeKind classifier.
Project/accepted nominal success uses current accepted catalog facts. Option
and Result use `RuntimeCheckedType::variant_case`; AgentBuiltin and every other
nested family are exhaustively destructured. The Need success certificate is
computed privately but is not publicly constructible until Cut 5 adds the one
live/snapshot carrier.

Gate: all 85 outer variants, every nested enum case, carrier construction,
canonical digest, unsupported-first-error, and no name-only projection.

### Cut 3 — compiler-local Match/View admission

Dependencies: Cuts 1–2 and current View identities only.

Add compiler-local `HirSnapshotId + ExprId` lookup and role-path construction.
Persistent View projection contains stable semantic identities/digests only;
it contains no HIR/compiler ID or copied compiler row. This cut remains
task-type-free.

Gate: compiler cache/reallocation differential and persistent bundle schema
tests.

### Cut 4 — standalone core identity and catalog substrate

Dependencies: current core only. Public task/value variants remain unchanged.

Add/finalize `GenerationId`, producer/task/correlation/capability IDs, canonical
value sink support for the current value graph, `HostOperationCatalog`, and the
core-owned upper View validation protocol traits. Add the structured
`RuntimeTaskPlanTable` as a field of the existing `RuntimePlan`; do not publish
`TaskValidationAuthority` or adapter envelopes yet. AWBC continues to use its
existing `AwbcProgram.task_plans` table.

Gate: identity transcripts, `GenerationId(0)` and Join ordinal `0` acceptance,
all-zero rejection restricted to fixed producer/Need/task/cancel-command
identities, all-zero acceptance for every semantic digest including
`RuntimeValueDigest`, Host catalog canonical order, route/capability contracts,
structured/AWBC plan digest differential, and crate dependency checks.

### Cut 5 — atomic task/runtime/persistence switch

Dependencies: Cuts 1–4. This is one atomic public-contract cut.

Publish together:

- final validated `TaskSpec`, read-only producer/correlation APIs,
  `TaskExecution`, Runtime requests, producer templates, and
  `TaskValidationAuthority`;
- `RuntimeGenerationJournal`, keyed ordinals, observers, committed launch/Need
  rows, core-owned `JournalTransaction`/sealed after-image/apply proof, public
  private-field accepted receipt, and final Need handle;
- all adapter batches, inspectable receipts, opaque-token wrappers, and
  launch/restore/rebind/cancel typed constructors/getters and trait methods;
- scheduler single/batch plan/apply engine, AwaitMany, timeout, cancellation,
  event ordering, restore, and replacement rebind;
- `RuntimeValue::NeedHandle`, canonical identity extension, and the in-place
  `AwbcRuntimeValueSnapshot` evolution under one outer authority;
- final public Need ownership certificate; and
- maintained scheduler/timeout/AWBC documents and generated schemas.

Every exhaustive match is allowed to break compilation until it is migrated;
no wildcard, compatibility wrapper, optional old field, or dual reader is
permitted.

Gate: all focused tests below, workspace check/clippy/tests selected by current
policy, structural audit/gate, and exact generated schema/codec fixtures.

## Same-cut deletion inventory

| Current/returned route | Final action |
|---|---|
| string-backed/current `TaskId`, `TaskKey`, `NeedId` constructors | replace directly with fixed typed identity owners; no alias |
| current `TaskSpec { id, key, ..., request, debug_label }` | replace with producer/class/priority/scope/policy/outcome/execution/debug; IDs derive only in journal transaction |
| current `TaskHandle` and direct-Await surrogate | delete; `RuntimeNeedHandle` is the sole direct Await carrier |
| `NeedHandleOrigin` | delete; state is ReusableJoin or AcceptedLaunch |
| `MakeNeedHandle` Runtime or AlwaysStart verification/lowering | delete/reject; Host+Join only |
| scalar `next_always_start_ordinal` and scalar batch after-image | delete; keyed map/rows only |
| scheduler-owned `RuntimeJournalBatchDelta` / `RuntimeObserverBatchDelta` | delete; core `JournalTransaction` is the sole journal/observer mutation owner |
| public fields, `pub(crate)` seams, or raw constructors for committed journal rows | delete; validated constructors/read-only getters plus sealed core apply only |
| finished-child/spec-only AwaitMany request | delete; captured/items/template owner only |
| per-child committed aggregate `ensure_task` | delete; shared internal plan/apply engine only |
| ephemeral observer allocation | delete; journal/snapshot counter only |
| inaccessible `pub(crate)` accepted proof or raw-field constructor | delete; public private-field journal receipt only |
| unconstrained opaque `PreparedLaunch` with hidden capabilities | delete; inspectable receipt + opaque token wrapper |
| `HostAdapter::submit` and `cancel(&TaskId) -> bool` | delete with direct worker-start timing; do not wrap |
| adapter commit error or `TaskEnsureError::AdapterCommit` | delete; commit has no Result |
| `TaskContractCatalogV1` | delete; one borrowed `TaskValidationAuthority` |
| `CheckedCallableCatalogV1` | delete; use current `CheckedCallableCatalog` |
| per-value `AwbcExecutableAuthorityRefV1` | delete; one outer nonserialized authority |
| redefined `DenseSeq`, `Bools`, or `Units(DenseSeqStorage<()>)` | delete; reuse current DenseSeq exactly |
| reversed Option map | delete; call current `RuntimeCheckedType::variant_case` |
| blanket `AgentBuiltin(_)` ownership arm | delete; nested exhaustive match |
| HIR-owned checked/core role payload | delete; HIR-only edge then sema enrichment |
| undefined/name-only projection registry rows | delete or replace with the complete same-cut schema; no placeholder production type |
| blanket active-Host snapshot rejection | delete; selected restart policy |
| sequence-before-TaskId event key | delete; maintained order only |
| compatibility snapshot reader/version bump | absent; version remains 1 |

## Focused and property tests

### Handle and MakeNeedHandle

- Host+Join constructs ReusableJoin with complete spec and zero journal/adapter
  calls.
- Runtime+Join and Host/Runtime+AlwaysStart reject before digest/counter/
  adapter mutation.
- Reusable Await ensures once, checks exact correlation, then registers one
  observer; accepted Await never ensures.
- a scheduler in another crate obtains a public receipt only after apply and
  constructs AcceptedLaunch; compile-fail fixtures reject field construction,
  raw receipt construction, uncommitted receipt, and a `pub(crate)` seam.
- equal NeedId handles with different structural/debug evidence compare, hash,
  and order equal; stale-generation use rejects.

### Keyed ordinals and observers

- `GenerationId(0)` is accepted and round trips; mutating its owner back to
  `NonZeroU64` fails the design validator.
- Join uses ordinal `0`; AlwaysStart, observer, route, operation, capability,
  and next-counter scalar IDs reject zero. Every listed semantic digest accepts
  all-zero bytes while fixed producer/Need/task/cancel-command IDs reject them.
- producer A ordinals are `1,2,...` under every permutation of unrelated B
  launches; A NeedId/TaskId transcripts are invariant to B interleaving.
- same-producer monotonicity, `u64::MAX` overflow, rollback gap freedom,
  unique/sorted snapshot rows, stale/missing/duplicate counter rejection, and
  restore continuation.
- observer start/monotonicity/overflow, removal nonrewind, batch rollback, and
  every persisted/reference row below next candidate.

### Adapter transactions

- launch receipt exact order/cardinality/generation/correlation/operation;
  missing, duplicate, reordered, foreign-route, zero, reused-active, and
  mismatched capabilities each rollback with no publication.
- mixed existing Join/new Host/new Runtime/AlwaysStart AwaitMany batch commits
  as one result; every prepare failure position reverses all prior tokens and
  preserves task/observer counters and aggregate status.
- launch → snapshot → restore → rebind → cancel retains exact capability pair;
  duplicate cancel input rejects before prepare and repeated committed cancel
  returns AlreadyRequested without adapter work.
- worker visibility count is zero before commit and exactly one afterward;
  post-commit failures become InfrastructureFailure events.
- route capability candidates are gap-free on rollback; the same batch after a
  rollback and without intervening committed route work receives the same
  launch/cancellation pair.
- compile fixtures in the scheduler and Host adapter crates construct/read
  every crossing batch, row, spec, correlation, producer, and catalog field
  only through the selected public API; making any required method private or
  any protected field public fails the design/structural gate.
- removing any ensure/restore/rebind/cancel coordinator, journal sealed apply,
  HIR edge `child`/`role` accessor, or adding a raw committed-row constructor
  fails a negative fixture.
- a stale generation/revision at journal apply rolls prepared tokens back in
  reverse order and leaves both after-images untouched; fault injection proves
  no fallible call occurs after a successful journal swap, and handles/accepted
  receipts are not observable until runtime swap and adapter commit complete.

### AwaitMany and timeout

- captured/item/index child argument bytes and child specs rederive exactly;
  caller-supplied digest/spec APIs do not exist.
- duplicate item values at different indices remain distinct; aggregate base
  is exactly Tuple(source items).
- tampered captured value, item, index, template, execution, derived spec,
  correlation, or status rejects restore before prepare.
- limit, output order, failure/cancel precedence, same-step timeout race,
  restart, and late source publication remain deterministic.

### Snapshot/value isomorphism

- every current RuntimeValue/iterator/sequence/reduction/Agent/predicate/
  variant nested case round trips under one authority and work limits.
- every existing DenseSeq case round trips; Units 0 and practical maximum,
  Bool, wrong tag/storage, and u64/usize overflow are covered.
- AWBC function validates against the one outer program; a foreign function ID
  and Structured function reject before bytes are exposed; serialized rows
  contain no generation/program authority.
- Need reusable/accepted states round trip; missing committed accepted row,
  stale generation, wrong capabilities/catalog, and forged correlation reject.
- unknown/duplicate field, noncanonical length/varint, trailing bytes, invalid
  ordinal, and depth/node/byte budget negatives.

### Match/callable/ownership

- all 38 HIR families and nested Choice/dialogue/line-plan fixtures satisfy
  child-edge projection equality before old switch deletion.
- current selected call facts join the current catalog; missing/ambiguous/
  intrinsic/Method cases follow `MATCH_CHILD_EDGES.md`.
- arena/span/spelling perturbation stability and accepted field/callable/role
  sensitivity.
- all 85 TypeKind variants and all AgentBuiltin, ArrayLength,
  IteratorStateKind, MapKind, HandleState, character-dialogue, character-
  nominal, project-nominal, and accepted-nominal cases.
- Option Some=0/None=1 payload rules and Result Ok=0/Err=1 across lowering,
  Match, canonical bytes, snapshot, and restore.

## Validation selection

This design cut is documentation/Rust-shaped design only. Its own acceptance
uses link/format checks, `git diff --check`, the repository-aware design
validator, and negative self-tests. Production implementation Cut 5 is a broad
cross-crate public runtime contract and therefore requires the current
workspace check, Clippy, workspace tests, structural audit/gate, and matching
Tier 2 runtime targets. Cargo receives no explicit job count.
