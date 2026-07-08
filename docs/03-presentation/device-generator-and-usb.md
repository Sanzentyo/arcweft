# Device Profiles, Generators, and USB

Arcweft supports microphones, cameras, gamepads, touch virtual controllers, HID accessories, serial-like USB devices, and custom USB peripherals through one shared device model.

Related chapters:

- [Microphone / Camera Capture Devices](capture-devices.md)
- [Virtual Touch Controller](virtual-controller.md)
- [Layer System / Input Routing](layers.md)
- [Layered Input runtime](../02-runtime/layered-input.md)
- [Agent Debug MCP / CLI](../04-tooling/agent-debug-mcp-cli.md)
- [Security](../05-build-and-security/security.md)
- [Device Profile Manifest](../schemas/device-profile-manifest.md)
- [USB / Virtual Controller Example](../examples/device-usb-virtual-controller.md)

## Decision

Arcweft does need generator support, but not as a loose textual code generator. The recommended mechanism is a **Device Profile Generator**:

```text
Device manifest / USB descriptors / HID report descriptors / Web constraints
  -> typed Arcweft device profile
  -> generated parsers and contracts
  -> generated backend adapter stubs
  -> generated signals, input events, agent targets, fixtures
  -> checked into source or emitted into precompile cache
```

The generated artifacts are first-class `ModuleItem`s, like shader precompile output, macro output, and Rust-exported Activity manifests.

## Why not only hand-written drivers?

Hand-written drivers are still necessary for non-trivial devices, but they should live behind generated interfaces. USB and HID devices have many repetitive pieces:

- vendor/product filters,
- interface and endpoint selection,
- HID report IDs,
- binary parser layouts,
- permission policy,
- signal bindings,
- test fixtures,
- Agent observation labels,
- WebUSB request filters,
- native backend feature flags.

A manifest-driven generator reduces mistakes and lets LSP, verification, tests, and Agent tools understand the device without reverse-engineering arbitrary Rust.

## Core crate additions

```text
arcweft-device-core
arcweft-device-profile
arcweft-device-generator
arcweft-device-lsp
arcweft-device-agent

arcweft-usb-core
arcweft-usb-nusb
arcweft-usb-rusb
arcweft-usb-webusb

arcweft-hid-core
arcweft-hid-hidapi
arcweft-hid-nusb
arcweft-hid-webhid   # optional future, if target browsers/project permit it

arcweft-controller-core
arcweft-controller-virtual
arcweft-controller-gamepad
arcweft-controller-usb
```

`arcweft-device-core` defines the stable API. Backend crates only implement it.

## USB backend policy

| Backend | Role |
|---|---|
| `nusb` | Preferred native low-level USB backend. Async/blocking APIs, descriptors, interface management, endpoint transfers. |
| `rusb` | Compatibility backend for existing libusb-oriented workflows. Useful where libusb ecosystem support is already needed. |
| `hidapi` | Preferred native HID backend for keyboards, controllers, custom HID devices, feature reports, and OS HID paths. |
| `web-sys` WebUSB | Web backend for USB devices where browsers support WebUSB and user permission is granted. |
| Web Gamepad API | Preferred Web path for standard controllers, separate from raw USB. |
| Virtual backend | Headless/CI/LLM/debug fixture path. |

Arcweft should not expose raw USB access directly to scripts. Scripts request a capability and receive a typed device port.

```text
DSL / Activity
  -> request device profile
  -> permission + backend selection
  -> typed DevicePort
  -> parsed events / signals / command methods
```

## Native USB choices

### `nusb` first

Use `nusb` as the default native low-level USB backend because it is pure Rust-oriented, cross-platform, and explicitly supports async and blocking APIs for listing, watching, descriptors, opening interfaces, and endpoint transfers.

```toml
[features]
device-usb-nusb = ["dep:nusb"]
```

### `rusb` compatibility

Use `rusb` when a driver already expects libusb semantics, when a platform setup is known to work better with libusb, or when a device-specific library depends on it.

```toml
[features]
device-usb-rusb = ["dep:rusb"]
```

### HID is separate from raw USB

Many controllers and accessories are HID devices. Do not force HID through raw USB endpoint handling when an OS HID backend is better.

```toml
[features]
device-hid-hidapi = ["dep:hidapi"]
```

HID reports should be described with `report` schemas and parsed into typed events.

## Web USB policy

WebUSB is permission-gated and browser-dependent. Arcweft uses `web-sys` for WebUSB APIs and keeps WebUSB behind explicit device profiles.

```toml
[features]
device-webusb = ["dep:web-sys", "dep:wasm-bindgen", "dep:js-sys"]
```

WebUSB constraints:

- secure context is required,
- user prompt is required,
- browser support is not universal,
- request filters must be explicit,
- product builds must never request arbitrary USB access without clear View,
- raw data must not be exposed to Agent tools unless capability permits it.

## Device profile DSL

```arcw
pub device @device.rhythm_pad: UsbHid {
    permission = user_prompt

    usb {
        vendor_id = 0x1209
        product_id = 0xA001
        class = hid
    }

    backend {
        native = prefer(hidapi, nusb)
        web = webusb
        headless = virtual_fixture("fixtures/devices/rhythm_pad.jsonl")
    }

    reports {
        input report 1 RhythmPadInput {
            buttons: u16 at bits(0..16)
            x: i16 at bytes(2..4) endian = little
            y: i16 at bytes(4..6) endian = little
            pressure: u8 at byte(6)
        }

        output report 2 RhythmPadLights {
            led_mask: u8 at byte(0)
            brightness: u8 at byte(1)
        }
    }

    signals {
        @signal.rhythm_pad.buttons <- input.buttons
        @signal.rhythm_pad.axis <- vec2(input.x, input.y)
        @signal.rhythm_pad.pressure <- input.pressure
    }

    maps_to controller @controller.rhythm_pad {
        button A <- input.buttons.bit(0)
        button B <- input.buttons.bit(1)
        axis left <- vec2(input.x, input.y).normalize_i16()
    }
}
```

