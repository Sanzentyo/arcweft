use arcweft_player_scene::controller::{
    ControllerAxis, ControllerButton, ControllerInputChange, ControllerInputNormalizer,
    NormalizedControllerAction,
};
use arcweft_render_wgpu::geometry::FocusNavigationDirection;

#[test]
fn dpad_confirm_cancel_are_normalized() {
    let mut normalizer = ControllerInputNormalizer::default();
    assert_eq!(
        normalizer.normalize(ControllerInputChange::Button {
            button: ControllerButton::DPadRight,
            pressed: true,
            time_millis: 0,
        }),
        vec![NormalizedControllerAction::Move(
            FocusNavigationDirection::Right
        )]
    );
    assert_eq!(
        normalizer.normalize(ControllerInputChange::Button {
            button: ControllerButton::Confirm,
            pressed: true,
            time_millis: 0,
        }),
        vec![NormalizedControllerAction::Confirm]
    );
    assert_eq!(
        normalizer.normalize(ControllerInputChange::Button {
            button: ControllerButton::Cancel,
            pressed: true,
            time_millis: 0,
        }),
        vec![NormalizedControllerAction::Cancel]
    );
}

#[test]
fn left_stick_dead_zone_and_repeat_are_deterministic() {
    let mut normalizer = ControllerInputNormalizer::default();
    assert!(
        normalizer
            .normalize(ControllerInputChange::Axis {
                axis: ControllerAxis::LeftX,
                value: 0.1,
                time_millis: 0,
            })
            .is_empty()
    );
    assert_eq!(
        normalizer.normalize(ControllerInputChange::Axis {
            axis: ControllerAxis::LeftX,
            value: 0.8,
            time_millis: 1,
        }),
        vec![NormalizedControllerAction::Move(
            FocusNavigationDirection::Right
        )]
    );
    assert!(
        normalizer
            .normalize(ControllerInputChange::Axis {
                axis: ControllerAxis::LeftX,
                value: 0.9,
                time_millis: 100,
            })
            .is_empty()
    );
    assert_eq!(
        normalizer.normalize(ControllerInputChange::Axis {
            axis: ControllerAxis::LeftX,
            value: 0.9,
            time_millis: 321,
        }),
        vec![NormalizedControllerAction::Move(
            FocusNavigationDirection::Right
        )]
    );
}
