use serde::{Deserialize, Serialize};
use thiserror::Error;

const SCALE_MICROS_PER_UNIT: u32 = 1_000_000;
const LOGICAL_MILLIS_PER_UNIT: i64 = 1_000;

/// Deterministic scale factor, represented as millionths instead of `f64`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct ScaleFactor(u32);

impl ScaleFactor {
    pub const ONE: Self = Self(SCALE_MICROS_PER_UNIT);

    pub fn from_f64(value: f64) -> Result<Self, GeometryError> {
        if !value.is_finite() || value <= 0.0 {
            return Err(GeometryError::InvalidScaleFactor);
        }
        let micros = value * f64::from(SCALE_MICROS_PER_UNIT);
        if micros > f64::from(u32::MAX) {
            return Err(GeometryError::ScaleFactorOverflow);
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let rounded = micros.round() as u32;
        (rounded > 0)
            .then_some(Self(rounded))
            .ok_or(GeometryError::InvalidScaleFactor)
    }

    pub const fn from_micros(micros: u32) -> Result<Self, GeometryError> {
        if micros == 0 {
            Err(GeometryError::InvalidScaleFactor)
        } else {
            Ok(Self(micros))
        }
    }

    pub const fn micros(self) -> u32 {
        self.0
    }

    pub fn as_f64(self) -> f64 {
        f64::from(self.0) / f64::from(SCALE_MICROS_PER_UNIT)
    }
}

impl TryFrom<u32> for ScaleFactor {
    type Error = GeometryError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::from_micros(value)
    }
}

impl From<ScaleFactor> for u32 {
    fn from(value: ScaleFactor) -> Self {
        value.micros()
    }
}

impl Default for ScaleFactor {
    fn default() -> Self {
        Self::ONE
    }
}

/// Screen or window-local position in physical pixels.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PhysicalPosition {
    pub x: i32,
    pub y: i32,
}

/// Size in physical pixels.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PhysicalSize {
    pub width: u32,
    pub height: u32,
}

/// Rectangle in physical pixels.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PhysicalRect {
    pub position: PhysicalPosition,
    pub size: PhysicalSize,
}

/// Logical position stored in thousandths of one logical pixel.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct LogicalPosition {
    pub x_millis: i64,
    pub y_millis: i64,
}

/// Logical size stored in thousandths of one logical pixel.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct LogicalSize {
    pub width_millis: u64,
    pub height_millis: u64,
}

impl PhysicalPosition {
    pub fn to_logical(self, scale: ScaleFactor) -> Result<LogicalPosition, GeometryError> {
        Ok(LogicalPosition {
            x_millis: physical_coordinate_to_logical(self.x, scale)?,
            y_millis: physical_coordinate_to_logical(self.y, scale)?,
        })
    }
}

impl LogicalPosition {
    pub fn to_physical(self, scale: ScaleFactor) -> Result<PhysicalPosition, GeometryError> {
        Ok(PhysicalPosition {
            x: logical_coordinate_to_physical(self.x_millis, scale)?,
            y: logical_coordinate_to_physical(self.y_millis, scale)?,
        })
    }
}

impl PhysicalSize {
    pub fn to_logical(self, scale: ScaleFactor) -> Result<LogicalSize, GeometryError> {
        Ok(LogicalSize {
            width_millis: physical_extent_to_logical(self.width, scale)?,
            height_millis: physical_extent_to_logical(self.height, scale)?,
        })
    }
}

impl LogicalSize {
    pub fn to_physical(self, scale: ScaleFactor) -> Result<PhysicalSize, GeometryError> {
        Ok(PhysicalSize {
            width: logical_extent_to_physical(self.width_millis, scale)?,
            height: logical_extent_to_physical(self.height_millis, scale)?,
        })
    }
}

fn physical_coordinate_to_logical(
    coordinate: i32,
    scale: ScaleFactor,
) -> Result<i64, GeometryError> {
    let numerator = i128::from(coordinate)
        * i128::from(LOGICAL_MILLIS_PER_UNIT)
        * i128::from(SCALE_MICROS_PER_UNIT);
    let logical = numerator / i128::from(scale.micros());
    i64::try_from(logical).map_err(|_| GeometryError::CoordinateOverflow)
}

fn logical_coordinate_to_physical(
    coordinate_millis: i64,
    scale: ScaleFactor,
) -> Result<i32, GeometryError> {
    let numerator = i128::from(coordinate_millis) * i128::from(scale.micros());
    let denominator = i128::from(LOGICAL_MILLIS_PER_UNIT) * i128::from(SCALE_MICROS_PER_UNIT);
    let physical = numerator / denominator;
    i32::try_from(physical).map_err(|_| GeometryError::CoordinateOverflow)
}

fn physical_extent_to_logical(extent: u32, scale: ScaleFactor) -> Result<u64, GeometryError> {
    let numerator = u128::from(extent)
        * u128::from(LOGICAL_MILLIS_PER_UNIT.unsigned_abs())
        * u128::from(SCALE_MICROS_PER_UNIT);
    let logical = numerator / u128::from(scale.micros());
    u64::try_from(logical).map_err(|_| GeometryError::CoordinateOverflow)
}

fn logical_extent_to_physical(
    extent_millis: u64,
    scale: ScaleFactor,
) -> Result<u32, GeometryError> {
    let numerator = u128::from(extent_millis) * u128::from(scale.micros());
    let denominator =
        u128::from(LOGICAL_MILLIS_PER_UNIT.unsigned_abs()) * u128::from(SCALE_MICROS_PER_UNIT);
    let physical = numerator / denominator;
    u32::try_from(physical).map_err(|_| GeometryError::CoordinateOverflow)
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GeometryError {
    #[error("scale factor must be finite and greater than zero")]
    InvalidScaleFactor,
    #[error("scale factor cannot be represented")]
    ScaleFactorOverflow,
    #[error("coordinate conversion overflowed")]
    CoordinateOverflow,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_factor_round_trips_common_value() {
        let scale = ScaleFactor::from_f64(1.25).expect("valid scale");
        assert_eq!(scale.micros(), 1_250_000);
        assert!((scale.as_f64() - 1.25).abs() < f64::EPSILON);
    }

    #[test]
    fn physical_and_logical_positions_convert_without_float_state() {
        let scale = ScaleFactor::from_f64(2.0).expect("valid scale");
        let physical = PhysicalPosition { x: 200, y: -40 };
        let logical = physical.to_logical(scale).expect("conversion succeeds");
        assert_eq!(logical.x_millis, 100_000);
        assert_eq!(logical.y_millis, -20_000);
        assert_eq!(logical.to_physical(scale), Ok(physical));
    }

    #[test]
    fn scale_factor_deserialization_rejects_zero() {
        let error = serde_json::from_str::<ScaleFactor>("0").expect_err("zero is invalid");
        assert!(error.to_string().contains("greater than zero"));
    }
}
