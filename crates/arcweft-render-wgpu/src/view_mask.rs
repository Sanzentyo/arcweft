//! Mask composition planning for the View compositor.

use crate::view_effects::ViewTextureExtent;
use crate::view_scene::{
    ViewElementMaskSource, ViewGradientStop, ViewLength, ViewMask, ViewMaskGradient, ViewMaskImage,
    ViewMaskPosition, ViewMaskRepeat, ViewMaskSize, ViewPoint,
};
use num_traits::ToPrimitive;
use thiserror::Error;

pub const MAX_MASK_GRADIENT_STOPS: usize = 8;

/// How a mask source contributes coverage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewMaskChannel {
    #[default]
    Alpha,
    Luminance,
}

/// Per-axis mask tile distribution mode after typed repeat normalization.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewMaskAxisRepeat {
    NoRepeat,
    #[default]
    Repeat,
    Space,
    Round,
}

/// Renderer-facing source requirement for one mask image.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewMaskImagePlan {
    None,
    Texture { resource: Box<str> },
    Gradient(ViewMaskGradient),
    Element(ViewElementMaskSource),
    Unsupported(Box<str>),
}

/// One mask pass after native mask fields are normalized for rendering.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewMaskPassPlan {
    pub mask_index: usize,
    pub image: ViewMaskImagePlan,
    pub size: ViewMaskSize,
    pub position: ViewMaskPosition,
    pub repeat: ViewMaskRepeat,
    pub channel: ViewMaskChannel,
}

/// Pixel-space sampling contract for one mask pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewMaskSamplingPlan {
    pub tile_size_px: [f32; 2],
    pub tile_origin_px: [f32; 2],
    pub tile_stride_px: [f32; 2],
    pub tile_count: [u32; 2],
    pub repeat_mode_x: ViewMaskAxisRepeat,
    pub repeat_mode_y: ViewMaskAxisRepeat,
    /// Convenience compatibility field for callers that only need old repeat/no-repeat behavior.
    pub repeat_x: bool,
    /// Convenience compatibility field for callers that only need old repeat/no-repeat behavior.
    pub repeat_y: bool,
}

