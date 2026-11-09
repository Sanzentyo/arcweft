//! Absolute typography lengths resolved in the shared logical-pixel space.

use arcweft_text_model::{CheckedLength, LengthUnit, Milli};
use serde::{Deserialize, Serialize};

/// Uniform conversion from resolved typography to output logical pixels.
/// Device-pixel density is applied separately by the raster adapter.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct TextMetricScale(f32);

/// A typography scale or its projected metric is outside the layout domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TextMetricScaleError {
    #[error("text metric scale must be finite and positive")]
    InvalidScale,
    #[error("scaled text metric {field} exceeds its logical milli-pixel domain")]
    MetricOutOfRange { field: &'static str },
}

impl TextMetricScale {
    pub const IDENTITY: Self = Self(1.0);

    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.0.to_bits() == 1.0_f32.to_bits()
    }

    /// Projects a nonnegative resolved length to output logical pixels.
    pub fn length_pixels(
        self,
        field: &'static str,
        length: ResolvedTextLength,
    ) -> Result<f32, TextMetricScaleError> {
        if self.is_identity() {
            return Ok(length.pixels());
        }
        let milli = self.unsigned_milli(field, length.milli_pixels())?;
        #[allow(clippy::cast_precision_loss, reason = "layout uses logical f32 pixels")]
        Ok(milli as f32 / 1_000.0)
    }

    pub(super) fn unsigned_milli(
        self,
        field: &'static str,
        value: u32,
    ) -> Result<u32, TextMetricScaleError> {
        let scaled = (f64::from(value) * f64::from(self.0)).round();
        if scaled > f64::from(u32::MAX) {
            return Err(TextMetricScaleError::MetricOutOfRange { field });
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Ok(scaled as u32)
    }

    pub(super) fn positive_milli(
        self,
        field: &'static str,
        value: u32,
    ) -> Result<u32, TextMetricScaleError> {
        let scaled = self.unsigned_milli(field, value)?;
        if scaled == 0 {
            return Err(TextMetricScaleError::MetricOutOfRange { field });
        }
        Ok(scaled)
    }

    pub(super) fn signed_milli(
        self,
        field: &'static str,
        value: i32,
    ) -> Result<i32, TextMetricScaleError> {
        let scaled = (f64::from(value) * f64::from(self.0)).round();
        if !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&scaled) {
            return Err(TextMetricScaleError::MetricOutOfRange { field });
        }
        #[allow(clippy::cast_possible_truncation)]
        Ok(scaled as i32)
    }
}

impl Default for TextMetricScale {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl TryFrom<f32> for TextMetricScale {
    type Error = TextMetricScaleError;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        if !value.is_finite() || value <= 0.0 {
            return Err(TextMetricScaleError::InvalidScale);
        }
        Ok(Self(value))
    }
}

impl From<TextMetricScale> for f32 {
    fn from(scale: TextMetricScale) -> Self {
        scale.0
    }
}

/// A nonnegative length admitted for text geometry in logical milli-pixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedTextLength(Milli);

/// A quantity cannot enter the absolute text-layout coordinate space.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TextLengthResolutionError {
    #[error("text layout requires a nonnegative length, received {length:?}")]
    Negative { length: CheckedLength },
    #[error("text layout has no admitted context for length unit {unit:?}")]
    UnresolvedUnit { unit: LengthUnit },
    #[error("text length {length:?} exceeds the logical milli-pixel domain")]
    Overflow { length: CheckedLength },
}

impl TryFrom<CheckedLength> for ResolvedTextLength {
    type Error = TextLengthResolutionError;

    /// Resolves absolute quantities only at the text geometry boundary.
    /// Logical points follow the CSS ratio 1pt = 4/3px, independently of device
    /// scaling. Round once to the nearest milli-pixel; thirds never tie.
    fn try_from(length: CheckedLength) -> Result<Self, Self::Error> {
        let (numerator, denominator) = match length.unit {
            LengthUnit::Px => (1_i64, 1_i64),
            LengthUnit::Pt => (4, 3),
            unit => return Err(TextLengthResolutionError::UnresolvedUnit { unit }),
        };
        if length.milli < 0 {
            return Err(TextLengthResolutionError::Negative { length });
        }
        let scaled = i64::from(length.milli) * numerator;
        let rounded = (scaled + denominator / 2) / denominator;
        let milli =
            i32::try_from(rounded).map_err(|_| TextLengthResolutionError::Overflow { length })?;
        Ok(Self(Milli(milli)))
    }
}

impl ResolvedTextLength {
    #[must_use]
    pub const fn milli_pixels(self) -> u32 {
        self.0.0.unsigned_abs()
    }

    #[must_use]
    pub fn pixels(self) -> f32 {
        self.0.as_f32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scaled_gap_can_round_to_zero_but_a_font_size_cannot() {
        let scale = TextMetricScale::try_from(0.25).unwrap();
        let gap = ResolvedTextLength::try_from(CheckedLength::px(1)).unwrap();
        assert_eq!(scale.length_pixels("ruby_gap", gap), Ok(0.0));
        assert_eq!(
            crate::ResolvedTextStyle::new(vec![crate::TextFontFamily::SansSerif], 1, 2)
                .unwrap()
                .scaled(scale),
            Err(TextMetricScaleError::MetricOutOfRange { field: "font_size" }),
        );
    }

    #[test]
    fn point_ratio_rounding_and_domain_are_explicit() {
        for (length, expected) in [
            (CheckedLength::px(24_000), 24_000),
            (CheckedLength::pt(18_000), 24_000),
            (CheckedLength::pt(1), 1),
            (CheckedLength::pt(2), 3),
            (CheckedLength::pt(0), 0),
            (CheckedLength::px(i32::MAX), i32::MAX.unsigned_abs()),
        ] {
            assert_eq!(
                ResolvedTextLength::try_from(length).unwrap().milli_pixels(),
                expected
            );
        }
        assert!(matches!(
            ResolvedTextLength::try_from(CheckedLength::pt(i32::MAX)),
            Err(TextLengthResolutionError::Overflow { .. })
        ));
        assert!(matches!(
            ResolvedTextLength::try_from(CheckedLength::px(-1)),
            Err(TextLengthResolutionError::Negative { .. })
        ));
        for unit in [
            LengthUnit::Ch,
            LengthUnit::Em,
            LengthUnit::Rem,
            LengthUnit::Vw,
            LengthUnit::Vh,
            LengthUnit::Percent,
        ] {
            assert_eq!(
                ResolvedTextLength::try_from(CheckedLength { milli: 1_000, unit }),
                Err(TextLengthResolutionError::UnresolvedUnit { unit })
            );
        }
    }
}
