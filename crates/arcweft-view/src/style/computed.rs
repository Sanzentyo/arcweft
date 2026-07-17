//! Canonical typed Style result shared by every View runtime consumer.

use super::cascade::{ViewStyleContributionSource, ViewStylePriority};
use super::{
    ViewAxisUsageSet, ViewBoxAxisMode, ViewBoxAxisRevision, ViewBoxAxisSeedSource,
    ViewBoxAxisSource, ViewComputedPropertyKind, ViewDisplay, ViewFlexDirection,
    ViewInheritedBoxAxes, ViewLengthMilli, ViewOverflow, ViewPhysicalBoxStyle,
    ViewPhysicalContainerStyle, ViewPhysicalFlow, ViewPosition, ViewPropertyKind, ViewScalarMilli,
    ViewSpecifiedValue, ViewStyleInvalidationSet,
};
use crate::geometry::{
    ViewGeometryConsumer, ViewGeometryError, ViewGeometryField, ViewRepresentedGeometryFeature,
    validate_supported_properties,
};
use crate::{ViewElementKind, ViewStyleNodeKey};
use std::collections::BTreeMap;

/// Revision carried by one computed result for parent/cache invalidation.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ComputedViewStyleRevision(u64);

/// Effective box axes and the provider that established them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComputedViewAxes {
    mode: ViewBoxAxisMode,
    revision: ViewBoxAxisRevision,
    source: ViewBoxAxisSource,
}

/// One winning typed property together with deterministic cascade provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComputedViewProperty {
    authored_property: ViewPropertyKind,
    expanded_property: ViewPropertyKind,
    resolved_property: ViewComputedPropertyKind,
    value: ViewSpecifiedValue,
    priority: ViewStylePriority,
    source: ViewStyleContributionSource,
}

/// Canonical transition target resolved against the same axis snapshot as values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComputedViewTransition {
    authored_property: ViewPropertyKind,
    resolved_property: ViewComputedPropertyKind,
    axis_snapshot: ViewBoxAxisMode,
    duration_millis: u32,
    delay_millis: u32,
}

/// Fully token-resolved Style for one retained View node and state snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComputedViewStyle {
    axes: ComputedViewAxes,
    properties: BTreeMap<ViewComputedPropertyKind, ComputedViewProperty>,
    transitions: Vec<ComputedViewTransition>,
    axis_usage: ViewAxisUsageSet,
    revision: ComputedViewStyleRevision,
}

impl Default for ComputedViewAxes {
    fn default() -> Self {
        Self::host_default()
    }
}

impl Default for ComputedViewStyle {
    fn default() -> Self {
        Self {
            axes: ComputedViewAxes::default(),
            properties: BTreeMap::new(),
            transitions: Vec::new(),
            axis_usage: ViewAxisUsageSet::NONE,
            revision: ComputedViewStyleRevision::default(),
        }
    }
}

impl ComputedViewStyleRevision {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl ComputedViewAxes {
    pub(crate) const fn host_default() -> Self {
        Self {
            mode: ViewBoxAxisMode::HorizontalLtr,
            revision: ViewBoxAxisRevision::from_raw(0),
            source: ViewBoxAxisSource::HostDefault,
        }
    }

    pub(crate) const fn from_inherited_seed(seed: ViewInheritedBoxAxes) -> Self {
        let source = match seed.source() {
            ViewBoxAxisSeedSource::HostDefault => ViewBoxAxisSource::HostDefault,
            ViewBoxAxisSeedSource::HostExplicit => ViewBoxAxisSource::HostExplicit,
            ViewBoxAxisSeedSource::Parent => ViewBoxAxisSource::Inherited {
                parent: seed.revision(),
            },
        };
        Self {
            mode: seed.mode(),
            revision: seed.revision(),
            source,
        }
    }

    pub(crate) const fn styled(
        mode: ViewBoxAxisMode,
        revision: ViewBoxAxisRevision,
        priority: ViewStylePriority,
        source: ViewStyleContributionSource,
    ) -> Self {
        Self {
            mode,
            revision,
            source: ViewBoxAxisSource::Style { priority, source },
        }
    }

    pub const fn mode(&self) -> ViewBoxAxisMode {
        self.mode
    }

    pub const fn revision(&self) -> ViewBoxAxisRevision {
        self.revision
    }

    pub const fn source(&self) -> &ViewBoxAxisSource {
        &self.source
    }

