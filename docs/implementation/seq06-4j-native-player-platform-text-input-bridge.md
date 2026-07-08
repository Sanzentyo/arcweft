# seq06.4j Native Player Platform Text Input Bridge

Date: 2026-06-29

## Status

Applied the seq06.4j package as the native-player ownership cut for platform
text input. The implementation moves final IME acceptance away from the
diagnostic `windows-tsf-ime-sample` binary and into the normal DSL-backed native
player path:

```bash
cargo run -p arcweft-cli --features native-player -- run \
  --runner native samples/native-text-input/src/main.arcw \
  --text-input-trace-out fixtures/native-text-input-trace/native-player-ime.real.json
```

The bridge is owned by `arcweft-player-native`, uses the portable
`TextInputDispatchState` contract, and keeps native handles inside the
window-adapter or diagnostic crates. Sans I/O crates continue to see only
Arcweft text-input snapshots, geometry snapshots, host commands, and routed
`TextInput` batches.

## Implemented

- Added `NativeTextInputBridge`, winit window text-input source boundary, and
  JSON trace capture under `arcweft-player-native`.
- Added native player run entry points that accept
  `NativeTextInputBridgeOptions`.
- Added `arcw run --text-input-trace-out` as a native-runner-only flag.
- Threaded the trace option through native run and native watch/hot-patch run.
- Added renderer-owned `PreparedTextInputTarget` and
  `PreparedFrame::focused_text_input_target()` hook.
- Added source gates for native identity leakage and CLI/native trace wiring.
- Added a DSL-backed native text-input sample and trace schema fixture.

## 2026-07-02 Event Path Update

The player-rendered sample now lowers focused Arcweft text controls into
`PreparedTextInputTarget` snapshots and geometry. Live Windows traces showed
focus and geometry records for the three controls, but ordinary key input did
not reliably become platform text events through the previous TSF experiment.

The native player now treats winit as the single windowed text source for the
normal `arcw run --runner native` path. It enables and updates IME state from
the prepared Arcweft snapshot/geometry, routes winit
preedit/commit/disable/delete-surrounding events into the shared player-owned
editor, routes ordinary printable keyboard text from `KeyEvent.text`, and keeps
secure controls from publishing surrounding text. The normal player no longer
installs a separate TSF/AppKit backend on the same live window; those
platform-specific adapters remain diagnostic/future specialized boundaries so
they cannot compete with winit for focus, candidate UI, or key dispatch.

The text-input contract also now has `TextDeleteUnit::Utf8Byte` so winit's
UTF-8 byte `DeleteSurrounding` event is represented exactly instead of being
approximated as scalar or grapheme deletion.

## Explicit Gap

Final Windows acceptance still needs a pinned real Japanese IME trace captured
from the normal native player after this winit-only event-source update. macOS
acceptance should use the same winit-backed native player route first; AppKit
experiments remain diagnostic until the project intentionally replaces the
window text source instead of layering it on top.

## 2026-07-02 Geometry Follow-up

Live Windows validation then showed that Japanese conversion itself reached the
player, but visible carets and candidate windows were shifted to the right
across the text field, text area, and secure field. The focused text-control
geometry still used a provisional single-line advance estimate based on the
full line box height, and the text-area text bounds exposed only one line of
height. That made the Arcweft caret and the winit IME cursor area drift away
from the glyphon-rendered text.

The render-wgpu text-control planner now builds one text-local glyph geometry
model per runtime text control and reuses it for the Arcweft-rendered caret,
selection rectangles, and `PreparedTextInputTarget` geometry. The model handles
newlines by moving caret/selection geometry onto the next visual line, uses
font-size-based width estimates instead of full-line-height estimates, and gives
multiline controls the full inner text bounds. This is still an approximation
until runtime text controls consume exact glyphon layout output, but it removes
the known right-shift and one-line text-area clipping root causes without adding
a second native input path.

The native text-input sample contract was also updated to stop requiring the
removed `platform_event` trace record; live winit input is observed through
`routed_text_input` records.

## Diagnostic Harness Boundary

`windows-tsf-ime-sample` remains a backend diagnostic harness only. Passing that
binary is useful for TSF debugging, but it does not satisfy seq06.4j acceptance.
The acceptance surface is `arcw run --runner native` with the DSL sample.

