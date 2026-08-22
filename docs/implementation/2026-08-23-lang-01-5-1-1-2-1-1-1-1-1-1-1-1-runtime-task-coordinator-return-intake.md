# Lang-01.5.1.1.2.1.1.1.1.1.1.1.1 runtime task coordinator return intake

Date: 2026-08-23
Inspected Git commit: `9168c8ac7285c6b44f29018626a0e7c1b0059796`
Working tree before intake: clean; `main` matched `origin/main`

## Intake result

- Archive safety and byte integrity: `PASS`
- Internal checksum list: `PASS`
- Required returned-archive contract: `FAIL`
- Repository reconciliation: `FAIL`
- Classification: `DESIGN_NOT_READY`
- Production implementation: blocked

The return did not inspect Arcweft source and replaces the request's fixed
host-owned `RuntimeTaskScheduler<A>` boundary with an invented coordinator and
durable I/O journal. Those choices are result-changing and conflict with both
the request and the accepted Sans-I/O transaction direction.

## Retained archive

External source archive:

- path:
  `D:/sanze/Downloads/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1-runtime-task-coordinator-and-two-phase-restore-correction-final-contract.zip`
- byte length: 44,834
- SHA-256:
  `097958A70DC0BF82BD75D79D1593D88451725B572FE6D2FE165D527DA91EB036`

The unchanged ZIP is retained at
[`docs/reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1-runtime-task-coordinator-and-two-phase-restore-correction-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1-runtime-task-coordinator-and-two-phase-restore-correction-final-contract.zip).
Its 17-file frozen mirror is retained under
[`docs/reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1-runtime-task-coordinator-and-two-phase-restore-correction-final-contract/`](../reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1-runtime-task-coordinator-and-two-phase-restore-correction-final-contract/README.md).

## Integrity checks passed

- one exact top-level wrapper;
- 17 file members plus one directory entry, 92,459 uncompressed bytes;
- no unsafe path, duplicate, case-fold collision, or special Unix member;
- all 15 `SHA256SUMS.txt` rows match the frozen mirror, although the checksum
  list does not cover all 17 package files; and
- embedded request SHA-256
  `62654CFCFADB1359523C3DBA2BA97663F813CBA3D3A8930FEDD22ED660F0BA68`
  exactly matches the maintained request.

## Required package failures

The archive omits the required `FINAL_STATUS`, `OPEN_QUESTIONS`,
repository-aware validator, and negative self-tests. `OPEN_QUESTIONS=0` appears
only as prose. The package records the inspected Git SHA, checkout, worktree,
`AGENTS.md`, source paths, and compilation as unavailable.

Its claimed source destinations under `crates/arcweft-runtime/src/task/` do not
exist. Current relevant owners are split across `arcweft-core`,
`arcweft-runtime-scheduler`, `arcweft-runtime-host`, and
`arcweft-runtime-driver`.

## Result-changing design failures

### The required host scheduler and driver boundary are absent

The request fixes a public host-owned
`RuntimeTaskScheduler<A: TaskLaunchAdapter>` and a narrow borrowed `TaskHost`
implemented by native, Web, and headless hosts. The return instead defines a
`pub(crate) RuntimeTaskCoordinator` with async persistence methods. It defines
neither the required public generic scheduler nor the final `TaskHost`
signatures/compositions.

Its traceability marks the TaskHost requirement closed by `D-36`, but no such
decision exists. Referenced acceptance rows `RTC-OWN-001`, `RTC-PUB-001`, and
`RTR-REQ-xxx` likewise do not define executable tests.

`PreparedTaskRestore` is not parameterized by the adapter and contains no
concrete prepared launch/restore/rebind/cancel tokens. The design therefore
cannot express the accepted adapter prepare/rollback/apply protocol without
erasing or duplicating its associated token authority.

### It creates a second durable journal authority

The accepted future core `RuntimeGenerationJournal` is a Sans-I/O committed
state owner applied through one sealed `JournalTransaction` after-image. The
return instead adds async `TaskPersistence`, PREPARED/COMMITTED disk records,
fsync, startup replay, and a separate coordinator epoch. This is a new I/O
transaction system, not the requested constructible host/scheduler boundary.

The package selects durable COMMITTED as the semantic commit point before
runtime publication. It then allows runnable-queue failure to become fatal or
recovery work after commit. That is a fallible post-commit state and directly
violates the request's requirement that no fallible operation remain after the
sole commit point.

The accepted transaction direction instead preconstructs both complete
after-images, prepares adapter tokens, applies the sealed core after-image,
performs the scheduler's infallible swap, commits adapter tokens infallibly,
and only then exposes receipts/handles.

### Compatibility and version policy are reversed

The return authorizes supported older snapshot versions, normalize-to-current
readers, append-only versioned migration records, and rollback readers. Current
policy requires every Arcweft-owned marker to remain `1`, replacement of the
unreleased shape in place, and no old reader or compatibility path without
explicit released/persisted/external-consumer evidence.

### Required live-state projection remains incomplete

The package introduces new `RuntimeTaskIdentity`, generation, restore ID,
coordinator epoch, snapshot ID/digest, capability, handle, and match-substrate
types without reconciling them to the accepted
`GenerationId`/`NeedId`/`TaskKey`/`TaskId`/correlation/journal/observer owners.
It does not provide the exact exhaustive event/current-outcome/snapshot mapping
requested for `Progress`, `Ready`, `InfrastructureFailure`, and `Cancelled`.

## Next action

This return must not be implemented. Sol max selected and closed the focused
correction in:

- [`Lang-01.5.1.1.2.1.1.1.1.1.1.1.1.2 request`](../reviews/requests/2026-08-23-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.2-current-host-scheduler-and-sans-io-restore-transaction-correction.md); and
- its [accepted design](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1.2-current-host-scheduler-and-sans-io-restore-transaction/README.md).

That design retains the current host-owned generic scheduler, borrowed core
`TaskHost`, core journal transaction, concrete adapter tokens, and final
infallible apply/commit/exposure ordering. It contains no durable restore WAL
or compatibility reader.

## Accepted focused correction package

Sol max produced the accepted design from current repository evidence. Its
local final-contract package is frozen at:

- [package mirror](../reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.2-current-host-scheduler-and-sans-io-restore-transaction-correction-final-contract/README.md);
- [ZIP](../reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.2-current-host-scheduler-and-sans-io-restore-transaction-correction-final-contract.zip);
- ZIP byte length: 56,160; and
- ZIP SHA-256:
  `9CDBCF355CF15D99AF75E7EFD22B22AC5CBFAD163B86206AB8463A39E92E21F9`.

The package has one exact wrapper and 17 files. The repository-aware Rust
validator passed against commit
`9168c8ac7285c6b44f29018626a0e7c1b0059796`; the in-memory negative mutation
corpus passed 13/13; Rust formatting passed; the request mirror, manifest,
source Git blobs, machine contract, single event-drain rule, and last-fallible
apply ordering all validated.

No production Rust, Cargo, generated production artifact, fixture, or runtime
test was changed or run for this design-only intake. The only Rust files are
the read-only design validator and its in-memory negative self-tests.
