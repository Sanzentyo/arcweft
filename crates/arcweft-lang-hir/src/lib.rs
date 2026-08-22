//! Structured HIR for Arcweft language tooling.
//!
//! HIR lowering depends on surface syntax, but semantic analysis, verifier
//! passes, runtime-plan lowering, CLI, and LSP tooling should import HIR through
//! this crate instead of reaching into parser internals.

mod arena;
pub mod body_edges;
pub mod database;
pub mod diagnostic;
pub mod dialogue_application;
pub mod expr;
mod final_lowering;
pub mod fx;
pub mod identity;
pub mod item;
pub mod leaf;
pub mod line_identity;
pub mod lowering;
pub mod module;
pub mod pattern;
#[path = "final_project.rs"]
pub mod project;
pub mod proof_return;
pub mod scope;
pub mod slot;
pub mod source_index;
pub mod stmt;
pub mod symbol;
pub mod type_ref;
