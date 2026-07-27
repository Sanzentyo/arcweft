# Proof convergence: compiler source-free HIR facade deletion

Date: 2026-07-27

Status: implementation and cut-specific validation complete

## Context

Proof-concurrency Stage 3 requires every HIR consumer to retain the exact
source-document revision that produced the syntax tree. The corrected final
HIR leaf package requested by
[`01.1.1.4.1`](../reviews/requests/2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md)
has not returned. The repository still retains only the rejected `01.1.1.4`
archive at external SHA-256
`414f95f8ef4c5f3abcce163f0c9b01f124098f0bac856f174af09b5c1e7d564b`.
This cut therefore deletes a source-free compiler escape hatch without
choosing any missing expression, literal, call, Dialogue, RichText, or runtime
assertion payload.

At this push boundary, all 29 ZIP archives under `docs/reviews/` have a
case-insensitive exact SHA-256 record under `docs/implementation/`. No
unrecorded archive exists, and the corrected `01.1.1.4.1` archive count remains
zero.

`arcweft_compiler::hir::lower_source_tree` had no production caller. Its 51
workspace callers were all tests: 45 compiler unit-test calls, five persistent
query unit-test calls, and one trait integration-test call. Every caller
already obtained its tree from one `ParsedSource`, so there was one lossless
migration: retain that parse product and pass its document and tree together
to `lower_source_document`.

## Deleted authority

- removed the public source-free `lower_source_tree` compiler facade and its
  direct import of the HIR crate's detached lowerer;
- migrated all 51 test callers to
  `lower_source_document(parsed.document(), parsed.typed_tree())`;
- removed the associated `ParsedSource` clones and `into_typed_tree()` calls
  that discarded the document while retaining only the detached tree;
- replaced the last compiler-test direct detached HIR read and synthetic
  whitespace document with one real parsed document;
- added positive evidence that compiler lowering retains the exact parsed
  document identity; and
- added compile-fail evidence that downstream code cannot import the deleted
  facade.

No alias, renamed wrapper, source-string reconstruction, synthetic identity,
extension trait, compatibility shim, source gate, or removed-syntax diagnostic
was added. The old API was deleted first and all compile fallout was repaired
through the existing document-bound owner.

## Deliberately open boundary

This is not the final HIR arena or accepted-project authority switch.
`arcweft_lang_hir::lower::lower_to_hir` remains public for test callers, and
the syntax crate's standalone `parse_source` fixture facade remains public.
Both currently have no production caller after the compiler facade deletion,
but deleting them requires separate workspace-wide test migrations. The final
expression/HIR/project schema remains gated on the corrected `01.1.1.4.1`
package and is not inferred here.

Runtime assertion inventory, AWBC assertion codec, save/checkpoint/replay
identity, and request-local LSP semantic-reader deletion also remain open.

## Validation

The implementation change is Jujutsu change
`stwzsmqptqrnxxsmvkoxomupkuqtlyvp` over parent Git commit
`21f792b54df70bb852571f7cea7d4a3f4d59d3be`.

- `cargo fmt --all`: passed;
- `cargo check -p arcweft-compiler --all-targets --all-features`: passed;
- `cargo test -p arcweft-compiler --lib --tests -- --nocapture`: passed all
  92 library tests and every compiler integration/compile-fail test;
- the new trybuild row proves that `lower_source_tree` is no longer importable;
- `cargo clippy -p arcweft-compiler --all-targets --all-features -- -D warnings`:
  passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-compiler-source-free-hir-facade-deletion-2026-07-27`:
  scanned 3,730 files, 1,944 Rust files, 904,127 Rust physical LOC, and 95
  package manifests; reported 0 errors and 146 existing warnings; and
- `just test-workspace`: every crate, integration, and compile-fail suite
  preceding the final CLI fixture gate passed. That final gate retained its
  exact existing 3-pass/2-fail baseline:
  `spec_should_pass/check/010_capability_fs_read.arcw` and
  `spec_should_pass/run/002_file_read_task.arcw`. Both still report the known
  capability-owned `FsError` attached-HIR publication gap and do not exercise
  the deleted compiler test facade.

Tier 2 is not applicable: this cut removes a compiler API with zero production
callers and changes no runtime, renderer, Agent, MCP, or capture path.

## Structure measurement

The changed compiler facade is 3,264 bytes / 83 physical LOC. The existing
`persistent.rs` production owner is 53,732 bytes / 1,427 LOC, but this cut only
changes its embedded test module. The compiler `src/tests.rs` unit-test owner
is 132,246 bytes / 3,664 LOC. The trait integration test is 1,065 bytes / 29
LOC, and the trybuild source and expectation are five lines each. No manifest,
dependency edge, crate boundary, or production responsibility was added.
Exact dependency, size, hotspot, and duplicate-type reports are retained under
[`structure-audits/proof-compiler-source-free-hir-facade-deletion-2026-07-27/`](structure-audits/proof-compiler-source-free-hir-facade-deletion-2026-07-27/).

## Next boundary

Continue deletion-driven convergence with the remaining source-free public
test facades while `01.1.1.4.1` is absent. Do not begin the final semantic leaf
or accepted-project switch until that corrected package has been returned and
accepted.
