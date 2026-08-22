# Runtime convergence Cut 1 — Match identity and child edges

Date: 2026-08-22
Inspected Git commit: `ee4a8e5c32cea50a438b15d6b2c947041ecf9d81`
Working tree during implementation: dirty only with the Cut 1 source, test,
generated structural-audit, and this evidence-note changes listed below;
`main` matched `origin/main` before the cut.

## Result

- Cut: `1 — generic Match identity and child edges`
- Accepted design:
  `docs/reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1-runtime-launch-receipt-keyed-ordinal-and-current-owner/`
- Implementation result: `PASS`
- Independent Sol-max design audit: `READY`; no remaining P0 or P1 finding
- Production commit/push at the time of this note: not yet performed

## Performed

- Added the one HIR-owned typed expression-child inventory for all 38
  `HirExprKind` families, including typed nested Choice and line-plan paths.
- Replaced the duplicate direct-child walk with a projection of
  `HirExprKind::child_edges()` and routed recovery-slot lookup through the same
  edge authority while preserving optional source-coordinate gaps.
- Added checker-owned ordinary-Match evidence for exact scrutinee, arm, guard
  presence, guard child, value child, and Bool-guard validation.
- Added checker-owned typed nested evidence for Choice and dialogue line-plan
  paths. Evidence construction retains typed first errors; semantic tags are
  used only for transcripts, not family classification.
- Added one atomic per-expression sema result containing checked child edges
  and, for ordinary calls, the current callable join. Child or callable
  failure publishes no sibling success payload.
- Added the callable-owned selected-call validator over the current
  `CheckedCallableCatalog`, including group/result/argument/effect/generic and
  receiver-mode checks. Free explicit-extension calls retain receiver `None`;
  dotted calls alone require the exact typed method lookup and Extension
  receiver evidence.
- Resolved checked record fields through accepted nominal declaration order,
  not authored record-literal order. Renamed the core numeric constructor to
  `RuntimeRecordFieldId::try_from_zero_based_ordinal` and documented that it
  does not prove layout membership.
- Deleted the pre-migration child-order switch, the provisional sema raw-edge
  cache, the duplicate callable join model/resolver, and the local duplicate
  callable group/result projection.
- Repaired the stale missing-Choice-body fixture so it supplies current Choice
  grammar evidence instead of parsing bare `choice` as an ordinary path.

## Passed

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo check -p arcweft-core -p arcweft-lang-hir -p arcweft-lang-sema --all-targets`
- `cargo check --workspace --all-targets --all-features`
- `cargo test -p arcweft-core --lib`: 219 passed, 0 failed
- `cargo test -p arcweft-lang-hir --lib`: 840 passed, 0 failed, 8 ignored
- `cargo test -p arcweft-lang-sema --lib`: 225 passed, 0 failed
- `cargo test -p arcweft-lang-sema callable::join`: 6 passed, 0 failed
- Focused HIR tests for the independent 38-family matrix, optional recovery
  gaps, deep Choice LIFO paths, and sibling Start/Together line-plan paths.
- Focused sema tests for ordinary Match evidence, missing/stale nested
  evidence, dialogue line-plan paths, declaration-order record identities,
  catalog/intrinsic joins, direct/dotted extension modes, and callable
  receiver/group/result negatives.
- `cargo clippy -p arcweft-core -p arcweft-lang-hir -p arcweft-lang-sema --all-targets`:
  exit 0; remaining warnings are pre-existing outside the new Cut 1 owners.
- `cargo clippy --workspace --all-targets --all-features`: exit 0; pre-existing
  workspace warnings remain non-blocking.
- `just test-doc`: passed after rebuilding the main-worktree core/sema
  artifacts.
- `just structure-audit` and `just structure-audit-gate`: passed with zero
  typed blocking violations.

## Failed but baseline-reproduced

- `just test-workspace` reached `arcweft-lsp` and failed 13 of its 212 library
  tests. The failures report either unresolved runtime reachability edges,
  missing accepted-local runtime seed handles, or downstream absent LSP output.
- Clean detached baseline `ee4a8e5c32cea50a438b15d6b2c947041ecf9d81`
  reproduces both root failure families exactly:
  - `features::entry_roles::tests::lsp_navigation_uses_typed_syntax_and_module_hir_ids`
    fails with `compiler.runtime_reachability.invalid_edge`;
  - `features::entry_roles::tests::entry_reference_ranges_follow_utf8_utf16_and_utf32_encodings`
    fails with a missing accepted-local runtime seed handle.
- The temporary clean baseline worktree was verified clean and removed. A
  shared-target baseline diagnostic temporarily replaced core/sema build
  artifacts; `cargo clean -p arcweft-core -p arcweft-lang-sema` removed only
  those generated artifacts (25.6 GiB), and `just test-doc` then rebuilt and
  passed from the main working tree.

These failures are not credited to Cut 1 and are not hidden as passed. They
belong to the already-open statement/reachability convergence work that must
close before the later runtime cuts can claim a wholly green workspace.

## Structural review

Retained generated evidence:
`docs/implementation/structure-audits/2026-08-22-cut1-match-child-edges/`.

- `arcweft-lang-hir::expr::child_edges` owns the exhaustive 38-family HIR walk,
  typed roles, typed nested coordinates, and recovery projection. It is 1,125
  physical LOC, below the production review trigger, and HIR retains no core
  or sema dependency.
- `arcweft-lang-sema::callable::join` owns selected-call validation and its
  transcript. Embedded tests were moved to `callable/join/tests.rs`; the
  production owner is 1,095 physical LOC, below the trigger.
- `arcweft-lang-sema::final_analysis::match_edges::model` owns checked role,
  path, evidence, atomic fact, error, and transcript types (759 physical LOC).
  `final_analysis::match_edges` owns enrichment/publication and HIR lookup
  adaptation (878 physical LOC). The split removes the crossed production
  review trigger without introducing a second model.
- `final_analysis/tests.rs` remains above the test-file review trigger. It is
  the existing shared final-analysis fixture and cross-family publication
  matrix; the Cut 1 tests deliberately reuse that fixture rather than create a
  parallel analyzer harness. This is a test-layout trigger, not a production
  ownership or dependency violation.

## Not run

- Tier 2 Agent/MCP/native capture/visual tests: not applicable because Cut 1
  changes HIR/sema identity and callable admission only; it does not change an
  Agent protocol, subprocess boundary, capture path, native attachment, or
  rendered output.
- Full CLI integration matrix: not applicable to this cut.

## Remaining work and non-goals

- Cuts 2–5 remain pending: ownership projections; compiler-local Match/View
  admission; task identity/catalog substrate; and the atomic Need/scheduler/
  adapter/snapshot switch.
- Cut 1 does not add public task, Need, View-runtime, scheduler, adapter, or
  snapshot types.
- The baseline runtime reachability/local-seed failures are not repaired in
  this cut; they remain evidence for the dedicated statement-origin typed-edge
  implementation rather than a reason to restore all trait methods to the
  runtime plan.

## Design deviations

None. The implementation keeps one HIR child-order authority, one current
callable catalog, typed first-error publication, schema/version marker `1`, and
no compatibility reader, fallback resolver, source-spelling identity, or
parallel committed side table.
