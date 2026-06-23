# Request: Windowed Native Live Patch Design

## Request

Please design the implementation path for applying `arcw run --watch` patch
updates to an already running windowed native player event loop.

The design should be concrete enough to turn into small Rust implementation
cuts with focused tests.

## Why this needs a decision

The incremental hot-swap bundle work now has:

- `arcweft-player-native::NativePatchEndpoint`, an in-process AWFB-backed patch
  endpoint that can apply live-compatible patch bundles or restart its owned
  `BundleSession`.
- `arcw run --watch --runner native`, which keeps such an endpoint alive and
  applies emitted patch bundles to it.
- `arcweft-player-native` binary support for applying one `.transport.json`
  sidecar before running a bundle.
- A separate windowed native scene loop that owns its own `BundleSession`,
  image catalog, renderer state, input state, and `winit` event loop.

The remaining windowed-live behavior is larger than passing a patch path into a
function. The current windowed event loop and `NativePatchEndpoint` own session
state through different boundaries. A correct design must answer how patch
events enter the `winit` loop, how the session/image catalog/render resources
are swapped or restarted at safe points, and how a restart-required patch
affects window state, input state, renderer resources, and user-visible
continuity.

This means the repository has the Sans I/O/session-level and headless
in-process patch foundations, but does not yet have an authoritative ownership
model for live patching an already running native window.

## Design questions

Please propose concrete answers for:

1. Which component owns the active `BundleSession` in windowed mode after this
   change: `NativePatchEndpoint`, `NativeSceneState`, or a new shared
   windowed-runtime state?
2. How should patch transport events enter the event loop: polling a sidecar,
   channel messages from `arcw run --watch`, file watch, local socket, or an
   explicit embedding API?
3. At what `winit`/runtime boundary may a patch be prepared, committed, or
   restarted so renderer and host adapter state are not mutated mid-frame?
4. How should content-only and code-compatible patches update image catalogs,
   display catalogs, renderer caches, active frames, and pending host tasks?
5. How should code-generational or restart-required patch artifacts behave in
   windowed mode before true code-generational execution exists?
6. Which state should survive an automatic restart: window handle, surface,
   renderer device/queue, input focus, pointer state, visual clocks, active
   flow, and presentation state?
7. How should errors be reported without killing the player when a dev patch is
   invalid or incompatible?
8. How should the binary `--patch-transport` one-shot path relate to a live
   event-loop patch stream?
9. What tests can validate the logic without requiring a real GPU/window, and
   which smoke tests should exercise the actual windowed path?

## Constraints

- Keep product players free of syntax/HIR/sema/compiler dependencies.
- Keep filesystem/socket/watch transport outside `arcweft-runtime-driver`.
- Do not mutate a `BundleSession`, image catalog, or renderer resource while a
  runtime step or frame render is in progress.
- Preserve deterministic session behavior and explicit restart reporting.
- Do not implement true code-generational execution as part of this design;
  that is tracked separately.
- Prefer typed patch events and outcomes over stringly sidecar handling in the
  scene loop.

## Expected output

Please provide:

- the recommended ownership model;
- affected crates/modules;
- new or changed public/private types;
- the patch event transport API;
- live-apply and restart state-machine steps;
- renderer/catalog refresh rules;
- error reporting behavior;
- step-by-step implementation order;
- focused unit tests and smoke validation commands.

## Current goal boundary

Until this design is answered, the current incremental hot-swap goal should not
implement:

- a live patch stream into an already running windowed native `winit` event
  loop;
- ad hoc shared mutable access to the windowed `BundleSession`;
- implicit renderer/image catalog mutation from outside the event-loop safe
  point;
- windowed true code-generational execution.

The current goal may keep:

- `NativePatchEndpoint` as the headless/in-process patch endpoint;
- `arcw run --watch --runner native` applying patch artifacts to that endpoint;
- native player binary one-shot `--patch-transport` before window/headless run;
- restart fallback for code-generational or restart-required patch artifacts.

## Useful current evidence

Start with these files:

- `crates/arcweft-player-native/src/patch_endpoint.rs`
- `crates/arcweft-player-native/src/scene_windowed.rs`
- `crates/arcweft-player-native/src/main.rs`
- `crates/arcweft-cli/src/app/runtime/run.rs`
- `crates/arcweft-runtime-driver/src/session.rs`
- `crates/arcweft-runtime-driver/src/swap.rs`
- `docs/implementation/incremental-hot-swap-bundle-2026-06-23.md`
