# Device Streams, Source Blocks, and Generator Policy

Arcweft supports camera, microphone, USB, HID, gamepad, touch, virtual controller, and test-fixture input through one runtime concept: a **device stream**. A device stream is not an ordinary lazy `Seq<T>` and not a raw Rust generator. It is a permissioned, cancelable, backpressure-aware source of timestamped events.

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

Source<T, E>
  Live device or event stream.
  Permissioned, timestamped, cancelable, backpressure-aware.

Need<Result<T, E>, TaskError>
  One-shot realization or acquisition.
  Must be awaited with pending/denied/error branches in user-visible flows.
```

Rust implementation may use `futures::Stream`, callback adapters, or generated state machines. The DSL may expose `source` blocks, but they lower into explicit stream state machines rather than relying on unstable Rust generator internals.

## Core types

```rust
pub struct Source<T, E> {
    pub id: SourceId,
    pub item_type: TypeId,
    pub policy: SourcePolicy,
}

pub struct SourcePolicy {
    pub backpressure: BackpressurePolicy,
    pub clock: SourceClock,
    pub replay: ReplayPolicy,
    pub privacy: PrivacyPolicy,
    pub max_queue: usize,
}

pub enum BackpressurePolicy {
    Latest,
    DropOldest,
    DropNewest,
    Exact,
    Coalesce,
    BlockProducer,
}

pub enum SourceEvent<T, E> {
    Item {
        source: SourceId,
        timestamp: SourceTimestamp,
        item: T,
    },
    Progress(Progress),
    Disconnected,
    PermissionRevoked,
    Error(E),
    End,
}
```

## Source block syntax

A `source` block is allowed, but it is declarative and policy-driven.

```awft
pub source @source.face_camera_frames: Source<VideoFrameHandle, CaptureError> {
    from capture.camera(@capture.face_camera)
    backpressure = latest
    replay = hash_only
    privacy = transient

    on item frame => yield frame
    on disconnected => emit signal @signal.camera_connected <- false
    on error e => log warn "camera stream error {err:?}" { err = e }
}
```

This is not a free-form coroutine. The compiler enforces:

- every `yield` has a typed item,
- every source has a backpressure policy,
- borrowed frame data cannot cross `yield` or `await`,
- source items are delivered only at frame boundary unless explicitly marked realtime,
- source replay policy is explicit.

## Source consumption

User-visible flows must decide what to do while a source is being acquired.

```awft
let mic =
    try await capture.microphone(@capture.player_microphone) with {
        pending p => scene @scene.permission_wait {
            text "マイクの許可を待っています"
            progress p.ratio
        }
        denied _ => return Ok(FlowExit::Goto(@flow.mic_optional))
    }

let frames = source.audio_frames(mic)
```

Once acquired, stream items are consumed by `select`, `poll`, Activity input ports, or signals.

```awft
select {
    audio = frames.next? => {
        signal @signal.voice_level <- audio.rms
    }

    frame _ => {
        scene @scene.listening {
            meter @signal.voice_level
        }
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

```awft
let reports =
    usb.interrupt_in(@usb.custom_pad, endpoint = @usb.ep.input)
        .map(parse_custom_pad_report)
        .filter(_.is_ok())
        .map(_.unwrap())
        .coalesce_latest()
```

`Source<T, E>` adapters do not become pure `Seq<T>`. They preserve timestamp, error, disconnect, and backpressure semantics.

## Headless and replay

Every device stream can be replaced by a fixture source.

```rust
pub source @source.test_camera_frames: Source<VideoFrameHandle, CaptureError> {
    from fixture.video("fixtures/camera/front_cam.webm")
    backpressure = exact
    replay = full
}
```

Replay records one of:

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
  -> SourceEvent queue
  -> frame-boundary normalization
```

Web implementation strategy:

```text
web-sys permission/bootstrap
  -> MediaStream / WebUSB / WebHID / Gamepad / Pointer events
  -> wasm bridge
  -> SourceEvent queue
```

Do not expose backend callbacks directly to DSL code. Always convert to `SourceEvent` first.
