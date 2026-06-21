//! Controller runner boundaries for compiled Agent Script programs.
//!
//! This crate does not interpret `.awfagent` source and does not own CLI, MCP,
//! renderer, database, filesystem, or transport I/O. It coordinates typed host
//! requests emitted by a controller VM with an `AgentSession`, debug sink, and
//! RAG service.

mod assertion;
mod budget;
pub mod config;
pub mod effect_policy;
pub mod error;
mod host_request;
mod label_parse;
pub mod policy;
mod predicate;
pub mod runner;
mod runtime_args;
mod runtime_payload;
mod runtime_value;
pub mod session;

#[cfg(test)]
mod tests;
