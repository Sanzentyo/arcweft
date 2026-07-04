# Implementation note: seq06.16.4.1 runtime-control backdrop-filter

## Source request

This applies
`arcweft-seq06.16.4.1-runtime-control-backdrop-filter-2026-07-04.zip`.
The package implements the focused request split from seq06.16.4: typed
runtime-control `filter` / `backdrop-filter`, diagnostics, renderer ordering,
tests, sample update gate, and native/web exact visual evidence requirements.

## Implemented

### `arcweft-bundle`

- Extends `UiRuntimeControlVisualStyle` with optional foreground and backdrop
  filter lists.
- Adds `UiRuntimeControlFilterList` and
  `UiRuntimeControlFilter::Blur { radius_milli }`.
- Resolves `backdrop-filter`, `-webkit-backdrop-filter`, and `filter` from
  `UiStyleResource` declarations.
- Accepts `none` and whitespace-separated `blur(<non-negative px length>)`
  functions.
- Emits `UnsupportedValue` for unsupported filter functions and invalid units.
- Keeps unsupported unrelated properties diagnosed as `UnsupportedProperty`.

### `arcweft-player-scene`

- Lowers bundle milli-pixel filter radii into renderer logical-pixel filter
  lists.
- Keeps conversion inside the existing `control_style` boundary.

### `arcweft-render-wgpu`

- Extends `RenderControlVisualStyle` with optional foreground/backdrop filter
  lists.
- Adds `PreparedControlBackdrop` and `PreparedControlFilter` plan records.
- Adds
  `RuntimeControlBackdropSamplePolicy::PriorFrameContentAndEarlierRuntimeControls`.
- Plans backdrop records before control shadows/fill for each sorted
  runtime-control item.
- Plans foreground filter records after fill/border/focus/selection/caret/text
  for the same control item.
- Adds `PreparedControlPaint` spans so shared renderer backends can replay
  runtime controls in prepared paint order instead of treating all rectangles
  and text as one overlay batch.
- Executes `PreparedControlBackdrop` inline in `SharedRenderer`: each backdrop
  copies the current Arcweft-owned intermediate target, runs a fixed-extent
  `UiFilterPassPlan` blur, and composites the blurred result back through a
  logical-rect clip before painting that control's fill/text.
- Keeps native and web surface textures at their existing render-attachment
  usage by rendering first into an Arcweft-owned intermediate texture, then
  compositing to the host-provided target view.
- Adds an ignored GPU smoke that verifies a transparent text-control
  `backdrop-filter: blur(...)` changes captured pixels through the shared
  offscreen renderer path.

This does not fake blur by DOM overlays, screenshots, or sample-specific
images. Native window/offscreen and web canvas hosts consume the same
`SharedRenderer` path.

## Tests added or updated

- `crates/arcweft-bundle/tests/runtime_control_style_resolution.rs`
  - `backdrop_filter_blur_resolves_to_typed_runtime_control_effect`
  - `foreground_filter_blur_resolves_to_typed_runtime_control_effect`
  - `unsupported_filter_function_produces_structured_diagnostic`
  - the unsupported-property test now uses `transform`, because
    `backdrop-filter` is supported
- `crates/arcweft-player-scene/tests/runtime_control_style_lowering.rs`
  - `runtime_control_backdrop_filter_reaches_render_style`
- `crates/arcweft-render-wgpu/tests/geometry_runtime_control_styles.rs`
  - `backdrop_filter_reaches_runtime_control_backdrop_plan`
  - `foreground_filter_reaches_runtime_control_filter_plan`
  - `runtime_control_paint_span_carries_inline_backdrop_order`
- `crates/arcweft-render-wgpu/tests/runtime_control_backdrop_gpu_smoke.rs`
  - `prepared_control_backdrop_blur_executes_shared_renderer_path`
    is ignored by default and can be run explicitly on a local GPU adapter.

## Validation

Completed in this checkout:

```bash
cargo fmt --all
cargo test -p arcweft-bundle --test runtime_control_style_resolution backdrop_filter
cargo test -p arcweft-bundle --test runtime_control_style_resolution foreground_filter
cargo test -p arcweft-bundle --test runtime_control_style_resolution unsupported_filter
cargo test -p arcweft-player-scene --test runtime_control_style_lowering runtime_control_backdrop_filter
cargo test -p arcweft-render-wgpu --test geometry_runtime_control_styles backdrop_filter
cargo test -p arcweft-render-wgpu --test geometry_runtime_control_styles foreground_filter
cargo test -p arcweft-render-wgpu
cargo test -p arcweft-render-wgpu --test runtime_control_backdrop_gpu_smoke -- --ignored
cargo check -p arcweft-player-native
cargo check -p arcweft-render-web
cargo check -p arcweft-player-web
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The structural audit reported 0 errors and 129 warnings, with no report files
written.

## Non-goals and follow-up boundary

- Checked-in PNG baselines are not updated by this package.
- `samples/modern-feedback-ui` still needs pinned exact visual-golden evidence
  before any checked-in PNG baseline promotion.
- `PreparedControlFilter` foreground execution remains a follow-up. This slice
  implements `PreparedControlBackdrop` inline blur execution.
- Unsupported filter functions stay diagnosed until each function has a typed
  payload and renderer execution path.
- The package does not add GPU/platform dependencies to `arcweft-bundle` or
  data-format crates.
