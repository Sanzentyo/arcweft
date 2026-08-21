# Lang-01.5.1.1.2.1.1.1.1.1.1.1 runtime handle/batch/snapshot return intake

Date: 2026-08-22
Inspected Git commit: `f42157fc4c8ca890eeaacec6dde3bb3e3af5d009`
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

The return materially improves the handle state, AwaitMany construction,
whole-batch staging, observer allocation, cancellation protocol, Match
inventories, ownership inventory, and event ordering. It still cannot be
implemented as one typed contract. Several published APIs cannot communicate
across their assigned crate boundary, two tables contradict current accepted
runtime carriers, and one generation-global counter changes Need identity under
unrelated scheduling interleavings.

## Retained archive

External source archive:

- path:
  `D:/sanze/Downloads/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1-runtime-handle-batch-and-snapshot-isomorphism-correction-final-contract.zip`
- byte length: 112,282
- SHA-256:
  `CB515E4C9F4873EDD8CB438ECFF887DD9182E557CEF3CE1CD006C05A8919EEB0`

The unchanged byte authority is retained at
[`docs/reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1-runtime-handle-batch-and-snapshot-isomorphism-correction-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1-runtime-handle-batch-and-snapshot-isomorphism-correction-final-contract.zip).
Its 40-file byte-identical frozen mirror is retained under
[`docs/reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1-runtime-handle-batch-and-snapshot-isomorphism-correction-final-contract/`](../reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1-runtime-handle-batch-and-snapshot-isomorphism-correction-final-contract/README.md).

## Performed and passed

- Verified one exact top-level wrapper, 40 members, 363,303 uncompressed
  bytes, no absolute/drive/parent-traversal path, duplicate entry, case-fold
  collision, symbolic link, or unsafe member.
- Verified the retained ZIP is byte-identical to the external attachment.
- Verified every extracted file is byte-identical to its ZIP member.
- Independently verified all 38 `MANIFEST.json` payload rows by byte length and
  SHA-256.
- Verified `MANIFEST.json` SHA-256
  `D824E6254C4A24D55BECC3129DFD51E8435F94016387770440B9AF8BAE0D105F`
  equals `MANIFEST.sha256`.
- Verified the packaged request is byte-identical to the maintained request:
  16,574 bytes, SHA-256
  `F3ADC4B80F21822237D813B9A27EB327F2F6CD4A243EFDF733E722489342CF76`,
  Git blob `0d19f45059ed9dfd67d8e750e9308682f94e15ad`.
- Inspected the standard-library-only package validator before execution.
- Ran the validator against both the retained extracted package and the
  retained ZIP with `uv run --no-project`; both reported 38 manifest rows,
  100 normative test rows, 12 blocker rows, and `PASS`.
- Ran all 12 negative self-tests against both forms; all reported `PASS`.
- One intermediate command constructed the nonexistent path
  `docs/reviews/packages/<basename>.zip` and reported `E_OPEN`. Re-running
  against the actual retained `docs/reviews/packages/zips/<basename>.zip`
  passed; this was an invocation-path failure, not a package validation
  failure.
- Whole-cut `git diff --cached --check` reports seven trailing-space lines in
  the byte-identical frozen mirror. They are retained unchanged as archive
  evidence. The same check restricted to the two repository-authored Markdown
  files passes.
- Enumerated all 63 retained `docs/reviews/**/*.zip` archives at the
  reviewable cut. The sorted relative-path, byte-length, SHA-256 LF transcript
  hashes to
  `FC7281E418E06BA1453E95F3FFD298CD5618470600B976E9DE036C9C8AD35192`.
- Compared the returned Rust-shaped schema, machine inventories, and prose to
  current core/HIR/sema owners and maintained runtime contracts.
- Obtained an independent Sol-max design audit of the returned contract
  against current source and frozen predecessor decisions.

## Corrections accepted from this return

The next return must retain these improvements:

- `RuntimeNeedHandleState` is closed as `ReusableJoin { spec }` or
  `AcceptedLaunch`; direct Await does not relaunch an accepted launch.
- AwaitMany retains captured values, source items, a typed child template, and
  the exact source-indexed child argument constructor. Its aggregate base
  argument remains the source-order tuple required by the request.
- AwaitMany uses one whole child-launch transaction, reverse rollback, and no
  per-child committed `ensure_task` path.
- observer IDs are generation-owned, monotonic, persisted, and not consumed on
  failure.
- launch/cancel prepare is fallible, commit is infallible, and the protocol is
  owned below the scheduler in the Sans-I/O layer.
- the current 38 `HirExprKind` variants have an explicit role/tag inventory;
  event normalization retains `(logical_epoch, task_id, sequence)`.
- Structured function snapshots reject; no compatibility reader or numeric
  AWBC allocation change is introduced.

## Failed repository reconciliation

### MakeNeedHandle reopens the frozen family policy

The returned truth table authorizes `MakeNeedHandle` for Host or Runtime and
for `AlwaysStart`, eagerly ensuring the latter. The frozen contract permits
this family only as `Host + JoinSameKey`: it constructs a lazy reusable handle.
An AlwaysStart producer obtains `AcceptedLaunch` only from the accepted launch
boundary after another authorized launch operation has committed. The package
therefore changes both family routing and observable launch behavior.

### AlwaysStart ordinal allocation has the wrong scope

The package stores one `next_always_start_ordinal: NonZeroU64` per generation
and one scalar after-image in a batch delta. Frozen identity scopes the counter
to `(GenerationId, NeedProducerInstanceKey)`. A generation-global counter makes
the ordinal, NeedId, and TaskId of one producer depend on unrelated producer
interleaving. Snapshot rows and batch deltas must retain sorted per-producer
counter entries, with absent entries beginning at one.

### Accepted-launch construction is unreachable across crates

The package assigns `RuntimeNeedHandle::try_from_accepted_launch` and its
`AcceptedTaskLaunch` proof to core with `pub(crate)` visibility, while saying a
private scheduler-module constructor creates the proof. The scheduler is a
different crate. Neither the proof nor constructor can cross that boundary.
A core-owned public validated boundary must query the committed journal/Need
rows; a raw-field or forgeable receipt is not sufficient.

### Adapter preparation does not expose the required receipt

`TaskLaunchAdapter::PreparedLaunch` is an unconstrained opaque associated type.
The scheduler must nevertheless persist per-row `HostLaunchCapability` and
later supply it to restore, rebind, and cancellation. `commit_launch` returns
nothing and no API exposes those capabilities. The prepare result must carry a
core-owned inspectable receipt plus the adapter-private token, and the
scheduler must validate the receipt against the exact input batch before state
publication.

### Dense sequence snapshot schema is not the current owner

Current `DenseSeq` has `Units(usize)` and `Bool(DenseSeqStorage<bool>)`. The
package redefines that owner as `Units(DenseSeqStorage<()>)` and `Bools(...)`
while also claiming it is the exact current type. This is neither an in-place
projection nor an isomorphic snapshot. The existing snapshot must reuse the
exact current `DenseSeq`; its purpose-built codec may encode the Units length as
a checked fixed-width integer without redefining the Rust owner.

### Option ordinals are reversed

The ownership matrix states `None = 0` and `Some = 1`. Current runtime-plan
authority uses `Some = 0` with a payload and `None = 1` without one. The return
would silently reinterpret persisted and matched Option values.

### Callable and task validation authorities are unconstructible

The package repeatedly references `TaskContractCatalogV1` but defines no such
type, rows, constructor, lookup, or owner. It also invents
`CheckedCallableCatalogV1`; current sema already owns
`CheckedCallableCatalog`, `CallTargetFacts`, selected `ResolvedCallable`,
`CheckedCallableId`, and `CheckedCallableDigest`. The final Match transcript
must join those existing values through current APIs, or add a minimal inherent
API to the existing owner. A parallel catalog is forbidden.

The projection registry also lists multiple types without Rust schemas,
including sequence/ownership/nominal projections, TaskSpec/template snapshots,
`RuntimeSnapshotAuthorityV1`, and `CheckedNestedPathV1`. A registry name does
not make the declared compile cuts type-complete.

### Snapshot authority is duplicated and incomplete

The package names but does not define `RuntimeSnapshotAuthorityV1`, then also
serializes an `AwbcExecutableAuthorityRefV1` into every function value. Current
`AwbcRuntimeFunctionSnapshot` deliberately omits its owning program: the value
is dormant evidence until the enclosing generation-pinned program admits it.
The final contract must use one nonserialized restore authority and validate
the function ID against it; it must not persist a second generation/program
authority in every closure.

### Match role ownership violates crate layering

The source-deletion table directs `arcweft-lang-hir` to own an inherent
`CheckedExpressionChildRole` projection, while the schema and owner table put
that type in sema and give it core/sema payloads such as runtime record-field
IDs and checked nested paths. HIR depends on neither core nor sema. The final
shape needs one HIR-only typed child-edge/order authority and one sema
projection that attaches accepted checked identities; it cannot put the
checked role on `HirExprKind`.

### AgentBuiltin classification is not exhaustive

The 85-row matrix blanket-rejects `TypeKind::AgentBuiltin(_)`, although the
current nested enum's `Diagnostics` and `ViewportPoint` cases have exact
`RuntimeAgentValue` and AWBC snapshot carriers, which the package itself lists.
Those cases require typed SnapshotClone rows while unsupported siblings retain
individual typed rejections. A parent-family row is not total when its current
subvariants have different dispositions.

### Package validation proves only internal agreement

The validator accepts its own `Bools`, reversed Option ordinals, global
ordinal counter, nonexistent catalogs, and cross-crate-private constructor.
The next validator must compare selected current-source inventories and
required signatures, not only agreement among generated tables.

## Blocking correction

The returned combined contract is blocked by:

- [`Lang-01.5.1.1.2.1.1.1.1.1.1.1.1`](../reviews/requests/2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1.1-runtime-launch-receipt-keyed-ordinal-and-current-owner-correction.md).

That request retains the useful handle/batch/snapshot work while closing the
cross-crate construction seam, adapter receipt, keyed ordinal journal, exact
live/snapshot carriers, Option convention, and existing callable authority.

No Rust, Cargo manifest, production fixture, generated artifact, Clippy, AOT,
platform, browser, or runtime test was changed or run for this intake cut.
