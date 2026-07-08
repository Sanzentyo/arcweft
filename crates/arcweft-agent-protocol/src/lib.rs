//! Shared Agent Debug Bus data types.
//!
//! This crate is Sans I/O. CLI commands, MCP servers, tests, and future player
//! adapters should exchange these typed observation records instead of defining
//! transport-local JSON shapes.

pub mod action;
pub mod artifact;
pub mod diagnostic;
pub mod geometry;
pub mod hit_test;
pub mod ids;
pub mod image;
pub mod object;
pub mod observation;
pub mod predicate;
pub mod presentation;
pub mod protocol;
pub mod proxy;
pub mod resource;
pub mod rich_text;
pub mod session;
pub mod trace;
pub mod value;
pub mod verified_effects;
pub mod view;

mod serde_helpers;

#[cfg(test)]
mod tests;
