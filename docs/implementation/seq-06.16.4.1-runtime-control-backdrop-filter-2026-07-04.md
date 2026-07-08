# Implementation note: seq06.16.4.1 runtime-control backdrop-filter

## Source request

This applies
`arcweft-seq06.16.4.1-runtime-control-backdrop-filter-2026-07-04.zip`.
The package implements the focused request split from seq06.16.4: typed
runtime-control `filter` / `backdrop-filter`, diagnostics, renderer ordering,
tests, sample update gate, and native/web exact visual evidence requirements.

## Implemented

### `arcweft-bundle`

- Extends `ViewRuntimeControlVisualStyle` with optional foreground and backdrop
  filter lists.
- Adds `ViewRuntimeControlFilterList` and
  `ViewRuntimeControlFilter::Blur { radius_milli }`.
- Resolves `backdrop-filter`, `-webkit-backdrop-filter`, and `filter` from
  `ViewStyleResource` declarations.
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
  `ViewFilterPassPlan` blur, and composites the blurred result back through a
  logical-rect clip before painting that control's fill/text.
- Executes `PreparedControlFilter` inline in `SharedRenderer`: filtered runtime
  controls are replayed into a transparent Arcweft-owned intermediate texture,
  passed through the same fixed-extent `ViewFilterPassPlan`, and composited back
  through the control bounds. This filters the completed control content without
  blurring the already-painted backdrop.
- Keeps native and web surface textures at their existing render-attachment
  usage by rendering first into an Arcweft-owned intermediate texture, then
  compositing to the host-provided target view.
- Adds an ignored GPU smoke that verifies a transparent text-control
  `backdrop-filter: blur(...)` changes captured pixels through the shared
  offscreen renderer path.
- Adds an ignored GPU smoke that verifies a foreground `filter: blur(...)`
  changes the completed control-content pixels through the shared offscreen
  renderer path.
- Updates `samples/modern-feedback-view` so TextField, TextArea, and Button use
  authored `backdrop-filter: blur(...)` now that the native/web shared renderer
  path executes runtime-control backdrops.

This does not fake blur by DOM overlays, screenshots, or sample-specific
images. Native window/offscreen and web canvas hosts consume the same
`SharedRenderer` path.

## 2026-07-06 rendering-order follow-up

Modern feedback UI debugging exposed that `glyphon::TextRenderer` cannot be
prepared repeatedly on the same renderer while earlier render commands in the
same command encoder still reference its vertex buffer. The visible symptoms
were missing runtime-control labels, partially missing styled dialogue text,
and occasional `glyphon vertices` validation failures.

The shared renderer now keeps ordinary frame text in one final main text pass.
Foreground-filtered controls still render completed control content, including
their text, into the filter source, but they use auxiliary text-renderer
instances retained on `SharedRenderer` until the next submitted frame. The main
text pass excludes those foreground-filtered control text ranges to avoid
unfiltered duplicate labels.

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
  - `prepared_control_foreground_filter_blur_executes_shared_renderer_path`
  - both are ignored by default and can be run explicitly on a local GPU
    adapter.

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
cargo test -p arcweft-render-wgpu --test runtime_control_backdrop_gpu_smoke -- --ignored
cargo test -p arcweft-player-scene --test action_button_submit
cargo test -p arcweft-player-scene --test runtime_text_controls
cargo run -p arcweft-cli --features native-capture --quiet -- agent observe samples/modern-feedback-view/src/main.arcw --json --image png --out target/modern-feedback-view-debug/single-text-pass-aux.png --mode drain --steps 8 --max-ops 128
cargo run -p arcweft-cli --features native-capture --quiet -- agent observe samples/native-text-input/src/main.arcw --json --image png --out target/modern-feedback-view-debug/native-text-input-single-pass-aux.png --mode drain --steps 8 --max-ops 128
cargo run -p arcweft-cli -- check --manifest-path samples/modern-feedback-view/arcw.toml
cargo run -p arcweft-cli -- bundle samples/modern-feedback-view/src/main.arcw --output target/arcweft/modern-feedback-view-backdrop-filter.awfb
cargo run -p arcweft-cli --features native-capture -- agent observe samples/modern-feedback-view/src/main.arcw --json --image png --capture color --content-policy-mode local-dev --out target/modern-feedback-view/backdrop-filter-observe.png --mode drain --steps 4 --max-ops 64
cargo test -p arcweft-render-wgpu
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
- `samples/modern-feedback-view` still needs pinned exact visual-golden evidence
  before any checked-in PNG baseline promotion.
- Unsupported filter functions stay diagnosed until each function has a typed
  payload and renderer execution path.
- The package does not add GPU/platform dependencies to `arcweft-bundle` or
  data-format crates.
