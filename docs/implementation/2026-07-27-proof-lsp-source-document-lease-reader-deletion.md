# Proof convergence: LSP source-document lease reader deletion

Date: 2026-07-27

Status: implementation and cut-specific validation complete

## Context

Proof Stage 3 requires syntax readers and their tooling consumers to retain one
revision-bound source allocation. The returned Proof-concurrency `v6.1.1.4`
archive is still the rejected SHA-256
`414f95f8ef4c5f3abcce163f0c9b01f124098f0bac856f174af09b5c1e7d564b`,
so this cut does not infer its missing HIR leaf payload. The atomic HIR switch
remains gated on the corrected
[`01.1.1.4.1` redelivery](../reviews/requests/2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md).

Two LSP detached-text syntax entrypoints were independently removable now.
Inlay syntax is only used to match source sites against the accepted project's
typecheck report, while document-symbol syntax is only used to project the
current open outline. Neither requires a local HIR or semantic fallback.

## Deleted authority

- changed `DocumentSnapshot::source_document` directly from a borrowed
  `SourceDocument` view to the exact retained `&Arc<SourceDocument>` lease;
- did not add a second accessor, wrapper, alias, or compatibility return path;
- changed inlay syntax to parse the exact accepted project document allocation
  after the existing URI/revision overlay gate succeeds;
- changed entry-role document-symbol syntax to parse the exact open snapshot
  allocation, including the no-manifest local-outline path;
- deleted both corresponding production `parse_source(text)` calls and imports;
- changed View-part metadata from `Arc::new(SourceDocument::clone())` to
  `Arc::clone` of the snapshot lease, deleting the second document allocation;
- added direct pointer-identity evidence that a full-sync result and the
  snapshot retained by `DocumentStore` expose the same source allocation;
- added no source gate, text fallback, detached parser wrapper, or shim.

## Explicit remaining boundary

This cut does not claim complete LSP semantic authority convergence.

- diagnostics still owns its local parse/lower/resolve/typecheck/verifier
  pipeline for open-document publication;
- hover and effect actions still own local HIR/typecheck readers;
- Character definition still owns its local registered typecheck/reference
  inventory and cache accounting;
- View-part metadata now uses the exact source allocation, but its local
  `lower_to_hir` and `analyze_types` reader remains until the accepted project
  catalog can replace all consumers atomically.
- inlay, document-symbol, and View-part consumers still perform a request-local
  bound `parse_document_with_source` and read its provisional `typed_tree()`;
  final Stage 3 must lease the accepted/open attached syntax snapshot and
  delete these caller-local reparses as part of the public authority switch.

Those pipelines must be deleted together with their semantic fallback and
dirty-overlay policy. Replacing only their parse call would hide, rather than
remove, dual authority.

## Validation

- working change: `pnxzwkolvplplzomxzrklzktmskxpznw` over parent
  `0880177493af`;
- `cargo test -p arcweft-lsp`: passed 211 library tests and all binary,
  integration, dependency-direction, and documentation tests;
- focused exact-source rows passed for the full-sync Arc lease, accepted-source
  inlays, document outlines without a manifest, and disjoint View-part
  definitions/references;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `just test-workspace`: every crate, integration, and compile-fail suite
  passed before the final `arcw_fixtures_check_run` suite retained the two
  pre-existing `FsError` failures
  `spec_should_pass/check/010_capability_fs_read.arcw` and
  `spec_should_pass/run/002_file_read_task.arcw`;
- `cargo test -p arcweft-cli --test arcw_fixtures_check_run -- --nocapture`
  independently confirmed exactly those two failures and no others;
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-lsp-source-document-lease-reader-deletion-2026-07-27`:
  scanned 3,705 files, 1,935 Rust files, and 902,316 Rust physical LOC; reported
  0 errors and 144 existing warnings. Exact measurements and dependency
  evidence are retained in the
  [structure audit](structure-audits/proof-lsp-source-document-lease-reader-deletion-2026-07-27/violations.md).

The changed files are `documents.rs` at 7,843 bytes / 230 physical LOC,
`features/inlay.rs` at 15,250 bytes / 454 LOC,
`features/entry_roles/presentation.rs` at 9,493 bytes / 269 LOC, and
`features/view_part_metadata.rs` at 13,858 bytes / 369 LOC. No file crosses a
structural warning threshold and no dependency edge changed. The
`DocumentSnapshot` public return contract changed materially, so the repository
structural audit was run before push despite the files remaining below warning
thresholds.
