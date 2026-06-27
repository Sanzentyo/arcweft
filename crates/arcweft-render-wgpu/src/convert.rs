//! Bounded numeric conversions for layout and sample asset generation.

use num_traits::ToPrimitive;

/// Converts `usize` to `f32`, saturating to `f32::MAX` when the value does not fit.
pub(crate) fn saturating_usize_as_f32(value: usize) -> f32 {
    value.to_f32().unwrap_or(f32::MAX)
}

/// Converts `u32` to `f32`, saturating to `f32::MAX` when the value does not fit.
pub(crate) fn saturating_u32_as_f32(value: u32) -> f32 {
    value.to_f32().unwrap_or(f32::MAX)
}

/// Converts `u64` to `f32`, saturating to `f32::MAX` when the value does not fit.
pub(crate) fn saturating_u64_as_f32(value: u64) -> f32 {
    value.to_f32().unwrap_or(f32::MAX)
}

/// Converts a `f32` pixel coordinate to `i32` by flooring after clamping to the
/// representable range and zeroing non-finite inputs.
pub(crate) fn pixel_floor_as_i32(value: f32) -> i32 {
    rounded_pixel_as_i32(value, f32::floor)
}

/// Converts a `f32` pixel coordinate to `i32` by ceiling after clamping to the
/// representable range and zeroing non-finite inputs.
pub(crate) fn pixel_ceil_as_i32(value: f32) -> i32 {
    rounded_pixel_as_i32(value, f32::ceil)
}

/// Converts a nonnegative alpha value to `u8`, returning zero for invalid or
/// non-finite inputs and flooring the positive range.
pub(crate) fn nonnegative_alpha_byte(value: f32) -> u8 {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    value.min(f32::from(u8::MAX)).floor().to_u8().unwrap_or(0)
}

fn rounded_pixel_as_i32(value: f32, round: fn(f32) -> f32) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    let min = i32::MIN.to_f32().unwrap_or(f32::MIN);
    let max = i32::MAX.to_f32().unwrap_or(f32::MAX);
    if value <= min {
        return i32::MIN;
    }
    if value >= max {
        return i32::MAX;
    }
    round(value).to_i32().unwrap_or(0)
}
