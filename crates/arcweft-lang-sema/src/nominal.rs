//! Bounded, source-aware nominal type resolution.

mod cache;
mod diagnostic;
mod index;
mod input;
mod limits;
mod model;
mod resolver;

pub use cache::{
    CheckedTypeReferenceCache, CheckedTypeReferenceCacheKey, HirTypeStructuralDigest,
    NominalResolverSchemaVersion,
};
pub use diagnostic::{
    NominalDiagnosticRelated, NominalRelatedMessage, NominalTypeDiagnostic,
    NominalTypeDiagnosticCode, NominalTypeDiagnosticKind, TypePoisonOrigin, TypePoisonRecord,
};
pub use index::{NominalResolutionIndex, NominalResolutionIndexError, NominalTypeNodeKey};
pub use input::{
    GenericTypeBinding, GenericTypeScope, GenericTypeScopeError, GenericTypeScopeFingerprint,
    SelfTypeScope, SelfTypeScopeFingerprint, TypeResolutionInput, TypeResolutionInputError,
    TypeResolutionModule, TypeResolutionProject, TypeResolutionWorld,
};
pub use limits::{
    AcceptedNominalCatalogLimitKind, AcceptedNominalCatalogLimits,
    AcceptedNominalCatalogLimitsError, NominalAggregationLimitKind, NominalAggregationLimits,
    NominalAggregationLimitsError, NominalResolutionLimitKind, NominalResolutionLimits,
    NominalResolutionLimitsError,
};
pub(crate) use model::ResolvedAssociatedTypeReceiver;
pub use model::{
    AliasExpansionFact, BuiltinTypeConstructor, DetachedNominalEvidence, DetachedNominalReason,
    DetachedTypeRef, ExternalNominalResolution, PoisonedTypeRef, ResolvedAliasReference,
    ResolvedOpenNominal, ResolvedTypeNode, ResolvedTypeProduct, ResolvedTypeRefOutcome,
    StructuralTypeNodeKind, TypeArgumentExpectation, TypeArgumentKind, TypeArityExpectation,
    TypeArityTarget, TypeNameResolution, TypeResolutionFailure, TypeResolutionReport,
    TypeSourceEvidence,
};
pub use resolver::resolve_type_ref;
