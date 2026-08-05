# Example: USB device + virtual touch controller

This example combines a USB/HID-like motion sensor, a serial knob, and a touch-screen virtual controller.

Related docs:

- [USB / HID / Serial device support](../03-presentation/usb-devices.md)
- [Virtual Touch Controller](../03-presentation/virtual-controller.md)
- [Device Generator / Profile System](../05-build-and-security/device-generator.md)

---

## Device declarations

```arcw
mod game.devices

pub device @device.motion_sensor: UsbRaw {
    permission = user_prompt

    match {
        vendor_id = 0x1209
        product_id = 0x00A1
        interface_class = vendor_specific
    }

    endpoint in bulk 0x81 packet = 64

    decoder = decode_motion_packet

    signal motion_sample: Stream<MotionSample>

    emits {
        sample => GameEvent.Device(.Motion(sample.summary()))
    }
}

pub enum KnobEvent {
    Turn { delta: i32 },
    Press,
}

pub device @device.serial_knob: Serial {
    permission = user_prompt
    baud = 115200

    line_decoder = decode_knob_line

    emits {
        .Turn { delta } => InputEvent.AxisDelta(.ViewScroll, delta)
        .Press => InputEvent.ButtonDown(.Confirm)
    }
}
```

---

## Virtual controller

```arcw
mod game.view.touch_controller

pub virtual_controller @controller.default_touch {
    visible when platform.touch_available || settings.force_touch_controls
    layer = @layer.touch_controls

    layout safe_area {
        left = Stick @control.move {
            anchor = bottom_left
            margin = 32
            radius = 92
            dead_zone = 0.12
            output = InputEvent.Axis2(.Move)
        }

        right = ButtonCluster @control.actions {
            anchor = bottom_right
            margin = 32
            buttons = [
                Button @control.action_a {
                    label = "A"
                    output down = InputEvent.ButtonDown(.ActionA)
                    output up = InputEvent.ButtonUp(.ActionA)
                },
                Button @control.menu {
                    label = "≡"
                    output down = InputEvent.ButtonDown(.Menu)
                }
            ]
        }
    }
}
```

---

## Flow usage

```arcw
pub flow device_setup(state: GameState) -> Result<FlowExit, FlowError> {
    let sensor =
        try await device.open(@device.motion_sensor) with {
            pending p => {
                scene.show(@scene.device_wait)
                text.show("USBセンサーの許可を待っています")
                progress.set(p.ratio)
            }

            denied _ => {
                log.warn("motion sensor denied")
                return Ok(FlowExit.Goto(@flow.opening_without_sensor))
            }
        }

    signal.set(@signal.device_ready, true)

    Ok(FlowExit.Goto(@flow.opening))
}
```

---

## Layer integration

```arcw
layer touch_controls {
    kind = view
    input = capture_if_hit
    visible = platform.touch_available || settings.force_touch_controls
    render = overlay
    agent_visible = true
}
```

---

## Agent script

```text
observe()
invoke(control.action_a, .press)
axis(control.move, value=[0.0, -1.0])
inject(device.motion_sensor, MotionSample { accel = [0.0, 1.0, 0.0] })
wait.signal(signal.device_ready, equals=true)
```

---

## Generator commands

```bash
arcw device gen game/devices.arcw
arcw device check
arcw device simulate @device.motion_sensor --fixture fixtures/devices/motion_sensor.ndjson
arcw test game/controllers/touch_controller.arcw
```

