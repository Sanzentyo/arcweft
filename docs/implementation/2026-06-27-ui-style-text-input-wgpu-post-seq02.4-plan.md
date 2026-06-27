# UI Style, Text Input, and Direct wgpu Plan — post seq02.4

## Scope

This plan records the package generated after recognizing that seq02.4 is being
implemented independently as an AudioGraph resource codec cut.

## Production overlay scope

Included:

- `arcweft-presentation` appearance/focus/text-input data modules;
- `arcweft-ui` program/style/text-source/TextField data modules;
- `arcweft-lang-syntax` style and view AST models;
- `arcweft-render-wgpu` UI scene primitive contract;
- design docs and follow-up requests.

Excluded:

- `arcweft-bundle` UI resource codec implementation;
- `BundleSectionKind::Ui` addition;
- patch v2 fingerprints for UI resources;
- platform IME adapters;
- direct renderer implementation.

## Reason

The attached seq02.4 package explicitly implemented only AudioGraph and marked
UI future/dependent. In this repository, seq02.3/seq02.4 catalog and AudioGraph
resource codec work has since landed, so this package is applied as the
post-seq02.4 presentation/UI substrate and request split. Product UI compact
resources remain in the separate `seq-02.4.1` request so they can consume the
stable UI/style/TextField data model rather than expanding the completed
AudioGraph/catalog cuts.

## Validation

Applied at repository change `ptwpnusl` before description/commit
finalization. Validation run after applying and adapting the package:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-presentation -p arcweft-ui -p arcweft-lang-syntax -p arcweft-render-wgpu --all-targets --all-features
cargo test -p arcweft-presentation -p arcweft-ui -p arcweft-lang-syntax -p arcweft-render-wgpu --all-targets --all-features
cargo clippy -p arcweft-presentation -p arcweft-ui -p arcweft-lang-syntax -p arcweft-render-wgpu --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

All commands passed. Structural audit reported `0` errors and `106`
workspace warning-level findings.

## Structural Audit Notes

Changed Rust file measurements at the validation point:

| Path | Crate | Kind | Bytes | LOC | Embedded test LOC | Responsibilities |
| --- | --- | --- | ---: | ---: | ---: | --- |
| `crates/arcweft-presentation/src/appearance.rs` | `arcweft-presentation` | production plus unit tests | 9,518 | 307 | 36 | presentation environment, system colors, palettes |
| `crates/arcweft-presentation/src/focus.rs` | `arcweft-presentation` | production plus unit tests | 10,461 | 421 | 43 | focus tree, focus lease, text-input session binding |
| `crates/arcweft-presentation/src/text_input.rs` | `arcweft-presentation` | production | 13,270 | 554 | 0 | text input operations, IME composition, snapshots, host commands |
| `crates/arcweft-ui/src/program.rs` | `arcweft-ui` | production | 5,193 | 208 | 0 | retained UI program instruction data |
| `crates/arcweft-ui/src/style_authoring.rs` | `arcweft-ui` | production plus unit tests | 8,032 | 316 | 43 | Arcweft/CSS style authoring, file/embed refs, component overrides |
| `crates/arcweft-ui/src/text_field.rs` | `arcweft-ui` | production plus unit tests | 5,810 | 219 | 26 | TextField/TextArea/SecureField specs and composition visual state |
| `crates/arcweft-ui/src/text_source.rs` | `arcweft-ui` | production | 3,101 | 131 | 0 | unified UI text source table |
| `crates/arcweft-lang-syntax/src/ast/style.rs` | `arcweft-lang-syntax` | production | 1,876 | 89 | 0 | syntax-level style declaration data |
| `crates/arcweft-lang-syntax/src/ast/view.rs` | `arcweft-lang-syntax` | production | 4,111 | 192 | 0 | syntax-level Component View DSL data |
| `crates/arcweft-render-wgpu/src/ui_scene.rs` | `arcweft-render-wgpu` | production plus unit tests | 5,708 | 238 | 36 | renderer-facing direct-wgpu UI scene primitives |
| `crates/arcweft-presentation/src/lib.rs` | `arcweft-presentation` | facade plus existing tests | 11,889 | 475 | 2 | presentation facade exports |
| `crates/arcweft-ui/src/lib.rs` | `arcweft-ui` | facade plus existing tests | 34,313 | 1,021 | 898 | UI facade exports and existing UI tests |
| `crates/arcweft-lang-syntax/src/ast.rs` | `arcweft-lang-syntax` | facade | 209 | 14 | 0 | AST module exports |
| `crates/arcweft-render-wgpu/src/lib.rs` | `arcweft-render-wgpu` | facade | 471 | 16 | 0 | renderer module exports |

No newly added production file crosses the AGENTS.md warning threshold. The
pre-existing `arcweft-ui/src/lib.rs` facade remains just above the 1,000 LOC
warning threshold because it owns a large existing embedded test module; this
cut only adds module exports there and does not expand its responsibilities.
