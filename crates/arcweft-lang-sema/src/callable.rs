//! Typed callable identities, schemas, catalogs, and semantic query results.
//!
//! This module owns the common in-memory contract used by accepted-world
//! registration, type checking, and semantic call queries. It intentionally
//! exposes immutable read models while keeping catalog construction and
//! resolver mutation crate-private.

mod application;
mod arguments;
mod builder;
mod catalog;
mod checked_application;
mod checked_catalog;
mod constraints;
mod continuation;
mod dialogue;
mod digest;
mod error;
mod facts;
mod identity;
mod join;
mod limits;
mod nominal_signature;
mod presentation;
mod projection;
mod publication;
mod resolver;
mod schema;
mod view_modifier;

pub use crate::types::StandardMapFamily;
pub use crate::types::{CheckedConstraintContainerConstructor, CheckedConstraintSourceProjection};
pub(crate) use application::{
    DetachedPreparedCallableApplication, PreparedCallableApplication,
    PreparedCallableApplicationReplayMismatch,
};
pub use arguments::CallableParameterCoordinate;
pub(crate) use arguments::{
    CallableRestContainerPolicy, MappedCallArgumentPassing, MappedCallArgumentSlot,
    PreparedArgumentSourceProjection, PreparedCallArgumentMapping,
    PreparedDialogueApplicationMetadataArgument, PreparedDialogueApplicationMetadataEvidence,
    PreparedDialogueApplicationMetadataInventory, map_call_arguments,
};
pub(crate) use builder::RegisteredCallableCatalogBuilder;
pub use catalog::{
    CallableAccess, CallableRecord, CatalogCallableEntry, EnvironmentCallableCatalog,
    EnvironmentDeclarationOrdinal, EquivalentCallableSource, NonEmptyCallableSet,
    ProjectCallableCatalog, RegisteredCallableCatalog, RegisteredProjectModuleCallables,
};
pub use checked_application::{
    CallableParameterAlternativeIndex, CheckedCallApplication, CheckedCallApplicationCore,
    CheckedCallApplicationCoreDigest, CheckedCallApplicationDigest, CheckedCallApplicationSite,
    CheckedCallArgumentPassing, CheckedCallCalleeExecution, CheckedCallCandidateInventoryDigest,
    CheckedCallContinuation, CheckedCallContinuationDigest, CheckedCallEffectBinding,
    CheckedCallExecutionArgument, CheckedCallExecutionProjection, CheckedCallExecutionSlot,
    CheckedCallExecutionSource, CheckedCallOperandDestination, CheckedCallReceiverProjection,
    CheckedCallResult, CheckedCallRuntimeOperand, CheckedCallRuntimeOperandOrder,
    CheckedCallSemanticOperand, CheckedCallSemanticOperandSource, CheckedCallSemanticSelection,
    CheckedCandidateIndex, CheckedCandidateInventory, CheckedCapacityMethodIdentity,
    CheckedCapacityOperation, CheckedCaptureMode, CheckedCaptureSignatureRow,
    CheckedContentCallableCoordinate, CheckedDeferredContinuationConstParameter,
    CheckedDeferredContinuationParameter, CheckedDialogueCallableIdentity,
    CheckedDomainMethodIdentity, CheckedFunctionValueIdentity, CheckedLanguageCallableIdentity,
    CheckedLexicalCallableIdentity, FrozenCallTypeSolution, FrozenCallTypeSolutionDigest,
    ResolvedCallable, ResolvedCallableAuthority, ResolvedCallableBase,
    ResolvedCallableBaseInstantiation, ResolvedCallableDigest, ResolvedCallableIssuerEvidence,
    ResolvedCallableOrigin, ResolvedCallableStableIdentity, ResolvedCallableState,
    ResolvedDialogueCalleeIdentity,
};
pub(crate) use checked_application::{
    CheckedCallApplicationCoreSeal, CheckedCallExecutionArgumentSeal,
    CheckedCallExecutionProjectionSeal, CheckedCallExecutionSlotSeal, CheckedCallResultSeal,
    CheckedCallSemanticOperandSeal, CheckedCaptureSignatureSeal, PreparedCandidateIndex,
    ResolvedCallableBaseSeal, ResolvedCallableCheckedDefinition,
    ResolvedCallableStableIdentitySeal,
};
pub use checked_catalog::{
    CallableEffectContract, CallableInterfaceDigest, CheckedCallableCatalog,
    CheckedCallableCatalogGeneration, CheckedCallableCatalogOrigin, CheckedCallableEffects,
    CheckedCallableExecution, CheckedCallableFacts, CheckedCallableLookupError,
    CheckedCallableSourceCategory, CheckedCallableSourceKey, CheckedMethodLookup,
    EffectClauseSource, EffectContractBuildError, EffectContractOrigin, EffectContractSource,
    EffectItemSource, EffectPermission,
};
pub(crate) use checked_catalog::{CheckedCallableCatalogBuildError, CheckedCallableCatalogBuilder};
pub(crate) use constraints::{
    CandidateConstraintDriverStartFailure, CandidateConstraintWorkSession,
    PreparedSourceConstraintGroup, SourceCallbackFailure, SourceCheckpointFailure,
    TypeConstraintClient,
};
pub(crate) use continuation::{
    CallConstraintInvariant, EnclosingGenericParameterScope, PreparedCallContinuationAuthority,
    PreparedCallContinuationRef, PreparedCallGraph, PreparedCallGraphCheckpoint,
    PreparedCallGraphDelta, PreparedCallGraphIngress, PreparedCallGraphReplayMismatch,
    PreparedCallGraphSealAuthority, PreparedCallGraphSealNodeKey, PreparedCallGraphSealPayload,
    PreparedCallGraphSelectedNode, PreparedCallGraphSiteState, PreparedCallPrefixPayload,
    PreparedCallPrefixReplayMismatch, PreparedCallSiteContinuation,
    PreparedConstraintInitialization,
};
pub use continuation::{CheckedCallSite, PreparedCallGraphInvariant};
pub use dialogue::{
    CharacterDialoguePatchContext, DialogueCallableId, DialogueCallableResultContext,
    DialogueCalleeIdentity, DialogueSchemaContext,
};
pub use digest::{
    CallableSignatureSchemaDigest, EnvironmentCallablePublicationDigest,
    RegisteredCallableCatalogDigest,
};
pub use error::{
    BuiltinIdentityError, CallableBuildLimitError, CallableCatalogBuildError, CallableCatalogError,
    CallableDiagnosticCode, CallableDocumentationError, CallableFamilyInvariantCode,
    CallableIdentityError, CallableIndexKind, CallablePathError, CallablePublicationError,
    CallableQueryLimitError, CallableScalarError, CallableScalarKind, CallableSchemaError,
    CallableSourceError, CorruptCallableCatalogReason, ResolveCallError, RustProvenanceError,
    RustProvenanceField, SemanticSignatureError, SignatureLimitExceeded, SignatureLimitKind,
    SignatureWorkKind,
};
pub(crate) use facts::CallTargetFactsInput;
pub use facts::{
    CallAnalysisOutcome, CallCalleeClassificationFact, CallPoison, CallTargetFacts,
    CallableDiagnostic, CallableDiagnosticRelated, CallableDiagnosticSeverity,
    CallableDiagnosticSubject, CheckedAmbiguousCallEvidence, CheckedCallArgumentSlotSource,
    CheckedMissingCallEvidence, CheckedNonCallableEvidence, CheckedRejectedCallEvidence,
    SemanticParameter, SemanticParameterGroup, SemanticSignature, SemanticSignatureHelp,
    SemanticSignatureIndex, SemanticSignatureRecovery, SemanticSignatureSurface,
};
pub use identity::{
    AdapterPackageId, AgentIntrinsicSignatureId, BuiltinCallableId, CallableArgumentSlotIndex,
    CallableAuthorityRank, CallableCandidateId, CallableFamily, CallableGroupIndex,
    CallableLookupKey, CallableName, CallableOverloadIndex, CallableParameterIndex, CallablePath,
    CallableProviderId, CapabilityCallableId, CapacityMethodId, CheckedCallableContext,
    CheckedCallableDeclaration, CheckedCallableDigest, CheckedCallableId,
    CheckedCallableIdentityError, CheckedClosureId, CheckedEffectCallableId, CollectionMethodId,
    DetachedCallableDeclarationId, DomainMethodId, DropCallableId, EnumVariantSignatureId,
    EnvironmentCallableDigest, EnvironmentCallableId, EnvironmentCallableKind,
    EnvironmentCallableOwner, FloatWidth, FunctionValueOrdinal, FunctionValueSignatureId,
    FxCallableSignatureId, FxResolution, IntegerMethodId, LanguageCallableFamily,
    LanguageDocumentationFamily, LineContextMethodId, LineScheduleCallableId, LocalCallableId,
    MathCallableId, OptionConstructorKind, PresentationHandleMethodId, ProbeComparisonId,
    ProbeComparisonOperator, ProjectCallablePath, ProjectNameBinding, PromotionCallableId,
    ReceiverMethodKey, ReductionConstructorKind, ResultConstructorKind, RustItemPath,
    STANDARD_TRAIT_CATALOG_VERSION, StageMethodId, StandardCallableDeclarationId,
    StandardEnvironmentId, StandardTraitCatalogVersion, StdFloatCallableId, StdFloatOperation,
    VectorDimensions,
};
pub(crate) use join::validate_selected_application;
pub use join::{
    CallableInstantiationDigest, CallableReceiverMode, CheckedCallableArgument,
    CheckedCallableArgumentSlot, CheckedCallableJoin, CheckedCallableJoinDigest,
    CheckedCallableJoinError, IntrinsicCallableCandidateTag,
};
pub use limits::{
    CallResolverAccountingReport, CallableLimits, PRODUCTION_CALLABLE_LIMITS,
    PRODUCTION_SIGNATURE_LIMITS, SignatureAccountingError, SignatureQueryLimits,
    SignatureQueryProjectionWork, SignatureQueryResolutionWork, SignatureQuerySearchWork,
    SignatureQueryWorkReport, SignatureWorkReport,
};
pub(crate) use limits::{
    CandidateConstraintSessionStartFailure, ResolverWork, SignatureQueryWorkMeter,
};
pub(crate) use nominal_signature::associated_scope_for;
pub(crate) use presentation::PresentationArgumentValuePolicy;
pub use presentation::{PresentationCallableId, PresentationSchemaContext};
pub use projection::{
    EnvironmentPublicationProjectionDiagnostic, EnvironmentPublicationProjectionErrorKind,
    EnvironmentPublicationProjectionReport, EnvironmentPublicationRelatedLabel,
    EnvironmentPublicationRelatedSource,
};
pub use publication::{EnvironmentCallablePublication, EnvironmentCallablePublicationRecord};
pub(crate) use resolver::{
    CallResolverAuthority, CallResolverContext, CallResolverRequest,
    CheckedCallableEffectInstantiation, DetachedPreparedResolvedCallable, FinalCallCalleeFacts,
    PrepareFinalCallCalleeError, PreparedCallCallee, PreparedCallCalleeConstraintInputs,
    PreparedCallInputProjection, PreparedCallableDefinitionKey,
    PreparedCallableEffectInstantiationEvidence, PreparedCaptureIdentityRow,
    PreparedDialogueCallConstraintInputs, PreparedDialogueCallOperandSource,
    PreparedDialogueCalleeIdentity, PreparedFunctionValueOriginEvidence,
    PreparedFunctionValueOriginIdentity, PreparedFunctionValueOriginProducer,
    PreparedFunctionValueOriginProgress, PreparedFunctionValueOriginQueryError,
    PreparedImplicitExtensionReceiver, PreparedResolvedCallable,
    PreparedResolvedCallableDefinition, PreparedResolvedCallableDefinitionBatch,
    PreparedResolvedCallableDefinitionSealInput, PreparedResolvedCallableDetachArena,
    PreparedResolvedCallableIdentity, ResolveCallOutcome, ResolvedCallTarget,
    prepare_final_call_callee, prepare_function_value_origin_query, prepare_language_free_dot_path,
    prepare_presentation_callee_id, resolve_call_target,
};
pub use resolver::{
    CallableInstantiation, CharacterOwnerSource, NonCallableSource, ResolvedCharacterOwner,
    ResolvedNonCallableTarget, SignatureOrigin, TypeReceiverInstantiation, UnknownCallKind,
    UnknownCallTarget,
};
pub use schema::{
    CallableArgumentPolicy, CallableArgumentSemanticAction, CallableDocumentation,
    CallableEffectSchema, CallableEvaluatedEffect, CallableExtensionReceiver,
    CallableGenericParameterIssuer, CallableGroupKind, CallableLogLevel, CallableMethodRole,
    CallableParameter, CallableParameterAdmission, CallableParameterConsumer,
    CallableParameterDocumentation, CallableParameterGroup,
    CallableParameterGuardedValueAlternative, CallableParameterOtherwiseValueAlternative,
    CallableParameterPassing, CallableParameterPresence, CallableParameterSource,
    CallableParameterValueAlternative, CallableParameterValueRule, CallableSemanticValueGuard,
    CallableSignatureSchema, CallableSource, CallableUnaryTypeConstructor, CallableValidator,
    CheckedSemanticValueEvidence, DialogueApplicationMetadataCoordinate, DocumentationProvenance,
    OpenArgumentId, ParameterExpectedTypeProjection, RustCallableProvenance, RustCallablePurity,
    RustPackageProvenance, SpreadArgumentPolicy, UnknownNamedArgumentPolicy,
    VariantPayloadRequirement,
};
pub(crate) use schema::{
    CallableEvaluatedEffectOperandRole, CallableGenericConstUse, CallableGenericFirstUse,
    CallableGenericParameterInventory, CallableGenericTypeUse, CallableSchemaGenericRole,
};
pub use view_modifier::ViewModifierId;
