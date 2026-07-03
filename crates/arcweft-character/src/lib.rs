//! Typed, deterministic character-composition assets for Arcweft.
//!
//! This crate is Sans I/O. It owns the versioned manifest, validation, catalog,
//! package invariant, and look resolution. Filesystem reads/writes and PSD
//! decoding live in adapter crates.

pub mod catalog;
pub mod id;
pub mod manifest;
pub mod package;

/// Stable format identifier written to every character manifest.
pub const CHARACTER_MANIFEST_FORMAT: &str = "arcweft.character";

/// Current character manifest schema version.
pub const CHARACTER_MANIFEST_VERSION: u32 = 1;
