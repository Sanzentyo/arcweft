# Implementation order

Each phase must end compile-clean. Do not leave old/new owner paths in parallel.

## Phase 0 — baseline and fixture lock

1. Record `9138efeeabdfca56809e8ad9c16fc85380ae18c5` as the implementation baseline or a newer `origin/main` SHA if main advances.
2. Re-read root/crates/docs AGENTS files.
3. Record the exact hash/content of fixture 013 and do not edit it.
4. Run current targeted tests to preserve the pre-change failure evidence.

## Phase 1 — HIR owner replacement

1. Add root/edge/executable/path/identity/error domain types in `runtime_semantic_owners.rs`.
2. Move structural root behavior onto existing `HirItemKind`, `HirExprKind`, and `HirStmtKind` inherent impls where missing.
3. Implement deterministic structural indexing and BFS closure.
4. Bind selected postfix/call-disposition decisions into construction.
5. Replace `HirRuntimeSemanticOwnerInventory` with `HirRuntimeSemanticReachability` atomically.
6. Update exports and HIR tests.
7. Delete old symbols and verify no grep hits remain.

Gate: HIR crate fmt/clippy/tests green.

## Phase 2 — sema classification and typed schema errors

1. Add `CheckedOrdinaryFunctionEmission` and the exhaustive inherent method on `CheckedItemRole`.
2. Add/confirm exact checked callable -> executable owner accessor on the owning catalog/fact type.
3. Add typed `NominalSchemaPath` and `OpaqueLeaf`/`UnsupportedLeaf` errors.
4. Preserve whole-project semantic publication and existing opaque identities.
5. Add semantic retention and schema-path tests.

Gate: sema crate fmt/clippy/tests green; no compiler-local duplicate classification.

## Phase 3 — compiler reachability bridge

1. Add `lower/reachability.rs` (not `mod.rs`).
2. Implement `CheckAll` and `SelectedEntry` root construction.
3. Project checked direct-call, trait, Flow-transfer, and Entry edges.
4. Compute HIR closure.
5. Prove edge completeness for reached sources.
6. Run reached callable preflight and typed path diagnostics.
7. Add integration tests for roots, missing edges, stale generation, deterministic paths.

Gate: reached suspending function fails before any call to runtime nominal projection.

## Phase 4 — compiler pipeline cut

1. Reorder `project.rs` to build checked Entries/selection/reachability/preflight first.
2. Change `project_runtime_semantic_facts` to accept reachability.
3. Filter every semantic fact family and Flow publication.
4. Add lightweight Entry root projection and filter full Entry runtime input by mode.
5. Map typed opaque schema errors.
6. Delete old imports/calls and broad scans.

Gate: fixture 013 passes unchanged; Flow/Entry reached variants fail with exact code/path.

## Phase 5 — runtime-plan admission

1. Bind fact input/output to reachability identity.
2. Reject owner-outside-closure and missing reached target facts.
3. Make Flow/helper inventories derive only from admitted facts.
4. Replace any reached-effectful `continue` with a defensive typed error.
5. Add deterministic digest/ordering tests.

Gate: runtime-plan crate fmt/clippy/tests green.

## Phase 6 — AWBC/native parity

1. Confirm no AWBC schema change is required.
2. Filter inventory through RuntimePlan only.
3. Add absence/dangling-target/type parity tests.
4. Verify no new `AwbcFunctionKind`, layout field, or version appears.
5. Run native/AWBC parity suites.

Gate: identical reached callable/type sets; unreachable function absent.

## Phase 7 — save/replay and tooling

1. Add save tests for absence of unreachable frames.
2. Add forged producer/semantic/layout restore tests.
3. Add display-only emission disposition to semantic tooling index.
4. Map CLI compile context to root mode and render typed paths.
5. Preserve schema/save versions at 1.

Gate: session-save and LSP/CLI tests green.

## Phase 8 — deletion and workspace closure

1. Search for every deleted owner/method/import and remove all hits.
2. Search for source/fixture-name gates, `TypeKind::Named` runtime success fallbacks, dummy schema construction, transient layout types, V2/version changes.
3. Run formatting, targeted tests, workspace tests, and Clippy.
4. Re-run fixture gates and deterministic/golden tests twice.
5. Update implementation status docs with actual SHA and test evidence.

No phase may retain an old owner as a compatibility alias. If a phase cannot compile cleanly, continue the same phase rather than checking in a dual path.
