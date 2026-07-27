# AW-AH-009.3 signature-help decided substrate

## Status

Partially implemented against parent Git commit
`76d39983ad8770a87d6e81745785b6b362a381b4`. This is a coherent prerequisite
cut, not completion of the AW-AH-009.3 acceptance matrix.

The source package is
`arcweft-aw-ah-009.3-character-nominal-signature-help-final-contract.zip`, with
SHA-256
`cdd1d7b764da238a6e4e8f3e774a3384017c8da5ffaea1969f2af279102a7cd5`.
All 11 archive members, manifest digests, lexical ordering, exact membership,
and the zero self-entry rule were verified. The Downloads location contained
no external summary, status, or SHA-256 sidecars; no claim is made that those
outside artifacts were supplied.

## Implemented contract subset

- `LineIndex::try_byte_offset_from_position` returns an exact UTF-8 byte
  offset under the negotiated UTF-8 or UTF-16 encoding. It rejects missing
  lines, characters beyond authored line content, UTF-8 scalar splits, UTF-16
  surrogate splits, and checked arithmetic overflow through
  `CheckedPositionError`; it never clamps.
- At this historical cut, clamping `byte_offset_from_position` behavior
  remained unchanged for unrelated LSP features. The method later reached zero
  production consumers and was deleted in favor of the checked API; see
  [`2026-07-27-proof-lsp-zero-consumer-public-surface-deletion.md`](2026-07-27-proof-lsp-zero-consumer-public-surface-deletion.md).
- `HirModule::source_identity` exposes only the exact revision-bound
  `SourceDocumentIdentity` established by `lower_document_to_hir`.
- `HirModule::module_path` always exposes one canonical path, using `crate` for
  an omitted source declaration and the exact project path after project
  binding.
- `ProjectSymbolTable::source_identity` retains and returns the exact document
  identity for every linked module, including modules with no declarations.

No new dependency or Cargo manifest change was required. No compatibility
wrapper, source gate, word resolver, CSS/Takumi path, or removed-syntax
diagnostic was added.

## Production reconciliation boundary

The package's required non-optional `CallExpressionSyntax` cannot represent
the current postfix callback-block call `target.member { closure }`: production
lowers that authored surface to `Expr::Call`, but it has no parenthesized
`ArgumentListSyntax`. Public and direct synthetic `Expr::Call` construction
also has no source identity from which exact ranges could be built.

Fabricating parentheses, adding a convenience `Option`, or treating braces as
the contract's parenthesized list would be result-changing implementation
decisions. The independently throwable
[AW-AH-009.3.1 call-surface syntax production reconciliation](../reviews/requests/2026-07-16-aw-ah-009.3.1-call-surface-syntax-production-reconciliation.md)
freezes that missing model. Parser call-range migration and all dependent
sema/LSP signature-query work remain open until that result is available.

This reconciliation does not block the implemented position and identity
substrate, nor the five other package workers running independently.

## Validation

All Rust commands used `CARGO_INCREMENTAL=0`.

```bash
cargo test -p arcweft-lsp positions
cargo test -p arcweft-lang-hir
cargo test -p arcweft-lang-hir table_retains_source_identity_for_every_module
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/aw-ah-009-3-decided-substrate
```

All commands passed. The LSP filter ran 6 position tests. The HIR suite ran
43 unit tests, 3 project-symbol integration tests, 1 public-API test, 2 style
tests, all compile-fail cases, and doc tests without failure. The exact
source-identity test was rerun successfully after the clippy-driven iterator
correction and root target cleanup.

The first clippy attempt identified an `expect` in symbol-table construction.
The fix exposed a crate-owned exact source-identity iterator on `HirProject`
and removed the panic rather than documenting it as part of the public link
contract. The full workspace clippy rerun then passed.

The canonical structural report is
[`structure-audits/aw-ah-009-3-decided-substrate/`](structure-audits/aw-ah-009-3-decided-substrate/).
It scanned 2,863 files, including 1,408 Rust files and 662,201 physical Rust
LOC, and reported zero errors and 128 existing warnings. This cut adds no
dependency edge or manifest change. Current changed-Rust-file measurements are:

| Path | Bytes | Physical LOC | Class | Embedded tests |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-lang-hir/src/lower.rs` | 14,724 | 435 | production | yes |
| `crates/arcweft-lang-hir/src/model.rs` | 27,486 | 1,078 | production | no |
| `crates/arcweft-lang-hir/src/project.rs` | 18,701 | 515 | production | yes |
| `crates/arcweft-lang-hir/src/symbol/table.rs` | 40,077 | 1,110 | production | no |
| `crates/arcweft-lang-hir/src/symbol/tests.rs` | 25,248 | 720 | test | no |
| `crates/arcweft-lsp/src/positions.rs` | 12,370 | 347 | production | yes |

The normal push-cut command was also attempted:

```bash
just test-workspace
```

It did not complete. Parallel workspace builds had reduced free space on D: to
336,756,736 bytes; MSVC linking failed first with `LNK1180` (insufficient disk
space) and then `LNK1140` (program database limit). No Rust test assertion or
compiler diagnostic failed before the resource failure. Per the repository's
capacity policy, `cargo clean` was run only in the root workspace and removed
63,953 generated files / 104.4 GiB, restoring 107,100,680,192 bytes free. The
independent package workspaces and their in-progress targets were not touched.

`just test-workspace` remains an explicitly unverified gate for this partial
cut and must be rerun at a later controlled-capacity milestone. Focused tests,
full workspace check, full workspace clippy, formatting, and structural audit
are successful evidence for the implemented subset; they are not presented as
a substitute for the missing normal workspace test.

Tier 2 MCP stdio and exact visual golden tests are not selected because this
cut changes neither risk area.

## Remaining AW-AH-009.3 work

- accept the 009.3.1 final call-surface model;
- retain exact parser call/argument/recovery ranges and preserve them in HIR;
- add the one shared sema call resolver and public signature query;
- normalize accepted adapter callable metadata into that query;
- add typed LSP request stamps and bounded signature cache;
- delete the competing word-only Rust signature resolver; and
- complete the package test matrix and final workspace validation.
