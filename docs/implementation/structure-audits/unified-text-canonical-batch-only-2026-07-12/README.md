# Unified text canonical prepared-batch structural audit

Audit checkout: Jujutsu working change `ruvumkxk`, based on revision
`309e5aeba99d` (`Remove legacy dialogue renderer staging`). The working commit
ID is content-dependent; the change ID is the stable audit identifier.

Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-canonical-batch-only-2026-07-12
```

Result: 2,635 files scanned, 1,254 Rust files, 620,421 physical Rust LOC,
91 package manifests, 0 errors, and 131 warnings. Generated, vendored,
`target/`, and VCS content are excluded. The CSV files and `violations.md` are
the exact checkout evidence.

## Changed Rust files

These are current file sizes, not diff additions. Embedded test LOC is counted
from a terminal `#[cfg(test)]` module where present. Deleted
`font_family.rs`, `font_system.rs`, and `renderer/tests.rs` are absent from the
current checkout and therefore correctly absent from current-size metrics.

| Path | Owner | Bytes | LOC | Kind | Embedded tests | Major responsibility |
| --- | --- | ---: | ---: | --- | ---: | --- |
| `crates/arcweft-cli/src/app/agent/native/player_observation/capture.rs` | `arcweft-cli` | 11,820 | 293 | production | — | prepared-frame Agent capture projection |
| `crates/arcweft-cli/src/app/agent/native/prepared_text_observation.rs` | `arcweft-cli` | 38,337 | 1,129 | production | — | canonical prepared-text observation orchestration |
| `crates/arcweft-cli/src/app/agent/native/prepared_text_observation/view.rs` | `arcweft-cli` | 15,258 | 426 | production | — | View-owned prepared-text geometry projection |
| `crates/arcweft-cli/src/app/agent/native/runtime_observation.rs` | `arcweft-cli` | 25,927 | 645 | production | — | runtime observation assembly |
| `crates/arcweft-player-native/src/dev_capture.rs` | `arcweft-player-native` | 12,543 | 363 | production | 125 | native shared-frame developer capture and focused tests |
| `crates/arcweft-player-scene/src/frame.rs` | `arcweft-player-scene` | 16,933 | 481 | production | — | player frame orchestration and shared planner ownership |
| `crates/arcweft-player-scene/src/input.rs` | `arcweft-player-scene` | 32,862 | 1,002 | production | — | typed player input state and dispatch |
| `crates/arcweft-player-scene/src/input/pointer.rs` | `arcweft-player-scene` | 15,753 | 409 | production | — | prepared-layout pointer selection and text-input routing |
| `crates/arcweft-player-scene/src/input/tests.rs` | `arcweft-player-scene` | 35,740 | 1,063 | unit-test module | — | input behavior matrix |
| `crates/arcweft-player-scene/tests/action_button_submit.rs` | `arcweft-player-scene` | 13,431 | 355 | integration test | — | action-button activation and canonical text behavior |
| `crates/arcweft-player-scene/tests/runtime_text_controls.rs` | `arcweft-player-scene` | 21,109 | 557 | integration test | — | editable, secure, selection, caret, and IME behavior |
| `crates/arcweft-player-scene/tests/scroll_regions.rs` | `arcweft-player-scene` | 29,050 | 800 | integration test | — | scroll, mapped prepared text, and selection behavior |
| `crates/arcweft-player-scene/tests/textbox_view.rs` | `arcweft-player-scene` | 4,620 | 142 | integration test | — | persistent TextBox/View canonical text path |
| `crates/arcweft-player-web/src/report.rs` | `arcweft-player-web` | 21,508 | 614 | production | — | single canonical Web frame-observation schema |
| `crates/arcweft-player-web/tests/input.rs` | `arcweft-player-web` | 16,976 | 456 | integration test | — | Web input projection |
| `crates/arcweft-player-web/tests/interaction_visual_state.rs` | `arcweft-player-web` | 2,468 | 79 | integration test | — | Web interaction visual state |
| `crates/arcweft-player-web/tests/parity.rs` | `arcweft-player-web` | 34,931 | 1,034 | integration test | — | Native/Web canonical frame parity |
| `crates/arcweft-player-web/tests/support/mod.rs` | `arcweft-player-web` | 457 | 12 | test support | — | deterministic project-font registration |
| `crates/arcweft-render-wgpu/src/geometry.rs` | `arcweft-render-wgpu` | 69,575 | 2,108 | production | — | prepared frame contract, planning, mapping, ownership, and hit geometry |
| `crates/arcweft-render-wgpu/src/geometry/action_buttons.rs` | `arcweft-render-wgpu` | 10,595 | 308 | production | — | action-button geometry, semantics, and prepared label source plan |
| `crates/arcweft-render-wgpu/src/geometry/control_style.rs` | `arcweft-render-wgpu` | 15,415 | 500 | production | — | typed runtime control visuals and effect plans |
| `crates/arcweft-render-wgpu/src/geometry/prepared_text.rs` | `arcweft-render-wgpu` | 4,422 | 126 | production | — | resolved-document preparation helpers |
| `crates/arcweft-render-wgpu/src/geometry/text_controls.rs` | `arcweft-render-wgpu` | 25,754 | 756 | production | — | canonical text-input source planning and interaction geometry |
| `crates/arcweft-render-wgpu/src/lib.rs` | `arcweft-render-wgpu` | 698 | 23 | facade | — | intentional renderer subsystem exports |
| `crates/arcweft-render-wgpu/src/renderer.rs` | `arcweft-render-wgpu` | 52,994 | 1,516 | production | — | shared WGPU frame and filtered-control submission orchestration |
| `crates/arcweft-render-wgpu/src/renderer/prepared_text.rs` | `arcweft-render-wgpu` | 13,926 | 428 | production | — | prepared glyph/effect submission and interaction paint ordering |
| `crates/arcweft-render-wgpu/src/renderer/view_text.rs` | `arcweft-render-wgpu` | 8,146 | 250 | production | — | direct View prepared-text callback |
| `crates/arcweft-render-wgpu/src/text_editor_geometry.rs` | `arcweft-render-wgpu` | 14,169 | 417 | production | 201 | editor geometry derived from canonical `TextLayout` plus focused tests |
| `crates/arcweft-render-wgpu/tests/geometry.rs` | `arcweft-render-wgpu` | 39,162 | 1,139 | integration test | — | frame planning and geometry behavior |
| `crates/arcweft-render-wgpu/tests/geometry_runtime_control_styles.rs` | `arcweft-render-wgpu` | 21,762 | 650 | integration test | — | authored control style and interaction paint plans |
| `crates/arcweft-render-wgpu/tests/prepared_text.rs` | `arcweft-render-wgpu` | 24,134 | 665 | integration test | — | prepared-text contract and local-adapter GPU readbacks |
| `crates/arcweft-render-wgpu/tests/runtime_control_backdrop_gpu_smoke.rs` | `arcweft-render-wgpu` | 11,452 | 331 | integration test | — | filtered control GPU paths |
| `crates/arcweft-render-wgpu/tests/view_scene_player_path.rs` | `arcweft-render-wgpu` | 4,485 | 136 | integration test | — | player-produced View scene submission |
| `crates/arcweft-text-layout/src/config.rs` | `arcweft-text-layout` | 5,460 | 151 | production | — | typed layout request including explicit wrap policy |
| `crates/arcweft-text-layout/src/document_hash.rs` | `arcweft-text-layout` | 8,003 | 230 | production | — | deterministic shaped-layout identity |
| `crates/arcweft-text-layout/src/document_layout.rs` | `arcweft-text-layout` | 27,904 | 788 | production | — | shaped document placement and wrap behavior |
| `crates/arcweft-text-layout/src/lib.rs` | `arcweft-text-layout` | 1,509 | 51 | facade | — | intentional text-layout exports |
| `tools/verify-text-raster-parity.rs` | repository tool | 58,027 | 1,744 | executable tool | — | canonical frame-evidence validation and raster comparison |

