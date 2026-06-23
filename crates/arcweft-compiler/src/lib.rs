//! Source-to-runtime-plan compiler driver for Arcweft.

pub mod agent;
pub mod agent_effects;
pub mod agent_project;
pub mod effect_manifest;
pub mod error;
pub mod hir;
pub mod lower;
pub mod parse;
pub mod project;
pub mod source;
pub mod types;

#[cfg(test)]
mod tests;
