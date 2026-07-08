# seq06.4j.1 Cross-Platform Native IME Player-Rendered Sample And Validation

Date: 2026-06-29

## Status

Prepared overlay for the final native IME acceptance surface:

```bash
cargo run -p arcweft-cli --features native-player -- run \
  --runner native samples/native-text-input/src/main.arcw \
  --text-input-trace-out target/native-text-input-trace/native-player-ime.real.json
```

The sample runs through the normal native player path, uses product/runtime View
text-control resources, and publishes focused text targets through renderer
geometry.

## Implemented by this overlay

- Replaces the narrative native sample with deterministic player-rendered
  `TextField`, `TextArea`, and `SecureField` resources.
- Adds generic `.arcweft/content/view.*.json` sidecar loading for bundle View
  resources.
- Adds keyboard traversal over text-control semantic targets.
- Adds selected-backend and runtime-write-back trace records.
- Extends native backend identity with Wayland, Android, and iOS typed unavailable
  surfaces without leaking platform object identity.
- Documents diagnostics-only boundaries for backend harness binaries.

## Validation

Validated in the receiving checkout:

```bash
cargo check -p arcweft-cli -p arcweft-player-native -p arcweft-player-scene -p arcweft-render-wgpu --features arcweft-cli/native-player --all-targets
cargo test -p arcweft-cli --features native-player --test native_text_input_sample_sidecars --quiet
cargo test -p arcweft-player-native --test native_text_input_seq06_4j1_source_gate --quiet
cargo test -p arcweft-player-scene --test runtime_text_controls --quiet
cargo test -p arcweft-render-wgpu focused_text_input_target --all-features --quiet
cargo test -p arcweft-player-native native_text_input_bridge --quiet
cargo +nightly -Zscript tools/source-gates/seq06_4j1_native_ime_player_rendered_gates.rs --root .
just ime-sample-check
cargo fmt --all -- --check
cargo clippy -p arcweft-cli -p arcweft-player-native -p arcweft-player-scene -p arcweft-render-wgpu --features arcweft-cli/native-player --all-targets -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Real Windows/macOS/Linux/Android/iOS IME acceptance still requires real-machine
manual traces. Local traces must be written under `target/native-text-input-trace/`
unless deliberately reviewed and promoted into fixtures.
