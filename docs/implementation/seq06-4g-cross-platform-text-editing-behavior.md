# Seq06.4g Cross-Platform Text Editing Behavior

This overlay adds the shared editor behavior layer requested by seq06.4g.

## Ownership

- `arcweft-presentation::text_index::TextIndexSnapshot` remains the canonical byte/UTF-16 range validation owner and now also owns shared scalar, grapheme, word, and pointer-slot movement boundaries.
- `arcweft-presentation::text_editor::TextEditorState` owns committed text, selection anchor/caret, active composition range, preedit selection, revision, secure/plain policy, and deterministic geometry snapshots.
- Platform adapters continue to normalize OS/browser callbacks into `TextInputOperation` and `TextEditCommand`; they do not own caret movement, deletion, replacement, clipboard, or composition state-machine semantics.

## Applied behavior

- Invalid byte and UTF-16 ranges are rejected through `TextIndexSnapshot`; no adapter-side silent clamp is introduced.
- Arrow, word, line, backspace/delete, select-all, copy/cut/paste, submit, and cancel are implemented by `TextEditorState::apply_operation`.
- Composition update replaces the requested range, tracks the original selected text, and cancels back to that original text for cancelled/focus-lost/session-invalidated/platform-disabled endings.
- Clipboard operations are rejected for secure fields before any text leaves the editor.
- `TextEditorLayout` converts renderer layout into `TextInputClientSnapshot` and `TextInputGeometrySnapshot`; production renderers should replace the monospaced fixture layout with glyph layout data.

## Integration hooks

- Web EditContext receives refreshed text/selection and viewport/client geometry through the seq06.4a.2 glue.
- Windows TSF and macOS AppKit overlays are retained and documented as consumers: TSF `ITextStoreACP` and AppKit `NSTextInputClient` must read/write via `TextEditorState` snapshots instead of implementing local movement/deletion logic.
- Wayland, Android, and iOS should follow the same contract in their later packages.

## Validation status

The overlay includes pure Rust tests under `crates/arcweft-presentation/tests/text_editor_behavior.rs`. This package environment did not contain `rustfmt`, `cargo`, or `rustc`; the commands to run in the real checkout are recorded in `verification/VALIDATION.md`.
