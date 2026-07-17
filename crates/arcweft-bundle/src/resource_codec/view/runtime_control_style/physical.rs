//! Transient canonical physical geometry projection for one retained View node.

use super::ViewRuntimeStyleProjectionError;
use arcweft_view::ViewElementKind;
use arcweft_view::geometry::{
    ViewGeometryConsumer, ViewGeometryError, ViewGeometryField, ViewGeometryPropertySupport,
    ViewRepresentedGeometryFeature, validate_supported_properties,
};
use arcweft_view::style::{
    ComputedViewStyle, ViewDisplay, ViewLengthMilli, ViewPhysicalBoxStyle,
    ViewPhysicalContainerStyle, ViewPropertyKind, ViewSpecifiedValue, ViewStyleNodeKey,
};

/// Runtime owner of one physical Style projection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewRuntimeGeometryOwner {
    Element(ViewElementKind),
    Text,
    Image,
    Custom,
    CallView,
}

/// Whether one runtime node participates in executable physical geometry.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewRuntimeGeometryParticipation {
    Transparent,
    Suppressed,
    Leaf,
    Container,
}

/// The sole transient physical packet passed beyond computed Style projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewRuntimePhysicalNodeStyle {
    node: ViewStyleNodeKey,
    owner: ViewRuntimeGeometryOwner,
    participation: ViewRuntimeGeometryParticipation,
    box_style: Option<ViewPhysicalBoxStyle>,
    container_style: Option<ViewPhysicalContainerStyle>,
}

impl ViewRuntimePhysicalNodeStyle {
    pub fn try_from_computed(
        node: ViewStyleNodeKey,
        owner: ViewRuntimeGeometryOwner,
        computed: &ComputedViewStyle,
    ) -> Result<Self, ViewRuntimeStyleProjectionError> {
        match owner {
            ViewRuntimeGeometryOwner::Element(element) => {
                Self::project_element(node, owner, element, computed)
            }
            ViewRuntimeGeometryOwner::Text | ViewRuntimeGeometryOwner::Image => {
                Self::project_non_element_leaf(node, owner, computed)
            }
            ViewRuntimeGeometryOwner::Custom | ViewRuntimeGeometryOwner::CallView => {
                Self::project_transparent(node, owner, computed)
            }
        }
    }

    pub const fn node(&self) -> &ViewStyleNodeKey {
        &self.node
    }

    pub const fn owner(&self) -> ViewRuntimeGeometryOwner {
        self.owner
    }

    pub const fn participation(&self) -> ViewRuntimeGeometryParticipation {
        self.participation
    }

    pub const fn box_style(&self) -> Option<&ViewPhysicalBoxStyle> {
        self.box_style.as_ref()
    }

    pub const fn container_style(&self) -> Option<&ViewPhysicalContainerStyle> {
        self.container_style.as_ref()
    }

    fn project_element(
        node: ViewStyleNodeKey,
        owner: ViewRuntimeGeometryOwner,
        element: ViewElementKind,
        computed: &ComputedViewStyle,
    ) -> Result<Self, ViewRuntimeStyleProjectionError> {
        let box_style = computed.physical_box();
        let container_style = computed
            .physical_container(&node, element)
            .map_err(|source| ViewRuntimeStyleProjectionError::Geometry {
                node: node.clone(),
                source,
            })?;
        let participation = if box_style.display == Some(ViewDisplay::None) {
            ViewRuntimeGeometryParticipation::Suppressed
        } else if container_style.is_some() {
            ViewRuntimeGeometryParticipation::Container
        } else {
            ViewRuntimeGeometryParticipation::Leaf
        };
        Ok(Self {
            node,
            owner,
            participation,
            box_style: Some(box_style),
            container_style,
        })
    }

