//! Final-outcome diagnostics and their shared source projection.

use arcweft_lang_hir::{
    module::HirModule,
    source_index::{HirExprSourceRole, HirSourceQuery},
};
use arcweft_source::{Diagnostic, DiagnosticLabel, DiagnosticSeverity};

use super::{
    CallAnalysisOutcome, CallableDiagnostic, CallableDiagnosticCode, CallableDiagnosticSeverity,
    CallableDiagnosticSubject, CallableLimits, CallableQueryLimitError, SemanticSignatureError,
    UnknownCallKind,
};

impl CallAnalysisOutcome {
    /// Only the final outcome may issue a call diagnostic. Candidate probes and
    /// discarded semantic branches cannot supply a separate diagnostic row.
    pub(super) fn seal_diagnostics(
        &self,
        module: &HirModule,
        limits: &CallableLimits,
    ) -> Result<Vec<CallableDiagnostic>, SemanticSignatureError> {
        let code = match self {
            Self::Selected(_) => return Ok(Vec::new()),
            Self::Ambiguous(_) => CallableDiagnosticCode::AmbiguousOverload,
            Self::Rejected(_) => CallableDiagnosticCode::NoViableSignature,
            Self::NonCallable(_) => CallableDiagnosticCode::NonCallableTarget,
            Self::Missing(evidence) => match evidence.kind() {
                UnknownCallKind::Method => CallableDiagnosticCode::UnknownMethod,
                UnknownCallKind::Free | UnknownCallKind::AssociatedType => {
                    CallableDiagnosticCode::UnknownCallable
                }
            },
        };
        if limits.max_diagnostics() == 0 {
            return Err(CallableQueryLimitError::Diagnostics {
                actual: 1,
                limit: 0,
            }
            .into());
        }
        let span = module
            .source_anchor(HirSourceQuery::Expr {
                owner: self.site().expression(),
                role: HirExprSourceRole::Whole,
            })?
            .ok_or(SemanticSignatureError::InvalidSpan)?;
        Ok(vec![CallableDiagnostic::try_new(
            code,
            CallableDiagnosticSeverity::Error,
            Some(span),
            CallableDiagnosticSubject::None,
            Vec::new(),
            Some(module.provenance().source_identity()),
            limits,
        )?])
    }
}

impl CallableDiagnostic {
    /// Projects the same sealed diagnostic for compiler, CLI and editor use.
    pub fn to_source_diagnostic(&self) -> Diagnostic {
        let severity = match self.severity() {
            CallableDiagnosticSeverity::Error => DiagnosticSeverity::Error,
            CallableDiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
            CallableDiagnosticSeverity::Information => DiagnosticSeverity::Info,
        };
        let mut diagnostic =
            Diagnostic::new(severity, self.code().message()).with_code(self.code().as_str());
        if let Some(span) = self.span() {
            diagnostic = diagnostic.with_label(DiagnosticLabel::primary(span.clone(), None));
        }
        for related in self.related() {
            if let Some(span) = related.span() {
                diagnostic = diagnostic.with_label(DiagnosticLabel::secondary(span.clone(), None));
            }
        }
        diagnostic
    }
}

impl CallableDiagnosticCode {
    /// Stable source-diagnostic code shared by every presentation consumer.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownCallable => "sema.call.unknown_callable",
            Self::UnknownMethod => "sema.call.unknown_method",
            Self::NonCallableTarget => "sema.call.non_callable_target",
            Self::UnknownFxConstructor => "sema.call.unknown_fx_constructor",
            Self::InvalidFxPath => "sema.call.invalid_fx_path",
            Self::AmbiguousOverload => "sema.call.ambiguous_overload",
            Self::NoViableSignature => "sema.call.no_viable_signature",
            Self::DiagnosticsTruncated => "sema.call.diagnostics_truncated",
            Self::InaccessibleMethod => "sema.call.inaccessible_method",
            Self::DuplicateArgument => "sema.call.duplicate_argument",
            Self::ParameterAlreadyBound => "sema.call.parameter_already_bound",
            Self::UnknownNamedArgument => "sema.call.unknown_named_argument",
            Self::MissingArgument => "sema.call.missing_argument",
            Self::TooManyPositionalArguments => "sema.call.too_many_positional_arguments",
            Self::UnsupportedSpread => "sema.call.unsupported_spread",
            Self::InvalidCallGroup => "sema.call.invalid_group",
            Self::ArgumentTypeMismatch => "sema.call.argument_type_mismatch",
            Self::ResultConstructorExpectedType => "sema.call.result_constructor_expected_type",
            Self::EnumConstructorExpectedType => "sema.call.enum_constructor_expected_type",
            Self::VirtualPathRejected => "sema.call.virtual_path_rejected",
            Self::CorruptCallableCatalog => "sema.call.corrupt_catalog",
            Self::UnsupportedProjectParameterDefault => "sema.call.unsupported_parameter_default",
            Self::WorldMismatch => "sema.call.world_mismatch",
            Self::SourceIdentityMismatch => "sema.call.source_identity_mismatch",
            Self::Cancelled => "sema.call.cancelled",
            Self::DeadlineExceeded => "sema.call.deadline_exceeded",
            Self::ResourceExhausted => "sema.call.resource_exhausted",
        }
    }

    const fn message(self) -> &'static str {
        match self {
            Self::UnknownCallable => "no callable declaration resolves this call",
            Self::UnknownMethod => "no method resolves this call",
            Self::NonCallableTarget => "the called expression does not have a callable type",
            Self::UnknownFxConstructor => "the Fx constructor is unknown",
            Self::InvalidFxPath => "the Fx constructor path is invalid",
            Self::AmbiguousOverload => "more than one callable signature matches this call",
            Self::NoViableSignature => "no callable signature satisfies this call",
            Self::DiagnosticsTruncated => "additional callable diagnostics were omitted",
            Self::InaccessibleMethod => "the method is not accessible here",
            Self::DuplicateArgument => "an argument was supplied more than once",
            Self::ParameterAlreadyBound => "the parameter was already bound by an earlier group",
            Self::UnknownNamedArgument => "the callable has no parameter with this argument name",
            Self::MissingArgument => "a required argument is missing",
            Self::TooManyPositionalArguments => "too many positional arguments were supplied",
            Self::UnsupportedSpread => "this callable does not accept this argument spread",
            Self::InvalidCallGroup => "the call group is invalid",
            Self::ArgumentTypeMismatch => "the argument type does not match the parameter",
            Self::ResultConstructorExpectedType => "the result constructor needs an expected type",
            Self::EnumConstructorExpectedType => "the enum constructor needs an expected type",
            Self::VirtualPathRejected => "the virtual callable path was rejected",
            Self::CorruptCallableCatalog => "the callable catalog is inconsistent",
            Self::UnsupportedProjectParameterDefault => "the parameter default is unsupported",
            Self::WorldMismatch => "the call belongs to a different semantic world",
            Self::SourceIdentityMismatch => "the call belongs to a different source revision",
            Self::Cancelled => "callable analysis was cancelled",
            Self::DeadlineExceeded => "callable analysis exceeded its deadline",
            Self::ResourceExhausted => "callable analysis exceeded its resource limit",
        }
    }
}
