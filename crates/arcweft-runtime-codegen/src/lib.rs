//! Executor-neutral full-script code-generation contracts.
//!
//! This crate does not allocate executable memory or perform host I/O. Backends
//! implement compiled regions against the safe Rust ABI in [`region`].

pub mod artifact;
pub mod awbc_region;
pub mod cache;
pub mod policy;
pub mod region;

#[cfg(test)]
mod tests;
