# Text Control Advanced Editing And Web Glue - 2026-07-08

## Scope

This slice extends runtime text-control editing behavior without preserving old
compatibility spellings or adding hidden DOM text-entry fallbacks.

Implemented behavior:

- `PageUp` / `PageDown` text-edit commands.
- Double-click word selection and triple-click logical-line selection.
- Drag-selection auto-scroll for text controls inside prepared scroll regions.
- Selected-text drag move within the same focused text control.
- Right-click context positioning for text controls, limited to focus/caret
  placement.
- Web `EditContext` command resolution moved from `web/player-editcontext.js`
  into Rust/wasm through `arcweft_web_text_input_command_for_key_event`.
- Web `EditContext` object creation moved from direct JavaScript constructor
  use into Rust/wasm through `arcweft_web_text_input_create_edit_context`,
  using `js_sys::Reflect` because `web-sys` does not expose a stable
  `EditContext` type.

## Boundaries

The editor core owns text ranges, grapheme-aware word selection, line
selection, page movement, and selected-text movement. Scene input owns click
count, pointer drag intent, and scroll-offset updates. Native and web players
only map platform events into the shared scene/editor model.

The JavaScript file remains as a DOM boundary for browser event listeners,
clipboard event payloads, `DOMRect` calls, and `EditContext` method calls. It no
longer owns keyboard command interpretation or the `EditContext` constructor.

## Non-goals

Host clipboard integration and actual context-menu item presentation remain in
`docs/reviews/requests/2026-07-08-seq-06.16.8-cross-platform-clipboard-text-control-capability.md`.
Right-click currently positions focus/caret only.

Double/triple-click timing is deterministic and replay-friendly: repeated
activation is recognized by target, pointer distance, and input epoch
proximity. There is not yet a platform timestamp threshold.

## Validation

- `cargo test -p arcweft-presentation --all-features --test text_editor_behavior`
- `cargo test -p arcweft-player-scene --all-features --test runtime_text_controls`
- `cargo check -p arcweft-player-native -p arcweft-player-web --all-targets --all-features`
- `cargo check -p arcweft-player-web --target wasm32-unknown-unknown --all-features`
- `cargo test -p arcweft-player-web --all-features --lib`
- `cargo test -p arcweft-player-web --all-features --test web_text_input_glue --test web_runtime_text_input_bridge_source_gate --test web_edit_context_source_gate --test input --test interaction_visual_state`
