# Native IME Focus Loss Fix - 2026-07-06

## Problem

On Windows, the player-rendered multiline text control could enter IME
composition, lose OS window focus, and then stop responding after focus returned.
The visible symptom was that keyboard and button interactions no longer changed
the player state, and Windows could mark the Arcweft Player window as not
responding.

The same report also showed a small native white composition popup near the
caret while the Japanese IME candidate window was open.

## Root Cause

`InputController` treated active IME composition as a keyboard/action suppression
gate. `focus_changed(false)` cleared focused editors and pointer state, but it
did not resolve the active composition. If Windows delivered focus loss during
preedit, the editor could be removed while the composition gate stayed active,
suppressing subsequent keyboard/focus/action routes after focus returned.

The native window loop also briefly stopped issuing a focus-loss IME disable in
order to avoid re-entering the platform IME during blur. That avoided one risk
but left the OS IME context alive, which can make Windows fall back to a native
composition/candidate popup at stale or top-left coordinates.

## Implemented Fix

- `InputController::focus_changed(false)` now commits an active preedit
  composition before clearing focus, emits a `TextControlWriteBack::change`, and
  clears `ime_composing` together with focused editor, pointer, pressed, drag,
  and pending text-selection state. This matches browser-like blur behavior:
  the visible composition text becomes the committed control value when the user
  switches to another window.
- `NativeSceneState::sync_window_ime` now returns early while the OS window is
  not focused.
- `NativeSceneState::ime` ignores preedit, commit, delete-surrounding, and
  disabled composition routing while the OS window is not focused. `Ime::Enabled`
  still records support, but does not force a sync until focus is active.
- `NativeSceneState::focus_changed(false)` blurs the Arcweft text-input bridge
  after the scene-level commit, then disables the winit IME context so Windows
  does not keep an orphaned composition context around the unfocused player
  window.
- Added a regression test that proves focus loss during preedit commits the
  composition as a runtime text-control change and clears the composition
  suppression gate.

## Remaining Native Composition Popup

The small white popup is not an Arcweft-rendered text-control element. The active
native player backend is still `WinitWindowIme`.

The follow-up upstream-handling investigation found that Arcweft's pinned
`winit-win32 0.31.0-beta.2` masked `ISC_SHOWUICOMPOSITIONWINDOW` from
`WM_IME_SETCONTEXT`'s `wparam`, while the Win32 visibility flags live in
`lparam`. Arcweft now pins `winit` / `winit-win32` to a public
`Sanzentyo/winit` fork at commit
`fc9145a7b4054408d3aea5fb86c044e2ee35e2c9`, which is
`v0.31.0-beta.2` plus only the `lparam` mask correction. This keeps the
`request_ime_update` API and avoids downgrading to winit 0.30.13's older
`set_ime_allowed` / `set_ime_cursor_area` API.

When a fixed crates.io release becomes available, replace the fork pin with the
released `winit` version and remove the `[patch.crates-io]` entries.

The public `ImeRequestData` surface still only exposes hint/purpose, cursor
area, and surrounding text. It does not expose a TSF UI-element sink or a
separate "hide native composition window while keeping candidate UI" contract.

Therefore the freeze fix is implemented in the current backend, while complete
control over Windows native composition UI is split into:

- `docs/reviews/requests/2026-07-06-seq-06.16.7-native-player-windows-tsf-ime-backend.md`

## Validation

- `cargo test -p arcweft-player-scene focus_loss_commits_active_ime_composition -- --nocapture`
- `cargo test -p arcweft-player-scene input::tests -- --nocapture`
- `cargo check -p arcweft-player-native --all-targets`
- `cargo test -p arcweft-player-native --test native_text_input_bridge --quiet`
- `cargo test -p arcweft-player-native --test native_text_input_seq06_4j1_source_gate --quiet`
- `cargo run -p arcweft-cli -- check --manifest-path samples\modern-feedback-ui\arcw.toml`
- `cargo clippy -p arcweft-player-native -p arcweft-player-scene --all-targets`
- `cargo tree -p arcweft-player-native -i winit`
- `cargo tree -p arcweft-player-native -i winit-core`
