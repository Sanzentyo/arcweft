//! Sans-I/O identity, path, digest, and canonical-encoding contracts shared by
//! Arcweft project manifests and generated adapter metadata.

mod canonical;
mod digest;
mod identity;
mod opaque_producer;
mod path;
mod project_locale;
mod schema;

pub use canonical::{CanonicalJsonError, canonical_json_bytes};
pub use digest::{DigestParseError, HostCallContractDigest, RawDigest, SemanticDigest};
pub use identity::{
    ActivityId, ActivityImplementationId, AdapterExportId, AdapterProfileId, AdapterTypeName,
    CapabilityId, ContentUnitId, EntrySelectionId, ExternalModuleId, ExternalModuleImportId,
    FieldName, FunctionName, GeneratorName, IdentifierError, ModuleMountPath, PackageId,
    PackageVersion, ProfileId, TargetTriple, TypeReference, WitWorldId,
};
pub use opaque_producer::{AdapterOpaqueTypeProducerId, AdapterOpaqueTypeProducerIdError};
pub use path::{NormalizedProjectPath, NormalizedProjectPathError};
pub use project_locale::{MAX_PROJECT_LOCALE_FALLBACKS, ProjectLocaleSpec, ProjectLocaleSpecError};
pub use schema::{
    ActivityBindingSpec, ActivityImplementationSpec, AdapterFamily, BuildSpec, ContentCompression,
    ContentPlacement, ContentResidency, ContentRootRef, ContentUnitSpec, DependencyDemand,
    EntityIdRef, ExternalModuleImportSpec, LaunchKind, ManifestSchemaVersion,
    ManifestSchemaVersionError, ManifestVisibility, NonEmptyVec, PackageSpec, ProfileContentSpec,
};
