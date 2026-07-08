use arcweft_view::{
    Milli, Rgba8, ViewEasingFunction, ViewKeyframe, ViewKeyframeTrack, ViewPropertyKind,
    ViewPropertyValue, ViewReducedMotionPolicy, ViewTimelineMillis, ViewTransition,
    ViewTransitionSpec,
};

fn transition(
    property: ViewPropertyKind,
    from: ViewPropertyValue,
    to: ViewPropertyValue,
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
        ViewPropertyValue::Color(Rgba8::new(0, 0, 0, 255)),
        ViewPropertyValue::Color(Rgba8::new(255, 128, 64, 127)),
    )
    .sample(ViewTimelineMillis::new(500), ViewReducedMotionPolicy::Full)
    .expect("sample succeeds");

    assert_eq!(sample.linear_progress, Milli(500));
    assert_eq!(
        sample.sampled_value,
        ViewPropertyValue::Color(Rgba8::new(128, 64, 32, 192))
    );
}

#[test]
fn opacity_scale_and_outline_width_are_transitionable_paint_values() {
    let opacity = transition(
        ViewPropertyKind::Opacity,
        ViewPropertyValue::Milli(Milli(1_000)),
        ViewPropertyValue::Milli(Milli(0)),
    )
    .sample(ViewTimelineMillis::new(250), ViewReducedMotionPolicy::Full)
    .expect("opacity samples");
    assert_eq!(opacity.sampled_value, ViewPropertyValue::Milli(Milli(751)));

    let scale = transition(
        ViewPropertyKind::Scale,
        ViewPropertyValue::Milli(Milli(1_000)),
        ViewPropertyValue::Milli(Milli(2_000)),
    )
    .sample(ViewTimelineMillis::new(500), ViewReducedMotionPolicy::Full)
    .expect("scale samples");
    assert_eq!(scale.sampled_value, ViewPropertyValue::Milli(Milli(1_500)));

    let outline = transition(
        ViewPropertyKind::OutlineWidth,
        ViewPropertyValue::Milli(Milli(0)),
        ViewPropertyValue::Milli(Milli(4_000)),
    )
    .sample(ViewTimelineMillis::new(250), ViewReducedMotionPolicy::Full)
    .expect("outline samples");
    assert_eq!(
        outline.sampled_value,
        ViewPropertyValue::Milli(Milli(1_000))
    );
}

#[test]
fn reduced_motion_policy_can_shorten_or_disable_motion() {
    let transition = transition(
        ViewPropertyKind::TranslateX,
        ViewPropertyValue::Milli(Milli(0)),
        ViewPropertyValue::Milli(Milli(1_000)),
    );

    let shortened = transition
        .sample(
            ViewTimelineMillis::new(50),
            ViewReducedMotionPolicy::Shorten {
                max_duration_ms: 100,
            },
        )
        .expect("shortened sample succeeds");
    assert_eq!(shortened.linear_progress, Milli(500));
    assert_eq!(
        shortened.sampled_value,
        ViewPropertyValue::Milli(Milli(500))
    );

    let disabled = transition
        .sample(ViewTimelineMillis::new(1), ViewReducedMotionPolicy::Disable)
        .expect("disabled sample succeeds");
    assert_eq!(disabled.linear_progress, Milli::ONE);
    assert_eq!(
        disabled.sampled_value,
        ViewPropertyValue::Milli(Milli(1_000))
    );
    assert!(disabled.finished);
}

#[test]
fn interruption_starts_next_transition_from_sampled_value() {
    let first = transition(
        ViewPropertyKind::Opacity,
        ViewPropertyValue::Milli(Milli(0)),
        ViewPropertyValue::Milli(Milli(1_000)),
    );
    let reversed = first
        .interrupt(
            ViewTimelineMillis::new(500),
            ViewPropertyValue::Milli(Milli(0)),
            ViewTransitionSpec::new(ViewPropertyKind::Opacity, 1_000, ViewEasingFunction::Linear),
            ViewReducedMotionPolicy::Full,
        )
        .expect("interruption succeeds");

    assert_eq!(
        reversed.source_value(),
        ViewPropertyValue::Milli(Milli(500))
    );
    let sample = reversed
        .sample(ViewTimelineMillis::new(750), ViewReducedMotionPolicy::Full)
        .expect("reversed sample succeeds");
    assert_eq!(sample.sampled_value, ViewPropertyValue::Milli(Milli(376)));
}

#[test]
fn keyframe_track_samples_ordered_offsets() {
    let track = ViewKeyframeTrack::new(
        ViewPropertyKind::Opacity,
        1_000,
        [
            ViewKeyframe::new(Milli(0), ViewPropertyValue::Milli(Milli(0))),
            ViewKeyframe::new(Milli(500), ViewPropertyValue::Milli(Milli(1_000))),
            ViewKeyframe::new(Milli(1_000), ViewPropertyValue::Milli(Milli(0))),
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

    assert_eq!(sample.source_value, ViewPropertyValue::Milli(Milli(0)));
    assert_eq!(sample.target_value, ViewPropertyValue::Milli(Milli(1_000)));
    assert_eq!(sample.sampled_value, ViewPropertyValue::Milli(Milli(500)));
}
