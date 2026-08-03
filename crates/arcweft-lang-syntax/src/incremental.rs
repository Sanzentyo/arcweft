//! Session-local syntax identities and transaction limits.

mod bound;
mod database;
pub mod limits;
mod reconcile;
mod shape;
mod transaction;

pub use bound::SyntaxDiagnostic;
pub use database::{
    InvalidEditSet, ParseFailure, ParseStatus, ParsedSource, SyntaxDatabase,
    SyntaxDatabaseCreateError, SyntaxInvariantFailure,
};
pub use limits::SyntaxLimit;
