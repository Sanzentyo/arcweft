//! Sans I/O script test and bench planning.
//!
//! The crate extracts `test` and `bench` declarations from HIR into a stable
//! manifest that CLI, LSP, and future runtime adapters can consume. It does not
//! open files, drive a renderer, sleep, or run benchmark timers.

pub mod agent;
mod script_manifest;

pub use script_manifest::{
    BenchSection, ManifestSpan, ScriptBench, ScriptCommand, ScriptExpectation, ScriptStep,
    ScriptTest, ScriptTestManifest, ScriptVirtualPath, ScriptVirtualPathRoot, collect_script_tests,
};
