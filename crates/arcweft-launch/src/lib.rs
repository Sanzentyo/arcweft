//! Sans I/O launch profile model for Arcweft project execution.
//!
//! Launch profiles are the canonical representation of command-specific runtime
//! context. CLI commands lower into this data before semantic checking or execution.

mod model;
pub mod parse;
pub mod source;

pub use arcweft_manifest_model::{
    ContentCompression, ContentPlacement, ContentResidency, EntrySelectionId, LaunchKind, ProfileId,
};

pub use model::{
    LaunchBuildProfileSpec, LaunchContentProfileSpec, LaunchDebugPolicy, LaunchHotReloadFallback,
    LaunchHotReloadMode, LaunchHotReloadProfileSpec, LaunchHotReloadStatePolicy, LaunchMathBackend,
    LaunchPlayerProfileSpec, LaunchPlayerViewportFit, LaunchPlayerViewportSpec, LaunchProfileError,
    LaunchProfileManifest, LaunchProfileSelection, LaunchProfileSpec, LaunchPureBackend,
    LaunchPureProfileSpec, LaunchSourcePolicy, ResolvedLaunchProfile,
};
pub use parse::{LaunchDocumentError, TomlStructuralErrorKind};
pub use source::{
    LaunchKeyPath, LaunchManifestSourceMap, LaunchToken, LaunchTokenPath,
    SourceBackedLaunchManifest,
};

#[cfg(test)]
mod tests;
