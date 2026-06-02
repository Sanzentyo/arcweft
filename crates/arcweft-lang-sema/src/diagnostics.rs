use crate::types::TypeKind;
use thiserror::Error;

/// Semantic type-checking diagnostic.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct TypeCheckError {
    message: String,
    kind: TypeCheckErrorKind,
}

/// Machine-readable type-checking diagnostic family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeCheckErrorKind {
    /// General diagnostic while older checker paths are being structured.
    Message,
    /// A function or method argument did not match the declared parameter type.
    ArgumentTypeMismatch {
        function: String,
        argument: String,
        expected: TypeKind,
        actual: TypeKind,
    },
}

/// Non-fatal semantic lint emitted by type analysis.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct TypeCheckWarning {
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
        Self {
            message,
            kind: TypeCheckErrorKind::Message,
        }
    }

    pub(crate) fn argument_type_mismatch(
        function: impl Into<String>,
        argument: impl Into<String>,
        expected: TypeKind,
        actual: TypeKind,
    ) -> Self {
        let function = function.into();
        let argument = argument.into();
        let message = format!(
            "function `{function}` argument `{argument}` must have type {expected:?}, found {actual:?}"
        );
        Self {
            message,
            kind: TypeCheckErrorKind::ArgumentTypeMismatch {
                function,
                argument,
                expected,
                actual,
            },
        }
    }

    /// Human-readable type-checking failure.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Machine-readable diagnostic family and structured fields.
    pub const fn kind(&self) -> &TypeCheckErrorKind {
        &self.kind
    }
}

impl TypeCheckWarning {
    pub(crate) fn new(message: String) -> Self {
        Self { message }
    }

    /// Human-readable type-analysis warning.
    pub fn message(&self) -> &str {
        &self.message
    }
}
