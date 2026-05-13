# Security and sandbox

## Capability tiers

```text
Tier 0 ObservePublic:
  screenshot, public UI tree, visible bbox

Tier 1 ControlPublic:
  click, choose, key input

Tier 2 InspectDebug:
  state inspection, graph query, logs/signals

Tier 3 MutateDebug:
  set state, hot reload, force task result

Tier 4 UnsafeDev:
  filesystem, plugin load, raw shader, eval
```

## WASM

- WASI off by default。
- preopened dir なし。
- host import whitelist。
- memory/fuel/time limit。

## HTML UI

- `app://` bundle resource only。
- remote network off by default。
- JS bridge is typed message only。
- product mode devtools off。

## Agent / MCP

- local token / auth for product and HTTP.
- audit log every tool call.
- no shell command tool.
- filesystem roots explicit.
- external URL fetch off by default.
- state mutation requires debug token.

## Shader

- WGSL only for user/mod/LLM.
- Naga + wgpu validation + capability check.
- unsafe passthrough disabled outside trusted dev.

## Audio / TTS

- external TTS providers require explicit capability.
- generated speech/audio cache is scoped and redacted in product logs.
- microphone input is out of scope unless explicit capability is added.



## Capture privacy

Microphone, camera, and screen capture are high-risk capabilities. Product builds must default them to disabled unless the game explicitly declares a capture feature and the host grants the capability.

Rules:

- Capture requests must be explicit ModuleItems.
- User-facing permission UI is mandatory.
- Product mode must show an active microphone/camera indicator.
- Raw capture frames/samples are not exposed to Agent tools, logs, telemetry, or plugins without a specific capability.
- Activities consume granted capture ports; they do not enumerate or open devices themselves.
- Headless and CI use virtual capture sources by default.

See [Microphone / Camera Capture Devices](../03-presentation/capture-devices.md).


## USB / HID device policy

- Product builds must not expose a generic all-device USB picker.
- USB/WebUSB access requires explicit `device` profiles with vendor/product or HID usage filters.
- Raw USB bytes, HID report bytes, camera frames, and microphone buffers are private by default.
- Device output commands require capability grants and profile contracts.
- Agent tools may observe typed signals, but raw bytes require `UnsafeDev` capability.
- WebUSB requires user permission and browser support; unavailable browsers must fall back to virtual fixtures or disable the feature.

## Virtual controller policy

- Virtual touch controls are visible UI and must expose accessible labels.
- Touch controls consume their layer input before lower scene layers unless explicitly configured otherwise.
- Product builds may allow user layout customization, but generated controller mappings remain contract-checked.

## USB / HID / gamepad / virtual controller

USB and HID access is high-risk and may expose hardware outside the game sandbox.

Rules:

- Raw USB/HID is disabled in product builds unless a device declaration and product capability explicitly enable it.
- Native raw USB uses a granted `UsbDeviceHandle`; scripts never call platform enumeration directly.
- WebUSB/WebHID require secure contexts and user device selection.
- Device filters must be as narrow as possible.
- Raw packets/reports are redacted from logs and Agent resources by default.
- Device protocol parsers must return `Result<T, ParseError>` with spans or byte offsets.
- Out-of-process Activities receive typed streams or granted ports; they do not open devices directly.
- Virtual controller synthetic input in product builds requires `ControlPublic` capability.
- Touch controller geometry can be observed in product observe mode, but generated input is still gated.

See [Device I/O / USB / HID](../03-presentation/device-io-usb.md) and [Touch Virtual Controller](../03-presentation/touch-virtual-controller.md).


## Device I/O and Virtual Controller Security

USB, HID, Serial, microphone, and camera APIs are treated as permissioned device capabilities. Activities and scripts receive granted `DevicePort<T>` values, not raw backend access.

Product-mode defaults:

```text
raw_usb = false
hid = false unless allowlisted
serial = false unless allowlisted
gamepad = true
virtual_controller = true
web_usb = false unless allowlisted and user-granted
web_hid = false unless allowlisted and user-granted
```

Raw packets are not logged by default. Device use requires audit events and, for camera/microphone/raw USB/HID, visible indicators or equivalent product UI. Virtual controller events are safe logical inputs and may be enabled in product mode, but debug-only state mutation through controller injection requires an Agent capability token.


## USB / HID / Serial devices

USB/HID/Serial access is high risk and must be capability gated.

Rules:

- Devices must be declared as `device` ModuleItems or generated device profiles.
- Product builds require explicit allow-list entries for VID/PID, HID usage, or serial profile.
- Game logic consumes typed ports and parsed events, not raw packets.
- Raw packets are never exposed to Agent tools, logs, telemetry, or plugins unless a signed debug capability enables it.
- WebUSB/WebHID/Web Serial must use browser permission flows and secure contexts.
- Activities receive granted device ports; they cannot enumerate or open devices directly.
- CI/headless uses virtual devices and fixture streams.

See [USB / HID / Serial device support](../03-presentation/usb-devices.md) and [Device Generator / Profile System](device-generator.md).

## Virtual touch controller

Virtual touch controllers are product-safe UI components, but debug-only controls must be marked and hidden from product builds. They emit typed input events through the LayerTree and must not directly mutate `GameState`.

See [Virtual Touch Controller](../03-presentation/virtual-controller.md).

## USB / HID / capture / virtual controller security

Arcweft treats live devices as capability-gated resources.

Rules:

- Camera, microphone, USB, HID, and WebUSB/WebHID all require explicit manifest declarations.
- Product builds must not allow wildcard USB filters without signed QA capability.
- Raw audio/video/USB packet payloads are not exposed to LLM/MCP tools by default.
- Device serial numbers are redacted unless debug capability is enabled.
- Activities receive granted ports and normalized `InputAction` streams, not backend handles.
- Standard gamepads should use `gilrs` or browser Gamepad API instead of raw USB/HID.
- Virtual controllers emit the same `InputAction` values as physical devices and are visible to Agent Debug Bus as normal UI action targets.
- WebUSB/WebHID access must be user-gesture/permission gated and may be unavailable on some browsers.
