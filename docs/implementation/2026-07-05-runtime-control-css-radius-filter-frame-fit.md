# Runtime-control CSS radius, filters, and frame fit - 2026-07-05

## Scope

This note records the runtime-control style and modern feedback UI rendering
slice. It intentionally excludes checked-in PNG baseline promotion and web exact
readback baseline work.

## Implemented

- Runtime-control `border-radius` / `radius` now accepts CSS-compatible one,
  two, three, and four-value shorthands, plus slash-separated elliptical radii.
- Runtime-control radii lower into per-corner elliptical renderer geometry.
  The previous scalar radius remains only as the uniform shorthand API.
- Shared rectangle rendering now carries per-corner elliptical radii and clip
  radii through the WGPU vertex contract and fragment shader.
- Text inputs and action buttons apply authored radii to fills, borders, focus
  rings, caret/selection clipping, and button surfaces.
- Runtime-control `filter` and `backdrop-filter` now accept typed
  `brightness`, `contrast`, `grayscale`, `saturate`, `hue-rotate`, `invert`,
  `sepia`, `opacity`, and `blur` functions. Unsupported functions still produce
  structured diagnostics.
- Inline runtime-control `backdrop-filter` now samples a pre-runtime-control
  scene texture by default, so control text is not fed back into its own glass
  blur. The older "earlier runtime controls are part of the backdrop" policy is
  still represented separately in renderer data.
- Native and web player frames now prepare normal interactive frames in a
  1280x720 design viewport and map them to the output viewport with
  `ScalePolicy::Contain`. Agent observe keeps the raw viewport contract.
- The frame fit mapping updates paint rectangles, images, text, styled text,
  semantic bounds, hit bounds, runtime-control backdrop/filter/shadow/paint
  bounds, and focused text-input IME snapshots in one pass.
- The modern feedback sample keeps blur on text controls but removes button
  backdrop blur, avoiding a small-control glass artifact where the button label
  looked like a pale duplicate under the text.

## Non-goals

- No checked-in PNG baseline promotion.
- No web exact readback baseline promotion.
- Retained `PreparedUiScene` direct-compositor scene mapping is not included in
  this slice. The modern feedback runtime controls and normal shared frame
  geometry are mapped; retained scene mapping needs its own dependency/resource
  contract if it becomes part of interactive player scaling.

## Validation

Completed during implementation:

```bash
cargo test -p arcweft-render-wgpu --test geometry
cargo test -p arcweft-bundle --test runtime_control_style_resolution
cargo test -p arcweft-render-wgpu --test geometry_runtime_control_styles
cargo check -p arcweft-player-scene -p arcweft-player-native -p arcweft-player-web -p arcweft-cli --features native-player,native-capture --all-targets
cargo clippy -p arcweft-presentation -p arcweft-bundle -p arcweft-render-wgpu -p arcweft-player-scene -p arcweft-player-native -p arcweft-player-web -p arcweft-cli --features native-player,native-capture --all-targets
cargo +nightly -Zscript tools/structure-audit.rs --root .
cargo run -p arcweft-cli --quiet -- check samples\modern-feedback-ui\src\main.arcw
cargo run -p arcweft-cli --features native-capture --quiet -- agent observe samples\modern-feedback-ui\src\main.arcw --json --image png --out target\modern-feedback-ui-debug\modern-feedback-ui-final.png --mode drain --steps 8 --max-ops 128
```

The structure audit scanned 2361 files, 1129 Rust files, and 531887 Rust
physical LOC. It reported 0 errors and 132 warnings, with no report files
written.

The final modern feedback capture was written to
`target/modern-feedback-ui-debug/modern-feedback-ui-final.png`.
