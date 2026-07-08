# Implementation note: seq06.16.4 modern feedback View style-to-runtime-control rendering

## Audit summary

Current repo inspection showed the missing link is exactly at the product View/runtime-control boundary:

- `ViewStyleResource` already stores rules, part rules, typed values, interaction states, element states, and system colors.
- `ViewRuntimeTextControl` and `ViewRuntimeActionButton` currently carry values/bounds/actions but no visual style.
- `RuntimeTextControlLowerer` and `RuntimeActionButtonLowerer` currently pass values, labels, bounds, and actions into renderer controls, but no authored style.
- `RenderTextInputControl` and `RenderActionButton` currently paint with shared palette colors only.
- The player frame path is already shared by native/web/Agent through `BundlePresentationSnapshot` and `PlayerFramePlanner`.
- Existing seq06.13e substrate exposes `ViewBoxShadow`, `ViewBoxShadowList`, and `ViewBoxShadowPassPlan`, so this implementation plans shadows through that path rather than duplicating shadow rendering.

## Implemented edits in the patch

### `arcweft-bundle`

- Adds `runtime_control_style.rs`.
- Adds data-only `ViewRuntimeControlStyle` / `ViewRuntimeControlVisualStyle` / border / focus-ring / shadow payloads.
- Adds structured diagnostics for unsupported runtime-control style declarations.
- Adds resolver methods on Arcweft-owned types:
  - `SystemColor::runtime_control_rgba`
  - `ViewStyleResource::runtime_text_control_style`
  - `ViewStyleResource::runtime_action_button_style`
  - `ViewInputResource::runtime_text_controls_with_style`
  - `ViewProgramResource::runtime_action_buttons_with_style`
- Adds `style` fields to runtime text/action controls with serde defaults for backward decode compatibility.
- Adds optional `style` id on `ViewActionButtonResource` for direct authored action-button style targeting.

### `arcweft-runtime-driver`

- Uses the styled runtime-control constructors in `build_session_runtime`.
- Stores style diagnostics on the session runtime and forwards them into step diagnostics.
- Preserves diagnostics during content-only and compatible hot swaps.

### `arcweft-player-scene`

- Adds one conversion module that lowers `ViewRuntimeControlStyle` into renderer-owned `RenderControlStyle`.
- Wires text-control and action-button lowerers to attach style directly to `RenderTextInputControl` and `RenderActionButton`.

### `arcweft-render-wgpu`

- Adds renderer-owned `RenderControlStyle` payload.
- Text controls and action buttons resolve state against `InteractionVisualState`.
- Fill opacity, text color, border color, focus-ring color, and button hover/pressed/disabled states affect prepared-frame paint/text data.
- Runtime-control shadows are converted to `ViewBoxShadowList` and planned with `ViewBoxShadowPassPlan`; plans are exposed as `PreparedFrame.control_shadows` for renderer/smoke validation.

## Tests added

- `crates/arcweft-bundle/tests/runtime_control_style_resolution.rs`
  - background alpha and border color resolution;
  - hover/pressed/disabled state resolution;
  - focus-visible ring resolution;
  - supported box-shadow parsing;
  - structured unsupported property diagnostics.
- `crates/arcweft-player-scene/tests/runtime_control_style_lowering.rs`
  - text-control and action-button style payloads survive player-scene lowering.
- `crates/arcweft-render-wgpu/tests/geometry_runtime_control_styles.rs`
  - hover fill/text color affect prepared frame;
  - focused text control uses authored focus ring;
  - box-shadow reaches `ViewBoxShadowPassPlan`.

## Visual smoke artifacts

The `docs/fixtures/native` and `docs/fixtures/web` JSON files intentionally contain the same runtime-control bounds and state-resolved style. They are contract fixtures, not captured PNGs. Native/web exact PNG capture remains covered by the seq06.13e pinned visual-golden flow rather than this runtime style bridge.

## Remaining unsupported gaps

- Full CSS specificity/cascade layers/inheritance are not implemented in this bridge.
- Inline CSS patch text is not re-parsed at runtime; it must be lowered into `ViewStyleResource` by the product compiler.
- Rounded fill rasterization for player-owned controls is not implemented because current `PreparedFrame.rectangles` are plain rectangles. Radius is carried and used for shadow radius planning.
- `box-shadow` parsing is intentionally focused on px/rgb/rgba/#hex declarations already emitted by the style resource; unsupported forms produce diagnostics.
- Environment predicates are diagnosed instead of guessed until seq06.11 retained style resolution supplies theme/environment inputs.

