//! Source-to-runtime-plan compiler driver for Arcweft.

pub mod agent;
pub mod agent_effects;
pub mod agent_project;
pub mod content_partition;
pub mod effect_manifest;
pub mod error;
pub mod hir;
pub mod incremental;
pub mod link;
pub mod lower;
pub mod object;
pub mod parse;
pub mod persistent;
pub mod project;
pub mod reachability;
pub mod source;
pub mod types;

#[cfg(test)]
mod tests;
