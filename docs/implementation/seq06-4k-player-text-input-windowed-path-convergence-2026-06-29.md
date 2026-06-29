# seq06.4k player text-input / windowed path convergence

Date: 2026-06-29
Audited revision: `33bef36476b8558551ff9f6010f11a44cbf000c4`

## Summary

This cleanup cut introduces `arcweft-runtime-host::player_text_input_bridge` as the shared native/Web player text-input core. The design makes `TextInputDispatchState` the single owner of session/generation validation, redaction, blur policy, shortcut admission, and dispatch-to-`TextInput` conversion. Native and Web remain host shells for TSF/AppKit/EditContext/unavailable backends and command publication.

## Implemented in package overlay

- New shared core: `crates/arcweft-runtime-host/src/player_text_input_bridge.rs`.
- Source gates: `tools/source-gates/seq06_4k_text_input_windowed_gates.rs`.
- Code patch for native/Web shell convergence.
- Documentation and follow-up requests.

## Text-input lifecycle after the patch

```text
PreparedFrame focused text target
  -> PlayerTextInputBridgeCore::sync_focus
  -> TextInputDispatchState activate/update/update_geometry/blur
  -> native/Web host command sink
  -> platform event source
  -> PlayerTextInputBridgeCore::dispatch_platform_event
  -> InputController::text_input
```

Shortcut suppression is decided by `PlayerTextInputBridgeCore::shortcuts_allowed`, which delegates to `TextInputDispatchState::shortcuts_allowed`.

## Geometry

The shared core accepts `TextInputGeometrySnapshot`. Native consumes screen geometry directly; Web converts viewport geometry to client coordinates in the Web shell. No DOM/native handles are stored in Sans I/O crates.

## `PreparedFrame` focus-target blocker

The current schema cannot produce real focused text targets: `PreparedTextInputTarget` exists, and shared editor/geometry code exists, but current `RenderScene` has no production text-control value/session/selection input. The package includes `2026-06-29-seq-06.4k.1-real-text-control-schema-to-prepared-frame.md` instead of fabricating a target.

## `windowed.rs` blocker

`windowed.rs` remains unsafe to delete because it still contains owned-window/cursor registration, native task pumping, audio command/event handling, and close signal behavior not proven migrated to the scene path. The package removes the public compatibility export in the patch and includes `2026-06-29-seq-06.4k.2-scene-windowed-owned-window-runtime-migration.md` for final deletion.

## Validation status

The original package did not run cargo/npm validation in its generation
environment. The application in this checkout was validated after adapting the
package patch to current `main`.

Commands run in this checkout:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-runtime-host -p arcweft-player-native -p arcweft-player-web --all-targets --all-features
cargo test -p arcweft-runtime-host player_text_input --all-features -- --nocapture
cargo test -p arcweft-player-web text_input --all-features -- --nocapture
cargo test -p arcweft-player-web edit_context --all-features -- --nocapture
cargo test -p arcweft-player-native text_input --all-features -- --nocapture
cargo +nightly -Zscript tools/source-gates/seq06_4k_text_input_windowed_gates.rs --root .
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

Results:

- `cargo fmt --all -- --check`: passed.
- focused cargo check: passed, with dead-code warnings from the legacy native
  windowed/audio/window-driver path after removing the public compatibility
  export.
- runtime-host shared bridge tests: passed, 4 tests.
- Web runtime/edit-context text-input tests: passed, 12 focused tests plus the
  Web source-gate filtered tests selected by the `text_input` filter.
- native text-input tests/source gates selected by the `text_input` filter:
  passed.
- seq06.4k source gate: passed.
- structural audit: passed with `0 error(s), 117 warning(s)`.
- `git diff --check`: passed.

The command below was also run:

```bash
cargo clippy -p arcweft-runtime-host -p arcweft-player-native -p arcweft-player-web --all-targets --all-features -- -D warnings
```

After fixing new seq06.4k lint findings, it still fails only on pre-existing
legacy native-windowed responsibility code now exposed as dead code:

- `native_audio.rs`: `NativeAudioRuntime` and microphone/audio helpers.
- `window_driver.rs`: `WindowCloseSignal`, `WinitOwnedWindowDriver`, and winit
  owned-window helper functions.
- `windowed.rs`: old `BundleWindowDriver` and `run_bundle_windowed`.

This is intentionally not hidden with `#[allow]`/`#[expect]`; it is the exact
seq06.4k.2 blocker. The old public export
`run_bundle_adapter_windowed` is removed, but `windowed.rs` itself remains until
owned-window, native-task, and audio behavior are migrated into the normal
`scene_windowed.rs` / `WindowedRuntimeOwner` path.

## Applied deviations from the package patch

The package patch file was not directly applicable to this checkout, so the
same design was ported manually:

- `arcweft-runtime-host::player_text_input_bridge` is added as the shared
  native/Web lifecycle and dispatch core.
- Native keeps backend and trace ownership, but focus sync, blur policy,
  dispatch validation, epoch allocation, and shortcut admission now go through
  `PlayerTextInputBridgeCore`.
- Web `EditContextAdapter` now normalizes browser callback state into
  `PlatformTextInputEvent` and no longer owns `TextInputDispatchState`.
- Web `WebPlayerTextInputBridge` owns the shared core and keeps only JS command
  publication, browser coordinate conversion, pending edit queue, and registry
  identity.
