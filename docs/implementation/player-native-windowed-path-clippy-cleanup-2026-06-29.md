# Player Native Windowed Path Clippy Cleanup - 2026-06-29

## Reason

Workspace all-features clippy was blocked by `arcweft-player-native` dead-code
warnings in the old `windowed.rs` path and its private `native_audio` /
`window_driver` support. The public native player entrypoints already route
through `scene_windowed.rs`, so keeping the old module compiled created an
unused compatibility path.

## Changes

- Removed the old `windowed.rs` module and its private `window_driver.rs`.
- Kept the current `scene_windowed.rs` player-owned window path as the only
  `run_bundle_windowed` implementation.
- Moved native audio runtime ownership into `NativeSceneState`.
- Threaded drained audio events into `BundleStepInput` and submitted runtime
  audio commands back to `NativeAudioRuntime`.
- Updated the text-input bridge source gate to check the current
  `PlayerTextInputBridgeCore` integration point instead of the removed
  implementation detail name.

## Validation

Passed:

```bash
cargo fmt --all -- --check
git diff --check
cargo check -p arcweft-player-native --all-targets --all-features
cargo clippy -p arcweft-player-native --all-targets --all-features -- -D warnings
cargo clippy -p arcweft-cli --all-targets --all-features -- -D warnings
cargo test -p arcweft-player-native --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The structural audit reported 0 errors and 119 warnings after the old modules
were removed.