    pub const fn inherited_snapshot(&self) -> ViewInheritedBoxAxes {
        ViewInheritedBoxAxes::from_parent(self.mode, self.revision)
    }
}

impl ComputedViewProperty {
    pub(super) const fn new(
        authored_property: ViewPropertyKind,
        expanded_property: ViewPropertyKind,
        resolved_property: ViewComputedPropertyKind,
        value: ViewSpecifiedValue,
        priority: ViewStylePriority,
        source: ViewStyleContributionSource,
    ) -> Self {
        Self {
            authored_property,
            expanded_property,
            resolved_property,
            value,
            priority,
            source,
        }
    }

    pub const fn authored_property(&self) -> ViewPropertyKind {
        self.authored_property
    }

    pub const fn expanded_property(&self) -> ViewPropertyKind {
        self.expanded_property
    }

    pub const fn resolved_property(&self) -> ViewComputedPropertyKind {
        self.resolved_property
    }

    pub const fn value(&self) -> &ViewSpecifiedValue {
        &self.value
    }

    pub const fn priority(&self) -> ViewStylePriority {
        self.priority
    }

    pub const fn source(&self) -> &ViewStyleContributionSource {
        &self.source
    }
}

impl ComputedViewTransition {
    pub const fn new(
        authored_property: ViewPropertyKind,
        resolved_property: ViewComputedPropertyKind,
        axis_snapshot: ViewBoxAxisMode,
        duration_millis: u32,
        delay_millis: u32,
    ) -> Self {
        Self {
            authored_property,
            resolved_property,
            axis_snapshot,
            duration_millis,
            delay_millis,
        }
    }

    pub const fn authored_property(self) -> ViewPropertyKind {
        self.authored_property
    }

    pub const fn resolved_property(self) -> ViewComputedPropertyKind {
        self.resolved_property
    }

    pub const fn axis_snapshot(self) -> ViewBoxAxisMode {
        self.axis_snapshot
    }

    pub const fn duration_millis(self) -> u32 {
        self.duration_millis
    }

    pub const fn delay_millis(self) -> u32 {
        self.delay_millis
    }
}

impl ComputedViewStyle {
    pub(super) const fn from_properties(
        axes: ComputedViewAxes,
        properties: BTreeMap<ViewComputedPropertyKind, ComputedViewProperty>,
        transitions: Vec<ComputedViewTransition>,
        axis_usage: ViewAxisUsageSet,
        revision: ComputedViewStyleRevision,
    ) -> Self {
        Self {
            axes,
            properties,
            transitions,
            axis_usage,
            revision,
        }
    }

    pub const fn revision(&self) -> ComputedViewStyleRevision {
        self.revision
    }

    pub const fn axes(&self) -> &ComputedViewAxes {
        &self.axes
    }

    pub fn property(&self, property: ViewPropertyKind) -> Option<&ComputedViewProperty> {
        ViewComputedPropertyKind::try_from_property(property)
            .and_then(|property| self.properties.get(&property))
    }

    pub fn canonical_property(
        &self,
        property: ViewComputedPropertyKind,
    ) -> Option<&ComputedViewProperty> {
        self.properties.get(&property)
    }

    pub fn value(&self, property: ViewPropertyKind) -> Option<&ViewSpecifiedValue> {
        self.property(property).map(ComputedViewProperty::value)
    }

    pub fn properties(
        &self,
    ) -> impl ExactSizeIterator<Item = (ViewPropertyKind, &ComputedViewProperty)> {
        self.properties
            .iter()
            .map(|(property, value)| (property.as_property(), value))
    }

    pub fn canonical_properties(
        &self,
    ) -> impl ExactSizeIterator<Item = (ViewComputedPropertyKind, &ComputedViewProperty)> {
        self.properties
            .iter()
            .map(|(property, value)| (*property, value))
    }

    pub fn transitions(&self) -> &[ComputedViewTransition] {
        &self.transitions
    }

    pub const fn axis_usage(&self) -> ViewAxisUsageSet {
        self.axis_usage
    }

    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }

