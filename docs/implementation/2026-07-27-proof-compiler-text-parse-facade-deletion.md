# Proof convergence: compiler text-parse facade deletion

Date: 2026-07-27

Status: implementation and cut-specific validation complete

## Context

Proof Stage 3 requires source parsing, HIR lowering, accepted project
publication, and tooling readers to retain one revision-bound source identity.
The returned Proof-concurrency `v6.1.1.4` archive is still the rejected
SHA-256
`414f95f8ef4c5f3abcce163f0c9b01f124098f0bac856f174af09b5c1e7d564b`,
so this cut does not infer the missing final HIR leaf payload or perform the
atomic public HIR switch. That switch remains gated on the corrected
[`01.1.1.4.1` redelivery](../reviews/requests/2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md).

The compiler nevertheless still exported `parse_source_text`, a pass-through
wrapper that accepted detached text and constructed a new parser-owned source
identity. Production CLI consumers already owned the real source document, so
keeping the wrapper permitted those consumers to discard the accepted
document identity before parsing.

## Deleted authority

- removed the public `arcweft_compiler::parse::parse_source_text` function;
- changed project persistent-fact reconstruction to parse the exact
  `Arc<SourceDocument>` retained by the accepted `ProjectSourceFile`;
- changed Agent RAG source parsing to create one path-owned
  `Arc<SourceDocument>` and pass that exact allocation through parsing, HIR
  lowering, and semantic-index projection;
- moved compiler-only test fixtures directly to the syntax crate's
  `parse_source` owner instead of preserving a compiler parser facade for test
  convenience;
- added compile-fail evidence that downstream code cannot import the deleted
  compiler API;
- retained no alias, renamed compiler wrapper, extension trait, text-to-parser
  fallback at this compiler boundary, source gate, or compatibility shim.

The compiler's private `parse_source_document` remains the production helper
for compiler-owned accepted documents. It accepts an exact
`Arc<SourceDocument>` and delegates to `parse_document_with_source`; it cannot
manufacture a source document from text.

This cut does not claim the complete Stage 3 single-parse switch. Project
persistent-fact reconstruction still reparses the exact accepted document,
and the syntax crate still exposes the detached `parse_source` test/standalone
entry and public detached lowerer. Deleting those remaining readers requires
the corrected final HIR leaf schema and the atomic consumer migration; they
are listed below as remaining work rather than hidden as completed here.

## Existing Agent graph blocker exposed during validation

The focused Agent RAG persistence fixture fails after source parsing, while
inserting a project graph dependency edge into SQLite. The same test was run
in an isolated worktree at parent `9fda76cb` and failed identically, proving
that this is not a regression from the parse-facade deletion.

The existing inconsistency is precise:

- `project_semantic_index_from_hir` builds no checked project-callable symbols;
- `index_project_symbol_dependency_relations` nevertheless publishes callable
  dependency endpoints from HIR function bodies;
- the first dangling endpoint in the fixture is the callable
  `current_route` referencing `flow.opening`;
- `ProjectGraphSymbolRef::Callable(QualifiedName)` also loses package, module,
  and Function/View owner identity, while the CLI graph ID projection
  hard-codes the `function` owner.

This cut does not restore the removed context-free `TypeKind::Named` callable
projection, synthesize endpoint stubs, filter missing edges, or weaken SQLite
foreign keys. The final graph repair must consume canonical checked project
callable declarations, retain `CallableDeclarationId`-equivalent owner
identity, and validate graph closure before persistence. It belongs after the
corrected HIR/project authority contract rather than inside this preparatory
parser deletion.

## Validation

- the independently audited working change was Jujutsu change
  `rrlxwswvmrkqxlsqssxmqnmosolzmpnz` over parent `9fda76cb`;
- `cargo test -p arcweft-compiler`: passed all library, integration,
  compile-fail, and documentation tests, including 92 library tests and the
  new removed-API row;
- `cargo test -p arcweft-cli --test check
  agent_rag_query_indexes_source_project_chunks -- --nocapture`: passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `just test-tier2`: passed the complete MCP stdio, Agent native capture, and
  exact visual-golden suite;
- `just test-workspace`: all workspace crate and compile-fail suites passed;
  the final `arcw_fixtures_check_run` suite retained only the pre-existing two
  `FsError` failures:
  `spec_should_pass/check/010_capability_fs_read.arcw` and
  `spec_should_pass/run/002_file_read_task.arcw`;
- `cargo test -p arcweft-cli --test arcw_fixtures_check_run -- --nocapture`
  independently confirmed exactly those two failures and no others;
- `cargo test -p arcweft-cli --test check
  agent_rag_index_persists_source_chunks_and_skips_unchanged -- --nocapture`
  failed with the existing graph foreign-key error both on this change and in
  the isolated parent worktree. The temporary worktree was removed after the
  comparison.

- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-compiler-text-parse-facade-deletion-2026-07-27`:
  scanned 3,702 files, 1,935 Rust files, and 902,306 Rust physical LOC; reported
  0 errors and 144 existing warnings. Exact measurements and dependency
  evidence are retained in the
  [structure audit](structure-audits/proof-compiler-text-parse-facade-deletion-2026-07-27/violations.md).

The changed production hotspots remain pre-existing responsibility warnings:
`agent/rag/source_index.rs` is 67,796 bytes / 1,840 physical LOC,
`agent/rag.rs` is 48,912 bytes / 1,353 LOC, `project_commands.rs` is 82,311
bytes / 2,276 LOC, and `compiler/persistent.rs` is 53,734 bytes / 1,427 LOC.
This cut adds no subsystem to those owners: it replaces one parse call at each
production consumer and removes the wrapper. The compiler parse module is
1,176 bytes / 31 LOC after deletion. No manifest or dependency edge changed.

## Remaining boundary

This cut removes one detached production parse entry point; it is not Proof
Stage 3 completion. The corrected final leaf schema, attached HIR expression
arena, module-preserving project authority, compiler/LSP switch, typed Agent
graph callable identity, runtime assertion inventory, codecs, and
save/replay identity remain open in dependency order.
