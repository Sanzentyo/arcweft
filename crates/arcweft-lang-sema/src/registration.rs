//! Atomic source-backed registration of one complete semantic world.

mod descriptor;
mod diagnostic;
mod environment_digest;
mod environment_input;
mod limits;
mod model;
mod registrar;
mod source_index;
pub use diagnostic::{
    CharacterRegistrationCode, CharacterRegistrationDiagnostic,
    CharacterRegistrationDiagnosticKind, CharacterRegistrationReport, RequiredCharacterToken,
};
pub(crate) use environment_input::BoundEnvironmentRegistrationInput;
pub use environment_input::{
    AcceptedNominalInputVisibility, AcceptedNominalInventoryInput,
    CharacterDialogueCustomFieldInput, EnvironmentCallableLookupInput,
    EnvironmentCallablePublicationMetadataInput, EnvironmentCallablePublicationRecordInput,
    EnvironmentCallableSignatureInput, EnvironmentHostCallContractInput, EnvironmentManifestDigest,
    EnvironmentParameterGroupInput, EnvironmentParameterInput, EnvironmentParameterMetadataInput,
    EnvironmentParameterTypeInput, EnvironmentPublicationItemId, EnvironmentTypeInputDigest,
    EnvironmentTypeProjectionKind, EnvironmentTypeProjectionNode, EnvironmentTypeSite,
    EnvironmentTypeSiteRoot, EnvironmentTypeSiteStep, EnvironmentValueBindingInput,
    SourceBackedEnvironmentRegistrationInput,
};
pub use limits::{CharacterRegistrationLimitKind, CharacterRegistrationLimits};
pub use model::{
    AcceptedNominalSource, AcceptedNominalVisibilityIndex, AcceptedNominalWorld,
    AcceptedNominalWorldLookupError, AcceptedNominalWorldStamp, CharacterInventoryDescriptorV1,
    CharacterInventoryDigest, CharacterInventoryIntegrityError, CharacterInventoryRevision,
    CharacterRegistrar, CharacterRegistrationRequest, ExternalOwnerLookupError,
    ExternalRegistrationFact, ProjectRegistrationFacts, ProofReturnRegistrationPrelude,
    ProofReturnRegistrationRequest, RegisteredCharacterResolutionError,
    RegisteredEnvironmentDigest, RegisteredEnvironmentExternalOwner, RegisteredExternalOwner,
    RegisteredExternalOwnerKind, RegisteredSemanticWorld, RegisteredTypeCheckEnv,
};
pub use source_index::{
    CharacterDeclarationSet, CharacterDeclarationSource, CharacterDefinitionIndex,
    CharacterDefinitionIndexBuildError, CharacterDefinitionIndexBuildReport,
    CharacterDefinitionIndexCode, CharacterDefinitionLimitKind, CharacterDefinitionLimits,
    CharacterDefinitionSpanError,
};
