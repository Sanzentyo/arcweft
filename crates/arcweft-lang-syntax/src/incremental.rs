//! Session-local syntax identities and transaction limits.

mod bound;
mod database;
pub mod limits;
mod reconcile;
mod shape;
mod transaction;

pub use database::{
    InvalidEditSet, ParseFailure, ParseStatus, ParsedSource, SyntaxDatabase, SyntaxIdentityKind,
    SyntaxIdentityMap, SyntaxNodeId,
};
pub use limits::SyntaxLimit;
