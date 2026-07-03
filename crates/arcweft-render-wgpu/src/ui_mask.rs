//! Mask composition planning for the UI compositor.

use crate::ui_effects::UiTextureExtent;
use crate::ui_scene::{
    UiElementMaskSource, UiGradientStop, UiLength, UiMask, UiMaskGradient, UiMaskImage,
    UiMaskPosition, UiMaskRepeat, UiMaskSize, UiPoint,
};
use num_traits::ToPrimitive;
use thiserror::Error;

pub const MAX_MASK_GRADIENT_STOPS: usize = 8;

/// How a mask source contributes coverage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UiMaskChannel {
    #[default]
    Alpha,
    Luminance,
}

/// Per-axis mask tile distribution mode after CSS repeat normalization.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UiMaskAxisRepeat {
    NoRepeat,
    #[default]
    Repeat,
    Space,
    Round,
}

/// Renderer-facing source requirement for one mask image.
#[derive(Clone, Debug, PartialEq)]
pub enum UiMaskImagePlan {
    None,
    Texture { resource: Box<str> },
    Gradient(UiMaskGradient),
    Element(UiElementMaskSource),
    Unsupported(Box<str>),
}

/// One mask pass after CSS mask fields are normalized by lowering.
#[derive(Clone, Debug, PartialEq)]
pub struct UiMaskPassPlan {
    pub mask_index: usize,
    pub image: UiMaskImagePlan,
    pub size: UiMaskSize,
    pub position: UiMaskPosition,
    pub repeat: UiMaskRepeat,
    pub channel: UiMaskChannel,
}

/// Pixel-space sampling contract for one mask pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiMaskSamplingPlan {
    pub tile_size_px: [f32; 2],
    pub tile_origin_px: [f32; 2],
    pub tile_stride_px: [f32; 2],
    pub tile_count: [u32; 2],
    pub repeat_mode_x: UiMaskAxisRepeat,
    pub repeat_mode_y: UiMaskAxisRepeat,
    /// Convenience compatibility field for callers that only need old repeat/no-repeat behavior.
    pub repeat_x: bool,
    /// Convenience compatibility field for callers that only need old repeat/no-repeat behavior.
    pub repeat_y: bool,
}

