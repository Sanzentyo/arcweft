# Device Manifest schema

The Device Manifest is the generated and optionally versioned representation of USB, HID, serial, capture, and virtual devices.

Source docs:

- [USB / HID / Serial device support](../03-presentation/usb-devices.md)
- [Device Generator / Profile System](../05-build-and-security/device-generator.md)
- [Virtual Controller Manifest](virtual-controller-manifest.md)

---

## Top-level structure

```rust
pub struct DeviceManifest {
    pub schema_version: u32,
    pub devices: Vec<DeviceProfile>,
    pub generated_from: Vec<SourceAnchor>,
    pub permissions: DevicePermissionPolicy,
}
```

JSON shape:

```json
{
  "schema_version": 1,
  "devices": [],
  "permissions": {},
  "generated_from": []
}
```

---

## DeviceProfile

```rust
pub struct DeviceProfile {
    pub entity: PublicId,
    pub kind: DeviceKind,
    pub visibility: Visibility,
    pub match_rules: Vec<DeviceMatchRule>,
    pub ports: Vec<DevicePortSpec>,
    pub parsers: Vec<ParserSpec>,
    pub signals: Vec<SignalSpec>,
    pub events: Vec<EventMapping>,
    pub contracts: Vec<ContractSpec>,
    pub backend: BackendPolicy,
    pub privacy: PrivacyPolicy,
    pub product_policy: ProductPolicy,
}
```

---

## DeviceKind

```rust
pub enum DeviceKind {
    UsbRaw,
    UsbHid,
    UsbSerial,
    Serial,
    Gamepad,
    Sensor,
    Camera,
    Microphone,
    Virtual,
}
```

---

## Match rules

```json
{
  "vendor_id": "0x1209",
  "product_id": "0x00A1",
  "interface_class": "vendor_specific",
  "usage_page": "0x0001",
  "serial_number": null
}
```

`serial_number` is `Option<String>`. Arcweft does not use null in the DSL, but JSON manifests may encode `None` as `null` for interoperability.

---

## Ports

```rust
pub struct DevicePortSpec {
    pub name: String,
    pub direction: PortDirection,
    pub ty: TypeSignature,
    pub replay_policy: ReplayPolicy,
    pub exposure: AgentExposure,
}
```

Example:

```json
{
  "name": "motion_sample",
  "direction": "In",
  "type": "Stream<MotionSample>",
  "replay_policy": "RecordSummary",
  "exposure": "SummaryOnly"
}
```

---

## Event mapping

```rust
pub struct EventMapping {
    pub source_port: String,
    pub condition: Option<Expr>,
    pub output_event: Expr,
}
```

Example:

```json
{
  "source_port": "button_report",
  "condition": "report.button == 1",
  "output_event": "InputEvent.ButtonDown(.ActionA)"
}
```

---

## Permission policy

```rust
pub struct DevicePermissionPolicy {
    pub native: PermissionMode,
    pub web: PermissionMode,
    pub product: ProductPermissionMode,
    pub allowed_backends: Vec<String>,
}
```

```rust
pub enum PermissionMode {
    Deny,
    DevOnly,
    UserPrompt,
    AllowWithPolicy,
}
```

---

## Agent exposure

```rust
pub enum AgentExposure {
    Hidden,
    SummaryOnly,
    ParsedEvents,
    RawDevOnly,
}
```

Product builds must not expose raw USB/HID/serial packets unless a signed debug capability enables it.

---

## Example manifest

```json
{
  "schema_version": 1,
  "devices": [
    {
      "entity": "device.motion_sensor",
      "kind": "UsbRaw",
      "match_rules": [
        {
          "vendor_id": "0x1209",
          "product_id": "0x00A1",
          "interface_class": "vendor_specific"
        }
      ],
      "ports": [
        {
          "name": "motion_sample",
          "direction": "In",
          "type": "Stream<MotionSample>",
          "replay_policy": "RecordSummary",
          "exposure": "ParsedEvents"
        }
      ],
      "backend": {
        "native_primary": "nusb",
        "native_fallback": "rusb",
        "web": "WebUSB"
      },
      "privacy": "transient",
      "product_policy": "user_prompt"
    }
  ]
}
```
