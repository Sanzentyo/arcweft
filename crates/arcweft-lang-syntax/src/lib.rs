//! Surface parser for `.arcw` source files.
//!
//! This crate owns syntax-level parsing only. It keeps enough structure for
//! formatter, diagnostics, and later HIR lowering, while deliberately avoiding
//! type resolution or runtime semantics.

pub mod assertion;
pub mod ast;
mod attachment;
mod cache_facts;
pub mod cst;
pub mod expr;
mod grammar;
pub mod incremental;
pub mod lint;
pub mod parser;
pub mod pattern;
pub mod reference;
pub mod source;
pub mod text;
pub mod types;

#[cfg(test)]
mod tests;
