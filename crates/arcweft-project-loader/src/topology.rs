//! Bounded, overlay-first loading of one exact launch-profile topology.

mod budget;
mod id;
mod loader;
mod model;

pub use id::{
    ProfileTopologyIdError, ProfileTopologyLogicalPath, ProfileTopologyOwnerId,
    ProfileTopologyPathError, ProfileTopologyResourceId,
};
pub use loader::load_profile_topology;
pub use model::{
    LoadedDocumentAccess, LoadedDocumentOwnership, LoadedProfileTopology,
    LoadedProfileTopologyResource, ProfileDependencyResourceSeed, ProfileTopologyErrorCode,
    ProfileTopologyLimitKind, ProfileTopologyLimits, ProfileTopologyLoadError,
    ProfileTopologyLoadRequest, ProfileTopologyOverlaySeed, ProfileTopologyResourceKind,
    ProfileTopologyResourceOrigin, ProfileTopologySeedError,
};

#[cfg(test)]
mod tests;