## Verification status

Applied and verified in the local checkout based on parent commit `09f49a87`.

- `cargo fmt --all`
- `cargo test -p arcweft-bundle --test runtime_control_style_resolution` (4 passed)
- `cargo test -p arcweft-player-scene --test runtime_control_style_lowering` (2 passed)
- `cargo test -p arcweft-render-wgpu --test geometry_runtime_control_styles` (3 passed)
- `cargo test -p arcweft-runtime-driver text_control` (3 passed; remaining runtime-driver tests filtered by command)
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` (0 errors, 129 warnings)

The package README's filter-style commands (`cargo test -p ... runtime_control_style`) compile the touched crates but do not execute these integration assertions because the test names do not contain that exact contiguous filter string. The binary-specific commands above are the executed assertion coverage.

## Structural audit

Audit revision: Jujutsu working-copy change `wyysvmzm` on parent commit `09f49a87`. The audit added no Cargo dependency edges; fan-in/fan-out remains within existing bundle, runtime-driver, player-scene, render-wgpu, and CLI crate boundaries. Major responsibilities added in this slice are typed style resolution, player-scene style lowering, renderer prepared-frame style application, and focused assertion tests.

Changed Rust file measurements from `target/structure-audit-seq06-16-4/file_metrics.csv`:

| Path | Bytes | LOC | Kind | Embedded tests |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-bundle/src/resource_codec.rs` | 3152 | 65 | production | false |
| `crates/arcweft-bundle/src/resource_codec/view.rs` | 770 | 18 | production | false |
| `crates/arcweft-bundle/src/resource_codec/view/model.rs` | 46781 | 1555 | production | true |
| `crates/arcweft-bundle/src/resource_codec/view/runtime_control_style.rs` | 29945 | 889 | production | false |
| `crates/arcweft-bundle/tests/runtime_control_style_resolution.rs` | 8241 | 240 | test | false |
| `crates/arcweft-bundle/tests/view_action_button_resources.rs` | 2165 | 56 | test | false |
| `crates/arcweft-cli/src/app/bundle_view.rs` | 38114 | 1092 | production | false |
| `crates/arcweft-player-scene/src/action_buttons.rs` | 4417 | 125 | production | false |
| `crates/arcweft-player-scene/src/control_style.rs` | 3170 | 88 | production | false |
| `crates/arcweft-player-scene/src/lib.rs` | 226 | 10 | production | false |
| `crates/arcweft-player-scene/src/text_controls.rs` | 7371 | 193 | production | false |
| `crates/arcweft-player-scene/tests/action_button_submit.rs` | 5593 | 146 | test | false |
| `crates/arcweft-player-scene/tests/runtime_control_style_lowering.rs` | 3548 | 89 | test | false |
| `crates/arcweft-player-scene/tests/runtime_text_controls.rs` | 9187 | 236 | test | false |
| `crates/arcweft-render-wgpu/src/geometry.rs` | 47809 | 1488 | production | false |
| `crates/arcweft-render-wgpu/src/geometry/action_buttons.rs` | 6429 | 181 | production | false |
| `crates/arcweft-render-wgpu/src/geometry/control_style.rs` | 8323 | 295 | production | false |
| `crates/arcweft-render-wgpu/src/geometry/text_controls.rs` | 29069 | 844 | production | true |
| `crates/arcweft-render-wgpu/tests/geometry_runtime_control_styles.rs` | 6749 | 206 | test | false |
| `crates/arcweft-runtime-driver/src/session.rs` | 56102 | 1488 | production | false |

Largest Rust hotspots from the same audit:

| Path | Bytes | LOC | Kind | Embedded tests |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | 357456 | 12399 | generated | false |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 255414 | 7944 | test | false |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 243051 | 6758 | test | false |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 222475 | 6161 | test | false |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 209852 | 5651 | test | false |
| `crates/arcweft-cli/tests/check/agent_script_debug.rs` | 195828 | 5250 | test | false |
| `crates/arcweft-render-native/src/tests.rs` | 154290 | 4415 | test | false |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs` | 143360 | 4181 | test | false |
| `crates/arcweft-cli/src/app/agent/native/tests.rs` | 121654 | 3535 | test | false |
| `crates/arcweft-core/src/engine/eval/calls.rs` | 89477 | 2481 | production | false |

## Design deviations

None. The bridge stays on the player-owned runtime-control path and does not add DOM/CSS overlays, browser-native controls, sample-specific geometry, or a duplicate shadow renderer.
