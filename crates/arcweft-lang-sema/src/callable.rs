//! Typed callable identities, schemas, catalogs, and semantic query results.
//!
//! This module owns the common in-memory contract used by accepted-world
//! registration, type checking, and semantic call queries. It intentionally
//! exposes immutable read models while keeping catalog construction and
//! resolver mutation crate-private.

mod arguments;
mod builder;
mod catalog;
mod dialogue;
mod digest;
mod error;
mod facts;
mod identity;
mod limits;
mod nominal_signature;
mod presentation;
mod projection;
mod publication;
mod resolver;
mod schema;

pub use arguments::CallableParameterCoordinate;
pub(crate) use arguments::{call_shape_is_viable, data_last_unsupported_spread_reason};
pub(crate) use builder::RegisteredCallableCatalogBuilder;
pub use catalog::{
    CallableRecord, CatalogCallableEntry, EnvironmentCallableCatalog,
    EnvironmentDeclarationOrdinal, EquivalentCallableSource, NonEmptyCallableSet,
    ProjectCallableCatalog, RegisteredCallableCatalog, RegisteredProjectModuleCallables,
};
pub use dialogue::{DialogueCallableId, DialogueCalleeIdentity, DialogueSchemaContext};
pub use digest::{
    CallableSignatureSchemaDigest, EnvironmentCallablePublicationDigest,
    RegisteredCallableCatalogDigest,
};
pub use error::{
    BuiltinIdentityError, CallTargetFactError, CallableBuildLimitError, CallableCatalogBuildError,
    CallableCatalogError, CallableDiagnosticCode, CallableDocumentationError,
    CallableFamilyInvariantCode, CallableIdentityError, CallableIndexKind, CallablePathError,
    CallablePublicationError, CallableQueryLimitError, CallableScalarError, CallableScalarKind,
    CallableSchemaError, CallableSourceError, CorruptCallableCatalogReason, ResolveCallError,
    RustProvenanceError, RustProvenanceField, SemanticSignatureError,
    SignatureLimitConfigurationError, SignatureLimitExceeded, SignatureLimitKind,
    SignatureWorkKind,
};
pub use facts::{
    CallPoison, CallTargetFact, CallTargetFacts, CallableDiagnostic, CallableDiagnosticRelated,
    CallableDiagnosticSeverity, CallableDiagnosticSubject, CheckedCallArgumentFact,
    CheckedCallArgumentSlotFact, SemanticParameter, SemanticParameterGroup, SemanticSignature,
    SemanticSignatureHelp, SemanticSignatureIndex, SemanticSignatureRecovery,
};
pub(crate) use facts::{
    CallTargetFactMode, CallTargetFactsInput, CheckedCallArgumentSlotInput, CheckedCallTarget,
};
#[cfg(test)]
pub(crate) use identity::migration_evidence::{
    MigrationAuthorityClass, MigrationCompletionDisposition,
};
pub use identity::{
    AdapterPackageId, AgentIntrinsicSignatureId, BuiltinCallableId, CallableArgumentIndex,
    CallableArgumentSlotIndex, CallableAuthorityRank, CallableCandidateId, CallableFamily,
    CallableGroupIndex, CallableLookupKey, CallableName, CallableOverloadIndex,
    CallableParameterIndex, CallablePath, CallableProviderId, CapabilityCallableId,
    CapacityMethodId, CollectionMethodId, CurriedCallableId, DataLastCallableId, DomainMethodId,
    DropCallableId, EnumVariantSignatureId, EnvironmentCallableId, EnvironmentCallableKind,
    EnvironmentCallableOwner, FloatWidth, FunctionValueOrdinal, FunctionValueSignatureId,
    FxCallableSignatureId, FxResolution, IntegerMethodId, LanguageCallableFamily,
    LanguageDocumentationFamily, LexicalBindingIndex, LocalCallableId, MathCallableId,
    OptionConstructorKind, PresentationHandleMethodId, ProbeComparisonId, ProjectCallablePath,
    ProjectNameBinding, ProjectNominalTypeId, PromotionCallableId, ReceiverMethodKey,
    ReductionConstructorKind, ResultConstructorKind, RustItemPath, SemanticScopeId,
    SpeakerCallableId, StageMethodId, StandardEnvironmentId, StdFloatCallableId, StdFloatOperation,
    TraitCallableId, TraitCallableSource, TraitImplementationIndex, VectorDimensions,
};
#[cfg(test)]
pub(crate) use limits::AssociatedResolverWorkReport;
pub(crate) use limits::{
    AssociatedResolverStep, CallableQueryDepth, ResolverWork, SignatureQueryWorkMeter,
};
pub use limits::{
    CallableLimits, PRODUCTION_CALLABLE_LIMITS, PRODUCTION_SIGNATURE_LIMITS,
    SignatureAccountingError, SignatureQueryLimits, SignatureQueryProjectionWork,
    SignatureQueryResolutionWork, SignatureQuerySearchWork, SignatureQueryWorkReport,
    SignatureWorkReport,
};
pub(crate) use presentation::{PresentationArgumentValuePolicy, PresentationNamedArgument};
pub use presentation::{PresentationCallableId, PresentationSchemaContext};
pub use projection::{
    EnvironmentPublicationProjectionDiagnostic, EnvironmentPublicationProjectionErrorKind,
    EnvironmentPublicationProjectionReport, EnvironmentPublicationRelatedLabel,
    EnvironmentPublicationRelatedSource,
};
pub use publication::{EnvironmentCallablePublication, EnvironmentCallablePublicationRecord};
pub(crate) use resolver::{
    CallCallee, CallResolverAuthority, CallResolverRequest, CallSourceContext, LexicalCallBinding,
    LexicalCallableScope, ResolvedEnumSeed, ResolvedFunctionValueSeed, SignatureQueryStep,
    SignatureQueryStepControl, resolve_call_target,
};
pub use resolver::{
    CallableInstantiation, CharacterOwnerSource, NonCallableSource, NonEmptyResolvedCandidates,
    ResolveCallOutcome, ResolvedCallTarget, ResolvedCallable, ResolvedCharacterOwner,
    ResolvedFunctionValue, ResolvedNonCallableTarget, SignatureOrigin, TypeReceiverInstantiation,
    UnknownCallKind, UnknownCallTarget,
};
pub use schema::{
    CallableArgumentPolicy, CallableDocumentation, CallableEffectSchema, CallableGroupKind,
    CallableParameter, CallableParameterDocumentation, CallableParameterGroup,
    CallableParameterPassing, CallableParameterPresence, CallableParameterSource,
    CallableParameterType, CallableSignatureSchema, CallableSource, CallableValidator,
    DocumentationProvenance, RustCallableProvenance, RustCallablePurity, RustPackageProvenance,
    SpreadArgumentPolicy, UnknownNamedArgumentPolicy,
};

#[cfg(test)]
mod resolver_tests;

#[cfg(test)]
mod tests;
