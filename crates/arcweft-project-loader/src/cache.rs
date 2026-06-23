//! Filesystem-backed incremental cache adapter.
//!
//! This module stores immutable content-addressed objects and immutable
//! key-addressed records. It does not execute compiler queries.

pub mod inspect;
pub mod lock;
pub mod record;
pub mod release;
pub mod store;
