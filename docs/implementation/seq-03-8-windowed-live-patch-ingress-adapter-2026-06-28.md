# Seq-03.8 — Windowed Live-Patch Ingress Adapter

Date: 2026-06-28
Package: `seq-03.8-windowed-live-patch-ingress-adapter-package`

## Goal

Add a narrow adapter-owned ingress path that lets local development producers enqueue typed live-patch events into an already running windowed native player while keeping session/catalog mutation inside the event-loop owner at the safe frame boundary.

## Architecture

```text
arcw run --watch / local sidecar producer
        |
        | already verified local bytes or local sidecar path
        v
WindowedPatchIngress  --mpsc-->  NativeSceneApp::proxy_wake_up/about_to_wait
        |                              |
        | typed ingress report          v
        |                      WindowedRuntimeOwner::push_patch_event
        |                              |
        |                       render frame submitted
        |                              v
        +--------------------> WindowedRuntimeOwner::drain_patch_boundary(
                                   FrameBoundary::AfterRenderSubmitted
                               )
```

`WindowedPatchIngress` is an adapter handle, not a transport server. It never exposes `BundleSession`, `BundleImageCatalog`, renderer state, or window state to producer threads.

## Files changed

- `crates/arcweft-player-native/src/lib.rs`
  - exports the ingress adapter surface and the windowed runner with an ingress hook.
- `crates/arcweft-player-native/src/windowed_ingress.rs`
  - new adapter-owned ingress API.
- `crates/arcweft-player-native/src/windowed_patch.rs`
  - extends typed patch events with sidecar base directory and explicit restart source.
- `crates/arcweft-player-native/src/patch_endpoint.rs`
  - makes the transport envelope a typed boundary object with inherent methods.
  - moves compatibility-to-action behavior onto `NativePatchTransportAction`.
- `crates/arcweft-player-native/src/windowed_runtime.rs`
  - consumes sidecar `base_dir` at commit time.
  - adds retained ingress rejection reporting without session/catalog mutation.
- `crates/arcweft-player-native/src/scene_windowed.rs`
  - wires `WindowedPatchIngress` into the winit event-loop wake path.
- `crates/arcweft-player-native/tests/windowed_ingress_runtime.rs`
  - adds owner-level ingress rejection tests.

## Public API

```rust
pub struct WindowedPatchIngress;

impl WindowedPatchIngress {
    pub fn enqueue_patch_event(
        &self,
        event: WindowedPatchEvent,
    ) -> Result<WindowedPatchIngressReport, WindowedPatchIngressError>;

    pub fn enqueue_local_sidecar_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<WindowedPatchIngressReport, WindowedPatchIngressError>;

    pub fn enqueue_local_sidecar(
        &self,
        sidecar: WindowedLocalSidecar,
    ) -> Result<WindowedPatchIngressReport, WindowedPatchIngressError>;

    pub fn last_report(&self) -> WindowedPatchIngressReport;
}

pub fn run_bundle_windowed_with_ingress(
    bundle: ArcweftBundle,
    max_steps: usize,
    configure_ingress: impl FnOnce(WindowedPatchIngress),
) -> Result<(), NativePlayerError>;
```

The normal `run_bundle_windowed` path remains available and delegates to the same implementation with a no-op ingress configuration.

## Required decisions

### Should the first ingress be `winit::EventLoopProxy`, an embedding handle, or both?

Both, but with a winit 0.31-compatible shape. In `winit 0.31.0-beta.2`, `EventLoopProxy` wakes the loop and `ApplicationHandler::proxy_wake_up` drains external event sources. It does not carry a typed custom payload itself. Therefore the concrete design is:

- public embedding handle: `WindowedPatchIngress`;
- payload queue: `std::sync::mpsc` of typed `WindowedPatchIngressMessage` values;
- wake mechanism: `EventLoopProxy::wake_up()`;
- event-loop consumer: `NativeSceneApp::proxy_wake_up` and `about_to_wait`.

This preserves type safety without depending on a stringly event dispatch channel.

### Which crate owns the ingress adapter?

`arcweft-player-native` owns it. This crate already contains:

- the native window loop;
- `WindowedRuntimeOwner`;
- windowed scene owner integration;
- the native patch endpoint and local transport sidecar decoder.

`arcweft-cli` should discover watched input files and sidecar paths, then call the ingress handle or host a local producer. `arcweft-runtime-driver` remains Sans I/O for this feature.

### How does `arcw run --watch` discover/enqueue sidecars without runtime-driver filesystem responsibilities?

The CLI continues to own polling, rebuild, patch artifact generation, and sidecar file materialization. The new adapter API accepts:

- `enqueue_local_sidecar_path(path)` when a producer has a local sidecar path;
- `enqueue_local_sidecar(WindowedLocalSidecar::new(bytes, base_dir))` when a producer has already read bytes.

The runtime driver only sees live patch bytes when the windowed event-loop owner commits a typed event at the safe boundary.

### Backpressure/coalescing

FIFO, no coalescing. The winit wake-up may coalesce, so the queue drains all available ingress messages each time the app wakes. This avoids a hidden policy where fast rebuilds can silently discard intermediate patches.

