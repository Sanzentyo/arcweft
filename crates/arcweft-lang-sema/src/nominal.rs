//! Bounded, source-aware nominal type resolution.

mod cache;
mod diagnostic;
mod index;
mod input;
mod limits;
mod model;
mod resolver;
mod shapes;
#[cfg(test)]
mod shapes_tests;

pub use diagnostic::{
    NominalDiagnosticRelated, NominalRelatedMessage, NominalTypeDiagnostic,
    NominalTypeDiagnosticCode, NominalTypeDiagnosticKind, TypePoisonOrigin, TypePoisonRecord,
};
pub use index::{NominalResolutionIndex, NominalResolutionIndexError, NominalTypeNodeKey};
pub use input::{
    AuthoredTypeInput, GenericTypeBinding, GenericTypeScope, GenericTypeScopeError,
    GenericTypeScopeFingerprint, SelfTypeScope, SelfTypeScopeFingerprint, TypeResolutionInput,
    TypeResolutionInputError, TypeResolutionWorld,
};
pub use limits::{
    AcceptedNominalCatalogLimitKind, AcceptedNominalCatalogLimits,
    AcceptedNominalCatalogLimitsError, NominalAggregationLimitKind, NominalAggregationLimits,
    NominalAggregationLimitsError, NominalResolutionLimitKind, NominalResolutionLimits,
    NominalResolutionLimitsError,
};
#[cfg(test)]
pub(crate) use model::AssociatedReceiverFailure;
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
pub(crate) use shapes::ProjectNominalShapeCatalog;

#[cfg(test)]
mod resolver_tests;
pub use cache::{
    AuthoredTypeRefStructuralDigest, CheckedTypeReferenceCache, CheckedTypeReferenceCacheKey,
    NominalResolverSchemaVersion,
};
