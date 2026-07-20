//! Sans I/O launch profile model for Arcweft project execution.
//!
//! Launch profiles are the canonical representation of command-specific runtime
//! context. CLI commands lower into this data before semantic checking or execution.

pub mod accepted;
mod decode;
pub mod diagnostic;
pub mod manifest;
mod model;
pub mod resolve;
mod source_map;
mod tree_de;

pub use arcweft_manifest_model::{
    ContentCompression, ContentPlacement, ContentResidency, EntrySelectionId, LaunchKind, ProfileId,
};

pub use model::{
    LaunchMathBackend, LaunchPlayerViewportFit, LaunchProfileSelection, LaunchPureBackend,
};
