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
`TextInputDispatchState` contract, and keeps Windows TSF/AppKit/native handles
inside native/player or desktop-native crates. Sans I/O crates continue to see
only Arcweft text-input snapshots, geometry snapshots, host commands, and routed
`TextInput` batches.

## Implemented

- Added `NativeTextInputBridge`, backend boundary, window-handle extraction, and
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
not reliably become platform text events through the TSF bridge alone.

The native player now treats winit `WindowEvent::Ime` as the primary windowed
text event source for the normal `arcw run --runner native` path. It enables and
updates IME state from the prepared Arcweft snapshot/geometry, routes winit
preedit/commit/disable/delete-surrounding events into the shared player-owned
editor, and keeps secure controls from publishing surrounding text. While this
winit path is available, the older TSF platform-event drain is suppressed to
avoid duplicate partial commits; the TSF bridge still owns backend focus,
geometry publication, capability trace, and Windows-specific diagnostics.

The text-input contract also now has `TextDeleteUnit::Utf8Byte` so winit's
UTF-8 byte `DeleteSurrounding` event is represented exactly instead of being
approximated as scalar or grapheme deletion.

## Explicit Gap

Final Windows acceptance still needs a pinned real Japanese IME trace captured
from the normal native player after this event-path update. macOS acceptance
remains blocked until the AppKit in-window backend is attached; the
helper-process AppKit sample is diagnostic only.

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

Executed on 2026-07-02 after the winit IME event-path update:

- `cargo fmt --all` passed.
- `cargo test -p arcweft-presentation text_editor::tests::delete_surrounding_utf8_byte_unit --lib` passed.
- `cargo test -p arcweft-player-native scene_windowed::tests --lib` passed.
- `cargo test -p arcweft-player-scene text_input --lib` passed.
- `cargo test -p arcweft-runtime-host player_text_input_bridge --lib` passed.
- `cargo test -p arcweft-ui text_field --lib` passed.
- `cargo check -p arcweft-player-native -p arcweft-player-scene -p arcweft-runtime-host -p arcweft-presentation -p arcweft-ui --all-targets` passed.
- `cargo clippy -p arcweft-player-native -p arcweft-player-scene -p arcweft-runtime-host -p arcweft-presentation -p arcweft-ui --all-targets -- -D warnings` passed.
- `cargo build -p arcweft-cli --features native-player` passed.
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` passed with `0 error(s), 124 warning(s)` across 1,046 Rust files and 491,589 Rust physical LOC.
- `git diff --check` passed.

Windows live validation:

```powershell
cargo run -p arcweft-cli --features native-player -- run `
  --runner native samples/native-text-input/src/main.arcw `
  --text-input-trace-out fixtures/windows-tsf-real-ime/microsoft-japanese-ime-hiragana.player.json
```

Expected live evidence after renderer-backed text input geometry exists:

- backend `windows_tsf` with real capability records;
- focus records for distinct controls and focus generations;
- platform events for composition, commit, selection, deletion, and commands;
- routed `TextInput` records through the player path;
- non-zero screen caret/character geometry for plain fields;
- `secure_redacted=true` and no plaintext/character geometry for secure fields.

macOS live validation should use the same native player bridge once the AppKit
in-window backend is attached. The helper-process AppKit sample is also
diagnostic only.

## Design Deviations

No intentional architecture deviation. The only incomplete runtime behavior is
the explicit `None` renderer hook described above.
