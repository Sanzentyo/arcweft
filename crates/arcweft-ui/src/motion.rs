//! Deterministic UI motion model for retained Arcweft UI styles.
//!
//! This module is deliberately Sans I/O. It never reads wall-clock time itself;
//! native and web players pass sampled timeline milliseconds into transitions or
//! keyframe tracks. CSS parsing remains outside this crate, while interpolation
//! behavior lives on Arcweft-owned style boundary types.

use crate::style::{Milli, UiPropertyKind, UiPropertyValue};
use thiserror::Error;

/// Monotonic player timeline timestamp in milliseconds.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct UiTimelineMillis(u64);

/// Reduced-motion behavior selected by the host/player accessibility policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UiReducedMotionPolicy {
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
pub enum UiEasingFunction {
    Linear,
    Ease,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier(UiCubicBezier),
    Steps {
        steps: u16,
        position: UiStepPosition,
    },
}

/// Cubic bezier control points in CSS timing-function coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiCubicBezier {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

/// CSS step timing position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiStepPosition {
    JumpStart,
    JumpEnd,
}

/// Transition timing and property selection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiTransitionSpec {
    pub property: UiPropertyKind,
    pub duration_ms: u32,
    pub delay_ms: i32,
    pub easing: UiEasingFunction,
}

/// One running transition from a sampled source value to a target value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiTransition {
    spec: UiTransitionSpec,
    started_at: UiTimelineMillis,
    source_value: UiPropertyValue,
    target_value: UiPropertyValue,
}

/// One sampled transition/keyframe evidence packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiMotionSample {
    pub property: UiPropertyKind,
    pub timestamp: UiTimelineMillis,
    pub source_value: UiPropertyValue,
    pub target_value: UiPropertyValue,
    pub sampled_value: UiPropertyValue,
    pub linear_progress: Milli,
    pub eased_progress: Milli,
    pub finished: bool,
}

/// One keyframe value in a per-property animation track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiKeyframe {
    pub offset: Milli,
    pub value: UiPropertyValue,
    pub easing_after: UiEasingFunction,
}

/// A normalized per-property keyframe track.
#[derive(Clone, Debug, PartialEq)]
pub struct UiKeyframeTrack {
    property: UiPropertyKind,
    duration_ms: u32,
    keyframes: Vec<UiKeyframe>,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum UiMotionError {
    #[error("UI property {0:?} is not transitionable in the seq06.13 motion model")]
    NonTransitionableProperty(UiPropertyKind),
    #[error(
        "UI property {property:?} cannot interpolate from {source_value:?} to {target_value:?}"
    )]
    IncompatibleValues {
        property: UiPropertyKind,
        source_value: UiPropertyValue,
        target_value: UiPropertyValue,
    },
    #[error("keyframe track for {0:?} must contain at least two ordered keyframes")]
    InvalidKeyframes(UiPropertyKind),
}