The changed production hotspots remain below the 2,500-LOC error threshold.
`geometry.rs` is a warning-level boundary but shrank from 2,196 LOC while the
separate 756-LOC text-control, 308-LOC action-button, and 126-LOC preparation
modules own the algorithms. `renderer.rs` shrank from 1,742 to 1,516 LOC and
delegates prepared submission to its 428-LOC responsibility module. The
parity verifier is a cohesive standalone executable contract; its parsing,
evidence validation, font verification, raster segmentation, and metrics are
not linked into a production crate.

## Largest workspace Rust files

The generated Unicode orientation table is recorded separately and is not an
ownership split candidate.

| Path | Bytes | LOC | Kind | Major responsibility |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | 357,456 | 12,399 | generated data | Unicode vertical-orientation lookup |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 255,668 | 7,948 | integration test | CLI runtime benchmark matrix |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 236,022 | 6,564 | integration test | native vertical observation/capture matrix |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 222,475 | 6,161 | integration test | published JLREQ class-mix fixtures |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 214,791 | 5,854 | integration test | native sample and Fx capture matrix |
| `crates/arcweft-compiler/src/tests.rs` | 179,347 | 5,350 | unit-test module | compiler behavior matrix |
| `crates/arcweft-cli/tests/check/agent_script_debug.rs` | 195,837 | 5,250 | integration test | Agent script/debug scenarios |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs` | 143,360 | 4,181 | integration test | published JLREQ unit fixtures |
| `crates/arcweft-lang-sema/src/tests/typecheck.rs` | 125,758 | 4,135 | unit-test module | semantic type-check matrix |
| `crates/arcweft-cli/src/app/agent/native/tests.rs` | 109,063 | 3,138 | unit-test module | native Agent adapter behavior |
| `crates/arcweft-core/src/tests/flow.rs` | 88,953 | 2,553 | unit-test module | core flow execution matrix |

The largest integration test remains below the 8,000-LOC error threshold.
Largest non-generated production files are:

| Path | Bytes | LOC | Embedded tests | Major responsibility |
| --- | ---: | ---: | ---: | --- |
| `crates/arcweft-core/src/value.rs` | 84,017 | 2,500 | — | typed runtime values and deterministic codecs |
| `crates/arcweft-core/src/engine/eval/calls.rs` | 89,488 | 2,481 | — | core call evaluation |
| `crates/arcweft-lang-sema/src/checker/expr.rs` | 94,248 | 2,469 | — | expression semantic analysis |
| `crates/arcweft-cli/src/toolchain_profile.rs` | 75,712 | 2,463 | 296 | toolchain profiles and validation tests |
| `crates/arcweft-lang-sema/src/checker.rs` | 85,502 | 2,456 | — | semantic checker orchestration |
| `crates/arcweft-core/src/awbc/product_step.rs` | 93,512 | 2,430 | — | product-step execution and validation |
| `crates/arcweft-runtime-driver/src/session.rs` | 93,063 | 2,408 | — | runtime session orchestration |
| `crates/arcweft-bundle/src/container.rs` | 78,267 | 2,389 | 662 | bundle container codec and focused tests |
| `crates/arcweft-cli/src/app/debug.rs` | 77,792 | 2,376 | 70 | CLI debug workflows and tests |
| `crates/arcweft-runtime-plan/src/expr.rs` | 83,504 | 2,363 | — | runtime expression lowering |
| `crates/arcweft-bundle/src/resource_codec/view/model.rs` | 74,341 | 2,358 | 42 | typed View resource model and codec fixtures |
| `crates/arcweft-runtime-accelerator/src/math.rs` | 86,092 | 2,340 | — | accelerator numeric operations |
| `crates/arcweft-cli/src/app/bundle.rs` | 83,263 | 2,325 | — | bundle compilation orchestration |
| `crates/arcweft-cli/src/app/project_commands.rs` | 82,506 | 2,296 | — | project command workflows |
| `crates/arcweft-lang-syntax/src/expr.rs` | 68,851 | 2,243 | 53 | expression syntax model/parser helpers and tests |

No production file exceeds the 2,500-LOC error threshold.

## Dependency and boundary review

No Cargo manifest, feature, or workspace dependency edge changed. Exact
unique-crate fan-in/fan-out at this checkout is:

| Crate | Fan-in | Fan-out |
| --- | ---: | ---: |
| `arcweft-render-wgpu` | 6 | 15 |
| `arcweft-text-layout` | 5 | 7 |
| `arcweft-player-scene` | 3 | 18 |
| `arcweft-player-web` | 0 | 28 |
| `arcweft-player-native` | 1 | 33 |
| `arcweft-cli` | 0 | 65 |

The public boundary now flows from `ResolvedTextDocument` and typed layout
configuration into the lower-level shared layout/Glyphon engine, then to one
prepared frame consumed by player, renderer, Web, Native, and Agent adapters.
No adapter owns shaping, a second font system, or a platform-specific text
formula. The private pre-shaping control plans do not cross the renderer
boundary. No source gate, compatibility alias, migration reader, unsafe code,
or new dependency direction was introduced.
