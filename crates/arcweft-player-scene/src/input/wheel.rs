//! Host-neutral conversion from platform wheel units to logical pixels.

use thiserror::Error;

// Arcweft product default retained from the pre-policy adapters. This is not a
// platform metric or a Web/native standard constant.
const DEFAULT_LOGICAL_PIXELS_PER_LINE: f64 = 32.0;

/// Platform wheel movement before Arcweft applies its shared unit policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WheelDelta {
    Lines { horizontal: f64, vertical: f64 },
    LogicalPixels { horizontal: f64, vertical: f64 },
}

impl WheelDelta {
    pub const fn lines(horizontal: f64, vertical: f64) -> Self {
        Self::Lines {
            horizontal,
            vertical,
        }
    }

    pub const fn logical_pixels(horizontal: f64, vertical: f64) -> Self {
        Self::LogicalPixels {
            horizontal,
            vertical,
        }
    }

    /// Converts physical platform pixels into logical pixels without losing the
    /// scale-factor validation at the shared player boundary.
    pub fn from_physical_pixels(
        horizontal: f64,
        vertical: f64,
        scale_factor: f64,
    ) -> Result<Self, WheelNormalizationError> {
        if !horizontal.is_finite() || !vertical.is_finite() {
            return Err(WheelNormalizationError::NonFinitePhysicalPixels {
                horizontal,
                vertical,
            });
        }
        if !scale_factor.is_finite() || scale_factor <= 0.0 {
            return Err(WheelNormalizationError::InvalidScaleFactor { scale_factor });
        }
        Ok(Self::logical_pixels(
            horizontal / scale_factor,
            vertical / scale_factor,
        ))
    }
}

/// Arcweft's explicit mapping from line-based wheel input to logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelNormalizationPolicy {
    logical_pixels_per_line: f64,
}

impl WheelNormalizationPolicy {
    pub fn new(logical_pixels_per_line: f64) -> Result<Self, WheelNormalizationError> {
        if !logical_pixels_per_line.is_finite() || logical_pixels_per_line <= 0.0 {
            return Err(WheelNormalizationError::InvalidPixelsPerLine {
                logical_pixels_per_line,
            });
        }
        Ok(Self {
            logical_pixels_per_line,
        })
    }

    pub const fn logical_pixels_per_line(self) -> f64 {
        self.logical_pixels_per_line
    }

    pub fn normalize(
        self,
        delta: WheelDelta,
    ) -> Result<LogicalWheelDelta, WheelNormalizationError> {
        let (horizontal, vertical) = match delta {
            WheelDelta::Lines {
                horizontal,
                vertical,
            } => {
                if !horizontal.is_finite() || !vertical.is_finite() {
                    return Err(WheelNormalizationError::NonFiniteLines {
                        horizontal,
                        vertical,
                    });
                }
                (
                    horizontal * self.logical_pixels_per_line,
                    vertical * self.logical_pixels_per_line,
                )
            }
            WheelDelta::LogicalPixels {
                horizontal,
                vertical,
            } => {
                if !horizontal.is_finite() || !vertical.is_finite() {
                    return Err(WheelNormalizationError::NonFiniteLogicalPixels {
                        horizontal,
                        vertical,
                    });
                }
                (horizontal, vertical)
            }
        };
        LogicalWheelDelta::checked(horizontal, vertical)
    }
}

impl Default for WheelNormalizationPolicy {
    fn default() -> Self {
        Self {
            logical_pixels_per_line: DEFAULT_LOGICAL_PIXELS_PER_LINE,
        }
    }
}

/// Finite wheel movement in the logical-pixel coordinate system used by View.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalWheelDelta {
    horizontal: f32,
    vertical: f32,
}

