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
    ExternalModuleFactsError, LoadedDocumentAccess, LoadedDocumentOwnership,
    LoadedExternalModuleMetadata, LoadedProfileTopology, LoadedProfileTopologyResource,
    ProfileDependencyResourceSeed, ProfileTopologyErrorCode, ProfileTopologyLimitKind,
    ProfileTopologyLimits, ProfileTopologyLoadError, ProfileTopologyLoadRequest,
    ProfileTopologyOverlaySeed, ProfileTopologyResourceKind, ProfileTopologyResourceOrigin,
    ProfileTopologySeedError, TypeReferenceLimitKind, TypeReferenceLimits,
};

#[cfg(test)]
mod tests;
