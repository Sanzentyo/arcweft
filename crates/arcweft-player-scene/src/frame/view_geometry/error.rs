//! Typed retained-geometry diagnostics.

use arcweft_bundle::resource_codec::{ViewRuntimeGeometryOwner, ViewRuntimeStyleProjectionError};
use arcweft_view::geometry::{
    ViewGeometryConsumer, ViewGeometryError, ViewGeometryField, ViewGeometryOperation,
};
use arcweft_view::style::{ViewPhysicalAxis, ViewStyleNodeKey};
use thiserror::Error;

use super::intrinsic::ViewIntrinsicGeometryError;
use super::{ViewGeometryConversionError, ViewGeometryConversionField, ViewGeometryPlatform};
use arcweft_render_wgpu::geometry::view_final::{
    ViewFinalGeometryField, ViewFinalGeometryLoweringError,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ViewGeometryProductKind {
    ActionButton,
    TextField,
    TextArea,
    SecureField,
    ScrollRegion,
    Surface,
    TextOutput,
    Image,
}

/// Stable non-transport-specific category for a retained geometry failure.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewGeometryFailureCode {
    Projection,
    TreeDuplicateNode,
    TreeMissingParent,
    TreeCycle,
    TreeCrossMountParent,
    TreeCallAttachmentMismatch,
    TreeLeafHasChildren,
    TreeMissingNestedRoot,
    TreeDuplicateTarget,
    ProductMissing,
    ProductDuplicate,
    ProductOwnerMismatch,
    IntrinsicMissingMeasure,
    IntrinsicBoundsOverflow,
    NegativeNonNegativeField,
    ConflictingConstraints,
    EdgesExceedUsedBorderBox,
    ArithmeticOverflow,
    InvertedSpan,
    InvertedRect,
    InvertedMarginSpan,
    InvertedMarginBox,
    InsetOnStatic,
    OverConstrainedRelativeAxis,
    OverConstrainedPositionedAxis,
    PositionedStretchConstraintViolation,
    ScrollOffsetOutOfRange,
    UnsupportedConsumer,
    DisplayRequiresContainer,
    ContainerStyleOnLeaf,
    CrossAxisGapRequiresWrap,
    GapRequiresLinearFlow,
    InvalidTree,
    MissingIntrinsicMeasure,
    InvalidPointerCoordinate,
    ConversionNonFiniteInput,
    ConversionOutsideMilliRange,
    ConversionNegativeExtent,
    ConversionInexactF32,
    ConversionIndexRange,
    StalePreparedGeneration,
    GenerationOverflow,
}

