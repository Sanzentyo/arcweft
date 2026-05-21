# Virtual Controller Manifest Schema

Virtual controller manifests describe touch UI controls that emit Arcweft `InputAction` values.

Related chapters:

- [Virtual Touch Controller](../03-presentation/virtual-controller.md)
- [Layer System / Input Routing](../03-presentation/layers.md)
- [Layered Input](../02-runtime/layered-input.md)

## Rust schema sketch

```arcw
pub struct VirtualControllerManifest {
    pub schema_version: u32,
    pub controllers: Vec<VirtualControllerSpec>,
}

pub struct VirtualControllerSpec {
    pub id: PublicId,
    pub layer: Ref<Layer>,
    pub visible_when: ExprId,
    pub output_profile: Ref<InputProfile>,
    pub layout: VirtualControllerLayoutSpec,
    pub controls: Vec<VirtualControlSpec>,
}

pub struct VirtualControlSpec {
    pub id: PublicId,
    pub kind: VirtualControlKind,
    pub label: Option<String>,
    pub anchor: Anchor,
    pub margin: Vec2,
    pub size: Option<Vec2>,
    pub shape: ControlShape,
    pub mapping: InputActionMapping,
    pub feedback: Option<FeedbackSpec>,
    pub agent: AgentTargetPolicy,
}

pub enum VirtualControlKind {
    Button,
    ToggleButton,
    DPad,
    AnalogStick,
    RadialMenu,
    TouchSurface,
    GesturePad,
    Slider,
    RegionTrigger,
}
```

## DSL example

```arcw
pub virtual_controller @vc.truck_touch: VirtualController {
    layer = @layer.touch_controls
    visible_when = env.touch_available && activity == @activity.truck_game
    output input_profile @input.truck_game

    stick @control.truck.steer {
        anchor = bottom_left
        radius = 96
        maps_to = axis2(.Steer)
    }

    button @control.truck.accelerate {
        label = "ACCEL"
        anchor = bottom_right
        size = vec2(96, 96)
        maps_to = button(.Accelerate)
    }
}
```

## Agent requirements

Every visible virtual control must be represented as an `ActionTarget` with:

- stable entity id,
- role,
- label if visible to the player,
- bbox,
- polygon if shape is not rectangular,
- semantic invoke action,
- mapped input action summary.

