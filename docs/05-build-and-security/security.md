# Security and sandbox

## Agent capability tiers

| Tier | Capability | Examples |
|---|---|---|
| 0 | ObservePublic | Screenshot, public View tree, visible bounds |
| 1 | ControlPublic | Click, choose, key input |
| 2 | InspectDebug | State inspection, graph query, logs and signals |
| 3 | MutateDebug | State changes, hot reload, forced task results |
| 4 | UnsafeDev | Filesystem access, plugin loading, raw shader, evaluation |

Higher tiers require an explicit host grant. Product mode must not expose
debug-only mutation or unsafe development capabilities by default.

## WASM

- WASI is disabled by default and no directory is preopened.
- Host imports use an explicit allowlist.
- Memory, fuel, and execution time are bounded.

## HTML View

- Product resources use the `app://` bundle scheme only.
- Remote network access is disabled by default.
- The JavaScript bridge carries typed messages only.
- Product mode disables developer tools.

## Agent / MCP

- Product and HTTP access require a local token or equivalent authentication.
- Tool calls are audited.
- No shell command tool is exposed.
- Filesystem roots are explicit and external URL fetch is disabled by default.
- State mutation requires a debug capability token.

## Shader

- User, mod, and Agent shader input is WGSL.
- Naga and wgpu validate shaders against host capability limits.
- Unsafe passthrough is disabled outside trusted development.

## Audio / TTS

- External TTS providers require an explicit capability.
- Generated speech/audio caches are scoped and redacted in product logs.
- Microphone access requires the capture capability described below.

## Device and capture capabilities

USB, HID, serial, gamepad, virtual input, microphone, camera, and screen capture
are host capabilities. Access requires an explicit declaration and host grant.
Scripts and Activities do not enumerate devices or open platform handles.
Activities receive granted typed ports or normalized input streams.

- Raw USB/HID access requires a narrowly filtered device profile and product
  allowlist entry for vendor/product IDs or HID usage; serial access requires
  an allowlisted serial profile. Product builds do not expose a generic
  all-device USB picker. Wildcard USB filters are unavailable unless a signed
  QA capability authorizes them. Native access uses a granted device handle.
- WebUSB, WebHID, Web Serial, camera, and microphone access require a secure
  context and the browser's user permission flow, including a user gesture
  where the browser API requires it. If an API is unavailable, the capability
  is disabled or uses a virtual fixture.
- Camera, microphone, and screen capture are disabled in product builds unless
  the game declares the capture feature and the host grants the capability.
  Capture requests are explicit manifest items. A user-facing permission view
  is required, and active microphone/camera capture has a visible indicator.
  Headless and CI runs use virtual devices, capture sources, and fixture
  streams by default.
- Device output commands require capability grants and profile contracts.
  Device protocol parsers return a typed result with a source span or byte
  offset on failure.
- Device use emits audit events. Active camera/microphone capture and raw
  USB/HID use require a visible indicator or equivalent product View.
- Raw USB/HID packets, serial identifiers, camera and screen frames, and
  microphone buffers are private in logs, telemetry, plugins, and Agent
  resources. Product exposure requires an explicit signed debug grant scoped to
  the relevant surface; Agent raw-byte access also requires UnsafeDev.
- Native gamepad input uses the platform gamepad API, and Web input uses the
  browser Gamepad API; raw USB/HID is not the standard gamepad path. Logical
  gamepad and virtual-controller input are available in product mode by
  default.
- Virtual controllers are visible, accessible View components. Their layout
  respects safe areas, labels are localized, and events enter through the
  LayerTree as the same typed input actions as physical controllers. Touch
  controls consume their layer input before lower scene layers unless routing
  is explicitly configured otherwise. Their geometry may be observed in
  product observe mode and exposed as ordinary View action targets, but
  generated input remains gated. Product synthetic input requires
  ControlPublic; debug-only state mutation requires an Agent capability token.
  Debug-only controls are marked and excluded from product builds. Controller
  input does not directly mutate game state. User layout customization remains
  subject to contract-checked mappings.

The detailed contracts are owned by the
[Device Profile Manifest](../schemas/device-profile-manifest.md),
[Device I/O Manifest](../schemas/device-io-manifest.md),
[USB Device Manifest](../schemas/usb-device-manifest.md),
[Capture Device Manifest](../schemas/capture-device-manifest.md),
[Microphone / Camera Capture Devices](../03-presentation/capture-devices.md),
[Virtual Controller Manifest](../schemas/virtual-controller-manifest.md),
[Layer System](../03-presentation/layers.md), and
[Device Streams](../02-runtime/device-streams.md).
