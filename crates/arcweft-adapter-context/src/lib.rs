//! Adapter manifests that contribute host facts to Arcweft tooling.
//!
//! The language checker stays adapter-agnostic. Adapter runners, CLIs, LSP
//! profiles, and tests opt into manifests when a host surface injects runtime
//! bindings such as HTTP request data, host effects, or Rust adapter exports.

pub mod callable;
pub mod codec;
pub mod manifest;
#[cfg(feature = "sema")]
pub mod publication;
pub mod standard;
pub mod symbol;
