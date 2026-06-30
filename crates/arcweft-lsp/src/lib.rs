//! Stdio Language Server Protocol transport for Arcweft.
//!
//! This crate owns client state, document synchronization, request dispatch,
//! and transport concerns. LSP value conversion stays in `arcweft-verify-lsp`.

pub mod commands;
pub mod config;
pub mod custom;
pub mod diagnostics;
pub mod documents;
pub mod features;
pub mod positions;
pub mod profiles;
pub mod repl_command;
pub mod server;
pub mod session;

pub use config::LspConfig;
pub use server::{LspServerError, run_stdio};
pub use session::ArcweftLspSession;
