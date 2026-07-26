//! Semantic projection of language-free adapter manifests.
//!
//! `arcweft-adapter-context` owns manifest data used by runtimes and tooling.
//! This bridge owns source-backed HIR/sema registration so runtime crates never
//! acquire compiler-layer dependencies through Cargo feature unification.

#![forbid(unsafe_code)]

pub mod registration;
