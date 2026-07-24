# AW-AH-009.3 focused boundary follow-up

Date: 2026-07-24

## Status

`IMPLEMENTED_VALIDATED_WITH_INHERITED_PROOF_GATE`. The production
implementation, focused matrix, strict workspace Clippy, Tier 2, and
structural error gate are complete. The normal workspace route passes every
subrecipe except the same two `extern capability` associated-type fixtures
already recorded on the parent revision as waiting for the Proof-concurrency
public AST/HIR authority switch. No partial capability reader or global type
fallback was added to turn that known dependency green.

This follow-up closes the recursion, work-accounting, snapshot/document API,
accepted-HIR lease, corrupt-catalog, request-publication, capacity-authority,
and production-limit gaps that remained after the earlier ordinary-call and
bounded-cache cuts.

The accepted package precedence remains:

- AW-AH-009.3.1 limits accepted parser call nesting to 32, while the base
  signature query keeps its defensive 64-level bound;
- AW-AH-009.3.3 limits an accepted callable key to 32 overloads, while the base
  public signature projector keeps its defensive 64-overload bound;
- AW-AH-009.4.2 supersedes the base S14-S16 CharacterDialogue application
  surfaces; this work freezes the legacy production carriers and does not
  enhance them before the typed authority switch;
- Lang-01.1.1.2.2 adds `RegisteredEnvironmentDigest` to persistent semantic and
  LSP signature-cache identity. Production keeps that later field without a
  legacy key or dual lookup.

The signature query remains independent from Proof-concurrency node identity.
Its source boundary is `SourceDocumentIdentity`, exact parser-owned call
ranges, document-bound HIR, and the accepted world/revisions. It does not
accept or derive `SourceSnapshotId`.

## Production changes in this follow-up

- `CallableQueryDepth` owns checked, inclusive registered-candidate recursion.
  Entering level 33 returns the typed nested-call limit without mutating depth.
- A singleton resolved candidate is evaluated once in one retained
  transaction. `CandidateProbe` is the pre-evaluation control boundary and
  `SelectedReplay` is the pre-commit boundary. Viable state commits only after
  terminal checks; rejected semantic mutations roll back while required
  diagnostics and focused nested facts remain available.
- Catalog-build checked-add overflow is the top-level
  `CallableCatalogBuildError::WorkOverflow`; ordinary exhaustion remains the
  typed build-work limit. Both leave the meter unchanged.
- The resolver now revalidates an environment candidate set while reading it:
  non-empty shape, exact lookup key, unique IDs, authority/provider agreement,
  by-ID reachability, valid equivalent provenance, and canonical ordering.
  Corruption returns the corresponding `ResolveCallError::CorruptCatalog`
  reason before a guessed target or argument check.
- Test-only typed query control can cancel between candidate probes or at the
  pre-commit boundary. Both paths return `Cancelled` and publish no partial
  semantic signature help.
- Compile-fail evidence proves that `SignatureQuery::production` cannot consume
  `SourceSnapshotId` and that no conversion to `SourceDocumentIdentity` exists.
- Static associated capacity calls now use the existing parenthesized
  `Expr::Call` surface, a source-backed typed receiver, nominal resolution,
  `CallCallee::AssociatedType`, and the single shared resolver. Direct and
  turbofish call type applications retain structured authored type arguments
  and ranges without embedding generic syntax in a member name.
- Dot-member classification is value-first across lexical, project, imported,
  and environment values. Only typed absence permits nominal-type fallback;
  ambiguous, inaccessible, or poisoned value results are terminal. Structured
  `DottedPath` traversal replaces the former string-only target helper.
- The old static-capacity early success branch,
  `well_known_static_capacity_method_type`, generic source-text slicing, bare
  `Vec` `_` placeholder, and static label readers are deleted. No renamed
  string helper or parallel resolver replaces them.
