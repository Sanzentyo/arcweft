//! Sans I/O source-edit helpers for Arcweft tooling.
//!
//! This crate produces deterministic text edits and lightweight tooling data.
//! It does not read files, write files, watch paths, or run an LSP transport.

pub mod agent_repl;
pub mod code_actions;
mod dialogue_content;
pub mod edit;
pub mod format;
pub mod model;
mod rich_text_canonicalization;
pub mod style_environment;

#[cfg(test)]
mod tests;
