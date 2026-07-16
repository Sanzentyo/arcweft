# Microphone / Camera Capture Devices

Arcweft supports microphone and camera input as first-class, permissioned capture sources. Capture is not treated as an ordinary asset load because it is live, permission-gated, timing-sensitive, and privacy-sensitive.

Related chapters:

- [Audio / Spatial / TTS / BGM](audio.md)
- [Device Streams / Generator Policy](../02-runtime/device-streams.md)
- [USB / HID Devices](usb-devices.md)
- [Virtual Touch Controller](virtual-controller.md)
- [Layer System / Input Routing](layers.md)
- [Agent Debug MCP / CLI](../04-tooling/agent-debug-mcp-cli.md)
- [Security](../05-build-and-security/security.md)
- [Virtual Touch Controller](virtual-controller.md)
- [Device Profiles, Generators, and USB](device-generator-and-usb.md)
- [Capture Device Manifest](../schemas/capture-device-manifest.md)
- [Capture Devices Example](../examples/capture-devices.md)

## Decisions

- Audio input/output uses `cpal` as the primary low-level audio I/O backend, including the WebAssembly path where available.
- Browser microphone and camera permission flow is still explicitly modelled through `web-sys` / browser MediaDevices because Web capture is permission-gated and constraint-driven.
- Native camera input prefers `shiguredo_video_device` as the first backend because it directly targets macOS AVFoundation, Linux V4L2/PipeWire, and Windows Media Foundation.
- `nokhwa` remains an optional compatibility backend for simple webcam capture and broader experimentation.
- Web camera input uses `web-sys` MediaDevices / MediaStream / MediaStreamTrack, not `nokhwa` or `shiguredo_video_device`.
- Capture frames and microphone buffers enter Arcweft as `Need<Result<CaptureHandle, CaptureError>, TaskError>` and then as live `Stream`/`Watch` signals.
- Raw camera/microphone data is never exposed to scripts, LLM agent tools, or product telemetry unless an explicit capability is enabled.

## Why CPAL for audio I/O

CPAL is a low-level cross-platform audio I/O library. It exposes hosts, devices, and streams; devices can have input and output streams, and stream callbacks receive captured samples or fill output samples.

Arcweft uses CPAL for:

- native microphone input,
- native speaker output,
- WebAudio output via the `wasm-bindgen` backend,
- optional lower-latency WebAudioWorklet output when the browser and deployment headers allow it,
- feedback / monitoring / capture-to-output development paths.

The audio backend abstraction still wraps CPAL rather than exposing it directly to the DSL. That lets Arcweft keep permission, device selection, trace, test, and product-mode policy stable.

```text
arcweft-audio-device
  -> cpal on native
  -> cpal wasm-bindgen / audioworklet where viable
  -> web-sys MediaDevices bootstrap for browser microphone permissions
```

## CPAL and Web caveat

CPAL has WebAssembly support through the Web Audio API backend. For lower-latency audio processing, CPAL also exposes an Audio Worklet backend that needs atomics support and SharedArrayBuffer-compatible deployment headers.

Browser microphone capture still has browser-specific requirements:

- user permission prompt,
- secure context,
- device constraints,
- track stopping,
- permissions revocation,
- echo cancellation / noise suppression / auto gain constraints,
- possible autoplay / user-gesture constraints for playback.

Therefore Arcweft's Web audio path is:

```text
web microphone permission/device discovery:
  web-sys MediaDevices.getUserMedia

sample stream integration:
  CPAL wasm path when available and sufficient
  or WebAudio AudioWorklet/custom bridge as fallback

Arcweft runtime:
  AudioInputStream -> AudioFrameRing -> Signal/Task/Activity
```

This is intentionally a policy boundary: scripts ask for `capture.microphone`, not for arbitrary browser APIs.

## Native camera backend comparison

| Topic | `shiguredo_video_device` | `nokhwa` |
|---|---|---|
| Primary role | Native video device library | Simple cross-platform webcam capture library |
| Native platforms | macOS AVFoundation, Linux V4L2/PipeWire, Windows Media Foundation | Cross-platform backends through `input-*` features |
| Web target | No; use `web-sys` for Web | Not the primary Web path for Arcweft |
| API style | Device list, config, callback capture | `Camera` abstraction, frame/frame_raw, optional callback camera |
| Frame lifetime | Callback frame memory valid only during callback; copy via owned frame if retained | `frame()` returns processed buffer, `frame_raw()` returns raw borrowed/Cow data |
| Blocking behavior | Callback based; backend details still matter | Docs note many capture backends are blocking |
| Linux choices | V4L2 default, optional PipeWire; mutually exclusive features | Backend feature selection; order generally native -> UVC -> GStreamer |
| Arcweft default | Preferred native backend | Optional compatibility backend |

### Recommendation

Use `shiguredo_video_device` as the default native camera backend for Arcweft because it has explicit AVFoundation/V4L2/PipeWire/Media Foundation coverage and a clear device/config/callback model. Use `nokhwa` as an optional backend for cases where its simple `Camera` abstraction, raw frame access, or existing ecosystem examples are useful.

