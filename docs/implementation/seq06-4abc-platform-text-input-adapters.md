# Seq06.4a-c Platform Text Input Adapters

Source packages:

- `D:/sanze/Downloads/arcweft-seq06.4a-web-editcontext-text-input-adapter-2026-06-28.zip`
- `D:/sanze/Downloads/arcweft-seq06.4b-windows-tsf-text-input-adapter-2026-06-28.zip`
- `D:/sanze/Downloads/arcweft-seq06.4c-macos-nstextinputclient-text-input-adapter-2026-06-28.zip`

This cut applies the common substrate and implementation-ready safe adapter
cores for Web `EditContext`, Windows TSF, and macOS `NSTextInputClient`.

## Applied Scope

- Added `arcweft-presentation::text_index` as the canonical UTF-16/byte text
  boundary conversion owner for platform text-input adapters.
- Extended the shared text-input capability model with `Limited`,
  `VersionDependent`, `HostDependent`, and `SecureRedacted` so adapters do not
  flatten platform-specific facts into plain supported/unsupported booleans.
- Extended runtime-host activation so host commands carry adapter capabilities
  and geometry updates are emitted through `TextInputHostCommand::UpdateGeometry`.
- Added `arcweft-player-web::edit_context` for standards-based Web
  `EditContext` activation, event conversion, secure redaction, and fallback
  rejection.
- Added `arcweft-desktop-native::text_input::windows_tsf` safe TSF adapter core:
  ACP range conversion, capability facts, edit-session batching, display
  attributes, and geometry conversion. Real COM bootstrap remains excluded
  because the workspace forbids unsafe code.
- Added feature-gated `arcweft-desktop-native::text_input::macos_text_input`
  safe adapter core for AppKit range/event conversion and coordinate mapping,
  plus a Swift reference owner under
  `crates/arcweft-desktop-native/native/macos/`.
- Added Web/Windows/macOS fixture traces under `fixtures/`.

## Package Deviations

- The uploaded patches targeted an older `main` and did not apply directly.
  The implementation was ported to the current shared seq06.3 text-input
  contract instead of overwriting it.
- The final implementation uses a dedicated shared `text_index` module rather
  than platform-local conversion helpers. This is intentionally broader than
  the smallest possible patch because all platform adapters share the same
  UTF-16/native-position problem.
- Windows TSF real COM activation is not implemented in this cut. It requires a
  separately approved unsafe boundary for COM implementation and callbacks.
- macOS AppKit runtime validation cannot run from this Windows checkout. The
  Rust core is feature/target gated and the Swift file is reference ownership
  evidence until macOS validation is available.

## Validation Results

Executed from `D:/git/arcweft` on 2026-06-28:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-presentation --test text_index_snapshot --all-features
cargo test -p arcweft-runtime-host text_input --all-features
cargo test -p arcweft-player-web --lib edit_context --all-features
cargo test -p arcweft-player-web --test web_edit_context_source_gate --all-features
cargo test -p arcweft-desktop-native --test windows_tsf_text_input --all-features
cargo test -p arcweft-desktop-native --test macos_text_input_gate --all-features
cargo check -p arcweft-presentation -p arcweft-runtime-host -p arcweft-player-web -p arcweft-desktop-native --all-targets --all-features
cargo clippy -p arcweft-presentation -p arcweft-runtime-host -p arcweft-player-web -p arcweft-desktop-native --all-targets --all-features -- -D warnings
just test-workspace
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

Results:

- `text_index_snapshot`: 3 passed.
- `arcweft-runtime-host text_input`: 7 passed.
- `arcweft-player-web --lib edit_context`: 5 passed.
- `web_edit_context_source_gate`: 1 passed.
- `windows_tsf_text_input`: 8 passed.
- `macos_text_input_gate`: 1 passed on this non-macOS checkout.
- Focused four-crate `cargo check` passed.
- Focused four-crate `cargo clippy ... -D warnings` passed.
- `just test-workspace` passed.
- Structural audit scanned 1,680 files, 911 Rust files, 436,813 Rust physical
  LOC, with 0 errors and 107 existing warnings.
- `git diff --check` passed.

Additional platform validation:

- Web: run browser automation with a browser exposing `EditContext`; unsupported
  browser paths must report `UnsupportedNoFallback` and install no hidden DOM
  text entry fallback.
- Windows: validate TSF bootstrap, document manager/context creation,
  `ITextStoreACP` callbacks, and Japanese IME traces after an audited unsafe COM
  boundary is approved.
- macOS: run `cargo check/test/clippy -p arcweft-desktop-native --features
  macos-text-input` on macOS, then capture real Japanese IME AppKit callback
  traces and compare them to `fixtures/macos-nstextinputclient`.

## Remaining Work

- Add approved unsafe Windows COM bootstrap for TSF object implementation and
  sink callbacks.
- Wire macOS Swift/AppKit owner into the actual native window/view lifecycle on
  macOS.
- Add browser automation evidence for real `EditContext` objects.
- Add platform-captured IME fixtures for Windows and macOS.
