# Seq06.4b.2 Windows TSF real IME window integration closure

This implementation note accompanies the seq06.4b.2 overlay package.

## Scope

The overlay adds the Windows-only production object boundary for native TSF IME
sessions:

- `arcweft-desktop-native::text_input::windows_tsf::real_ime` safe owner.
- `arcweft-desktop-native::text_input::windows_tsf::unsafe_com` audited unsafe
  COM implementation boundary.
- Windows-only lifecycle tests and non-Windows source gates.
- Native sample binary `windows-tsf-ime-sample` and trace output.

## Architecture

The bridge activates `ITfThreadMgr`, creates a document manager, creates an
`ITextStoreACP` COM object backed by Arcweft snapshots, and installs a context
for the focused Arcweft text control. Mutating TSF callbacks become existing
`PlatformTextInputEvent::Batch` values and pass through runtime-host validation
before shared editor mutation.

No Windows COM identity is stored in `arcweft-presentation`,
`arcweft-runtime-host`, `arcweft-ui`, or replay/capture data.

## Integration adjustments

The applied implementation was reconciled with the current workspace rather than
blindly copying the package hunk:

- `windows` 0.62.2 generated bindings use return-value oriented
  `ITextStoreACP_Impl` signatures and typed HRESULT constants, so the COM
  boundary was updated to those APIs.
- TSF state is owned on the single-threaded apartment via `Rc<RefCell<_>>`
  instead of pretending to be cross-thread with `Arc<Mutex<_>>`.
- RAII-only COM fields are named with `_` prefixes and have comments explaining
  that they intentionally keep the apartment, context, edit cookie, and text
  store alive.
- The sample uses winit 0.31's `Arc<dyn Window>` shape and retrieves `HWND`
  through `raw-window-handle` 0.6.

## Security

Secure fields install redacted snapshots and geometry at the Windows boundary:
empty surrounding text, collapsed selection, no composition segments, no
character bounds, and no selected-range anchors. Trace output records secure
operation kinds but not text, native ranges, object identity, or candidate
character geometry.

## Validation status

This checkout compiled the Windows-only TSF COM boundary and sample on Windows
and ran the source/lifecycle tests listed below. A real Microsoft Japanese IME
interactive trace was not captured in this run. The remaining evidence gate is
running the sample with Microsoft Japanese IME enabled and capturing real
preedit, candidate movement, commit, deletion, selection movement, focus loss,
and secure redaction behavior.

## Required commands

```bash
cargo fmt --all -- --check
cargo check -p arcweft-desktop-native --target x86_64-pc-windows-msvc --all-targets --all-features
cargo test -p arcweft-desktop-native --test windows_tsf_text_input --all-features
cargo test -p arcweft-desktop-native --test windows_tsf_real_ime_source_gate --all-features
cargo test -p arcweft-desktop-native --test windows_tsf_real_ime_lifecycle --target x86_64-pc-windows-msvc --all-features
cargo run -p arcweft-player-native --bin windows-tsf-ime-sample -- --trace-out fixtures/windows-tsf-real-ime/microsoft-japanese-ime-hiragana.real.json
cargo clippy -p arcweft-desktop-native -p arcweft-player-native --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```
