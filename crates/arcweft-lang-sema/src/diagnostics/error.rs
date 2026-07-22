use super::{effect_trace::with_effect_trace_notes, trait_diagnostic::TraitDiagnostic};
use crate::{
    effect_diagnostics::EffectDiagnostic, nominal::NominalTypeDiagnostic, style::StyleDiagnostic,
    types::TypeKind,
};
use arcweft_source::{Diagnostic, DiagnosticSeverity};
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
    /// A presentation command received a named argument outside its canonical contract.
    UnknownPresentationArgument { command: String, argument: String },
    /// An assignment target is not an executable lvalue in the current source grammar.
    UnsupportedAssignmentTarget { target: String, reason: String },
    /// An assertion condition did not type as Boolean.
    AssertionConditionNotBool {
        index: usize,
        actual: Option<TypeKind>,
    },
    /// An assertion condition inferred a runtime effect or nondeterministic operation.
    AssertionConditionNotPure {
        index: usize,
        diagnostic: EffectDiagnostic,
    },
    /// A method-call expression matched a data-last callable fallback shape,
    /// but used argument syntax that is not representable by the fallback
    /// lowering contract.
    UnsupportedDataLastMethodFallback { method: String, reason: String },
    /// A signature-backed partial call used argument syntax that is not
    /// representable by the fixed partial-call lowering contract.
    UnsupportedSignaturePartialCall { function: String, reason: String },
    /// A function-value call used argument syntax that is not representable by
    /// the runtime function-value apply contract.
    UnsupportedFunctionValueCall {
        callee: Option<String>,
        reason: String,
    },
    /// A selected method was referenced as a value before Arcweft has a stable
    /// receiver-binding contract for method values.
    UnsupportedMethodValueReference {
        receiver: TypeKind,
        method: String,
        reason: String,
    },
    /// A function-value call supplied more positional arguments than the
    /// function value can accept.
    FunctionValueArityMismatch {
        callee: Option<String>,
        expected: usize,
        actual: usize,
    },
    /// An integer literal's raw digits could not be interpreted as a `u128`
    /// magnitude, so no target-width check or lowering is possible.
    InvalidIntegerLiteral { literal: String, reason: String },
    /// A non-negative integer literal does not fit the type selected by its
    /// suffix, expected type, or stable fallback rule.
    IntegerLiteralOutOfRange { literal: String, target: TypeKind },
    /// A finite source float overflows the selected IEEE width.
    FloatLiteralOutOfRange { literal: String, target: TypeKind },
    /// A method-call expression has more than one viable data-last callable
    /// fallback candidate, so lowering would depend on source ordering or
    /// environment merge order instead of a typed rule.
    AmbiguousDataLastMethodFallback {
        method: String,
        receiver: TypeKind,
        candidates: Vec<String>,
    },
    /// A closure captured a borrowed value and its body contains a suspension
    /// boundary that may outlive the borrowed lifetime.
    BorrowedClosureCaptureCrossesBoundary {
        capture: String,
        ty: TypeKind,
        lifetimes: Vec<String>,
        boundary: String,
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
    /// A structured trait / impl / associated-type diagnostic.
    Trait { diagnostic: TraitDiagnostic },
    /// A structured native Style semantic diagnostic.
    Style { diagnostic: StyleDiagnostic },
    /// A source-backed nominal resolution diagnostic.
    Nominal { diagnostic: NominalTypeDiagnostic },
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

    /// Builds the shared diagnostic representation for compiler, CLI, LSP, and Agent surfaces.
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic::new(DiagnosticSeverity::Error, self.message.clone()).with_code("sema.readiness")
    }
}

impl TypeCheckError {
    pub(crate) fn new(message: String) -> Self {
        Self {
            message,
            kind: TypeCheckErrorKind::Message,
        }
    }

    pub(crate) fn style(diagnostic: StyleDiagnostic) -> Self {
        Self {
            message: diagnostic.message().to_owned(),
            kind: TypeCheckErrorKind::Style { diagnostic },
        }
    }

