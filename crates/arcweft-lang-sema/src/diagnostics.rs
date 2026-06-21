use crate::{effect_diagnostics::EffectDiagnostic, types::TypeKind};
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
    /// An `extern rust mod` declaration references a package without loaded ABI metadata.
    MissingRustPackageMetadata { package: String },
    /// An `extern rust mod` member is not present in loaded ABI metadata.
    MissingRustExport { package: String, export: String },
    /// An `extern rust mod` function signature differs from loaded ABI metadata.
    RustExportSignatureMismatch {
        package: String,
        export: String,
        expected: String,
        actual: String,
    },
    /// An inline dialogue function call omitted explicit error handling.
    InlineCallErrorPolicyMissing { function: String },
    /// An inline dialogue function call declared more than one failure policy.
    InlineFailurePolicyConflict { function: String },
    /// An inline dialogue function call declared an unknown failure policy.
    UnknownInlineFailurePolicy { function: String, policy: String },
    /// A structured error or warning from transitive effect analysis.
    Effect { diagnostic: EffectDiagnostic },
}

/// Non-fatal semantic lint emitted by type analysis.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct TypeCheckWarning {
    message: String,
    kind: TypeCheckWarningKind,
}

/// Machine-readable type-checking warning family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeCheckWarningKind {
    /// A public signature exposes an anonymous sum type that should be nominal.
    PublicAbiAnonymousSum {
        /// Public signature position that exposes the anonymous sum.
        context: String,
        /// Source-level type expression for the anonymous sum.
        type_ref: String,
    },
    /// A structured warning from transitive effect analysis.
    Effect { diagnostic: EffectDiagnostic },
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

    pub(crate) fn inline_call_error_policy_missing(function: impl Into<String>) -> Self {
        let function = function.into();
        Self {
            message: format!(
                "inline dialogue call `{function}` must declare `on_error`, `fallback`, or `discard_error`"
            ),
            kind: TypeCheckErrorKind::InlineCallErrorPolicyMissing { function },
        }
    }

    pub(crate) fn inline_failure_policy_conflict(function: impl Into<String>) -> Self {
        let function = function.into();
        Self {
            message: format!(
                "inline dialogue call `{function}` declares multiple failure policies; use exactly one of `on_error`, `fallback`, or `discard_error`"
            ),
            kind: TypeCheckErrorKind::InlineFailurePolicyConflict { function },
        }
    }

    pub(crate) fn unknown_inline_failure_policy(
        function: impl Into<String>,
        policy: impl Into<String>,
    ) -> Self {
        let function = function.into();
        let policy = policy.into();
        Self {
            message: format!(
                "inline dialogue call `{function}` uses unknown inline failure policy `{policy}`"
            ),
            kind: TypeCheckErrorKind::UnknownInlineFailurePolicy { function, policy },
        }
    }

    pub(crate) fn effect(diagnostic: EffectDiagnostic) -> Self {
        Self {
            message: diagnostic.message().to_owned(),
            kind: TypeCheckErrorKind::Effect { diagnostic },
        }
    }

    pub(crate) fn missing_rust_package_metadata(package: impl Into<String>) -> Self {
        let package = package.into();
        Self {
            message: format!(
                "extern rust module imports crate `{package}`, but no Rust ABI metadata for that crate was loaded"
            ),
            kind: TypeCheckErrorKind::MissingRustPackageMetadata { package },
        }
    }

    pub(crate) fn missing_rust_export(
        package: impl Into<String>,
        export: impl Into<String>,
    ) -> Self {
        let package = package.into();
        let export = export.into();
        Self {
            message: format!(
                "extern rust module imports `{export}` from crate `{package}`, but the export is missing from loaded Rust ABI metadata"
            ),
            kind: TypeCheckErrorKind::MissingRustExport { package, export },
        }
    }

    pub(crate) fn rust_export_signature_mismatch(
        package: impl Into<String>,
        export: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        let package = package.into();
        let export = export.into();
        let expected = expected.into();
        let actual = actual.into();
        Self {
            message: format!(
                "extern rust export `{export}` from crate `{package}` has signature `{actual}`, but Arcweft declared `{expected}`"
            ),
            kind: TypeCheckErrorKind::RustExportSignatureMismatch {
                package,
                export,
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
    pub(crate) fn public_abi_anonymous_sum(
        context: impl Into<String>,
        type_ref: impl Into<String>,
    ) -> Self {
        let context = context.into();
        let type_ref = type_ref.into();
        Self {
            message: format!(
                "{context} exposes anonymous sum `{type_ref}`; public ABI and save data are more stable with a nominal enum"
            ),
            kind: TypeCheckWarningKind::PublicAbiAnonymousSum { context, type_ref },
        }
    }

    pub(crate) fn effect(diagnostic: EffectDiagnostic) -> Self {
        Self {
            message: diagnostic.message().to_owned(),
            kind: TypeCheckWarningKind::Effect { diagnostic },
        }
    }

    /// Human-readable type-analysis warning.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Machine-readable warning family and structured fields.
    pub const fn kind(&self) -> &TypeCheckWarningKind {
        &self.kind
    }
}