A future bounded or generation-coalescing queue should be designed as a separate policy layer with explicit tooling observability.

### What status is observable before the safe boundary?

`WindowedPatchIngressReport` is the adapter-side status:

- `idle` before any enqueue;
- `queued` after a message was accepted into the event-loop channel;
- `rejected` for malformed sidecars, wrong base roots, unsupported actions, read failures, or disconnected loop receivers.

`WindowedPatchReport` remains the owner-side retained report after the event reaches the window loop or is committed/rejected at `FrameBoundary::AfterRenderSubmitted`.

## Error model

`WindowedPatchIngressError` covers:

- `Disconnected` — mpsc receiver/event-loop owner is closed;
- `ReadSidecar` — local sidecar file could not be read;
- `ReadRestartBundle` — restart action referenced a target bundle that could not be read;
- `MalformedIngressMessage` — JSON/envelope/header/digest validation failed;
- `WrongBaseRoot` — producer expected a different active base root;
- `UnsupportedTransportAction` — sidecar action is not accepted by this ingress producer.

Malformed/wrong-base/unsupported-action errors are also retained into the owner report path through `WindowedRuntimeOwner::retain_patch_ingress_rejection` when the event loop is still alive.

## Mutation boundary

The ingress module never mutates runtime state. It sends either:

```rust
WindowedPatchIngressMessage::Enqueue(WindowedPatchEvent)
```

or:

```rust
WindowedPatchIngressMessage::RetainRejected { source, message }
```

`NativeSceneState` handles these messages on the event-loop thread. Actual session/catalog mutation remains in the existing `WindowedRuntimeOwner::drain_patch_boundary(FrameBoundary::AfterRenderSubmitted)` path.

## Tests added

### Unit tests in `windowed_ingress.rs`

- `ingress_enqueues_patch_event_without_mutating_session_state`
  - proves the adapter enqueues a typed event and only wakes the event loop.
- `ingress_preserves_fifo_order_across_coalesced_wakeups`
  - proves FIFO ordering is carried by the mpsc queue even if wakeups coalesce.
- `disconnected_ingress_reports_typed_error`
  - proves a closed event-loop receiver reports `WindowedPatchIngressError::Disconnected`.
- `malformed_sidecar_retains_rejected_report`
  - proves malformed sidecar bytes are rejected into the retained report path.
- `wrong_base_sidecar_reports_typed_error_before_enqueue`
  - proves wrong-base root preflight fails before enqueue.
- `unsupported_transport_action_reports_typed_error`
  - proves unsupported sidecar action is typed and retained.

### Integration tests in `tests/windowed_ingress_runtime.rs`

- `malformed_sidecar_reaches_owner_retained_report_without_session_mutation`
  - drives a malformed local sidecar event through `WindowedRuntimeOwner` and proves the active session still presents the old text.
- `wrong_base_sidecar_does_not_mutate_active_session_catalog`
  - builds a real wrong-base AWFB patch artifact, enqueues it through the sidecar event path, and proves the active session root and presentation remain unchanged.

## Structural audit notes

The change adds a new responsibility module instead of growing `scene_windowed.rs` or `windowed_runtime.rs` into transport adapters. The new module owns ingress queuing, sidecar preflight, typed adapter errors, and adapter-side reporting. The windowed scene module only wires the receiver into the existing event loop and forwards typed messages to `WindowedRuntimeOwner`.

No new workspace dependency is introduced. The implementation reuses existing `std::sync::mpsc`, `winit`, `thiserror`, `serde_json`, and `arcweft-bundle` APIs already available to `arcweft-player-native`.

## Local validation

Validated after applying the package to this checkout:

```bash
cargo fmt --all
cargo test -p arcweft-player-native --all-targets windowed_ingress
cargo test -p arcweft-player-native --test windowed_ingress_runtime
cargo clippy -p arcweft-player-native --all-targets --all-features -- -D warnings
cargo test -p arcweft-player-native --all-targets
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
cargo fmt --all -- --check
just test-workspace
```

Results:

- `arcweft-player-native` tests: 33 lib tests, 4 binary tests, 1 AWBC product input test, 2 windowed ingress runtime tests, and 8 windowed live patch smoke tests passed.
- clippy: passed with `-D warnings`.
- structural audit: 1,804 files scanned, 956 Rust files, 455,123 Rust physical LOC, 0 errors, 110 warnings.
- formatting and whitespace checks passed.
- workspace fast path: `just test-workspace` passed.

## Apply-time adjustments

- `WindowedPatchIngressError` variant fields named `source` in the package were renamed to `event_source` because `thiserror` treats a field named `source` as an error source. This is a naming fix, not a behavior change.
- `WindowedPatchIngress::local_sidecar_event` was made an associated function because it does not use receiver state.
- `WindowedLocalSidecar` builder-style `with_*` methods gained `#[must_use]` to satisfy the workspace clippy gate.

## Remaining TODOs

- Native window/GPU manual smoke was not launched as part of this non-interactive validation pass.
- CLI watch-mode producer wiring can now target `WindowedPatchIngress`; that producer integration remains a later slice.