impl ViewGeometryFailureCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Projection => "view.geometry.projection",
            Self::TreeDuplicateNode => "view.geometry.tree.duplicate_node",
            Self::TreeMissingParent => "view.geometry.tree.missing_parent",
            Self::TreeCycle => "view.geometry.tree.cycle",
            Self::TreeCrossMountParent => "view.geometry.tree.cross_mount_parent",
            Self::TreeCallAttachmentMismatch => "view.geometry.tree.call_attachment_mismatch",
            Self::TreeLeafHasChildren => "view.geometry.tree.leaf_has_children",
            Self::TreeMissingNestedRoot => "view.geometry.tree.missing_nested_root",
            Self::TreeDuplicateTarget => "view.geometry.tree.duplicate_target",
            Self::ProductMissing => "view.geometry.product.missing",
            Self::ProductDuplicate => "view.geometry.product.duplicate",
            Self::ProductOwnerMismatch => "view.geometry.product.owner_mismatch",
            Self::IntrinsicMissingMeasure => "view.geometry.intrinsic.missing_measure",
            Self::IntrinsicBoundsOverflow => "view.geometry.intrinsic.bounds_overflow",
            Self::NegativeNonNegativeField => "view.geometry.negative_non_negative_field",
            Self::ConflictingConstraints => "view.geometry.conflicting_constraints",
            Self::EdgesExceedUsedBorderBox => "view.geometry.edges_exceed_used_border_box",
            Self::ArithmeticOverflow => "view.geometry.arithmetic_overflow",
            Self::InvertedSpan => "view.geometry.inverted_span",
            Self::InvertedRect => "view.geometry.inverted_rect",
            Self::InvertedMarginSpan => "view.geometry.inverted_margin_span",
            Self::InvertedMarginBox => "view.geometry.inverted_margin_box",
            Self::InsetOnStatic => "view.geometry.inset_on_static",
            Self::OverConstrainedRelativeAxis => "view.geometry.over_constrained_relative_axis",
            Self::OverConstrainedPositionedAxis => "view.geometry.over_constrained_positioned_axis",
            Self::PositionedStretchConstraintViolation => {
                "view.geometry.positioned_stretch_constraint_violation"
            }
            Self::ScrollOffsetOutOfRange => "view.geometry.scroll_offset_out_of_range",
            Self::UnsupportedConsumer => "view.geometry.unsupported_consumer",
            Self::DisplayRequiresContainer => "view.geometry.display_requires_container",
            Self::ContainerStyleOnLeaf => "view.geometry.container_style_on_leaf",
            Self::CrossAxisGapRequiresWrap => "view.geometry.cross_axis_gap_requires_wrap",
            Self::GapRequiresLinearFlow => "view.geometry.gap_requires_linear_flow",
            Self::InvalidTree => "view.geometry.invalid_tree",
            Self::MissingIntrinsicMeasure => "view.geometry.missing_intrinsic_measure",
            Self::InvalidPointerCoordinate => "view.geometry.invalid_pointer_coordinate",
            Self::ConversionNonFiniteInput => "view.geometry.conversion.non_finite_input",
            Self::ConversionOutsideMilliRange => "view.geometry.conversion.outside_milli_range",
            Self::ConversionNegativeExtent => "view.geometry.conversion.negative_extent",
            Self::ConversionInexactF32 => "view.geometry.conversion.inexact_f32",
            Self::ConversionIndexRange => "view.geometry.conversion.index_range",
            Self::StalePreparedGeneration => "view.geometry.stale_prepared_generation",
            Self::GenerationOverflow => "view.geometry.generation_overflow",
        }
    }
}

/// Geometry or adapter field retained in a structured failure record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewGeometryFailureField {
    Geometry(ViewGeometryField),
    Conversion(ViewGeometryConversionField),
}

/// Signed range context retained without formatting it into a message.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewGeometryFailureRange {
    pub value_milli: Option<i64>,
    pub min_milli: Option<i64>,
    pub max_milli: Option<i64>,
}

/// Rectangle endpoints retained even when the source rectangle is inverted.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewGeometryFailureRect {
    pub left_milli: i32,
    pub top_milli: i32,
    pub right_milli: i32,
    pub bottom_milli: i32,
}

/// Generation context for stale and overflow failures.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewGeometryFailureGeneration {
    pub base: Option<super::cache::ViewGeometryGeneration>,
    pub current: super::cache::ViewGeometryGeneration,
}

/// Structured geometry failure suitable for adapters, reports, and traces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewGeometryFailure {
    pub code: ViewGeometryFailureCode,
    pub node: Option<ViewStyleNodeKey>,
    pub axis: Option<ViewPhysicalAxis>,
    pub field: Option<ViewGeometryFailureField>,
    pub operation: Option<ViewGeometryOperation>,
    pub range: Option<ViewGeometryFailureRange>,
    pub rect: Option<ViewGeometryFailureRect>,
    pub consumer: Option<ViewGeometryConsumer>,
    pub value_bits: Option<u64>,
    pub index_value: Option<u64>,
    pub index_max: Option<u64>,
    pub generation: Option<ViewGeometryFailureGeneration>,
}