This declaration generates:

- `Ref<DeviceProfile>` entity,
- typed parser for input reports,
- typed writer for output reports,
- `DevicePort<RhythmPadInput, RhythmPadLights>`,
- signal bindings,
- controller mapping,
- WebUSB filter,
- native backend adapter stub,
- fixture reader,
- Agent observation metadata,
- contract and test skeleton.

## Generated Rust shape

Conceptually generated API:

```rust
pub struct RhythmPadInput {
    pub buttons: u16,
    pub x: i16,
    pub y: i16,
    pub pressure: u8,
}

pub struct RhythmPadLights {
    pub led_mask: u8,
    pub brightness: u8,
}

pub type RhythmPadPort = DevicePort<RhythmPadInput, RhythmPadLights>;
```

Generated contracts:

```arcw
ensures parse_input_report(bytes).is_ok() => bytes.len() >= 7
ensures output_report_bytes(value).len() == 3
```

Generated tests:

```arcw
test @test.rhythm_pad_report_parse fixture {
    let bytes = hex("01 03 00 10 00 00 7f")
    let report = parse RhythmPadInput from bytes?
    assert(report.buttons == 3)
}
```

## Runtime use

Opening a device is `Need<Result<DevicePort, DeviceError>, TaskError>` and must show pending/denied View in player-visible flows.

```arcw
let pad =
    try await device.open(@device.rhythm_pad) with {
        pending p => {
            scene.show(@scene.device_permission_wait)
            text.show("USBデバイスの接続許可を待っています")
            progress.set(p.ratio)
        }

        denied e => {
            log.warn("USB device denied: {e:?}", e = e)
            return Ok(FlowExit::Goto(@flow.device_optional))
        }
    }
```

Once granted, scripts do not receive raw handles. They receive a typed port and signals.

```arcw
watch signal @signal.rhythm_pad.buttons from pad.latest().buttons
```

## USB command output

Output reports and bulk writes are commands, not arbitrary host calls.

```arcw
command device @device.rhythm_pad send RhythmPadLights {
    led_mask = 0b0000_1111
    brightness = 180
}
```

The command is validated by profile contracts and capability policy.

## Bulk/control/interrupt endpoints

For non-HID USB, declare endpoint ports:

```arcw
pub device @device.led_board: UsbRaw {
    permission = user_prompt

    usb {
        vendor_id = 0x1209
        product_id = 0xA010
        interface = 1
    }

    endpoints {
        input status: interrupt endpoint 0x81 packet = 64
        output command: bulk endpoint 0x02 packet = 64
        control config: vendor request_type = out
    }

    parser status: LedBoardStatus from status.bytes
    writer command: LedBoardCommand to command.bytes
}
```

Raw endpoint bytes can only enter user code after parsing.

## Security rules

- A device profile must declare vendor/product filters or HID usage filters.
- Product builds cannot expose a generic “all USB devices” picker.
- Raw USB bytes are not sent to LLM tools or logs unless explicit debug capability is enabled.
- Device output commands require a capability and profile contract.
- WebUSB must use explicit filters.
- Device profiles can be disabled by product policy.
- Headless tests must use virtual devices or recorded fixtures.

## Agent observation

Agent observation exposes typed state, not raw sensitive bytes by default.

```json
{
  "device": "device.rhythm_pad",
  "status": "connected",
  "signals": {
    "signal.rhythm_pad.buttons": 3,
    "signal.rhythm_pad.axis": [0.12, -0.02]
  },
  "actions": [
    {
      "kind": "invoke",
      "target": "device.rhythm_pad",
      "action": "send_lights"
    }
  ]
}
```

## CLI

```bash
arcw device list
arcw device inspect device.rhythm_pad
arcw device generate game/devices/rhythm_pad.arcw --out generated/
arcw device test device.rhythm_pad --fixture fixtures/devices/rhythm_pad.jsonl
arcw usb list --backend nusb
arcw usb inspect --vid 1209 --pid A001
arcw hid list
```

## MCP tools

```text
arcweft.device_list
arcweft.device_inspect
arcweft.device_open
arcweft.device_close
arcweft.device_signal_get
arcweft.device_send_command
arcweft.usb_list
arcweft.hid_list
```

Product mode should usually expose only `device_signal_get` for already-granted profiles.

## Generator output policy

Generated files may be handled in two ways:

```text
Source checked-in mode:
  generated/arcweft/devices/*.arcw
  generated/arcweft/devices/*.rs

Cache mode:
  .arcweft/cache/devices/*
```

For stable projects, check in generated `.arcw` summaries and Rust stubs. Keep binary tables in cache.

## Implementation order

1. `arcweft-device-core`: profile, port, event, signal model.
2. `arcweft-device-generator`: manifest -> typed parser/writer/signal bindings.
3. HID backend via `hidapi` for controllers/accessories.
4. USB backend via `nusb`.
5. WebUSB via `web-sys`.
6. Raw USB endpoint profiles.
7. Agent/MCP/CLI inspection.

