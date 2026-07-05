# Runtime Control Rounded Shadow Font Parity - 2026-07-06

## Scope

This note records the fix for the modern feedback UI regression where runtime
controls displayed broken rounded corners, coarse shifted shadows, and missing
CSS-style font family propagation for Japanese input/display text.

## Findings

- `agent observe` and the native window path both prepare frames through the
  shared `PlayerFramePlanner` and `SharedFramePlanner`. The current geometry
  mismatch was therefore not a separate observe-only layout path.
- Runtime control style parsing accepted visual colors, radii, depth, filters,
  and shadows, but `font-family` was not part of
  `UiRuntimeControlVisualStyle`. The authored sample token was ignored and did
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

## Evidence

- `target/modern-feedback-ui-debug/rounded-shadow-font-fixed-2048x1152.png`
  was captured through:

```bash
cargo run -p arcweft-cli --features native-capture --quiet -- agent observe samples/modern-feedback-ui/src/main.arcw --json --image png --out target/modern-feedback-ui-debug/rounded-shadow-font-fixed-2048x1152.png --viewport-width 2048 --viewport-height 1152 --mode drain --steps 8 --max-ops 128
```

The capture shows continuous rounded strokes and no duplicate shifted
box-shadow rectangle. The observe JSON has empty runtime style diagnostics.

## Remaining Notes

- This does not add bundled Japanese font assets. Long-lived native and Web
  player hosts now register their loaded font bytes with both the renderer and
  the frame planner, but parity for one-shot tools still depends on those tools
  choosing the same font bytes or system font stack.
- The observe image is content-only; the native interactive window includes the
  OS title bar and typed runtime state. Those differences remain expected.