```toml
[features]
capture-audio-cpal = ["dep:cpal"]
capture-video-shiguredo = ["dep:shiguredo_video_device"]
capture-video-nokhwa = ["dep:nokhwa"]
capture-web-media = ["dep:web-sys", "dep:wasm-bindgen", "dep:js-sys"]
```

## Capture architecture

```text
Capture permission request
  -> device enumeration
  -> device selection
  -> format negotiation
  -> stream creation
  -> frame/audio callback
  -> ring buffer / frame arena
  -> Arcweft signals / activities / render textures
```

Core types:

```rust
pub enum CaptureKind {
    Microphone,
    Camera,
    Screen,
}

pub struct CaptureRequest {
    pub id: CaptureRequestId,
    pub kind: CaptureKind,
    pub constraints: CaptureConstraints,
    pub permission: CapturePermissionPolicy,
    pub privacy: CapturePrivacyPolicy,
}

pub enum CaptureEvent {
    PermissionPrompted { request: CaptureRequestId },
    PermissionGranted { request: CaptureRequestId },
    PermissionDenied { request: CaptureRequestId, reason: CaptureError },
    DeviceListChanged,
    StreamStarted { stream: CaptureStreamId },
    StreamStopped { stream: CaptureStreamId },
    AudioFrame { stream: CaptureStreamId, frame: AudioFrameRef },
    VideoFrame { stream: CaptureStreamId, frame: VideoFrameRef },
    Error { stream: Option<CaptureStreamId>, error: CaptureError },
}
```

## DSL syntax

Capture requests must be explicit. No script can implicitly access the microphone or camera.

```arcw
pub capture @capture.player_microphone: Microphone {
    permission = user_prompt
    channels = 1
    sample_rate = prefer(48000)
    echo_cancellation = true
    noise_suppression = true
    auto_gain_control = false
    privacy = transient
}

pub capture @capture.face_camera: Camera {
    permission = user_prompt
    resolution = prefer(1280x720)
    fps = prefer(30)
    pixel_format = prefer(nv12, rgba, yuy2)
    privacy = transient
}
```

Starting capture returns a `Need` and therefore must define a pending View in a player-visible `flow` or `view`.

```arcw
let mic =
    try await capture.microphone(@capture.player_microphone) with {
        pending p => {
            scene.show(@scene.permission_wait)
            text.show("マイクの許可を待っています")
            progress.set(p.ratio)
        }

        denied e => {
            log.warn("microphone permission denied: {e:?}", e = e)
            return Ok(FlowExit::Goto(@flow.no_mic_fallback))
        }
    }
```

Camera capture is the same:

```arcw
let cam =
    try await capture.camera(@capture.face_camera) with {
        pending p => {
            scene.show(@scene.permission_wait)
            text.show("カメラの許可を待っています")
            progress.set(p.ratio)
        }

        denied _ => return Ok(FlowExit::Goto(@flow.camera_optional))
    }
```

## Capture as signals

Capture exposes latest-state and stream signals:

```arcw
pub signal @signal.microphone_level: Watch<f32>
pub signal @signal.microphone_vad: Watch<bool>
pub signal @signal.camera_frame: Watch<VideoFrameHandle>
pub signal @signal.camera_pose: Watch<Option<FacePose>>
pub signal @signal.capture_error: Stream<CaptureError>
```

Typical usage:

```arcw
watch signal @signal.microphone_level from capture.level(@capture.player_microphone)
watch signal @signal.camera_frame from capture.latest_frame(@capture.face_camera)
```

## Camera frames and rendering

Camera frames are treated as live external textures/frames. They may be:

- copied into an Arcweft-owned texture,
- converted to RGBA/I420/NV12 by a capture preprocessing task,
- passed to an Activity through a frame lease,
- exposed to View as `CameraPreviewView`,
- used as a shader resource only if the capability permits it.

```arcw
CameraPreview(@capture.face_camera)
    .fit(cover)
    .clip(.rounded_rect(radius = 16))
    .agent_target(@view.camera_preview)
```

For zero-copy or borrowed frame paths, lifetimes follow the existing borrow rules: `VideoFrameRef<'frame>` cannot cross `await`, `yield`, or `thread` boundaries. If a frame must outlive the callback/frame scope, it must become an owned frame handle.

## Activity integration

Activities receive capture through typed ports, not by opening devices themselves.

```arcw
pub activity @activity.voice_minigame VoiceMinigame {
    input {
        mic: stream<AudioFrame>
    }
    output {
        result: event<VoiceResult>
    }
    capability {
        capture.microphone = read
    }
}
```

```arcw
let result =
    await #<activity.voice_minigame>.run({
        mic = capture.stream(@capture.player_microphone),
    })? with {
        pending p => scene.show(@scene.voice_game_loading); progress.set(p.ratio)
    }
```

