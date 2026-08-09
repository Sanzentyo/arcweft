//! LSP feature handlers built on top of Sans I/O helper crates.

pub mod actions;
pub mod character_definition;
pub mod character_metadata;
pub mod completion;
pub mod definition;
pub(crate) mod dialogue_lines;
mod dialogue_view_metadata;
pub(crate) mod entry_roles;
pub mod hover;
pub mod inlay;
mod nominal_types;
pub mod references;
pub mod rename;
pub mod semantic_tokens;
pub mod signature;
