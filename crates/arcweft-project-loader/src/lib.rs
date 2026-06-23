//! Filesystem loaders for Arcweft project metadata.
//!
//! This crate owns only host/tooling I/O plus format dispatch. CLI and LSP
//! callers remain responsible for their own error presentation policy.

pub mod adapter_manifest;
pub mod character_manifest;
pub mod project;
pub mod rust_metadata;