    /// Shared canonical physical box packet. No logical alias fallback occurs here.
    pub fn physical_box(&self) -> ViewPhysicalBoxStyle {
        let mut physical = ViewPhysicalBoxStyle {
            axes: self.axes.mode(),
            ..ViewPhysicalBoxStyle::default()
        };
        physical.display = self.display(ViewPropertyKind::Display);
        physical.position = self
            .position(ViewPropertyKind::Position)
            .unwrap_or(ViewPosition::Static);
        physical.width = self.length(ViewPropertyKind::Width);
        physical.height = self.length(ViewPropertyKind::Height);
        physical.min_width = self.length(ViewPropertyKind::MinWidth);
        physical.min_height = self.length(ViewPropertyKind::MinHeight);
        physical.max_width = self.length(ViewPropertyKind::MaxWidth);
        physical.max_height = self.length(ViewPropertyKind::MaxHeight);
        physical.padding.top = self.length_or_zero(ViewPropertyKind::PaddingTop);
        physical.padding.right = self.length_or_zero(ViewPropertyKind::PaddingRight);
        physical.padding.bottom = self.length_or_zero(ViewPropertyKind::PaddingBottom);
        physical.padding.left = self.length_or_zero(ViewPropertyKind::PaddingLeft);
        let border = self.length_or_zero(ViewPropertyKind::BorderWidth);
        physical.border = super::ViewPhysicalEdges::all(border);
        physical.margin.top = self.length_or_zero(ViewPropertyKind::MarginTop);
        physical.margin.right = self.length_or_zero(ViewPropertyKind::MarginRight);
        physical.margin.bottom = self.length_or_zero(ViewPropertyKind::MarginBottom);
        physical.margin.left = self.length_or_zero(ViewPropertyKind::MarginLeft);
        physical.inset.top = self.length(ViewPropertyKind::Top);
        physical.inset.right = self.length(ViewPropertyKind::Right);
        physical.inset.bottom = self.length(ViewPropertyKind::Bottom);
        physical.inset.left = self.length(ViewPropertyKind::Left);
        physical.translate_x = self
            .length(ViewPropertyKind::TranslateX)
            .unwrap_or_else(|| ViewLengthMilli::new(0));
        physical.translate_y = self
            .length(ViewPropertyKind::TranslateY)
            .unwrap_or_else(|| ViewLengthMilli::new(0));
        physical.scale = self
            .scalar(ViewPropertyKind::Scale)
            .unwrap_or(ViewScalarMilli::ONE);
        physical.overflow_x = self
            .overflow(ViewPropertyKind::OverflowX)
            .unwrap_or(ViewOverflow::Visible);
        physical.overflow_y = self
            .overflow(ViewPropertyKind::OverflowY)
            .unwrap_or(ViewOverflow::Visible);
        physical
    }

    /// Canonical physical container packet. Row/Column and gaps are never remapped by box axes.
    pub fn physical_container(
        &self,
        node: &ViewStyleNodeKey,
        element: ViewElementKind,
    ) -> Result<Option<ViewPhysicalContainerStyle>, ViewGeometryError> {
        let geometry_properties = self
            .properties()
            .map(|(property, _)| property)
            .collect::<Vec<_>>();
        validate_supported_properties(node, ViewGeometryConsumer::Layout, &geometry_properties)?;

        let display = self.display(ViewPropertyKind::Display);
        let flex_direction = self.flex_direction(ViewPropertyKind::FlexDirection);
        let row_gap = self.length_or_zero(ViewPropertyKind::RowGap);
        let column_gap = self.length_or_zero(ViewPropertyKind::ColumnGap);
        for (property, value) in [
            (ViewPropertyKind::RowGap, row_gap),
            (ViewPropertyKind::ColumnGap, column_gap),
        ] {
            if value.value() < 0 {
                return Err(ViewGeometryError::NegativeNonNegativeField {
                    node: node.clone(),
                    field: match property {
                        ViewPropertyKind::RowGap => ViewGeometryField::RowGap,
                        ViewPropertyKind::ColumnGap => ViewGeometryField::ColumnGap,
                        _ => unreachable!("closed gap inventory"),
                    },
                    value_milli: value.value(),
                });
            }
        }

        let default_flow = element.default_physical_flow();
        if default_flow.is_none() {
            let offending_property = flex_direction
                .map(|_| ViewPropertyKind::FlexDirection)
                .or_else(|| (row_gap.value() != 0).then_some(ViewPropertyKind::RowGap))
                .or_else(|| (column_gap.value() != 0).then_some(ViewPropertyKind::ColumnGap));
            if let Some(property) = offending_property {
                return Err(ViewGeometryError::ContainerStyleOnLeaf {
                    node: node.clone(),
                    element,
                    property,
                });
            }
        }

        if display == Some(ViewDisplay::None) {
            return Ok(None);
        }

        let flow = match display {
            Some(ViewDisplay::Inline) => {
                return Err(ViewGeometryError::UnsupportedConsumer {
                    node: node.clone(),
                    consumer: ViewGeometryConsumer::Layout,
                    property: ViewPropertyKind::Display,
                    feature: ViewRepresentedGeometryFeature::InlineLayout,
                });
            }
            Some(ViewDisplay::Stack) if default_flow.is_none() => {
                return Err(ViewGeometryError::DisplayRequiresContainer {
                    node: node.clone(),
                    element,
                    display: ViewDisplay::Stack,
                });
            }
            Some(ViewDisplay::Flex) if default_flow.is_none() => {
                return Err(ViewGeometryError::DisplayRequiresContainer {
                    node: node.clone(),
                    element,
                    display: ViewDisplay::Flex,
                });
            }
            Some(ViewDisplay::Stack) => Some(ViewPhysicalFlow::Overlay),
            Some(ViewDisplay::Block) => default_flow.map(|_| ViewPhysicalFlow::Column),
            Some(ViewDisplay::Flex) => default_flow.map(|_| {
                flex_direction.map_or(ViewPhysicalFlow::Row, ViewPhysicalFlow::from_flex_direction)
            }),
            Some(ViewDisplay::None) => None,
            None => default_flow
                .map(|flow| flex_direction.map_or(flow, ViewPhysicalFlow::from_flex_direction)),
        };

        let Some(flow) = flow else {
            return Ok(None);
        };
        validate_flow_gaps(node, flow, row_gap, column_gap)?;
        Ok(Some(ViewPhysicalContainerStyle {
            flow,
            row_gap,
            column_gap,
        }))
    }

