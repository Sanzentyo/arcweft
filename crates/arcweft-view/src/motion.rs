//! Deterministic View motion model for retained Arcweft View styles.
//!
//! This module is deliberately Sans I/O. It never reads wall-clock time itself;
//! native and web players pass sampled timeline milliseconds into transitions or
//! keyframe tracks. CSS parsing remains outside this crate, while interpolation
//! behavior lives on Arcweft-owned style boundary types.

use crate::style::{Milli, ViewPropertyKind, ViewPropertyValue};
use thiserror::Error;

/// Monotonic player timeline timestamp in milliseconds.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct ViewTimelineMillis(u64);

/// Reduced-motion behavior selected by the host/player accessibility policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewReducedMotionPolicy {
    /// Preserve author durations and easing exactly.
    #[default]
    Full,
    /// Clamp every motion duration to `max_duration_ms` while preserving easing.
    Shorten { max_duration_ms: u32 },
    /// Jump to the final value at the first sampled frame.
    Disable,
}

/// Easing functions supported by the first Arcweft CSS-motion cut.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewEasingFunction {
    Linear,
    Ease,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier(ViewCubicBezier),
    Steps {
        steps: u16,
        position: ViewStepPosition,
    },
}

/// Cubic bezier control points in CSS timing-function coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewCubicBezier {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

/// CSS step timing position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewStepPosition {
    JumpStart,
    JumpEnd,
}

/// Transition timing and property selection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTransitionSpec {
    pub property: ViewPropertyKind,
    pub duration_ms: u32,
    pub delay_ms: i32,
    pub easing: ViewEasingFunction,
}

/// One running transition from a sampled source value to a target value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTransition {
    spec: ViewTransitionSpec,
    started_at: ViewTimelineMillis,
    source_value: ViewPropertyValue,
    target_value: ViewPropertyValue,
}

/// One sampled transition/keyframe evidence packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewMotionSample {
    pub property: ViewPropertyKind,
    pub timestamp: ViewTimelineMillis,
    pub source_value: ViewPropertyValue,
    pub target_value: ViewPropertyValue,
    pub sampled_value: ViewPropertyValue,
    pub linear_progress: Milli,
    pub eased_progress: Milli,
    pub finished: bool,
}

/// One keyframe value in a per-property animation track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewKeyframe {
    pub offset: Milli,
    pub value: ViewPropertyValue,
    pub easing_after: ViewEasingFunction,
}

/// A normalized per-property keyframe track.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewKeyframeTrack {
    property: ViewPropertyKind,
    duration_ms: u32,
    keyframes: Vec<ViewKeyframe>,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ViewMotionError {
    #[error("View property {0:?} is not transitionable in the seq06.13 motion model")]
    NonTransitionableProperty(ViewPropertyKind),
    #[error(
        "View property {property:?} cannot interpolate from {source_value:?} to {target_value:?}"
    )]
    IncompatibleValues {
        property: ViewPropertyKind,
        source_value: ViewPropertyValue,
        target_value: ViewPropertyValue,
    },
    #[error("keyframe track for {0:?} must contain at least two ordered keyframes")]
    InvalidKeyframes(ViewPropertyKind),
}

impl ViewTimelineMillis {
    pub const ZERO: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

    pub const fn saturating_elapsed_since(self, earlier: Self) -> u64 {
        self.0.saturating_sub(earlier.0)
    }
}

impl ViewReducedMotionPolicy {
    pub const fn duration_ms(self, author_duration_ms: u32) -> u32 {
        match self {
            Self::Full => author_duration_ms,
            Self::Shorten { max_duration_ms } => {
                if author_duration_ms < max_duration_ms {
                    author_duration_ms
                } else {
                    max_duration_ms
                }
            }
            Self::Disable => 0,
        }
    }
}

impl ViewCubicBezier {
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }
}

