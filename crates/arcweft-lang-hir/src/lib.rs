//! Structured HIR for Arcweft language tooling.
//!
//! HIR lowering depends on surface syntax, but semantic analysis, verifier
//! passes, runtime-plan lowering, CLI, and LSP tooling should import HIR through
//! this crate instead of reaching into parser internals.

pub mod assertion;
mod cache_facts;
mod dialogue_application;
mod dialogue_identity;
pub mod entry;
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
pub mod reference;
pub mod style;
pub mod symbol;
pub mod view_part;

/// Syntax types intentionally exposed through a namespace instead of flat
/// crate-root re-exports, so downstream crates can see which layer owns a type.
pub mod syntax {
    pub use arcweft_lang_syntax::{
        assertion, ast, cst, expr, lint, parser, pattern, reference, source, text, types,
    };
}
pub mod callable_source;
