# Capture Devices example

This example shows microphone input, camera preview, virtual test devices, and an Activity that consumes a microphone stream.

Related:

- [Microphone / Camera Capture Devices](../03-presentation/capture-devices.md)
- [Audio / Spatial / TTS / BGM](../03-presentation/audio.md)
- [Agent Debug MCP / CLI](../04-tooling/agent-debug-mcp-cli.md)

## Declarations

```awft
pub capture #capture.player_microphone: Microphone {
    permission = user_prompt
    channels = 1
    sample_rate = prefer(48000)
    echo_cancellation = true
    noise_suppression = true
    auto_gain_control = false
    privacy = transient
}

pub capture #capture.face_camera: Camera {
    permission = user_prompt
    resolution = prefer(1280x720)
    fps = prefer(30)
    pixel_format = prefer(nv12, rgba, yuy2)
    privacy = transient
}

pub signal #signal.microphone_level: Watch<f32>
pub signal #signal.camera_frame: Watch<VideoFrameHandle>

watch signal #signal.microphone_level from capture.level(#capture.player_microphone)
watch signal #signal.camera_frame from capture.latest_frame(#capture.face_camera)
```

## Flow with required pending UI

```awft
pub flow #flow.capture_setup capture_setup(state: GameState) -> Result<FlowExit, FlowError> {
    let mic =
        try await capture.microphone(#capture.player_microphone) with {
            pending _ => scene #scene.permission_wait {
                text "マイクの使用を許可してください"
            }
            denied _ => return Ok(FlowExit::Goto(#flow.no_microphone))
        }

    let cam =
        try await capture.camera(#capture.face_camera) with {
            pending _ => scene #scene.permission_wait {
                text "カメラの使用を許可してください"
            }
            denied _ => return Ok(FlowExit::Goto(#flow.no_camera))
        }

    scene #scene.capture_preview {
        CameraPreview(#capture.face_camera)
            .frame(width = 320, height = 180)
            .corner_radius(12)
            .agent_target(#ui.camera_preview)

        Meter(value = signal(#signal.microphone_level))
            .label("Mic")
    }

    Ok(FlowExit::Goto(#flow.next))
}
```

## Activity consuming microphone stream

```awft
pub activity #activity.voice_minigame VoiceMinigame {
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

let result =
    await #<activity.voice_minigame>.run({
        mic = capture.stream(#capture.player_microphone),
    })? with {
        pending p => scene #scene.voice_game_loading { progress p.ratio }
    }
```

## Deterministic test devices

```awft
pub capture #capture.test_camera: Camera {
    backend = virtual_pattern
    resolution = 1280x720
    fps = 30
}

pub capture #capture.test_microphone: Microphone {
    backend = fixture_audio("fixtures/audio/voice.wav")
}
```

## Agent CLI

```bash
arcw capture list-devices
arcw capture status #capture.face_camera
arcw agent observe --signals microphone_level,camera_frame
arcw agent capture start-test-pattern #capture.test_camera
```
