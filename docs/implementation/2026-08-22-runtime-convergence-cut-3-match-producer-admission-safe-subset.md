# Runtime convergence Cut 3 — Match/producer admission safe subset

Date: 2026-08-22
Inspected Git commit: `515bb071437c3af053f1560c3119906dc8002efc`
Working tree during implementation: clean dedicated worktree
`D:\git\arcweft-cut3` on local branch
`codex/cut3-match-view-admission` before this Cut 3 change.

## Result

- Cut: `3 — compiler-local Match/producer admission safe subset`
- Implementation result: `PARTIAL — BLOCKED BY .1.2 VIEW PATH PREDECESSOR`
- Commit/push: intentionally not performed

## Performed

- Added private-field, non-Serde `CheckedMatchRef { HirSnapshotId, ExprId }` as
  compiler-local lookup evidence only.
- Added `FinalSemanticAnalysis::checked_match_ref` and replaced the raw-ExprId
  Match construction path with `build_checked_match_for_ref`. Both validate the
  exact report/module snapshot and checked Match fact; declaration inference
  from a ref is absent.
- Completed the constructible producer-admission subset for direct selected
  calls. The constructor derives source-ordered arguments only from current
  HIR, `CallTargetFacts`, checked expression facts, and the current callable
  join; callers cannot submit rows or evidence digests.
- Producer argument ownership is classified transactionally in one traversal,
  and `CheckedNeedProducerAdmissionDigest` uses the accepted stable-coordinate,
  semantic-type, disposition, and merged consulted-evidence grammar.
- Explicit receiver/function-value captures, spread/compact slot inventories,
  live Need values, Function arguments, recovery, and incomplete call facts
  fail closed with typed errors.
- The authored argument-count work limit is enforced immediately after the HIR
  Call shape check, before selected-fact and callable-join lookup, so oversized
  physical input has deterministic `WorkLimit` precedence.
- Added the independently throwable retained View completeness request:
  `docs/reviews/requests/2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1.4-retained-view-operation-value-slot-completeness-correction.md`.

## Blocked boundary

Current `arcweft-compiler::view` lowers only checked `ViewCall` values into
element/text/rich-text instructions. There is no checked Match subscription
instruction, retained output/value-slot inventory, capture completeness proof,
accepted View-declaration/body semantic path, or executable consumer for a
`ViewMatchSiteId`, `CheckedViewMatchAdmission`, or compiler-local catalog row.
The initially implemented site constructor was removed after audit showed it
could mint a site from an ordinary function Match plus an arbitrary program;
the current semantic path authority reports `MissingBody` for a View item.
Adding a site, catalog, persistent/cache projection, bundle row, or
`CompiledViewProduct` side table now would publish unconsumed or incorrectly
rooted semantic state.

Accordingly this cut does not construct `CheckedViewMatchAdmission`, add a
`ViewMatchSiteId`, compiler catalog row, connection from Match to
`CompiledViewProduct` or `ValidatedViewProduct`, or persistent/bundle
projection. Generic Match completion and View path closure are first blocked
by request `.1.2`; retained View operation/value-slot completeness then remains
blocked by request `.1.4`, before the atomic Cut 5 product/runtime switch.

## Validation

### Passed

- `cargo check -p arcweft-lang-sema --lib`
- `cargo check -p arcweft-lang-sema --all-targets --all-features`
- focused generic Match tests after ref migration: 10 passed, 0 failed
- focused producer admission tests: 5 passed, 0 failed
- focused compiler-local certificate trybuild test: 2 fixtures passed
- `cargo test -p arcweft-lang-sema --all-features`: 261 unit tests,
  11 API compile-test groups, 4 integration tests, and 0 doctests passed; 0
  failed
- `cargo +nightly -Zscript tools/structure-audit.rs --root .
  --fail-on-blocking`: 2,183 files, 2,055 Rust files, 199 review triggers,
  0 blocking violations

### Failed but baseline-known

- `cargo clippy -p arcweft-lang-sema --lib --no-deps -- -D warnings`
  remains failed on the same 24 pre-existing or separately owned sema
  diagnostics. The final path audit reports no diagnostic in `ownership.rs` or
  the newly added Ref/producer code.

### Not run

- Workspace-wide check/tests/Clippy were not run because this partial cut
  changes only the sema proof boundary and publishes no executable View,
  runtime, bundle, persistence, or cross-crate product contract.
- View/runtime Tier 2 tests are not applicable until the blocked executable
  View operation and Cut 5 product/runtime join are implemented.

## Structural review

- `final_analysis::semantic_transcript` remains the cohesive owner for accepted
  declaration paths, the implemented generic Match safe subset, stable value
  coordinates, and compiler-local Match references.
- crate-root `producer_admission` is the sole composer for producer admission
  rows, the public result/error/digest types, exact current-call derivation, and
  the admission digest grammar. It consumes final-analysis coordinates and a
  narrow ownership batch-classification boundary in one direction.
- `ownership` remains only the cohesive type-directed
  classifier/evidence/disposition owner. It publishes one narrow crate-private
  transactional batch function; it does not own current-call traversal,
  producer admission rows, or producer digest construction.
- The pre-existing `semantic_transcript` and `ownership` owners already cross
  the repository structure review size trigger; the focused root composer does
  not. The change does not mix I/O/runtime/persistence into them or widen a
  dependency edge; splitting the shared transcript/classifier algorithms would
  create duplicate authority, so this cut retains them with this cohesion
  justification.

## Explicit non-goals

- no `ViewMatchSiteId`, `CheckedViewMatchAdmission` success, or compiler-local
  View Match catalog;
- no `CompiledViewProduct`, `ValidatedViewProduct`, persistent cache, bundle,
  AWBC, runtime View, observer, or replacement connection;
- no `NeedProducerContractDigest`, `TaskPlanSemanticDigest`, `RuntimeValueDigest`,
  `TaskSpec`, `TaskExecution`, Cut 4 identity, or final task/Need carrier use;
- no caller slices, whole-catalog digest, evidence-digest concatenation,
  source reconstruction, Serde codec, compatibility route, or version bump;
  and
- no claim that generic Match is complete: request `.1.2` owns remaining
  transcript/coverage and View declaration/body path closure; no redesign of
  accepted ownership evidence, current View identity/revision, or task
  requests.
