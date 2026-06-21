//! MCP-facing adapters for Arcweft Agent Debug Bus resources.
//!
//! This crate is Sans I/O. It does not own stdio, HTTP, sessions, or renderer
//! readback. It maps `arcweft-agent-protocol` records into MCP-compatible JSON
//! shapes so CLI, tests, and a future MCP transport share one contract.

pub mod model;
pub mod resources;
pub mod tools;

#[cfg(test)]
mod tests;
