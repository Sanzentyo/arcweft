# Lang-01.5.1.1.2.1.1.1.1.1.1 task substrate return intake

Date: 2026-08-22
Inspected Git commit: `3670625a02b9e7e8578b57fc7b148a1758a17dba`
Working tree before intake: clean; `main` matched `origin/main`

## Intake result

- Archive safety and integrity: `PASS`
- Internal package validator and negative self-tests: `PASS`
- Repository reconciliation: `FAIL`
- Classification: `DESIGN_NOT_READY`
- Production implementation: `BLOCKED_FOR_THE_RETURNED_COMBINED_CONTRACT`
- Open questions claimed by the package: none
- Production source, tests, fixtures, or generated artifacts changed by the
  package: none

The return corrects several important predecessor failures, but it still does
not define one constructible runtime contract. The package validator checks
agreement among its inventories; it does not prove that the invented rows are
isomorphic to current live carriers or that the claimed transactions can be
performed by the published APIs.

## Retained archive

External source archive:

- path:
  `D:/sanze/Downloads/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1-runtime-task-persistence-and-match-substrate-correction-final-contract.zip`
- byte length: 197,348
- SHA-256:
  `034A2EEAB2D083B5BB4496F4EE63040B2F93B30ABDDA1B18E93138E28B65391B`

