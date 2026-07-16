//! Verified entry identities, role metadata, and persistent value schemas.
//!
//! These types are data-only runtime contracts. They contain no source-name
//! resolution, manifest access, project I/O, or adapter calls.

mod identity;
mod roles;
mod schema;

pub use identity::*;
pub use roles::*;
pub use schema::*;