- Accepted project publication, signature-cache access, cancellation, panic,
  terminal response, and replacement cleanup are linearized through the
  request/publication owners. The compiler, registration, and LSP paths retain
  the same accepted `Arc<HirProject>` lease instead of manufacturing a second
  accepted project value.

## Direct evidence added

- A real 32-deep registered `panic(...)` query succeeds under the fixed 4,096
  callable-work limit and records 32 argument mappings, 32 type checks, and 32
  specificity projections. This detects recursive probe/replay duplication.
- Exact production boundary tests cover the accepted 32-deep parser/query
  path, 32 overloads, 128 parameters, source size, outer signature work, and
  checked arithmetic seams.
- Corrupt fixtures cover every typed reason: `EmptySet`, `KeyMismatch`,
  `DuplicateId`, `WrongAuthority`, `MissingRecord`, `InvalidEquivalent`, and
  `Unsorted`. The fixture constructors remain `cfg(test)` and crate-owned.
- Cancellation tests directly cover the second candidate probe and the
  selected pre-commit boundary.
- I21/I22 `trybuild` cases prove the snapshot/document type boundary without a
  source-text gate.
- Associated-capacity tests cover `String`, `Bytes`, applied `Vec<I32>`,
  generic `Vec<T>`, aliases, qualified receivers, value/type collisions,
  malformed/recovery input, exact/one-over limits, registered/detached parity,
  public fact/signature parity, and exactly-once argument accounting. Bare
  `Vec.with_capacity(8)` remains a typed generic-arity failure with no
  candidate, following AW-AH-009.3.3.4 T08/C17 over the package-local CAP-005
  drift.

## Closure disposition

The previously open implementation rows are closed as follows:

- AW-AH-009.3.2 identity, accepted-HIR, loader-bound, cache-stamp,
  four-worker, panic/cancel, stale-publication, retained-reader, and repeated
  replacement rows have direct compiler, loader, and LSP tests;
- AW-AH-009.3.3 build atomicity, reversed-order determinism, typed result
  stability, failed-rebuild isolation, dispatcher counters, and production
  exact/one-over limits have owning-crate tests;
- AW-AH-009.3.3.1 SR-01 through SR-08 and DR-01 through DR-05 are represented
  through the single resolver and retained physical-versus-semantic accounting;
- AW-AH-009.3.3.4 is the sole static capacity authority, and every old
  stringly success path named by the contract has been deleted;
- the final workspace, Tier 2, and structural gate results are recorded below.

The current typed migration inventory classifies all 23 `CallableFamily`
entries exactly once. Before the accepted capacity switch there are 21
current-phase executable shared-resolver observations and 42 observation
cases, but only 20 final-model-compliant rows and 40 completion cases. Eighteen
families have reachable accepted and rejected/poisoned cases. Drop and
Promotion retain the package's own unchecked production semantics. Speaker is
also currently executable and unchecked, but it is `PendingRemoval` for final
completion and earns no final matrix credit.

CapacityMethod now uses its accepted `variadic_unchecked` final contract
through the typed associated-callee authority. Current-phase observations are
therefore 22/44 while final-model evidence is 21/42. Dialogue remains pending the
AW-AH-009.4.2/.3 typed authority switch; the frozen legacy carrier is not final
matrix evidence. That switch deletes Speaker and activates final Dialogue, so
the resulting 22-family inventory has 19 rejecting and three intentionally
unchecked families, with 22/44 final-model rows/cases. No phase credits both
Speaker and final Dialogue. The implementation fabricates neither a capacity
spread rejection nor a Dialogue fixture. The returned AW-AH-009.3.3.3.1
package supplies the staged classification and physical-versus-retained
accounting contract; it is accepted with the explicit CAP-005 precedence
adjudication recorded in its
[intake](2026-07-24-aw-ah-009-3-3-3-1-dispatch-intake.md), and no further
request is required.

Package section 28 explicitly permits small-limit owning-crate tests for
catalog build loops. Production constants and their actual owners must remain
directly coupled in those tests; an invalid accepted world is not fabricated
to reach defensive bounds.

