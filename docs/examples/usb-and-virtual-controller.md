# USB and Virtual Touch Controller Example

This example combines a custom USB light gun, a virtual touch controller, and the layer-based input system.

Related chapters:

- [USB / HID Devices](../03-presentation/usb-devices.md)
- [Virtual Touch Controller](../03-presentation/virtual-controller.md)
- [Device Streams](../02-runtime/device-streams.md)
- [Layered Input](../02-runtime/layered-input.md)

## USB report

```arcw
pub struct LightgunReport {
    x: u16,
    y: u16,
    trigger: bool,
    confidence: u8,
}

pub fn decode_lightgun_report(input: &[u8])
    -> Result<LightgunReport, UsbParseError>
requires input.len() >= 6 {
    Ok(LightgunReport {
        x = try le_u16(input[0..2]),
        y = try le_u16(input[2..4]),
        trigger = input[4] & 0x01 != 0,
        confidence = input[5],
    })
}
```

## USB device

```arcw
pub usb @usb.lightgun: UsbRawDevice {
    permission = user_prompt
    filter { vendor_id = 0xCAFE; product_id = 0x4001 }
    claim interface 0

    endpoint @usb.lightgun.input: InterruptIn {
        address = 0x81
        packet = LightgunReport
        decoder = decode_lightgun_report
        backpressure = latest
    }
}
```

## Input map

```arcw
input_map @input.lightgun_map {
    source @usb.lightgun.input

    on report r when r.confidence >= 40 => {
        event.emit(
            InputAction.PointerAim,
            x = r.x.normalized(0..1920),
            y = r.y.normalized(0..1080),
            space = .LogicalViewport,
        )

        if r.trigger {
            event.emit(InputAction.ButtonDown(.Fire))
        }
    }
}
```

## Touch fallback

```arcw
layer touch_controls {
    z = 900
    kind = view_overlay
    visibility = env.touch_available
    input { accepts = touch | pointer; capture = handled; pass_through = true }
}

pub virtual_controller @vc.shooter_touch: VirtualController {
    layer = @layer.touch_controls
    visible_when = env.touch_available && activity == @activity.shooting_gallery
    output input_profile @input.shooter

    touch_surface @control.shooter.aim {
        rect = safe_area
        maps_to = pointer_aim(.LogicalViewport)
    }

    button @control.shooter.fire {
        label = "FIRE"
        anchor = bottom_right
        margin = vec2(32, 32)
        size = vec2(108, 108)
        maps_to = button(.Fire)
    }
}
```

## Flow

```arcw
flow @flow.shooting_gallery_intro opening(state: GameState) -> Result<FlowExit, FlowError> {
    let gun = match usb.open(@usb.lightgun).optional() {
        .Err(.Denied(_)) => None
        .Err(error) => return Err(error.into())
        .Ok(request) => await request with {
            pending p => {
                scene.show(@scene.usb_wait)
                text.show("専用コントローラーを探しています。タッチ操作でも遊べます。")
                progress.set(p.ratio)
            }
        }
    }

    let result =
        try await @<activity.shooting_gallery>.run({
            usb_lightgun = gun,
            touch_controller = Some(@vc.shooter_touch),
        }) with {
            pending p => scene.show(@scene.loading_activity); progress.set(p.ratio)
        }

    if result.score >= 1000 {
        Ok(FlowExit.Goto(@flow.secret_route))
    } else {
        Ok(FlowExit.Goto(@flow.normal_route))
    }
}
```

## Test

```arcw
test @test.shooter_touch_fallback scenario {
    goto @flow.shooting_gallery_intro
    permission.deny(.usb, @usb.lightgun)

    wait.object(@control.shooter.fire, state=.visible)
    invoke(@control.shooter.aim, .set_pointer, x=0.5, y=0.5)
    invoke(@control.shooter.fire, .press)

    expect.input_action(ButtonDown(.Fire))
}
```

