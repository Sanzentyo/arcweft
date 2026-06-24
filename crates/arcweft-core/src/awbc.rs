//! Canonical Arcweft executable bytecode (AWBC) contracts.
//!
//! AWBC is the single executor-neutral executable representation shared by the
//! compact VM and compiled regions. The module is Sans I/O: it owns typed data,
//! canonical bytes, validation, and fiber state, but no filesystem, network,
//! dynamic-library, or executable-memory operations.

pub mod codec;
pub mod fiber;
pub mod schema;
pub mod verify;

#[cfg(test)]
mod tests;
