# Device I/O, USB, HID, and Gamepads

Arcweft supports hardware devices as permissioned ports, not as raw host APIs exposed to scripts. This covers native USB, browser WebUSB, HID, WebHID, standard gamepads, and virtual devices used for tests or LLM debugging.

Related chapters:

- [Streams and generators](../02-runtime/streams-generators.md)
- [Layered input](../02-runtime/layered-input.md)
- [Touch virtual controller](touch-virtual-controller.md)
- [Security](../05-build-and-security/security.md)
- [Device I/O manifest](../schemas/device-io-manifest.md)
- [USB example](../examples/device-io-usb.md)

## Decision

Arcweft uses a layered device model:

```text
High-level game input:
  InputAction / Axis / Button / Gesture

Standard controller input:
  Gamepad backend

Generic HID:
  HID reports with typed parser

Raw USB:
  control / bulk / interrupt transfers

Web:
  WebUSB / WebHID / Gamepad API through web-sys
```

Recommended backends:

```text
Native raw USB:
  nusb as default
  rusb/libusb as optional compatibility backend

Native HID:
  hidapi as compatibility backend
  async-hid/nusb-based drivers can be added per device class

Native gamepads:
  gilrs

Web raw USB:
  web-sys WebUSB

Web HID:
  web-sys WebHID

Web standard gamepads:
  web-sys Gamepad API
```

## Why `nusb` first for raw USB?

`nusb` is a pure-Rust cross-platform low-level USB library. It supports Windows, macOS, and Linux and exposes both async and blocking APIs for listing/watching USB devices, opening devices/interfaces, and performing control, bulk, and interrupt transfers.

`rusb` remains useful because it wraps native libusb and has a mature ecosystem, but Arcweft treats it as an optional backend because it depends on libusb and is less aligned with a pure-Rust async runtime.

## Browser constraints

WebUSB and WebHID are not transparent equivalents of native USB/HID:

- they are permission-gated,
- they require secure contexts,
- they are not available in all browsers,
- `requestDevice()` triggers a user selection / pairing flow,
- device filters should be narrow,
- product builds must surface active device indicators and audit logs.

Therefore Web device access is always represented as `Need<Result<DeviceHandle, DeviceError>, TaskError>` and must use `await ... with` in player-visible flows.

## Device declarations

```awft
pub device #device.light_panel: Usb {
    permission = user_prompt
    backend = prefer(native_nusb, web_usb)

    filter {
        vendor_id = 0x1209
        product_id = 0xA001
        class = vendor_specific
    }

    interface = 0
    endpoints {
        out = bulk_out(0x01)
        in  = interrupt_in(0x81)
    }

    privacy = transient
    product_access = disabled_by_default
}
```

HID device:

```awft
pub device #device.macro_pad: Hid {
    permission = user_prompt
    backend = prefer(native_hidapi, web_hid)

    filter {
        vendor_id = 0x1209
        product_id = 0xA010
        usage_page = 0xFF60
    }

    reports {
        input  #report.macro_pad.input  = bytes(32)
        output #report.macro_pad.output = bytes(32)
    }
}
```

Standard gamepad:

```awft
pub device #device.primary_gamepad: Gamepad {
    backend = prefer(native_gilrs, web_gamepad)
    layout = standard
    dead_zone = 0.12
    normalize = true
}
```

## Starting a device

```awft
let panel =
    await device.usb(#device.light_panel)? with {
        pending p => scene #scene.usb_permission_wait {
            text "USB ライトパネルの許可を待っています"
            progress p.ratio
        }

        denied _ => {
            log warn "USB device permission denied"
            return Ok(FlowExit::Goto(#flow.no_usb_fallback))
        }
    }
```

The handle is not a raw platform handle. It is a capability-scoped Arcweft handle.

```rust
pub struct UsbDeviceHandle {
    device: DevicePortId,
    capabilities: UsbCapabilities,
}
```

## Typed protocols

Raw USB/HID bytes should be parsed immediately into typed messages.

```awft
pub enum LightPanelCommand {
    SetColor { led: u8, color: Color },
    SetBrightness { value: u8 },
}

pub parser parse_light_panel_event: Parser<LightPanelEvent, ParseError>
input &[u8]
ensures result.is_err() => result.err().span.is_some()
{
    ...
}
```

Sending:

```awft
task fn set_panel_color(
    panel: UsbDeviceHandle,
    led: u8,
    color: Color,
) -> Result<Unit, DeviceError>
requires led < 64
{
    let packet = encode_light_panel_command(.SetColor { led, color })?
    await panel.bulk_out(endpoint = 0x01, packet)?
    Ok(())
}
```

Receiving:

```awft
let events: Stream<LightPanelEvent, DeviceError> =
    panel.interrupt_in(0x81)
        .parse_with(parse_light_panel_event)
```

## Mapping devices to input actions

Hardware events are mapped into layer-based input.

```awft
pub input_map #input_map.gamepad_default for #device.primary_gamepad {
    button South -> action confirm
    button East  -> action cancel
    axis LeftX   -> axis move_x
    axis LeftY   -> axis move_y
}

pub input_map #input_map.macro_pad for #device.macro_pad {
    report key_1 -> action advance_text
    report key_2 -> action open_settings
}
```

Layer routing then decides which layer consumes the action.

```text
Touch virtual controller layer
  -> gameplay layer
  -> narrative UI layer
  -> system menu layer
```

## Product security

USB/HID are Tier 3/4 capabilities by default.

Rules:

- scripts cannot enumerate devices directly;
- device declarations must specify filters;
- product builds require explicit feature flags;
- WebUSB/WebHID require a user gesture and secure context;
- raw bytes are never logged by default;
- only typed summaries may enter Agent Debug Bus unless debug capability is enabled;
- out-of-process Activity plugins cannot open USB directly; they receive a granted port or message stream.

## Feature flags

```toml
[features]
device-usb-nusb = ["dep:nusb"]
device-usb-rusb = ["dep:rusb"]
device-hid = ["dep:hidapi"]
device-gamepad-gilrs = ["dep:gilrs"]
device-webusb = ["dep:web-sys", "dep:wasm-bindgen"]
device-webhid = ["dep:web-sys", "dep:wasm-bindgen"]
device-web-gamepad = ["dep:web-sys", "dep:js-sys"]
device-virtual = []
```

## External references

- `nusb`: <https://docs.rs/nusb/latest/nusb/>
- `rusb`: <https://docs.rs/rusb>
- WebUSB: <https://developer.mozilla.org/en-US/docs/Web/API/WebUSB_API>
- WebHID: <https://developer.mozilla.org/en-US/docs/Web/API/HID/requestDevice>
- `hidapi`: <https://docs.rs/hidapi>
- `gilrs`: <https://docs.rs/gilrs/>
- Gamepad API: <https://developer.mozilla.org/en-US/docs/Web/API/Gamepad_API>
