# Device Generator / Profile System

Arcweft should include a generator, but not as a monolithic source-code generator that owns all device logic. The recommended system is a **device profile generator**:

```text
device declaration / descriptor / parser / contract
  -> typed ModuleItems
  -> ports
  -> events
  -> schema
  -> Rust stubs
  -> Web permission descriptors
  -> virtual fixtures
  -> LSP/Agent metadata
```

Manual implementation remains possible through `DeviceDriver` traits, but the generator handles the repetitive and safety-critical glue.

Related docs:

- [USB / HID / Serial device support](../03-presentation/usb-devices.md)
- [Virtual Touch Controller](../03-presentation/virtual-controller.md)
- [Capture Devices](../03-presentation/capture-devices.md)
- [Device Manifest schema](../schemas/device-profile-manifest.md)
- [Virtual Controller Manifest](../schemas/virtual-controller-manifest.md)

---

## Why a generator is useful

USB/HID/serial/capture devices need consistent handling for:

```text
- permission flow
- backend selection
- device matching
- port typing
- parsers
- contracts
- input event mapping
- signal/log wiring
- virtual fixtures
- Agent observation
- product capability policy
- Native/Web divergence
```

Hand-writing all of that makes it easy to forget privacy, bounds, or replay rules. The generator ensures all device definitions produce the same runtime surface.

---

## What the generator consumes

The generator consumes one or more of:

```text
.arcw device declaration
USB descriptor snapshot
HID report descriptor
Serial protocol grammar
Capture device manifest
Virtual controller manifest
Rust driver manifest
Web permission descriptor
```

Example input:

```arcw
pub device #device.serial_knob: Serial {
    baud = 115200
    parser line parse_knob_line: Parser<KnobEvent, ParseError>
    emits { .Turn { delta } => InputEvent.AxisDelta(.UiScroll, delta) }
}
```

---

## What it generates

```text
1. ModuleItem entries
2. Ref<Device> / Ref<Control> IDs
3. DeviceProfile manifest
4. typed DevicePort<T>
5. parser wrappers and diagnostics
6. contract checks
7. virtual backend fixture schema
8. Agent observation schema
9. LSP completions
10. Rust driver stubs where requested
```

Generated items are source-mapped back to the original `.arcw` declaration.

---

## CLI

```bash
arcw device inspect usb --json
arcw device inspect hid --json
arcw device new-profile --from-usb --out devices/motion_sensor.arcw
arcw device gen devices/motion_sensor.arcw --out generated/
arcw device check
arcw device simulate #device.motion_sensor --fixture fixtures/devices/motion.ndjson
arcw device permissions --target web
```

`inspect` never grants device access in product mode. It is a dev tool that still follows host OS/browser permission rules.

---

## Generated Rust driver stub

For complex native devices, the generator can create a Rust stub.

```rust
#[arcweft_device]
pub struct MotionSensorDriver;

impl DeviceDriver for MotionSensorDriver {
    type Input = RawUsbPacket;
    type Output = MotionSample;

    fn decode(&mut self, packet: RawUsbPacket) -> Result<MotionSample, DeviceError> {
        decode_vendor_packet(packet)
    }
}
```

The stub is optional. Simple HID/serial devices can be fully described in `.arcw`.

---

## Generator phases

```text
SourceScan:
  read declarations and public device names

Resolve:
  resolve Ref<Device>, Ref<Signal>, Ref<InputButton>

ParseProfile:
  parse descriptors / report layout / serial grammar

Validate:
  contracts, bounds, capability policy, parser totality

Generate:
  manifests, ModuleItems, Rust stubs, fixtures

Precompile:
  parser tables, report decoders, virtual-device playback metadata
```

The generator is part of `arcweft-precompile` and must be deterministic.

---

## Generated data is not the source of truth

Source of truth:

```text
*.arcw
.arcweft/entities.toml
.arcweft/links.toml
fixtures/devices/*
```

Generated cache:

```text
.arcweft/cache/device-profiles.redb
generated/device-manifests/*.json
generated/rust-stubs/*
```

Only generated files explicitly checked into the project are treated as source.

---

## DeviceDriver trait

Manual drivers implement this trait.

```rust
pub trait DeviceDriver {
    type Config;
    type Ports;

    fn manifest() -> DeviceDriverManifest;

    fn open(&mut self, lease: DeviceLeaseRaw, config: Self::Config)
        -> Need<Result<Self::Ports, DeviceError>, TaskError>;

    fn poll(&mut self, budget: DevicePollBudget) -> Result<Vec<DeviceEvent>, DeviceError>;

    fn close(&mut self) -> Result<(), DeviceError>;
}
```

The driver receives a granted lease, never a global permission to enumerate devices.

---

## Web generation

For Web targets, the generator emits permission descriptors.

```json
{
  "web_usb": {
    "filters": [
      { "vendorId": 4617, "productId": 161 }
    ]
  },
  "web_hid": {
    "filters": [
      { "vendorId": 4617, "productId": 1 }
    ]
  }
}
```

It also emits `web-sys` feature hints:

```toml
web-sys = { features = [
  "Usb", "UsbDevice", "UsbDeviceRequestOptions",
  "Hid", "HidDevice",
  "Serial", "SerialPort"
] }
```

Some Web device APIs are unstable in `web-sys` and require `--cfg=web_sys_unstable_apis`. The generated build report must surface this clearly.

---

## LLM and Agent use

The generator also creates an Agent-readable summary:

```json
{
  "device": "device.motion_sensor",
  "ports": ["motion_sample"],
  "events": ["Device.Motion"],
  "virtual_fixtures": ["fixtures/devices/motion_sensor.ndjson"],
  "security": {
    "raw_packets_exposed": false,
    "product_allowed": false
  }
}
```

This lets LLM debugging tools know how to simulate or inspect devices without raw hardware.

---

## Decision

Use the generator for:

```text
- HID reports
- serial grammar wrappers
- USB profile manifests
- web permission descriptors
- virtual fixtures
- LSP/Agent metadata
```

Use manual Rust drivers for:

```text
- complex vendor protocols
- timing-sensitive hardware
- nontrivial state machines
- devices needing native SDK interop
```

Both paths feed the same `DeviceProfile` and `DevicePort` runtime model.


