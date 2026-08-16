# Device Streams and Generator Policy

Arcweft supports camera, microphone, USB, HID, gamepad, touch, virtual controller, and test-fixture input through one runtime concept: a **device stream**. A device stream is not an ordinary lazy `Seq<T>` and not a raw Rust generator. An external capability operation returns a typed `Stream<T, E>` handle; host adapters own permission, cancellation, queueing, and timestamp normalization.

Related chapters:

- [Async Scheduler](async-scheduler.md)
- [Layered Input](layered-input.md)
- [Microphone / Camera Capture Devices](../03-presentation/capture-devices.md)
- [USB / HID Devices](../03-presentation/usb-devices.md)
- [Virtual Touch Controller](../03-presentation/virtual-controller.md)
- [Agent Debug MCP / CLI](../04-tooling/agent-debug-mcp-cli.md)
- [USB Device Manifest](../schemas/usb-device-manifest.md)

## Decision: not a general-purpose runtime generator

A general-purpose generator with arbitrary `yield` looks attractive, but for device input it is the wrong primitive if used directly:

- device streams need permission state,
- they need cancellation,
- they need hotplug / disconnect handling,
- they need backpressure and frame dropping policy,
- they must work in native, Web, headless, replay, and test modes,
- they must not hide long waits or permission prompts,
- they must not keep borrowed frame data across suspension boundaries.

Therefore Arcweft uses this split:

```text
Seq<T>
  Lazy pure sequence.
  map/filter/fold/fusion.
  No permission, no wall-clock input.

Stream<T, E>
  Sole asynchronous sequence abstraction. It may be returned by an external
  capability or produced by a transform over an existing stream or granted
  port.

Need<Result<T, E>, TaskError>
  One-shot realization or acquisition.
  Must be awaited with pending/denied/error branches in user-visible flows.
```

Rust implementation may use `futures::Stream`, callback adapters, or generated state machines. External capability calls lower into explicit typed stream requests; the DSL has no `source` declaration role and does not rely on unstable Rust generator internals.

## Core types

```rust
pub struct Stream<T, E> {
    pub id: StreamId,
    pub item_type: TypeId,
    pub error_type: TypeId,
}

pub struct StreamEvent<T, E> {
    pub stream: StreamId,
    pub sequence: TaskSequence,
    pub kind: StreamEventKind<T, E>,
}

pub enum StreamEventKind<T, E> {
    Item(T),
    Error(E),
    End,
}
```

Backend adapters may attach richer native timestamps while recording/replaying
through `StreamEvent.sequence`, but runtime core stores only Sans I/O stream
state and deterministic frame-boundary event data.

## External Stream capabilities

Live device access is an ordinary external capability operation returning a
`Stream<T, E>` value. It is not a declaration, root, or special callable role.

```arcw
extern capability capture {
    fn camera(device: CaptureDevice) -> Stream<VideoFrameHandle, CaptureError>
}
```

This is not a free-form coroutine. The compiler enforces:

- every `yield` has a typed item,
- every generator has a typed `Stream<T, E>` result,
- borrowed frame data cannot cross `yield` or `await`,
- stream items are delivered only at frame boundary unless the capability
  explicitly provides realtime semantics.

## Source consumption

User-visible flows must decide what to do while a source is being acquired.

```arcw
let mic =
    try await capture.microphone(@capture.player_microphone) with {
        pending p => {
            scene.show(@scene.permission_wait)
            text.show("マイクの許可を待っています")
            progress.set(p.ratio)
        }
        denied _ => return Ok(FlowExit::Goto(@flow.mic_optional))
    }

let frames = capture.audio_frames(mic)
```

Once acquired, stream items are consumed by `select`, `poll`, Activity input ports, or signals.

```arcw
select {
    audio = frames.next? => {
        signal.set(@signal.voice_level, audio.rms)
    }

    frame _ => {
        scene.show(@scene.listening)
        meter.show(source = @signal.voice_level)
        continue
    }

    event .Back => {
        close frames
        return Ok(FlowExit::Goto(@flow.title))
    }
}
```

## Stream adapters

Standard adapters mirror function-pipeline style, but remain source-aware.

```arcw
let reports =
    usb.interrupt_in(@usb.custom_pad, endpoint = @usb.ep.input)
        .map(parse_custom_pad_report)
        .filter(_.is_ok())
        .map(_.unwrap())
        .coalesce_latest()
```

`Stream<T, E>` adapters do not become pure `Seq<T>`. They preserve the typed
item/error boundary; capability-owned lifecycle and queue policy remain at the
external boundary.

## Headless and replay

Every device stream can be replaced by a fixture capability returning a stream.

```rust
let test_camera_frames =
    fixture.video("fixtures/camera/front_cam.webm")
        -> Stream<VideoFrameHandle, CaptureError>
```

Replay policy is selected by the fixture or host capability. The runtime records
typed stream events and their sequence values; it does not expose a second
Source interface.

```text
full:
  exact item payloads or fixture ids

hash_only:
  item hash + summary; suitable for camera/mic privacy

event_only:
  connection/disconnect/error events only

none:
  product mode for private capture
```

## Implementation guidance

Native implementation strategy:

```text
callback device backend
  -> owned frame/audio packet
  -> bounded ring buffer
  -> StreamEvent queue
  -> frame-boundary normalization
```

Web implementation strategy:

```text
web-sys permission/bootstrap
  -> MediaStream / WebUSB / WebHID / Gamepad / Pointer events
  -> wasm bridge
  -> StreamEvent queue
```

Do not expose backend callbacks directly to DSL code. Always convert them to
typed `StreamEvent` values at the capability/host boundary first.

