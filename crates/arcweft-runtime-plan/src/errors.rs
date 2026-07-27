//! Diagnostics emitted while lowering HIR into core runtime plans.

use crate::lowering_context::ExecutableLoweringLocation;
use arcweft_lang_syntax::assertion::AssertionStmt;
use arcweft_lang_syntax::ast::common::TextRange;
use arcweft_source::{Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceSpan};
use std::fmt;
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

    /// Builds the shared diagnostic representation for compiler, CLI, LSP, and Agent surfaces.
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic::new(DiagnosticSeverity::Error, self.message.clone())
            .with_code("runtime.line_task.lower")
    }
}

/// Error produced while converting HIR flows to the executable runtime plan.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct RuntimePlanLowerError {
    message: String,
    reason: String,
    context: Option<Box<RuntimePlanLowerContext>>,
    span: Option<SourceSpan>,
    kind: RuntimePlanLowerErrorKind,
}

/// Stable diagnostic class for runtime-plan lowering failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimePlanLowerErrorKind {
    /// An authored construct cannot be represented by the runtime plan.
    Lowering,
    /// Code generation reached an undischarged `assert.prove` obligation.
    UnresolvedProof,
}

impl RuntimePlanLowerErrorKind {
    /// Stable diagnostic code shared by compiler, CLI, LSP, and Agent surfaces.
    pub const fn diagnostic_code(self) -> &'static str {
        match self {
            Self::Lowering => "runtime.plan.lower",
            Self::UnresolvedProof => "verify.proof.unresolved",
        }
    }
}

/// Authored executable location associated with a runtime-plan lowering error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimePlanLowerContext {
    /// One statement was not representable by the owning runtime plan.
    Statement {
        owner: String,
        path: Vec<String>,
        kind: String,
        source_range: Option<TextRange>,
    },
    /// One expression within a statement failed checked expression lowering.
    Expression {
        owner: String,
        path: Vec<String>,
        statement_kind: String,
        role: String,
        source_range: Option<TextRange>,
    },
    /// One binding/event pattern failed checked runtime-pattern lowering.
    Pattern {
        owner: String,
        path: Vec<String>,
        statement_kind: String,
        role: String,
        source_range: Option<TextRange>,
    },
    /// One host-request argument failed checked runtime expression lowering.
    HostRequestArgument {
        owner: String,
        path: Vec<String>,
        capability: String,
        operation: String,
        argument: RuntimeHostRequestArgument,
        source_range: Option<TextRange>,
    },
    /// An await target did not identify a host capability call.
    HostRequestTarget {
        owner: String,
        path: Vec<String>,
        expression: String,
        source_range: Option<TextRange>,
    },
}

/// Authored argument slot within a host capability request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeHostRequestArgument {
    /// Zero-based positional argument index.
    Positional(usize),
    /// Authored named argument.
    Named(String),
    /// Zero-based argument index of a spread payload.
    Spread(usize),
}

impl RuntimePlanLowerError {
    pub fn new(message: impl Into<String>) -> Self {
        let reason = message.into();
        Self {
            message: reason.clone(),
            reason,
            context: None,
            span: None,
            kind: RuntimePlanLowerErrorKind::Lowering,
        }
    }

    /// Creates an error tied to a structured authored lowering location.
    pub fn in_context(context: RuntimePlanLowerContext, reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self {
            message: format!("{context}: {reason}"),
            reason,
            context: Some(Box::new(context)),
            span: None,
            kind: RuntimePlanLowerErrorKind::Lowering,
        }
    }

    /// Creates the mandatory code-generation failure for an undischarged proof assertion.
    pub(crate) fn unresolved_proof(
        location: &ExecutableLoweringLocation,
        assertion: &AssertionStmt,
    ) -> Self {
        let reason = "compile-time proof assertion was not discharged before runtime-plan lowering"
            .to_owned();
        let context = RuntimePlanLowerContext::statement(
            location.owner(),
            location.path().to_vec(),
            "assertion",
            Some(assertion.range()),
        );
        location.bind_error(Self {
            message: format!("{context}: {reason}"),
            reason,
            context: Some(Box::new(context)),
            span: None,
            kind: RuntimePlanLowerErrorKind::UnresolvedProof,
        })
    }

    /// Human-readable runtime lowering diagnostic.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Underlying failure reason without the rendered location prefix.
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Structured authored location, when the lowering boundary supplied one.
    pub fn context(&self) -> Option<&RuntimePlanLowerContext> {
        self.context.as_deref()
    }

    /// Exact authored source revision retained by this lowering failure.
    pub const fn source(&self) -> Option<&SourceSpan> {
        self.span.as_ref()
    }

    pub(crate) fn with_source(mut self, source: Option<SourceSpan>) -> Self {
        if let Some(source) = &source {
            debug_assert_eq!(
                self.context()
                    .and_then(RuntimePlanLowerContext::source_range)
                    .map(|range| (range.start(), range.end())),
                Some((source.range().start(), source.range().end()))
            );
        }
        self.span = source;
        self
    }

    /// Returns the typed failure class used to select a stable diagnostic code.
    pub const fn kind(&self) -> RuntimePlanLowerErrorKind {
        self.kind
    }

