//! Semantic analysis for Arcweft HIR.
//!
//! This crate owns name resolution, symbol collection, and the current minimal
//! type-checking pass. It depends on parsed syntax and HIR, but parser/runtime
//! crates do not depend on it.

pub mod assertion;
pub mod borrow;
pub mod canonicalization;
pub mod character_definition;
pub mod check;
pub mod checker;
pub mod diagnostics;
pub mod dialogue_view;
pub mod effect_analysis;
pub mod effect_catalog;
pub mod effect_collector;
pub mod effect_contract;
pub mod effect_diagnostics;
pub mod effect_model;
pub mod effect_row;
pub mod effects;
pub mod env;
pub mod fact_layer;
pub mod lifetime;
pub mod project_index;
pub mod registration;
pub mod resolve;
pub mod semantic;
pub mod style;
pub mod symbols;
pub mod traits;
pub mod types;
pub mod view_part;

#[cfg(test)]
mod tests;
