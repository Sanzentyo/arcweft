# Device I/O Manifest Schema

The device I/O manifest describes USB, HID, Serial, Gamepad, and virtual input devices.

## Rust-like Schema

```rust
pub struct DeviceIoManifest {
    pub schema_version: u32,
    pub devices: Vec<DeviceSpec>,
    pub protocols: Vec<ProtocolSpec>,
    pub permissions: DevicePermissionPolicy,
}

pub struct DeviceSpec {
    pub id: PublicId,
    pub kind: DeviceKind,
    pub visibility: Visibility,
    pub permission: PermissionMode,
    pub backend: BackendPreference,
    pub filters: Vec<DeviceFilter>,
    pub streams: Vec<SourceSpec>,
    pub replay: ReplayPolicy,
    pub privacy: PrivacyPolicy,
}

pub enum DeviceKind {
    Usb,
    Hid,
    Serial,
    Gamepad,
    VirtualController,
    VirtualFixture,
}

pub enum DeviceBackend {
    NativeNusb,
    NativeRusb,
    NativeHidapi,
    NativeSerial,
    NativeGamepad,
    WebUsb,
    WebHid,
    WebSerial,
    WebGamepad,
    Virtual,
}

pub struct DeviceFilter {
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub class: Option<UsbClass>,
    pub usage_page: Option<u16>,
    pub usage: Option<u16>,
    pub serial_number_hash: Option<Hash>,
}

pub struct SourceSpec {
    pub id: PublicId,
    pub item_type: TypeRef,
    pub error_type: TypeRef,
    pub backpressure: SourceBackpressure,
    pub contracts: Vec<ContractRef>,
}
```

## Example

```toml
schema_version = 1

[[devices]]
id = "device.usb.sensor"
kind = "Usb"
permission = "user_prompt"
backend = ["NativeNusb", "WebUsb"]
replay = "packets_when_test"
privacy = "transient"

[[devices.filters]]
vendor_id = 0x1209
product_id = 0xA001
class = "vendor_specific"

[[devices.streams]]
id = "source.sensor.frames"
item_type = "SensorFrame"
error_type = "SensorError"
backpressure = { BoundedQueue = { len = 4 } }

[[devices]]
id = "device.gamepad.primary"
kind = "Gamepad"
permission = "passive_or_user_prompt"
backend = ["NativeGamepad", "WebGamepad"]
replay = "logical_events"
privacy = "public_input"
```

## Validation Rules

```text
- Raw USB/HID devices require permission=user_prompt unless virtual.
- Product mode requires allowlisted vendor/product IDs.
- Raw packet logging is disabled unless replay policy explicitly allows it.
- Web backends must have fallback behavior.
- Streams must declare backpressure policy.
- Device Activity access must pass through granted DevicePort.
```

## Related

- [Device I/O and USB](../03-presentation/device-io-usb.md)
- [Streams and generators](../02-runtime/streams-generators.md)
- [Virtual controller manifest](virtual-controller-manifest.md)
