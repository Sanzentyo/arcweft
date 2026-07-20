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
mod error;
mod facts;
mod identity;
mod limits;
mod presentation;
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
pub(crate) use error::CallTargetFactError;
pub use error::{
    BuiltinIdentityError, CallableBuildLimitError, CallableCatalogBuildError, CallableCatalogError,
    CallableDiagnosticCode, CallableDocumentationError, CallableFamilyInvariantCode,
    CallableIdentityError, CallableIndexKind, CallablePathError, CallablePublicationError,
    CallableQueryLimitError, CallableScalarError, CallableScalarKind, CallableSchemaError,
    CallableSourceError, CorruptCallableCatalogReason, ResolveCallError, RustProvenanceError,
    RustProvenanceField, SemanticSignatureError,
};
pub use facts::{
    CallPoison, CallableDiagnostic, CallableDiagnosticRelated, CallableDiagnosticSeverity,
    CallableDiagnosticSubject, SemanticParameter, SemanticParameterGroup, SemanticSignature,
    SemanticSignatureHelp, SemanticSignatureIndex, SemanticSignatureRecovery,
};
pub(crate) use facts::{
    CallTargetFact, CallTargetFactMode, CallTargetFacts, CheckedCallArgumentFact,
    CheckedCallArgumentSlotFact, CheckedCallArgumentSlotInput, CheckedCallTarget,
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
    ReductionConstructorKind, ResultConstructorKind, RustItemPath, SpeakerCallableId,
    StandardEnvironmentId, StdFloatCallableId, StdFloatOperation, TraitCallableId,
    TraitCallableSource, TraitImplementationIndex, VectorDimensions,
};
pub(crate) use limits::ResolverWork;
pub use limits::{CallableLimits, PRODUCTION_CALLABLE_LIMITS, SignatureWorkReport};
pub(crate) use presentation::{PresentationArgumentValuePolicy, PresentationNamedArgument};
pub use presentation::{PresentationCallableId, PresentationSchemaContext};
pub use publication::{EnvironmentCallablePublication, EnvironmentCallablePublicationRecord};
pub(crate) use resolver::{
    CallCallee, CallResolverRequest, CallSourceContext, LexicalCallBinding, LexicalCallableScope,
    ResolvedEnumSeed, ResolvedFunctionValueSeed, resolve_call_target,
};
pub use resolver::{
    CallableInstantiation, CharacterOwnerResolution, CharacterOwnerSource, NonCallableSource,
    NonEmptyResolvedCandidates, ResolveCallOutcome, ResolvedCallTarget, ResolvedCallable,
    ResolvedCharacterOwner, ResolvedFunctionValue, ResolvedNonCallableTarget, SignatureOrigin,
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
