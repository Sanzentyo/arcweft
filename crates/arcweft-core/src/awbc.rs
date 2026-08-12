//! Canonical Arcweft executable bytecode (AWBC) contracts.
//!
//! AWBC is the single executor-neutral executable representation shared by the
//! compact VM and compiled regions. The module is Sans I/O: it owns typed data,
//! canonical bytes, validation, VM stepping, and fiber state, but no filesystem,
//! network, dynamic-library, or executable-memory operations.

pub mod codec;
pub mod fiber;
mod identity;
pub mod parity;
pub mod product_step;
pub mod schema;
mod schema_impls;
pub mod type_projection;
pub mod verify;
pub mod vm;

#[cfg(test)]
mod tests;
