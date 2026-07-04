//! Deterministic effect-pass planning for the Arcweft UI compositor.
//!
//! The types in this module are pure renderer data: they translate the seq06.9a
//! `UiFilterList` contract into color-matrix, blur, and drop-shadow passes, and
//! compute the exact offscreen target extents that the wgpu executor must use.

use crate::ui_scene::{UiColorRgba8, UiFilter, UiFilterList};
use arcweft_presentation::hit::HitRect;
use num_traits::ToPrimitive;

const LUMA_RED: f32 = 0.2126;
const LUMA_GREEN: f32 = 0.7152;
const LUMA_BLUE: f32 = 0.0722;
const MAX_TEXTURE_DIMENSION: u32 = 16_384;

/// Device-pixel size of a compositor intermediate texture.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UiTextureExtent {
    pub width: u32,
    pub height: u32,
}

/// Device-pixel rectangle used by clipping and target-copy planning.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UiTextureRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// One horizontal or vertical blur pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiBlurDirection {
    Horizontal,
    Vertical,
}

/// CSS/SVG-compatible 4x4 color matrix plus additive offset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiColorMatrix {
    pub matrix: [[f32; 4]; 4],
    pub offset: [f32; 4],
}

/// A planned separable blur pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiBlurPassPlan {
    pub direction: UiBlurDirection,
    pub radius_px: f32,
    pub input_extent: UiTextureExtent,
    pub output_extent: UiTextureExtent,
}

/// Planned drop-shadow construction from source alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiDropShadowPassPlan {
    pub offset_x_px: f32,
    pub offset_y_px: f32,
    pub blur_radius_px: f32,
    pub tint: UiColorRgba8,
    pub source_extent: UiTextureExtent,
    pub shadow_extent: UiTextureExtent,
}

/// One compositor effect pass.
#[derive(Clone, Debug, PartialEq)]
pub enum UiEffectPass {
    ColorMatrix(UiColorMatrix),
    Blur(UiBlurPassPlan),
    DropShadow(UiDropShadowPassPlan),
    Unsupported { name: Box<str>, reason: Box<str> },
}

/// Deterministic pass sequence for one `UiFilterList`.
#[derive(Clone, Debug, PartialEq)]
pub struct UiFilterPassPlan {
    input_extent: UiTextureExtent,
    output_extent: UiTextureExtent,
    passes: Vec<UiEffectPass>,
}

impl UiTextureExtent {
    pub const MAX: Self = Self {
        width: MAX_TEXTURE_DIMENSION,
        height: MAX_TEXTURE_DIMENSION,
    };

    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn from_logical_bounds(bounds: HitRect, device_pixel_ratio: f32, outset_px: f32) -> Self {
        let scale = positive_f32(device_pixel_ratio).max(1.0);
        let outset = positive_f32(outset_px);
        Self {
            width: ceil_positive((bounds.width + outset * 2.0) * scale),
            height: ceil_positive((bounds.height + outset * 2.0) * scale),
        }
        .clamped(Self::MAX)
    }

    pub fn from_viewport(width: f32, height: f32, device_pixel_ratio: f32) -> Self {
        let scale = positive_f32(device_pixel_ratio).max(1.0);
        Self {
            width: ceil_positive(width * scale),
            height: ceil_positive(height * scale),
        }
        .clamped(Self::MAX)
    }

    #[must_use]
    pub fn expanded(self, outset_px: u32) -> Self {
        let doubled = outset_px.saturating_mul(2);
        Self {
            width: self.width.saturating_add(doubled),
            height: self.height.saturating_add(doubled),
        }
        .clamped(Self::MAX)
    }

    #[must_use]
    pub fn bucketed(self, max: Self) -> Self {
        Self {
            width: bucket_dimension(self.width).min(max.width.max(1)),
            height: bucket_dimension(self.height).min(max.height.max(1)),
        }
    }

    #[must_use]
    pub fn clamped(self, max: Self) -> Self {
        Self {
            width: self.width.clamp(1, max.width.max(1)),
            height: self.height.clamp(1, max.height.max(1)),
        }
    }

