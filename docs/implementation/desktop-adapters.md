# Desktop Adapter Implementation Status

Last updated: 2026-06-19.

## Implemented Boundary

Desktop OS access is implemented as host adapters, not as Arcweft core behavior.
`arcweft-core` remains Sans I/O; task requests and task events are unchanged.

The implementation is split into:

- `arcweft-desktop-contract`: serializable request/response, platform, geometry, window, pointer, and file-grant value types.
- `arcweft-desktop-host`: Sans I/O dispatch, host-main-thread queueing, pending completion, cancellation, and an in-memory backend for deterministic tests.
- `arcweft-desktop-native`: native backend for capability negotiation, user file grants/dialogs, known-directory policy, optional global pointer, optional external window observation, and optional external-window control driver hooks.
- `arcweft-adapter-desktop`: ten logical Arcweft adapter manifests plus JSON codec and host-adapter bindings.

`arcweft-host-adapter` now supports both synchronous completion and pending host-main-thread work:

- `HostAdapter::submit`
- `HostAdapter::drain_completions`
- `HostAdapter::cancel`
- `HostAdapter::pump_main_thread`

Synchronous adapters continue to implement `complete`; `submit` wraps that result. This is the current task boundary, not a compatibility shim.

## Authority Model

The ten logical desktop adapters are registered by the native CLI/player host, but registration does not grant authority. A host call still needs to be allowed by the active manifest-derived policy.

The CLI standard native policy includes only these desktop manifests:

- `desktop-platform`
- `desktop-window-owned`
- `desktop-files-user-read`

Bundle packaging adds desktop manifests only for required `desktop.*` host calls. High-authority calls such as global pointer and external-window control therefore require an explicit host call in the program and still remain disabled by native backend feature flags and host policy unless enabled by the embedding host.

The current headless/offscreen native player has no owned-window driver. Owned-window and owned-cursor capabilities correctly report `unsupported`; no fake no-op window implementation is installed.

## Windows Validation

Validated on Windows on 2026-06-19.

Commands run:

```bash
cargo test -p arcweft-desktop-contract -p arcweft-desktop-host -p arcweft-desktop-native -p arcweft-adapter-desktop
cargo test -p arcweft-runtime-host
cargo run -p arcweft-cli --bin arcw -- run samples/desktop-capabilities.arcw --mode drain --steps 8 --json
```

Observed results:

- Desktop crate tests passed, including native host-registry integration.
- Runtime-host tests passed after pending completion support was added.
- `samples/desktop-capabilities.arcw` completed one native desktop task.
- Capability response reported `"platform":"windows"`.
- `user_file_dialog` reported `supported_with_user_consent`.
- `granted_file_io` reported `supported`.
- `owned_window_*`, `owned_cursor_control`, `global_pointer_*`, and `external_window_*` reported `unsupported` in the default headless CLI backend, as expected.

Bundle path was also validated with a temporary `.awfb`:

```bash
cargo run -p arcweft-cli --bin arcw -- bundle <desktop-capabilities.arcw> --output <main.awfb> --json
cargo run -p arcweft-cli --bin arcw -- run-bundle <main.awfb> --steps 8 --json
```

Observed results:

- Bundle summary included `required_host_calls: ["desktop.platform.capabilities"]`.
- Bundle summary included two adapter manifests: the selected Sans I/O manifest and `desktop-platform`.
- `run-bundle` completed one native desktop task and returned the Windows capabilities response.

## Other Platform Validation

macOS, Linux X11, Linux Wayland, and Web were not live-validated in this Windows pass.

Use the same non-interactive baseline first:

```bash
cargo test -p arcweft-desktop-contract -p arcweft-desktop-host -p arcweft-desktop-native -p arcweft-adapter-desktop
cargo test -p arcweft-runtime-host
cargo run -p arcweft-cli --bin arcw -- run samples/desktop-capabilities.arcw --mode drain --steps 8 --json
```

Expected platform-specific checks:

- macOS: capabilities should report `macos`; file dialogs should be user-consent gated; global pointer and external window observation require explicit feature/policy and OS privacy authorization.
- Linux X11: capabilities should report `linux_x11`; file dialogs depend on the desktop portal/RFD backend; global pointer support is best-effort only when the feature and host policy are enabled.
- Linux Wayland: capabilities should report `linux_wayland`; global pointer and generic external-window observation/control should remain unsupported.
- Web: the native backend is not the target backend; validate a web-specific backend separately and keep native desktop crates out of the browser runtime.

Interactive file-dialog validation must be run manually because it opens OS UI:

1. Use `desktop.files.user.read` with a `ShowDialog` request.
2. Select a file.
3. Confirm the response contains only an opaque grant id and relative metadata, not an absolute path.
4. Read through the grant and confirm bytes/text are returned.
5. Revoke the grant and confirm later reads fail with a structured stale/permission error.

Known-directory validation requires host policy to allow a directory family first; consuming a known-directory grant must still go through the user file read/write adapter path.

## Remaining Platform Work

- Install a real owned-window driver in an event-loop native player before marking owned-window/cursor operations supported.
- Add live macOS/Linux validation results when those environments are available.
- Keep persistent file grants unsupported until a sealed, platform-specific token store is designed and implemented.
- Enable `global-pointer` and `external-window-observe` only in explicitly privileged builds and document the host policy used for each validation run.