/// Packed gradient coverage plan consumed by the compositor uniform contract.
#[derive(Clone, Debug, PartialEq)]
pub struct UiMaskGradientPlan {
    pub kind: UiMaskGradientKind,
    pub stops: Vec<UiMaskGradientStopPlan>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiMaskGradientKind {
    Linear {
        angle_degrees: f32,
    },
    Radial {
        center_px: [f32; 2],
        radius_px: [f32; 2],
    },
    Conic {
        center_px: [f32; 2],
        from_degrees: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiMaskGradientStopPlan {
    pub offset: f32,
    pub alpha_coverage: f32,
    pub luminance_coverage: f32,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum UiMaskPlanError {
    #[error("unsupported mask image: {0}")]
    UnsupportedImage(Box<str>),
    #[error("unsupported mask-size: {0}")]
    UnsupportedSize(Box<str>),
    #[error("unsupported mask-position: {0}")]
    UnsupportedPosition(Box<str>),
    #[error("unsupported mask-repeat: {0}")]
    UnsupportedRepeat(Box<str>),
    #[error("unsupported mask gradient: {reason}")]
    UnsupportedGradient { reason: Box<str> },
    #[error("mask gradient has {count} stops but supports at most {maximum}")]
    TooManyGradientStops { count: usize, maximum: usize },
    #[error("mask gradient requires at least two stops but found {count}")]
    InvalidGradientStopCount { count: usize },
    #[error("mask element `{element_id}` has no prepared capture resource in seq06.13c")]
    ElementMaskCaptureUnavailable { element_id: Box<str> },
}

/// Ordered chain of mask passes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiMaskChainPlan {
    passes: Vec<UiMaskPassPlan>,
}

impl UiMaskChainPlan {
    pub fn from_masks(masks: &[UiMask], default_channel: UiMaskChannel) -> Self {
        Self {
            passes: masks
                .iter()
                .enumerate()
                .map(|(mask_index, mask)| {
                    UiMaskPassPlan::from_mask(mask_index, mask, default_channel)
                })
                .collect(),
        }
    }

    pub fn passes(&self) -> &[UiMaskPassPlan] {
        &self.passes
    }

    pub fn is_empty(&self) -> bool {
        self.passes.is_empty()
    }

    pub fn requires_external_texture(&self) -> bool {
        self.passes
            .iter()
            .any(UiMaskPassPlan::requires_external_texture)
    }

    pub fn unsupported_count(&self) -> usize {
        self.passes
            .iter()
            .filter(|pass| pass.is_unsupported())
            .count()
    }
}

impl UiMaskPassPlan {
    pub fn from_mask(mask_index: usize, mask: &UiMask, channel: UiMaskChannel) -> Self {
        Self {
            mask_index,
            image: UiMaskImagePlan::from_image(&mask.image),
            size: mask.size.clone(),
            position: mask.position.clone(),
            repeat: mask.repeat.clone(),
            channel,
        }
    }

    pub fn requires_external_texture(&self) -> bool {
        matches!(self.image, UiMaskImagePlan::Texture { .. })
    }

    pub fn sampling_plan(
        &self,
        source_extent: UiTextureExtent,
        mask_extent: UiTextureExtent,
    ) -> Result<UiMaskSamplingPlan, UiMaskPlanError> {
        self.validate_image_for_sampling()?;
        let source_width = dimension_to_f32(source_extent.width);
        let source_height = dimension_to_f32(source_extent.height);
        let mask_width = dimension_to_f32(mask_extent.width);
        let mask_height = dimension_to_f32(mask_extent.height);
        let tile_size_px = resolve_size(
            &self.size,
            source_width,
            source_height,
            mask_width,
            mask_height,
        )?;
        let tile_origin_px =
            resolve_position(&self.position, source_width, source_height, tile_size_px)?;
        let (repeat_mode_x, repeat_mode_y) = repeat_modes(&self.repeat)?;
        Ok(resolve_sampling_plan(
            [source_width, source_height],
            tile_size_px,
            tile_origin_px,
            [repeat_mode_x, repeat_mode_y],
        ))
    }

    pub fn gradient_plan(
        &self,
        tile_size_px: [f32; 2],
    ) -> Result<Option<UiMaskGradientPlan>, UiMaskPlanError> {
        match &self.image {
            UiMaskImagePlan::Gradient(gradient) => {
                UiMaskGradientPlan::from_gradient(gradient, tile_size_px).map(Some)
            }
            UiMaskImagePlan::Element(source) => {
                Err(UiMaskPlanError::ElementMaskCaptureUnavailable {
                    element_id: source.element_id.clone(),
                })
            }
            UiMaskImagePlan::Unsupported(reason) => {
                Err(UiMaskPlanError::UnsupportedImage(reason.clone()))
            }
            UiMaskImagePlan::None | UiMaskImagePlan::Texture { .. } => Ok(None),
        }
    }

    pub fn is_unsupported(&self) -> bool {
        matches!(self.image, UiMaskImagePlan::Unsupported(_))
            || matches!(self.image, UiMaskImagePlan::Element(_))
            || matches!(self.size, UiMaskSize::Unsupported(_))
            || matches!(self.repeat, UiMaskRepeat::Unsupported(_))
    }

    fn validate_image_for_sampling(&self) -> Result<(), UiMaskPlanError> {
        match &self.image {
            UiMaskImagePlan::Unsupported(reason) => {
                Err(UiMaskPlanError::UnsupportedImage(reason.clone()))
            }
            UiMaskImagePlan::Element(source) => {
                Err(UiMaskPlanError::ElementMaskCaptureUnavailable {
                    element_id: source.element_id.clone(),
                })
            }
            UiMaskImagePlan::Gradient(UiMaskGradient::Unsupported(reason)) => {
                Err(UiMaskPlanError::UnsupportedGradient {
                    reason: reason.clone(),
                })
            }
            UiMaskImagePlan::None
            | UiMaskImagePlan::Texture { .. }
            | UiMaskImagePlan::Gradient(_) => Ok(()),
        }
    }
}

impl UiMaskImagePlan {
    pub fn from_image(image: &UiMaskImage) -> Self {
        match image {
            UiMaskImage::None => Self::None,
            UiMaskImage::Url(resource) => Self::Texture {
                resource: resource.clone(),
            },
            UiMaskImage::Gradient(UiMaskGradient::Unsupported(reason)) => {
                Self::Unsupported(reason.clone())
            }
            UiMaskImage::Gradient(gradient) => Self::Gradient(gradient.clone()),
            UiMaskImage::Element(source) => Self::Element(source.clone()),
            UiMaskImage::Unsupported(reason) => Self::Unsupported(reason.clone()),
        }
    }
}

impl UiMaskGradientPlan {
    pub fn from_gradient(
        gradient: &UiMaskGradient,
        tile_size_px: [f32; 2],
    ) -> Result<Self, UiMaskPlanError> {
        match gradient {
            UiMaskGradient::Linear {
                angle_degrees,
                stops,
            } => Ok(Self {
                kind: UiMaskGradientKind::Linear {
                    angle_degrees: *angle_degrees,
                },
                stops: canonical_gradient_stops(stops)?,
            }),
            UiMaskGradient::Radial {
                center,
                radius_x,
                radius_y,
                stops,
            } => Ok(Self {
                kind: UiMaskGradientKind::Radial {
                    center_px: resolve_point_px(center, tile_size_px)?,
                    radius_px: [
                        resolve_length_px(radius_x, tile_size_px[0], "radial-radius-x")?,
                        resolve_length_px(radius_y, tile_size_px[1], "radial-radius-y")?,
                    ],
                },
                stops: canonical_gradient_stops(stops)?,
            }),
            UiMaskGradient::Conic {
                center,
                from_degrees,
                stops,
            } => Ok(Self {
                kind: UiMaskGradientKind::Conic {
                    center_px: resolve_point_px(center, tile_size_px)?,
                    from_degrees: *from_degrees,
                },
                stops: canonical_gradient_stops(stops)?,
            }),
            UiMaskGradient::Unsupported(reason) => Err(UiMaskPlanError::UnsupportedGradient {
                reason: reason.clone(),
            }),
        }
    }
}

fn resolve_sampling_plan(
    source_size_px: [f32; 2],
    tile_size_px: [f32; 2],
    tile_origin_px: [f32; 2],
    repeat_modes: [UiMaskAxisRepeat; 2],
) -> UiMaskSamplingPlan {
    let x = resolve_axis(
        source_size_px[0],
        tile_size_px[0],
        tile_origin_px[0],
        repeat_modes[0],
    );
    let y = resolve_axis(
        source_size_px[1],
        tile_size_px[1],
        tile_origin_px[1],
        repeat_modes[1],
    );
    UiMaskSamplingPlan {
        tile_size_px: [x.tile_size_px, y.tile_size_px],
        tile_origin_px: [x.origin_px, y.origin_px],
        tile_stride_px: [x.stride_px, y.stride_px],
        tile_count: [x.tile_count, y.tile_count],
        repeat_mode_x: repeat_modes[0],
        repeat_mode_y: repeat_modes[1],
        repeat_x: repeat_modes[0] != UiMaskAxisRepeat::NoRepeat,
        repeat_y: repeat_modes[1] != UiMaskAxisRepeat::NoRepeat,
    }
}

struct AxisSampling {
    tile_size_px: f32,
    origin_px: f32,
    stride_px: f32,
    tile_count: u32,
}

fn resolve_axis(
    source_size_px: f32,
    tile_size_px: f32,
    origin_px: f32,
    repeat: UiMaskAxisRepeat,
) -> AxisSampling {
    let source_size_px = source_size_px.max(1.0);
    let tile_size_px = tile_size_px.max(1.0);
    match repeat {
        UiMaskAxisRepeat::NoRepeat => AxisSampling {
            tile_size_px,
            origin_px,
            stride_px: tile_size_px,
            tile_count: 1,
        },
        UiMaskAxisRepeat::Repeat => AxisSampling {
            tile_size_px,
            origin_px,
            stride_px: tile_size_px,
            tile_count: 0,
        },
        UiMaskAxisRepeat::Space => {
            let count = repeat_tile_count((source_size_px / tile_size_px).floor());
            if count <= 1 {
                AxisSampling {
                    tile_size_px,
                    origin_px: (source_size_px - tile_size_px) * 0.5,
                    stride_px: tile_size_px,
                    tile_count: 1,
                }
            } else {
                AxisSampling {
                    tile_size_px,
                    origin_px: 0.0,
                    stride_px: (source_size_px - tile_size_px)
                        / (count - 1).to_f32().unwrap_or(1.0),
                    tile_count: count,
                }
            }
        }
        UiMaskAxisRepeat::Round => {
            let count = repeat_tile_count((source_size_px / tile_size_px).round());
            let resized = source_size_px / count.to_f32().unwrap_or(1.0).max(1.0);
            AxisSampling {
                tile_size_px: resized,
                origin_px: 0.0,
                stride_px: resized,
                tile_count: count,
            }
        }
    }
}

fn repeat_tile_count(value: f32) -> u32 {
    value.max(1.0).to_u32().unwrap_or(u32::MAX).max(1)
}

fn resolve_size(
    size: &UiMaskSize,
    source_width: f32,
    source_height: f32,
    mask_width: f32,
    mask_height: f32,
) -> Result<[f32; 2], UiMaskPlanError> {
    match size {
        UiMaskSize::Unspecified | UiMaskSize::Auto => Ok([mask_width, mask_height]),
        UiMaskSize::Cover => {
            let scale = (source_width / mask_width)
                .max(source_height / mask_height)
                .max(0.0);
            Ok([mask_width * scale, mask_height * scale])
        }
        UiMaskSize::Contain => {
            let scale = (source_width / mask_width)
                .min(source_height / mask_height)
                .max(0.0);
            Ok([mask_width * scale, mask_height * scale])
        }
        UiMaskSize::Explicit { width, height } => {
            let width = width
                .resolve_px(source_width)
                .ok_or_else(|| UiMaskPlanError::UnsupportedSize("width".into()))?;
            let height = height
                .resolve_px(source_height)
                .ok_or_else(|| UiMaskPlanError::UnsupportedSize("height".into()))?;
            Ok([width.max(1.0), height.max(1.0)])
        }
        UiMaskSize::Unsupported(reason) => Err(UiMaskPlanError::UnsupportedSize(reason.clone())),
    }
}

fn resolve_position(
    position: &UiMaskPosition,
    source_width: f32,
    source_height: f32,
    tile_size_px: [f32; 2],
) -> Result<[f32; 2], UiMaskPlanError> {
    let available_width = (source_width - tile_size_px[0]).max(0.0);
    let available_height = (source_height - tile_size_px[1]).max(0.0);
    let x = position
        .anchor
        .x
        .resolve_px(available_width)
        .ok_or_else(|| UiMaskPlanError::UnsupportedPosition("x".into()))?;
    let y = position
        .anchor
        .y
        .resolve_px(available_height)
        .ok_or_else(|| UiMaskPlanError::UnsupportedPosition("y".into()))?;
    Ok([x, y])
}

fn repeat_modes(
    repeat: &UiMaskRepeat,
) -> Result<(UiMaskAxisRepeat, UiMaskAxisRepeat), UiMaskPlanError> {
    match repeat {
        UiMaskRepeat::Unspecified | UiMaskRepeat::Repeat => {
            Ok((UiMaskAxisRepeat::Repeat, UiMaskAxisRepeat::Repeat))
        }
        UiMaskRepeat::NoRepeat => Ok((UiMaskAxisRepeat::NoRepeat, UiMaskAxisRepeat::NoRepeat)),
        UiMaskRepeat::RepeatX => Ok((UiMaskAxisRepeat::Repeat, UiMaskAxisRepeat::NoRepeat)),
        UiMaskRepeat::RepeatY => Ok((UiMaskAxisRepeat::NoRepeat, UiMaskAxisRepeat::Repeat)),
        UiMaskRepeat::Space => Ok((UiMaskAxisRepeat::Space, UiMaskAxisRepeat::Space)),
        UiMaskRepeat::Round => Ok((UiMaskAxisRepeat::Round, UiMaskAxisRepeat::Round)),
        UiMaskRepeat::Unsupported(reason) => {
            Err(UiMaskPlanError::UnsupportedRepeat(reason.clone()))
        }
    }
}

fn canonical_gradient_stops(
    stops: &[UiGradientStop],
) -> Result<Vec<UiMaskGradientStopPlan>, UiMaskPlanError> {
    if stops.len() < 2 {
        return Err(UiMaskPlanError::InvalidGradientStopCount { count: stops.len() });
    }
    if stops.len() > MAX_MASK_GRADIENT_STOPS {
        return Err(UiMaskPlanError::TooManyGradientStops {
            count: stops.len(),
            maximum: MAX_MASK_GRADIENT_STOPS,
        });
    }
    let mut previous = 0.0f32;
    let mut result = Vec::with_capacity(stops.len());
    for stop in stops {
        let offset = stop.offset.clamp(0.0, 1.0).max(previous);
        previous = offset;
        let alpha = f32::from(stop.color.alpha) / 255.0;
        let luminance = (f32::from(stop.color.red) / 255.0) * 0.2126
            + (f32::from(stop.color.green) / 255.0) * 0.7152
            + (f32::from(stop.color.blue) / 255.0) * 0.0722;
        result.push(UiMaskGradientStopPlan {
            offset,
            alpha_coverage: alpha,
            luminance_coverage: luminance * alpha,
        });
    }
    Ok(result)
}

fn resolve_point_px(point: &UiPoint, tile_size_px: [f32; 2]) -> Result<[f32; 2], UiMaskPlanError> {
    Ok([
        resolve_length_px(&point.x, tile_size_px[0], "gradient-center-x")?,
        resolve_length_px(&point.y, tile_size_px[1], "gradient-center-y")?,
    ])
}

fn resolve_length_px(
    length: &UiLength,
    basis_px: f32,
    role: &'static str,
) -> Result<f32, UiMaskPlanError> {
    length
        .resolve_px(basis_px)
        .map(|value| value.max(0.0))
        .ok_or_else(|| UiMaskPlanError::UnsupportedGradient {
            reason: role.into(),
        })
}

fn dimension_to_f32(value: u32) -> f32 {
    value.max(1).to_f32().unwrap_or(f32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_scene::{UiColorRgba8, UiMaskGradient};

    #[test]
    fn url_masks_require_external_textures() {
        let masks = [UiMask {
            image: UiMaskImage::Url("arcweft://mask/card".into()),
            ..UiMask::default()
        }];
        let plan = UiMaskChainPlan::from_masks(&masks, UiMaskChannel::Alpha);
        assert!(plan.requires_external_texture());
        assert_eq!(plan.unsupported_count(), 0);
    }

    #[test]
    fn space_and_round_repeat_resolve_deterministically() {
        let space = UiMaskPassPlan::from_mask(
            0,
            &UiMask {
                image: UiMaskImage::Url("arcweft://mask/space".into()),
                size: UiMaskSize::Explicit {
                    width: UiLength::Px(30.0),
                    height: UiLength::Px(20.0),
                },
                repeat: UiMaskRepeat::Space,
                ..UiMask::default()
            },
            UiMaskChannel::Alpha,
        );
        let sampling = space
            .sampling_plan(UiTextureExtent::new(100, 60), UiTextureExtent::new(10, 10))
            .expect("space repeat resolves");
        assert_eq!(sampling.repeat_mode_x, UiMaskAxisRepeat::Space);
        assert_eq!(sampling.tile_count[0], 3);
        assert!((sampling.tile_stride_px[0] - 35.0).abs() <= 0.001);

        let round = UiMaskPassPlan::from_mask(
            0,
            &UiMask {
                image: UiMaskImage::Url("arcweft://mask/round".into()),
                size: UiMaskSize::Explicit {
                    width: UiLength::Px(30.0),
                    height: UiLength::Px(20.0),
                },
                repeat: UiMaskRepeat::Round,
                ..UiMask::default()
            },
            UiMaskChannel::Alpha,
        );
        let sampling = round
            .sampling_plan(UiTextureExtent::new(100, 60), UiTextureExtent::new(10, 10))
            .expect("round repeat resolves");
        assert_eq!(sampling.tile_count[0], 3);
        assert!((sampling.tile_size_px[0] - 33.333).abs() <= 0.01);
    }

    #[test]
    fn gradient_alpha_and_luminance_stops_differ() {
        let plan = UiMaskGradientPlan::from_gradient(
            &UiMaskGradient::Linear {
                angle_degrees: 90.0,
                stops: vec![
                    UiGradientStop {
                        offset: 0.0,
                        color: UiColorRgba8 {
                            red: 255,
                            green: 0,
                            blue: 0,
                            alpha: 255,
                        },
                    },
                    UiGradientStop {
                        offset: 1.0,
                        color: UiColorRgba8 {
                            red: 0,
                            green: 0,
                            blue: 0,
                            alpha: 0,
                        },
                    },
                ],
            },
            [100.0, 100.0],
        )
        .expect("gradient resolves");
        assert!((plan.stops[0].alpha_coverage - 1.0).abs() <= f32::EPSILON);
        assert!((plan.stops[0].luminance_coverage - 0.2126).abs() <= 0.0001);
    }

    #[test]
    fn element_mask_is_a_typed_capture_diagnostic() {
        let pass = UiMaskPassPlan::from_mask(
            0,
            &UiMask {
                image: UiMaskImage::Element(UiElementMaskSource {
                    element_id: "dialogue-mask".into(),
                }),
                ..UiMask::default()
            },
            UiMaskChannel::Alpha,
        );
        assert_eq!(
            pass.sampling_plan(UiTextureExtent::new(64, 64), UiTextureExtent::new(64, 64)),
            Err(UiMaskPlanError::ElementMaskCaptureUnavailable {
                element_id: "dialogue-mask".into(),
            })
        );
    }
}
