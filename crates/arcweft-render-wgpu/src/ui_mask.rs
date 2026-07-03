//! Mask composition planning for the UI compositor.

use crate::ui_effects::UiTextureExtent;
use crate::ui_scene::{UiMask, UiMaskImage, UiMaskPosition, UiMaskRepeat, UiMaskSize};
use num_traits::ToPrimitive;
use thiserror::Error;

/// How a mask texture contributes coverage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UiMaskChannel {
    #[default]
    Alpha,
    Luminance,
}

/// External resource requirement for one mask image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiMaskImagePlan {
    None,
    Texture { resource: Box<str> },
    Unsupported(Box<str>),
}

/// One mask pass after CSS mask fields are normalized by seq06.9a lowering.
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
    pub repeat_x: bool,
    pub repeat_y: bool,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum UiMaskPlanError {
    #[error("unsupported mask image: {0}")]
    UnsupportedImage(Box<str>),
    #[error("unsupported mask-size: {0}")]
    UnsupportedSize(Box<str>),
    #[error("unsupported mask-position: {0}")]
    UnsupportedPosition(Box<str>),
    #[error("unsupported mask-repeat: {0}")]
    UnsupportedRepeat(Box<str>),
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
        if let UiMaskImagePlan::Unsupported(reason) = &self.image {
            return Err(UiMaskPlanError::UnsupportedImage(reason.clone()));
        }
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
        let (repeat_x, repeat_y) = resolve_repeat(&self.repeat)?;
        Ok(UiMaskSamplingPlan {
            tile_size_px,
            tile_origin_px,
            repeat_x,
            repeat_y,
        })
    }

    pub fn is_unsupported(&self) -> bool {
        matches!(self.image, UiMaskImagePlan::Unsupported(_))
            || matches!(self.size, UiMaskSize::Unsupported(_))
            || matches!(
                self.repeat,
                UiMaskRepeat::Space | UiMaskRepeat::Round | UiMaskRepeat::Unsupported(_)
            )
    }
}

impl UiMaskImagePlan {
    pub fn from_image(image: &UiMaskImage) -> Self {
        match image {
            UiMaskImage::None => Self::None,
            UiMaskImage::Url(resource) => Self::Texture {
                resource: resource.clone(),
            },
            UiMaskImage::Unsupported(reason) => Self::Unsupported(reason.clone()),
        }
    }
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

fn resolve_repeat(repeat: &UiMaskRepeat) -> Result<(bool, bool), UiMaskPlanError> {
    match repeat {
        UiMaskRepeat::Unspecified | UiMaskRepeat::Repeat => Ok((true, true)),
        UiMaskRepeat::NoRepeat => Ok((false, false)),
        UiMaskRepeat::RepeatX => Ok((true, false)),
        UiMaskRepeat::RepeatY => Ok((false, true)),
        UiMaskRepeat::Space => Err(UiMaskPlanError::UnsupportedRepeat("space".into())),
        UiMaskRepeat::Round => Err(UiMaskPlanError::UnsupportedRepeat("round".into())),
        UiMaskRepeat::Unsupported(reason) => {
            Err(UiMaskPlanError::UnsupportedRepeat(reason.clone()))
        }
    }
}

fn dimension_to_f32(value: u32) -> f32 {
    value.max(1).to_f32().unwrap_or(f32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn unsupported_mask_values_are_not_silent() {
        let masks = [UiMask {
            image: UiMaskImage::Unsupported("paint worklet".into()),
            ..UiMask::default()
        }];

        let plan = UiMaskChainPlan::from_masks(&masks, UiMaskChannel::Luminance);

        assert_eq!(plan.unsupported_count(), 1);
        assert_eq!(plan.passes()[0].channel, UiMaskChannel::Luminance);
    }
}
