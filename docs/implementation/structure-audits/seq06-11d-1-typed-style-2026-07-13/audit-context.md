# Seq06.11d.1 structural audit context

- Jujutsu change ID: `smtzskpqkywtpwwqwpypnxlltuwruvnm`
- Parent revision: `4204d25965129ced50abe82cf5de67d528b483d0`
- Files scanned: 2,681
- Rust files: 1,278
- Rust physical LOC: 623,296
- Cargo package manifests: 91
- Result: 0 errors, 126 warnings

`file_metrics.csv` is the exhaustive current-checkout measurement for changed
Rust files and the largest workspace Rust files: path, exact bytes, physical
LOC, code LOC, production/test/generated classification, and embedded-test
presence. `dependency_edges.csv` is the structured fan-in/fan-out source.

Relevant normal-dependency fan-in / fan-out counts:

| Package | Fan-in | Fan-out |
| --- | ---: | ---: |
| `arcweft-lang-syntax` | 9 | 5 |
| `arcweft-lang-hir` | 8 | 3 |
| `arcweft-lang-sema` | 6 | 9 |
| `arcweft-view` | 9 | 6 |
| `arcweft-presentation` | 18 | 5 |

Largest non-generated Rust files at this checkout:

| Path | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 255,217 | 7,935 | test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 238,805 | 6,620 | test |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 220,473 | 6,109 | test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 214,731 | 5,850 | test |
| `crates/arcweft-compiler/src/tests.rs` | 179,339 | 5,350 | test |
| `crates/arcweft-cli/tests/check/agent_script_debug.rs` | 195,821 | 5,249 | test |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs` | 143,206 | 4,177 | test |
| `crates/arcweft-lang-sema/src/tests/typecheck.rs` | 125,373 | 4,120 | test |
| `crates/arcweft-cli/src/app/agent/native/tests.rs` | 110,752 | 3,181 | test |
| `crates/arcweft-core/src/tests/flow.rs` | 88,953 | 2,553 | test |
| `crates/arcweft-core/src/value.rs` | 84,017 | 2,500 | production |
| `crates/arcweft-core/src/engine/eval/calls.rs` | 89,488 | 2,481 | production |

Changed production files with embedded test-gated code have these physical
test-only LOC totals:

| Path | Embedded test LOC | Form |
| --- | ---: | --- |
| `crates/arcweft-cli/src/app/bundle.rs` | 7 | test-only helper; external test module declaration is 0 LOC |
| `crates/arcweft-lang-hir/src/lower.rs` | 134 | inline test module |
| `crates/arcweft-presentation/src/appearance.rs` | 36 | inline test module |
| `crates/arcweft-verify/src/runtime_type.rs` | 199 | test-only imports/helper plus inline module |
| `crates/arcweft-view/src/program.rs` | 75 | inline test module |
| `crates/arcweft-lang-sema/src/lib.rs` | 0 | external test module declaration |
| `crates/arcweft-lang-sema/src/project_index.rs` | 0 | external test module declaration |
| `crates/arcweft-view/src/lib.rs` | 0 | external test module declaration |

The changed `arcweft-cli/src/app/bundle.rs` hotspot is warning-level and owns
provisional Style conversion only until d.2. The package explicitly requires
that cut to remove the `dsl_view_style_*` block and move checked-catalog
lowering into `arcweft-compiler`; retaining or further expanding the block is
not an accepted exception.
