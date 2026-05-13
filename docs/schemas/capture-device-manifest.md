# Capture Device Manifest schema

This schema records microphone and camera capture declarations, device constraints, permission policy, privacy policy, backend hints, and virtual test sources.

Related:

- [Microphone / Camera Capture Devices](../03-presentation/capture-devices.md)
- [Audio Manifest](audio-manifest.md)
- [Agent Protocol](agent-protocol.md)

## Top-level shape

```rust
pub struct CaptureManifest {
    pub schema_version: u32,
    pub captures: Vec<CaptureSpec>,
    pub virtual_sources: Vec<VirtualCaptureSource>,
}
```

## CaptureSpec

```rust
pub struct CaptureSpec {
    pub id: PublicId,
    pub entity: EntityId,
    pub kind: CaptureKind,
    pub backend: CaptureBackendPolicy,
    pub constraints: CaptureConstraints,
    pub permission: CapturePermissionPolicy,
    pub privacy: CapturePrivacyPolicy,
    pub signals: Vec<SignalBinding>,
}
```

```rust
pub enum CaptureKind {
    Microphone,
    Camera,
    Screen,
}
```

## Backend policy

```rust
pub enum CaptureBackendPolicy {
    Auto,
    NativeShiguredoVideoDevice,
    NativeNokhwa,
    NativeCpalAudio,
    WebSysMediaDevices,
    VirtualPattern,
    Fixture,
    UsbDeviceProfile,
    HidDeviceProfile,
}
```

## Constraints

```rust
pub struct CaptureConstraints {
    pub audio: Option<AudioCaptureConstraints>,
    pub video: Option<VideoCaptureConstraints>,
}

pub struct AudioCaptureConstraints {
    pub channels: Preference<u16>,
    pub sample_rate: Preference<u32>,
    pub echo_cancellation: Option<bool>,
    pub noise_suppression: Option<bool>,
    pub auto_gain_control: Option<bool>,
}

pub struct VideoCaptureConstraints {
    pub width: Preference<u32>,
    pub height: Preference<u32>,
    pub fps: Preference<f32>,
    pub pixel_format: Vec<PixelFormatPreference>,
}
```

## Permission policy

```rust
pub enum CapturePermissionPolicy {
    UserPrompt,
    DevAutoGrant,
    ProductDisabled,
    GrantedByHostCapability,
}
```

## Privacy policy

```rust
pub struct CapturePrivacyPolicy {
    pub raw_agent_access: bool,
    pub raw_log_access: bool,
    pub telemetry_access: bool,
    pub product_indicator_required: bool,
    pub retention: CaptureRetention,
}

pub enum CaptureRetention {
    Transient,
    SessionOnly,
    PersistWithExplicitUserAction,
    NeverPersist,
}
```

## Example JSON

```json
{
  "schema_version": 1,
  "captures": [
    {
      "id": "capture.face_camera",
      "kind": "Camera",
      "backend": "Auto",
      "constraints": {
        "video": {
          "width": { "prefer": 1280 },
          "height": { "prefer": 720 },
          "fps": { "prefer": 30.0 },
          "pixel_format": ["nv12", "rgba", "yuy2"]
        }
      },
      "permission": "UserPrompt",
      "privacy": {
        "raw_agent_access": false,
        "raw_log_access": false,
        "telemetry_access": false,
        "product_indicator_required": true,
        "retention": "Transient"
      }
    }
  ]
}
```

## Device profile integration

Camera and microphone capture remain separate from raw USB, but a capture source can be associated with a generated device profile for specialized hardware. See [Device Profile Manifest](device-profile-manifest.md).
