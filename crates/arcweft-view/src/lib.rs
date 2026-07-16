//! Sans I/O View entity and fragment data for Arcweft presentation.

pub mod dialogue;
pub mod display;
pub mod entity;
pub mod fragment;
pub mod frame;
pub mod fx;
pub mod geometry;
pub mod handler;
pub mod image;
pub mod layout;
pub mod motion;
pub mod part;
pub mod presentation_image;
pub mod program;
pub mod reactive;
pub mod semantics;
pub mod style;
pub mod text_field;
pub mod text_source;
pub mod value_program;
pub mod view;
pub mod virtualization;

use thiserror::Error;

pub use dialogue::{
    DialogueAdvanceTarget, DialogueEntryId, DialogueInstanceId, DialoguePresentationId,
    DialogueRevision, DialogueStageIndex,
};
pub use display::{
    DisplayItem, DisplayItemId, DisplayItemKind, DisplayList, ResolvedDisplayItem,
    ResolvedDisplayList,
};
pub use entity::{DirtyFlags, Entity, EntityStore, RawEntity};
pub use fragment::{
    ContainerKind, CustomElementId, EventBinding, EventKind, FragmentKind, FragmentNode, HandlerId,
    ImageId, NodeId, RichTextSourceId, SemanticSpecId, Span32, TextSourceId, ViewFragment,
    ViewFragmentBuilder,
};
pub use frame::ViewLayerOutput;
pub use fx::{
    RetainedViewFxApplication, RetainedViewFxTable, ViewFxArgumentBinding, ViewFxError,
    ViewFxIdentity, ViewFxOrdinal,
};
pub use handler::{ViewHandlerInvocation, ViewHandlerRoute, ViewHandlerRouteTable};
pub use image::{
    ImageAlignment, ImageFit, ImagePlayback, ViewImagePresentationMetadata, ViewImageSource,
    ViewImageSourceTable, ViewResolvedImageFrame,
};
pub use layout::{
    LayoutBox, LayoutKind, LayoutLength, LayoutNode, LayoutPoint, LayoutResults, LayoutSize,
    LayoutTree,
};
pub use motion::{
    ViewCubicBezier, ViewEasingFunction, ViewKeyframe, ViewKeyframeTrack, ViewMotionError,
    ViewMotionSample, ViewReducedMotionPolicy, ViewStepPosition, ViewTimelineMillis,
    ViewTransition, ViewTransitionSpec,
};
pub use part::{
    ViewEvaluationSiteId, ViewInstructionIndex, ViewPartExport, ViewPartId,
    ViewPartInstructionKind, ViewPartLocalName, ViewPartName, ViewPartStaticReachability,
    ViewProgramBuildError, ViewStaticPart,
};
pub use presentation_image::{ViewImagePresentationFrame, ViewImagePresentationInput};
pub use program::{
    ViewBranch, ViewCall, ViewCallArgument, ViewCustomSpec, ViewElementKind, ViewElementLayoutKind,
    ViewElementSpec, ViewElementTextInputKind, ViewEventBindingSpec, ViewHandlerProgram,
    ViewImageSpec, ViewInstruction, ViewInstructionRange, ViewProgram, ViewProgramBuilder,
    ViewRepeat, ViewSemanticSpec, ViewStableKey, ViewTextSpec,
};
pub use reactive::{EntityInvalidation, ReactiveGraph, ReactiveInvalidation, Revision};
pub use semantics::{
    ViewNodeId, ViewSemanticFragment, ViewSemanticFragmentBuilder, ViewSemanticNode,
};
pub use style::{
    ComputedViewAxes, ComputedViewProperty, ComputedViewStyle, ComputedViewStyleBuilder,
    ComputedViewStyleRevision, ComputedViewTransition, PropertyBinding, PropertyBindingTable,
    PropertyBindingTableBuilder, ValueSourceId, ViewAlignment, ViewAngleMilliDegrees,
    ViewAxisProviderParticipation, ViewAxisSign, ViewAxisUsageSet, ViewAxisValueError,
    ViewBlendMode, ViewBorderRadii, ViewBoxAxisHostSeed, ViewBoxAxisMode, ViewBoxAxisModeError,
    ViewBoxAxisRevision, ViewBoxAxisSeedGeneration, ViewBoxAxisSeedGenerationError,
    ViewBoxAxisSeedSource, ViewBoxAxisSource, ViewClip, ViewColorValue, ViewComputedPropertyKind,
    ViewContainerAxis, ViewContainerPredicate, ViewDisplay, ViewElementState, ViewElementStateSet,
    ViewEnvironmentPredicate, ViewFilter, ViewFlexDirection, ViewFlexWrap, ViewFontFamily,
    ViewFontFamilyList, ViewFontStyle, ViewFontWeight, ViewInheritedBoxAxes,
    ViewInteractionSelector, ViewInteractionStateSet, ViewLengthMilli, ViewMask, ViewOverflow,
    ViewPhysicalAxis, ViewPhysicalBoxStyle, ViewPhysicalContainerStyle, ViewPhysicalEdges,
    ViewPhysicalFlow, ViewPhysicalSide, ViewPosition, ViewPropertyExpansion, ViewPropertyId,
    ViewPropertyKind, ViewPropertyResolution, ViewPropertyValueTransform, ViewRatioMilli,
    ViewResolvedAxis, ViewResolvedBoxAxes, ViewScalarMilli, ViewShadow, ViewSpecifiedValue,
    ViewStyleApplication, ViewStyleApplicationTarget, ViewStyleAssignOp, ViewStyleBoundaryFacts,
    ViewStyleCombinator, ViewStyleComparison, ViewStyleContribution, ViewStyleContributionSource,
    ViewStyleDeclaration, ViewStyleInvalidationSet, ViewStyleModelError, ViewStyleNodeFacts,
    ViewStyleNodeKey, ViewStylePatch, ViewStylePatchId, ViewStylePredicate, ViewStylePriority,
    ViewStyleProgram, ViewStyleResolution, ViewStyleResolveContext, ViewStyleResolveError,
    ViewStyleResolver, ViewStyleResolverLimits, ViewStyleRevisionSet, ViewStyleRule,
    ViewStyleScopeId, ViewStyleSelector, ViewStyleSelectorSequence, ViewStyleSheet,
    ViewStyleSheetId, ViewStyleSourceId, ViewStyleSpecificity, ViewStyleToken, ViewStyleTokenId,
    ViewStyleTrace, ViewStyleTraceEntry, ViewStyleTraceMode, ViewStyleTraceRejection,
    ViewStyleTransition, ViewStyleValueKind, ViewSystemFontFamily,
};
pub use text_field::{
    ExternalTextUpdatePolicy, TextEditError, TextEditOutcome, TextEditState, TextEditorMode,
    TextEditorPart, TextFieldBindingCommitPolicy, TextFieldEditPolicy, TextFieldGeometryPolicy,
    TextFieldId, TextFieldMetrics, TextFieldPartId, TextFieldPartRect, TextFieldPolicyEditError,
    TextFieldSpec, TextFieldVisualBuffer,
};
pub use text_source::{ViewRichTextHandle, ViewTextByteRange, ViewTextSource, ViewTextSourceTable};
pub use value_program::{
    ViewMountSnapshot, ViewMountState, ViewValueEvaluation, ViewValueEvaluationError,
    ViewValueEvaluationStatus, ViewValueInventoryError, ViewValueProgram, ViewValueProgramId,
    ViewValueProgramInventory, ViewValueSlotSnapshot,
};
pub use view::{
    RustViewId, ViewDescriptor, ViewId, ViewImplementation, ViewMountAllocationError,
    ViewMountAllocator, ViewMountId, ViewProgramId, ViewRegistry, ViewSchemaId,
};

