use arcweft_ui::{
    Milli, Rgba8, UiEasingFunction, UiKeyframe, UiKeyframeTrack, UiPropertyKind, UiPropertyValue,
    UiReducedMotionPolicy, UiTimelineMillis, UiTransition, UiTransitionSpec,
};

fn transition(
    property: UiPropertyKind,
    from: UiPropertyValue,
    to: UiPropertyValue,
) -> UiTransition {
    UiTransition::new(
        UiTransitionSpec::new(property, 1_000, UiEasingFunction::Linear),
        UiTimelineMillis::ZERO,
        from,
        to,
    )
    .expect("transition is valid")
}

#[test]
fn background_color_transition_interpolates_rgba_channels() {
    let sample = transition(
        UiPropertyKind::BackgroundColor,
        UiPropertyValue::Color(Rgba8::new(0, 0, 0, 255)),
        UiPropertyValue::Color(Rgba8::new(255, 128, 64, 127)),
    )
    .sample(UiTimelineMillis::new(500), UiReducedMotionPolicy::Full)
    .expect("sample succeeds");

    assert_eq!(sample.linear_progress, Milli(500));
    assert_eq!(
        sample.sampled_value,
        UiPropertyValue::Color(Rgba8::new(128, 64, 32, 192))
    );
}

#[test]
fn opacity_scale_and_outline_width_are_transitionable_paint_values() {
    let opacity = transition(
        UiPropertyKind::Opacity,
        UiPropertyValue::Milli(Milli(1_000)),
        UiPropertyValue::Milli(Milli(0)),
    )
    .sample(UiTimelineMillis::new(250), UiReducedMotionPolicy::Full)
    .expect("opacity samples");
    assert_eq!(opacity.sampled_value, UiPropertyValue::Milli(Milli(751)));

    let scale = transition(
        UiPropertyKind::Scale,
        UiPropertyValue::Milli(Milli(1_000)),
        UiPropertyValue::Milli(Milli(2_000)),
    )
    .sample(UiTimelineMillis::new(500), UiReducedMotionPolicy::Full)
    .expect("scale samples");
    assert_eq!(scale.sampled_value, UiPropertyValue::Milli(Milli(1_500)));

    let outline = transition(
        UiPropertyKind::OutlineWidth,
        UiPropertyValue::Milli(Milli(0)),
        UiPropertyValue::Milli(Milli(4_000)),
    )
    .sample(UiTimelineMillis::new(250), UiReducedMotionPolicy::Full)
    .expect("outline samples");
    assert_eq!(outline.sampled_value, UiPropertyValue::Milli(Milli(1_000)));
}

#[test]
fn reduced_motion_policy_can_shorten_or_disable_motion() {
    let transition = transition(
        UiPropertyKind::TranslateX,
        UiPropertyValue::Milli(Milli(0)),
        UiPropertyValue::Milli(Milli(1_000)),
    );

    let shortened = transition
        .sample(
            UiTimelineMillis::new(50),
            UiReducedMotionPolicy::Shorten {
                max_duration_ms: 100,
            },
        )
        .expect("shortened sample succeeds");
    assert_eq!(shortened.linear_progress, Milli(500));
    assert_eq!(shortened.sampled_value, UiPropertyValue::Milli(Milli(500)));

    let disabled = transition
        .sample(UiTimelineMillis::new(1), UiReducedMotionPolicy::Disable)
        .expect("disabled sample succeeds");
    assert_eq!(disabled.linear_progress, Milli::ONE);
    assert_eq!(disabled.sampled_value, UiPropertyValue::Milli(Milli(1_000)));
    assert!(disabled.finished);
}

#[test]
fn interruption_starts_next_transition_from_sampled_value() {
    let first = transition(
        UiPropertyKind::Opacity,
        UiPropertyValue::Milli(Milli(0)),
        UiPropertyValue::Milli(Milli(1_000)),
    );
    let reversed = first
        .interrupt(
            UiTimelineMillis::new(500),
            UiPropertyValue::Milli(Milli(0)),
            UiTransitionSpec::new(UiPropertyKind::Opacity, 1_000, UiEasingFunction::Linear),
            UiReducedMotionPolicy::Full,
        )
        .expect("interruption succeeds");

    assert_eq!(reversed.source_value(), UiPropertyValue::Milli(Milli(500)));
    let sample = reversed
        .sample(UiTimelineMillis::new(750), UiReducedMotionPolicy::Full)
        .expect("reversed sample succeeds");
    assert_eq!(sample.sampled_value, UiPropertyValue::Milli(Milli(376)));
}

#[test]
fn keyframe_track_samples_ordered_offsets() {
    let track = UiKeyframeTrack::new(
        UiPropertyKind::Opacity,
        1_000,
        [
            UiKeyframe::new(Milli(0), UiPropertyValue::Milli(Milli(0))),
            UiKeyframe::new(Milli(500), UiPropertyValue::Milli(Milli(1_000))),
            UiKeyframe::new(Milli(1_000), UiPropertyValue::Milli(Milli(0))),
        ],
    )
    .expect("keyframe track is valid");

    let sample = track
        .sample(
            UiTimelineMillis::ZERO,
            UiTimelineMillis::new(250),
            UiReducedMotionPolicy::Full,
        )
        .expect("keyframe sample succeeds");

    assert_eq!(sample.source_value, UiPropertyValue::Milli(Milli(0)));
    assert_eq!(sample.target_value, UiPropertyValue::Milli(Milli(1_000)));
    assert_eq!(sample.sampled_value, UiPropertyValue::Milli(Milli(500)));
}
