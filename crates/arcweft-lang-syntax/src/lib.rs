//! Surface parser for `.arcw` source files.
//!
//! This crate owns syntax-level parsing only. It keeps enough structure for
//! formatter, diagnostics, and later HIR lowering, while deliberately avoiding
//! type resolution or runtime semantics.

pub mod assertion;
pub mod ast;
pub mod attachment;
pub mod cst;
pub mod expressions;
pub mod grammar;
pub mod id_ref;
pub mod incremental;
pub mod lint;
pub mod literal;
pub mod name;
pub mod parser;
pub mod patterns;
pub mod reference;
pub mod text;
pub mod types;
