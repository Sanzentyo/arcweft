//! Runtime-facing projection of one current native computed Style snapshot.

use arcweft_presentation::appearance::{
    PresentationColor, PresentationEnvironment, SystemPaletteSet,
};
use arcweft_view::style::{
    ComputedViewStyle, ViewPropertyKind, ViewSpecifiedValue, ViewStyleValueKind,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

mod projection;

/// One current computed node snapshot partitioned by its downstream owner.
///
/// Every computed property is retained in exactly one typed partition. The
/// control visual is a derived renderer packet, not a second cascade model.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewRuntimeNodeStyle {
    layout: ViewRuntimeStyleProperties,
    text: ViewRuntimeStyleProperties,
    paint: ViewRuntimeStyleProperties,
    composite: ViewRuntimeStyleProperties,
    transition: ViewRuntimeStyleProperties,
    visual: ViewRuntimeControlVisualStyle,
}

/// Typed properties owned by one runtime application tier.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewRuntimeStyleProperties(BTreeMap<ViewPropertyKind, ViewSpecifiedValue>);

/// Failure to project a canonical computed snapshot without losing information.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewRuntimeStyleProjectionError {
    #[error("computed Style property {property:?} expects {expected:?}, found {actual:?}")]
    ValueKindMismatch {
        property: ViewPropertyKind,
        expected: ViewStyleValueKind,
        actual: ViewStyleValueKind,
    },
}

/// Existing renderer-facing paint/text packet for the current state only.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeControlVisualStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<PresentationColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<PresentationColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<PresentationColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<PresentationColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caret: Option<PresentationColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composition_underline: Option<PresentationColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size_milli: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height_milli: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub letter_spacing_milli: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<ViewRuntimeControlBorderStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner_frame: Option<ViewRuntimeControlCornerFrameStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_ring: Option<ViewRuntimeControlFocusRingStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity_milli: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius_milli: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radii_milli: Option<ViewRuntimeControlRadii>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth_milli: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filters: Option<ViewRuntimeControlFilterList>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backdrop_filters: Option<ViewRuntimeControlFilterList>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shadows: Vec<ViewRuntimeShadow>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeControlBorderStyle {
    pub color: PresentationColor,
    pub width_milli: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeControlCornerFrameStyle {
    pub color: PresentationColor,
    pub width_milli: u32,
    pub length_milli: u32,
    pub offset_milli: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeControlFocusRingStyle {
    pub color: PresentationColor,
    pub width_milli: u32,
    pub offset_milli: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeControlRadii {
    pub top_left: ViewRuntimeControlCornerRadius,
    pub top_right: ViewRuntimeControlCornerRadius,
    pub bottom_right: ViewRuntimeControlCornerRadius,
    pub bottom_left: ViewRuntimeControlCornerRadius,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeControlCornerRadius {
    pub x_milli: u32,
    pub y_milli: u32,
}

impl ViewRuntimeControlRadii {
    pub const fn uniform(radius_milli: u32) -> Self {
        let radius = ViewRuntimeControlCornerRadius::circular(radius_milli);
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    pub const fn new(
        top_left: ViewRuntimeControlCornerRadius,
        top_right: ViewRuntimeControlCornerRadius,
        bottom_right: ViewRuntimeControlCornerRadius,
        bottom_left: ViewRuntimeControlCornerRadius,
    ) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }
}

impl ViewRuntimeControlCornerRadius {
    pub const fn circular(radius_milli: u32) -> Self {
        Self {
            x_milli: radius_milli,
            y_milli: radius_milli,
        }
    }

    pub const fn new(x_milli: u32, y_milli: u32) -> Self {
        Self { x_milli, y_milli }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeShadow {
    pub offset_x_milli: i32,
    pub offset_y_milli: i32,
    pub blur_milli: u32,
    pub spread_milli: i32,
    pub radius_milli: u32,
    pub color: PresentationColor,
    pub kind: ViewRuntimeShadowKind,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewRuntimeShadowKind {
    #[default]
    Outer,
    Inset,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeControlFilterList {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<ViewRuntimeControlFilter>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ViewRuntimeControlFilter {
    Brightness { factor_milli: u32 },
    Contrast { factor_milli: u32 },
    Grayscale { amount_milli: u16 },
    Saturate { factor_milli: u32 },
    HueRotate { degrees_milli: i32 },
    Invert { amount_milli: u16 },
    Sepia { amount_milli: u16 },
    Opacity { amount_milli: u16 },
    Blur { radius_milli: u32 },
}

impl ViewRuntimeNodeStyle {
    pub fn try_from_computed(
        computed: &ComputedViewStyle,
        environment: &PresentationEnvironment,
        palettes: &SystemPaletteSet,
    ) -> Result<Self, ViewRuntimeStyleProjectionError> {
        projection::project_computed_style(computed, environment, palettes)
    }

    pub const fn layout(&self) -> &ViewRuntimeStyleProperties {
        &self.layout
    }

    pub const fn text(&self) -> &ViewRuntimeStyleProperties {
        &self.text
    }

    pub const fn paint(&self) -> &ViewRuntimeStyleProperties {
        &self.paint
    }

    pub const fn composite(&self) -> &ViewRuntimeStyleProperties {
        &self.composite
    }

    pub const fn transition(&self) -> &ViewRuntimeStyleProperties {
        &self.transition
    }

    pub const fn visual(&self) -> &ViewRuntimeControlVisualStyle {
        &self.visual
    }

    pub fn into_visual(self) -> ViewRuntimeControlVisualStyle {
        self.visual
    }
}

impl ViewRuntimeStyleProperties {
    pub fn value(&self, property: ViewPropertyKind) -> Option<&ViewSpecifiedValue> {
        self.0.get(&property)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (ViewPropertyKind, &ViewSpecifiedValue)> {
        self.0.iter().map(|(property, value)| (*property, value))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(super) fn insert(&mut self, property: ViewPropertyKind, value: ViewSpecifiedValue) {
        self.0.insert(property, value);
    }
}

impl ViewRuntimeControlVisualStyle {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}