The unchanged byte authority is retained at
[`docs/reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1-runtime-task-persistence-and-match-substrate-correction-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1-runtime-task-persistence-and-match-substrate-correction-final-contract.zip).
Its 61-file byte-identical frozen mirror is retained under
[`docs/reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1-runtime-task-persistence-and-match-substrate-correction-final-contract/`](../reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1-runtime-task-persistence-and-match-substrate-correction-final-contract/README.md).

## Performed and passed

- Verified one exact top-level wrapper, 61 members, 706,441 uncompressed
  bytes, no absolute/drive/parent-traversal path, duplicate entry, case-fold
  collision, or symbolic link.
- Verified the retained ZIP is byte-identical to the external attachment.
- Verified every extracted file is byte-identical to its ZIP member.
- Independently verified all 59 `MANIFEST.json` payload rows by byte length and
  SHA-256.
- Verified `MANIFEST.json` SHA-256
  `606302D68623E7BD8C815813BC1614D5BD38019D4AB7D2EA70B5679391CBC2C8`
  equals `MANIFEST.sha256`.
- Verified the packaged request is byte-identical to the maintained request:
  20,408 bytes, SHA-256
  `804F68C052640FE3964E70BFE011CAD2C4429873A70B790C3A0526B5F46C7E6E`,
  Git blob `6b3d614e7813fa6552e84f15610175633470227d`.
- Inspected the standard-library-only package validator before execution.
- Ran the validator against the retained extracted package with
  `uv run --no-project`; it reported 9 producer families, 72 persistence
  schemas, 85 TypeKind rows, 94 test rows, and `PASS`.
- Ran all 10 negative self-tests; all reported `PASS`.
- Enumerated all 62 retained `docs/reviews/**/*.zip` archives at the
  reviewable cut. The sorted relative-path, byte-length, SHA-256 LF transcript
  hashes to
  `B728FB24D8F748941B2CD3B534A1B7D5D47871E5EF495699E77031BF2026F504`.
- Independently verified the package's cited source blobs against current Git.
- Obtained an independent Sol-max design audit of the complete package against
  current source and maintained contracts.

## Corrections accepted from this return

The next return must retain these improvements:

- Plain+SnapshotOnly canonical identity is separated from explicit constant
  admission, with no second RuntimeValue digest grammar.
- `TaskSpec` has one closed `TaskExecution::Host | Runtime` field.
- Timeout and AwaitMany aggregate are scheduler-owned runtime requests rather
  than host adapter work.
- `TaskEnsureError::AdapterCommit` is deleted and commit remains infallible.
- RuntimeNeedHandle semantic equality/hash/order is NeedId-only while use
  validation remains structural and generation-aware.
- `CheckedMatchRef` uses current `HirSnapshotId + ExprId` only as
  compiler-local lookup evidence.
- compiler-local View rows and persistent bundle rows are separate.
- Predicate is a TypeKind leaf and Shared rejects
  `MissingRuntimeSnapshotOwner`.
- Cut 3 does not depend on Cut 4 task types; the public RuntimeValue variant
  appears only in atomic Cut 5.
- numeric AWBC allocation remains out of scope.

## Failed repository reconciliation

### Reusable Join handles cannot start

The return freezes reusable pre-launch Join handles, but its
`RuntimeNeedHandle` stores only correlation, producer, outcome, and an
undefined `NeedHandleOrigin`. It stores neither the complete `TaskSpec` nor its
`TaskExecution`, and defines no `ReusableJoin` versus `AcceptedLaunch`
construction path. Await therefore has no Host/runtime request to submit.

The truth-table row `MakeNeedHandle + Host + JoinSameKey` is consequently
ambiguous between eager launch and lazy handle construction. Eager launch
violates the retained MakeNeedHandle behavior; lazy construction is impossible
with the returned carrier.

### AwaitMany child identity cannot be rederived

`RuntimeAwaitManyAggregateRequest` stores source items and complete child
TaskSpecs but not the captured arguments required by the retained child
transcript:

```text
(captured arguments, exact source index, item)
```

The stored child `RuntimeValueDigest` is not invertible. Restore cannot perform
the package's claimed child-argument rehash or prove that a child spec belongs
to its source index.

### AwaitMany batch rollback has no API

The stepping prose calls per-child `ensure_task_internal`, which commits each
child journal row, ordinal, runtime state, and adapter token. If a later child
fails, those committed launches cannot be rolled back. The prose suggests a
whole-batch transaction but defines no batch inspection, delta, prepared-token
collection, observer delta, or commit API.

### Observer allocation is not owned or persisted

`register_observer` allocates a next observer ID, but neither
`RuntimeTaskGeneration` nor its version-1 snapshot contains an allocator or
next-ID counter. Removal followed by restore may reuse an ID while stale
references remain. Failure/no-gap behavior is likewise undefined.

### Host cancellation cannot execute the claimed transaction

The cancellation state machine requires prepared/infallible Host cancellation,
but `TaskLaunchAdapter` exposes only launch, restore, and rebind methods. There
is no `PreparedCancel`, Host cancel batch, or prepare/commit/rollback API.

### Adapter placement and protocol do not preserve Sans-I/O

The owner map assigns `TaskLaunchAdapter` to an upper host-adapter layer while
the core-only `arcweft-runtime-scheduler` directly depends on and invokes it.
Current scheduler layering permits a core-owned Sans-I/O protocol trait and an
upper-layer implementation, not a dependency from scheduler to the host
adapter crate.

Current `HostAdapter::submit` may reject or begin work at submission time and
`cancel -> bool` is not a rollback token. Merely implementing the returned
trait cannot make commit infallible. Prepare must only validate and reserve an
unpublished queue slot; commit exposes that slot; actual I/O begins afterward
and reports failure as a TaskEvent.

### The 72-row graph is not isomorphic to current values

The reference graph is name-closed but several rows cannot round-trip current
live values:

- `RuntimeOpaqueValueSnapshotV1.payload: BoundedBytes` conflicts with the
  current recursive `Box<RuntimeValue>` payload;
- one `{ source, cursor }` iterator row loses current Range, Values, and
  Witness variants and witness method identity;
- one `{ kind, items }` sequence row loses Values, Dense, TupleColumns, and
  RecordColumns shapes;
- the function row loses Structured/AWBC body distinction, owning plan/site,
  remaining parameters, captures, and bound arguments;
- the reduction row loses owner, state, and commands;
- `RuntimeCheckedTypeProjectionV1` and `RuntimeAgentValueProjectionV1` are
  referenced without closed variant schemas; and
- `RuntimeHostOperationId` has no current or returned owner/constructor.

The package validator only verifies named reference closure and therefore does
not detect these carrier mismatches.

### Match transcripts still omit constructible joins and exact tags

The tables list current checked variant names, but do not define numeric
semantic tags or an exhaustive `HirExprKind`/child-role inventory. They say
`Structural` emits an expression-family tag and “every other current child
position” without specifying either authority.

Several checked facts also do not currently retain the proposed payload:

- `ImplicitCallable` does not own RuntimeCallableId/CallableContractHash;
- `Call` is a unit resolution variant; and
- selected Method evidence retains a HirName rather than the proposed accepted
  runtime callable identity.

The same cut needs an exact join to the existing checked callable
catalog/digest or an explicit final projection. Source spelling cannot fill the
gap.

### Ownership success rows still use invented or conflicting carriers

- numeric Copy rows cite nonexistent
  `RuntimeValueSnapshotV1::IntOrUInt`, while the returned snapshot enum has
  separate `Int` and `UInt` variants;
- Result/Option/Tuple rows retain “Tuple or Variant as selected” instead of one
  exact type-to-carrier projection;
- Agent/dialogue projection owners are only partial summaries without closed
  case/field maps and construction/restore rules; and
- several Cut-2 success rows cite a Cut-5 snapshot owner rather than a current
  or same-cut owner.

The four-column evidence rule is therefore not actually satisfied.

### Event normalization changes accepted behavior

Maintained and predecessor authority normalizes by:

```text
(logical_epoch, task_id, sequence)
```

The return silently changes this to:

```text
(generation, logical_epoch, sequence, task_id)
```

Putting sequence before TaskId changes same-epoch cross-task application
order. If multiple generations must be sorted together, generation may prefix
the maintained tuple, but TaskId must remain before sequence.

### Snapshot quiescence statements disagree

One returned rule rejects snapshot while any nonquiescent Host task is active;
another persists active `Restartable` rows and prepares them on restore. The
latter is the selected useful behavior. Snapshot rejection must be limited to
prepared transactions and active `MustBeQuiescent` rows.

## Blocking correction

The returned combined contract is blocked by:

- [`Lang-01.5.1.1.2.1.1.1.1.1.1.1`](../reviews/requests/2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1-runtime-handle-batch-and-snapshot-isomorphism-correction.md).

That request preserves the accepted corrections and closes reusable handle
construction, captured child identity, whole-batch atomicity, observer and
cancel ownership, Sans-I/O adapter placement, current-carrier snapshot
isomorphism, exact Match/ownership projections, and maintained event ordering.

No Rust, Cargo manifest, production fixture, generated artifact, Clippy, AOT,
platform, browser, or runtime test was changed or run for this intake cut.
