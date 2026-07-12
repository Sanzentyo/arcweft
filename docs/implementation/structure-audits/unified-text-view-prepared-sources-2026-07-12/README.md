# Typed View text preparation structural review

Audit checkout: Jujutsu change `xuxwrolxnzxx`, based on commit
`4f82638f267e` (`Execute per-mount View runtime`). The working commit ID is
content-dependent; the change ID is the stable audit identifier.

Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-view-prepared-sources-2026-07-12
```

Result: 2,632 files scanned, 1,258 Rust files, 633,404 physical Rust LOC,
92 package manifests, 0 errors, and 142 warnings. Generated, vendored,
`target/`, and VCS content are excluded by the audit script.

## Changed Rust files

The measurements below are current file sizes, not diff additions. Embedded
test LOC is counted from the file's terminal `#[cfg(test)]` module when present.

| Path | Bytes | LOC | Kind | Embedded tests | Major responsibility |
| --- | ---: | ---: | --- | ---: | --- |
| `crates/arcweft-bundle/src/resource_codec/view/codec.rs` | 66,755 | 1,871 | production | — | canonical View section encoding, budgets, and reference validation |
| `crates/arcweft-bundle/src/resource_codec/view/model.rs` | 74,341 | 2,358 | production | 42 | typed View program/text/style resource records |
| `crates/arcweft-bundle/src/resource_codec/view/runtime_control_style.rs` | 70,983 | 2,054 | production | — | authored-to-runtime control and text style resolution |
| `crates/arcweft-bundle/src/resource_codec.rs` | 4,026 | 76 | production facade | — | intentional resource-codec module exports |
| `crates/arcweft-bundle/tests/runtime_control_style_resolution.rs` | 32,905 | 953 | integration test | — | runtime style cascade behavior |
| `crates/arcweft-bundle/tests/view_resource_codecs.rs` | 33,287 | 900 | integration test | — | canonical View resource round trips and invalid references |
| `crates/arcweft-bundle/tests/view_runtime_text_controls.rs` | 10,007 | 289 | integration test | — | runtime text target/style records |
| `crates/arcweft-cli/src/app/bundle/tests.rs` | 65,369 | 2,111 | unit test module | — | bundle compilation and sidecar integration |
| `crates/arcweft-cli/src/app/bundle.rs` | 83,263 | 2,325 | production | — | bundle orchestration and default localization hydration |
| `crates/arcweft-cli/src/app/bundle_view/lowering/content.rs` | 13,510 | 370 | production | — | View Text/Image content lowering and concrete target allocation |
| `crates/arcweft-cli/src/app/bundle_view/lowering.rs` | 39,999 | 1,042 | production | — | View declaration/program lowering orchestration |
| `crates/arcweft-player-scene/src/frame/surfaces.rs` | 25,531 | 730 | production | 202 | exact mounted View Element/Text/Image/child painter expansion |
| `crates/arcweft-player-scene/src/frame/view_text.rs` | 12,568 | 344 | production | — | typed View source resolution into canonical prepared text |
| `crates/arcweft-player-scene/src/frame.rs` | 19,436 | 553 | production | — | shared player frame orchestration |
| `crates/arcweft-player-scene/tests/scroll_regions.rs` | 29,148 | 805 | integration test | — | prepared View text, scroll, selection, vertical/ruby behavior |
| `crates/arcweft-player-web/tests/parity.rs` | 35,558 | 1,038 | integration test | — | native/web observation, executable View fixture, and runtime parity |
| `crates/arcweft-render-text/src/resolved_document.rs` | 35,900 | 1,178 | production | — | canonical static/display RichText document resolution |
| `crates/arcweft-render-wgpu/src/geometry/prepared_text.rs` | 5,655 | 155 | production | — | legacy block and resolved-document preparation boundary |
| `crates/arcweft-render-wgpu/src/geometry.rs` | 71,812 | 2,163 | production | — | prepared frame data, mapping, and shared planning context |
| `crates/arcweft-render-wgpu/src/view_direct_renderer.rs` | 34,917 | 1,049 | production | — | direct View primitive GPU submission, including cropped image UVs |
| `crates/arcweft-render-wgpu/src/view_scene/core.rs` | 17,786 | 635 | production | 113 | renderer-neutral View primitives and painter scene |
| `crates/arcweft-render-wgpu/src/view_scene.rs` | 1,838 | 34 | production facade | — | intentional View scene exports |
| `crates/arcweft-runtime-driver/src/display.rs` | 56,030 | 1,485 | production | 691 | presentation snapshots and display resource reconciliation |
| `crates/arcweft-runtime-driver/src/presentation_handles.rs` | 39,593 | 1,175 | production | 327 | persistent presentation-handle resource ownership |
| `crates/arcweft-runtime-driver/src/session.rs` | 91,984 | 2,384 | production | — | session lifecycle, frame projection, save/load, and hot swap |
| `crates/arcweft-runtime-driver/src/view_projection.rs` | 11,159 | 325 | production | — | mount-scoped executable View resource projection |
| `crates/arcweft-runtime-driver/src/view_runtime/evaluator/text.rs` | 9,030 | 246 | production | — | exact typed View text-store lookup and diagnostics |
| `crates/arcweft-runtime-driver/src/view_runtime/evaluator.rs` | 49,453 | 1,210 | production | — | bounded View control-flow evaluation and paint IR emission |
| `crates/arcweft-runtime-driver/src/view_runtime.rs` | 28,138 | 755 | production | — | retained View runtime, snapshots, typed frame output, diagnostics |
| `crates/arcweft-runtime-driver/tests/session.rs` | 88,264 | 2,428 | integration test | — | session projection/save/restore behavior |
| `crates/arcweft-runtime-driver/tests/view_runtime.rs` | 24,989 | 723 | integration test | — | typed stores, diagnostics, mount persistence, exact paint order |
| `crates/arcweft-view/src/view.rs` | 5,764 | 187 | production | — | mount identity, allocator, and retained mount state |

