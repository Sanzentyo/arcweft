# Runtime Control Rounded Shadow Font Parity - 2026-07-06

## Scope

This note records the fix for the modern feedback View regression where runtime
controls displayed broken rounded corners, coarse shifted shadows, and missing
CSS-style font family propagation for Japanese input/display text.

## Findings

- `agent observe` and the native window path both prepare frames through the
  shared `PlayerFramePlanner` and `SharedFramePlanner`. The current geometry
  mismatch was therefore not a separate observe-only layout path.
- Runtime control style parsing accepted visual colors, radii, depth, filters,
  and shadows, but `font-family` was not part of
  `ViewRuntimeControlVisualStyle`. The authored sample token was ignored and did
  not reach renderer text blocks.
- Runtime control borders and focus rings were drawn as four clipped filled
  rectangles. That did not match CSS border geometry for rounded corners and
  produced discontinuities at corners.
- Box-shadow blur used a sparse 9-tap coverage approximation. Large blur radii
  could look like shifted translucent copies of the caster rectangle.

## Implemented

- Added `font_family` to the runtime control style contract and lowered it into
  `RenderControlVisualStyle`.
- Text input and action button text blocks now use the authored runtime control
  font family.
- Renderer font family resolution now treats comma-separated CSS font stacks as
  a stack and prefers common Japanese system families such as `Yu Gothic` when
  present.
- `PaintRect` now supports stroke-only rendering. Runtime control borders and
  focus rings use one rounded stroke primitive instead of four clipped strips.
- Runtime control box shadows now use the element's resolved border radii when
  available, while preserving the previous per-shadow single-radius fallback.
- The box-shadow compositor shader now uses signed-distance coverage for blur
  instead of the sparse shifted sample kernel.
- Player font registration now uses `PlayerFontSet` as the shared contract for
  the frame planner and renderer. Native window, Web player, and player-backed
  Agent observe all register the same font bytes through that contract instead
  of relying on per-host ad hoc registration.
- `ViewCompositorTarget` now carries an explicit logical extent separate from its
  physical texture extent. Root and runtime-control targets map physical
  textures back to the design viewport, while offscreen group targets keep a
  target-local logical pixel domain for bucketed slack.
- Direct primitive NDC conversion, scissor calculation, clip uniforms, and
  analytic box-shadow uniforms now consume that explicit logical extent instead
  of inferring coordinate scale from texture dimensions or target origin. This
  fixes the class of observe/native display mismatches where shadows, rounded
  clips, and control surfaces could be displaced when the output texture was
  larger than the design viewport.

## Evidence

- `target/modern-feedback-view-debug/rounded-shadow-font-fixed-2048x1152.png`
  was captured through:

```bash
cargo run -p arcweft-cli --features native-capture --quiet -- agent observe samples/modern-feedback-view/src/main.arcw --json --image png --out target/modern-feedback-view-debug/rounded-shadow-font-fixed-2048x1152.png --viewport-width 2048 --viewport-height 1152 --mode drain --steps 8 --max-ops 128
```

The capture shows continuous rounded strokes and no duplicate shifted
box-shadow rectangle. The observe JSON has empty runtime style diagnostics.

- `target/modern-feedback-view-debug/observe-native-font-unified.png` was
  captured through:

```bash
cargo run -p arcweft-cli --features native-capture --quiet -- agent observe samples/modern-feedback-view/src/main.arcw --json --image png --out target/modern-feedback-view-debug/observe-native-font-unified.png --viewport-width 2048 --viewport-height 1152 --mode drain --steps 8 --max-ops 128
```

This verifies that player-backed observe registers the bundled player font set
with both the planner and offscreen renderer before capture.

- `target/modern-feedback-view-debug/modern-feedback-view-logical-extents.png` was
  captured through:

```bash
cargo run -p arcweft-cli --features native-capture -- agent observe samples/modern-feedback-view/src/main.arcw --json --image png --out target/modern-feedback-view-debug/modern-feedback-view-logical-extents.png --viewport-width 2048 --viewport-height 1152 --mode drain --steps 8 --max-ops 128
```

This capture verifies that the 1280x720 design-space controls render into a
2048x1152 output without the earlier ghost shadow rectangles or rounded-corner
misalignment. The new unit coverage checks that clip and box-shadow uniforms
preserve the explicit logical extent contract.

Validation for this coordinate-contract fix:

```bash
cargo test -p arcweft-render-wgpu view_compositor_uniform -- --nocapture
cargo test -p arcweft-render-wgpu --test geometry_runtime_control_styles -- --nocapture
cargo check -p arcweft-render-wgpu -p arcweft-player-scene -p arcweft-player-native -p arcweft-player-web -p arcweft-cli --features native-player,native-capture --all-targets
cargo clippy -p arcweft-render-wgpu -p arcweft-player-scene -p arcweft-player-native -p arcweft-player-web -p arcweft-cli --features native-player,native-capture --all-targets
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The structure audit scanned 2368 files, 1132 Rust files, and 534664 Rust
physical LOC. It reported 0 errors and 138 warnings, with no report files
written.

## Remaining Notes

- This does not add bundled Japanese font assets. Long-lived native and Web
  player hosts now register their loaded font bytes with both the renderer and
  the frame planner, and player-backed Agent observe does the same for its
  offscreen capture path. Other one-shot tools that bypass the player-backed
  path must still explicitly choose the same font bytes or system font stack.
- The observe image is content-only; the native interactive window includes the
  OS title bar and typed runtime state. Those differences remain expected.
- Scope-coupled presentation resource handles such as `let handle = image(...)`
  with automatic hide/unmount/drop semantics are not part of the current DSL or
  runtime contract. That design work is split to
  `docs/reviews/requests/2026-07-06-seq-06.16.6-scoped-presentation-resource-handles.md`.
