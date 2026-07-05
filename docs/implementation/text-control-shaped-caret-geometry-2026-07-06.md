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
- Added stateful shared/player frame planner contexts that accept the same
  project-owned font bytes as `SharedRenderer`. Native and Web player startup
  now register their loaded font bytes with both the renderer and the planner,
  so dynamically supplied font data is available to caret, selection, scroll,
  and IME geometry planning.
- Added text-control layout cache counters and reused shaped layouts across
  unchanged stateful prepares. The player frame planner now only rebuilds a
  second frame when initial keyboard focus actually changes, instead of
  preparing every redraw twice.

## Evidence

- `cargo test -p arcweft-render-wgpu --lib geometry::text_controls::tests -- --nocapture`
- `cargo test -p arcweft-render-wgpu --test geometry_runtime_control_styles`
- `cargo test -p arcweft-render-wgpu --lib font_family::tests -- --nocapture`
- `cargo test -p arcweft-render-wgpu stateful_planner_reuses_text_control_layout_cache_with_registered_fonts -- --nocapture`
- `cargo test -p arcweft-player-scene --test action_button_submit -- --nocapture`
- `cargo check -p arcweft-render-wgpu`
- `cargo check -p arcweft-player-scene -p arcweft-player-native -p arcweft-player-web -p arcweft-render-wgpu`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`
- `cargo run -p arcweft-cli --features native-capture --quiet -- agent observe samples/modern-feedback-ui/src/main.arcw --json --image png --out target/modern-feedback-ui-debug/text-caret-shaped-layout-2048x1152.png --viewport-width 2048 --viewport-height 1152 --mode drain --steps 8 --max-ops 128`

## Remaining Notes

- Long-lived native and Web hosts should use `PlayerFramePlannerState` or
  `SharedFramePlanContext` and register the same font bytes that they register
  with `SharedRenderer`. The stateless `SharedFramePlanner::prepare` remains a
  one-shot compatibility facade and intentionally uses a fresh planner context.
- This change reduces repeated shaping/prepare work, but the native and Web
  event loops still request continuous redraws for runtime/animation safety.
  Idle redraw scheduling can be tightened separately once the runtime exposes a
  typed "visual work pending" signal.
