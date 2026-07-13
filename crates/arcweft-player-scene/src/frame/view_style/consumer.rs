//! Runtime consumer support and lossless computed-Style projection.

use super::super::PlayerFrameError;
use super::{NodeBinding, ResolvedViewStyleFrame, StyleTargetKind};
use arcweft_bundle::BundleImageObject;
use arcweft_bundle::resource_codec::{
    ViewRuntimeActionButton, ViewRuntimeNodeStyle, ViewRuntimeScrollRegion,
    ViewRuntimeStyleProjectionError, ViewRuntimeSurface, ViewRuntimeTextControl,
};
use arcweft_runtime_driver::display::BundlePresentationSnapshot;
use arcweft_runtime_driver::view_runtime::{
    BundleViewMountOutput, BundleViewStyleNode, BundleViewStyleNodeKind,
};
use arcweft_view::ViewElementKind;
use arcweft_view::style::{
    ComputedViewStyle, ViewOverflow, ViewPropertyKind, ViewSpecifiedValue,
    ViewStyleContributionSource,
};

/// Presentation resources with the current live Style snapshot applied.
pub(in crate::frame) struct StyledViewResources {
    pub(in crate::frame) text_inputs: Vec<ViewRuntimeTextControl>,
    pub(in crate::frame) action_buttons: Vec<ViewRuntimeActionButton>,
    pub(in crate::frame) scroll_regions: Vec<ViewRuntimeScrollRegion>,
    pub(in crate::frame) surfaces: Vec<ViewRuntimeSurface>,
    pub(in crate::frame) images: Vec<BundleImageObject>,
}

impl ResolvedViewStyleFrame {
    pub(in crate::frame) fn apply_to_presentation(
        &self,
        presentation: &BundlePresentationSnapshot,
    ) -> StyledViewResources {
        let mut resources = StyledViewResources {
            text_inputs: presentation
                .text_inputs
                .iter()
                .cloned()
                .map(|mut control| {
                    if let Some(style) = self.control(&control.target) {
                        control.style = style.visual().clone();
                        apply_text_control_box(&mut control, style);
                    }
                    control
                })
                .collect(),
            action_buttons: presentation
                .action_buttons
                .iter()
                .cloned()
                .map(|mut button| {
                    if let Some(style) = self.control(&button.target) {
                        button.style = style.visual().clone();
                        apply_action_button_box(&mut button, style);
                    }
                    button
                })
                .collect(),
            scroll_regions: presentation
                .scroll_regions
                .iter()
                .cloned()
                .map(|mut region| {
                    if let Some(style) = self.control(&region.target) {
                        apply_scroll_region_box(&mut region, style);
                    }
                    region
                })
                .collect(),
            surfaces: presentation
                .surfaces
                .iter()
                .cloned()
                .map(|mut surface| {
                    if let Some(style) = self
                        .control(&surface.target)
                        .or_else(|| self.part(&surface.public_id))
                    {
                        surface.style = style.visual().clone();
                        apply_surface_box(&mut surface, style);
                    }
                    surface
                })
                .collect(),
            images: presentation
                .images
                .iter()
                .cloned()
                .map(|mut image| {
                    let style = image
                        .target
                        .as_deref()
                        .and_then(|target| self.image(target))
                        .or_else(|| self.image(&image.id));
                    if let Some(style) = style {
                        apply_image_box(&mut image, style);
                    }
                    image
                })
                .collect(),
        };
        self.apply_layout_offsets(&mut resources);
        resources
    }

