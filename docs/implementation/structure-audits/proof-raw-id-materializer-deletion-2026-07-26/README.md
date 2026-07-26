# Proof raw-source ID materializer deletion structural audit

Date: 2026-07-26

Repository state: Jujutsu change `wkotzwtz` / working-copy commit `c387b9da`,
based on `e7656f71` (`Derive typed Proof runtime assertion guards`).

## Result

The canonical structural audit scanned 3,700 files, including 1,939 Rust files
and 906,506 physical Rust lines. Cargo metadata covered 95 workspace package
manifests. It reported 0 errors and 146 pre-existing warnings. This deletion
does not add a crate, dependency edge, public compatibility surface, duplicate
identity payload, or source gate.

The generated evidence is retained in:

- `file_metrics.csv`;
- `dependency_edges.csv`;
- `public_type_duplicates.csv`; and
- `violations.md`.

## Changed Rust files

All sizes are exact measurements of the current checkout. Deleted files use
their last pre-deletion measurements so the removed ownership remains auditable.

| Path | Bytes | Physical LOC | Classification | Responsibility after this cut |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-cli/src/app/commands.rs` | 4,055 | 124 | production | Current Clap command vocabulary; no ID materializer command |
| `crates/arcweft-cli/src/app/tooling.rs` | 10,389 | 303 | production | Formatter and semantic canonicalization adapters only |
| `crates/arcweft-cli/src/app.rs` | 5,243 | 123 | production | Current CLI dispatch without an ID-materialization arm |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 226,768 | 6,990 | integration test | CLI/runtime/benchmark product tests; obsolete materializer case removed |
| `crates/arcweft-lang-hir/src/id_context.rs` | 8,040 | 233 | deleted production, including 21 embedded test LOC | Removed raw-source/CST identity scanner and public materialization records |
| `crates/arcweft-lang-hir/src/lib.rs` | 961 | 34 | production facade | HIR responsibility namespaces; no provisional `id_context` export |
| `crates/arcweft-lsp/src/commands.rs` | 1,473 | 46 | production | Current command IDs and advertisement list |
| `crates/arcweft-lsp/src/features/inlay.rs` | 15,859 | 470 | production | Ordinary typed expression/function inlay projection |
| `crates/arcweft-lsp/src/session/tests.rs` | 80,868 | 2,360 | integration-style crate test | LSP session behavior for retained commands, edits, and hints |
| `crates/arcweft-tooling/src/code_actions.rs` | 1,828 | 64 | production | Retained source actions without raw ID edits |
| `crates/arcweft-tooling/src/id_context.rs` | 1,462 | 46 | deleted production | Removed ID edit/inlay projection adapter |
| `crates/arcweft-tooling/src/lib.rs` | 519 | 22 | production facade | Current tooling subsystem namespaces |
| `crates/arcweft-tooling/src/model.rs` | 6,927 | 211 | production | Transport-neutral diagnostics, edits, reports, and code actions |
| `crates/arcweft-tooling/src/tests.rs` | 41,023 | 1,110 | crate test | Retained formatter, canonicalization, and action behavior |
| `crates/arcweft-verify-lsp/src/lib.rs` | 70,688 | 1,865 | production with 796 embedded test LOC | Verifier-to-tooling/LSP projection; inferred raw-ID hint mapper removed |

No changed production file crossed a threshold because of this cut. The
existing `arcweft-verify-lsp/src/lib.rs` size/test warnings and the 6,990-line
CLI integration-test warning remain visible in `violations.md`; this deletion
reduces both files and does not add another responsibility to them.

## Dependency context

No manifest changed. The affected package fan-in/fan-out counts from the
generated dependency graph are:

| Package | Fan-in | Fan-out | Boundary relevance |
| --- | ---: | ---: | --- |
| `arcweft-cli` | 0 | 69 | top-level adapter; deleted its tooling command consumer |
| `arcweft-lang-hir` | 11 | 5 | lower typed owner; deleted the false raw-source producer at its source |
| `arcweft-lsp` | 1 | 33 | transport adapter; deleted command and inlay projections |
| `arcweft-tooling` | 4 | 7 | Sans I/O tooling boundary; deleted ID edit/hint projection |
| `arcweft-verify-lsp` | 1 | 10 | verifier/tooling adapter; deleted the inferred-hint consumer |

The remaining dependency direction continues to place syntax below HIR,
tooling above syntax/HIR/sema, and CLI/LSP above tooling. No higher-layer
dependency was introduced into HIR.