impl ViewEasingFunction {
    pub const CSS_EASE: Self = Self::CubicBezier(ViewCubicBezier::new(0.25, 0.1, 0.25, 1.0));
    pub const CSS_EASE_IN: Self = Self::CubicBezier(ViewCubicBezier::new(0.42, 0.0, 1.0, 1.0));
    pub const CSS_EASE_OUT: Self = Self::CubicBezier(ViewCubicBezier::new(0.0, 0.0, 0.58, 1.0));
    pub const CSS_EASE_IN_OUT: Self = Self::CubicBezier(ViewCubicBezier::new(0.42, 0.0, 0.58, 1.0));

    pub fn sample(self, linear_progress: Milli) -> Milli {
        let linear_progress = clamp_progress(linear_progress);
        match self {
            Self::Linear => linear_progress,
            Self::Ease => Self::CSS_EASE.sample(linear_progress),
            Self::EaseIn => Self::CSS_EASE_IN.sample(linear_progress),
            Self::EaseOut => Self::CSS_EASE_OUT.sample(linear_progress),
            Self::EaseInOut => Self::CSS_EASE_IN_OUT.sample(linear_progress),
            Self::CubicBezier(bezier) => bezier.sample(linear_progress),
            Self::Steps { steps, position } => sample_steps(linear_progress, steps, position),
        }
    }
}

impl ViewTransitionSpec {
    pub const fn new(
        property: ViewPropertyKind,
        duration_ms: u32,
        easing: ViewEasingFunction,
    ) -> Self {
        Self {
            property,
            duration_ms,
            delay_ms: 0,
            easing,
        }
    }

    #[must_use]
    pub const fn with_delay_ms(mut self, delay_ms: i32) -> Self {
        self.delay_ms = delay_ms;
        self
    }
}

impl ViewTransition {
    pub fn new(
        spec: ViewTransitionSpec,
        started_at: ViewTimelineMillis,
        source_value: ViewPropertyValue,
        target_value: ViewPropertyValue,
    ) -> Result<Self, ViewMotionError> {
        ensure_interpolable(spec.property, source_value, target_value)?;
        Ok(Self {
            spec,
            started_at,
            source_value,
            target_value,
        })
    }

    pub const fn spec(&self) -> ViewTransitionSpec {
        self.spec
    }

    pub const fn source_value(&self) -> ViewPropertyValue {
        self.source_value
    }

    pub const fn target_value(&self) -> ViewPropertyValue {
        self.target_value
    }

    pub fn sample(
        &self,
        timestamp: ViewTimelineMillis,
        policy: ViewReducedMotionPolicy,
    ) -> Result<ViewMotionSample, ViewMotionError> {
        let linear_progress = self.linear_progress(timestamp, policy);
        let eased_progress = self.spec.easing.sample(linear_progress);
        let sampled_value = self
            .spec
            .property
            .interpolate_value(self.source_value, self.target_value, eased_progress)
            .ok_or(ViewMotionError::IncompatibleValues {
                property: self.spec.property,
                source_value: self.source_value,
                target_value: self.target_value,
            })?;
        Ok(ViewMotionSample {
            property: self.spec.property,
            timestamp,
            source_value: self.source_value,
            target_value: self.target_value,
            sampled_value,
            linear_progress,
            eased_progress,
            finished: linear_progress.value() >= Milli::ONE.value(),
        })
    }

    /// Start a new transition at `timestamp` from the current sampled value.
    pub fn interrupt(
        &self,
        timestamp: ViewTimelineMillis,
        target_value: ViewPropertyValue,
        spec: ViewTransitionSpec,
        policy: ViewReducedMotionPolicy,
    ) -> Result<Self, ViewMotionError> {
        let sampled = self.sample(timestamp, policy)?;
        Self::new(spec, timestamp, sampled.sampled_value, target_value)
    }

