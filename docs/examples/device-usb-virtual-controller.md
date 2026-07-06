# Example: USB Device Profile and Virtual Touch Controller

Related:

- [Device Profiles, Generators, and USB](../03-presentation/device-generator-and-usb.md)
- [Virtual Touch Controller](../03-presentation/virtual-controller.md)
- [Layer System / Input Routing](../03-presentation/layers.md)

## USB HID profile

```arcw
mod game.devices.rhythm_pad

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
        @signal.rhythm_pad.axis <- vec2(input.x, input.y).normalize_i16()
        @signal.rhythm_pad.pressure <- input.pressure
    }
}
```

## Virtual controller

```arcw
mod game.ui.mobile_controls

pub controller @controller.mobile_default: VirtualTouchController {
    layer = @layer.controls
    visibility = when platform.touch_available
    safe_area = true

    left_stick @control.left_stick {
        position = bottom_left(x = 96, y = 96)
        radius = 72
        dead_zone = 0.12
        emits axis left
    }

    button @control.action_a {
        label = "A"
        position = bottom_right(x = 120, y = 112)
        radius = 44
        haptic on press { kind = light duration = 30ms }
        emits button A
    }

    button @control.action_b {
        label = "B"
        position = bottom_right(x = 214, y = 72)
        radius = 38
        emits button B
    }
}
```

## Shared controller map

```arcw
pub controller_map @controller_map.action_game {
    button Confirm <- any(button A, keyboard Enter, device @device.rhythm_pad.button(0))
    button Cancel <- any(button B, keyboard Escape, device @device.rhythm_pad.button(1))
    axis Move <- first_active(axis left, device @device.rhythm_pad.axis, keyboard_wasd())
}
```

## Flow usage

```arcw
pub flow @flow.enter_truck_game enter_truck_game(state: GameState) -> Result<FlowExit, FlowError> {
    let pad =
        try await device.open(@device.rhythm_pad).optional() with {
            pending p => {
                scene.show(@scene.device_permission_wait)
                text.show("USBコントローラーの接続を確認しています")
                progress.set(p.ratio)
            }

            denied _ => None
        }

    let result =
        await @<activity.truck_game>.run({
            controller = @controller_map.action_game,
            optional_device = pad,
            virtual_controller = Some(@controller.mobile_default),
        })? with {
            pending p => {
                scene.show(@scene.loading_minigame)
                text.show("ミニゲームを準備中")
                progress.set(p.ratio)
            }
        }

    Ok(FlowExit.Goto(result.next_flow))
}
```

## Test

```arcw
test @test.mobile_controller_drives_truck scenario {
    start(@flow.enter_truck_game)

    wait.object(@control.action_a, state=.visible)

    controller.press(@control.action_a, frames=8)
    controller.axis(@control.left_stick, value=vec2(1.0, 0.0), frames=60)

    expect.signal(@signal.truck.speed, 10.0)
}
```

## Agent CLI

```bash
arcw device generate game/devices/rhythm_pad.arcw
arcw device test device.rhythm_pad --fixture fixtures/devices/rhythm_pad.jsonl
arcw agent invoke control.action_a press
arcw agent controller axis control.left_stick 1.0 0.0 --frames 60
```

