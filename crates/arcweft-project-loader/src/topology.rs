//! Bounded, overlay-first loading of one exact launch-profile topology.

mod budget;
mod external;
mod id;
mod loader;
mod model;

pub use id::{
    ProfileTopologyIdError, ProfileTopologyLogicalPath, ProfileTopologyOwnerId,
    ProfileTopologyPathError, ProfileTopologyResourceId,
};
pub use loader::load_profile_topology;
pub use model::{
    ExternalModuleFactsError, LoadedCharacterPackage, LoadedDocumentAccess,
    LoadedDocumentOwnership, LoadedExternalModuleMetadata, LoadedProfileTopology,
    LoadedProfileTopologyResource, LoadedProfileTopologyResourcePayload,
    ProfileDependencyBinaryResourceSeed, ProfileDependencyResourceSeed,
    ProfileTopologyBinaryOverlaySeed, ProfileTopologyErrorCode, ProfileTopologyLimitKind,
    ProfileTopologyLimits, ProfileTopologyLoadError, ProfileTopologyLoadRequest,
    ProfileTopologyOverlaySeed, ProfileTopologyResourceKind, ProfileTopologyResourceOrigin,
    ProfileTopologySeedError, ProfileTopologyWatchEntry, ProfileTopologyWatchExpectation,
    TypeReferenceLimitKind, TypeReferenceLimits,
};

#[cfg(test)]
mod loader_limits_tests;
#[cfg(test)]
mod tests;
