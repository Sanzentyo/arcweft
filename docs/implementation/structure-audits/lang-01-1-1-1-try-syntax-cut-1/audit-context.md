# Lang-01.1.1.1 Try syntax Cut 1 structural audit

## Scope

- Jujutsu change: `uulltkts` on parent `579c2fd5`.
- Command: `cargo +nightly -Zscript tools/structure-audit.rs --root .`.
- Result: 3,492 files, 1,818 Rust files, 845,201 Rust physical LOC,
  94 package manifests, 0 errors, and 137 warnings.
- The generated full-workspace evidence is in `file_metrics.csv`,
  `dependency_edges.csv`, `public_type_duplicates.csv`, and `violations.md`.

This cut changes no Cargo dependency or feature edge. The syntax crate remains
the sole owner of the Try AST and its exact authored ranges. HIR, semantic,
runtime-plan, verifier, Agent REPL, and CLI files only migrate exhaustive
consumers to the owned accessor; they do not duplicate the source model.

## Changed Rust files

The table records current checkout values, not diff additions.

| Path | Bytes | Physical LOC | Classification | Embedded tests |
|---|---:|---:|---|---|
| `crates/arcweft-agent-repl/src/binding.rs` | 11813 | 364 | production | false |
| `crates/arcweft-cli/src/app/agent/native/repl_snapshot.rs` | 5812 | 172 | production | false |
| `crates/arcweft-lang-sema/src/checker/expr/partial.rs` | 17210 | 468 | production | false |
| `crates/arcweft-lang-sema/src/checker/expr/support.rs` | 12921 | 418 | production | false |
| `crates/arcweft-lang-sema/src/checker/expr.rs` | 90003 | 2342 | production | false |
| `crates/arcweft-lang-sema/src/fact_layer.rs` | 12010 | 383 | production | false |
| `crates/arcweft-lang-sema/src/project_index/entities.rs` | 39150 | 1105 | production | false |
| `crates/arcweft-lang-sema/src/project_index/flow_control.rs` | 16312 | 464 | production | false |
| `crates/arcweft-lang-sema/src/project_index/relations.rs` | 42540 | 1160 | production | false |
| `crates/arcweft-lang-sema/src/semantic/traversal.rs` | 29839 | 806 | production | false |
| `crates/arcweft-lang-sema/src/semantic.rs` | 78985 | 2120 | production | false |
| `crates/arcweft-lang-sema/src/signature/surface.rs` | 36227 | 987 | production | false |
| `crates/arcweft-lang-sema/src/style/token_graph.rs` | 8593 | 245 | production | false |
| `crates/arcweft-lang-sema/src/symbols.rs` | 34092 | 1004 | production | false |
| `crates/arcweft-lang-sema/src/tests/control_flow.rs` | 41187 | 1406 | test | false |
| `crates/arcweft-lang-sema/src/tests/declarations.rs` | 41229 | 1357 | test | false |
| `crates/arcweft-lang-sema/src/tests/expressions.rs` | 9056 | 248 | test | false |
| `crates/arcweft-lang-sema/src/tests/line_plan.rs` | 27574 | 934 | test | false |
| `crates/arcweft-lang-syntax/src/ast/view/recovery.rs` | 24781 | 595 | production | false |
| `crates/arcweft-lang-syntax/src/ast/view.rs` | 50489 | 1843 | production | false |
| `crates/arcweft-lang-syntax/src/expr/pipe_scope.rs` | 10488 | 261 | production | true |
| `crates/arcweft-lang-syntax/src/expr/pratt.rs` | 27530 | 765 | production | false |
| `crates/arcweft-lang-syntax/src/expr/prefix.rs` | 11211 | 284 | production | false |
| `crates/arcweft-lang-syntax/src/expr/source_ranges.rs` | 41240 | 1177 | production | true |
| `crates/arcweft-lang-syntax/src/expr/tests.rs` | 21798 | 598 | test | false |
| `crates/arcweft-lang-syntax/src/expr.rs` | 49630 | 1765 | production | false |
| `crates/arcweft-lang-syntax/src/parser/dialogue.rs` | 33864 | 859 | production | true |
| `crates/arcweft-lang-syntax/src/parser/expression.rs` | 19245 | 595 | production | false |
| `crates/arcweft-lang-syntax/src/parser/helpers.rs` | 33608 | 996 | production | true |
| `crates/arcweft-runtime-plan/src/expr/desugar.rs` | 2835 | 72 | production | false |
| `crates/arcweft-runtime-plan/src/expr/tests.rs` | 33053 | 971 | test | false |
| `crates/arcweft-runtime-plan/src/expr.rs` | 84541 | 2382 | production | false |
| `crates/arcweft-runtime-plan/src/flow/syntax_helpers.rs` | 3233 | 104 | production | false |
| `crates/arcweft-runtime-plan/src/function_values.rs` | 28998 | 872 | production | false |
| `crates/arcweft-runtime-plan/src/host_request.rs` | 24153 | 647 | production | true |
| `crates/arcweft-runtime-plan/src/line_task.rs` | 30287 | 825 | production | false |
| `crates/arcweft-verify/src/lib.rs` | 67274 | 1958 | production | false |

The major responsibilities are: syntax model and exact source ownership;
strict and private lossless parsing; expression range traversal; semantic and
project-index visitation; runtime/verifier traversal; and focused fixtures.
No manual field projection, new dependency fan-out, or duplicate public Try
type was introduced.

## Current largest non-generated production Rust files

| Path | Bytes | Physical LOC | Embedded tests |
|---|---:|---:|---|
| `crates/arcweft-core/src/engine/eval/calls.rs` | 89488 | 2481 | false |
| `crates/arcweft-core/src/value.rs` | 83366 | 2465 | false |
| `crates/arcweft-cli/src/toolchain_profile.rs` | 75712 | 2463 | true |
| `crates/arcweft-bundle/src/container.rs` | 78366 | 2393 | true |
| `crates/arcweft-runtime-plan/src/expr.rs` | 84541 | 2382 | false |
| `crates/arcweft-cli/src/app/debug.rs` | 77792 | 2376 | true |
| `crates/arcweft-lang-sema/src/checker/expr.rs` | 90003 | 2342 | false |
| `crates/arcweft-runtime-accelerator/src/math.rs` | 86092 | 2340 | false |
| `crates/arcweft-bundle/src/lib.rs` | 84752 | 2313 | true |
| `crates/arcweft-lang-sema/src/checker/module.rs` | 85967 | 2287 | false |
| `crates/arcweft-cli/src/app/project_commands.rs` | 82523 | 2281 | false |
| `crates/arcweft-runtime-plan/src/awbc_lower/flow.rs` | 81431 | 2194 | false |

The changed `checker/expr.rs`, `runtime-plan/expr.rs`, syntax `expr.rs`, View
AST, verifier root, and semantic root remain warning-level hotspots. This cut
adds only exhaustive enum migration or the cohesive Try source model to those
owners; it does not add a second responsibility. Their existing decomposition
work remains repository-wide maintenance and is not hidden by this audit.