impl UiTimelineMillis {
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

impl UiReducedMotionPolicy {
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

impl UiCubicBezier {
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }
}

impl UiEasingFunction {
    pub const CSS_EASE: Self = Self::CubicBezier(UiCubicBezier::new(0.25, 0.1, 0.25, 1.0));
    pub const CSS_EASE_IN: Self = Self::CubicBezier(UiCubicBezier::new(0.42, 0.0, 1.0, 1.0));
    pub const CSS_EASE_OUT: Self = Self::CubicBezier(UiCubicBezier::new(0.0, 0.0, 0.58, 1.0));
    pub const CSS_EASE_IN_OUT: Self = Self::CubicBezier(UiCubicBezier::new(0.42, 0.0, 0.58, 1.0));

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

impl UiTransitionSpec {
    pub const fn new(property: UiPropertyKind, duration_ms: u32, easing: UiEasingFunction) -> Self {
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

impl UiTransition {
    pub fn new(
        spec: UiTransitionSpec,
        started_at: UiTimelineMillis,
        source_value: UiPropertyValue,
        target_value: UiPropertyValue,
    ) -> Result<Self, UiMotionError> {
        ensure_interpolable(spec.property, source_value, target_value)?;
        Ok(Self {
            spec,
            started_at,
            source_value,
            target_value,
        })
    }

    pub const fn spec(&self) -> UiTransitionSpec {
        self.spec
    }

    pub const fn source_value(&self) -> UiPropertyValue {
        self.source_value
    }

    pub const fn target_value(&self) -> UiPropertyValue {
        self.target_value
    }

    pub fn sample(
        &self,
        timestamp: UiTimelineMillis,
        policy: UiReducedMotionPolicy,
    ) -> Result<UiMotionSample, UiMotionError> {
        let linear_progress = self.linear_progress(timestamp, policy);
        let eased_progress = self.spec.easing.sample(linear_progress);
        let sampled_value = self
            .spec
            .property
            .interpolate_value(self.source_value, self.target_value, eased_progress)
            .ok_or(UiMotionError::IncompatibleValues {
                property: self.spec.property,
                source_value: self.source_value,
                target_value: self.target_value,
            })?;
        Ok(UiMotionSample {
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
        timestamp: UiTimelineMillis,
        target_value: UiPropertyValue,
        spec: UiTransitionSpec,
        policy: UiReducedMotionPolicy,
    ) -> Result<Self, UiMotionError> {
        let sampled = self.sample(timestamp, policy)?;
        Self::new(spec, timestamp, sampled.sampled_value, target_value)
    }

    fn linear_progress(&self, timestamp: UiTimelineMillis, policy: UiReducedMotionPolicy) -> Milli {
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

impl UiKeyframe {
    pub const fn new(offset: Milli, value: UiPropertyValue) -> Self {
        Self {
            offset,
            value,
            easing_after: UiEasingFunction::Linear,
        }
    }

    #[must_use]
    pub const fn with_easing_after(mut self, easing_after: UiEasingFunction) -> Self {
        self.easing_after = easing_after;
        self
    }
}

impl UiKeyframeTrack {
    pub fn new(
        property: UiPropertyKind,
        duration_ms: u32,
        keyframes: impl IntoIterator<Item = UiKeyframe>,
    ) -> Result<Self, UiMotionError> {
        if !property.is_transitionable() {
            return Err(UiMotionError::NonTransitionableProperty(property));
        }
        let mut keyframes = keyframes.into_iter().collect::<Vec<_>>();
        keyframes.sort_by_key(|keyframe| clamp_progress(keyframe.offset).value());
        if keyframes.len() < 2 {
            return Err(UiMotionError::InvalidKeyframes(property));
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

    pub const fn property(&self) -> UiPropertyKind {
        self.property
    }

    pub const fn duration_ms(&self) -> u32 {
        self.duration_ms
    }

    pub fn keyframes(&self) -> &[UiKeyframe] {
        &self.keyframes
    }

    pub fn sample(
        &self,
        started_at: UiTimelineMillis,
        timestamp: UiTimelineMillis,
        policy: UiReducedMotionPolicy,
    ) -> Result<UiMotionSample, UiMotionError> {
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
            .ok_or(UiMotionError::InvalidKeyframes(self.property))?;
        self.sample_between(timestamp, last, last, Milli::ONE, Milli::ONE, true)
    }

    fn sample_between(
        &self,
        timestamp: UiTimelineMillis,
        start: UiKeyframe,
        end: UiKeyframe,
        linear_progress: Milli,
        eased_progress: Milli,
        finished: bool,
    ) -> Result<UiMotionSample, UiMotionError> {
        ensure_interpolable(self.property, start.value, end.value)?;
        let sampled_value = self
            .property
            .interpolate_value(start.value, end.value, eased_progress)
            .ok_or(UiMotionError::IncompatibleValues {
                property: self.property,
                source_value: start.value,
                target_value: end.value,
            })?;
        Ok(UiMotionSample {
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

impl UiCubicBezier {
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
    property: UiPropertyKind,
    source: UiPropertyValue,
    target: UiPropertyValue,
) -> Result<(), UiMotionError> {
    if !property.is_transitionable() {
        return Err(UiMotionError::NonTransitionableProperty(property));
    }
    if property
        .interpolate_value(source, target, Milli::ZERO)
        .is_none()
    {
        return Err(UiMotionError::IncompatibleValues {
            property,
            source_value: source,
            target_value: target,
        });
    }
    Ok(())
}

fn transition_progress(
    started_at: UiTimelineMillis,
    timestamp: UiTimelineMillis,
    duration_ms: u32,
    policy: UiReducedMotionPolicy,
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

fn sample_steps(progress: Milli, steps: u16, position: UiStepPosition) -> Milli {
    let steps = i32::from(steps.max(1));
    let progress = clamp_progress(progress).value();
    let raw_step = match position {
        UiStepPosition::JumpStart => (progress.saturating_mul(steps) + 999) / 1_000,
        UiStepPosition::JumpEnd => progress.saturating_mul(steps) / 1_000,
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
