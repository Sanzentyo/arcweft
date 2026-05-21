# Example: Touch Virtual Controller

```arcw
mod game::ui::virtual_controller

pub virtual_controller @controller.touch.default {
    layer = @layer.input.overlay
    visibility = when platform.touch_available
    input_scope = gameplay

    layout {
        stick @control.left_stick {
            position = bottom_left(margin = 32)
            radius = 72
            deadzone = 0.12
            action = InputAction.Move
        }

        button @control.confirm {
            label = "A"
            position = bottom_right(x = 64, y = 96)
            action = InputAction.Confirm
        }

        button @control.cancel {
            label = "B"
            position = bottom_right(x = 132, y = 32)
            action = InputAction.Cancel
        }
    }
}
```

Input mapping:

```arcw
input_map @input.touch_controller {
    source @controller.touch.default
    map StickChanged(@control.left_stick, x, y) => InputAction.Move { x, y }
    map ButtonDown(@control.confirm) => InputAction.Confirm
    map ButtonDown(@control.cancel) => InputAction.Cancel
}
```

Agent operation:

```bash
arcw agent invoke control.confirm press
arcw agent invoke control.left_stick set '{"x": 0.4, "y": -0.7}'
arcw agent observe --objects --json
```

Headless test:

```arcw
test @test.virtual_controller_confirm scenario {
    start @flow.action_demo

    invoke @control.confirm press

    expect log.info contains "confirm pressed"
    expect signal @signal.last_input == InputAction.Confirm
}
```

