#![forbid(unsafe_code)]
//! One syntax authority for Rust data attributes consumed by reflection and
//! accepted ABI metadata. This crate runs during macro expansion only.

pub mod attrs;
pub mod rename;
pub mod validation;