The new `view_text.rs` responsibility module is 344 LOC and the expanded
`surfaces.rs` module is 730 LOC, both inside the preferred 300–800 LOC range.
The evaluator is 1,210 LOC, ten lines above the warning threshold; its typed
text lookup is already split into a 246-line child module. A further split is
appropriate when another instruction family is added, but moving a few match
arms solely to cross the threshold would make the bounded interpreter harder
to audit. Existing warning-level codec, geometry, display, session, and CLI
files predate this slice; this change removes the text-block projections from
them and does not add a new mixed platform/I/O responsibility.

## Largest workspace Rust files

Largest non-generated Rust files, including tests:

| Path | Bytes | LOC | Kind | Major responsibility |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 255,668 | 7,948 | integration test | CLI runtime benchmark matrix |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 236,015 | 6,564 | integration test | native vertical-text observation/capture cases |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 222,475 | 6,161 | integration test | published JLREQ class-mix fixtures |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 214,665 | 5,848 | integration test | native sample/effect capture matrix |
| `crates/arcweft-compiler/src/tests.rs` | 179,347 | 5,350 | unit test module | compiler behavior matrix |
| `crates/arcweft-cli/tests/check/agent_script_debug.rs` | 195,837 | 5,250 | integration test | Agent script/debug CLI scenarios |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs` | 143,360 | 4,181 | integration test | published JLREQ unit fixtures |
| `crates/arcweft-lang-sema/src/tests/typecheck.rs` | 125,758 | 4,135 | unit test module | semantic type-check matrix |
| `crates/arcweft-render-native/src/tests.rs` | 142,252 | 4,096 | unit test module | legacy native renderer behavior |
| `crates/arcweft-cli/src/app/agent/native/tests.rs` | 127,169 | 3,705 | unit test module | native Agent adapter behavior |

Largest non-generated production Rust files:

| Path | Bytes | LOC | Major responsibility |
| --- | ---: | ---: | --- |
| `crates/arcweft-core/src/value.rs` | 84,017 | 2,500 | typed runtime values and deterministic codecs |
| `crates/arcweft-core/src/engine/eval/calls.rs` | 89,488 | 2,481 | core call evaluation |
| `crates/arcweft-lang-sema/src/checker/expr.rs` | 94,248 | 2,469 | expression semantic analysis |
| `crates/arcweft-cli/src/toolchain_profile.rs` | 75,712 | 2,463 | toolchain profile model plus embedded tests |
| `crates/arcweft-lang-sema/src/checker.rs` | 85,502 | 2,456 | semantic checker orchestration |
| `crates/arcweft-core/src/awbc/product_step.rs` | 93,512 | 2,430 | product-step AWBC execution and validation |
| `crates/arcweft-render-wgpu/src/renderer.rs` | 81,969 | 2,403 | shared WGPU frame submission orchestration |
| `crates/arcweft-bundle/src/container.rs` | 78,267 | 2,389 | bundle container codec plus embedded tests |
| `crates/arcweft-runtime-driver/src/session.rs` | 91,984 | 2,384 | runtime session orchestration |
| `crates/arcweft-cli/src/app/debug.rs` | 77,792 | 2,376 | CLI debug workflows plus embedded tests |
| `crates/arcweft-runtime-plan/src/expr.rs` | 83,504 | 2,363 | runtime expression lowering |
| `crates/arcweft-bundle/src/resource_codec/view/model.rs` | 74,341 | 2,358 | typed View resource model plus 42 embedded test LOC |
| `crates/arcweft-cli/src/app/agent/native/capture.rs` | 82,107 | 2,350 | legacy native Agent capture orchestration |
| `crates/arcweft-runtime-accelerator/src/math.rs` | 86,092 | 2,340 | accelerator numeric operations |
| `crates/arcweft-cli/src/app/bundle.rs` | 83,263 | 2,325 | bundle compilation orchestration |

No production file exceeds the 2,500-LOC error threshold.

## Dependency review

`arcweft-player-scene` has fan-out 18 (15 normal, 3 development) and normal
fan-in 3 (`arcweft-cli`, `arcweft-player-native`, and `arcweft-player-web`).
This slice adds normal edges to the lower-level `arcweft-render-text` and
`arcweft-text-layout` contracts consumed by `frame/view_text.rs`, plus test-only
edges to `arcweft-core` and `arcweft-view`. The direction remains
`render-text/text-layout -> render-wgpu/player-scene`; no low-level crate gains
a dependency on player, runtime-driver, CLI, platform, filesystem, or GPU I/O.
The complete structured edge inventory is in `dependency_edges.csv`.

## Boundary conclusion

The typed text-store lookup belongs to the Sans I/O runtime evaluator, document
resolution belongs to render-text, layout/preparation belongs to the shared
frame context, and exact Element/Text/Image/child ordering belongs to the
player-owned View scene adapter. Native and Web continue to consume the same
prepared frame. No compatibility alias, source gate, platform-specific text
evaluator, or duplicate layout implementation was introduced.
