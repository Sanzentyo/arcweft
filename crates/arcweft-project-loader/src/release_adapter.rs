//! Adapter-level release publication, verification, and materialization helpers.
//!
//! These modules own filesystem/cache behavior around the Sans I/O AWFR and
//! signing-policy types. They intentionally do not choose production key stores,
//! remote signing services, platform trust stores, or CDN APIs.

pub mod consume;
pub mod materialize;
pub mod publish;
pub mod trust;
