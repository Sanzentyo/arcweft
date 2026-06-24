//! Sans I/O project model shared by loaders, compiler drivers, CLI, and LSP.
//!
//! This crate owns package metadata, source inventories, and module graph policy.
//! Filesystem discovery and reads remain in `arcweft-project-loader`.

pub mod artifact;
pub mod fingerprint;
pub mod graph;
pub mod incremental;
pub mod manifest;
pub mod persistent_object;
pub mod sources;
