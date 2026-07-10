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

#[test]
fn right_stick_scroll_is_dead_zone_adjusted_and_time_integrated() {
    let mut normalizer = ControllerInputNormalizer::default();
    assert!(
        normalizer
            .normalize(ControllerInputChange::Axis {
                axis: ControllerAxis::RightY,
                value: 0.675,
                time_millis: 10,
            })
            .is_empty()
    );
    let actions = normalizer.normalize(ControllerInputChange::Axis {
        axis: ControllerAxis::RightY,
        value: 0.675,
        time_millis: 110,
    });
    let [NormalizedControllerAction::Scroll { delta_x, delta_y }] = actions.as_slice() else {
        panic!("right-stick sample should normalize to one scroll action");
    };
    assert!(delta_x.abs() < f32::EPSILON);
    assert!((*delta_y + 36.0).abs() < 0.001);

    // A stalled poll is capped, so reconnecting a held stick cannot jump an
    // arbitrary distance through retained content.
    assert_eq!(
        normalizer.normalize(ControllerInputChange::Axis {
            axis: ControllerAxis::RightY,
            value: 1.0,
            time_millis: 10_000,
        }),
        vec![NormalizedControllerAction::Scroll {
            delta_x: 0.0,
            delta_y: -72.0,
        }]
    );
}