    pub(crate) fn nominal(diagnostic: NominalTypeDiagnostic) -> Self {
        let message = diagnostic.to_source_diagnostic().map_or_else(
            || "detached nominal resolution failed".to_owned(),
            |value| value.message().to_owned(),
        );
        Self {
            message,
            kind: TypeCheckErrorKind::Nominal { diagnostic },
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

    pub(crate) fn unknown_presentation_argument(
        command: impl Into<String>,
        argument: impl Into<String>,
    ) -> Self {
        let command = command.into();
        let argument = argument.into();
        Self {
            message: format!(
                "presentation call `{command}` does not accept named argument `{argument}`"
            ),
            kind: TypeCheckErrorKind::UnknownPresentationArgument { command, argument },
        }
    }

    pub(crate) fn unsupported_assignment_target(
        target: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let target = target.into();
        let reason = reason.into();
        Self {
            message: format!("unsupported assignment target `{target}`: {reason}"),
            kind: TypeCheckErrorKind::UnsupportedAssignmentTarget { target, reason },
        }
    }

    pub(crate) fn assertion_condition_not_bool(index: usize, actual: Option<TypeKind>) -> Self {
        let actual_label = actual
            .as_ref()
            .map_or_else(|| "an unresolved type".to_owned(), TypeKind::source_label);
        Self {
            message: format!(
                "assertion condition {index} must have type Bool, found {actual_label}"
            ),
            kind: TypeCheckErrorKind::AssertionConditionNotBool { index, actual },
        }
    }

    pub(crate) fn assertion_condition_not_pure(index: usize, diagnostic: EffectDiagnostic) -> Self {
        Self {
            message: format!(
                "assertion condition {index} must be pure and deterministic: {}",
                diagnostic.message()
            ),
            kind: TypeCheckErrorKind::AssertionConditionNotPure { index, diagnostic },
        }
    }

    pub(crate) fn unsupported_data_last_method_fallback(
        method: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let method = method.into();
        let reason = reason.into();
        Self {
            message: format!("data-last method fallback for `{method}` is not available: {reason}"),
            kind: TypeCheckErrorKind::UnsupportedDataLastMethodFallback { method, reason },
        }
    }

    pub(crate) fn unsupported_signature_partial_call(
        function: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let function = function.into();
        let reason = reason.into();
        Self {
            message: format!("partial call for `{function}` is not available: {reason}"),
            kind: TypeCheckErrorKind::UnsupportedSignaturePartialCall { function, reason },
        }
    }

    pub(crate) fn unsupported_function_value_call(
        callee: Option<&str>,
        reason: impl Into<String>,
    ) -> Self {
        let callee = callee.map(str::to_owned);
        let reason = reason.into();
        let target = callee.as_deref().map_or_else(
            || "function value".to_owned(),
            |callee| format!("`{callee}`"),
        );
        Self {
            message: format!("function value call for {target} is not available: {reason}"),
            kind: TypeCheckErrorKind::UnsupportedFunctionValueCall { callee, reason },
        }
    }

    pub(crate) fn unsupported_method_value_reference(
        receiver: TypeKind,
        method: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let method = method.into();
        let reason = reason.into();
        Self {
            message: format!(
                "method value reference `{}.{method}` is not available: {reason}",
                receiver.source_label()
            ),
            kind: TypeCheckErrorKind::UnsupportedMethodValueReference {
                receiver,
                method,
                reason,
            },
        }
    }

    pub(crate) fn function_value_arity_mismatch(
        callee: Option<&str>,
        expected: usize,
        actual: usize,
    ) -> Self {
        let callee = callee.map(str::to_owned);
        let target = callee.as_deref().map_or_else(
            || "function value".to_owned(),
            |callee| format!("`{callee}`"),
        );
        Self {
            message: format!(
                "function value call for {target} expected at most {expected} positional argument(s), got {actual}"
            ),
            kind: TypeCheckErrorKind::FunctionValueArityMismatch {
                callee,
                expected,
                actual,
            },
        }
    }

    pub(crate) fn ambiguous_data_last_method_fallback(
        method: impl Into<String>,
        receiver: TypeKind,
        candidates: impl IntoIterator<Item = String>,
    ) -> Self {
        let method = method.into();
        let candidates = candidates.into_iter().collect::<Vec<_>>();
        Self {
            message: format!(
                "data-last method fallback for `{method}` is ambiguous on {}: candidates are {}",
                receiver.source_label(),
                candidates.join(", ")
            ),
            kind: TypeCheckErrorKind::AmbiguousDataLastMethodFallback {
                method,
                receiver,
                candidates,
            },
        }
    }

    pub(crate) fn borrowed_closure_capture_crosses_boundary(
        capture: impl Into<String>,
        ty: TypeKind,
        lifetimes: Vec<String>,
        boundary: impl Into<String>,
    ) -> Self {
        let capture = capture.into();
        let boundary = boundary.into();
        let lifetime_context = if lifetimes.is_empty() {
            String::new()
        } else {
            format!(" with lifetimes {lifetimes:?}")
        };
        Self {
            message: format!(
                "closure capture `{capture}` of borrowed type {ty:?}{lifetime_context} cannot cross {boundary}"
            ),
            kind: TypeCheckErrorKind::BorrowedClosureCaptureCrossesBoundary {
                capture,
                ty,
                lifetimes,
                boundary,
            },
        }
    }

    pub(crate) fn invalid_integer_literal(
        literal: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let literal = literal.into();
        let reason = reason.into();
        Self {
            message: format!("integer literal `{literal}` is invalid: {reason}"),
            kind: TypeCheckErrorKind::InvalidIntegerLiteral { literal, reason },
        }
    }

    pub(crate) fn integer_literal_out_of_range(
        literal: impl Into<String>,
        target: TypeKind,
    ) -> Self {
        let literal = literal.into();
        Self {
            message: format!(
                "integer literal `{literal}` is out of range for {}",
                target.source_label()
            ),
            kind: TypeCheckErrorKind::IntegerLiteralOutOfRange { literal, target },
        }
    }

    pub(crate) fn float_literal_out_of_range(literal: impl Into<String>, target: TypeKind) -> Self {
        let literal = literal.into();
        Self {
            message: format!(
                "float literal `{literal}` overflows {}",
                target.source_label()
            ),
            kind: TypeCheckErrorKind::FloatLiteralOutOfRange { literal, target },
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

    pub(crate) fn trait_diagnostic(diagnostic: TraitDiagnostic) -> Self {
        Self {
            message: diagnostic.message().to_owned(),
            kind: TypeCheckErrorKind::Trait { diagnostic },
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

    /// Stable compiler-wide diagnostic code.
    pub fn stable_code(&self) -> String {
        typecheck_error_code(&self.kind)
    }

    /// Builds the shared diagnostic representation for compiler, CLI, LSP, and Agent surfaces.
    pub fn diagnostic(&self) -> Diagnostic {
        let diagnostic = Diagnostic::new(DiagnosticSeverity::Error, self.message.clone())
            .with_code(self.stable_code());
        match &self.kind {
            TypeCheckErrorKind::Effect { diagnostic: effect }
            | TypeCheckErrorKind::AssertionConditionNotPure {
                diagnostic: effect, ..
            } => with_effect_trace_notes(diagnostic, effect),
            TypeCheckErrorKind::Message
            | TypeCheckErrorKind::ArgumentTypeMismatch { .. }
            | TypeCheckErrorKind::UnknownPresentationArgument { .. }
            | TypeCheckErrorKind::UnsupportedAssignmentTarget { .. }
            | TypeCheckErrorKind::AssertionConditionNotBool { .. }
            | TypeCheckErrorKind::UnsupportedDataLastMethodFallback { .. }
            | TypeCheckErrorKind::UnsupportedSignaturePartialCall { .. }
            | TypeCheckErrorKind::UnsupportedFunctionValueCall { .. }
            | TypeCheckErrorKind::UnsupportedMethodValueReference { .. }
            | TypeCheckErrorKind::FunctionValueArityMismatch { .. }
            | TypeCheckErrorKind::InvalidIntegerLiteral { .. }
            | TypeCheckErrorKind::IntegerLiteralOutOfRange { .. }
            | TypeCheckErrorKind::FloatLiteralOutOfRange { .. }
            | TypeCheckErrorKind::AmbiguousDataLastMethodFallback { .. }
            | TypeCheckErrorKind::BorrowedClosureCaptureCrossesBoundary { .. }
            | TypeCheckErrorKind::MissingRustPackageMetadata { .. }
            | TypeCheckErrorKind::MissingRustExport { .. }
            | TypeCheckErrorKind::RustExportSignatureMismatch { .. }
            | TypeCheckErrorKind::InlineCallErrorPolicyMissing { .. }
            | TypeCheckErrorKind::InlineFailurePolicyConflict { .. }
            | TypeCheckErrorKind::UnknownInlineFailurePolicy { .. }
            | TypeCheckErrorKind::Trait { .. }
            | TypeCheckErrorKind::Style { .. } => diagnostic,
            TypeCheckErrorKind::Nominal {
                diagnostic: nominal,
            } => nominal.to_source_diagnostic().unwrap_or(diagnostic),
        }
    }
}

fn typecheck_error_code(kind: &TypeCheckErrorKind) -> String {
    match kind {
        TypeCheckErrorKind::Message => "sema.typecheck".to_owned(),
        TypeCheckErrorKind::ArgumentTypeMismatch { .. } => {
            "sema.typecheck.argument_type_mismatch".to_owned()
        }
        TypeCheckErrorKind::UnknownPresentationArgument { .. } => {
            "sema.presentation.unknown_argument".to_owned()
        }
        TypeCheckErrorKind::UnsupportedAssignmentTarget { .. } => {
            "sema.typecheck.unsupported_assignment_target".to_owned()
        }
        TypeCheckErrorKind::AssertionConditionNotBool { .. } => {
            "sema.assert.condition_not_bool".to_owned()
        }
        TypeCheckErrorKind::AssertionConditionNotPure { .. } => {
            "sema.assert.condition_not_pure".to_owned()
        }
        TypeCheckErrorKind::UnsupportedDataLastMethodFallback { .. } => {
            "sema.typecheck.unsupported_data_last_method_fallback".to_owned()
        }
        TypeCheckErrorKind::UnsupportedSignaturePartialCall { .. } => {
            "sema.typecheck.unsupported_signature_partial_call".to_owned()
        }
        TypeCheckErrorKind::UnsupportedFunctionValueCall { .. } => {
            "sema.typecheck.unsupported_function_value_call".to_owned()
        }
        TypeCheckErrorKind::UnsupportedMethodValueReference { .. } => {
            "sema.typecheck.unsupported_method_value_reference".to_owned()
        }
        TypeCheckErrorKind::FunctionValueArityMismatch { .. } => {
            "sema.typecheck.function_value_arity_mismatch".to_owned()
        }
        TypeCheckErrorKind::InvalidIntegerLiteral { .. } => {
            "sema.numeric.invalid_integer_literal".to_owned()
        }
        TypeCheckErrorKind::IntegerLiteralOutOfRange { .. } => {
            "sema.numeric.integer_out_of_range".to_owned()
        }
        TypeCheckErrorKind::FloatLiteralOutOfRange { .. } => {
            "sema.numeric.float_out_of_range".to_owned()
        }
        TypeCheckErrorKind::AmbiguousDataLastMethodFallback { .. } => {
            "sema.typecheck.ambiguous_data_last_method_fallback".to_owned()
        }
        TypeCheckErrorKind::BorrowedClosureCaptureCrossesBoundary { .. } => {
            "sema.typecheck.borrowed_closure_capture_crosses_boundary".to_owned()
        }
        TypeCheckErrorKind::MissingRustPackageMetadata { .. } => {
            "sema.extern_rust.missing_metadata".to_owned()
        }
        TypeCheckErrorKind::MissingRustExport { .. } => {
            "sema.extern_rust.missing_export".to_owned()
        }
        TypeCheckErrorKind::RustExportSignatureMismatch { .. } => {
            "sema.extern_rust.signature_mismatch".to_owned()
        }
        TypeCheckErrorKind::InlineCallErrorPolicyMissing { .. } => {
            "sema.dialogue.inline_error_policy_missing".to_owned()
        }
        TypeCheckErrorKind::InlineFailurePolicyConflict { .. } => {
            "sema.dialogue.inline_failure_policy_conflict".to_owned()
        }
        TypeCheckErrorKind::UnknownInlineFailurePolicy { .. } => {
            "sema.dialogue.unknown_inline_failure_policy".to_owned()
        }
        TypeCheckErrorKind::Effect { diagnostic } => diagnostic.code().as_str().to_owned(),
        TypeCheckErrorKind::Trait { diagnostic } => diagnostic.code().to_owned(),
        TypeCheckErrorKind::Style { diagnostic } => diagnostic.code().as_str().to_owned(),
        TypeCheckErrorKind::Nominal { diagnostic } => diagnostic.kind().code().as_str().to_owned(),
    }
}
