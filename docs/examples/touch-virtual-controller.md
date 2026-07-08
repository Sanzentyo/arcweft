# Example: Touch Virtual Controller

```arcw
mod game.view.touch_controller

pub layer @layer.input.touch_controller {
    z = 900
    render = true
    input = true
    hit_test = controls_only
    pass_through = true
}

pub virtual_controller @controller.touch_default: TouchController {
    layer = @layer.input.touch_controller
    visible_when = platform.touch_available || state.settings.force_touch_controller
    opacity = state.settings.touch_controller.opacity
    safe_area = true

    left {
        joystick @control.left_stick {
            action_axis x = move_x
            action_axis y = move_y
            radius = 86
            dead_zone = 0.12
            position = anchor(.bottom_left, x = 92, y = 92)
        }
    }

    right {
        button @control.confirm {
            label = "A"
            action = confirm
            position = anchor(.bottom_right, x = 96, y = 112)
        }

        button @control.cancel {
            label = "B"
            action = cancel
            position = anchor(.bottom_right, x = 170, y = 56)
        }
    }
}
```

## Test

```arcw
test @test.touch_controller_confirm scenario {
    goto @flow.opening
    invoke(@control.confirm, .press)
    expect.event(InputAction.confirm)
}
```

## Agent script

```text
observe()
invoke(control.confirm, .press)
drag(control.left_stick, .axis, x=0.6, y=-0.3)
wait.signal(signal.current_flow, equals=flow.alice_intro)
```

