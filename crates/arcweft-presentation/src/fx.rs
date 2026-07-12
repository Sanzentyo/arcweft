//! Typed, renderer-independent presentation treatment graphs and evaluation.
//!
//! This module is the Sans I/O boundary shared by authored Fx definitions,
//! View and rich-text applications, renderer providers, save state, and Agent
//! diagnostics.  Backends consume resolved operations; they do not implement
//! their own sampler arithmetic.

pub mod application;
pub mod capability;
pub mod diagnostic;
mod evaluator;
pub mod graph;
mod graph_evaluator;
pub mod identity;
pub mod plan;
pub mod program;
pub mod provider;
pub mod render_resource;
pub mod state;
pub mod value;

pub use application::{
    FxApplication, FxApplicationError, FxApplicationResolver, FxEvaluationBinding,
};
pub use capability::{
    FxCapability, FxCapabilitySet, FxPhase, FxRendererInterface, FxRendererInterfaceSet, FxTarget,
};
pub use diagnostic::{
    FxDiagnostic, FxDiagnosticCode, FxDiagnosticContext, FxDiagnosticSeverity, FxSourceRange,
};
pub use graph::{
    FX_MAX_DEFINITIONS_PER_SECTION, FX_MAX_GRAPH_DEPTH, FX_MAX_GRAPH_NODES_PER_DEFINITION,
    FX_MAX_PARAMETERS_PER_DEFINITION, FX_MAX_TOTAL_GRAPH_NODES_PER_SECTION, FxDefinition,
    FxDefinitionError, FxGraph, FxGraphError, FxNode, FxNodeKind, FxParameter, FxParameterSlot,
    FxProperty, FxResourceId, FxStaticType, FxStaticValue,
};
pub use graph_evaluator::{FxGraphEvaluator, FxTargetSample};
pub use identity::{
    FxAbiHash, FxId, FxIdError, FxInstanceId, FxPackageId, FxQualifiedName, FxSemanticHash,
};
pub use plan::{
    FxInteractionGeometry, FxNamedValue, FxResolvedValue, ResolvedFxOperation, ResolvedFxPlan,
    ResolvedTransformOperation, ResolvedValueOperation,
};
pub use program::{
    FX_DEFAULT_EVALUATOR_OPERATIONS, FX_MAX_CAPTURED_OR_PARAMETER_SLOTS,
    FX_MAX_CONSTANTS_PER_SAMPLER, FX_MAX_INSTRUCTIONS_PER_SAMPLER, FX_MAX_STACK_VALUES_PER_PROGRAM,
    FxContextSlot, FxEvaluationBudget, FxEvaluationError, FxSamplerProgram, ValidatedValueProgram,
    ValueInstruction, ValueProgramInputs, ValueProgramLimits, ValueProgramSchema,
    ValueProgramValidationError,
};
pub use provider::{
    FxProvider, FxProviderDescriptor, FxProviderError, FxProviderKind, FxProviderLimits,
    FxProviderOutput, FxProviderRegistry, FxProviderRequest,
};
pub use render_resource::{
    FxRenderProgram, FxRenderResourceError, FxRenderResourceTable, ResolvedFxDisplacementKind,
    ResolvedFxGlyphPass, ResolvedFxMask, ResolvedFxOffscreenPass, ResolvedFxPostProcess,
    ResolvedFxResourceOutput,
};
pub use state::{
    FX_MAX_GRAPH_CHILD_DEPTH, FX_MAX_PROVIDER_STATE_VALUES, FX_MAX_PROVIDER_STATES_PER_INSTANCE,
    FxGraphChildPath, FxInstanceSnapshot, FxInstanceSnapshotError, FxLogicalTime,
    FxProviderStateRecord, FxSampleContext, FxSampleGeometry, derive_deterministic_seed,
};
pub use value::{
    Angle, FX_GOLDEN_ANGLE_RAD, FiniteF32, FiniteF32Error, FxColor, FxRuntimeType, FxRuntimeValue,
    FxVec2, Length, Opacity, ResolvedTransform2D, Seconds, Transform2D, Transform2DError,
};

#[cfg(test)]
mod tests;
