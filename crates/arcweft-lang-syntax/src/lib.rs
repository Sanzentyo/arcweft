//! Surface parser for `.arcw` source files.
//!
//! This crate owns syntax-level parsing only. It keeps enough structure for
//! formatter, diagnostics, and later HIR lowering, while deliberately avoiding
//! type resolution or runtime semantics.

pub mod ast;
pub mod cst;
pub mod expr;
pub mod lint;
pub mod parser;
pub mod pattern;
pub mod source;
pub mod text;
pub mod types;

#[cfg(test)]
mod tests;
