# Lang-01.5.1.1.2.1.1.1.1.1 runtime Need/View return intake

Date: 2026-08-22
Inspected Git commit: `17b384a36e1412cc7e7d9f13073d8dd33dcb5cbc`
Working tree before intake: clean; `main` matched `origin/main`

## Intake result

- Archive safety and integrity: `PASS`
- Internal package validator: `PASS`
- Repository reconciliation: `FAIL`
- Classification: `DESIGN_NOT_READY`
- Production implementation: `BLOCKED_FOR_THE_RETURNED_COMBINED_CONTRACT`
- Open questions claimed by the package: none
- Production source, tests, fixtures, or generated artifacts changed by the
  package: none

The return correctly preserves the already selected numeric AWBC allocation and
closes important predecessor identity questions. It is nevertheless not
implementable as one final contract. Several normative schemas cannot represent
the required runtime behavior, and other referenced persistence and semantic
owners are absent or retain session-local identity. The package's own
`READY_FOR_IMPLEMENTATION` claim is therefore rejected.

## Retained archive

External source archive:

- path:
  `D:/sanze/Downloads/arcweft-lang-01.5.1.1.2.1.1.1.1.1-runtime-need-instance-view-match-admission-correction-final-contract.zip`
- byte length: 150,949
- SHA-256:
  `2B9B55043E8168D99838C81048E13F752A75B03F48293010BB36B5401043DB0B`

