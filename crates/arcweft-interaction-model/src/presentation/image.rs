//! Host-neutral image registration data shared by artifacts and execution.
//!
//! This is inert presentation data. Program/catalog admission owns reference
//! validation; render layers and adapters do not define a parallel image shape.

use super::{ImageObjectAlignment, ImageObjectFit, ImageObjectPlayback, ImageObjectTransform};
use arcweft_layout::stage_placement::StagePlacement;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PresentationImage {
    pub id: String,
    pub asset: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containing_scroll_region: Option<String>,
    pub bounds: PresentationImageBounds,
    /// Authored placement contract. When absent, `bounds` is explicit absolute
    /// placement for image object data that has no responsive stage placement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<StagePlacement>,
    pub fit: ImageObjectFit,
    pub alignment: ImageObjectAlignment,
    pub playback: ImageObjectPlayback,
    pub transform: ImageObjectTransform,
    #[serde(default)]
    pub depth_milli: i32,
    #[serde(default = "default_opacity_milli")]
    pub opacity_milli: u16,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, PresentationImageParam>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proxies: Vec<PresentationImageProxy>,
    #[serde(default = "default_true")]
    pub visible: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PresentationImageParam {
    Bool { value: bool },
    Integer { value: i64 },
    Milli { value: i32 },
    Text { value: String },
    Id { value: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PresentationImageProxy {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth_milli: Option<i32>,
    #[serde(default)]
    pub hit_test: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, PresentationImageParam>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PresentationImageBounds {
    pub x_milli: i32,
    pub y_milli: i32,
    pub width_milli: u32,
    pub height_milli: u32,
}

/// Component that cannot be represented in the image's milli-unit storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationImageBoundsComponent {
    X,
    Y,
    Width,
    Height,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("image {0:?} pixel conversion exceeds milli-unit storage")]
pub struct PresentationImageBoundsError(pub PresentationImageBoundsComponent);

impl PresentationImageBounds {
    /// Converts exact pixel coordinates and extents without clamping overflow.
    pub const fn try_from_px(
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<Self, PresentationImageBoundsError> {
        let Some(x_milli) = x.checked_mul(1_000) else {
            return Err(PresentationImageBoundsError(
                PresentationImageBoundsComponent::X,
            ));
        };
        let Some(y_milli) = y.checked_mul(1_000) else {
            return Err(PresentationImageBoundsError(
                PresentationImageBoundsComponent::Y,
            ));
        };
        let Some(width_milli) = width.checked_mul(1_000) else {
            return Err(PresentationImageBoundsError(
                PresentationImageBoundsComponent::Width,
            ));
        };
        let Some(height_milli) = height.checked_mul(1_000) else {
            return Err(PresentationImageBoundsError(
                PresentationImageBoundsComponent::Height,
            ));
        };
        Ok(Self {
            x_milli,
            y_milli,
            width_milli,
            height_milli,
        })
    }
}

const fn default_opacity_milli() -> u16 {
    1_000
}
const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_conversion_preserves_negative_coordinates_and_zero_extents() {
        let bounds = PresentationImageBounds::try_from_px(-12, 34, 0, 56).unwrap();
        assert_eq!(
            bounds,
            PresentationImageBounds {
                x_milli: -12_000,
                y_milli: 34_000,
                width_milli: 0,
                height_milli: 56_000,
            }
        );
    }

    #[test]
    fn pixel_conversion_accepts_exact_storage_boundaries_and_rejects_each_overflow() {
        let min = i32::MIN / 1_000;
        let max = i32::MAX / 1_000;
        let extent = u32::MAX / 1_000;
        let bounds = PresentationImageBounds::try_from_px(min, max, extent, extent).unwrap();
        assert_eq!(bounds.x_milli, min * 1_000);
        assert_eq!(bounds.y_milli, max * 1_000);
        assert_eq!(bounds.width_milli, extent * 1_000);
        for (args, component) in [
            ((min - 1, 0, 0, 0), PresentationImageBoundsComponent::X),
            ((max + 1, 0, 0, 0), PresentationImageBoundsComponent::X),
            ((0, min - 1, 0, 0), PresentationImageBoundsComponent::Y),
            ((0, max + 1, 0, 0), PresentationImageBoundsComponent::Y),
            (
                (0, 0, extent + 1, 0),
                PresentationImageBoundsComponent::Width,
            ),
            (
                (0, 0, 0, extent + 1),
                PresentationImageBoundsComponent::Height,
            ),
        ] {
            assert_eq!(
                PresentationImageBounds::try_from_px(args.0, args.1, args.2, args.3),
                Err(PresentationImageBoundsError(component))
            );
        }
    }
}