/// Packed gradient coverage plan consumed by the compositor uniform contract.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewMaskGradientPlan {
    pub kind: ViewMaskGradientKind,
    pub stops: Vec<ViewMaskGradientStopPlan>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewMaskGradientKind {
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
pub struct ViewMaskGradientStopPlan {
    pub offset: f32,
    pub alpha_coverage: f32,
    pub luminance_coverage: f32,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum ViewMaskPlanError {
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
pub struct ViewMaskChainPlan {
    passes: Vec<ViewMaskPassPlan>,
}

impl ViewMaskChainPlan {
    pub fn from_masks(masks: &[ViewMask], default_channel: ViewMaskChannel) -> Self {
        Self {
            passes: masks
                .iter()
                .enumerate()
                .map(|(mask_index, mask)| {
                    ViewMaskPassPlan::from_mask(mask_index, mask, default_channel)
                })
                .collect(),
        }
    }

    pub fn passes(&self) -> &[ViewMaskPassPlan] {
        &self.passes
    }

    pub fn is_empty(&self) -> bool {
        self.passes.is_empty()
    }

    pub fn requires_external_texture(&self) -> bool {
        self.passes
            .iter()
            .any(ViewMaskPassPlan::requires_external_texture)
    }

    pub fn unsupported_count(&self) -> usize {
        self.passes
            .iter()
            .filter(|pass| pass.is_unsupported())
            .count()
    }
}

impl ViewMaskPassPlan {
    pub fn from_mask(mask_index: usize, mask: &ViewMask, channel: ViewMaskChannel) -> Self {
        Self {
            mask_index,
            image: ViewMaskImagePlan::from_image(&mask.image),
            size: mask.size.clone(),
            position: mask.position.clone(),
            repeat: mask.repeat.clone(),
            channel,
        }
    }

    pub fn requires_external_texture(&self) -> bool {
        matches!(self.image, ViewMaskImagePlan::Texture { .. })
    }

    pub fn sampling_plan(
        &self,
        source_extent: ViewTextureExtent,
        mask_extent: ViewTextureExtent,
    ) -> Result<ViewMaskSamplingPlan, ViewMaskPlanError> {
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
    ) -> Result<Option<ViewMaskGradientPlan>, ViewMaskPlanError> {
        match &self.image {
            ViewMaskImagePlan::Gradient(gradient) => {
                ViewMaskGradientPlan::from_gradient(gradient, tile_size_px).map(Some)
            }
            ViewMaskImagePlan::Element(source) => {
                Err(ViewMaskPlanError::ElementMaskCaptureUnavailable {
                    element_id: source.element_id.clone(),
                })
            }
            ViewMaskImagePlan::Unsupported(reason) => {
                Err(ViewMaskPlanError::UnsupportedImage(reason.clone()))
            }
            ViewMaskImagePlan::None | ViewMaskImagePlan::Texture { .. } => Ok(None),
        }
    }

    pub fn is_unsupported(&self) -> bool {
        matches!(self.image, ViewMaskImagePlan::Unsupported(_))
            || matches!(self.image, ViewMaskImagePlan::Element(_))
            || matches!(self.size, ViewMaskSize::Unsupported(_))
            || matches!(self.repeat, ViewMaskRepeat::Unsupported(_))
    }

    fn validate_image_for_sampling(&self) -> Result<(), ViewMaskPlanError> {
        match &self.image {
            ViewMaskImagePlan::Unsupported(reason) => {
                Err(ViewMaskPlanError::UnsupportedImage(reason.clone()))
            }
            ViewMaskImagePlan::Element(source) => {
                Err(ViewMaskPlanError::ElementMaskCaptureUnavailable {
                    element_id: source.element_id.clone(),
                })
            }
            ViewMaskImagePlan::Gradient(ViewMaskGradient::Unsupported(reason)) => {
                Err(ViewMaskPlanError::UnsupportedGradient {
                    reason: reason.clone(),
                })
            }
            ViewMaskImagePlan::None
            | ViewMaskImagePlan::Texture { .. }
            | ViewMaskImagePlan::Gradient(_) => Ok(()),
        }
    }
}

impl ViewMaskImagePlan {
    pub fn from_image(image: &ViewMaskImage) -> Self {
        match image {
            ViewMaskImage::None => Self::None,
            ViewMaskImage::Url(resource) => Self::Texture {
                resource: resource.clone(),
            },
            ViewMaskImage::Gradient(ViewMaskGradient::Unsupported(reason)) => {
                Self::Unsupported(reason.clone())
            }
            ViewMaskImage::Gradient(gradient) => Self::Gradient(gradient.clone()),
            ViewMaskImage::Element(source) => Self::Element(source.clone()),
            ViewMaskImage::Unsupported(reason) => Self::Unsupported(reason.clone()),
        }
    }
}

impl ViewMaskGradientPlan {
    pub fn from_gradient(
        gradient: &ViewMaskGradient,
        tile_size_px: [f32; 2],
    ) -> Result<Self, ViewMaskPlanError> {
        match gradient {
            ViewMaskGradient::Linear {
                angle_degrees,
                stops,
            } => Ok(Self {
                kind: ViewMaskGradientKind::Linear {
                    angle_degrees: *angle_degrees,
                },
                stops: canonical_gradient_stops(stops)?,
            }),
            ViewMaskGradient::Radial {
                center,
                radius_x,
                radius_y,
                stops,
            } => Ok(Self {
                kind: ViewMaskGradientKind::Radial {
                    center_px: resolve_point_px(center, tile_size_px)?,
                    radius_px: [
                        resolve_length_px(radius_x, tile_size_px[0], "radial-radius-x")?,
                        resolve_length_px(radius_y, tile_size_px[1], "radial-radius-y")?,
                    ],
                },
                stops: canonical_gradient_stops(stops)?,
            }),
            ViewMaskGradient::Conic {
                center,
                from_degrees,
                stops,
            } => Ok(Self {
                kind: ViewMaskGradientKind::Conic {
                    center_px: resolve_point_px(center, tile_size_px)?,
                    from_degrees: *from_degrees,
                },
                stops: canonical_gradient_stops(stops)?,
            }),
            ViewMaskGradient::Unsupported(reason) => Err(ViewMaskPlanError::UnsupportedGradient {
                reason: reason.clone(),
            }),
        }
    }
}

fn resolve_sampling_plan(
    source_size_px: [f32; 2],
    tile_size_px: [f32; 2],
    tile_origin_px: [f32; 2],
    repeat_modes: [ViewMaskAxisRepeat; 2],
) -> ViewMaskSamplingPlan {
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
    ViewMaskSamplingPlan {
        tile_size_px: [x.tile_size_px, y.tile_size_px],
        tile_origin_px: [x.origin_px, y.origin_px],
        tile_stride_px: [x.stride_px, y.stride_px],
        tile_count: [x.tile_count, y.tile_count],
        repeat_mode_x: repeat_modes[0],
        repeat_mode_y: repeat_modes[1],
        repeat_x: repeat_modes[0] != ViewMaskAxisRepeat::NoRepeat,
        repeat_y: repeat_modes[1] != ViewMaskAxisRepeat::NoRepeat,
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
    repeat: ViewMaskAxisRepeat,
) -> AxisSampling {
    let source_size_px = source_size_px.max(1.0);
    let tile_size_px = tile_size_px.max(1.0);
    match repeat {
        ViewMaskAxisRepeat::NoRepeat => AxisSampling {
            tile_size_px,
            origin_px,
            stride_px: tile_size_px,
            tile_count: 1,
        },
        ViewMaskAxisRepeat::Repeat => AxisSampling {
            tile_size_px,
            origin_px,
            stride_px: tile_size_px,
            tile_count: 0,
        },
        ViewMaskAxisRepeat::Space => {
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
        ViewMaskAxisRepeat::Round => {
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
    size: &ViewMaskSize,
    source_width: f32,
    source_height: f32,
    mask_width: f32,
    mask_height: f32,
) -> Result<[f32; 2], ViewMaskPlanError> {
    match size {
        ViewMaskSize::Unspecified | ViewMaskSize::Auto => Ok([mask_width, mask_height]),
        ViewMaskSize::Cover => {
            let scale = (source_width / mask_width)
                .max(source_height / mask_height)
                .max(0.0);
            Ok([mask_width * scale, mask_height * scale])
        }
        ViewMaskSize::Contain => {
            let scale = (source_width / mask_width)
                .min(source_height / mask_height)
                .max(0.0);
            Ok([mask_width * scale, mask_height * scale])
        }
        ViewMaskSize::Explicit { width, height } => {
            let width = width
                .resolve_px(source_width)
                .ok_or_else(|| ViewMaskPlanError::UnsupportedSize("width".into()))?;
            let height = height
                .resolve_px(source_height)
                .ok_or_else(|| ViewMaskPlanError::UnsupportedSize("height".into()))?;
            Ok([width.max(1.0), height.max(1.0)])
        }
        ViewMaskSize::Unsupported(reason) => {
            Err(ViewMaskPlanError::UnsupportedSize(reason.clone()))
        }
    }
}

fn resolve_position(
    position: &ViewMaskPosition,
    source_width: f32,
    source_height: f32,
    tile_size_px: [f32; 2],
) -> Result<[f32; 2], ViewMaskPlanError> {
    let available_width = (source_width - tile_size_px[0]).max(0.0);
    let available_height = (source_height - tile_size_px[1]).max(0.0);
    let x = position
        .anchor
        .x
        .resolve_px(available_width)
        .ok_or_else(|| ViewMaskPlanError::UnsupportedPosition("x".into()))?;
    let y = position
        .anchor
        .y
        .resolve_px(available_height)
        .ok_or_else(|| ViewMaskPlanError::UnsupportedPosition("y".into()))?;
    Ok([x, y])
}

fn repeat_modes(
    repeat: &ViewMaskRepeat,
) -> Result<(ViewMaskAxisRepeat, ViewMaskAxisRepeat), ViewMaskPlanError> {
    match repeat {
        ViewMaskRepeat::Unspecified | ViewMaskRepeat::Repeat => {
            Ok((ViewMaskAxisRepeat::Repeat, ViewMaskAxisRepeat::Repeat))
        }
        ViewMaskRepeat::NoRepeat => {
            Ok((ViewMaskAxisRepeat::NoRepeat, ViewMaskAxisRepeat::NoRepeat))
        }
        ViewMaskRepeat::RepeatX => Ok((ViewMaskAxisRepeat::Repeat, ViewMaskAxisRepeat::NoRepeat)),
        ViewMaskRepeat::RepeatY => Ok((ViewMaskAxisRepeat::NoRepeat, ViewMaskAxisRepeat::Repeat)),
        ViewMaskRepeat::Space => Ok((ViewMaskAxisRepeat::Space, ViewMaskAxisRepeat::Space)),
        ViewMaskRepeat::Round => Ok((ViewMaskAxisRepeat::Round, ViewMaskAxisRepeat::Round)),
        ViewMaskRepeat::Unsupported(reason) => {
            Err(ViewMaskPlanError::UnsupportedRepeat(reason.clone()))
        }
    }
}

fn canonical_gradient_stops(
    stops: &[ViewGradientStop],
) -> Result<Vec<ViewMaskGradientStopPlan>, ViewMaskPlanError> {
    if stops.len() < 2 {
        return Err(ViewMaskPlanError::InvalidGradientStopCount { count: stops.len() });
    }
    if stops.len() > MAX_MASK_GRADIENT_STOPS {
        return Err(ViewMaskPlanError::TooManyGradientStops {
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
        result.push(ViewMaskGradientStopPlan {
            offset,
            alpha_coverage: alpha,
            luminance_coverage: luminance * alpha,
        });
    }
    Ok(result)
}

fn resolve_point_px(
    point: &ViewPoint,
    tile_size_px: [f32; 2],
) -> Result<[f32; 2], ViewMaskPlanError> {
    Ok([
        resolve_length_px(&point.x, tile_size_px[0], "gradient-center-x")?,
        resolve_length_px(&point.y, tile_size_px[1], "gradient-center-y")?,
    ])
}

fn resolve_length_px(
    length: &ViewLength,
    basis_px: f32,
    role: &'static str,
) -> Result<f32, ViewMaskPlanError> {
    length
        .resolve_px(basis_px)
        .map(|value| value.max(0.0))
        .ok_or_else(|| ViewMaskPlanError::UnsupportedGradient {
            reason: role.into(),
        })
}

fn dimension_to_f32(value: u32) -> f32 {
    value.max(1).to_f32().unwrap_or(f32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_scene::{ViewColorRgba8, ViewMaskGradient};

    #[test]
    fn url_masks_require_external_textures() {
        let masks = [ViewMask {
            image: ViewMaskImage::Url("arcweft://mask/card".into()),
            ..ViewMask::default()
        }];
        let plan = ViewMaskChainPlan::from_masks(&masks, ViewMaskChannel::Alpha);
        assert!(plan.requires_external_texture());
        assert_eq!(plan.unsupported_count(), 0);
    }

    #[test]
    fn space_and_round_repeat_resolve_deterministically() {
        let space = ViewMaskPassPlan::from_mask(
            0,
            &ViewMask {
                image: ViewMaskImage::Url("arcweft://mask/space".into()),
                size: ViewMaskSize::Explicit {
                    width: ViewLength::Px(30.0),
                    height: ViewLength::Px(20.0),
                },
                repeat: ViewMaskRepeat::Space,
                ..ViewMask::default()
            },
            ViewMaskChannel::Alpha,
        );
        let sampling = space
            .sampling_plan(
                ViewTextureExtent::new(100, 60),
                ViewTextureExtent::new(10, 10),
            )
            .expect("space repeat resolves");
        assert_eq!(sampling.repeat_mode_x, ViewMaskAxisRepeat::Space);
        assert_eq!(sampling.tile_count[0], 3);
        assert!((sampling.tile_stride_px[0] - 35.0).abs() <= 0.001);

        let round = ViewMaskPassPlan::from_mask(
            0,
            &ViewMask {
                image: ViewMaskImage::Url("arcweft://mask/round".into()),
                size: ViewMaskSize::Explicit {
                    width: ViewLength::Px(30.0),
                    height: ViewLength::Px(20.0),
                },
                repeat: ViewMaskRepeat::Round,
                ..ViewMask::default()
            },
            ViewMaskChannel::Alpha,
        );
        let sampling = round
            .sampling_plan(
                ViewTextureExtent::new(100, 60),
                ViewTextureExtent::new(10, 10),
            )
            .expect("round repeat resolves");
        assert_eq!(sampling.tile_count[0], 3);
        assert!((sampling.tile_size_px[0] - 33.333).abs() <= 0.01);
    }

    #[test]
    fn gradient_alpha_and_luminance_stops_differ() {
        let plan = ViewMaskGradientPlan::from_gradient(
            &ViewMaskGradient::Linear {
                angle_degrees: 90.0,
                stops: vec![
                    ViewGradientStop {
                        offset: 0.0,
                        color: ViewColorRgba8 {
                            red: 255,
                            green: 0,
                            blue: 0,
                            alpha: 255,
                        },
                    },
                    ViewGradientStop {
                        offset: 1.0,
                        color: ViewColorRgba8 {
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
        let pass = ViewMaskPassPlan::from_mask(
            0,
            &ViewMask {
                image: ViewMaskImage::Element(ViewElementMaskSource {
                    element_id: "dialogue-mask".into(),
                }),
                ..ViewMask::default()
            },
            ViewMaskChannel::Alpha,
        );
        assert_eq!(
            pass.sampling_plan(
                ViewTextureExtent::new(64, 64),
                ViewTextureExtent::new(64, 64)
            ),
            Err(ViewMaskPlanError::ElementMaskCaptureUnavailable {
                element_id: "dialogue-mask".into(),
            })
        );
    }
}
