//! Sans I/O launch profile model for Arcweft project execution.
//!
//! Launch profiles are the canonical representation of command-specific runtime
//! context. CLI commands lower into this data before semantic checking or execution.

mod model;
pub mod parse;
pub mod source;

pub use model::{
    LaunchBuildProfileSpec, LaunchContentCompression, LaunchContentPlacement,
    LaunchContentProfileSpec, LaunchContentResidency, LaunchDebugPolicy, LaunchHotReloadFallback,
    LaunchHotReloadMode, LaunchHotReloadProfileSpec, LaunchHotReloadStatePolicy, LaunchKind,
    LaunchMathBackend, LaunchPlayerProfileSpec, LaunchPlayerViewportFit, LaunchPlayerViewportSpec,
    LaunchProfileError, LaunchProfileManifest, LaunchProfileSpec, LaunchPureBackend,
    LaunchPureProfileSpec, LaunchSourcePolicy, ProfileId, ResolvedLaunchProfile,
};
pub use parse::{LaunchDocumentError, TomlStructuralErrorKind};
pub use source::{
    LaunchKeyPath, LaunchManifestSourceMap, LaunchToken, LaunchTokenPath,
    SourceBackedLaunchManifest,
};

#[cfg(test)]
mod tests;
