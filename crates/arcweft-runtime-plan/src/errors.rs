//! Diagnostics emitted while lowering HIR into core runtime plans.

use thiserror::Error;

/// Error produced while converting syntax/HIR line plans to core data.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct LinePlanLowerError {
    message: String,
}

impl LinePlanLowerError {
    pub(crate) fn new(message: String) -> Self {
        Self { message }
    }

    /// Human-readable lowering diagnostic.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Error produced while converting HIR flows to the executable runtime plan.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct RuntimePlanLowerError {
    message: String,
}

impl RuntimePlanLowerError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Human-readable runtime lowering diagnostic.
    pub fn message(&self) -> &str {
        &self.message
    }
}
