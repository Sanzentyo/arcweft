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

The implementation exposes deterministic prepared-frame effect records first.
Backend execution must consume those records inline with runtime-control paint
order; this slice does not fake blur by DOM overlays, screenshots, or
sample-specific images.

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
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The structural audit reported 0 errors and 129 warnings, with no report files
written.

## Non-goals and follow-up boundary

- Checked-in PNG baselines are not updated by this package.
- `samples/modern-feedback-ui` is not changed until native and web backends
  execute the prepared backdrop plan in the approved exact visual-golden
  environment.
- Unsupported filter functions stay diagnosed until each function has a typed
  payload and renderer execution path.
- The package does not add GPU/platform dependencies to `arcweft-bundle` or
  data-format crates.
