//! Absolute typography lengths resolved in the shared logical-pixel space.

use arcweft_text_model::{CheckedLength, LengthUnit, Milli};

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
