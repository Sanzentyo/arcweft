# Touch Virtual Controller

Arcweft includes a Game Native UI virtual controller for touch screens. It is not a separate overlay hack: it is a first-class UI component, an input layer, and an Agent-observable action surface.

Related chapters:

- [Game Native UI](ui-reactive.md)
- [Layer System / Input Routing](layers.md)
- [Layered input runtime](../02-runtime/layered-input.md)
- [Device I/O / USB / HID](device-io-usb.md)
- [Virtual controller manifest](../schemas/virtual-controller-manifest.md)
- [Virtual controller example](../examples/touch-virtual-controller.md)

## Decision

The virtual controller is implemented as:

```text
Reactive UI component
  + LayerTree input consumer/producer
  + InputAction mapper
  + Agent action target provider
  + headless/test virtual input surface
```

It is available on native, web, and headless builds.

## Why UI-owned?

A virtual controller must be visible, themeable, animatable, localizable, testable, and observable by LLM agents. Therefore it belongs to Game Native UI rather than raw input code.

It still emits normalized input actions, so gameplay code does not care whether an action came from:

- touch virtual controller,
- physical gamepad,
- keyboard,
- USB macro pad,
- Agent semantic action,
- replay trace.

## Declaration

```awft
pub virtual_controller #controller.touch_default: TouchController {
    layer = #layer.input.touch_controller
    visible_when = platform.touch_available || settings.force_touch_controller
    opacity = 0.82
    safe_area = true

    left {
        joystick #control.left_stick {
            action_axis x = move_x
            action_axis y = move_y
            radius = 86
            dead_zone = 0.12
            position = anchor(.bottom_left, x = 92, y = 92)
        }
    }

    right {
        button #control.confirm {
            label = "A"
            action = confirm
            position = anchor(.bottom_right, x = 96, y = 112)
        }

        button #control.cancel {
            label = "B"
            action = cancel
            position = anchor(.bottom_right, x = 170, y = 56)
        }

        button #control.menu {
            icon = #vector.icon.menu
            action = open_menu
            position = anchor(.top_right, x = 48, y = 48)
        }
    }
}
```

## UI component form

A controller can also be authored as a component.

```awft
pub component #ui.touch_controller TouchControllerView(
    config: TouchControllerConfig,
) -> View {
    ZStack {
        VirtualJoystick(#control.left_stick)
            .agent_target(#control.left_stick)
            .on_axis |axis| emit InputEvent.Axis {
                x = axis.x,
                y = axis.y,
                source = #controller.touch_default,
            }

        VirtualButton("A")
            .agent_target(#control.confirm)
            .on_press { emit InputEvent.Action(.confirm) }

        VirtualButton("B")
            .agent_target(#control.cancel)
            .on_press { emit InputEvent.Action(.cancel) }
    }
    .layer(#layer.input.touch_controller)
    .hit_test(.opaque_controls_only)
}
```

## Input layer behavior

The controller layer consumes touch events for its controls and emits normalized actions to lower gameplay or UI layers.

```text
raw touch event
  -> LayerTree hit test
  -> virtual controller control
  -> controller state update
  -> normalized InputAction / Axis
  -> routed to gameplay/narrative layer
```

The controller does not consume touches outside its hit regions unless configured.

```awft
layer #layer.input.touch_controller {
    z = 900
    input = true
    render = true
    hit_test = controls_only
    pass_through = true
}
```

## Touch tracking

The controller supports multi-touch:

```rust
pub enum TouchControlState {
    Idle,
    Captured { touch_id: TouchId, start: Vec2, current: Vec2 },
}
```

A joystick captures the first touch inside its base. Buttons can either capture or allow chorded multi-touch. The runtime maps `Started`, `Moved`, `Ended`, and `Cancelled` phases into control state updates.

## Gestures

Gestures are optional. They map to actions or higher-level commands.

```awft
pub gesture #gesture.swipe_skip {
    source = #controller.touch_default
    area = full_screen_except_controls
    kind = swipe(direction = left, min_distance = 120)
    action = skip_text
}

pub gesture #gesture.two_finger_menu {
    kind = tap(count = 2, fingers = 2)
    action = open_menu
}
```

Gestures must respect layer priority and cannot steal touches captured by controls.

## Agent and test support

Every control exposes an `ActionTarget`.

```json
{
  "entity": "control.confirm",
  "role": "VirtualButton",
  "label": "A",
  "bbox": [1120, 560, 72, 72],
  "preferred_action": {
    "type": "invoke",
    "target": "control.confirm",
    "action": "press"
  }
}
```

Tests can use semantic actions instead of coordinates:

```awft
test #test.touch_confirm scenario {
    start #flow.opening
    invoke #control.confirm press
    expect event InputAction.confirm
}
```

Headless sessions can enable virtual touch surfaces:

```bash
arcw agent start --headless --touch-surface 1280x720
arcw agent invoke control.confirm press
arcw agent drag control.left_stick --x 0.7 --y -0.2
```

## Rendering and masks

Virtual controller controls are normal UI nodes. They produce:

- bbox,
- polygon,
- mask,
- role,
- action target,
- current pressed/axis state.

This makes the controller visible to visual tests and LLM debugging.

## Product policy

Product builds may expose virtual controller settings:

```awft
pub state TouchControllerSettings {
    enabled: bool = auto
    opacity: f32 = 0.82
    scale: f32 = 1.0
    layout: TouchControllerLayout = .Default
}
```

Rules:

- controller visibility is user-configurable;
- controls respect safe area;
- button labels/icons are localizable;
- accessibility labels are required;
- Agent tools can observe control geometry in product observe mode, but synthetic input requires control capability.

## External references

- winit touch events: <https://docs.rs/winit/latest/winit/event/>
- Gamepad API: <https://developer.mozilla.org/en-US/docs/Web/API/Gamepad_API>