## Agent / MCP / test handling

Agent tools can observe capture state, but product builds must not expose raw audio/video without explicit permission.

Allowed by default in dev:

```text
arcweft.capture_list_devices
arcweft.capture_get_status
arcweft.capture_get_level
arcweft.capture_get_frame_metadata
arcweft.capture_start_test_pattern
```

Restricted:

```text
arcweft.capture_get_audio_samples
arcweft.capture_get_camera_frame
arcweft.capture_save_recording
```

Headless test mode should support virtual devices:

```arcw
pub capture @capture.test_camera: Camera {
    backend = virtual_pattern
    resolution = 1280x720
    fps = 30
}

pub capture @capture.test_microphone: Microphone {
    backend = fixture_audio("fixtures/audio/voice.wav")
}
```

This keeps tests deterministic and avoids permission prompts in CI.

## Security and privacy

Capture is high-risk. Arcweft enforces:

- explicit capability declaration,
- explicit user-facing permission flow,
- no raw capture exposure to product telemetry by default,
- no capture access from untrusted WASM/Rust plugin unless a port is granted,
- redaction by default in logs and Agent observations,
- audit log for every capture start/stop/device change,
- product-mode visual indicator when camera/microphone capture is active.

```toml
[capture.product]
allow_microphone = false
allow_camera = false
agent_raw_frames = false
show_indicator = true
```

## Crate layout

```text
arcweft-capture-core
  CaptureRequest, CaptureEvent, CaptureStream, constraints, permissions

arcweft-capture-audio-cpal
  CPAL adapter for microphone/speaker streams

arcweft-capture-video-shiguredo
  shiguredo_video_device native camera adapter

arcweft-capture-video-nokhwa
  optional nokhwa adapter

arcweft-capture-web
  web-sys MediaDevices / MediaStream / MediaStreamTrack adapter

arcweft-capture-virtual
  deterministic fixtures, test patterns, generated audio/video

arcweft-capture-agent
  MCP/CLI observation and controlled test capture tools
```

## Implementation notes

- Keep device callbacks short and non-panicking.
- Copy or lease data before passing it to scripts or Activities.
- Convert frame formats off the callback thread.
- Use bounded ring buffers and drop policies for backpressure.
- Track timestamps with a media clock and expose AV sync metadata.
- Do not let Activities enumerate devices directly; they consume granted ports.


## Specialized USB capture hardware

Some capture hardware appears as USB or HID devices rather than ordinary camera/microphone devices. Arcweft treats these through [Device Profiles, Generators, and USB](device-generator-and-usb.md). The device profile may expose typed signals or typed capture frames, but player-visible flows still receive `Need<Result<..., ...>, ...>` and must handle pending/denied branches.

```arcw
pub device @device.depth_camera: UsbRaw {
    permission = user_prompt
    usb { vendor_id = 0x1209 product_id = 0xD001 interface = 1 }
    endpoints { input frame: bulk endpoint 0x81 packet = 512 }
    decoder frame = decode_depth_frame
}

watch signal @signal.depth_frame from device.latest(@device.depth_camera)
```

## Generator / Stream policy

Capture devices do not require a general-purpose generator as their primary abstraction. Arcweft uses:

```text
Need<Result<CaptureHandle, CaptureError>, TaskError>
  for permission/startup

Stream<AudioFrame, AudioError> / Stream<VideoFrame, VideoError>
  for ordered frame delivery

Watch<VideoFrameHandle> / Watch<f32>
  for latest camera frame, audio level, VAD, pose, etc.
```

Generator syntax is still useful as a **stream transform**:

```arcw
stream fn microphone_level(
    frames: Stream<AudioFrame, AudioError>,
) -> Stream<f32, AudioError> {
    for frame in frames {
        yield frame.rms()
    }
}
```

A generator cannot open microphone or camera devices. It must consume a granted capture port. See [Streams and generators](../02-runtime/streams-generators.md).

## USB-adjacent cameras and microphones

If a camera or microphone appears as a standard OS media device, Arcweft treats it as capture and uses the capture backends in this chapter. If it is a vendor-specific USB/HID device, Arcweft treats it as [Device I/O / USB / HID](device-io-usb.md) and requires a typed protocol parser.


## Related device systems

- [USB / HID / Serial device support](usb-devices.md)
- [Device Generator / Profile System](../05-build-and-security/device-generator.md)
- [Virtual Touch Controller](virtual-controller.md)

Capture devices, USB/HID/Serial devices, and virtual controllers all use the same `DeviceProfile` / `Port` / `Need<Result<...>>` permission model. Microphone and camera remain capture-specific because they need privacy indicators and media pipeline integration.

## Device stream policy

Camera and microphone frames are represented as `Source<T, E>` streams after permissioned acquisition. Arcweft does not require a general-purpose generator to model capture devices. Instead, callbacks and browser events are normalized into `SourceEvent` queues with explicit backpressure and replay policy. See [Device Streams](../02-runtime/device-streams.md).

