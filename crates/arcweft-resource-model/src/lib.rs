//! Sans-I/O typed resource identity, schema, descriptor, and immutable
//! registry contracts.
//!
//! This crate deliberately owns no source parser, HIR, semantic environment,
//! bundle section, runtime handle, filesystem access, or package discovery.

pub mod canonical;
pub mod descriptor;
pub mod identity;
pub mod registry;
pub mod retained;
pub mod value;