/// Stable key for one retained View fragment node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeKey(pub u64);

/// Error while building or updating View state.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ViewError {
    #[error("duplicate View node key {0:?}")]
    DuplicateNodeKey(NodeKey),
    #[error("duplicate view public id {0}")]
    DuplicateViewPublicId(arcweft_id::PublicId),
    #[error("stale View entity {0:?}")]
    StaleEntity(RawEntity),
    #[error("View entity has a different state type: {0:?}")]
    EntityTypeMismatch(RawEntity),
    #[error("invalid View fragment node {0:?}")]
    InvalidFragmentNode(NodeId),
    #[error("View fragment node {0:?} has multiple parents")]
    MultipleFragmentParents(NodeId),
    #[error("missing layout for View fragment node {0:?}")]
    MissingLayout(NodeId),
    #[error("duplicate View property binding {0:?}")]
    DuplicatePropertyBinding(ViewPropertyId),
    #[error("duplicate View image source {0:?}")]
    DuplicateImageSource(ImageId),
    #[error("unknown View image source {0:?}")]
    UnknownImageSource(ImageId),
    #[error("View node {0:?} binds an event without semantic target metadata")]
    HandlerNodeMissingSemantics(NodeId),
    #[error("View node {node:?} references unknown handler semantic {semantic:?}")]
    UnknownHandlerSemantic {
        node: NodeId,
        semantic: SemanticSpecId,
    },
    #[error("View node {node:?} references unknown display semantic {semantic:?}")]
    UnknownDisplaySemantic {
        node: NodeId,
        semantic: SemanticSpecId,
    },
    #[error(transparent)]
    StyleResolution(#[from] ViewStyleResolveError),
    #[error("too many View items")]
    CapacityExceeded,
}

#[cfg(test)]
mod tests;
