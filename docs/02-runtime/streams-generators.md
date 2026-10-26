# Streams, Generators, and External Capabilities

This chapter defines the language boundary for `Need`, `Stream`, `Watch`, and
generator syntax. Device access and its policy are owned by external capability
manifests and hosts; see [Device Stream Runtime Boundary](device-streams.md).

Related chapters:

- [Async scheduler](async-scheduler.md)
- [Layered input](layered-input.md)
- [Microphone / Camera Capture Devices](../03-presentation/capture-devices.md)
- [Device I/O Manifest](../schemas/device-io-manifest.md)
- [Layer System / Input Routing](../03-presentation/layers.md)
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

`Need<T>` is the one-shot temporal carrier and `Stream<T, E>` is the ordered
asynchronous sequence descriptor. Their type and suspension rules are owned by
[Await and Need](../01-language/await-need-result.md) and the
[standard prelude](../01-language/standard-types-and-prelude.md).

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

Backpressure, replay, and privacy are selected at the device/capture capability
boundary by its manifest and host; see [Device Stream Runtime
Boundary](device-streams.md) and the linked device schemas there.

## Design rule

```text
Use Need for startup.
Use Stream for ordered live data.
Use Watch for latest value.
Use generator syntax only in explicit seq/stream contexts.
Never let a generator open hardware directly.
```

