use thiserror::Error;

/// Semantic type-checking diagnostic.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct TypeCheckError {
    message: String,
}

/// Syntax-to-HIR readiness error for the future type checker.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct TypeCheckReadinessError {
    message: String,
}

impl TypeCheckReadinessError {
    pub(crate) fn new(message: String) -> Self {
        Self { message }
    }

    /// Human-readable readiness failure.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl TypeCheckError {
    pub(crate) fn new(message: String) -> Self {
        Self { message }
    }

    /// Human-readable type-checking failure.
    pub fn message(&self) -> &str {
        &self.message
    }
}
