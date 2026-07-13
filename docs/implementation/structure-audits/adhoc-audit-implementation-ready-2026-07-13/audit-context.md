# Ad-hoc audit implementation-ready structural audit context

- Jujutsu change ID: `ysvwwopyyxmwmkrskuyxvlkxsvkmxooq`
- Parent revision: `3e08bba900ef37bbc06aea0468ceeb5fd07271c3`
- Files scanned: 2,687
- Rust files: 1,282
- Rust physical LOC: 625,380
- Cargo package manifests: 91
- Result: 0 errors, 126 warnings

`file_metrics.csv` is the exhaustive current-checkout measurement for changed
Rust files and the largest workspace Rust files: owning path, exact bytes,
physical LOC, code LOC, production/test/generated classification, and embedded
test presence. `dependency_edges.csv` is the structured fan-in/fan-out source.

Relevant normal-dependency fan-in / fan-out counts:

| Package | Fan-in | Fan-out |
| --- | ---: | ---: |
| `arcweft-bundle` | 10 | 21 |
| `arcweft-dialogue` | 4 | 4 |
| `arcweft-glyphon` | 3 | 7 |
| `arcweft-lang-syntax` | 9 | 5 |
| `arcweft-tooling` | 4 | 4 |
| `arcweft-verify-lsp` | 1 | 7 |
| `arcweft-lsp` | 0 | 25 |
| `arcweft-player-scene` | 3 | 16 |
| `arcweft-player-native` | 1 | 30 |
| `arcweft-player-web` | 0 | 17 |
| `arcweft-render-wgpu` | 5 | 13 |
| `arcweft-runtime-plan` | 3 | 8 |

Changed production hotspots at or above 1,000 physical LOC:

| Owning crate / path | Bytes | Physical LOC | Embedded test LOC | Major responsibilities |
| --- | ---: | ---: | ---: | --- |
| `arcweft-bundle/src/lib.rs` | 77,458 | 2,164 | 728 | bundle data model, cross-resource validation, codec errors, unit tests |
| `arcweft-player-native/src/scene_windowed.rs` | 63,931 | 1,732 | 54 | native window event adaptation, input, frame preparation |
| `arcweft-verify-lsp/src/lib.rs` | 59,146 | 1,590 | 667 | verifier/LSP projection and direct tests |
| `arcweft-render-wgpu/src/view_compositor.rs` | 55,987 | 1,560 | 0 | View compositor orchestration and checked render propagation |
| `arcweft-lsp/src/features/actions.rs` | 48,649 | 1,491 | 261 | LSP action conversion and protocol-facing tests |
| `arcweft-bundle/src/product.rs` | 41,941 | 1,145 | 428 | AWFB product encoding/decoding and negative codec tests |
| `arcweft-lang-syntax/src/text.rs` | 39,316 | 1,134 | 237 | RichText tokenization, builtin classification, syntax tests |
| `arcweft-lang-syntax/src/ast/dialogue.rs` | 31,285 | 1,129 | 0 | typed dialogue AST and parser-owned speaker surface ranges |
| `arcweft-player-scene/src/input.rs` | 34,357 | 1,042 | 0 | host-neutral input controller and responsibility modules |
| `arcweft-lang-syntax/src/parser/helpers.rs` | 33,720 | 1,001 | 75 | parser recovery/range helpers and focused tests |

The changed `arcweft-runtime-plan/src/render_text/tests.rs` is a test file at
55,294 bytes / 1,767 physical LOC. The changed
`arcweft-tooling/src/tests.rs` is a test file at 35,149 bytes / 1,072 physical
LOC. Both remain below the 2,500-LOC integration-test warning threshold.

New responsibility modules stay in the preferred size range or below it:

| Owning crate / path | Bytes | Physical LOC | Classification | Responsibility |
| --- | ---: | ---: | --- | --- |
| `arcweft-glyphon/src/physical_bounds.rs` | 11,591 | 357 | production + 144 embedded test LOC | checked logical-to-physical text bounds |
| `arcweft-player-scene/src/input/wheel.rs` | 8,906 | 263 | production + 74 embedded test LOC | shared wheel units and normalization policy |
| `arcweft-lang-syntax/src/cst/path.rs` | 2,487 | 78 | production | lossless typed path-root ranges |
| `arcweft-tooling/src/path_sugar.rs` | 972 | 30 | production | CST path alias edit inventory |

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

No new dependency cycle, error-level size violation, source gate, or layer
inversion was introduced. Existing warning-level hotspots are not expanded into
new responsibilities by this cut: the new numeric and wheel policies live in
focused owner modules, RichText metadata moves downward to `arcweft-dialogue`,
and tooling consumes syntax-owned ranges rather than adding another parser.