This follow-up adds no CSS/Takumi path, removed-syntax recognizer, source gate,
display-label parser, compatibility alias, dual reader, or migration shim.

## Final validation

Focused and owner-level evidence passed after the deletion-driven authority
switch and after the final responsibility split:

```text
cargo test -p arcweft-lang-syntax --lib
  485 passed

cargo test -p arcweft-lang-sema --lib
  1116 passed

cargo test -p arcweft-lang-sema --test call_surface_signature_matrix
  7 passed

cargo test -p arcweft-lang-sema --test character_signature_fact_parity
  4 passed

cargo test -p arcweft-cli --test seq04_8_4_persistent_cache_build_cli_goldens
  2 passed
```

The compiler, project-loader, and LSP all-target focused routes also pass.
The persistent-cache fixtures were stale under the already-authoritative flow
return contract: three flows returned `String` while omitting their return
type. The fixtures now write `() -> String`; production return checking and
the cache goldens were not weakened or regenerated.

The settled broad gates are:

```text
cargo fmt --all -- --check
  passed

git diff --check
  passed

cargo check --workspace --all-targets
  passed

CARGO_BUILD_JOBS=1 cargo clippy --workspace --all-targets --all-features -- -D warnings
  passed

CARGO_BUILD_JOBS=1 just test-tier2
  46 passed
  - MCP stdio: 22
  - slow Agent observe: 1
  - native auxiliary capture: 16
  - visual smoke: 2
  - checked-in golden integrity: 1
  - exact PNG/imq goldens: 4

cargo +nightly -Zscript tools/structure-audit.rs --root .
  files scanned: 3653
  Rust files: 1937
  Rust physical LOC: 908928
  package manifests: 94
  violations: 0 errors, 146 warnings
```

The first parallel workspace-test attempt encountered a rustc 1.96
incremental compiler ICE while compiling sema; the directly affected focused
test passed afterward. A second parallel attempt stopped on Windows OS error
1455 while mapping an `arcweft-bundle` artifact. The fixed-feature single-job
route avoided both host failures and passed the non-CLI workspace suite, CLI
library/binaries, and the selected CLI integrations until the two known
Proof-switch fixtures:

- `spec_should_pass/check/010_capability_fs_read.arcw`; and
- `spec_should_pass/run/002_file_read_task.arcw`.

Both report `sema.nominal.unknown_type` for capability-owned `FsError`. The
parent revision already records that the public detached
`ExternCapabilityItem` publishes functions but not its private typed `type`
member. Proof-concurrency must publish that member together with the bound
syntax/HIR/project authority and delete the detached reader in the same public
switch. This AW-AH-009.3 cut deliberately does not add a global `FsError`, a
named-type fallback, a fixture bypass, or a partial capability reader. The
last workspace subrecipe, which the recipe does not reach after those two
failures, was run directly and passed 2/2 after the explicit fixture return
types above.

## Structural decomposition

The initial settled-checkout audit found two error-level production hotspots.
They were split by domain rather than suppressed:

| Path | Bytes | Physical LOC | Responsibility |
| --- | ---: | ---: | --- |
| `crates/arcweft-lang-sema/src/checker/expr.rs` | 89,080 | 2,325 | expression checking facade and non-member expression families |
| `crates/arcweft-lang-sema/src/checker/expr/member.rs` | 22,081 | 544 | member selection, value/type receiver classification, associated nominal calls |
| `crates/arcweft-lang-sema/src/checker/module.rs` | 91,498 | 2,298 | module type-check orchestration and checker completion |
| `crates/arcweft-lang-sema/src/checker/module/focused.rs` | 10,246 | 312 | focused-call acceptance, caller control, retained fact/accounting query |

The split changes no public API, visibility contract, candidate authority, or
behavior. It adds no lint suppression, wrapper dispatcher, source gate, or
compatibility surface. The 146 remaining audit findings are repository-wide
ownership-review warnings; the error gate is zero.
