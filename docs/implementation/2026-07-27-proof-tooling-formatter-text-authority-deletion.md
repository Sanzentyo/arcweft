# Proof convergence: tooling formatter text-authority deletion

Date: 2026-07-27

Status: implementation and cut-specific validation complete

## Context

Proof Stage 3 requires every syntax consumer to remain bound to one exact
source-document allocation and revision. The returned Proof-concurrency
`v6.1.1.4` archive is byte-identical to the rejected package at SHA-256
`414f95f8ef4c5f3abcce163f0c9b01f124098f0bac856f174af09b5c1e7d564b`.
This cut therefore does not infer or implement the missing final HIR semantic
leaf payload. The atomic leaf-schema switch remains gated on the corrected
[`01.1.1.4.1` redelivery](../reviews/requests/2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md).

The formatter and its LSP code-action projection did not need that leaf schema,
but still exposed a public raw-text parser authority. CLI formatting passed
only the file text and established no path-owned document before tooling, while
LSP code actions discarded their retained `Arc<SourceDocument>` before
formatting. Those detached readers were independently removable.

## Deleted authority

- deleted the public `format_source(&str, ...)` entrypoint outright;
- replaced it with `format_document(Arc<SourceDocument>, ...)`, which moves the
  exact lease into `parse_document_with_source` and reads the source only from
  the resulting bound `ParsedSource`;
- changed source code actions to require an `Arc<SourceDocument>`, capture the
  exact document length, and move the same lease into the formatter;
- changed verify-LSP to borrow an `&Arc<SourceDocument>`, clone that exact lease
  for tooling, and project returned edit ranges against the same document;
- changed the CLI filesystem adapter to create one path-derived
  `SourceDocument`, wrap it once in `Arc`, and pass it through the tooling
  boundary instead of passing raw source text;
- added compile-fail evidence that the deleted formatter symbol is unavailable
  and a raw `&str` cannot call the code-action boundary;
- added no wrapper, compatibility alias, dual reader, source gate, sentinel,
  or text fallback.

Test-only `format_fixture` helpers construct typed fixture documents and call
the production API. They do not preserve or expose a production raw-text
authority.

## Explicit remaining boundary

This cut does not claim the final Proof syntax/HIR authority switch.

- the formatter still performs one request-local, source-bound
  `parse_document_with_source` and reads its provisional typed tree through
  formatting helpers;
- LSP diagnostics, hover, effect actions, Character definition, and View-part
  metadata still contain local syntax/HIR/sema pipelines that cannot be
  replaced safely without the accepted project inventory and final leaf
  schema;
- Stage 3 must replace those caller-local parsed/typed readers with the owned
  attached syntax snapshot and accepted HIR/project generation, then delete the
  old pipelines in the same public-authority switch.

Changing only those parse calls would leave their independent semantic
authorities intact, so they remain frozen rather than partially wrapped.

## Validation

- working change: `wonzolqxkvpxqvmuzpslpnxulowqrnsz` over parent
  `3c797b581ada`;
- `cargo fmt --all -- --check`: passed;
- `cargo test -p arcweft-tooling`: passed all library, integration,
  compile-fail, and documentation tests;
- focused verify-LSP, LSP code-action, and CLI formatter tests passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- independent read-only review reported no findings for exact-Arc identity,
  edit ranges, LSP projection, compile-fail evidence, or compatibility paths;
- `just test-workspace`: every crate, integration, and compile-fail suite
  passed before the final `arcw_fixtures_check_run` suite retained the two
  pre-existing `FsError` fixture failures
  `spec_should_pass/check/010_capability_fs_read.arcw` and
  `spec_should_pass/run/002_file_read_task.arcw`;
- `cargo test -p arcweft-cli --test arcw_fixtures_check_run -- --nocapture`
  independently confirmed exactly those two failures and no others;
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-tooling-formatter-text-authority-deletion-2026-07-27`:
  scanned 3,710 files, 1,939 Rust files, and 902,395 Rust physical LOC; reported
  0 errors and 144 existing warnings. Exact file measurements and dependency
  evidence are retained in the
  [structure audit](structure-audits/proof-tooling-formatter-text-authority-deletion-2026-07-27/violations.md).

Changed production files measure as follows: CLI `app/tooling.rs` is 5,066
bytes / 166 physical LOC; tooling `format.rs` is 1,532 bytes / 39 LOC; tooling
`code_actions.rs` is 1,273 bytes / 48 LOC; and verify-LSP `lib.rs` is 68,170
bytes / 1,810 LOC. The verify-LSP facade remains above the repository warning
threshold, but this cut adds no new subsystem responsibility: its changed code
is the existing tooling-to-LSP boundary projection. No production dependency
edge changed; tooling only inherits the existing workspace `trybuild`
dependency for compile-fail tests. The public contract and manifest changed, so
the repository structural audit was run before push. This cut does not touch a
runtime, render, Agent, MCP, or capture path, so Tier 2 is not required by the
test-execution policy.