The unchanged byte authority is retained at
[`docs/reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1.1-runtime-need-instance-view-match-admission-correction-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1.1-runtime-need-instance-view-match-admission-correction-final-contract.zip).
Its 44-file byte-identical frozen mirror is retained under
[`docs/reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1-runtime-need-instance-view-match-admission-correction-final-contract/`](../reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1-runtime-need-instance-view-match-admission-correction-final-contract/README.md).

## Performed and passed

- Verified one exact top-level wrapper, 44 members, 516,320 uncompressed
  bytes, no absolute/drive/parent-traversal path, no duplicate entry, no
  case-fold collision, and no symbolic link.
- Verified the retained ZIP is byte-identical to the external attachment.
- Verified all 44 extracted files are byte-identical to their ZIP members and
  all 42 `MANIFEST.json` payload rows match independently calculated lengths
  and SHA-256 values.
- Verified `MANIFEST.json` SHA-256
  `7A4A5561E6E707C1F6B331D5E7EB64E849A597C3813A9837CF104B68EE4E17EF`
  equals `MANIFEST.sha256`.
- Verified the packaged request is byte-identical to the maintained request:
  26,729 bytes, SHA-256
  `0152F1DD5F6FD315722F729700D3B94D1B0DAA596A59445313E7796BDDDE8322`,
  Git blob `7ed008dec6eddb820e228ea0803bf97a1ead2c36`.
- Inspected the standard-library-only validator before execution. It writes
  only temporary self-test copies and has no repository, subprocess, or
  network mutation path.
- Ran the validator against the extracted directory and retained ZIP with
  `uv run --no-project`; both reported `PASS`.
- Ran the validator's 16 negative self-tests; all reported `PASS`.
- Enumerated all 61 retained `docs/reviews/**/*.zip` archives at the
  reviewable cut. The sorted relative-path, byte-length, SHA-256 LF transcript
  hashes to
  `865A930AB1B268A26E2F5D3436F7CBBDD41CDD794E2D4B405D10C2C281DCAF5A`.
- Obtained an independent Sol-max design audit against current production and
  the complete returned package.

The package validator proves package consistency, not constructibility against
the repository. It does not cover the failures below.

## Accepted decisions retained by the correction

The correction does not reopen these returned decisions:

- `NeedProducerInstanceKey`, `NeedId`, `TaskKey`, and `TaskId` have distinct
  roles; Join uses ordinal zero and AlwaysStart allocates from one.
- `TaskKey` excludes the launch ordinal and `TaskId` includes it once.
- reusable handles are Join-only; AlwaysStart returns an accepted-launch
  handle.
- `GenerationId` moves to core and the host derives correlation.
- the existing `RuntimeValueDigest` and canonical `RuntimeValue` grammar are
  the only runtime-value identity authority.
- generic Match and retained View admission are separate products.
- current `ViewProgramId` and `AcceptedViewProgramRevision([u8; 32])` retain
  their current roles.
- opaque value class and persistence are mandatory accepted-catalog evidence.
- domain errors, infrastructure failure, and cancellation remain distinct.
- numeric AWBC opcode, function-kind, function-flag, varint, and encoder
  allocation remains the already maintained semantic-range allocation.

## Failed repository reconciliation

### Snapshot-only producer values have no identity

The return admits `RuntimeOpaqueValueClass::Plain` plus
`RuntimeOpaquePersistence::SnapshotOnly` as `SnapshotClone` for Need producer
arguments. `NeedProducerInstanceKey` then requires the sole
`RuntimeValue::try_digest` result. Current
`canonical_runtime_value_bytes` rejects that exact value as not
constant-admissible. Thus an admitted producer value cannot construct its
required identity.

Canonical identity and constant admission must be separated without adding a
second grammar. Plain snapshot-only values use the existing canonical opaque
transcript for bytes/direct hashing; constant publication retains an explicit
constant-only fence. Affine handles remain rejected producer values.

### Internal producers are falsely modeled as host tasks

Every returned `TaskSpec` contains `HostTaskRequest`, every
`NeedProducerTemplate` contains `HostTaskRequestTemplate`, and every new task
is sent to `TaskLaunchAdapter::prepare_launch`. That cannot represent the
runtime-owned AwaitMany aggregate or deterministic Timeout producer. Timeout
is driven by `RuntimeStepInput.dt`, and AwaitMany aggregate publication is
computed from child Need states; neither is host I/O.

The final task schema needs one typed execution owner which distinguishes host
requests from runtime-owned requests. Adapter prepare/commit is valid only for
the host row. Runtime rows are driven by the scheduler/journal using their
complete typed state and snapshot form.

### No concrete atomic transaction owner exists

The return assigns `TaskHost` implementation to the scheduler, journal state to
runtime-driver-facing schemas, and adapter coordination to runtime-driver
prose, but defines no concrete value that owns the journal, ordinal counters,
adapter, and `ensure_task` transaction together. Its rollback guarantees cannot
be implemented across those separate owners.

The correction selects one `RuntimeTaskScheduler<A: TaskLaunchAdapter>`-style
owner for journal, counters, adapter, runtime tasks, and `TaskHost`. The runtime
driver consumes this owner's APIs and snapshot/replacement transactions; it
does not coordinate a second journal transaction.

### Infallible commit has a recoverable error variant

`TaskLaunchAdapter::commit_launch` returns `()`, and the lifecycle requires it
to be infallible after prepare, but `TaskEnsureError` still exposes
`AdapterCommit`. That result-changing branch is impossible under the selected
transaction protocol and must be deleted. Fallible reservation belongs to
prepare; commit only makes the prepared token visible.

### Need handle equality disagrees with canonical value identity

The canonical `RuntimeValue::NeedHandle` transcript is exactly tag 20 plus
`NeedId`, but derived `RuntimeNeedHandle::PartialEq` compares correlation,
boxed spec, and origin. Because NeedId is generation-independent, semantically
identical handles can compare unequal while producing the same declared value
identity.

The selected NeedId-only value identity requires NeedId-only semantic equality.
Await/timeout use must separately validate handle generation against the active
generation. Generation changes are admitted only through the explicit atomic
replacement rebind.

### Persistence schemas are references, not final schemas

`GenerationTaskJournalSnapshotV1` refers to `TaskGroupSnapshotV1`,
`TaskSnapshotV1`, `NeedObserverSnapshotV1`, `TaskSpecSnapshotV1`,
`NeedSnapshotV1`, and `RuntimeNeedOutcomeSnapshotV1` without defining their
field sets or strict decode invariants. The returned exact correlated
save/restore contract is therefore incomplete.

### Match and View semantic substrates are unconstructible

The returned Match digest delegates to unspecified checked-expression and
checked-pattern digests, stable checked value coordinates, and accepted
declaration semantic identity. It does not define exhaustive transcripts for
current checked enum variants or their local/callable/nominal references.
`CheckedMatchRef` also names nonexistent `AcceptedSemanticGeneration`; current
final semantic authority is bound to `HirSnapshotId` and the module snapshot
map in `FinalSemanticAnalysis`.

The final contract must bind compiler-local lookup to the current
`HirSnapshotId`, define every semantic transcript exhaustively, and prove HIR
allocation/source-span invariance.

### Persistent View rows contain session-local HIR identity

`AcceptedViewMatchBundleRowV1` embeds `CheckedViewMatchCatalogRow`, which embeds
`CheckedMatchRef { expression: ExprId, ... }`. This directly contradicts the
returned rule that HIR/session coordinates never enter bundle identity or
persistence.

Compiler-local lookup rows and persistent bundle projections must be distinct.
The persistent row carries only program/site, accepted revision where the
bundle owner needs it, semantic/admission/ownership/producer/type/plan/value
digests, and any fully defined typed evidence needed for verification.

### Ownership rows claim missing carriers

The matrix admits `Shared<T>` through a future "ownership/carrier cut" but
defines no runtime projection, live `RuntimeValue` carrier, canonical identity,
or snapshot codec. It also says `TypeKind::Predicate` recursively visits a
child even though the current variant has no child. A total classifier cannot
be generated from this matrix.

Every row without a current or fully defined same-cut carrier must fail
`MissingRuntimeSnapshotOwner`. Shared may become `SnapshotClone` only when the
same returned contract defines its exact projection, carrier, canonical and
snapshot schemas. Predicate is a leaf row.

### The five-cut sequence is not compile-clean

Cut 3 publishes View bundle/runtime rows that depend on task digest types not
introduced until Cut 4. Cut 4 calls a new `RuntimeValue` enum variant private,
which Rust cannot express: adding an enum variant changes every exhaustive
consumer. The variant, bundle join, View runtime subscription, persistence,
replacement, and all exhaustive consumers must switch in the atomic public
cut.

Cut 3 is limited to sema/compiler-local View admission. Cut 4 may add private
sink infrastructure and standalone identity types, but no public enum variant
or consumer-visible schema. The corrected sequence must prove each published
cut compiles against only already-published final owners.

## Blocking correction

The returned combined contract is blocked by:

- [`Lang-01.5.1.1.2.1.1.1.1.1.1`](../reviews/requests/2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1-runtime-task-persistence-and-match-substrate-correction.md).

That request retains the closed numeric and identity choices and requires one
constructible task execution/transaction owner, complete persistence shapes,
current semantic lookup owners, a HIR-free bundle projection, carrier-backed
ownership rows, and a genuinely compile-clean publication sequence.

No Rust, Cargo manifest, production fixture, generated artifact, Clippy, AOT,
platform, browser, or runtime test was changed or run for this intake cut.
