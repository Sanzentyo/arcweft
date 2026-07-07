# Selectable static view text blocks

Date: 2026-07-08

## Summary

Static view-authored text blocks now have an opt-in selection policy. Runtime text controls keep their existing editable editor state; static text blocks use a separate lightweight selection state in `InputController`.

## Contract

- `ViewTextBlockResource` and `ViewRuntimeTextBlock` carry `selection_policy`.
- Text block selection defaults to disabled even though editable text controls still default to enabled.
- View lowering accepts `selection` / `selection_policy` and `selectable` style properties for `Text(...)`.
- `RenderTextBlock` can carry an optional target and selection range.
- `PreparedFrame` exposes `selectable_text_blocks` plus hit-test helpers for static text selection.

## Performance shape

Selectable text blocks are renderer-backed only when opted in. Disabled static text blocks remain ordinary glyphon text blocks and do not receive selection geometry. Enabled blocks reuse the frame planner's glyphon font context instead of constructing a separate editor or platform IME state.

The selection state is not persisted in runtime save snapshots. It is transient presentation state, like browser page text selection.

## Implemented behavior

- Pointer drag selects ranges in selectable static text blocks.
- Shift-click extends from the existing anchor when the same text block and text value are still active.
- Double-click selects a shared `TextIndexSnapshot` word-like run.
- Triple-click selects the logical line containing the hit offset.
- Prepared selection rectangles are derived from the same shaped glyph ranges as rendering, so proportional Latin, Japanese fallback, and `Дﾟ` style clusters use renderer-backed bounds.

## Current boundary

OS clipboard export for selected static text is intentionally not wired in this slice because `InputController::keyboard_with_modifiers` currently receives only Shift. Static text copy should be added when Ctrl/Meta modifier state is carried through the native/web input boundary, using the same `TextClipboardIntent`/host request path added for editable text controls.

## Validation

```bash
cargo check -p arcweft-player-scene --tests --all-features
cargo test -p arcweft-player-scene selectable_runtime_text_block_drag_adds_selection_rectangles --test scroll_regions --all-features
```