    fn linear_progress(
        &self,
        timestamp: ViewTimelineMillis,
        policy: ViewReducedMotionPolicy,
    ) -> Milli {
        let duration_ms = u64::from(policy.duration_ms(self.spec.duration_ms));
        if duration_ms == 0 {
            return Milli::ONE;
        }

        let elapsed = timestamp.saturating_elapsed_since(self.started_at);
        let adjusted_elapsed = if self.spec.delay_ms >= 0 {
            elapsed.saturating_sub(u64::from(self.spec.delay_ms.unsigned_abs()))
        } else {
            elapsed.saturating_add(u64::from(self.spec.delay_ms.unsigned_abs()))
        };
        let progress = adjusted_elapsed
            .min(duration_ms)
            .saturating_mul(u64::try_from(Milli::ONE.value()).unwrap_or(1_000))
            / duration_ms;
        Milli(i32::try_from(progress).unwrap_or(Milli::ONE.value()))
    }
}

impl ViewKeyframe {
    pub const fn new(offset: Milli, value: ViewPropertyValue) -> Self {
        Self {
            offset,
            value,
            easing_after: ViewEasingFunction::Linear,
        }
    }

    #[must_use]
    pub const fn with_easing_after(mut self, easing_after: ViewEasingFunction) -> Self {
        self.easing_after = easing_after;
        self
    }
}

impl ViewKeyframeTrack {
    pub fn new(
        property: ViewPropertyKind,
        duration_ms: u32,
        keyframes: impl IntoIterator<Item = ViewKeyframe>,
    ) -> Result<Self, ViewMotionError> {
        if !property.is_transitionable() {
            return Err(ViewMotionError::NonTransitionableProperty(property));
        }
        let mut keyframes = keyframes.into_iter().collect::<Vec<_>>();
        keyframes.sort_by_key(|keyframe| clamp_progress(keyframe.offset).value());
        if keyframes.len() < 2 {
            return Err(ViewMotionError::InvalidKeyframes(property));
        }
        for keyframe in &mut keyframes {
            keyframe.offset = clamp_progress(keyframe.offset);
        }
        Ok(Self {
            property,
            duration_ms,
            keyframes,
        })
    }

    pub const fn property(&self) -> ViewPropertyKind {
        self.property
    }

    pub const fn duration_ms(&self) -> u32 {
        self.duration_ms
    }

    pub fn keyframes(&self) -> &[ViewKeyframe] {
        &self.keyframes
    }

    pub fn sample(
        &self,
        started_at: ViewTimelineMillis,
        timestamp: ViewTimelineMillis,
        policy: ViewReducedMotionPolicy,
    ) -> Result<ViewMotionSample, ViewMotionError> {
        let progress = transition_progress(started_at, timestamp, self.duration_ms, policy);
        let first = self.keyframes[0];
        if progress.value() <= first.offset.value() {
            return self.sample_between(timestamp, first, first, Milli::ZERO, Milli::ZERO, false);
        }
        for pair in self.keyframes.windows(2) {
            let start = pair[0];
            let end = pair[1];
            if progress.value() <= end.offset.value() {
                let segment_progress = segment_progress(progress, start.offset, end.offset);
                let eased = start.easing_after.sample(segment_progress);
                return self.sample_between(
                    timestamp,
                    start,
                    end,
                    segment_progress,
                    eased,
                    progress == Milli::ONE,
                );
            }
        }
        let last = *self
            .keyframes
            .last()
            .ok_or(ViewMotionError::InvalidKeyframes(self.property))?;
        self.sample_between(timestamp, last, last, Milli::ONE, Milli::ONE, true)
    }

    fn sample_between(
        &self,
        timestamp: ViewTimelineMillis,
        start: ViewKeyframe,
        end: ViewKeyframe,
        linear_progress: Milli,
        eased_progress: Milli,
        finished: bool,
    ) -> Result<ViewMotionSample, ViewMotionError> {
        ensure_interpolable(self.property, start.value, end.value)?;
        let sampled_value = self
            .property
            .interpolate_value(start.value, end.value, eased_progress)
            .ok_or(ViewMotionError::IncompatibleValues {
                property: self.property,
                source_value: start.value,
                target_value: end.value,
            })?;
        Ok(ViewMotionSample {
            property: self.property,
            timestamp,
            source_value: start.value,
            target_value: end.value,
            sampled_value,
            linear_progress,
            eased_progress,
            finished,
        })
    }
}

