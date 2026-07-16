//! Filesystem loaders for Arcweft project metadata.
//!
//! This crate owns only host/tooling I/O plus format dispatch. CLI and LSP
//! callers remain responsible for their own error presentation policy.

pub mod adapter_manifest;
pub mod cache;
pub mod character_manifest;
pub mod environment;
pub mod project;
pub mod release_adapter;
pub mod rust_metadata;
pub mod source_document;
