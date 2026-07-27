# Proof convergence: source-free HIR lowering deletion

Date: 2026-07-27

Status: implementation and cut-specific validation complete

## Context

Proof-concurrency Stage 3 requires HIR allocation to retain the exact source
document revision that produced its syntax. An implementation-ready corrected
final HIR leaf package requested by
[`01.1.1.4.1`](../reviews/requests/2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md)
has not returned. The rejected `01.1.1.4` archive remains recorded at external
SHA-256
`414f95f8ef4c5f3abcce163f0c9b01f124098f0bac856f174af09b5c1e7d564b`.
A subsequently returned `01.1.1.4.1` archive is integrity-valid at SHA-256
`9ccb9af261a3d55bddefe570b4902d9ba6395725904f88bf389b4565e5bd8374`
but explicitly reports `NOT_READY` and contains no contract. Its intake is
recorded in
[`2026-07-27-proof-01-1-1-4-1-not-ready-redelivery-intake.md`](2026-07-27-proof-01-1-1-4-1-not-ready-redelivery-intake.md).
This cut therefore removes a source-free overload without choosing any missing
expression, literal, call, Dialogue, RichText, or runtime-assertion payload.

At this push boundary, all 30 ZIP archives under `docs/reviews/` have a
case-insensitive exact SHA-256 record under `docs/implementation/`. No
unrecorded archive exists. One archive has the corrected filename, but the
implementation-ready corrected contract count remains zero because its own
status is `NOT_READY`.

After the compiler facade deletion in parent `f9d53542`, public
`arcweft_lang_hir::lower::lower_to_hir` had no production caller. The remaining
517 direct calls across 51 Rust files were all tests backed by a parser-owned
`ParsedSource`; none constructed a `TypedSyntaxTree` by hand. Retaining the
parse product and passing its document and tree together was therefore the
single lossless migration.

## Deleted authority

- deleted public `lower_to_hir(&TypedSyntaxTree)` completely;
- moved the lowering implementation directly into
  `lower_document_to_hir(&SourceDocument, &TypedSyntaxTree)`, so source-text
  equality validation, HIR construction, and source-document binding are one
  non-bypassable operation;
- migrated 398 sema, 91 runtime-plan, 19 HIR, four verifier, and five remaining
  tooling/CLI/project-loader/test direct callers to the exact
  `ParsedSource.document()` / `.typed_tree()` pair;
- changed shared test owners to retain `ParsedSource` rather than consuming it
  into a detached tree;
- removed source-less HIR tests whose premise the public API no longer permits,
  replacing the observable readiness case with exact source-identity retention;
- removed test-only whitespace/source-document reconstruction and reused the
  document already retained by HIR or created one stable fixture document
  before parsing;
- added compile-fail evidence that downstream code cannot import
  `lower_to_hir`; and
- updated the stable crate map, CLI pipeline, and current implementation status
  to name the document-bound owner.

No alias, renamed wrapper, feature-gated escape hatch, synthetic identity,
source-string reconstruction, extension trait, compatibility shim, source
gate, or removed-syntax diagnostic was added. The old symbol was deleted first
and all compiler fallout was repaired through the existing typed owner.

## Deliberately open boundary

This cut is not the final HIR arena or accepted-project authority switch.
`lower_document_to_hir(document, tree)` and `TypedSyntaxTree` remain until the
corrected `01.1.1.4.1` package fixes the final semantic leaf payload.

The syntax crate's public raw-text `parse_source` facade now also has zero
production callers, but still has 385 test/fixture callers. It is the next
leaf-independent deletion cut. The private item-fragment parser remains on the
Agent REPL production path and must not be hidden behind another wrapper; its
removal belongs to the atomic attached-fragment tooling switch after the
corrected leaf contract is accepted.

The accepted module-preserving HIR project, compiler/LSP semantic-reader
switch, runtime assertion inventory, AWBC assertion codec, and
save/checkpoint/replay identity remain open in their dependency order.

## Validation

The implementation change is Jujutsu change
`pqqzoswynuylmwxsyopqlnzoxulkzmzz` over parent Git commit
`f95497e6d6abbedeb916e81d8608372ea7f6b3b2`.

- `cargo fmt --all`: passed;
- `cargo test -p arcweft-lang-hir --all-targets --all-features -- --nocapture`:
  passed 85 library tests, all HIR integration suites, and the new removed-API
  trybuild row;
- `cargo test -p arcweft-lang-sema --all-targets`: passed all 1,118 library
  tests and its integration/trybuild suites;
- combined runtime-plan and verifier tests: passed all 266 tests;
- focused tooling, project-loader, `arcweft-test`, and CLI bundle tests passed;
- strict changed-crate Clippy passed for every migrated crate;
- `git diff --check`: passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-source-free-hir-lowering-deletion-2026-07-27`:
  scanned 3,733 files, 1,945 Rust files, 904,604 Rust physical LOC, and 95
  package manifests; reported 0 errors and 146 existing warnings; and
- `just test-workspace`: every crate, integration, and compile-fail suite
  preceding the final CLI fixture gate passed. That final gate retained its
  exact existing 3-pass/2-fail baseline:
  `spec_should_pass/check/010_capability_fs_read.arcw` and
  `spec_should_pass/run/002_file_read_task.arcw`. Both still report the known
  capability-owned `FsError` attached-HIR publication gap and do not exercise
  the deleted source-free lowerer.

Tier 2 is not applicable: production lowering now performs the same HIR work
inside its already-used document-bound entry, all migrated callers are tests,
and no runtime, renderer, Agent, MCP, or capture path changes.

## Structure measurement

The production lowering owner is 21,372 bytes / 616 physical LOC after
integration and remains one cohesive parser-to-HIR lowering responsibility.
Most of this cut is test migration. The largest changed test owners are sema
`src/tests/typecheck.rs` at 143,720 bytes / 4,591 LOC and runtime-plan
`tests/runtime_plan.rs` at 63,173 bytes / 2,057 LOC. No manifest, dependency
edge, crate boundary, or production subsystem responsibility was added.
Exact dependency, size, hotspot, and duplicate-type reports are retained under
[`structure-audits/proof-source-free-hir-lowering-deletion-2026-07-27/`](structure-audits/proof-source-free-hir-lowering-deletion-2026-07-27/).

## Next boundary

Delete the public raw-text `parse_source` fixture facade by moving each crate's
tests to stable, exact `SourceDocument` fixtures. Do not rename that facade or
couple the deletion to the corrected leaf contract, and do not touch the
production item-fragment path until its atomic attached-fragment replacement is
ready.
