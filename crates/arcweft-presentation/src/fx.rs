//! Typed, renderer-independent presentation treatment graphs and evaluation.
//!
//! This module is the Sans I/O boundary shared by authored Fx definitions,
//! View and rich-text applications, renderer providers, save state, and Agent
//! diagnostics.  Backends consume resolved operations; they do not implement
//! their own sampler arithmetic.

pub mod application;
pub mod builtin;
mod canonical;
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
pub mod uniform;
pub mod value;

pub use application::{
    FxApplication, FxApplicationDraft, FxApplicationError, FxApplicationResolver,
    FxBoundApplicationTemplate, FxEvaluationBinding, FxStaticDefinitionArgumentValue,
};
pub use builtin::{
    BUILTIN_FX_CALLABLE_CATALOG, BUILTIN_FX_CALLABLE_SCHEMA_VERSION, BuiltinFxAbiProjection,
    BuiltinFxActiveAbiParameterSet, BuiltinFxApplicationBindingPlan, BuiltinFxApplicationParameter,
    BuiltinFxArgument, BuiltinFxBuildError, BuiltinFxCallableCatalog, BuiltinFxCallableId,
    BuiltinFxCallableParameter, BuiltinFxCallableRow, BuiltinFxCallableRowId,
    BuiltinFxCallableSchemaDigest, BuiltinFxDefaultValue, BuiltinFxDefinitionTemplate,
    BuiltinFxGraphArgument, BuiltinFxNumericConstraint, BuiltinFxParameterBinding,
    BuiltinFxParameterId, BuiltinFxParameterPassing, BuiltinFxParameterPredicate,
    BuiltinFxParameterPresence, BuiltinFxParameterType, BuiltinFxSpecialization, BuiltinFxUnit,
    BuiltinFxValueConstraint, build_builtin_fx_definition, build_builtin_fx_graph,
};
pub use canonical::FxCanonicalDecodeError;
pub use capability::{
    FX_CLOSED_ENUM_DOMAINS, FX_MAX_SELECTOR_NAME_BYTES, FxCapability, FxCapabilitySet,
    FxEnumDomain, FxPhase, FxRendererInterface, FxRendererInterfaceSet, FxSelectorDomain,
    FxSelectorId, FxSelectorName, FxSelectorNameError, FxShaderStage, FxTarget, MotionFunction,
};
pub use diagnostic::{
    FxDiagnostic, FxDiagnosticCode, FxDiagnosticContext, FxDiagnosticSeverity, FxSourceRange,
};
pub use graph::{
    FX_MAX_DEFINITION_CANONICAL_BYTES, FX_MAX_DEFINITION_PARAMETER_NAME_BYTES,
    FX_MAX_DEFINITIONS_PER_SECTION, FX_MAX_FONT_FAMILY_NAME_BYTES,
    FX_MAX_GRAPH_CHILD_EDGES_PER_DEFINITION, FX_MAX_GRAPH_DEPTH, FX_MAX_GRAPH_NODES_PER_DEFINITION,
    FX_MAX_PARAMETERS_PER_DEFINITION, FX_MAX_RESOURCE_ID_BYTES,
    FX_MAX_TOTAL_GRAPH_NODES_PER_SECTION, FxDefinition, FxDefinitionArgumentValue,
    FxDefinitionDecodeError, FxDefinitionError, FxDefinitionGraphContext, FxDefinitionParameter,
    FxDefinitionParameterIndex, FxDefinitionParameterLayout, FxDefinitionParameterLayoutDigest,
    FxDefinitionParameterLayoutRow, FxDefinitionParameterName, FxDefinitionParameterNameError,
    FxDefinitionParameterRef, FxDefinitionParameterSchema, FxDefinitionParameterSchemaDigest,
    FxDefinitionParameterType, FxFontFamilyName, FxFontFamilyNameError, FxGraph, FxGraphError,
    FxNode, FxNodeKind, FxParameterStorageSlot, FxProperty, FxPropertyId, FxResourceId,
    FxResourceIdError, FxRuntimeParameterLayoutRow, FxRuntimeParameterRef, FxRuntimeParameterSlot,
    FxSourceConstructor, FxSourceParameter, FxSourceParameterPassing, FxSourceParameterPresence,
    FxSourceParameterRole, FxSourceParameterType, FxStaticParameterLayoutRow,
    FxStaticParameterSlot, FxStaticType, FxStaticValue,
};
pub use graph_evaluator::{FxGraphEvaluator, FxTargetSample};
pub use identity::{
    FX_MAX_PACKAGE_ID_BYTES, FX_MAX_PACKAGE_ID_SEGMENTS, FX_MAX_QUALIFIED_NAME_BYTES,
    FX_MAX_QUALIFIED_NAME_SEGMENTS, FxAbiHash, FxId, FxIdCanonicalDecodeError, FxIdError,
    FxIdentitySemanticDigest, FxInstanceId, FxInstanceIdentity, FxInstanceOwnerKey, FxPackageId,
    FxPackageIdError, FxQualifiedName, FxQualifiedNameError, FxSemanticHash,
};
pub use plan::{
    FxInteractionGeometry, FxResolvedValue, FxRuntimeOperationOpcode, FxShaderUniform,
    ResolvedColorOperation, ResolvedFilterOperation, ResolvedFxOperation, ResolvedFxPlan,
    ResolvedMaskOperation, ResolvedOffscreenPassOperation, ResolvedPostProcessOperation,
    ResolvedShaderUniformOperation, ResolvedTextStyleOperation, ResolvedTransformOperation,
    ResolvedTransitionOperation,
};
pub use program::{
    FX_DEFAULT_EVALUATOR_OPERATIONS, FX_MAX_CAPTURED_OR_PARAMETER_SLOTS,
    FX_MAX_CONSTANTS_PER_SAMPLER, FX_MAX_INSTRUCTIONS_PER_SAMPLER, FX_MAX_STACK_VALUES_PER_PROGRAM,
    FxContextSlot, FxEvaluationBudget, FxEvaluationError, FxSamplerProgram,
    FxSamplerProgramCanonicalError, FxSamplerProgramDecodeError, ValidatedValueProgram,
    ValueInstruction, ValueProgramInputs, ValueProgramLimits, ValueProgramSchema,
    ValueProgramSemanticDigestError, ValueProgramValidationError,
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
    FxAuthoredSeed, FxGraphChildPath, FxInstanceActivation, FxInstanceSnapshot,
    FxInstanceSnapshotError, FxLogicalTime, FxProviderStateRecord, FxSampleContext,
    FxSampleGeometry, derive_deterministic_seed,
};
pub use uniform::{
    FX_MAX_UNIFORM_FIELDS, FX_MAX_UNIFORM_NAME_BYTES, FX_MAX_UNIFORM_RECORD_CANONICAL_BYTES,
    FxUniformField, FxUniformName, FxUniformParameterRef, FxUniformProgram, FxUniformRecord,
    FxUniformRecordDecodeError, FxUniformType, FxUniformValue,
};
pub use value::{
    Angle, FX_GOLDEN_ANGLE_RAD, FiniteF32, FiniteF32Error, FxColor, FxRuntimeType, FxRuntimeValue,
    FxRuntimeValueDecodeError, FxVec2, Length, Opacity, ResolvedTransform2D, Seconds, Transform2D,
    Transform2DError,
};

#[cfg(test)]
mod tests;
