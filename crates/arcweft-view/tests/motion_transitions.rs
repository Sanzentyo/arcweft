use arcweft_presentation::appearance::PresentationColor;
use arcweft_view::{
    ViewColorValue, ViewEasingFunction, ViewKeyframe, ViewKeyframeTrack, ViewLengthMilli,
    ViewPropertyKind, ViewRatioMilli, ViewReducedMotionPolicy, ViewScalarMilli, ViewSpecifiedValue,
    ViewTimelineMillis, ViewTransition, ViewTransitionSpec,
};

fn ratio(value: u16) -> ViewRatioMilli {
    ViewRatioMilli::new(value).unwrap()
}

fn ratio_value(value: u16) -> ViewSpecifiedValue {
    ViewSpecifiedValue::Ratio {
        value: ratio(value),
    }
}

fn scalar_value(value: u32) -> ViewSpecifiedValue {
    ViewSpecifiedValue::Scalar {
        value: ViewScalarMilli::new(value),
    }
}

fn length_value(value: i32) -> ViewSpecifiedValue {
    ViewSpecifiedValue::Length {
        value: ViewLengthMilli::new(value),
    }
}

fn color(red: u8, green: u8, blue: u8, alpha: u8) -> ViewSpecifiedValue {
    ViewSpecifiedValue::Color {
        value: ViewColorValue::Literal {
            color: PresentationColor::rgba(red, green, blue, alpha),
        },
    }
}

fn transition(
    property: ViewPropertyKind,
    from: ViewSpecifiedValue,
    to: ViewSpecifiedValue,
) -> ViewTransition {
    ViewTransition::new(
        ViewTransitionSpec::new(property, 1_000, ViewEasingFunction::Linear),
        ViewTimelineMillis::ZERO,
        from,
        to,
    )
    .expect("transition is valid")
}

#[test]
fn background_color_transition_interpolates_rgba_channels() {
    let sample = transition(
        ViewPropertyKind::BackgroundColor,
        color(0, 0, 0, 255),
        color(255, 128, 64, 127),
    )
    .sample(ViewTimelineMillis::new(500), ViewReducedMotionPolicy::Full)
    .expect("sample succeeds");

    assert_eq!(sample.linear_progress, ratio(500));
    assert_eq!(sample.sampled_value, color(128, 64, 32, 192));
}

#[test]
fn opacity_scale_and_outline_width_are_transitionable_typed_values() {
    let opacity = transition(
        ViewPropertyKind::Opacity,
        ratio_value(1_000),
        ratio_value(0),
    )
    .sample(ViewTimelineMillis::new(250), ViewReducedMotionPolicy::Full)
    .expect("opacity samples");
    assert_eq!(opacity.sampled_value, ratio_value(751));

    let scale = transition(
        ViewPropertyKind::Scale,
        scalar_value(1_000),
        scalar_value(2_000),
    )
    .sample(ViewTimelineMillis::new(500), ViewReducedMotionPolicy::Full)
    .expect("scale samples");
    assert_eq!(scale.sampled_value, scalar_value(1_500));

    let outline = transition(
        ViewPropertyKind::OutlineWidth,
        length_value(0),
        length_value(4_000),
    )
    .sample(ViewTimelineMillis::new(250), ViewReducedMotionPolicy::Full)
    .expect("outline samples");
    assert_eq!(outline.sampled_value, length_value(1_000));
}

#[test]
fn reduced_motion_policy_can_shorten_or_disable_motion() {
    let transition = transition(
        ViewPropertyKind::TranslateX,
        length_value(0),
        length_value(1_000),
    );

    let shortened = transition
        .sample(
            ViewTimelineMillis::new(50),
            ViewReducedMotionPolicy::Shorten {
                max_duration_ms: 100,
            },
        )
        .expect("shortened sample succeeds");
    assert_eq!(shortened.linear_progress, ratio(500));
    assert_eq!(shortened.sampled_value, length_value(500));

    let disabled = transition
        .sample(ViewTimelineMillis::new(1), ViewReducedMotionPolicy::Disable)
        .expect("disabled sample succeeds");
    assert_eq!(disabled.linear_progress, ViewRatioMilli::ONE);
    assert_eq!(disabled.sampled_value, length_value(1_000));
    assert!(disabled.finished);
}

#[test]
fn interruption_starts_next_transition_from_sampled_value() {
    let first = transition(
        ViewPropertyKind::Opacity,
        ratio_value(0),
        ratio_value(1_000),
    );
    let reversed = first
        .interrupt(
            ViewTimelineMillis::new(500),
            ratio_value(0),
            ViewTransitionSpec::new(ViewPropertyKind::Opacity, 1_000, ViewEasingFunction::Linear),
            ViewReducedMotionPolicy::Full,
        )
        .expect("interruption succeeds");

    assert_eq!(reversed.source_value(), &ratio_value(500));
    let sample = reversed
        .sample(ViewTimelineMillis::new(750), ViewReducedMotionPolicy::Full)
        .expect("reversed sample succeeds");
    assert_eq!(sample.sampled_value, ratio_value(376));
}

#[test]
fn keyframe_track_samples_ordered_offsets() {
    let track = ViewKeyframeTrack::new(
        ViewPropertyKind::Opacity,
        1_000,
        [
            ViewKeyframe::new(ratio(0), ratio_value(0)),
            ViewKeyframe::new(ratio(500), ratio_value(1_000)),
            ViewKeyframe::new(ratio(1_000), ratio_value(0)),
        ],
    )
    .expect("keyframe track is valid");

    let sample = track
        .sample(
            ViewTimelineMillis::ZERO,
            ViewTimelineMillis::new(250),
            ViewReducedMotionPolicy::Full,
        )
        .expect("keyframe sample succeeds");

    assert_eq!(sample.source_value, ratio_value(0));
    assert_eq!(sample.target_value, ratio_value(1_000));
    assert_eq!(sample.sampled_value, ratio_value(500));
}
