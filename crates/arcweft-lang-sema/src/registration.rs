//! Atomic source-backed registration of one complete semantic world.

mod descriptor;
mod diagnostic;
mod limits;
mod model;
mod registrar;
mod source_index;
#[cfg(test)]
mod tests;

pub use diagnostic::{
    CharacterRegistrationCode, CharacterRegistrationDiagnostic,
    CharacterRegistrationDiagnosticKind, CharacterRegistrationReport, RequiredCharacterToken,
};
pub use limits::{CharacterRegistrationLimitKind, CharacterRegistrationLimits};
pub use model::{
    CharacterInventoryDescriptorV1, CharacterInventoryDigest, CharacterInventoryIntegrityError,
    CharacterInventoryRevision, CharacterRegistrar, CharacterRegistrationRequest,
    EnvironmentBindingId, EnvironmentBindingIdError, ExternalOwnerLookupError,
    ExternalRegistrationFact, ProjectRegistrationFacts, RegisteredCharacterResolutionError,
    RegisteredExternalOwner, RegisteredExternalOwnerKind, RegisteredSemanticWorld,
    RegisteredTypeCheckEnv,
};
pub use source_index::{
    CharacterDeclarationSet, CharacterDeclarationSource, CharacterDefinitionIndex,
    CharacterDefinitionIndexBuildError, CharacterDefinitionIndexBuildReport,
    CharacterDefinitionIndexCode, CharacterDefinitionLimitKind, CharacterDefinitionLimits,
    CharacterDefinitionSpanError,
};