## Validation

Run in this checkout:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-player-native -p arcweft-cli --features arcweft-cli/native-player --all-targets
cargo test -p arcweft-player-native native_text_input_bridge -- --nocapture
cargo test -p arcweft-player-native --test native_text_input_bridge_source_gate -- --nocapture
cargo test -p arcweft-cli --test native_text_input_trace_cli -- --nocapture
cargo clippy -p arcweft-player-native -p arcweft-cli --features arcweft-cli/native-player --all-targets -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Executed on 2026-06-29:

- `cargo fmt --all -- --check` passed.
- `cargo check -p arcweft-player-native -p arcweft-cli --features arcweft-cli/native-player --all-targets` passed.
- `cargo check -p arcweft-cli --all-targets` passed.
- `cargo test -p arcweft-player-native --test native_text_input_bridge -- --nocapture` passed.
- `cargo test -p arcweft-player-native --test native_text_input_bridge_source_gate -- --nocapture` passed.
- `cargo test -p arcweft-cli --test native_text_input_trace_cli -- --nocapture` passed.
- `cargo clippy -p arcweft-player-native -p arcweft-cli --features arcweft-cli/native-player --all-targets -- -D warnings` passed.
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` passed with `0 error(s), 117 warning(s)` across 982 Rust files and 466,233 Rust physical LOC.

Executed on 2026-07-02 after the winit-only event-source convergence:

- `cargo fmt --all` passed.
- `cargo test -p arcweft-presentation text_editor::tests::delete_surrounding_utf8_byte_unit --lib` passed.
- `cargo test -p arcweft-player-native scene_windowed::tests --lib` passed.
- `cargo test -p arcweft-player-scene text_input --lib` passed.
- `cargo test -p arcweft-runtime-host player_text_input_bridge --lib` passed.
- `cargo test -p arcweft-view text_field --lib` passed.
- `cargo check -p arcweft-player-native -p arcweft-player-scene -p arcweft-runtime-host -p arcweft-presentation -p arcweft-view --all-targets` passed.
- `cargo clippy -p arcweft-player-native -p arcweft-player-scene -p arcweft-runtime-host -p arcweft-presentation -p arcweft-view --all-targets -- -D warnings` passed.
- `cargo build -p arcweft-cli --features native-player` passed.
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` passed with `0 error(s), 124 warning(s)` across 1,045 Rust files and 491,089 Rust physical LOC.
- `git diff --check` passed.

Executed on 2026-07-02 after the text-control geometry follow-up:

- `cargo fmt --all` passed.
- `cargo test -p arcweft-render-wgpu text_controls::tests --lib` passed.
- `cargo test -p arcweft-player-scene --test runtime_text_controls` passed.
- `cargo test -p arcweft-player-native scene_windowed::tests --lib` passed.
- `cargo check -p arcweft-render-wgpu -p arcweft-player-scene -p arcweft-player-native --all-targets` passed.
- `cargo clippy -p arcweft-render-wgpu -p arcweft-player-scene -p arcweft-player-native --all-targets -- -D warnings` passed.
- `cargo build -p arcweft-cli --features native-player` passed.
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` passed with `0 error(s), 124 warning(s)` across 1,045 Rust files and 491,247 Rust physical LOC.
- `git diff --check` passed.

Windows live validation:

```powershell
cargo run -p arcweft-cli --features native-player -- run `
  --runner native samples/native-text-input/src/main.arcw `
  --text-input-trace-out fixtures/windows-tsf-real-ime/microsoft-japanese-ime-hiragana.player.json
```

Expected live evidence after renderer-backed text input geometry exists:

- backend `winit_window_ime` with capability records;
- focus records for distinct controls and focus generations;
- routed text-input records for printable keyboard text;
- routed text-input records for composition, commit, deletion, and commands when
  winit emits IME events;
- routed `TextInput` records through the player path;
- non-zero screen caret/character geometry for plain fields;
- `secure_redacted=true` and no plaintext/character geometry for secure fields.

macOS live validation should use the same native player bridge and winit event
source. The helper-process AppKit sample is also diagnostic only.

## Design Deviations

No intentional architecture deviation. The current architecture intentionally
uses one live text source per native player window: winit for the normal player,
with TSF/AppKit samples kept out of that path unless a future cut replaces the
source end to end.
