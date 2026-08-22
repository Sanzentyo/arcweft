//! Typed callable identities, schemas, catalogs, and semantic query results.
//!
//! This module owns the common in-memory contract used by accepted-world
//! registration, type checking, and semantic call queries. It intentionally
//! exposes immutable read models while keeping catalog construction and
//! resolver mutation crate-private.

mod arguments;
mod builder;
mod catalog;
mod checked_catalog;
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

pub use arguments::CallableParameterCoordinate;
pub(crate) use arguments::{
    CallArgumentMapping, MappedCallArgument, MappedCallArgumentSlot, map_call_arguments,
    map_unmapped_call_arguments,
};
pub(crate) use builder::RegisteredCallableCatalogBuilder;
pub use catalog::{
    CallableAccess, CallableRecord, CatalogCallableEntry, EnvironmentCallableCatalog,
    EnvironmentDeclarationOrdinal, EquivalentCallableSource, NonEmptyCallableSet,
    ProjectCallableCatalog, RegisteredCallableCatalog, RegisteredProjectModuleCallables,
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
pub use dialogue::{
    CharacterDialoguePatchContext, DialogueCallableId, DialogueCalleeIdentity,
    DialogueSchemaContext,
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
pub use facts::{
    CallCalleeClassificationFact, CallPoison, CallTargetFact, CallTargetFacts, CallableDiagnostic,
    CallableDiagnosticRelated, CallableDiagnosticSeverity, CallableDiagnosticSubject,
    CheckedCallArgumentFact, CheckedCallArgumentSlotFact, CheckedCallArgumentSlotSource,
    SemanticParameter, SemanticParameterGroup, SemanticSignature, SemanticSignatureHelp,
    SemanticSignatureIndex, SemanticSignatureRecovery, SemanticSignatureSurface,
};
pub(crate) use facts::{CallTargetFactsInput, CheckedCallArgumentSlotInput, CheckedCallTarget};
pub use identity::{
    AdapterPackageId, AgentIntrinsicSignatureId, BuiltinCallableId, CallableArgumentSlotIndex,
    CallableAuthorityRank, CallableCandidateId, CallableFamily, CallableGroupIndex,
    CallableLookupKey, CallableName, CallableOverloadIndex, CallableParameterIndex, CallablePath,
    CallableProviderId, CapabilityCallableId, CapacityMethodId, CheckedCallableContext,
    CheckedCallableDeclaration, CheckedCallableDigest, CheckedCallableId,
    CheckedCallableIdentityError, CheckedClosureId, CheckedEffectCallableId, CollectionMethodId,
    CurriedCallableId, DetachedCallableDeclarationId, DomainMethodId, DropCallableId,
    EnumVariantSignatureId, EnvironmentCallableDigest, EnvironmentCallableId,
    EnvironmentCallableKind, EnvironmentCallableOwner, FloatWidth, FunctionValueOrdinal,
    FunctionValueSignatureId, FxCallableSignatureId, FxResolution, IntegerMethodId,
    LanguageCallableFamily, LanguageDocumentationFamily, LexicalBindingIndex, LineContextMethodId,
    LineScheduleCallableId, LocalCallableId, MathCallableId, OptionConstructorKind,
    PresentationHandleMethodId, ProbeComparisonId, ProbeComparisonOperator, ProjectCallablePath,
    ProjectNameBinding, ProjectNominalTypeId, PromotionCallableId, ReceiverMethodKey,
    ReductionConstructorKind, ResultConstructorKind, RustItemPath, STANDARD_TRAIT_CATALOG_VERSION,
    SemanticScopeId, StageMethodId, StandardCallableDeclarationId, StandardEnvironmentId,
    StandardTraitCatalogVersion, StdFloatCallableId, StdFloatOperation, VectorDimensions,
};
pub use join::{
    CallableInstantiationDigest, CallableReceiverMode, CheckedCallableArgument,
    CheckedCallableArgumentSlot, CheckedCallableJoin, CheckedCallableJoinError,
    IntrinsicCallableCandidateTag, validate_selected_call,
};
pub use limits::{
    CallResolverAccountingReport, CallableLimits, PRODUCTION_CALLABLE_LIMITS,
    PRODUCTION_SIGNATURE_LIMITS, SignatureAccountingError, SignatureQueryLimits,
    SignatureQueryProjectionWork, SignatureQueryResolutionWork, SignatureQuerySearchWork,
    SignatureQueryWorkReport, SignatureWorkReport,
};
pub(crate) use limits::{CallableQueryDepth, ResolverWork, SignatureQueryWorkMeter};
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
    CallResolverAuthority, CallResolverContext, CallResolverRequest, FinalCallCalleeFacts,
    PreparedCallCallee, prepare_final_call_callee, prepare_language_free_dot_path,
    resolve_call_target,
};
pub use resolver::{
    CallableInstantiation, CharacterOwnerSource, NonCallableSource, NonEmptyResolvedCandidates,
    ResolveCallOutcome, ResolvedCallTarget, ResolvedCallable, ResolvedCharacterOwner,
    ResolvedFunctionValue, ResolvedNonCallableTarget, SignatureOrigin, TypeReceiverInstantiation,
    UnknownCallKind, UnknownCallTarget,
};
pub use schema::{
    CallableArgumentPolicy, CallableDocumentation, CallableEffectSchema, CallableEvaluatedEffect,
    CallableExtensionReceiver, CallableGroupKind, CallableLogLevel, CallableMethodRole,
    CallableParameter, CallableParameterDocumentation, CallableParameterGroup,
    CallableParameterPassing, CallableParameterPresence, CallableParameterSource,
    CallableParameterType, CallableSignatureSchema, CallableSource, CallableValidator,
    DocumentationProvenance, RustCallableProvenance, RustCallablePurity, RustPackageProvenance,
    SpreadArgumentPolicy, UnknownNamedArgumentPolicy,
};