    fn project_non_element_leaf(
        node: ViewStyleNodeKey,
        owner: ViewRuntimeGeometryOwner,
        computed: &ComputedViewStyle,
    ) -> Result<Self, ViewRuntimeStyleProjectionError> {
        validate_geometry_support(&node, computed)?;
        let box_style = computed.physical_box();
        let row_gap = length(computed, ViewPropertyKind::RowGap);
        let column_gap = length(computed, ViewPropertyKind::ColumnGap);
        for (property, field, value) in [
            (ViewPropertyKind::RowGap, ViewGeometryField::RowGap, row_gap),
            (
                ViewPropertyKind::ColumnGap,
                ViewGeometryField::ColumnGap,
                column_gap,
            ),
        ] {
            if value.is_some_and(|value| value.value() < 0) {
                let value_milli = value.expect("negative checked value is present").value();
                return Err(ViewRuntimeStyleProjectionError::Geometry {
                    node: node.clone(),
                    source: ViewGeometryError::NegativeNonNegativeField {
                        node,
                        field,
                        value_milli,
                    },
                });
            }
            if value.is_some_and(|value| value.value() != 0) {
                return Err(
                    ViewRuntimeStyleProjectionError::GeometryOnTransparentOwner {
                        node,
                        owner,
                        property,
                    },
                );
            }
        }
        if computed.value(ViewPropertyKind::FlexDirection).is_some() {
            return Err(
                ViewRuntimeStyleProjectionError::GeometryOnTransparentOwner {
                    node,
                    owner,
                    property: ViewPropertyKind::FlexDirection,
                },
            );
        }
        match box_style.display {
            Some(ViewDisplay::Inline) => {
                return Err(ViewRuntimeStyleProjectionError::Geometry {
                    node: node.clone(),
                    source: ViewGeometryError::UnsupportedConsumer {
                        node,
                        consumer: ViewGeometryConsumer::Layout,
                        property: ViewPropertyKind::Display,
                        feature: ViewRepresentedGeometryFeature::InlineLayout,
                    },
                });
            }
            Some(display @ (ViewDisplay::Stack | ViewDisplay::Flex)) => {
                let _ = display;
                return Err(
                    ViewRuntimeStyleProjectionError::GeometryOnTransparentOwner {
                        node,
                        owner,
                        property: ViewPropertyKind::Display,
                    },
                );
            }
            None | Some(ViewDisplay::None | ViewDisplay::Block) => {}
        }
        let participation = if box_style.display == Some(ViewDisplay::None) {
            ViewRuntimeGeometryParticipation::Suppressed
        } else {
            ViewRuntimeGeometryParticipation::Leaf
        };
        Ok(Self {
            node,
            owner,
            participation,
            box_style: Some(box_style),
            container_style: None,
        })
    }

    fn project_transparent(
        node: ViewStyleNodeKey,
        owner: ViewRuntimeGeometryOwner,
        computed: &ComputedViewStyle,
    ) -> Result<Self, ViewRuntimeStyleProjectionError> {
        validate_geometry_support(&node, computed)?;
        let display = computed.physical_box().display;
        for (property, _) in computed.properties() {
            if property.geometry_support() == ViewGeometryPropertySupport::Supported
                && !(property == ViewPropertyKind::Display && display == Some(ViewDisplay::None))
            {
                return Err(
                    ViewRuntimeStyleProjectionError::GeometryOnTransparentOwner {
                        node,
                        owner,
                        property,
                    },
                );
            }
        }
        Ok(Self {
            node,
            owner,
            participation: if display == Some(ViewDisplay::None) {
                ViewRuntimeGeometryParticipation::Suppressed
            } else {
                ViewRuntimeGeometryParticipation::Transparent
            },
            box_style: None,
            container_style: None,
        })
    }
}

fn validate_geometry_support(
    node: &ViewStyleNodeKey,
    computed: &ComputedViewStyle,
) -> Result<(), ViewRuntimeStyleProjectionError> {
    let properties = computed
        .properties()
        .map(|(property, _)| property)
        .collect::<Vec<_>>();
    validate_supported_properties(node, ViewGeometryConsumer::Layout, &properties).map_err(
        |source| ViewRuntimeStyleProjectionError::Geometry {
            node: node.clone(),
            source,
        },
    )
}

fn length(computed: &ComputedViewStyle, property: ViewPropertyKind) -> Option<ViewLengthMilli> {
    match computed.value(property) {
        Some(ViewSpecifiedValue::Length { value }) => Some(*value),
        _ => None,
    }
}
