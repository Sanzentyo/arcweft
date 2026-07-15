//! Source-backed LSP profile loading and atomically published semantic environments.

pub mod cache;
mod diagnostic;
mod environment;
mod load;
mod model;
mod uri;

pub use diagnostic::{LspProfileDiagnostic, LspProfileDiagnosticKind};
pub use load::LspProfileResolver;
pub use model::{LspProfile, ProfileSourceSelection};

pub(crate) use environment::register_loaded_environment;
pub(crate) use uri::file_path_from_uri;

#[cfg(test)]
mod tests;
