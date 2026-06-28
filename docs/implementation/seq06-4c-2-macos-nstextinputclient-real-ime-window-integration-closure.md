# Seq06.4c.2 macOS NSTextInputClient real IME window integration closure

## Applied design

This overlay adds a macOS-only AppKit helper process and a native sample runner for validating Arcweft text input through a live `NSView<NSTextInputClient>`.

The helper-process boundary is intentional: the current workspace forbids Rust `unsafe`, and the existing Swift file was only reference material. The new path builds Swift with `swiftc` on macOS, keeps AppKit identity in Swift, and sends callback facts to Rust over JSON-lines.

## Ownership

- Swift owns AppKit objects, native ranges, attributed strings, selectors, responder-chain identity, and synchronous AppKit callback return values.
- `arcweft-desktop-native::text_input::macos_text_input` owns native-range conversion into Arcweft text-input events.
- `arcweft-runtime-host::TextInputDispatchState` owns focus generation, serial validation, secure privacy tagging, and stale callback rejection.
- `arcweft-presentation::text_editor::TextEditorState` owns mutation, movement, deletion, selection, replacement, clipboard policy, composition state, and geometry snapshots.

## Implemented files

- `crates/arcweft-desktop-native/build.rs`
- `crates/arcweft-desktop-native/src/text_input/macos_appkit_bridge.rs`
- `crates/arcweft-desktop-native/native/macos/ArcweftTextInputClientView.swift`
- `crates/arcweft-player-native/examples/macos_nstextinputclient_real_ime.rs`
- `crates/arcweft-desktop-native/tests/macos_appkit_source_gate.rs`

## Validation status

This checkout validated the Rust bridge wiring, Windows-host source gates, and
native-player target integration with Cargo. AppKit, Xcode, and a macOS Japanese
IME session were not available on this Windows host, so real macOS execution and
trace capture remain pending.

Required macOS validation:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-desktop-native --features macos-appkit-text-input --all-targets
cargo run -p arcweft-player-native --example macos_nstextinputclient_real_ime --features macos-appkit-ime-sample -- --mode text-field
cargo run -p arcweft-player-native --example macos_nstextinputclient_real_ime --features macos-appkit-ime-sample -- --mode text-area
cargo run -p arcweft-player-native --example macos_nstextinputclient_real_ime --features macos-appkit-ime-sample -- --mode secure-field
cargo test -p arcweft-desktop-native --test macos_appkit_source_gate --all-features
cargo clippy -p arcweft-desktop-native -p arcweft-player-native --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

## Remaining validation evidence

- Real Japanese IME callback trace capture on macOS.
- Xcode SDK header excerpt capture for `NSTextInputClient` method signatures and `NSRange` behavior.
- Optional in-window winit/wgpu embedding if the project later approves a dedicated unsafe macOS window-handle boundary.
