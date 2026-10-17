//! Image value contracts shared by executable commands, bundles and presentation.
//!
//! These values contain no retained View, renderer, device or runtime state.
//! Defaults are authoring conveniences; deserialization requires the encoded
//! fields and never substitutes a default for an invalid value.

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

/// How intrinsic image pixels map into authored presentation bounds.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageObjectFit {
    #[default]
    Contain,
    Cover,
    Stretch,
    Intrinsic,
}

/// Alignment inside image bounds, with each axis in `0..=1000` milli-units.
///
/// The checked axes cannot be overwritten through the public API:
///
/// ```compile_fail
/// use arcweft_interaction_model::presentation::ImageObjectAlignment;
/// let mut alignment = ImageObjectAlignment::center();
/// alignment.x_milli = 1_001;
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ImageObjectAlignment {
    x_milli: i32,
    y_milli: i32,
}

/// The axis of an invalid image alignment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageAlignmentAxis {
    X,
    Y,
}

/// An alignment outside its closed milli-unit interval.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("image alignment {axis:?} must be in 0..=1000 milli-units, received {value}")]
pub struct ImageAlignmentError {
    pub axis: ImageAlignmentAxis,
    pub value: i32,
}

impl ImageObjectAlignment {
    /// Validates both axes without clamping or applying authoring defaults.
    pub const fn try_new(x_milli: i32, y_milli: i32) -> Result<Self, ImageAlignmentError> {
        if x_milli < 0 || x_milli > 1_000 {
            return Err(ImageAlignmentError {
                axis: ImageAlignmentAxis::X,
                value: x_milli,
            });
        }
        if y_milli < 0 || y_milli > 1_000 {
            return Err(ImageAlignmentError {
                axis: ImageAlignmentAxis::Y,
                value: y_milli,
            });
        }
        Ok(Self { x_milli, y_milli })
    }

    pub const fn center() -> Self {
        Self {
            x_milli: 500,
            y_milli: 500,
        }
    }

    pub const fn top_left() -> Self {
        Self {
            x_milli: 0,
            y_milli: 0,
        }
    }

    pub const fn x_milli(self) -> i32 {
        self.x_milli
    }

    pub const fn y_milli(self) -> i32 {
        self.y_milli
    }
}

impl Default for ImageObjectAlignment {
    fn default() -> Self {
        Self::center()
    }
}

impl<'de> Deserialize<'de> for ImageObjectAlignment {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct EncodedAlignment {
            x_milli: i32,
            y_milli: i32,
        }

        let encoded = EncodedAlignment::deserialize(deserializer)?;
        Self::try_new(encoded.x_milli, encoded.y_milli).map_err(serde::de::Error::custom)
    }
}

/// Deterministic image playback, independent of clocks and decoded frame caches.
///
/// Durations are unsigned milliseconds and the non-negative rate is in
/// milli-units. A pinned local time takes precedence over pause and visual time.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImageObjectPlayback {
    start_time_millis: u64,
    rate_milli: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    paused_at_millis: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pinned_local_time_millis: Option<u64>,
}

impl ImageObjectPlayback {
    pub const fn new(start_time_millis: u64) -> Self {
        Self::from_parts(start_time_millis, 1_000, None, None)
    }

    /// Constructs a completely specified playback policy; all integer values fit
    /// their units by construction, including a zero (stopped) playback rate.
    pub const fn from_parts(
        start_time_millis: u64,
        rate_milli: u32,
        paused_at_millis: Option<u64>,
        pinned_local_time_millis: Option<u64>,
    ) -> Self {
        Self {
            start_time_millis,
            rate_milli,
            paused_at_millis,
            pinned_local_time_millis,
        }
    }

    #[must_use]
    pub const fn paused_at(mut self, visual_time_millis: u64) -> Self {
        self.paused_at_millis = Some(visual_time_millis);
        self
    }

    #[must_use]
    pub const fn with_rate_milli(mut self, rate_milli: u32) -> Self {
        self.rate_milli = rate_milli;
        self
    }

    #[must_use]
    pub const fn pinned_local_time(mut self, local_time_millis: u64) -> Self {
        self.pinned_local_time_millis = Some(local_time_millis);
        self
    }

    pub const fn start_time_millis(self) -> u64 {
        self.start_time_millis
    }

    pub const fn rate_milli(self) -> u32 {
        self.rate_milli
    }

    pub const fn paused_at_millis(self) -> Option<u64> {
        self.paused_at_millis
    }

    pub const fn pinned_local_time_millis(self) -> Option<u64> {
        self.pinned_local_time_millis
    }

    pub fn local_time_millis(self, visual_time_millis: u64) -> u64 {
        if let Some(pinned) = self.pinned_local_time_millis {
            return pinned;
        }
        let sample_time = self.paused_at_millis.unwrap_or(visual_time_millis);
        let elapsed = sample_time.saturating_sub(self.start_time_millis);
        elapsed.saturating_mul(u64::from(self.rate_milli)) / 1_000
    }
}

impl Default for ImageObjectPlayback {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Fixed-point transform in layer-local coordinates. Every component is an
/// encoded `i32` milli-unit value; no floating-point intermediate is stored.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImageObjectTransform {
    pub m11_milli: i32,
    pub m12_milli: i32,
    pub m21_milli: i32,
    pub m22_milli: i32,
    pub tx_milli: i32,
    pub ty_milli: i32,
}

impl ImageObjectTransform {
    pub const fn identity() -> Self {
        Self {
            m11_milli: 1_000,
            m12_milli: 0,
            m21_milli: 0,
            m22_milli: 1_000,
            tx_milli: 0,
            ty_milli: 0,
        }
    }

    pub const fn translation_milli(
        horizontal_offset_milli: i32,
        vertical_offset_milli: i32,
    ) -> Self {
        Self {
            tx_milli: horizontal_offset_milli,
            ty_milli: vertical_offset_milli,
            ..Self::identity()
        }
    }
}

impl Default for ImageObjectTransform {
    fn default() -> Self {
        Self::identity()
    }
}

#[cfg(test)]
mod tests;
