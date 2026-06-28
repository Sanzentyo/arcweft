//! Mask composition planning for the UI compositor.

use crate::ui_scene::{UiMask, UiMaskImage, UiMaskPosition, UiMaskRepeat, UiMaskSize};

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

    pub fn is_unsupported(&self) -> bool {
        matches!(self.image, UiMaskImagePlan::Unsupported(_))
            || matches!(self.size, UiMaskSize::Unsupported(_))
            || matches!(self.repeat, UiMaskRepeat::Unsupported(_))
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
