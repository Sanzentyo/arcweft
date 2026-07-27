//! Filesystem loaders for Arcweft project metadata.
//!
//! This crate owns only host/tooling I/O plus format dispatch. CLI and LSP
//! callers remain responsible for their own error presentation policy.

pub mod cache;
mod character_manifest;
pub mod environment;
pub mod layout;
pub mod project;
pub mod project_limits;
pub mod release_adapter;
pub mod source_document;
pub mod topology;
