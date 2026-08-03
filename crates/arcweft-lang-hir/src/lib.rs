//! Structured HIR for Arcweft language tooling.
//!
//! HIR lowering depends on surface syntax, but semantic analysis, verifier
//! passes, runtime-plan lowering, CLI, and LSP tooling should import HIR through
//! this crate instead of reaching into parser internals.

mod arena;
mod cache_facts;
mod database;
mod diagnostic;
mod dialogue_application;
mod dialogue_identity;
pub mod entry;
mod expr;
mod final_lowering;
mod final_project;
pub mod fx;
pub mod identity;
mod item;
mod leaf;
pub mod lower;
mod lower_choice;
mod lower_context;
mod lower_dialogue;
mod lower_flow;
mod lower_ids;
pub mod model;
mod module;
mod pattern;
pub mod project;
mod scope;
mod slot;
mod source_index;
mod stmt;
pub mod style;
pub mod symbol;
mod type_ref;
pub mod view_part;

pub mod callable_source;
