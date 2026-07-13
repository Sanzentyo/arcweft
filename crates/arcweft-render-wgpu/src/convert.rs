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

/// Converts a nonnegative alpha value to `u8`, returning zero for invalid or
/// non-finite inputs and flooring the positive range.
pub(crate) fn nonnegative_alpha_byte(value: f32) -> u8 {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    value.min(f32::from(u8::MAX)).floor().to_u8().unwrap_or(0)
}
