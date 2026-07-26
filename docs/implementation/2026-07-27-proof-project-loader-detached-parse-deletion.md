# Proof convergence: project-loader detached parse deletion

Date: 2026-07-27

Status: implementation and cut-specific validation complete

## Context

Proof Stage 3 requires every production parse to retain the identity of the
accepted `SourceDocument`. The returned Proof-concurrency `v6.1.1.4` archive
is still the rejected SHA-256
`414f95f8ef4c5f3abcce163f0c9b01f124098f0bac856f174af09b5c1e7d564b`,
so this cut does not infer the missing final HIR leaf payload. The atomic HIR
authority switch remains gated on the corrected
[`01.1.1.4.1` redelivery](../reviews/requests/2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md).

The project loader nevertheless had two schema-independent production readers
that parsed detached text before or beside the exact document retained by the
loaded project. Both readers only inspect module declarations and imports, so
they can be deleted without changing the HIR schema.

## Deleted authority

- `project::scan_source` now constructs the canonical project
  `Arc<SourceDocument>` before parsing and gives that exact allocation to
  `parse_document_with_source`;
- the parsed module declaration, import inventory, and retained
  `ProjectSourceFile` therefore share one document identity rather than a
  parser-owned synthetic identity and a separately constructed project
  identity;
- `topology::loader::load_module_dependencies` now receives the already
  retained module `Arc<SourceDocument>` lease and gives the same allocation to
  the bound parser while retaining it for exact import spans;
- both production `parse_source(text)` imports and calls were removed from
  `arcweft-project-loader` in the same change;
- no replacement wrapper, string parser, fallback, alias, source gate, or
  compatibility reader was added.

The topology unit-test fixture that parses a standalone string to exercise
adapter semantic registration remains test-only and is not a production
reader. The syntax crate's detached standalone/test API is not claimed as
deleted by this cut.

## Validation

- working change: `ppmrxylvpurttxwnrpnnpllpuuyuwvxp` over parent
  `06c810230f49`;
- `cargo test -p arcweft-project-loader`: passed 136 library tests, 4
  dependency-direction tests, the public API compile-fail row, 6 release-trust
  integration tests, and documentation tests;
- the passing library matrix includes bounded project loading, exact selected
  source overlay import closure, unresolved-import source evidence, bounded
  topology diagnostics, and retained source-revision coverage;
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
  docs/implementation/structure-audits/proof-project-loader-detached-parse-deletion-2026-07-27`:
  scanned 3,703 files, 1,935 Rust files, and 902,306 Rust physical LOC; reported
  0 errors and 144 existing warnings. Exact measurements and dependency
  evidence are retained in the
  [structure audit](structure-audits/proof-project-loader-detached-parse-deletion-2026-07-27/violations.md).

The changed production files are
`crates/arcweft-project-loader/src/project.rs` at 27,626 bytes / 836 physical
LOC and `crates/arcweft-project-loader/src/topology/loader.rs` at 51,744 bytes /
1,341 physical LOC. The topology loader remains an existing structural warning
because it owns profile topology orchestration across resource acquisition,
module discovery, and retained-document publication. This cut adds no new
responsibility or dependency edge; it replaces one detached reader at each
existing owner.

## Remaining boundary

This is a preparatory deletion, not Proof Stage 3 completion. The corrected
final leaf schema, attached HIR expression arena, module-preserving accepted
project, compiler/LSP authority switch, and remaining detached syntax readers
stay open. LSP hover and action readers must be migrated together with their
local HIR/typecheck fallbacks; replacing only their parse call would preserve
dual semantic authority.