impl ViewGeometryFailure {
    fn new(code: ViewGeometryFailureCode) -> Self {
        Self {
            code,
            node: None,
            axis: None,
            field: None,
            operation: None,
            range: None,
            rect: None,
            consumer: None,
            value_bits: None,
            index_value: None,
            index_max: None,
            generation: None,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ViewGeometryTargetKey {
    kind: ViewGeometryProductKind,
    id: String,
}

impl ViewGeometryTargetKey {
    pub(crate) fn new(kind: ViewGeometryProductKind, id: String) -> Self {
        Self { kind, id }
    }

    #[allow(
        dead_code,
        reason = "the typed product discriminator is part of the crate-owned diagnostic contract"
    )]
    pub(crate) const fn kind(&self) -> ViewGeometryProductKind {
        self.kind
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum ViewGeometryProductError {
    #[error("node {node:?} owner {owner:?} is missing required {expected:?} product {target:?}")]
    MissingProductRecord {
        node: ViewStyleNodeKey,
        owner: ViewRuntimeGeometryOwner,
        expected: ViewGeometryProductKind,
        target: Option<ViewGeometryTargetKey>,
    },
    #[error("node {node:?} owner {owner:?} has {count} {expected:?} records for {target:?}")]
    DuplicateProductRecord {
        node: ViewStyleNodeKey,
        owner: ViewRuntimeGeometryOwner,
        expected: ViewGeometryProductKind,
        target: ViewGeometryTargetKey,
        count: usize,
    },
    #[error("node {node:?} owner {owner:?} expected {expected:?}, but {target:?} is {actual:?}")]
    OwnerProductMismatch {
        node: ViewStyleNodeKey,
        owner: ViewRuntimeGeometryOwner,
        expected: Option<ViewGeometryProductKind>,
        actual: ViewGeometryProductKind,
        target: ViewGeometryTargetKey,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ViewGeometryTreeRelation {
    DuplicateNode,
    MissingParent {
        parent: ViewStyleNodeKey,
    },
    Cycle {
        repeated: ViewStyleNodeKey,
    },
    CrossMountParent {
        parent: ViewStyleNodeKey,
    },
    CallAttachmentMismatch {
        expected_call: ViewStyleNodeKey,
        actual_parent: Option<ViewStyleNodeKey>,
    },
    LeafHasChildren {
        first_child: ViewStyleNodeKey,
    },
    MissingNestedRoot {
        call: ViewStyleNodeKey,
    },
    DuplicateTarget {
        target: ViewGeometryTargetKey,
        first: ViewStyleNodeKey,
        second: ViewStyleNodeKey,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[allow(
    private_interfaces,
    reason = "the public top-level error preserves crate-owned inventory details without exporting inventory APIs"
)]
pub enum ViewGeometryRuntimeError {
    #[error("node {node:?} Style projection failed: {source}")]
    Projection {
        node: ViewStyleNodeKey,
        #[source]
        source: Box<ViewRuntimeStyleProjectionError>,
    },
    #[error("retained geometry tree failure for {node:?}: {relation:?}")]
    Tree {
        node: Option<ViewStyleNodeKey>,
        relation: Box<ViewGeometryTreeRelation>,
    },
    #[error(transparent)]
    Product(Box<ViewGeometryProductError>),
    #[error("node {node:?} owner {owner:?} intrinsic measurement failed: {source}")]
    Intrinsic {
        node: ViewStyleNodeKey,
        owner: ViewRuntimeGeometryOwner,
        #[source]
        source: ViewIntrinsicGeometryError,
    },
    #[error(transparent)]
    Geometry(Box<ViewGeometryError>),
    #[error("geometry conversion failed for {node:?} and {consumer:?}: {source}")]
    Conversion {
        node: Option<ViewStyleNodeKey>,
        consumer: ViewGeometryConsumer,
        #[source]
        source: super::conversion::ViewGeometryConversionError,
    },
    #[error("prepared geometry generation {base:?} is stale; current is {current:?}")]
    StalePreparedGeneration {
        base: super::cache::ViewGeometryGeneration,
        current: super::cache::ViewGeometryGeneration,
    },
    #[error("geometry generation overflow at {current:?}")]
    GenerationOverflow {
        current: super::cache::ViewGeometryGeneration,
    },
}

impl ViewGeometryRuntimeError {
    pub fn geometry_failure(&self) -> ViewGeometryFailure {
        match self {
            Self::Projection { node, .. } => {
                failure_with_node(ViewGeometryFailureCode::Projection, node)
            }
            Self::Tree { node, relation } => {
                let code = match relation.as_ref() {
                    ViewGeometryTreeRelation::DuplicateNode => {
                        ViewGeometryFailureCode::TreeDuplicateNode
                    }
                    ViewGeometryTreeRelation::MissingParent { .. } => {
                        ViewGeometryFailureCode::TreeMissingParent
                    }
                    ViewGeometryTreeRelation::Cycle { .. } => ViewGeometryFailureCode::TreeCycle,
                    ViewGeometryTreeRelation::CrossMountParent { .. } => {
                        ViewGeometryFailureCode::TreeCrossMountParent
                    }
                    ViewGeometryTreeRelation::CallAttachmentMismatch { .. } => {
                        ViewGeometryFailureCode::TreeCallAttachmentMismatch
                    }
                    ViewGeometryTreeRelation::LeafHasChildren { .. } => {
                        ViewGeometryFailureCode::TreeLeafHasChildren
                    }
                    ViewGeometryTreeRelation::MissingNestedRoot { .. } => {
                        ViewGeometryFailureCode::TreeMissingNestedRoot
                    }
                    ViewGeometryTreeRelation::DuplicateTarget { .. } => {
                        ViewGeometryFailureCode::TreeDuplicateTarget
                    }
                };
                let mut failure = ViewGeometryFailure::new(code);
                failure.node.clone_from(node);
                failure
            }
            Self::Product(error) => match error.as_ref() {
                ViewGeometryProductError::MissingProductRecord { node, .. } => {
                    failure_with_node(ViewGeometryFailureCode::ProductMissing, node)
                }
                ViewGeometryProductError::DuplicateProductRecord { node, .. } => {
                    failure_with_node(ViewGeometryFailureCode::ProductDuplicate, node)
                }
                ViewGeometryProductError::OwnerProductMismatch { node, .. } => {
                    failure_with_node(ViewGeometryFailureCode::ProductOwnerMismatch, node)
                }
            },
            Self::Intrinsic { node, source, .. } => failure_with_node(
                match source {
                    ViewIntrinsicGeometryError::MissingIntrinsicMeasure => {
                        ViewGeometryFailureCode::IntrinsicMissingMeasure
                    }
                    ViewIntrinsicGeometryError::IntrinsicBoundsOverflow => {
                        ViewGeometryFailureCode::IntrinsicBoundsOverflow
                    }
                },
                node,
            ),
            Self::Geometry(error) => pure_geometry_failure(error.as_ref()),
            Self::Conversion {
                node,
                consumer,
                source,
            } => conversion_failure(node.as_ref(), *consumer, source),
            Self::StalePreparedGeneration { base, current } => {
                let mut failure =
                    ViewGeometryFailure::new(ViewGeometryFailureCode::StalePreparedGeneration);
                failure.generation = Some(ViewGeometryFailureGeneration {
                    base: Some(*base),
                    current: *current,
                });
                failure
            }
            Self::GenerationOverflow { current } => {
                let mut failure =
                    ViewGeometryFailure::new(ViewGeometryFailureCode::GenerationOverflow);
                failure.generation = Some(ViewGeometryFailureGeneration {
                    base: None,
                    current: *current,
                });
                failure
            }
        }
    }
}

impl From<ViewGeometryProductError> for ViewGeometryRuntimeError {
    fn from(error: ViewGeometryProductError) -> Self {
        Self::Product(Box::new(error))
    }
}

impl From<ViewGeometryError> for ViewGeometryRuntimeError {
    fn from(error: ViewGeometryError) -> Self {
        Self::Geometry(Box::new(error))
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "this exhaustive typed projection preserves every pure geometry error field without string parsing"
)]
fn pure_geometry_failure(error: &ViewGeometryError) -> ViewGeometryFailure {
    match error {
        ViewGeometryError::NegativeNonNegativeField {
            node,
            field,
            value_milli,
        } => {
            let mut failure =
                failure_with_node(ViewGeometryFailureCode::NegativeNonNegativeField, node);
            failure.field = Some(ViewGeometryFailureField::Geometry(*field));
            failure.range = Some(range_value(i64::from(*value_milli)));
            failure
        }
        ViewGeometryError::ConflictingConstraints {
            node,
            axis,
            min_milli,
            max_milli,
        } => {
            let mut failure =
                failure_with_node(ViewGeometryFailureCode::ConflictingConstraints, node);
            failure.axis = Some(*axis);
            failure.range = Some(ViewGeometryFailureRange {
                value_milli: None,
                min_milli: Some(i64::from(*min_milli)),
                max_milli: Some(i64::from(*max_milli)),
            });
            failure
        }
        ViewGeometryError::EdgesExceedUsedBorderBox {
            node,
            axis,
            used_milli,
            edges_milli,
        } => {
            let mut failure =
                failure_with_node(ViewGeometryFailureCode::EdgesExceedUsedBorderBox, node);
            failure.axis = Some(*axis);
            failure.range = Some(ViewGeometryFailureRange {
                value_milli: i64::try_from(*edges_milli).ok(),
                min_milli: Some(0),
                max_milli: Some(i64::from(*used_milli)),
            });
            failure
        }
        ViewGeometryError::ArithmeticOverflow {
            node,
            axis,
            operation,
        } => {
            let mut failure = failure_with_node(ViewGeometryFailureCode::ArithmeticOverflow, node);
            failure.axis = *axis;
            failure.operation = Some(*operation);
            failure
        }
        ViewGeometryError::InvertedSpan {
            start_milli,
            end_milli,
        } => {
            let mut failure = ViewGeometryFailure::new(ViewGeometryFailureCode::InvertedSpan);
            failure.range = Some(ViewGeometryFailureRange {
                value_milli: None,
                min_milli: Some(i64::from(*start_milli)),
                max_milli: Some(i64::from(*end_milli)),
            });
            failure
        }
        ViewGeometryError::InvertedRect {
            left_milli,
            top_milli,
            right_milli,
            bottom_milli,
        } => {
            let mut failure = ViewGeometryFailure::new(ViewGeometryFailureCode::InvertedRect);
            failure.rect = Some(ViewGeometryFailureRect {
                left_milli: *left_milli,
                top_milli: *top_milli,
                right_milli: *right_milli,
                bottom_milli: *bottom_milli,
            });
            failure
        }
        ViewGeometryError::InvertedMarginSpan {
            node,
            axis,
            border_extent_milli,
            ..
        } => {
            let mut failure = failure_with_node(ViewGeometryFailureCode::InvertedMarginSpan, node);
            failure.axis = Some(*axis);
            failure.range = Some(range_value(i64::from(*border_extent_milli)));
            failure
        }
        ViewGeometryError::InvertedMarginBox {
            node, border_box, ..
        } => {
            let mut failure = failure_with_node(ViewGeometryFailureCode::InvertedMarginBox, node);
            failure.rect = Some(rect_failure(*border_box));
            failure
        }
        ViewGeometryError::InsetOnStatic { node, axis } => {
            failure_with_axis(ViewGeometryFailureCode::InsetOnStatic, node, *axis)
        }
        ViewGeometryError::OverConstrainedRelativeAxis { node, axis } => failure_with_axis(
            ViewGeometryFailureCode::OverConstrainedRelativeAxis,
            node,
            *axis,
        ),
        ViewGeometryError::OverConstrainedPositionedAxis { node, axis } => failure_with_axis(
            ViewGeometryFailureCode::OverConstrainedPositionedAxis,
            node,
            *axis,
        ),
        ViewGeometryError::PositionedStretchConstraintViolation {
            node,
            axis,
            candidate_milli,
            min_milli,
            max_milli,
            ..
        } => {
            let mut failure = failure_with_axis(
                ViewGeometryFailureCode::PositionedStretchConstraintViolation,
                node,
                *axis,
            );
            failure.range = Some(ViewGeometryFailureRange {
                value_milli: Some(*candidate_milli),
                min_milli: min_milli.map(i64::from),
                max_milli: max_milli.map(i64::from),
            });
            failure
        }
        ViewGeometryError::ScrollOffsetOutOfRange {
            node,
            axis,
            current_milli,
            min_milli,
            max_milli,
        } => {
            let mut failure =
                failure_with_axis(ViewGeometryFailureCode::ScrollOffsetOutOfRange, node, *axis);
            failure.consumer = Some(ViewGeometryConsumer::Scroll);
            failure.range = Some(ViewGeometryFailureRange {
                value_milli: Some(i64::from(*current_milli)),
                min_milli: Some(i64::from(*min_milli)),
                max_milli: Some(i64::from(*max_milli)),
            });
            failure
        }
        ViewGeometryError::UnsupportedConsumer { node, consumer, .. } => {
            let mut failure = failure_with_node(ViewGeometryFailureCode::UnsupportedConsumer, node);
            failure.consumer = Some(*consumer);
            failure
        }
        ViewGeometryError::DisplayRequiresContainer { node, .. } => {
            failure_with_node(ViewGeometryFailureCode::DisplayRequiresContainer, node)
        }
        ViewGeometryError::ContainerStyleOnLeaf { node, .. } => {
            failure_with_node(ViewGeometryFailureCode::ContainerStyleOnLeaf, node)
        }
        ViewGeometryError::CrossAxisGapRequiresWrap {
            node, value_milli, ..
        } => {
            let mut failure =
                failure_with_node(ViewGeometryFailureCode::CrossAxisGapRequiresWrap, node);
            failure.range = Some(range_value(i64::from(*value_milli)));
            failure
        }
        ViewGeometryError::GapRequiresLinearFlow {
            node, value_milli, ..
        } => {
            let mut failure =
                failure_with_node(ViewGeometryFailureCode::GapRequiresLinearFlow, node);
            failure.range = Some(range_value(i64::from(*value_milli)));
            failure
        }
        ViewGeometryError::InvalidTree { node } => {
            failure_with_node(ViewGeometryFailureCode::InvalidTree, node)
        }
        ViewGeometryError::MissingIntrinsicMeasure { node } => {
            failure_with_node(ViewGeometryFailureCode::MissingIntrinsicMeasure, node)
        }
        ViewGeometryError::InvalidPointerCoordinate { value_bits, .. } => {
            let mut failure =
                ViewGeometryFailure::new(ViewGeometryFailureCode::InvalidPointerCoordinate);
            failure.value_bits = Some(*value_bits);
            failure
        }
    }
}

fn conversion_failure(
    outer_node: Option<&ViewStyleNodeKey>,
    outer_consumer: ViewGeometryConsumer,
    error: &ViewGeometryConversionError,
) -> ViewGeometryFailure {
    let (code, node, consumer, field) = match error {
        ViewGeometryConversionError::NonFiniteInput {
            node,
            consumer,
            field,
            ..
        } => (
            ViewGeometryFailureCode::ConversionNonFiniteInput,
            node,
            *consumer,
            *field,
        ),
        ViewGeometryConversionError::OutsideMilliRange {
            node,
            consumer,
            field,
            ..
        } => (
            ViewGeometryFailureCode::ConversionOutsideMilliRange,
            node,
            *consumer,
            *field,
        ),
        ViewGeometryConversionError::NegativeExtent {
            node,
            consumer,
            field,
            ..
        } => (
            ViewGeometryFailureCode::ConversionNegativeExtent,
            node,
            *consumer,
            *field,
        ),
        ViewGeometryConversionError::InexactF32 {
            node,
            consumer,
            field,
            ..
        } => (
            ViewGeometryFailureCode::ConversionInexactF32,
            node,
            *consumer,
            *field,
        ),
        ViewGeometryConversionError::IndexRange {
            node,
            consumer,
            field,
            ..
        } => (
            ViewGeometryFailureCode::ConversionIndexRange,
            node,
            *consumer,
            *field,
        ),
    };
    let mut failure = ViewGeometryFailure::new(code);
    failure.node = node.clone().or_else(|| outer_node.cloned());
    debug_assert_eq!(consumer, outer_consumer);
    failure.consumer = Some(consumer);
    failure.field = Some(ViewGeometryFailureField::Conversion(field));
    match error {
        ViewGeometryConversionError::NonFiniteInput { value_bits, .. } => {
            failure.value_bits = Some(*value_bits);
        }
        ViewGeometryConversionError::OutsideMilliRange {
            value_bits,
            min_milli,
            max_milli,
            ..
        } => {
            failure.value_bits = Some(*value_bits);
            failure.range = Some(ViewGeometryFailureRange {
                value_milli: None,
                min_milli: Some(*min_milli),
                max_milli: Some(*max_milli),
            });
        }
        ViewGeometryConversionError::NegativeExtent { value_milli, .. } => {
            failure.range = Some(range_value(*value_milli));
        }
        ViewGeometryConversionError::InexactF32 {
            value_milli,
            round_trip_milli,
            ..
        } => {
            failure.range = Some(ViewGeometryFailureRange {
                value_milli: Some(*value_milli),
                min_milli: Some(*round_trip_milli),
                max_milli: Some(*round_trip_milli),
            });
        }
        ViewGeometryConversionError::IndexRange { value, max, .. } => {
            failure.index_value = Some(*value);
            failure.index_max = Some(*max);
        }
    }
    failure
}

fn failure_with_node(
    code: ViewGeometryFailureCode,
    node: &ViewStyleNodeKey,
) -> ViewGeometryFailure {
    let mut failure = ViewGeometryFailure::new(code);
    failure.node = Some(node.clone());
    failure
}

fn failure_with_axis(
    code: ViewGeometryFailureCode,
    node: &ViewStyleNodeKey,
    axis: ViewPhysicalAxis,
) -> ViewGeometryFailure {
    let mut failure = failure_with_node(code, node);
    failure.axis = Some(axis);
    failure
}

const fn range_value(value_milli: i64) -> ViewGeometryFailureRange {
    ViewGeometryFailureRange {
        value_milli: Some(value_milli),
        min_milli: None,
        max_milli: None,
    }
}

const fn rect_failure(rect: arcweft_view::geometry::ViewGeometryRect) -> ViewGeometryFailureRect {
    ViewGeometryFailureRect {
        left_milli: rect.left_milli,
        top_milli: rect.top_milli,
        right_milli: rect.right_milli,
        bottom_milli: rect.bottom_milli,
    }
}

impl From<ViewFinalGeometryLoweringError> for ViewGeometryRuntimeError {
    fn from(error: ViewFinalGeometryLoweringError) -> Self {
        match error {
            ViewFinalGeometryLoweringError::InexactF32 {
                node,
                consumer,
                field,
                value_milli,
                round_trip_milli,
            } => {
                let outer_node = Some(node.clone());
                Self::Conversion {
                    node: outer_node.clone(),
                    consumer,
                    source: ViewGeometryConversionError::InexactF32 {
                        node: outer_node,
                        platform: ViewGeometryPlatform::Wgpu,
                        consumer,
                        field: conversion_field(field),
                        value_milli,
                        round_trip_milli,
                    },
                }
            }
            ViewFinalGeometryLoweringError::IndexRange {
                node,
                consumer,
                field,
                value,
                max,
            } => Self::Conversion {
                node: node.clone(),
                consumer,
                source: ViewGeometryConversionError::IndexRange {
                    node,
                    platform: ViewGeometryPlatform::Wgpu,
                    consumer,
                    field: conversion_field(field),
                    value,
                    max,
                },
            },
        }
    }
}

const fn conversion_field(field: ViewFinalGeometryField) -> ViewGeometryConversionField {
    match field {
        ViewFinalGeometryField::Left => ViewGeometryConversionField::Left,
        ViewFinalGeometryField::Top => ViewGeometryConversionField::Top,
        ViewFinalGeometryField::Right => ViewGeometryConversionField::Right,
        ViewFinalGeometryField::Bottom => ViewGeometryConversionField::Bottom,
        ViewFinalGeometryField::Clip => ViewGeometryConversionField::Clip,
        ViewFinalGeometryField::Raster => ViewGeometryConversionField::Raster,
        ViewFinalGeometryField::IndexRange => ViewGeometryConversionField::IndexRange,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ViewGeometryConversionError, ViewGeometryConversionField, ViewGeometryFailureCode,
        ViewGeometryFailureField, ViewGeometryPlatform, ViewGeometryRuntimeError,
    };
    use arcweft_view::ViewMountId;
    use arcweft_view::geometry::{ViewGeometryConsumer, ViewGeometryError};
    use arcweft_view::style::{ViewPhysicalAxis, ViewStyleNodeKey};

    fn node() -> ViewStyleNodeKey {
        ViewStyleNodeKey::new(ViewMountId::from_raw(9), vec![3, 5], 7)
    }

    #[test]
    fn structured_scroll_failure_retains_node_axis_range_and_consumer() {
        let node = node();
        let failure = ViewGeometryRuntimeError::Geometry(Box::new(
            ViewGeometryError::ScrollOffsetOutOfRange {
                node: node.clone(),
                axis: ViewPhysicalAxis::Y,
                current_milli: -9_000,
                min_milli: -8_000,
                max_milli: 12_000,
            },
        ))
        .geometry_failure();

        assert_eq!(
            failure.code,
            ViewGeometryFailureCode::ScrollOffsetOutOfRange
        );
        assert_eq!(
            failure.code.as_str(),
            "view.geometry.scroll_offset_out_of_range"
        );
        assert_eq!(failure.node, Some(node));
        assert_eq!(failure.axis, Some(ViewPhysicalAxis::Y));
        assert_eq!(failure.consumer, Some(ViewGeometryConsumer::Scroll));
        let range = failure.range.expect("scroll range is retained");
        assert_eq!(range.value_milli, Some(-9_000));
        assert_eq!(range.min_milli, Some(-8_000));
        assert_eq!(range.max_milli, Some(12_000));
    }

    #[test]
    fn structured_conversion_failure_retains_exact_index_context() {
        let node = node();
        let failure = ViewGeometryRuntimeError::Conversion {
            node: Some(node.clone()),
            consumer: ViewGeometryConsumer::Capture,
            source: ViewGeometryConversionError::IndexRange {
                node: Some(node.clone()),
                platform: ViewGeometryPlatform::Headless,
                consumer: ViewGeometryConsumer::Capture,
                field: ViewGeometryConversionField::IndexRange,
                value: u64::from(u32::MAX) + 1,
                max: u64::from(u32::MAX),
            },
        }
        .geometry_failure();

        assert_eq!(failure.code, ViewGeometryFailureCode::ConversionIndexRange);
        assert_eq!(failure.node, Some(node));
        assert_eq!(failure.consumer, Some(ViewGeometryConsumer::Capture));
        assert_eq!(
            failure.field,
            Some(ViewGeometryFailureField::Conversion(
                ViewGeometryConversionField::IndexRange,
            ))
        );
        assert_eq!(failure.index_value, Some(u64::from(u32::MAX) + 1));
        assert_eq!(failure.index_max, Some(u64::from(u32::MAX)));
    }
}
