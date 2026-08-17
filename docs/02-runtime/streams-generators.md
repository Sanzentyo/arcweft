# Streams, Generators, and External Capabilities

Arcweft supports live microphone, camera, USB, HID, gamepad, touch, and virtual-controller input. These sources are timing-sensitive, permissioned, and often backed by host callbacks or device queues. They must not be modelled as ordinary lazy values that can be implicitly forced.

This chapter defines when to use `Need`, `Stream`, `Watch`, and generator syntax.

Related chapters:

- [Async scheduler](async-scheduler.md)
- [Layered input](layered-input.md)
- [Microphone / Camera Capture Devices](../03-presentation/capture-devices.md)
- [Device I/O / USB / HID](../03-presentation/device-io-usb.md)
- [Touch Virtual Controller](../03-presentation/touch-virtual-controller.md)
- [Signals / logging / tests](../04-tooling/logging-signal-test-bench.md)

## Decision

Arcweft does **not** use a general-purpose language generator as the main device abstraction.

Instead:

```text
Need<T>
  startup / permission / realization that may take time

Stream<T, E>
  ordered stream transform or granted-port sequence

Watch<T>
  latest-value signal

Generator syntax
  optional sugar for pure sequences and stream transforms
```

This keeps capture and USB handling deterministic, permission-aware, replayable, and testable.

## Why not make devices plain generators?

A microphone, camera, USB endpoint, or HID report stream has host-level constraints:

- permission prompts,
- device unplug,
- callback lifetime,
- buffering/backpressure,
- platform-specific errors,
- product privacy policy,
- replay and headless substitution.

If a DSL generator could directly open a device, those concerns would leak into game logic and would be hard to sandbox. Therefore generators may **consume granted ports**, but may not enumerate or open devices by themselves.

## Core types

```rust
pub enum Need<T> {
    NotStarted,
    Pending(Progress),
    Ready(T),
    Cancelled,
}

pub struct Stream<T, E> {
    source: StreamSourceId,
    policy: StreamPolicy,
    item: PhantomData<T>,
    error: PhantomData<E>,
}

pub struct Watch<T> {
    signal: SignalId,
    value: Option<T>,
}
```

`Need<T>` never coerces into `T`. A visible `flow` or `view` must describe what
to show while it is pending. Domain failure is carried in a payload such as
`Need<Result<T, E>>`; cancellation is a separate control outcome.

```arcw
let usb =
    try await device.usb(@device.light_panel) with {
        pending p => {
            scene.show(@scene.device_permission_wait)
            text.show("USB デバイスの許可を待っています")
            progress.set(p.ratio)
        }
    }
```

Permission denial is represented by the producer's typed admission Result,
domain Result payload, or cancellation policy. It is not a Need state branch.

## Generator functions

An ordinary `fn` whose own body yields is a state machine that produces values.
It is allowed for pure transforms and for processing a granted stream or port.
It must return `Stream<T, E>`. An ordinary `fn` that merely returns a `Stream<T, E>`
without its own `yield` is a passthrough, not a generator.

```arcw
fn rms_level(
    frames: Stream<AudioFrame, AudioError>,
) -> Stream<f32, AudioError> {
    for frame in frames {
        yield frame.samples
            .seq()
            .map(|s| s * s)
            .mean()
            .sqrt()
    }
}
```

The following is not allowed:

```arcw
fn unsafe_open_mic() -> Stream<AudioFrame, AudioError> {
    // error: a generator function cannot open devices directly
    let mic = capture.microphone(@capture.player_microphone)
}
```

Use a granted capture handle instead:

```arcw
let mic =
    try await capture.microphone(@capture.player_microphone) with {
        pending p => scene.show(@scene.permission_wait); progress.set(p.ratio)
    }

let level_stream = rms_level(mic.frames())
```

## `seq`, `stream`, and external capabilities

`yield` is valid only in explicit generation contexts:

```text
seq { ... yield ... }
  pure lazy sequence; no runtime effects

stream { ... yield ... }
fn ... -> Stream<T, E> { ... yield ... }
  deterministic transform over existing values/streams

extern capability capture { fn frames() -> Stream<Frame, CaptureError> }
  live external input exposed as an ordinary capability operation
```

External capability operations are ordinary callable members. Their host
adapter owns permission, replay, privacy, cancellation, and queue policy; the
language sees only the typed `Stream<T, E>` result.

```arcw
extern capability capture {
    fn camera(device: CaptureDevice) -> Stream<VideoFrameHandle, CaptureError>
}
```

The operation is not a content root and does not create a source-specific
callable kind. Runtime lowering does not invent queue, replay, or privacy
defaults; those policies remain in the external capability/host contract.

## `yield` is a suspension boundary

Like `await`, `yield` is a suspension boundary.

The following may not cross `yield`:

- `&'frame T`,
- `&'lease T`,
- `&mut T`,
- raw pointer,
- borrowed device callback buffer.

```arcw
fn bad<'frame>(bytes: &'frame [u8]) -> Stream<u8, Unit> {
    yield bytes[0] // error if bytes is a frame-local borrow that may not outlive yield
}
```

Own or lease the data explicitly:

```arcw
fn ok(bytes: Bytes) -> Stream<u8, Unit> {
    yield bytes[0]
}
```

## Stream backpressure

Each stream declares a backpressure policy.

```rust
pub enum BackpressurePolicy {
    LatestOnly,
    BoundedQueue { capacity: usize, on_overflow: OverflowPolicy },
    BlockingNotAllowed,
}

pub enum OverflowPolicy {
    DropOldest,
    DropNewest,
    Error,
    Coalesce,
}
```

Camera preview typically uses `LatestOnly`. USB protocol streams usually use `BoundedQueue` and error on overflow.

## Replay and virtual streams

Live sources are recorded as summaries or deterministic fixtures.

```text
product/dev live stream:
  device callback -> stream events

test/headless stream:
  fixture stream -> same Stream<T, E> interface
```

A trace records:

- stream identity,
- event timestamps/ticks,
- hashes or selected payload summaries,
- permission outcomes,
- device identity redacted by policy.

## DSL sugar

The following are equivalent:

```rust
let levels = mic.frames().stream_map(rms)
```

```arcw
let levels = stream {
    for frame in mic.frames() {
        yield rms(frame)
    }
}
```

The block form is useful for complex stream transforms but is not required for most game code.

## Design rule

```text
Use Need for startup.
Use Stream for ordered live data.
Use Watch for latest value.
Use generator syntax only in explicit seq/stream contexts.
Never let a generator open hardware directly.
```

