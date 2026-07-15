# Current-grammar parser cleanup

## Result

Jujutsu change `rwptwtxr`, based on `main` `dd5d53b8`, removes the remaining
presentation-call, hook-header, memo-option, and Agent line-command
old-spelling recognizers in this cut. The flow parser now uses its ordinary
unsupported-item recovery. Hook headers are validated as the closed current
`on` / `phase` / `when` / `priority` / `once` / `effects` grammar. Memo
declarations retain only validated typed `scope` / `key` / `depends` / `track`
options; invalid or unknown lines produce ordinary current-grammar diagnostics
and do not enter the typed option list.

Repeated header lines retain their original byte offsets. Duplicate and unknown
diagnostics therefore point at each authored line instead of resolving repeated
text back to its first occurrence.

## Structural audit

The canonical audit scanned 2,744 files, including 1,306 Rust files and 641,152
Rust physical LOC. It reported 0 errors and 127 existing warning-level
hotspots. Reports were generated outside the checkout under
`D:/git/arcweft-targets/removed-parser-structure`.

Changed Rust file measurements from the current checkout:

| Path | Bytes | Physical LOC | Role | Major responsibility |
|---|---:|---:|---|---|
| `crates/arcweft-lang-syntax/src/ast/items.rs` | 45,104 | 1,910 | production | top-level surface AST; owns `MemoOption` |
| `crates/arcweft-lang-syntax/src/parser/flow.rs` | 31,106 | 747 | production | current flow grammar and generic recovery |
| `crates/arcweft-lang-syntax/src/parser/headers.rs` | 35,490 | 1,059 | production | shared declaration-header parsing |
| `crates/arcweft-lang-syntax/src/parser/helpers.rs` | 34,367 | 1,019 | production + 75 embedded test LOC | shared syntax parsing helpers and source offsets |
| `crates/arcweft-lang-syntax/src/parser/hooks.rs` | 8,079 | 215 | production | closed hook-header grammar |
| `crates/arcweft-lang-syntax/src/parser/items.rs` | 54,632 | 1,560 | production | top-level item parsing and typed memo options |
| `crates/arcweft-lang-syntax/src/tests/cst.rs` | 21,207 | 650 | unit test | Agent-dialect syntax boundary |
| `crates/arcweft-lang-syntax/tests/parser_declarations_recovery_comments.rs` | 16,082 | 580 | integration test | header validation, recovery, and ranges |
| `crates/arcweft-lang-sema/src/tests/declarations.rs` | 45,621 | 1,522 | unit test | declaration parse/lower/typecheck coverage |
| `crates/arcweft-lang-sema/src/tests/parser_basics.rs` | 10,919 | 428 | unit test | general surface parser coverage |
| `crates/arcweft-lang-sema/src/tests/support.rs` | 5,686 | 166 | test support | shared typed test imports/helpers |
| `crates/arcweft-compiler/src/tests.rs` | 179,372 | 5,351 | unit test | compiler and Agent-source coverage |

The largest non-generated Rust files remain test suites:

| Path | Bytes | Physical LOC | Role |
|---|---:|---:|---|
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 256,594 | 7,974 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 238,805 | 6,620 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 220,473 | 6,109 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 214,731 | 5,850 | integration test |
| `crates/arcweft-compiler/src/tests.rs` | 179,372 | 5,351 | unit test |

Cargo metadata fan-in / fan-out is 13 / 5 for `arcweft-lang-syntax`, 10 / 10
for `arcweft-lang-sema`, and 4 / 14 for `arcweft-compiler`. No dependency edge
changed. The warning-size syntax aggregators predate this cut; the new public
type remains in the owning AST module and hook parsing was decomposed to a
215-line responsibility module rather than adding another cross-layer helper.

## Verification

- `cargo fmt --all`
- syntax declaration/recovery integration tests: 21 passed
- syntax library tests: 79 passed
- syntax Agent-dialect focused test: 1 passed
- semantic declaration tests: 34 passed
- compiler Agent-source focused test: 1 passed
- changed-crate clippy, all targets and all features, with warnings denied:
  passed
- canonical structural audit: 0 errors, 127 warnings
- `cargo check --workspace --all-targets --all-features` from the main checkout:
  passed, including the rendering targets that consume the repository-local
  Noto Sans JP test asset

There were no parser, semantic, compiler, workspace-check, or Clippy failures
after the final source-range correction. The isolated Jujutsu workspace did not
materialize the ignored font asset, so the workspace-wide command was rerun
successfully from the main checkout before push.