    fn apply_layout_offsets(&self, resources: &mut StyledViewResources) {
        for control in &mut resources.text_inputs {
            let (x, y) = self
                .layout_offset(StyleTargetKind::Control, &control.target)
                .unwrap_or_default();
            control.bounds.x_milli = control.bounds.x_milli.saturating_add(x);
            control.bounds.y_milli = control.bounds.y_milli.saturating_add(y);
        }
        for button in &mut resources.action_buttons {
            let (x, y) = self
                .layout_offset(StyleTargetKind::Control, &button.target)
                .unwrap_or_default();
            button.bounds.x_milli = button.bounds.x_milli.saturating_add(x);
            button.bounds.y_milli = button.bounds.y_milli.saturating_add(y);
        }
        for region in &mut resources.scroll_regions {
            let (x, y) = self
                .layout_offset(StyleTargetKind::Control, &region.target)
                .unwrap_or_default();
            region.bounds.x_milli = region.bounds.x_milli.saturating_add(x);
            region.bounds.y_milli = region.bounds.y_milli.saturating_add(y);
        }
        for surface in &mut resources.surfaces {
            let (x, y) = self
                .layout_offset(StyleTargetKind::Control, &surface.target)
                .or_else(|| self.layout_offset(StyleTargetKind::Part, &surface.public_id))
                .unwrap_or_default();
            surface.bounds.x_milli = surface.bounds.x_milli.saturating_add(x);
            surface.bounds.y_milli = surface.bounds.y_milli.saturating_add(y);
        }
        for image in &mut resources.images {
            let (x, y) = image
                .target
                .as_deref()
                .and_then(|target| self.layout_offset(StyleTargetKind::Image, target))
                .or_else(|| self.layout_offset(StyleTargetKind::Image, &image.id))
                .unwrap_or_default();
            image.bounds.x_milli = image.bounds.x_milli.saturating_add(x);
            image.bounds.y_milli = image.bounds.y_milli.saturating_add(y);
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum StyleConsumer {
    Structural(ViewElementKind),
    Surface(ViewElementKind),
    Scroll,
    Control,
    Text,
    Image,
    Boundary,
}

impl StyleConsumer {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Structural(_) => "structural layout",
            Self::Surface(_) => "surface",
            Self::Scroll => "scroll viewport",
            Self::Control => "control",
            Self::Text => "text",
            Self::Image => "image",
            Self::Boundary => "View boundary",
        }
    }

    const fn passes_through_unconsumed_inheritance(self) -> bool {
        matches!(
            self,
            Self::Structural(_) | Self::Surface(_) | Self::Scroll | Self::Image | Self::Boundary
        )
    }
}

pub(super) fn validate_supported_properties(
    presentation: &BundlePresentationSnapshot,
    mount: &BundleViewMountOutput,
    node: &BundleViewStyleNode,
    bindings: &[NodeBinding],
    computed: &ComputedViewStyle,
) -> Result<(), PlayerFrameError> {
    let consumer = match &node.kind {
        BundleViewStyleNodeKind::Element { element, target } => {
            let scoped_target = target.as_deref().map(|target| mount.scoped_id(target));
            let scoped_part = node.part.as_deref().map(|part| mount.scoped_id(part));
            let has_surface = presentation.surfaces.iter().any(|surface| {
                scoped_target
                    .as_deref()
                    .is_some_and(|target| surface.target == target)
                    || scoped_part
                        .as_deref()
                        .is_some_and(|part| surface.public_id == part || surface.target == part)
            });
            let has_scroll = scoped_target.as_deref().is_some_and(|target| {
                presentation
                    .scroll_regions
                    .iter()
                    .any(|region| region.target == target)
            });
            let has_control = scoped_target.as_deref().is_some_and(|target| {
                presentation
                    .text_inputs
                    .iter()
                    .any(|control| control.target == target)
                    || presentation
                        .action_buttons
                        .iter()
                        .any(|button| button.target == target)
            });
            match element {
                ViewElementKind::Button
                | ViewElementKind::TextField
                | ViewElementKind::TextArea
                | ViewElementKind::SecureField
                    if has_control =>
                {
                    StyleConsumer::Control
                }
                ViewElementKind::Panel | ViewElementKind::Box if has_surface => {
                    StyleConsumer::Surface(*element)
                }
                ViewElementKind::Scroll if has_scroll => StyleConsumer::Scroll,
                element => StyleConsumer::Structural(*element),
            }
        }
        BundleViewStyleNodeKind::Text { .. } if !bindings.is_empty() => StyleConsumer::Text,
        BundleViewStyleNodeKind::Image { .. }
            if bindings
                .iter()
                .flat_map(|binding| &binding.keys)
                .any(|key| {
                    key.kind == StyleTargetKind::Image
                        && presentation.images.iter().any(|image| {
                            image.id == key.id || image.target.as_deref() == Some(key.id.as_str())
                        })
                }) =>
        {
            StyleConsumer::Image
        }
        BundleViewStyleNodeKind::Text { .. }
        | BundleViewStyleNodeKind::Image { .. }
        | BundleViewStyleNodeKind::Custom { .. }
        | BundleViewStyleNodeKind::CallView { .. } => StyleConsumer::Boundary,
    };
    validate_consumer_properties(mount, node, consumer, computed)
}

pub(super) fn validate_consumer_properties(
    mount: &BundleViewMountOutput,
    node: &BundleViewStyleNode,
    consumer: StyleConsumer,
    computed: &ComputedViewStyle,
) -> Result<(), PlayerFrameError> {
    if let Some((property, _)) = computed.properties().find(|(property, value)| {
        !(consumer_supports(consumer, *property)
            || matches!(value.source(), ViewStyleContributionSource::Inherited)
                && consumer.passes_through_unconsumed_inheritance())
    }) {
        return Err(PlayerFrameError::UnsupportedStyleProperty {
            mount: mount.mount.get(),
            instruction: node.instruction,
            target: consumer.label(),
            property,
        });
    }
    Ok(())
}

const fn consumer_supports(consumer: StyleConsumer, property: ViewPropertyKind) -> bool {
    match consumer {
        StyleConsumer::Structural(element) => {
            property.is_inherited() || container_gap_property(element, property)
        }
        StyleConsumer::Surface(element) => {
            property.is_inherited()
                || container_gap_property(element, property)
                || surface_box_property(property)
                || surface_visual_property(property)
        }
        StyleConsumer::Scroll => property.is_inherited() || scroll_box_property(property),
        StyleConsumer::Control => {
            control_box_property(property)
                || surface_visual_property(property)
                || control_adornment_property(property)
                || control_text_visual_property(property)
        }
        StyleConsumer::Text => text_box_property(property) || static_text_visual_property(property),
        StyleConsumer::Image => image_box_property(property) || image_visual_property(property),
        StyleConsumer::Boundary => property.is_inherited(),
    }
}

const fn scroll_box_property(property: ViewPropertyKind) -> bool {
    matches!(
        property,
        ViewPropertyKind::Width
            | ViewPropertyKind::Height
            | ViewPropertyKind::InlineSize
            | ViewPropertyKind::BlockSize
            | ViewPropertyKind::TranslateX
            | ViewPropertyKind::TranslateY
            | ViewPropertyKind::TranslateInline
            | ViewPropertyKind::TranslateBlock
            | ViewPropertyKind::Scale
    )
}

const fn container_gap_property(element: ViewElementKind, property: ViewPropertyKind) -> bool {
    matches!(element, ViewElementKind::Column | ViewElementKind::Row)
        && matches!(
            property,
            ViewPropertyKind::Gap | ViewPropertyKind::RowGap | ViewPropertyKind::ColumnGap
        )
}

const fn box_geometry_property(property: ViewPropertyKind) -> bool {
    matches!(
        property,
        ViewPropertyKind::Width
            | ViewPropertyKind::Height
            | ViewPropertyKind::InlineSize
            | ViewPropertyKind::BlockSize
            | ViewPropertyKind::TranslateX
            | ViewPropertyKind::TranslateY
            | ViewPropertyKind::TranslateInline
            | ViewPropertyKind::TranslateBlock
            | ViewPropertyKind::Scale
    )
}

const fn surface_box_property(property: ViewPropertyKind) -> bool {
    box_geometry_property(property)
        || matches!(
            property,
            ViewPropertyKind::ZIndex
                | ViewPropertyKind::Overflow
                | ViewPropertyKind::OverflowX
                | ViewPropertyKind::OverflowY
                | ViewPropertyKind::OverflowInline
                | ViewPropertyKind::OverflowBlock
        )
}

const fn control_box_property(property: ViewPropertyKind) -> bool {
    box_geometry_property(property) || matches!(property, ViewPropertyKind::ZIndex)
}

const fn text_box_property(property: ViewPropertyKind) -> bool {
    box_geometry_property(property)
        || matches!(
            property,
            ViewPropertyKind::Overflow
                | ViewPropertyKind::OverflowX
                | ViewPropertyKind::OverflowY
                | ViewPropertyKind::OverflowInline
                | ViewPropertyKind::OverflowBlock
        )
}

const fn image_box_property(property: ViewPropertyKind) -> bool {
    box_geometry_property(property) || matches!(property, ViewPropertyKind::ZIndex)
}

const fn surface_visual_property(property: ViewPropertyKind) -> bool {
    matches!(
        property,
        ViewPropertyKind::BackgroundColor
            | ViewPropertyKind::BorderColor
            | ViewPropertyKind::BorderWidth
            | ViewPropertyKind::BorderRadius
            | ViewPropertyKind::Opacity
            | ViewPropertyKind::BoxShadow
            | ViewPropertyKind::Filter
            | ViewPropertyKind::BackdropFilter
    )
}

const fn control_adornment_property(property: ViewPropertyKind) -> bool {
    matches!(
        property,
        ViewPropertyKind::OutlineColor
            | ViewPropertyKind::OutlineWidth
            | ViewPropertyKind::OutlineOffset
            | ViewPropertyKind::FocusRingColor
            | ViewPropertyKind::FocusRingWidth
            | ViewPropertyKind::CornerFrameColor
            | ViewPropertyKind::CornerFrameWidth
            | ViewPropertyKind::CornerFrameLength
            | ViewPropertyKind::CornerFrameOffset
    )
}

const fn static_text_visual_property(property: ViewPropertyKind) -> bool {
    matches!(
        property,
        ViewPropertyKind::Color
            | ViewPropertyKind::FontFamily
            | ViewPropertyKind::FontSize
            | ViewPropertyKind::FontWeight
            | ViewPropertyKind::LineHeight
            | ViewPropertyKind::LetterSpacing
            | ViewPropertyKind::SelectionColor
    )
}

const fn control_text_visual_property(property: ViewPropertyKind) -> bool {
    static_text_visual_property(property)
        || matches!(
            property,
            ViewPropertyKind::PlaceholderColor
                | ViewPropertyKind::CaretColor
                | ViewPropertyKind::CompositionUnderlineColor
        )
}

const fn image_visual_property(property: ViewPropertyKind) -> bool {
    matches!(property, ViewPropertyKind::Opacity)
}

#[derive(Clone, Copy, Debug)]
pub(in crate::frame) struct BoxStyle {
    pub(in crate::frame) width: Option<u32>,
    pub(in crate::frame) height: Option<u32>,
    pub(in crate::frame) translate_x: i32,
    pub(in crate::frame) translate_y: i32,
    pub(in crate::frame) scale_milli: u32,
    pub(in crate::frame) overflow_x: ViewOverflow,
    pub(in crate::frame) overflow_y: ViewOverflow,
}

pub(in crate::frame) fn box_style(style: &ViewRuntimeNodeStyle) -> BoxStyle {
    let width = length_property(style, ViewPropertyKind::Width)
        .or_else(|| length_property(style, ViewPropertyKind::InlineSize));
    let height = length_property(style, ViewPropertyKind::Height)
        .or_else(|| length_property(style, ViewPropertyKind::BlockSize));
    let translate_x = signed_length_property(style, ViewPropertyKind::TranslateX)
        .or_else(|| signed_length_property(style, ViewPropertyKind::TranslateInline))
        .unwrap_or_default();
    let translate_y = signed_length_property(style, ViewPropertyKind::TranslateY)
        .or_else(|| signed_length_property(style, ViewPropertyKind::TranslateBlock))
        .unwrap_or_default();
    let scale_milli = match style.composite().value(ViewPropertyKind::Scale) {
        Some(ViewSpecifiedValue::Scalar { value }) => value.value(),
        _ => 1_000,
    };
    let overflow_x = overflow_property(
        style,
        &[
            ViewPropertyKind::OverflowX,
            ViewPropertyKind::OverflowInline,
            ViewPropertyKind::Overflow,
        ],
    );
    let overflow_y = overflow_property(
        style,
        &[
            ViewPropertyKind::OverflowY,
            ViewPropertyKind::OverflowBlock,
            ViewPropertyKind::Overflow,
        ],
    );
    BoxStyle {
        width,
        height,
        translate_x,
        translate_y,
        scale_milli,
        overflow_x,
        overflow_y,
    }
}

fn overflow_property(
    style: &ViewRuntimeNodeStyle,
    properties: &[ViewPropertyKind],
) -> ViewOverflow {
    properties
        .iter()
        .find_map(|property| match style.layout().value(*property) {
            Some(ViewSpecifiedValue::Overflow { value }) => Some(*value),
            _ => None,
        })
        .unwrap_or(ViewOverflow::Visible)
}

fn length_property(style: &ViewRuntimeNodeStyle, property: ViewPropertyKind) -> Option<u32> {
    signed_length_property(style, property)
        .map(|value| u32::try_from(value.max(0)).unwrap_or(u32::MAX))
}

fn signed_length_property(style: &ViewRuntimeNodeStyle, property: ViewPropertyKind) -> Option<i32> {
    match style
        .layout()
        .value(property)
        .or_else(|| style.composite().value(property))
    {
        Some(ViewSpecifiedValue::Length { value }) => Some(value.value()),
        _ => None,
    }
}

pub(super) fn scaled_dimension(value: u32, scale_milli: u32) -> u32 {
    u32::try_from(u64::from(value).saturating_mul(u64::from(scale_milli)) / 1_000)
        .unwrap_or(u32::MAX)
}

fn scaled_i32(value: i32, scale_milli: u32) -> i32 {
    let value = i64::from(value)
        .saturating_mul(i64::from(scale_milli))
        .saturating_div(1_000);
    i32::try_from(value).unwrap_or(if value.is_negative() {
        i32::MIN
    } else {
        i32::MAX
    })
}

fn apply_text_control_box(control: &mut ViewRuntimeTextControl, style: &ViewRuntimeNodeStyle) {
    let style = box_style(style);
    control.bounds.x_milli = control.bounds.x_milli.saturating_add(style.translate_x);
    control.bounds.y_milli = control.bounds.y_milli.saturating_add(style.translate_y);
    control.bounds.width_milli = scaled_dimension(
        style.width.unwrap_or(control.bounds.width_milli),
        style.scale_milli,
    );
    control.bounds.height_milli = scaled_dimension(
        style.height.unwrap_or(control.bounds.height_milli),
        style.scale_milli,
    );
}

fn apply_action_button_box(button: &mut ViewRuntimeActionButton, style: &ViewRuntimeNodeStyle) {
    let style = box_style(style);
    button.bounds.x_milli = button.bounds.x_milli.saturating_add(style.translate_x);
    button.bounds.y_milli = button.bounds.y_milli.saturating_add(style.translate_y);
    button.bounds.width_milli = scaled_dimension(
        style.width.unwrap_or(button.bounds.width_milli),
        style.scale_milli,
    );
    button.bounds.height_milli = scaled_dimension(
        style.height.unwrap_or(button.bounds.height_milli),
        style.scale_milli,
    );
}

fn apply_scroll_region_box(region: &mut ViewRuntimeScrollRegion, style: &ViewRuntimeNodeStyle) {
    let style = box_style(style);
    region.bounds.x_milli = region.bounds.x_milli.saturating_add(style.translate_x);
    region.bounds.y_milli = region.bounds.y_milli.saturating_add(style.translate_y);
    region.bounds.width_milli = scaled_dimension(
        style.width.unwrap_or(region.bounds.width_milli),
        style.scale_milli,
    );
    region.bounds.height_milli = scaled_dimension(
        style.height.unwrap_or(region.bounds.height_milli),
        style.scale_milli,
    );
}

fn apply_surface_box(surface: &mut ViewRuntimeSurface, style: &ViewRuntimeNodeStyle) {
    let style = box_style(style);
    surface.bounds.x_milli = surface.bounds.x_milli.saturating_add(style.translate_x);
    surface.bounds.y_milli = surface.bounds.y_milli.saturating_add(style.translate_y);
    surface.bounds.width_milli = scaled_dimension(
        style.width.unwrap_or(surface.bounds.width_milli),
        style.scale_milli,
    );
    surface.bounds.height_milli = scaled_dimension(
        style.height.unwrap_or(surface.bounds.height_milli),
        style.scale_milli,
    );
}

fn apply_image_box(image: &mut BundleImageObject, style: &ViewRuntimeNodeStyle) {
    let box_style = box_style(style);
    image.bounds.width_milli = box_style.width.unwrap_or(image.bounds.width_milli);
    image.bounds.height_milli = box_style.height.unwrap_or(image.bounds.height_milli);
    image.transform.m11_milli = scaled_i32(image.transform.m11_milli, box_style.scale_milli);
    image.transform.m12_milli = scaled_i32(image.transform.m12_milli, box_style.scale_milli);
    image.transform.m21_milli = scaled_i32(image.transform.m21_milli, box_style.scale_milli);
    image.transform.m22_milli = scaled_i32(image.transform.m22_milli, box_style.scale_milli);
    image.transform.tx_milli = image
        .transform
        .tx_milli
        .saturating_add(box_style.translate_x);
    image.transform.ty_milli = image
        .transform
        .ty_milli
        .saturating_add(box_style.translate_y);
    if let Some(depth) = style.visual().depth_milli {
        image.depth_milli = depth;
    }
    if let Some(opacity) = style.visual().opacity_milli {
        image.opacity_milli = opacity;
    }
}

impl From<ViewRuntimeStyleProjectionError> for PlayerFrameError {
    fn from(source: ViewRuntimeStyleProjectionError) -> Self {
        Self::StyleProjection(source)
    }
}
