# USB Device Manifest Schema

USB manifests describe raw USB and HID devices that Arcweft may access. They are source-controlled and capability-gated.

Related chapters:

- [USB / HID Devices](../03-presentation/usb-devices.md)
- [Device Streams](../02-runtime/device-streams.md)
- [Security](../05-build-and-security/security.md)

## Rust schema sketch

```rust
pub struct UsbDeviceManifest {
    pub schema_version: u32,
    pub devices: Vec<UsbDeviceSpec>,
    pub hid_devices: Vec<HidDeviceSpec>,
    pub virtual_devices: Vec<VirtualDeviceSpec>,
}

pub struct UsbDeviceSpec {
    pub id: PublicId,
    pub permission: PermissionPolicy,
    pub privacy: PrivacyPolicy,
    pub backend: UsbBackendPolicy,
    pub filters: Vec<UsbFilter>,
    pub interfaces: Vec<UsbInterfaceSpec>,
    pub endpoints: Vec<UsbEndpointSpec>,
    pub product_mode: ProductModePolicy,
}

pub enum UsbBackendPolicy {
    Auto,
    NativeNusb,
    NativeRusb,
    WebUsb,
    VirtualFixture,
}

pub struct UsbFilter {
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub class: Option<UsbClass>,
    pub subclass: Option<u8>,
    pub protocol: Option<u8>,
    pub serial_policy: SerialPolicy,
}

pub struct UsbEndpointSpec {
    pub id: PublicId,
    pub interface: u8,
    pub address: u8,
    pub transfer: UsbTransferKind,
    pub direction: UsbDirection,
    pub packet_type: StableTypeId,
    pub parser: Option<Ref<Parser>>,
    pub backpressure: BackpressurePolicy,
}

pub enum UsbTransferKind {
    Control,
    Bulk,
    Interrupt,
    IsochronousUnsupported,
}
```

Isochronous transfers are intentionally unsupported in the initial Arcweft USB layer. Camera and audio devices should use the capture/audio systems instead.

## DSL example

```arcw
pub usb @usb.custom_lightgun: UsbRawDevice {
    permission = user_prompt
    backend = auto
    filter { vendor_id = 0xCAFE; product_id = 0x4001 }
    claim interface 0
    endpoint @usb.custom_lightgun.input: InterruptIn {
        address = 0x81
        packet = LightgunReport
        parser = parse_lightgun_report
        backpressure = latest
    }
}
```

## Product restrictions

- `vendor_id` and `product_id` should be specified for product builds.
- Serial numbers are not exported to logs unless debug capability is enabled.
- Raw packet logging is disabled by default.
- Wildcard filters require signed QA capability.
- WebUSB and WebHID must display a user-facing reason string.

