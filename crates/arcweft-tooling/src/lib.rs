//! Sans I/O source-edit helpers for Arcweft tooling.
//!
//! This crate produces deterministic text edits and lightweight tooling data.
//! It does not read files, write files, watch paths, or run an LSP transport.

pub mod agent_repl;
mod canonicalization;
pub mod code_actions;
mod decl_identity;
mod dialogue_content;
mod dialogue_defaults;
mod dialogue_sugar;
pub mod edit;
pub mod format;
pub mod id_context;
mod line_sugar;
pub mod model;
mod path_sugar;

pub use canonicalization::canonicalize_source;

#[cfg(test)]
mod tests;