    pub const fn area_pixels(self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

impl UiTextureRect {
    pub fn from_logical_bounds(bounds: HitRect, device_pixel_ratio: f32, outset_px: f32) -> Self {
        let scale = positive_f32(device_pixel_ratio).max(1.0);
        let outset = positive_f32(outset_px);
        let x = ((bounds.x - outset) * scale).floor();
        let y = ((bounds.y - outset) * scale).floor();
        Self {
            x: clamp_i32(x),
            y: clamp_i32(y),
            width: ceil_positive((bounds.width + outset * 2.0) * scale),
            height: ceil_positive((bounds.height + outset * 2.0) * scale),
        }
    }

    pub const fn extent(self) -> UiTextureExtent {
        UiTextureExtent::new(self.width, self.height)
    }
}

impl UiColorMatrix {
    pub const IDENTITY: Self = Self {
        matrix: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        offset: [0.0, 0.0, 0.0, 0.0],
    };

    pub const fn identity() -> Self {
        Self::IDENTITY
    }

    pub fn brightness(value: f32) -> Self {
        Self::diagonal(value, value, value, 1.0)
    }

    pub fn contrast(value: f32) -> Self {
        Self {
            matrix: [
                [value, 0.0, 0.0, 0.0],
                [0.0, value, 0.0, 0.0],
                [0.0, 0.0, value, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            offset: [
                0.5 * (1.0 - value),
                0.5 * (1.0 - value),
                0.5 * (1.0 - value),
                0.0,
            ],
        }
    }

    pub fn grayscale(amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let inverse = 1.0 - amount;
        Self {
            matrix: [
                [
                    inverse + LUMA_RED * amount,
                    LUMA_GREEN * amount,
                    LUMA_BLUE * amount,
                    0.0,
                ],
                [
                    LUMA_RED * amount,
                    inverse + LUMA_GREEN * amount,
                    LUMA_BLUE * amount,
                    0.0,
                ],
                [
                    LUMA_RED * amount,
                    LUMA_GREEN * amount,
                    inverse + LUMA_BLUE * amount,
                    0.0,
                ],
                [0.0, 0.0, 0.0, 1.0],
            ],
            offset: [0.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn sepia(amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let inverse = 1.0 - amount;
        let sepia = [
            [0.393, 0.769, 0.189, 0.0],
            [0.349, 0.686, 0.168, 0.0],
            [0.272, 0.534, 0.131, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        Self::interpolate_matrix(Self::IDENTITY.matrix, sepia, inverse, amount)
    }

    pub fn saturate(amount: f32) -> Self {
        let inverse = 1.0 - amount;
        Self {
            matrix: [
                [
                    LUMA_RED * inverse + amount,
                    LUMA_GREEN * inverse,
                    LUMA_BLUE * inverse,
                    0.0,
                ],
                [
                    LUMA_RED * inverse,
                    LUMA_GREEN * inverse + amount,
                    LUMA_BLUE * inverse,
                    0.0,
                ],
                [
                    LUMA_RED * inverse,
                    LUMA_GREEN * inverse,
                    LUMA_BLUE * inverse + amount,
                    0.0,
                ],
                [0.0, 0.0, 0.0, 1.0],
            ],
            offset: [0.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn hue_rotate_degrees(degrees: f32) -> Self {
        let radians = degrees.to_radians();
        let cos = radians.cos();
        let sin = radians.sin();
        Self {
            matrix: [
                [
                    LUMA_RED + cos * (1.0 - LUMA_RED) + sin * -LUMA_RED,
                    LUMA_GREEN + cos * -LUMA_GREEN + sin * -LUMA_GREEN,
                    LUMA_BLUE + cos * -LUMA_BLUE + sin * (1.0 - LUMA_BLUE),
                    0.0,
                ],
                [
                    LUMA_RED + cos * -LUMA_RED + sin * 0.143,
                    LUMA_GREEN + cos * (1.0 - LUMA_GREEN) + sin * 0.140,
                    LUMA_BLUE + cos * -LUMA_BLUE + sin * -0.283,
                    0.0,
                ],
                [
                    LUMA_RED + cos * -LUMA_RED + sin * -(1.0 - LUMA_RED),
                    LUMA_GREEN + cos * -LUMA_GREEN + sin * LUMA_GREEN,
                    LUMA_BLUE + cos * (1.0 - LUMA_BLUE) + sin * LUMA_BLUE,
                    0.0,
                ],
                [0.0, 0.0, 0.0, 1.0],
            ],
            offset: [0.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn invert(amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let scale = 1.0 - amount * 2.0;
        Self {
            matrix: [
                [scale, 0.0, 0.0, 0.0],
                [0.0, scale, 0.0, 0.0],
                [0.0, 0.0, scale, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            offset: [amount, amount, amount, 0.0],
        }
    }

    pub fn opacity(amount: f32) -> Self {
        Self::diagonal(1.0, 1.0, 1.0, amount)
    }

    pub fn from_filter(filter: &UiFilter) -> Option<Self> {
        match filter {
            UiFilter::Brightness(value) => Some(Self::brightness(*value)),
            UiFilter::Contrast(value) => Some(Self::contrast(*value)),
            UiFilter::Grayscale(value) => Some(Self::grayscale(*value)),
            UiFilter::Saturate(value) => Some(Self::saturate(*value)),
            UiFilter::HueRotateDegrees(value) => Some(Self::hue_rotate_degrees(*value)),
            UiFilter::Invert(value) => Some(Self::invert(*value)),
            UiFilter::Sepia(value) => Some(Self::sepia(*value)),
            UiFilter::Opacity(value) => Some(Self::opacity(*value)),
            UiFilter::Blur { .. } | UiFilter::DropShadow { .. } | UiFilter::Unsupported { .. } => {
                None
            }
        }
    }

    pub fn as_uniform(self) -> ([[f32; 4]; 4], [f32; 4]) {
        (self.matrix, self.offset)
    }

    fn diagonal(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            matrix: [
                [red, 0.0, 0.0, 0.0],
                [0.0, green, 0.0, 0.0],
                [0.0, 0.0, blue, 0.0],
                [0.0, 0.0, 0.0, alpha],
            ],
            offset: [0.0, 0.0, 0.0, 0.0],
        }
    }

    fn interpolate_matrix(
        identity: [[f32; 4]; 4],
        target: [[f32; 4]; 4],
        identity_weight: f32,
        target_weight: f32,
    ) -> Self {
        let mut matrix = [[0.0; 4]; 4];
        for row in 0..4 {
            for column in 0..4 {
                matrix[row][column] =
                    identity[row][column] * identity_weight + target[row][column] * target_weight;
            }
        }
        Self {
            matrix,
            offset: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

impl Default for UiColorMatrix {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl UiFilterPassPlan {
    pub fn from_filter_list(
        filters: &UiFilterList,
        input_extent: UiTextureExtent,
        device_pixel_ratio: f32,
    ) -> Self {
        Self::from_filter_list_with_extent_policy(
            filters,
            input_extent,
            device_pixel_ratio,
            FilterExtentPolicy::ExpandForVisualOutset,
        )
    }

    pub fn from_filter_list_fixed_extent(
        filters: &UiFilterList,
        input_extent: UiTextureExtent,
        device_pixel_ratio: f32,
    ) -> Self {
        Self::from_filter_list_with_extent_policy(
            filters,
            input_extent,
            device_pixel_ratio,
            FilterExtentPolicy::KeepInputExtent,
        )
    }

    fn from_filter_list_with_extent_policy(
        filters: &UiFilterList,
        input_extent: UiTextureExtent,
        device_pixel_ratio: f32,
        extent_policy: FilterExtentPolicy,
    ) -> Self {
        let mut passes = Vec::new();
        let mut current_extent = input_extent;
        let scale = positive_f32(device_pixel_ratio).max(1.0);

        for filter in filters.filters() {
            if let Some(matrix) = UiColorMatrix::from_filter(filter) {
                passes.push(UiEffectPass::ColorMatrix(matrix));
                continue;
            }

            match filter {
                UiFilter::Blur { radius_px } => {
                    let radius_px = positive_f32(*radius_px) * scale;
                    let output_extent = extent_policy.output_extent(current_extent, radius_px);
                    passes.push(UiEffectPass::Blur(UiBlurPassPlan {
                        direction: UiBlurDirection::Horizontal,
                        radius_px,
                        input_extent: current_extent,
                        output_extent,
                    }));
                    passes.push(UiEffectPass::Blur(UiBlurPassPlan {
                        direction: UiBlurDirection::Vertical,
                        radius_px,
                        input_extent: output_extent,
                        output_extent,
                    }));
                    current_extent = output_extent;
                }
                UiFilter::DropShadow {
                    offset_x_px,
                    offset_y_px,
                    blur_radius_px,
                    color,
                } => {
                    let shadow_outset = filter.visual_outset_px() * scale;
                    let shadow_extent = current_extent.expanded(ceil_positive(shadow_outset));
                    passes.push(UiEffectPass::DropShadow(UiDropShadowPassPlan {
                        offset_x_px: *offset_x_px * scale,
                        offset_y_px: *offset_y_px * scale,
                        blur_radius_px: positive_f32(*blur_radius_px) * scale,
                        tint: *color,
                        source_extent: current_extent,
                        shadow_extent,
                    }));
                    current_extent = shadow_extent;
                }
                UiFilter::Unsupported { name, reason } => passes.push(UiEffectPass::Unsupported {
                    name: name.clone(),
                    reason: reason.clone(),
                }),
                UiFilter::Brightness(_)
                | UiFilter::Contrast(_)
                | UiFilter::Grayscale(_)
                | UiFilter::Saturate(_)
                | UiFilter::HueRotateDegrees(_)
                | UiFilter::Invert(_)
                | UiFilter::Sepia(_)
                | UiFilter::Opacity(_) => {}
            }
        }

        Self {
            input_extent,
            output_extent: current_extent,
            passes,
        }
    }

    pub fn empty(extent: UiTextureExtent) -> Self {
        Self {
            input_extent: extent,
            output_extent: extent,
            passes: Vec::new(),
        }
    }

    pub fn passes(&self) -> &[UiEffectPass] {
        &self.passes
    }

    pub const fn input_extent(&self) -> UiTextureExtent {
        self.input_extent
    }

    pub const fn output_extent(&self) -> UiTextureExtent {
        self.output_extent
    }

    pub fn is_empty(&self) -> bool {
        self.passes.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FilterExtentPolicy {
    ExpandForVisualOutset,
    KeepInputExtent,
}

impl FilterExtentPolicy {
    fn output_extent(self, input: UiTextureExtent, blur_radius_px: f32) -> UiTextureExtent {
        match self {
            Self::ExpandForVisualOutset => input.expanded(ceil_positive(blur_radius_px * 3.0)),
            Self::KeepInputExtent => input,
        }
    }
}

fn bucket_dimension(value: u32) -> u32 {
    value.max(1).checked_next_power_of_two().unwrap_or(u32::MAX)
}

fn ceil_positive(value: f32) -> u32 {
    if !value.is_finite() || value <= 0.0 {
        return 1;
    }
    let rounded = f64::from(value.ceil());
    if rounded >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        rounded.to_u32().unwrap_or(u32::MAX).max(1)
    }
}

fn clamp_i32(value: f32) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    let value = f64::from(value);
    if value <= f64::from(i32::MIN) {
        i32::MIN
    } else if value >= f64::from(i32::MAX) {
        i32::MAX
    } else {
        value.to_i32().unwrap_or(0)
    }
}

fn positive_f32(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba(alpha: u8) -> UiColorRgba8 {
        UiColorRgba8 {
            red: 12,
            green: 34,
            blue: 56,
            alpha,
        }
    }

    #[test]
    fn color_matrix_contrast_and_invert_are_deterministic() {
        let contrast = UiColorMatrix::contrast(1.5);
        assert!((contrast.matrix[0][0] - 1.5).abs() <= f32::EPSILON);
        assert!((contrast.offset[0] + 0.25).abs() <= f32::EPSILON);

        let invert = UiColorMatrix::invert(1.0);
        assert!((invert.matrix[0][0] + 1.0).abs() <= f32::EPSILON);
        assert!((invert.offset[0] - 1.0).abs() <= f32::EPSILON);
    }

    #[test]
    fn filter_plan_expands_for_blur_and_drop_shadow() {
        let filters = UiFilterList::new([
            UiFilter::Brightness(1.2),
            UiFilter::Blur { radius_px: 4.0 },
            UiFilter::DropShadow {
                offset_x_px: 8.0,
                offset_y_px: 2.0,
                blur_radius_px: 6.0,
                color: rgba(200),
            },
        ]);
        let plan = UiFilterPassPlan::from_filter_list(&filters, UiTextureExtent::new(32, 16), 1.0);

        assert_eq!(plan.passes().len(), 4);
        assert!(plan.output_extent().width > 32);
        assert!(plan.output_extent().height > 16);
    }

    #[test]
    fn fixed_extent_filter_plan_keeps_backdrop_target_size() {
        let filters = UiFilterList::new([UiFilter::Blur { radius_px: 4.0 }]);
        let extent = UiTextureExtent::new(32, 16);
        let plan = UiFilterPassPlan::from_filter_list_fixed_extent(&filters, extent, 1.0);

        assert_eq!(plan.passes().len(), 2);
        assert_eq!(plan.input_extent(), extent);
        assert_eq!(plan.output_extent(), extent);
    }

    #[test]
    fn texture_extent_buckets_without_exceeding_cap() {
        let extent = UiTextureExtent::new(257, 511).bucketed(UiTextureExtent::new(512, 512));
        assert_eq!(extent, UiTextureExtent::new(512, 512));
    }
}