impl ViewCubicBezier {
    fn sample(self, linear_progress: Milli) -> Milli {
        let target_x = progress_to_unit(linear_progress);
        let mut low = 0.0;
        let mut high = 1.0;
        for _ in 0..14 {
            let mid = (low + high) * 0.5;
            if cubic(mid, self.x1, self.x2) < target_x {
                low = mid;
            } else {
                high = mid;
            }
        }
        unit_to_progress(cubic((low + high) * 0.5, self.y1, self.y2))
    }
}

fn ensure_interpolable(
    property: ViewPropertyKind,
    source: ViewPropertyValue,
    target: ViewPropertyValue,
) -> Result<(), ViewMotionError> {
    if !property.is_transitionable() {
        return Err(ViewMotionError::NonTransitionableProperty(property));
    }
    if property
        .interpolate_value(source, target, Milli::ZERO)
        .is_none()
    {
        return Err(ViewMotionError::IncompatibleValues {
            property,
            source_value: source,
            target_value: target,
        });
    }
    Ok(())
}

fn transition_progress(
    started_at: ViewTimelineMillis,
    timestamp: ViewTimelineMillis,
    duration_ms: u32,
    policy: ViewReducedMotionPolicy,
) -> Milli {
    let duration_ms = u64::from(policy.duration_ms(duration_ms));
    if duration_ms == 0 {
        return Milli::ONE;
    }
    let progress = timestamp
        .saturating_elapsed_since(started_at)
        .min(duration_ms)
        .saturating_mul(u64::try_from(Milli::ONE.value()).unwrap_or(1_000))
        / duration_ms;
    Milli(i32::try_from(progress).unwrap_or(Milli::ONE.value()))
}

fn segment_progress(progress: Milli, start: Milli, end: Milli) -> Milli {
    let start = i64::from(clamp_progress(start).value());
    let end = i64::from(clamp_progress(end).value());
    if end <= start {
        return Milli::ONE;
    }
    let progress = i64::from(clamp_progress(progress).value()).clamp(start, end);
    let numerator = (progress - start).saturating_mul(i64::from(Milli::ONE.value()));
    let denominator = end - start;
    Milli(i32::try_from((numerator + denominator / 2) / denominator).unwrap_or(Milli::ONE.value()))
}

fn sample_steps(progress: Milli, steps: u16, position: ViewStepPosition) -> Milli {
    let steps = i32::from(steps.max(1));
    let progress = clamp_progress(progress).value();
    let raw_step = match position {
        ViewStepPosition::JumpStart => (progress.saturating_mul(steps) + 999) / 1_000,
        ViewStepPosition::JumpEnd => progress.saturating_mul(steps) / 1_000,
    };
    Milli(raw_step.clamp(0, steps).saturating_mul(1_000) / steps)
}

fn clamp_progress(progress: Milli) -> Milli {
    Milli(
        progress
            .value()
            .clamp(Milli::ZERO.value(), Milli::ONE.value()),
    )
}

fn progress_to_unit(progress: Milli) -> f32 {
    let progress = clamp_progress(progress).value();
    f32::from(i16::try_from(progress).unwrap_or(0)) / 1_000.0
}

fn unit_to_progress(value: f32) -> Milli {
    let clamped = if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut low = 0_i32;
    let mut high = Milli::ONE.value();
    while low < high {
        let mid = (low + high + 1) / 2;
        let mid_unit = f32::from(i16::try_from(mid).unwrap_or(0)) / 1_000.0;
        if mid_unit <= clamped {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    Milli(low)
}

fn cubic(t: f32, p1: f32, p2: f32) -> f32 {
    let inv = 1.0 - t;
    3.0 * inv * inv * t * p1 + 3.0 * inv * t * t * p2 + t * t * t
}
