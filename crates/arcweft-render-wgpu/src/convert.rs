//! Bounded numeric conversions for layout and sample asset generation.

use num_traits::ToPrimitive;

pub(crate) fn usize_to_f32(value: usize) -> f32 {
    value.to_f32().unwrap_or(f32::MAX)
}

pub(crate) fn u32_to_f32(value: u32) -> f32 {
    value.to_f32().unwrap_or(f32::MAX)
}

pub(crate) fn u64_to_f32(value: u64) -> f32 {
    value.to_f32().unwrap_or(f32::MAX)
}

pub(crate) fn f32_floor_to_i32(value: f32) -> i32 {
    f32_to_i32(value, f32::floor)
}

pub(crate) fn f32_ceil_to_i32(value: f32) -> i32 {
    f32_to_i32(value, f32::ceil)
}

pub(crate) fn f32_to_u8_nonnegative(value: f32) -> u8 {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    value.min(f32::from(u8::MAX)).floor().to_u8().unwrap_or(0)
}

fn f32_to_i32(value: f32, round: fn(f32) -> f32) -> i32 {
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
