//! Source-backed LSP profile loading and atomically published semantic environments.

pub(crate) mod accepted_project;
pub(crate) mod caches;
mod diagnostic;
mod environment;
mod load;
mod model;
pub mod state;
mod uri;

pub use diagnostic::{LspProfileDiagnostic, LspProfileDiagnosticKind};
pub(crate) use load::apply_registered_topology;
pub use load::{LspProfileBuild, LspProfileResolver};
pub use model::{LspProfile, ProfileSourceSelection};

#[cfg(test)]
pub(crate) use environment::{
    AcceptedBuildWorkSnapshot, accepted_build_work_snapshot_for_test, register_loaded_environment,
};
pub(crate) use environment::{
    ProfileRegistrationOverlay, register_profile_environment_with_overlays,
};
pub(crate) use uri::file_path_from_uri;

#[cfg(test)]
mod tests;
