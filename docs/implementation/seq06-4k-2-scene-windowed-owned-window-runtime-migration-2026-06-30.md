# seq06.4k.2 scene-windowed owned-window/runtime migration

Date: 2026-06-30
Applied checkout parent: `Sanzentyo/arcweft` `main` at `1114429ab0d030e48a81726f08e5775d793bb302`.

## Goal

Finish the seq06.4k.2 migration by making the normal `scene_windowed.rs` path own desktop owned-window behavior, main-thread adapter pumping, audio event routing, and close-request pumping. The legacy `windowed.rs` path must stay deleted, and the source gate must fail if it reappears.

## Implemented design

- `scene_windowed.rs` remains the owner of the winit event loop and the scene window.
- `window_driver.rs` is restored as the only owned-window/cursor implementation. It exposes only native-crate `pub(crate)` types:
  - `WindowCloseSignal` for runtime-requested close edges;
  - `WinitOwnedWindowDriver` for `OwnedWindowDriver` and cursor requests.
- `NativeSceneState::new` creates the primary `WindowCloseSignal` and `WinitOwnedWindowDriver`, installs the driver into `NativeDesktopBackend::builder().with_owned_window_driver(...)`, and then starts `WindowedRuntimeOwner` with that backend.
- `WindowedRuntimeOwner` now owns the session, image catalog, patch queue, native-task bridge, materialized bundle workspace, and pending host/audio events.
- `WindowedRuntimeOwner::from_bundle_with_desktop_backend` is the narrow construction API used by native scene code. It registers desktop adapters before the AWFB-backed session starts.
- `WindowedRuntimeOwner::pump_main_thread` runs HostMainThread adapter work and normalizes adapter completion events back to the `BundleSession` task dispatch sequence.
- `WindowedRuntimeOwner::push_audio_events` and `WindowedRuntimeOwner::step_with_clock` define the audio and task-event ingress boundary used by the scene.
- `NativeSceneState::redraw` pumps main-thread work before stepping runtime.
- `NativeSceneState::step_runtime` drains device/capture audio events into the owner before the step, submits runtime audio commands to `NativeAudioRuntime`, and pushes generated command events back into the owner for the next step.
- `NativeSceneApp::about_to_wait` checks `WindowCloseSignal::take()` and exits the event loop on an owned-window `request_close`.
- `tools/source-gates/seq06_4k_text_input_windowed_gates.rs` now requires `windowed.rs` to be absent unconditionally and checks for the new owned-window/runtime migration hooks.

## Files changed by the patch

- `crates/arcweft-player-native/src/lib.rs`
- `crates/arcweft-player-native/src/scene_windowed.rs`
- `crates/arcweft-player-native/src/window_driver.rs`
- `crates/arcweft-player-native/src/windowed_runtime.rs`
- `tools/source-gates/seq06_4k_text_input_windowed_gates.rs`
- `docs/implementation/seq06-4k-2-scene-windowed-owned-window-runtime-migration-2026-06-30.md`

## Package drift

The package patch was malformed for `git apply`, and the package apply script stopped after updating `lib.rs` because the inspected `scene_windowed.rs` shape had more than one `window: Arc<dyn Window>` insertion point. The migration was integrated manually against the current checkout. The resulting code follows the package contract but uses the current source shape and was cargo-validated locally.

## Validation

Passed in this checkout:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-player-native -p arcweft-cli --all-targets --all-features
cargo test -p arcweft-player-native --all-features -- --nocapture owned_window
cargo clippy -p arcweft-player-native -p arcweft-cli --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
cargo +nightly -Zscript tools/source-gates/seq06_4k_text_input_windowed_gates.rs --root .
git diff --check
```

Manual desktop validation also remains to run on a workstation with a display server/audio device:

```bash
cargo run -p arcweft-cli -- run samples/desktop-owned-window-demo.arcw --runner native --watch=false
cargo run -p arcweft-cli -- run samples/desktop-owned-window-close.arcw --runner native --watch=false
```

Expected manual evidence:

- `set_title`, `set_bounds`, cursor icon, fullscreen/normal mode, and `request_close` operate through the normal scene window.
- Native main-thread tasks are pumped while the scene window is open.
- Audio commands/events still flow through `NativeAudioRuntime` and `WindowedRuntimeOwner`.
- Live-patch ingress remains bounded by `WindowedRuntimeOwner` after render submission.
- No public `run_bundle_adapter_windowed` export and no `crates/arcweft-player-native/src/windowed.rs` file remain.

## Remaining TODOs

- Run the listed manual desktop commands on a workstation with an interactive display and audio device.
- If future live patching changes adapter manifests at runtime, decide whether the native-task bridge policy should be rebuilt during restart-required patch commits. This patch preserves the initial adapter bridge across runtime restart, matching the current owned-window lifetime requirement.
