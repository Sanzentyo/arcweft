# Seq-03.8 - Windowed Live-Patch Ingress Closure Audit

Date: 2026-06-28
Package: `arcweft-seq03.8-windowed-live-patch-ingress-closure-audit-2026-06-28.zip`

## Audit conclusion after application

The uploaded audit package correctly identified that current main had the
windowed owner substrate but was still missing the external adapter-owned
ingress path. This application consumes the package's seq03.8a implementation
request and closes the missing slice for local native watch ingress.

Seq-03 can now treat windowed live-patch ingress as closed for:

- local CLI watch rebuilds;
- local patch bundle bytes;
- local transport sidecar bytes;
- bounded FIFO handoff into the running native window;
- event-loop-owned acceptance and frame-boundary commit;
- typed adapter-side status and rejection reports.

## Current classification

| Seq03.8 item | Classification after this change |
| --- | --- |
| Adapter-owned `WindowedPatchIngress` handle | implemented_and_validated |
| Native watch enqueues into the running window | implemented_and_validated |
| Event-loop owner remains the mutation boundary | implemented_and_validated |
| `AfterRenderSubmitted` remains the safe commit boundary | implemented_and_validated |
| Malformed/wrong-base/unsupported sidecar retained reports | implemented_and_validated |
| Disconnected loop / closed player / queue-full typed errors | implemented_and_validated |
| FIFO/backpressure/coalescing semantics | implemented_and_validated |
| Runtime-driver Sans I/O boundary | implemented_and_validated |
| Seq03.7 direct-owner fixtures counted separately from ingress | implemented_and_validated |

## Design deviation from package text

The package's implementation request spoke in terms of a typed custom
`EventLoopProxy` payload. Current main's `winit 0.31.0-beta.2` API exposes
`EventLoopProxy::wake_up()` and `ApplicationHandler::proxy_wake_up` for this
path. The applied implementation therefore stores typed payloads in a bounded
FIFO queue and uses the proxy only to wake the event loop.

This is an API adaptation, not a boundary change: only `NativeSceneApp` drains
the queue, and only `WindowedRuntimeOwner` mutates runtime state.

## Follow-up boundary

No additional seq03 request is required for the non-interactive implementation
closure. The remaining validation item is an operator/platform run:

```bash
cargo run -p arcweft-cli --features native-player -- run --runner native --watch --watch-poll-ms 250 path/to/game.arcw
```

That run should be used to observe the visible native window updating from a
real edited source file and then closed manually. It is not a design gap in the
seq03.8 implementation.
