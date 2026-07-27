# Proof convergence: LSP feature exact-source leases

Date: 2026-07-27

Status: implementation and cut-specific validation complete

## Context

Proof-concurrency Stage 3 requires LSP readers to preserve one exact source
revision from the open document through parsing and HIR lowering. The corrected
final HIR leaf package requested by
[`01.1.1.4.1`](../reviews/requests/2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md)
has not returned. The repository still retains only the rejected `01.1.1.4`
archive at external SHA-256
`414f95f8ef4c5f3abcce163f0c9b01f124098f0bac856f174af09b5c1e7d564b`.
This cut therefore deletes source-identity splits that do not require the
missing leaf schema; it does not infer final expression, literal, call,
Thread, Dialogue, or RichText payloads.

At this push boundary, all 29 ZIP archives under `docs/reviews/` have a
case-insensitive exact SHA-256 record under `docs/implementation/`, and the
required corrected `01.1.1.4.1` archive count remains zero.

## Deleted authority

Four LSP feature families already owned an exact `Arc<SourceDocument>` but
discarded it at a subordinate parser or HIR boundary:

- code actions constructed a separate parser-owned memory document from
  `DocumentSnapshot::text()` and used source-free `lower_to_hir`;
- Character nominal, closure-effect, and callable-effect hover repeated the
  same source-free parse and lowering path;
- Character definition parsed `context.rebound.text()` into a second memory
  document before binding the resulting tree back to `context.rebound`; and
- View-part metadata parsed the exact document but dropped its identity during
  source-free HIR lowering. Its tests also kept a parallel source-string
  constructor.

The production readers now pass `Arc::clone(document.source_document())` or
`Arc::clone(&context.rebound)` directly to `parse_document_with_source` and
pass `parsed.document()` to `lower_document_to_hir`. The View-part tests enter
through the same `DocumentStore` and `for_document` boundary as production.
The old imports, text allocation, source-free HIR calls, and test-only metadata
constructor were deleted; no renamed wrapper or fallback was added.

This preserves existing grammar, default parse options, diagnostics, effect
accounting, Character request budget charges, cache keys, and LSP output while
ensuring the parse and HIR source maps describe the exact open or rebound
revision.

## Deliberately open final switch

This is not the public HIR expression authority switch. The following readers
remain frozen until the corrected leaf contract is accepted:

- code actions still perform request-local parse/HIR/type analysis and locate
  an authored effect clause or body insertion point with raw text scanning;
- hover still performs request-local HIR/type analysis rather than consuming
  one accepted project analysis;
- Character reference collection still accepts a detached `TypedSyntaxTree`,
  records `syntax_snapshot: None`, and reparses judgment source slices through
  `parse_expr`;
- Character definition still creates request-local HIR/type facts beside the
  accepted project generation; and
- test modules elsewhere still use the syntax crate's standalone
  `parse_source` fixture entrypoint.

Deleting those paths requires the final qualified expression arena, typed
effect/source components, actual `SourceSnapshotId`, and accepted HIR project
publication. No provisional ID, raw range, source-string callee, compatibility
reader, source gate, or removed-syntax diagnostic is introduced here.

Runtime assertion site/inventory, AWBC assertion codec, and save/checkpoint/
replay evidence likewise remain after the final `StmtId`/`ExprId` public switch;
this source-lease cut is not evidence for their completion.

## Validation

The implementation change was Jujutsu change
`lkmsopnpvowzqpmstxrsotpxrzxzysvw` over parent Git commit
`ab377b0ada9bdab2e1cbad8cd68e4a20d553da91`.

- `cargo fmt --all`: passed;
- `cargo check -p arcweft-lsp --all-targets --all-features`: passed;
- `cargo test -p arcweft-lsp --lib -- --nocapture`: passed all 212 tests,
  including code actions, effect/Character hover, View-part metadata, exact
  source lease, Character cache/work accounting, stale publication, and
  signature lifecycle rows;
- `cargo clippy -p arcweft-lsp --all-targets --all-features -- -D warnings`:
  passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-lsp-feature-exact-source-lease-2026-07-27`:
  scanned 3,728 files, 1,943 Rust files, 904,084 Rust physical LOC, and 95
  package manifests; reported 0 errors and 146 existing warnings; and
- `just test-workspace`: all preceding crate, integration, and compile-fail
  suites passed. The final `arcw_fixtures_check_run` suite retained its exact
  existing 3-pass/2-fail baseline:
  `spec_should_pass/check/010_capability_fs_read.arcw` and
  `spec_should_pass/run/002_file_read_task.arcw`. Both still report the known
  capability-owned `FsError` publication gap that belongs to the attached HIR
  public switch and do not exercise the changed LSP feature paths.

Tier 2 is not applicable: the cut does not change runtime, renderer, Agent,
MCP, capture, or a cross-crate public contract.

## Structure measurement

All changed Rust files belong to `arcweft-lsp`; no Cargo dependency or feature
edge changed.

| path | bytes | physical LOC | embedded test LOC | responsibility |
| --- | ---: | ---: | ---: | --- |
| `features/actions.rs` | 9,559 | 300 | 47 | typed verifier/effect code-action projection |
| `features/hover.rs` | 30,307 | 899 | 306 | nominal, effect, Dialogue, and presentation hover projection |
| `features/character_definition.rs` | 23,817 | 606 | 0 | bounded accepted-generation Character definition dispatch |
| `features/view_part_metadata.rs` | 14,155 | 371 | 62 | typed View-part metadata shared by hover/definition/reference/completion |

No production file crosses the 1,200 LOC warning threshold, no integration
test file is changed, and no new responsibility is added. Exact dependency,
file-size, and duplicate-type reports are retained under
[`structure-audits/proof-lsp-feature-exact-source-lease-2026-07-27/`](structure-audits/proof-lsp-feature-exact-source-lease-2026-07-27/).

## Next boundary

Continue only with leaf-independent deletion while the corrected package is
absent. Once it is returned and accepted, replace the request-local syntax/HIR
readers and source-slice reparsing in the same public compiling authority
switch as the final expression arena and module-preserving accepted project.
