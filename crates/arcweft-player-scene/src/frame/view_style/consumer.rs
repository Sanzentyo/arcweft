//! Runtime consumer support and lossless computed-Style projection.

use super::super::PlayerFrameError;
use super::super::ViewCommittedGeometryFrame;
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
use arcweft_view::style::{ComputedViewStyle, ViewPropertyKind, ViewStyleContributionSource};
use std::sync::Arc;

/// Presentation resources with the current live Style snapshot applied.
pub(in crate::frame) struct StyledViewResources {
    pub(in crate::frame) text_inputs: Vec<ViewRuntimeTextControl>,
    pub(in crate::frame) action_buttons: Vec<ViewRuntimeActionButton>,
    pub(in crate::frame) scroll_regions: Vec<ViewRuntimeScrollRegion>,
    pub(in crate::frame) surfaces: Vec<ViewRuntimeSurface>,
    pub(in crate::frame) images: Vec<BundleImageObject>,
    pub(in crate::frame) geometry: Arc<ViewCommittedGeometryFrame>,
}

impl ResolvedViewStyleFrame {
    pub(in crate::frame) fn apply_to_presentation(
        &self,
        presentation: &BundlePresentationSnapshot,
        geometry: Arc<ViewCommittedGeometryFrame>,
    ) -> StyledViewResources {
        StyledViewResources {
            text_inputs: presentation
                .text_inputs
                .iter()
                .cloned()
                .map(|mut control| {
                    if let Some(style) = self.control(&control.target) {
                        control.style = style.visual().clone();
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
                    }
                    button
                })
                .collect(),
            scroll_regions: presentation.scroll_regions.clone(),
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
                        apply_image_visual(&mut image, style);
                    }
                    image
                })
                .collect(),
            geometry,
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
            let scoped_part = node
                .part
                .as_ref()
                .map(|part| mount.scoped_id(part.as_public_id().as_str()));
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
            property.is_inherited()
                || box_geometry_property(property)
                || container_gap_property(element, property)
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
    box_geometry_property(property)
        || matches!(
            property,
            ViewPropertyKind::OverflowX | ViewPropertyKind::OverflowY
        )
}

const fn container_gap_property(element: ViewElementKind, property: ViewPropertyKind) -> bool {
    matches!(element, ViewElementKind::Column | ViewElementKind::Row)
        && matches!(
            property,
            ViewPropertyKind::RowGap | ViewPropertyKind::ColumnGap
        )
}

const fn box_geometry_property(property: ViewPropertyKind) -> bool {
    matches!(
        property,
        ViewPropertyKind::Display
            | ViewPropertyKind::Width
            | ViewPropertyKind::Height
            | ViewPropertyKind::MinWidth
            | ViewPropertyKind::MinHeight
            | ViewPropertyKind::MaxWidth
            | ViewPropertyKind::MaxHeight
            | ViewPropertyKind::PaddingTop
            | ViewPropertyKind::PaddingRight
            | ViewPropertyKind::PaddingBottom
            | ViewPropertyKind::PaddingLeft
            | ViewPropertyKind::MarginTop
            | ViewPropertyKind::MarginRight
            | ViewPropertyKind::MarginBottom
            | ViewPropertyKind::MarginLeft
            | ViewPropertyKind::Position
            | ViewPropertyKind::Top
            | ViewPropertyKind::Right
            | ViewPropertyKind::Bottom
            | ViewPropertyKind::Left
            | ViewPropertyKind::TranslateX
            | ViewPropertyKind::TranslateY
            | ViewPropertyKind::Scale
    )
}

const fn surface_box_property(property: ViewPropertyKind) -> bool {
    box_geometry_property(property)
        || matches!(
            property,
            ViewPropertyKind::ZIndex | ViewPropertyKind::OverflowX | ViewPropertyKind::OverflowY
        )
}

const fn control_box_property(property: ViewPropertyKind) -> bool {
    box_geometry_property(property) || matches!(property, ViewPropertyKind::ZIndex)
}

const fn text_box_property(property: ViewPropertyKind) -> bool {
    box_geometry_property(property)
        || matches!(
            property,
            ViewPropertyKind::OverflowX | ViewPropertyKind::OverflowY
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

fn apply_image_visual(image: &mut BundleImageObject, style: &ViewRuntimeNodeStyle) {
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
