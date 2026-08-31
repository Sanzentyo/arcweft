//! Sans I/O source-edit helpers for Arcweft tooling.
//!
//! This crate produces deterministic text edits and lightweight tooling data.
//! It does not read files, write files, watch paths, or run an LSP transport.

pub mod agent_repl;
pub mod edit;
pub mod format;
pub mod model;
pub mod runtime_diagnostic;

#[cfg(test)]
mod tests;
