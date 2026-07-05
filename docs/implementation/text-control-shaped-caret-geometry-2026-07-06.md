# Text Control Shaped Caret Geometry - 2026-07-06

## Scope

Runtime text controls no longer estimate caret, selection, scroll, and IME
geometry with fixed ASCII/CJK width heuristics. The visible text is shaped with
glyphon/cosmic-text before geometry is produced, so narrow glyphs such as `l`
advance differently from wider glyphs such as `a`.

## Implemented

- Added a renderer-local text-control font context used during
  `SharedFramePlanner` runtime control planning.
- Replaced `estimated_text_input_glyph_width` with glyphon `Buffer` layout runs
  for text-control geometry.
- Text-control caret, selection rectangles, soft-wrap scroll, and IME geometry
  now use shaped glyph cluster bounds.
- Secure controls shape the displayed mask text, such as `**`, and map those
  displayed glyph ranges back to source byte ranges. Caret geometry therefore
  follows what is visible, not the hidden source glyph widths.
- Moved CSS-like font stack resolution into a shared renderer module so the
  planner-side shaping path and draw-side glyphon path use the same family
  selection rule.

## Evidence

- `cargo test -p arcweft-render-wgpu --lib geometry::text_controls::tests -- --nocapture`
- `cargo test -p arcweft-render-wgpu --test geometry_runtime_control_styles`
- `cargo test -p arcweft-render-wgpu --lib font_family::tests -- --nocapture`
- `cargo check -p arcweft-render-wgpu`
- `cargo run -p arcweft-cli --features native-capture --quiet -- agent observe samples/modern-feedback-ui/src/main.arcw --json --image png --out target/modern-feedback-ui-debug/text-caret-shaped-layout-2048x1152.png --viewport-width 2048 --viewport-height 1152 --mode drain --steps 8 --max-ops 128`

## Remaining Notes

- This uses the same glyphon/cosmic-text shaping algorithm and font-family
  resolver as rendering, but `SharedFramePlanner` still owns its own
  `FontSystem`. If future products rely on dynamically registered font bytes
  that are not available through the system font database, the planner font
  context should receive those same bytes before exact native/web parity is
  claimed.
