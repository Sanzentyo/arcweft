# Unified Text / View definition structural audit

Audit point: Jujutsu working change `tqkzspus`, parent `dc2cd458`.

Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-view-definitions-2026-07-12
```

The checkout contains 1,250 Rust files / 627,340 physical Rust LOC. The audit
reports 0 errors and 143 pre-existing or tracked warnings. No Cargo dependency,
feature, facade-crate, or crate-direction edge changed in this slice; the full
edge inventory is in `dependency_edges.csv`.

## Changed Rust files

| Path | Owning crate | Bytes | Physical LOC | Kind | Responsibility |
|---|---:|---:|---:|---|---|
| `crates/arcweft-bundle/src/resource_codec/view/codec.rs` | `arcweft-bundle` | 61,765 | 1,751 | production | bounded canonical View codec and cross-record validation |
| `crates/arcweft-bundle/src/resource_codec/view/model.rs` | `arcweft-bundle` | 73,210 | 2,331 | production | typed View product-resource model |
| `crates/arcweft-bundle/src/resource_codec.rs` | `arcweft-bundle` | 3,946 | 75 | facade | intentional resource-codec exports |
| `crates/arcweft-bundle/tests/view_action_button_resources.rs` | `arcweft-bundle` | 4,365 | 115 | integration test | action-button resource behavior |
| `crates/arcweft-bundle/tests/view_focus_navigation_resources.rs` | `arcweft-bundle` | 3,217 | 82 | integration test | focus resource behavior |
| `crates/arcweft-bundle/tests/view_resource_codecs.rs` | `arcweft-bundle` | 31,589 | 856 | integration test | View codec round-trip, budget, call, and type invariants |
| `crates/arcweft-bundle/tests/view_runtime_text_controls.rs` | `arcweft-bundle` | 9,735 | 281 | integration test | runtime text-control resource projection |
| `crates/arcweft-cli/src/app/bundle/tests.rs` | `arcweft-cli` | 61,912 | 2,013 | unit-test module | bundle lowering and assembly behavior |
| `crates/arcweft-cli/src/app/bundle/view_merge.rs` | `arcweft-cli` | 8,375 | 226 | production | deterministic View inventory rebasing and merge |
| `crates/arcweft-cli/src/app/bundle/view_mounts.rs` | `arcweft-cli` | 16,721 | 457 | production | mounted-root discovery and nested-View reachability |
| `crates/arcweft-cli/src/app/bundle_view/lowering.rs` | `arcweft-cli` | 39,428 | 1,030 | production | typed View declaration/instruction/resource lowering |
| `crates/arcweft-cli/src/app/bundle_view.rs` | `arcweft-cli` | 274 | 8 | facade | narrow View-lowering exports |
| `crates/arcweft-cli/src/app/bundle_view_schema.rs` | `arcweft-cli` | 25,318 | 659 | production | typed reactive View value-program compiler |
| `crates/arcweft-lang-syntax/src/ast/view.rs` | `arcweft-lang-syntax` | 43,265 | 1,633 | production | View AST and owned structural queries |
| `crates/arcweft-lang-syntax/src/parser/view.rs` | `arcweft-lang-syntax` | 51,211 | 1,521 | production | retained View grammar, recovery, and module scoping |
| `crates/arcweft-lang-syntax/tests/style_view.rs` | `arcweft-lang-syntax` | 19,809 | 753 | integration test | View/style surface grammar behavior |
| `crates/arcweft-player-web/src/inset_shadow_exact_capture.rs` | `arcweft-player-web` | 22,273 | 640 | production | browser WebGPU exact-capture fixture adapter |
| `crates/arcweft-player-web/tests/parity.rs` | `arcweft-player-web` | 34,213 | 999 | integration test | native/Web presentation parity fixtures |
| `crates/arcweft-runtime-driver/tests/session.rs` | `arcweft-runtime-driver` | 73,571 | 2,014 | integration test | session/save/runtime resource behavior |

The warning-level production files above were already multi-subsystem
boundaries. This slice adds fewer than 300 physical lines to each and keeps new
logic in owned responsibilities: call reachability is in `view_mounts.rs`,
inventory rebasing is in `view_merge.rs`, and call lowering is separated from
the main expression dispatcher. The 1,030-LOC lowering module remains below
the 1,200-LOC production warning threshold. The larger model, codec, syntax AST,
and View parser remain tracked decomposition candidates; no new responsibility
was duplicated across them.

## Largest workspace Rust files

| Path | Bytes | Physical LOC | Kind |
|---|---:|---:|---|
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | 357,456 | 12,399 | generated Unicode data |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 255,668 | 7,948 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 236,015 | 6,564 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 222,475 | 6,161 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 214,665 | 5,848 | integration test |
| `crates/arcweft-compiler/src/tests.rs` | 179,347 | 5,350 | unit-test module |
| `crates/arcweft-cli/tests/check/agent_script_debug.rs` | 195,837 | 5,250 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs` | 143,360 | 4,181 | integration test |
| `crates/arcweft-lang-sema/src/tests/typecheck.rs` | 125,758 | 4,135 | unit-test module |
| `crates/arcweft-render-native/src/tests.rs` | 142,252 | 4,096 | unit-test module |

The first file is generated lookup data and is marked generated by the audit;
it is not an ownership split candidate. The remaining largest files are
existing test inventories below the 8,000-LOC integration-test error threshold.
No error-level size exception is introduced by this change.