    /// Builds the shared diagnostic representation for compiler, CLI, LSP, and Agent surfaces.
    pub fn diagnostic(&self) -> Diagnostic {
        let diagnostic = Diagnostic::new(DiagnosticSeverity::Error, self.message.clone())
            .with_code(self.kind.diagnostic_code());
        let Some(source) = &self.span else {
            return diagnostic;
        };
        diagnostic.with_label(DiagnosticLabel::primary(
            source.clone(),
            Some(self.reason.clone()),
        ))
    }
}

impl RuntimePlanLowerContext {
    /// Creates a statement context within one runtime-plan owner.
    pub fn statement(
        owner: impl Into<String>,
        path: impl Into<Vec<String>>,
        kind: impl Into<String>,
        source_range: Option<TextRange>,
    ) -> Self {
        Self::Statement {
            owner: owner.into(),
            path: path.into(),
            kind: kind.into(),
            source_range,
        }
    }

    /// Creates an expression context within one runtime-plan statement.
    pub fn expression(
        owner: impl Into<String>,
        path: impl Into<Vec<String>>,
        statement_kind: impl Into<String>,
        role: impl Into<String>,
        source_range: Option<TextRange>,
    ) -> Self {
        Self::Expression {
            owner: owner.into(),
            path: path.into(),
            statement_kind: statement_kind.into(),
            role: role.into(),
            source_range,
        }
    }

    /// Creates a pattern context within one runtime-plan statement or handler.
    pub fn pattern(
        owner: impl Into<String>,
        path: impl Into<Vec<String>>,
        statement_kind: impl Into<String>,
        role: impl Into<String>,
        source_range: Option<TextRange>,
    ) -> Self {
        Self::Pattern {
            owner: owner.into(),
            path: path.into(),
            statement_kind: statement_kind.into(),
            role: role.into(),
            source_range,
        }
    }

    /// Creates checked host-request argument context.
    pub fn host_request_argument(
        owner: impl Into<String>,
        path: impl Into<Vec<String>>,
        capability: impl Into<String>,
        operation: impl Into<String>,
        argument: RuntimeHostRequestArgument,
        source_range: Option<TextRange>,
    ) -> Self {
        Self::HostRequestArgument {
            owner: owner.into(),
            path: path.into(),
            capability: capability.into(),
            operation: operation.into(),
            argument,
            source_range,
        }
    }

    /// Creates context for an unsupported non-call await target.
    pub fn host_request_target(
        owner: impl Into<String>,
        path: impl Into<Vec<String>>,
        expression: impl Into<String>,
        source_range: Option<TextRange>,
    ) -> Self {
        Self::HostRequestTarget {
            owner: owner.into(),
            path: path.into(),
            expression: expression.into(),
            source_range,
        }
    }

    /// Runtime-plan owner label, such as a Stream plan or source handler.
    pub fn owner(&self) -> &str {
        match self {
            Self::Statement { owner, .. }
            | Self::Expression { owner, .. }
            | Self::Pattern { owner, .. }
            | Self::HostRequestArgument { owner, .. }
            | Self::HostRequestTarget { owner, .. } => owner,
        }
    }

    /// Zero-based nested statement path from the owner body.
    pub fn path(&self) -> &[String] {
        match self {
            Self::Statement { path, .. }
            | Self::Expression { path, .. }
            | Self::Pattern { path, .. }
            | Self::HostRequestArgument { path, .. }
            | Self::HostRequestTarget { path, .. } => path,
        }
    }

    /// Authored expression range when the HIR boundary retained it.
    pub const fn source_range(&self) -> Option<TextRange> {
        match self {
            Self::Statement { source_range, .. }
            | Self::Expression { source_range, .. }
            | Self::Pattern { source_range, .. }
            | Self::HostRequestArgument { source_range, .. }
            | Self::HostRequestTarget { source_range, .. } => *source_range,
        }
    }
}

impl fmt::Display for RuntimePlanLowerContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = self
            .path()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(".");
        match self {
            Self::Statement { owner, kind, .. } => {
                write!(formatter, "{owner} statement `{kind}` at {path}")
            }
            Self::Expression {
                owner,
                statement_kind,
                role,
                ..
            } => write!(
                formatter,
                "{owner} statement `{statement_kind}` at {path} expression `{role}`"
            ),
            Self::Pattern {
                owner,
                statement_kind,
                role,
                ..
            } => write!(
                formatter,
                "{owner} statement `{statement_kind}` at {path} pattern `{role}`"
            ),
            Self::HostRequestArgument {
                owner,
                capability,
                operation,
                argument,
                ..
            } => write!(
                formatter,
                "{owner} host request `{capability}.{operation}` argument `{argument}` at {path}"
            ),
            Self::HostRequestTarget {
                owner, expression, ..
            } => {
                write!(
                    formatter,
                    "{owner} host request target `{expression}` at {path}"
                )
            }
        }
    }
}

impl fmt::Display for RuntimeHostRequestArgument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Positional(index) => write!(formatter, "positional #{index}"),
            Self::Named(name) => write!(formatter, "named {name}"),
            Self::Spread(index) => write!(formatter, "spread #{index}"),
        }
    }
}