    /// Exact retained work caused by moving from `previous` to this result.
    pub fn invalidation_from(&self, previous: &Self) -> ViewStyleInvalidationSet {
        ViewPropertyKind::ALL
            .iter()
            .copied()
            .filter(|property| property.is_computed_canonical())
            .filter(|property| self.value(*property) != previous.value(*property))
            .fold(ViewStyleInvalidationSet::NONE, |invalidation, property| {
                invalidation.union(property.default_invalidation())
            })
    }

    fn length(&self, property: ViewPropertyKind) -> Option<ViewLengthMilli> {
        match self.value(property) {
            Some(ViewSpecifiedValue::Length { value }) => Some(*value),
            _ => None,
        }
    }

    fn length_or_zero(&self, property: ViewPropertyKind) -> ViewLengthMilli {
        self.length(property)
            .unwrap_or_else(|| ViewLengthMilli::new(0))
    }

    fn scalar(&self, property: ViewPropertyKind) -> Option<ViewScalarMilli> {
        match self.value(property) {
            Some(ViewSpecifiedValue::Scalar { value }) => Some(*value),
            _ => None,
        }
    }

    fn display(&self, property: ViewPropertyKind) -> Option<ViewDisplay> {
        match self.value(property) {
            Some(ViewSpecifiedValue::Display { value }) => Some(*value),
            _ => None,
        }
    }

    fn position(&self, property: ViewPropertyKind) -> Option<ViewPosition> {
        match self.value(property) {
            Some(ViewSpecifiedValue::Position { value }) => Some(*value),
            _ => None,
        }
    }

    fn flex_direction(&self, property: ViewPropertyKind) -> Option<ViewFlexDirection> {
        match self.value(property) {
            Some(ViewSpecifiedValue::FlexDirection { value }) => Some(*value),
            _ => None,
        }
    }

    fn overflow(&self, property: ViewPropertyKind) -> Option<ViewOverflow> {
        match self.value(property) {
            Some(ViewSpecifiedValue::Overflow { value }) => Some(*value),
            _ => None,
        }
    }
}

fn validate_flow_gaps(
    node: &ViewStyleNodeKey,
    flow: ViewPhysicalFlow,
    row_gap: ViewLengthMilli,
    column_gap: ViewLengthMilli,
) -> Result<(), ViewGeometryError> {
    let error = match flow {
        ViewPhysicalFlow::Row | ViewPhysicalFlow::RowReverse if row_gap.value() != 0 => {
            ViewGeometryError::CrossAxisGapRequiresWrap {
                node: node.clone(),
                flow,
                property: ViewPropertyKind::RowGap,
                value_milli: row_gap.value(),
            }
        }
        ViewPhysicalFlow::Column | ViewPhysicalFlow::ColumnReverse if column_gap.value() != 0 => {
            ViewGeometryError::CrossAxisGapRequiresWrap {
                node: node.clone(),
                flow,
                property: ViewPropertyKind::ColumnGap,
                value_milli: column_gap.value(),
            }
        }
        ViewPhysicalFlow::Overlay if row_gap.value() != 0 => {
            ViewGeometryError::GapRequiresLinearFlow {
                node: node.clone(),
                flow,
                property: ViewPropertyKind::RowGap,
                value_milli: row_gap.value(),
            }
        }
        ViewPhysicalFlow::Overlay if column_gap.value() != 0 => {
            ViewGeometryError::GapRequiresLinearFlow {
                node: node.clone(),
                flow,
                property: ViewPropertyKind::ColumnGap,
                value_milli: column_gap.value(),
            }
        }
        _ => return Ok(()),
    };
    Err(error)
}
