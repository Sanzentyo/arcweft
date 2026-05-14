# Streams, Generators, and Live Device Sources

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
Need<T, E>
  startup / permission / realization that may take time

Stream<T, E>
  ordered event or frame sequence

Watch<T>
  latest-value signal

Generator syntax
  optional sugar for pure or granted-port stream transforms
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
pub enum Need<T, E> {
    NotStarted,
    Pending(Progress),
    Ready(T),
    Err(E),
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

`Need<T, E>` never coerces into `T`. A visible `flow` or `component` must describe what to show while it is pending.

```awft
let usb =
    try await device.usb(#device.light_panel) with {
        pending p => scene @scene.device_permission_wait {
            text "USB デバイスの許可を待っています"
            progress p.ratio
        }

        denied _ => return Ok(FlowExit::Goto(@flow.device_optional))
    }
```

## Stream functions

A `stream fn` is a state machine that yields values. It is allowed for pure transforms and for processing a granted port.

```awft
stream fn rms_level(
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

```awft
stream fn unsafe_open_mic() -> Stream<AudioFrame, AudioError> {
    // error: stream fn cannot open devices directly
    let mic = capture.microphone(#capture.player_microphone)
}
```

Use a granted capture handle instead:

```awft
let mic =
    try await capture.microphone(#capture.player_microphone) with {
        pending p => scene @scene.permission_wait { progress p.ratio }
    }

let level_stream = rms_level(mic.frames())
```

## `yield` is a suspension boundary

Like `await`, `yield` is a suspension boundary.

The following may not cross `yield`:

- `&'frame T`,
- `&'lease T`,
- `&mut T`,
- raw pointer,
- borrowed device callback buffer.

```awft
stream fn bad<'frame>(bytes: &'frame [u8]) -> Stream<u8, Unit> {
    yield bytes[0] // error if bytes is a frame-local borrow that may not outlive yield
}
```

Own or lease the data explicitly:

```awft
stream fn ok(bytes: Bytes) -> Stream<u8, Unit> {
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

## Replay and virtual sources

Live streams are recorded as summaries or deterministic fixtures.

```text
product/dev live source:
  device callback -> stream events

test/headless source:
  fixture stream -> same Stream<T, E> interface
```

A trace records:

- stream source entity,
- event timestamps/ticks,
- hashes or selected payload summaries,
- permission outcomes,
- device identity redacted by policy.

## DSL sugar

The following are equivalent:

```rust
let levels = mic.frames().stream_map(rms)
```

```awft
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
Use generator syntax only as a stream-transform convenience.
Never let a generator open hardware directly.
```
