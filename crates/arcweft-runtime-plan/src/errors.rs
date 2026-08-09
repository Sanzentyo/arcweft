//! Diagnostics emitted while lowering final HIR into core runtime plans.

use arcweft_source::{Diagnostic, DiagnosticSeverity};
use thiserror::Error;

/// Error produced while projecting accepted final HIR into an executable runtime plan.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct RuntimePlanLowerError {
    message: String,
}

impl RuntimePlanLowerError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Human-readable runtime lowering diagnostic.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Builds the shared diagnostic representation for compiler, CLI, LSP, and Agent surfaces.
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic::new(DiagnosticSeverity::Error, self.message.clone())
            .with_code("runtime.plan.lower")
    }
}
