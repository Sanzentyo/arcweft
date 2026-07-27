//! Structured HIR for Arcweft language tooling.
//!
//! HIR lowering depends on surface syntax, but semantic analysis, verifier
//! passes, runtime-plan lowering, CLI, and LSP tooling should import HIR through
//! this crate instead of reaching into parser internals.

mod cache_facts;
mod dialogue_application;
mod dialogue_identity;
pub mod entry;
mod expr;
pub mod fx;
pub mod identity;
pub mod lower;
mod lower_choice;
mod lower_context;
mod lower_dialogue;
mod lower_flow;
mod lower_ids;
pub mod model;
pub mod project;
pub mod style;
pub mod symbol;
pub mod view_part;

pub mod callable_source;
