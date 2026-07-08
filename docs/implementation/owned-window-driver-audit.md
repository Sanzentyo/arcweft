# Owned Window Driver Audit

Last updated: 2026-06-21.

## Source Package

This audit tracks the integration of:

```text
D:/sanze/Downloads/arcweft-owned-window-driver.zip
```

The package requested a native player owned-window/cursor driver that is only
installed for the windowed `.awfb` path after winit creates the primary window.

## Acceptance Evidence

| Requirement | Evidence |
| --- | --- |
| Headless native backend has no owned-window driver | `run_bundle_headless` still registers `NativeDesktopBackend::builder().build()` without `with_owned_window_driver`. |
| Windowed `.awfb` path installs the driver after window creation | `run_driven_frames_window` creates the winit window, then calls `NativeWindowLoopDriver::attach_window`; `BundleWindowDriver::attach_window` installs `WinitOwnedWindowDriver`. |
| Runtime advances from the event loop one step at a time | `BundleRunnerSession::step` executes at most one runtime step; `BundleWindowDriver::event_loop_turn` calls it once per turn when not waiting for presentation advance. |
| Host-main-thread requests are pumped while presentation is paused | `BundleWindowDriver::event_loop_turn` calls `BundleRunnerSession::pump_main_thread` before checking `waiting_for_advance`. |
| Native handles do not cross runtime/Sans I/O boundaries | The driver exposes only `WindowId("owned:primary")`; no winit id or OS handle is serialized into `WindowSnapshot`. |
| Owned window requests are implemented | `WinitOwnedWindowDriver::execute_window` handles `List`, `Get`, `SetTitle`, `SetVisible`, `SetMode`, `SetBounds`, `RequestFocus`, and `RequestClose`. |
| Owned cursor requests are implemented | `WinitOwnedWindowDriver::execute_cursor` handles `SetIcon`, `SetVisible`, `SetGrab`, and `SetPosition`. |
| Borderless fullscreen and exclusive fullscreen are distinct | `WindowMode::BorderlessFullscreen` serializes as `borderless_fullscreen` and maps to winit borderless fullscreen; `WindowMode::Fullscreen` maps to winit exclusive fullscreen using the best available current-monitor video mode. |
| Absolute placement is platform gated | `platform_supports_absolute_position` returns true only for Windows, macOS, and Linux X11; Wayland and non-desktop hosts return unsupported. |
| Platform failures are not reported as success | Cursor grab/position map winit errors to `DesktopError::Platform`; invalid/unsupported bounds map to structured desktop errors. |
| Global pointer and external window authority are unchanged | The integration only installs an owned-window driver; no global pointer or external-window control driver is added. |
| Owned-window source API is typed at the adapter boundary | The desktop manifest injects functions such as `desktop.window.owned.set_title(title: String)`, `set_bounds(x: i32, y: i32, width: u32, height: u32)`, `set_mode(mode: WindowMode)`, and `request_close()` returning `Need<String, DesktopError>`; source code passes enum variants such as `.BorderlessFullscreen`, not JSON request strings. |
| Owned-cursor source API uses typed enums | `desktop.cursor.owned.set_icon(icon: CursorIcon)` accepts enum variants such as `.Pointer`; string icon names are rejected by the typed host-call decoder. |
| Generic custom host calls preserve named payloads | `HostTaskRequest::Custom` now carries both positional payloads and `named_args`, and host adapters decode payloads through shared typed helpers. The desktop adapter no longer has a generic JSON request fallback. |
| Windowed owned-window requests have a product smoke fixture | `samples/desktop-owned-window-close.arcw` bundles with `required_host_calls: ["desktop.window.owned.request_close"]` and exits the native windowed player through `RequestClose`. |

## Validation Run

Passed:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-desktop-contract -p arcweft-player-native
cargo check -p arcweft-runtime-host -p arcweft-render-native -p arcweft-player-native
cargo test -p arcweft-desktop-contract -p arcweft-player-native --lib
cargo test -p arcweft-runtime-host
cargo test -p arcweft-player-native --lib
cargo test -p arcweft-render-native --lib
cargo test -p arcweft-player-native
cargo clippy -p arcweft-desktop-contract -p arcweft-runtime-host -p arcweft-render-native -p arcweft-player-native --all-targets --all-features -- -D warnings
cargo check --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-workspace
just test-fast
cargo run -p arcweft-cli --bin arcw -- check samples/desktop-owned-window-close.arcw --json
cargo run -p arcweft-cli --bin arcw -- bundle samples/desktop-owned-window-close.arcw --output target/codex-owned-window/owned-window-close.awfb --json
target/debug/arcweft-player-native.exe target/codex-owned-window/owned-window-close.awfb
cargo run -p arcweft-cli --bin arcw -- check samples/desktop-owned-window-demo.arcw --json
cargo run -p arcweft-cli --bin arcw -- bundle samples/desktop-owned-window-demo.arcw --output target/codex-owned-window/owned-window-demo.awfb --json
target/debug/arcweft-player-native.exe target/codex-owned-window/owned-window-demo.awfb
cargo test -p arcweft-adapter-context -p arcweft-host-adapter -p arcweft-adapter-desktop
```

Also scanned the added Rust files for `unsafe`, `Box::leak`, `mem::forget`,
`todo!`, `unimplemented!`, `transmute`, and `#[allow]`; none were introduced.

Known validation caveats:

- `cargo test --workspace --quiet` was attempted and failed in
  `arcweft-cli --test check` because the broad default CLI integration suite
  selects native capture tests without the `native-capture` feature. A
  representative exact failure was
  `agent_observe_native_renderer_writes_framebuffer_png`, which reported:
  `this arcw agent command requires the native-capture feature`.
- `just test-cli-check` was attempted and failed in the existing Agent observe
  JSON route:
  `agent_observe_json_reports_rich_text_reset_controls_and_host_markers`.
  That failure is outside the owned-window/player-native integration surface.

## Remaining TODOs

- Live-validate owned window/cursor behavior on Windows, macOS, Linux X11, and
  Linux Wayland with a fuller interactive windowed fixture. The current Windows
  live smoke covers bundle execution through owned-window `RequestClose`.
- Add a direct automated windowed integration fixture once the native test
  harness can drive a real event loop without manual View interaction.
- Add typed source functions and payload decoders for the remaining desktop
  file/grant host-call surfaces; the desktop adapter no longer accepts generic
  JSON request strings for those calls.
