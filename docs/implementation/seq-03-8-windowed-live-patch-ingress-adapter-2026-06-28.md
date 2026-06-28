# Seq-03.8 - Windowed Live-Patch Ingress Adapter

Date: 2026-06-28
Package: `arcweft-seq03.8-windowed-live-patch-ingress-closure-audit-2026-06-28.zip`

## Result

Seq-03.8 is implemented for the local native watch / event-loop ingress path.

The native windowed player now exposes a cloneable adapter-owned
`WindowedPatchIngress` handle. Local producers can enqueue typed
`WindowedPatchEvent` payloads without owning `BundleSession`,
`BundleImageCatalog`, renderer state, or window state. The visible running
window remains the only owner that accepts queued patch events and commits them
at `FrameBoundary::AfterRenderSubmitted`.

## Files changed

- `crates/arcweft-player-native/src/windowed_ingress.rs`
  - Adds `WindowedPatchIngress`, `WindowedPatchIngressConfig`,
    `WindowedPatchIngressAccepted`, `WindowedPatchIngressReport`,
    `WindowedPatchIngressReportState`, `WindowedPatchIngressErrorKind`,
    and `WindowedPatchIngressError`.
  - Implements bounded FIFO reservation with default capacity 32.
  - Reports `QueueFull`, `EventLoopClosed`, `PlayerClosed`,
    malformed sidecar, wrong-base, and unsupported action errors.
- `crates/arcweft-player-native/src/scene_windowed.rs`
  - Creates the ingress handle beside the `winit` event loop.
  - Drains ingress messages from `proxy_wake_up` and `about_to_wait`.
  - Marks events accepted by the event loop before forwarding to
    `WindowedRuntimeOwner::push_patch_event`.
  - Marks completion only after `FrameBoundary::AfterRenderSubmitted`.
  - Closes the ingress report when the scene exits or fails.
- `crates/arcweft-player-native/src/lib.rs`
  - Exports the public ingress adapter API and
    `run_bundle_windowed_with_ingress`.
- `crates/arcweft-cli/src/app/runtime/run.rs`
  - Routes `arcw run --watch --runner native` through the running windowed
    player and its `WindowedPatchIngress` handle.
  - Keeps polling, rebuild, patch generation, artifact writing, and transport
    sidecar emission in CLI code.
  - Removes native watch mutation through a standalone `NativePatchEndpoint`.

## Architecture

```text
arcw run --watch --runner native
        |
        | CLI-owned file polling / rebuild / patch bytes
        v
WindowedPatchIngress
        |
        | bounded FIFO + EventLoopProxy::wake_up()
        v
NativeSceneApp::proxy_wake_up / about_to_wait
        |
        | event-loop owner only
        v
WindowedRuntimeOwner::push_patch_event
        |
        | render submitted
        v
WindowedRuntimeOwner::drain_patch_boundary(FrameBoundary::AfterRenderSubmitted)
```

`arcweft-runtime-driver` remains Sans I/O. Release fetch, signing, trust
policy, sockets, HTTP mirrors, and durable publication are outside this slice.

## Apply-time decision

The package request described a `winit::EventLoopProxy` custom event carrying
typed payloads. Current main uses `winit 0.31.0-beta.2`, where the active API is
`EventLoopProxy::wake_up()` plus `ApplicationHandler::proxy_wake_up`; it does
not carry a custom user-event payload. The production implementation therefore
uses:

- a typed `std::sync::mpsc` FIFO owned by `arcweft-player-native`;
- a cloneable public `WindowedPatchIngress` producer handle;
- `EventLoopProxy::wake_up()` only as the wake signal;
- `NativeSceneApp` as the sole consumer that drains typed messages and forwards
  them to `WindowedRuntimeOwner`.

This preserves the requested ownership boundary and avoids stringly dispatch
without depending on an unavailable winit custom-event API.

## Acceptance status

| Requirement | Status | Evidence |
| --- | --- | --- |
| Cloneable adapter-owned ingress handle | implemented | `WindowedPatchIngress` in `windowed_ingress.rs` |
| Local native watch producer enqueues into the running window | implemented | `run_native_windowed_watch_target` and `push_patch_bundle_bytes` in `run.rs` |
| Runtime/session/catalog mutation remains event-loop owned | implemented | `NativeSceneState::apply_ingress_message` forwards to `WindowedRuntimeOwner` |
| Safe mutation only after render submission | implemented | `drain_patch_events_after_render_submitted` calls `FrameBoundary::AfterRenderSubmitted` |
| FIFO and backpressure semantics | implemented | default capacity 32, `QueueFull`, sequence numbers, completion release |
| Typed retained adapter errors | implemented | `WindowedPatchIngressErrorKind` and `WindowedPatchIngressReportState` |
| Runtime-driver remains Sans I/O | preserved | no runtime-driver transport/watch changes |

## Validation

Validated in this checkout:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-player-native --lib windowed_ingress --all-features -- --nocapture
cargo test -p arcweft-player-native --lib scene_windowed --all-features -- --nocapture
cargo test -p arcweft-player-native --lib windowed_patch --all-features -- --nocapture
cargo test -p arcweft-player-native --lib windowed_runtime --all-features -- --nocapture
cargo test -p arcweft-cli --lib watch --all-features -- --nocapture
cargo +nightly -Zscript tools/regenerate-windowed-live-patch-fixtures.rs --check
cargo test -p arcweft-player-native --test windowed_live_patch_smoke --all-features -- --nocapture
cargo check -p arcweft-player-native -p arcweft-runtime-driver -p arcweft-cli --all-targets --all-features
cargo clippy -p arcweft-player-native -p arcweft-runtime-driver -p arcweft-cli --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Observed results:

- ingress/scene/windowed patch/windowed runtime focused lib tests passed;
- CLI watch source and transport tests passed;
- seq03.7 generated live-patch fixtures are current;
- windowed live-patch smoke integration tests passed;
- multi-crate check passed;
- clippy passed with `-D warnings`;
- structural audit scanned 1,846 files, 957 Rust files, 456,842 Rust LOC, with
  0 errors and 113 warnings.

## Remaining TODOs

- Manual native GUI smoke for `arcw run --watch --runner native` still requires
  a desktop session where the window can be opened, edited, observed, and
  closed. The non-interactive validation above proves the adapter, routing,
  fixture, and owner boundaries.
