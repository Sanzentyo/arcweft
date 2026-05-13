# Device Profile Manifest schema

This schema describes generated and hand-authored device profiles for USB, HID, WebUSB, camera, microphone, gamepad, and virtual devices.

Related:

- [Device Profiles, Generators, and USB](../03-presentation/device-generator-and-usb.md)
- [Capture Device Manifest](capture-device-manifest.md)
- [Virtual Controller Manifest](virtual-controller-manifest.md)
- [Agent Protocol](agent-protocol.md)

## Top-level shape

```rust
pub struct DeviceProfileManifest {
    pub schema_version: u32,
    pub profiles: Vec<DeviceProfileSpec>,
    pub generated: Vec<GeneratedDeviceArtifact>,
    pub fixtures: Vec<DeviceFixtureSpec>,
}
```

## DeviceProfileSpec

```rust
pub struct DeviceProfileSpec {
    pub id: PublicId,
    pub entity: EntityId,
    pub kind: DeviceKind,
    pub permission: DevicePermissionPolicy,
    pub backend: DeviceBackendPolicy,
    pub usb: Option<UsbDescriptorFilter>,
    pub hid: Option<HidDescriptorFilter>,
    pub reports: Vec<ReportSpec>,
    pub endpoints: Vec<EndpointSpec>,
    pub signals: Vec<SignalBinding>,
    pub controller_map: Option<ControllerMapSpec>,
    pub privacy: DevicePrivacyPolicy,
    pub contracts: Vec<ContractSpec>,
}
```

```rust
pub enum DeviceKind {
    UsbRaw,
    UsbHid,
    Hid,
    Gamepad,
    Microphone,
    Camera,
    VirtualController,
    VirtualFixture,
}
```

## Backend policy

```rust
pub struct DeviceBackendPolicy {
    pub native: Vec<DeviceBackend>,
    pub web: Vec<DeviceBackend>,
    pub headless: Vec<DeviceBackend>,
}

pub enum DeviceBackend {
    Nusb,
    Rusb,
    Hidapi,
    WebUsb,
    WebGamepad,
    WebSysMediaDevices,
    Cpal,
    ShiguredoVideoDevice,
    Nokhwa,
    VirtualFixture,
    VirtualPattern,
}
```

## USB filters

```rust
pub struct UsbDescriptorFilter {
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub class: Option<u8>,
    pub subclass: Option<u8>,
    pub protocol: Option<u8>,
    pub interface: Option<u8>,
}
```

Product builds should require at least one of `vendor_id/product_id` or a trusted profile signature for raw USB access.

## HID filters

```rust
pub struct HidDescriptorFilter {
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub usage_page: Option<u16>,
    pub usage: Option<u16>,
}
```

## Reports

```rust
pub struct ReportSpec {
    pub direction: ReportDirection,
    pub report_id: u8,
    pub name: Symbol,
    pub fields: Vec<ReportFieldSpec>,
}

pub enum ReportDirection {
    Input,
    Output,
    Feature,
}

pub struct ReportFieldSpec {
    pub name: Symbol,
    pub ty: TypeRef,
    pub location: BitOrByteRange,
    pub endian: Option<Endian>,
    pub scale: Option<ScaleSpec>,
}
```

## Endpoints

```rust
pub struct EndpointSpec {
    pub name: Symbol,
    pub direction: EndpointDirection,
    pub transfer: UsbTransferKind,
    pub address: u8,
    pub max_packet_size: u16,
    pub parser: Option<ParserRef>,
    pub writer: Option<WriterRef>,
}
```

## Generated artifacts

```rust
pub struct GeneratedDeviceArtifact {
    pub profile: EntityId,
    pub artifact_kind: GeneratedDeviceArtifactKind,
    pub path: Utf8PathBuf,
    pub content_hash: Hash,
}

pub enum GeneratedDeviceArtifactKind {
    DslSummary,
    RustStub,
    ParserTable,
    WebUsbFilterJson,
    AgentMetadata,
    TestFixture,
}
```

## Example JSON

```json
{
  "schema_version": 1,
  "profiles": [
    {
      "id": "device.rhythm_pad",
      "kind": "UsbHid",
      "permission": "UserPrompt",
      "backend": {
        "native": ["Hidapi", "Nusb"],
        "web": ["WebUsb"],
        "headless": ["VirtualFixture"]
      },
      "usb": {
        "vendor_id": 4617,
        "product_id": 40961,
        "class": 3
      },
      "reports": [
        {
          "direction": "Input",
          "report_id": 1,
          "name": "RhythmPadInput",
          "fields": [
            { "name": "buttons", "ty": "u16", "location": { "bits": [0, 16] }, "endian": "little" },
            { "name": "x", "ty": "i16", "location": { "bytes": [2, 4] }, "endian": "little" },
            { "name": "y", "ty": "i16", "location": { "bytes": [4, 6] }, "endian": "little" }
          ]
        }
      ]
    }
  ]
}
```
