# Required implementation order

This preserves the request's eight phases. A later phase may begin only after the prior phase's acceptance gate passes.

## 1. Finalize typed binary resource and revision ownership

**Owners/files:**

- `crates/arcweft-project/src/content.rs` plus deliberate facade export;
- `crates/arcweft-project/src/fingerprint.rs` only for owner-local canonical helpers/conversions;
- Cargo dependencies from project to the existing low-level manifest/character/resource/source owners;
- focused project content/revision tests.

**Work:** add `ProjectBinaryResource`, candidate/accepted content facts, `ProjectTopologyRevision`, canonical records, duplicate rejection, and exact v1 vectors. Replace the compiler-local duplicate `ContentUnitId` with the manifest-model owner.

**Gate:** revision vectors, insertion-order determinism, duplicate rejection, source/binary/absence changes, exact crate check/Clippy/fmt.

## 2. Add binary overlay/admission input without changing schema-1 decode

**Owners/files:** project-loader topology model/id/budget/loader/tests.

**Work:** add binary overlay/dependency seeds, payload enum, Character layer resource kind, separate maps, normalized containment, conflict/unconsumed checks, and combined overlay/resource/work charging. Rename the text-only topology accessor to `source_documents_revision`; add the candidate topology revision.

**Gate:** text paths unchanged; binary never enters `SourceDocument`; overlay precedence/no-fallback/conflict/budget tests pass; strict Taplo decoder files have no design change.

## 3. Construct and validate complete Character packages at the topology boundary

**Owners/files:** `arcweft-character::package`, project-loader Character acquisition, existing bundle package tests.

**Work:** share layer bytes with `Arc<[u8]>`, add source-backed constructor, full PNG/frame/dimension validation, exact manifest-named acquisition, and `LoadedCharacterPackage` provenance. Reuse existing membership checks and `BundleCharacterPackage`.

**Gate:** TCRA-001–020 and relevant current `.awchar` tests pass. One package or no package; no manifest-only successful topology remains for a selected/required Character root.

## 4. Add required/optional facts and closed family validation

**Owners/files:** launch `SourceBackedManifest` accessors, project content candidate, sema `EntityKind` inherent classification, accepted resource declaration lookup, compiler reachability/finalizer.

**Work:** process all content units, not only profile-selected units; group shared Character targets; build explicit absence candidates; resolve source/resources; build one project-wide typed `ContentRootReferenceInventory`; intercept exact absent-root reservations; finalize reference flags/errors. Existing compiler reachability remains the partition/bundle consumer, not the absence-admission oracle.

**Gate:** TCRA-051–088 and TCRA-111–114 pass, including optional absent/unreferenced acceptance, profile/runtime referenced failure, present-invalid fail-closed, aliases/reexports, and unknown/wrong family diagnostics.

## 5. Inject accepted manifest facts into `ProjectIndex`

**Owners/files:** sema project index/model/relations/tests; ProgramHash owner; compiler/LSP call sites.

**Work:** add `ProjectSemanticIndexInput`, content-unit/root fact tables, manifest-owned graph symbols/relations, topology revision invariant, and typed ProgramHash construction.

**Gate:** TCRA-089–092 and 096 pass; binary-only changes invalidate program/cache identity; no TOML reparse.

## 6. Delete source `content` syntax/HIR/sema/tooling ownership atomically

**Owners/files:** syntax parser/AST/CST/tests, HIR model/lowering/tests, sema symbols/checker/project-index producer, tooling/LSP/formatter/docs/fixtures.

**Work:** remove all source declaration variants and call sites, migrate maintained source to manifest units, retain manifest-generated graph relations, ordinary parser rejection only.

**Gate:** TCRA-093–095 pass, `cargo check` exposes and resolves every call site, no compatibility recognizer/alias/source gate remains.

## 7. Migrate bundle, watch, LSP, and maintained fixtures

**Owners/files:** bundle/CLI build orchestration, project-loader watch inventory consumers, LSP accepted project/environment/state/caches, maintained schema-1 manifests and `.awchar` fixtures.

**Work:** consume `AcceptedContentInventory`, direct package bundling, exact watch lists, topology-revision generation checks, binary-only rebuild handling, and no-LKG failure reporting.

**Gate:** TCRA-097–110 pass; bundle/watch inventories derive from the same accepted set; candidate failures publish nothing.

## 8. Focused, workspace, Tier 2, and structural validation

Run at the final reviewable cut:

```bash
cargo fmt --all --check
cargo test -p arcweft-project -p arcweft-character -p arcweft-project-loader \
  -p arcweft-lang-sema -p arcweft-compiler -p arcweft-bundle -p arcweft-lsp
cargo check -p arcweft-project -p arcweft-character -p arcweft-project-loader \
  -p arcweft-lang-sema -p arcweft-compiler -p arcweft-bundle -p arcweft-lsp \
  --all-targets --all-features
cargo clippy -p arcweft-project -p arcweft-character -p arcweft-project-loader \
  -p arcweft-lang-sema -p arcweft-compiler -p arcweft-bundle -p arcweft-lsp \
  --all-targets --all-features -- -D warnings
just test-workspace
just test-tier2
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

Tier 2 is mandatory because the cut changes a public multi-crate contract and affects bundle/runtime/LSP/Agent-observable generation. Structure-audit evidence must use current-file sizes and dependency graphs, not source-text gates.
