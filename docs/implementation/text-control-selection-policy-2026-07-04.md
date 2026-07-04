# Text control selection and edit policy

This cut adds runtime text-control selection behavior and edit-policy controls
for `TextField`, `TextArea`, and `SecureField`.

## Implemented

- `TextInputOptions` now carries typed selection, shortcut, and tab policies:
  selection is enabled by default, shortcuts are enabled by default, and Tab
  keeps the existing focus-navigation behavior unless explicitly configured to
  insert a tab character.
- `UiInputOptions` and `UiRuntimeTextControlOptions` carry the same policies
  with serde defaults so existing resources keep their previous behavior.
- Component/View text controls can author these policies through arguments such
  as `selection: disabled`, `shortcuts: disabled`, and `tab: insert`.
- The shared editor enforces disabled selection and disabled shortcut commands
  even when a platform/Web adapter sends `SetSelection`, `SelectAll`, `Copy`,
  `Cut`, or `Paste` directly.
- Native keyboard routing maps focused text controls to ordinary editing
  commands: Shift extends selection, Ctrl/Alt arrows move by word, Meta arrows
  move to line start/end, Ctrl/Meta A/C/X/V route select/copy/cut/paste, and
  Tab inserts `\t` when the text-control tab policy requests it.
- Player pointer routing now keeps an initial click intent until focused
  renderer geometry is available, then places the caret from glyph hit-testing.
  Dragging a focused text control extends the live selection using the same
  renderer geometry.
- Runtime control style resolution accepts `selection-color`,
  `selection-background-color`, and `caret-color`, lowers them into render
  control style, and uses them for focused text-control selection and caret
  rectangles.
- Web text-input command labels now include `move_up` and `move_down` so the
  shared editor command surface has vertical movement labels.

## Remaining TODOs

- Up/down movement in the shared editor is logical-line based. It does not yet
  use renderer soft-wrap geometry to preserve visual columns across wrapped
  lines.
- Secure-field geometry remains redacted before it reaches host/platform
  surfaces. Pointer hit-testing for secure controls therefore needs a future
  non-leaking internal surrogate if precise secure-field click placement is
  required.

## Design deviations

- No intentional contract deviation. The secure hit-test limitation follows the
  existing secure geometry redaction contract rather than exposing character
  bounds.

## Validation

- `cargo fmt`
- `cargo fmt --all -- --check`
- `cargo check -p arcweft-bundle`
- `cargo check -p arcweft-presentation -p arcweft-ui -p arcweft-bundle -p arcweft-player-scene -p arcweft-render-wgpu -p arcweft-runtime-host -p arcweft-runtime-driver -p arcweft-player-native -p arcweft-player-web -p arcweft-cli`
- `cargo test -p arcweft-presentation --test text_editor_behavior`
- `cargo test -p arcweft-player-scene --test runtime_text_controls --test runtime_control_style_lowering`
- `cargo test -p arcweft-bundle --test runtime_control_style_resolution --test ui_runtime_text_controls --test ui_resource_codecs`
- `cargo test -p arcweft-render-wgpu --test geometry_runtime_control_styles`
- `cargo test -p arcweft-player-native --lib`
- `cargo test -p arcweft-player-web --lib`
- `cargo test -p arcweft-ui --lib`
- `cargo test -p arcweft-runtime-host --lib`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

Structure audit completed as a dry run and reported `0 error(s), 129
warning(s)` without writing report files.