impl LogicalWheelDelta {
    fn checked(horizontal: f64, vertical: f64) -> Result<Self, WheelNormalizationError> {
        if !horizontal.is_finite() || !vertical.is_finite() {
            return Err(WheelNormalizationError::NonFiniteLogicalPixels {
                horizontal,
                vertical,
            });
        }
        if horizontal < f64::from(f32::MIN)
            || horizontal > f64::from(f32::MAX)
            || vertical < f64::from(f32::MIN)
            || vertical > f64::from(f32::MAX)
        {
            return Err(WheelNormalizationError::LogicalPixelRangeExceeded {
                horizontal,
                vertical,
            });
        }
        #[allow(
            clippy::cast_possible_truncation,
            reason = "finite values were proven to be inside View's f32 coordinate domain"
        )]
        let (horizontal, vertical) = (horizontal as f32, vertical as f32);
        Ok(Self {
            horizontal,
            vertical,
        })
    }

    pub const fn horizontal(self) -> f32 {
        self.horizontal
    }

    pub const fn vertical(self) -> f32 {
        self.vertical
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq)]
pub enum WheelNormalizationError {
    #[error("wheel line delta must be finite, got horizontal={horizontal}, vertical={vertical}")]
    NonFiniteLines { horizontal: f64, vertical: f64 },
    #[error(
        "wheel physical-pixel delta must be finite, got horizontal={horizontal}, vertical={vertical}"
    )]
    NonFinitePhysicalPixels { horizontal: f64, vertical: f64 },
    #[error("wheel scale factor must be finite and positive, got {scale_factor}")]
    InvalidScaleFactor { scale_factor: f64 },
    #[error(
        "wheel logical-pixel delta must be finite, got horizontal={horizontal}, vertical={vertical}"
    )]
    NonFiniteLogicalPixels { horizontal: f64, vertical: f64 },
    #[error(
        "wheel logical-pixel delta exceeds the f32 coordinate range: horizontal={horizontal}, vertical={vertical}"
    )]
    LogicalPixelRangeExceeded { horizontal: f64, vertical: f64 },
    #[error(
        "wheel pixels-per-line policy must be finite and positive, got {logical_pixels_per_line}"
    )]
    InvalidPixelsPerLine { logical_pixels_per_line: f64 },
}

#[cfg(test)]
mod tests {
    use super::{WheelDelta, WheelNormalizationError, WheelNormalizationPolicy};

    #[test]
    fn default_policy_normalizes_lines_and_physical_pixels_identically() {
        let policy = WheelNormalizationPolicy::default();
        let lines = policy
            .normalize(WheelDelta::lines(1.0, -2.0))
            .expect("line delta normalizes");
        let pixels = policy
            .normalize(
                WheelDelta::from_physical_pixels(64.0, -128.0, 2.0)
                    .expect("physical pixels normalize to logical pixels"),
            )
            .expect("logical pixels normalize");

        assert_eq!(
            policy.logical_pixels_per_line().to_bits(),
            32.0_f64.to_bits()
        );
        assert_eq!(lines, pixels);
        assert_eq!(lines.horizontal().to_bits(), 32.0_f32.to_bits());
        assert_eq!(lines.vertical().to_bits(), (-64.0_f32).to_bits());
    }

    #[test]
    fn custom_policy_preserves_negative_and_zero_components() {
        let policy = WheelNormalizationPolicy::new(24.0).expect("valid policy");
        let delta = policy
            .normalize(WheelDelta::lines(0.0, -0.5))
            .expect("delta normalizes");

        assert_eq!(delta.horizontal().to_bits(), 0.0_f32.to_bits());
        assert_eq!(delta.vertical().to_bits(), (-12.0_f32).to_bits());
    }

    #[test]
    fn invalid_scale_and_non_finite_input_are_rejected() {
        assert!(matches!(
            WheelDelta::from_physical_pixels(1.0, 2.0, 0.0),
            Err(WheelNormalizationError::InvalidScaleFactor { .. })
        ));
        assert!(matches!(
            WheelDelta::from_physical_pixels(f64::NAN, 2.0, 1.0),
            Err(WheelNormalizationError::NonFinitePhysicalPixels { .. })
        ));
        assert!(matches!(
            WheelNormalizationPolicy::default().normalize(WheelDelta::lines(1.0, f64::INFINITY)),
            Err(WheelNormalizationError::NonFiniteLines { .. })
        ));
    }

    #[test]
    fn extreme_logical_delta_is_rejected_instead_of_saturating() {
        assert!(matches!(
            WheelNormalizationPolicy::default()
                .normalize(WheelDelta::logical_pixels(f64::MAX, 0.0)),
            Err(WheelNormalizationError::LogicalPixelRangeExceeded { .. })
        ));
    }

    #[test]
    fn invalid_line_policy_is_rejected() {
        assert!(matches!(
            WheelNormalizationPolicy::new(f64::NAN),
            Err(WheelNormalizationError::InvalidPixelsPerLine { .. })
        ));
        assert!(matches!(
            WheelNormalizationPolicy::new(-1.0),
            Err(WheelNormalizationError::InvalidPixelsPerLine { .. })
        ));
    }
}
