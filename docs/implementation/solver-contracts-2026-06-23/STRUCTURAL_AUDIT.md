# Structural audit: solver-backed contracts

## Trigger

This change materially expands the public verifier IR and adapter contract, and
splits SMT ownership out of an oversized verifier `lib.rs`.

## Result

- `arcweft-verify/src/smt.rs`: canonical typed SMT IR and deterministic emitter.
- `arcweft-verify/src/contract_smt.rs`: HIR contract lowering only.
- `arcweft-verify-oxiz`: OxiZ term execution/model adapter only.
- `arcweft-verify-z3`: external-process execution/model adapter only.
- `arcweft-lang-syntax`: original `ContractClause` and `Pattern` impls own
  behavior needed by downstream verification.
- no new workspace crate;
- `arcweft-verify-oxiz` depends directly on `oxiz-core` so it can build OxiZ
  terms without routing through SMT-LIB parsing;
- no syntax -> verifier dependency inversion;
- no concrete solver dependency in the Sans-I/O verifier core;
- no parallel SMT expression enum or duplicate backend trait.

## Current measurements

Revision measured: working copy `nztwymqn` on parent `acebcb33`.

Changed Rust files:

| Path | Owning crate | Bytes | LOC | Kind | Embedded test LOC | Main responsibilities |
| --- | --- | ---: | ---: | --- | ---: | --- |
| `crates/arcweft-cli/src/app/verify.rs` | `arcweft-cli` | 16843 | 482 | production | 0 | CLI policy, backend selection, SMT artifact I/O |
| `crates/arcweft-lang-sema/src/checker/effects.rs` | `arcweft-lang-sema` | 4221 | 121 | production | 0 | contract expression typecheck environment |
| `crates/arcweft-lang-sema/src/checker/expr.rs` | `arcweft-lang-sema` | 92558 | 2461 | production | 0 | expression typechecking, scalar method typing |
| `crates/arcweft-lang-sema/src/checker/module.rs` | `arcweft-lang-sema` | 48067 | 1262 | production | 0 | module/function typechecking orchestration |
| `crates/arcweft-lang-syntax/src/ast/flow.rs` | `arcweft-lang-syntax` | 22287 | 983 | production | 0 | flow AST and contract clause behavior |
| `crates/arcweft-lang-syntax/src/ast/pattern.rs` | `arcweft-lang-syntax` | 1977 | 85 | production | 0 | pattern AST accessors |
| `crates/arcweft-verify/src/contract_smt.rs` | `arcweft-verify` | 24440 | 672 | production with unit tests | 97 | HIR contract-to-SMT lowering |
| `crates/arcweft-verify/src/lib.rs` | `arcweft-verify` | 54530 | 1585 | production facade/orchestration | 2 | verifier report model and semantic scan orchestration |
| `crates/arcweft-verify/src/smt.rs` | `arcweft-verify` | 27737 | 895 | production with unit tests | 56 | typed SMT IR, validation, emission, outcome parsing |
| `crates/arcweft-verify/src/tests.rs` | `arcweft-verify` | 10847 | 369 | unit test module | 0 | verifier report regression tests |
| `crates/arcweft-verify-oxiz/src/lib.rs` | `arcweft-verify-oxiz` | 11325 | 313 | production with unit tests | 106 | OxiZ term adapter and model collection |
| `crates/arcweft-verify-z3/src/lib.rs` | `arcweft-verify-z3` | 11556 | 343 | production with unit tests | 24 | external Z3 process adapter and model parsing |

Largest current Rust files:

| Path | Bytes | LOC | Kind |
| --- | ---: | ---: | --- |
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | 357456 | 12399 | generated lookup-heavy production |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 255424 | 7945 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 225209 | 6282 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 222475 | 6161 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 209852 | 5651 | integration test |
| `crates/arcweft-cli/tests/check/agent_script_debug.rs` | 195828 | 5250 | integration test |
| `crates/arcweft-render-native/src/tests.rs` | 153634 | 4395 | unit/integration-style test module |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs` | 143360 | 4181 | integration test |
| `crates/arcweft-cli/src/app/agent/native/tests.rs` | 139939 | 4056 | unit/integration-style test module |
| `crates/arcweft-core/src/value.rs` | 83039 | 2483 | production |
| `crates/arcweft-runtime-plan/src/flow.rs` | 89343 | 2472 | production |
| `crates/arcweft-cli/src/toolchain_profile.rs` | 75712 | 2463 | production |

Warnings requiring future ownership review:

- `crates/arcweft-verify/src/lib.rs` remains over the 1,000 LOC `lib.rs`
  warning threshold after extracting SMT IR and contract lowering. It should be
  decomposed further around verifier report types and semantic scan ownership.
- `crates/arcweft-lang-sema/src/checker/expr.rs` and
  `crates/arcweft-lang-sema/src/checker/module.rs` were touched lightly but are
  already over production warning thresholds; this change did not broaden their
  ownership beyond scalar contract typechecking support.
- Several existing integration-test hot spots remain warning-level large but
  were not modified by this task.

Automated audit:

```bash
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root .
```

Result: 1,355 files scanned, 751 Rust files, 362,889 Rust physical LOC,
0 errors, 88 warnings.
