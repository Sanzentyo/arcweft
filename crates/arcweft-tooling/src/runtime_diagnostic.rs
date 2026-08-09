//! Shared presentation for typed runtime assertion failures.

use std::sync::Arc;

use arcweft_core::effect::RuntimeAssertionFailure;
use arcweft_runtime_plan::assertion_identity::{RuntimeAssertionFault, RuntimeAssertionMode};
use arcweft_source::{Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceSpan};

/// Stable diagnostic code for every runtime assertion failure.
pub const RUNTIME_ASSERTION_FAILED_CODE: &str = "runtime.assertion_failed";

/// Typed identity available to a runtime assertion diagnostic.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RuntimeAssertionDiagnosticIdentity {
    /// Exact identity projected through a matching fresh-session inventory.
    Session {
        mode: RuntimeAssertionMode,
        condition_index: u8,
    },
    /// Only persisted core/source-map evidence was available.
    PersistedOnly,
}

/// One source label in a runtime diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDiagnosticLabel {
    span: SourceSpan,
    message: Arc<str>,
}

impl RuntimeDiagnosticLabel {
    fn new(span: SourceSpan, message: impl Into<Arc<str>>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }

    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Adapter-neutral runtime assertion diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAssertionDiagnostic {
    message: String,
    primary: Option<RuntimeDiagnosticLabel>,
    secondary: Box<[RuntimeDiagnosticLabel]>,
    identity: RuntimeAssertionDiagnosticIdentity,
}

impl RuntimeAssertionDiagnostic {
    fn new(
        message: String,
        primary: Option<RuntimeDiagnosticLabel>,
        secondary: impl Into<Box<[RuntimeDiagnosticLabel]>>,
        identity: RuntimeAssertionDiagnosticIdentity,
    ) -> Self {
        Self {
            message,
            primary,
            secondary: secondary.into(),
            identity,
        }
    }

    pub const fn code(&self) -> &'static str {
        RUNTIME_ASSERTION_FAILED_CODE
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn primary(&self) -> Option<&RuntimeDiagnosticLabel> {
        self.primary.as_ref()
    }

    pub fn secondary(&self) -> &[RuntimeDiagnosticLabel] {
        &self.secondary
    }

    pub const fn identity(&self) -> &RuntimeAssertionDiagnosticIdentity {
        &self.identity
    }

    /// Projects the shared runtime model into the existing source-diagnostic
    /// boundary consumed by CLI, LSP, Agent, and debug adapters.
    ///
    /// Session-only HIR identifiers remain in this in-memory object; the
    /// returned diagnostic contains only the stable code, message, and exact
    /// source spans already admitted by the projection path.
    pub fn to_source_diagnostic(&self) -> Diagnostic {
        let mut diagnostic = Diagnostic::new(DiagnosticSeverity::Error, self.message.clone())
            .with_code(RUNTIME_ASSERTION_FAILED_CODE);
        if let Some(primary) = &self.primary {
            diagnostic = diagnostic.with_label(DiagnosticLabel::primary(
                primary.span.clone(),
                Some(primary.message.to_string()),
            ));
        }
        for secondary in &self.secondary {
            diagnostic = diagnostic.with_label(DiagnosticLabel::secondary(
                secondary.span.clone(),
                Some(secondary.message.to_string()),
            ));
        }
        diagnostic
    }
}

/// Projects an exact fresh-session fault without parsing runtime strings.
pub fn project_runtime_assertion_fault(
    fault: &RuntimeAssertionFault,
) -> RuntimeAssertionDiagnostic {
    let condition = fault.identity().condition().get();
    let message = materialized_failure_message(fault.observed().message(), condition);
    let primary = RuntimeDiagnosticLabel::new(
        fault.identity().span().clone(),
        Arc::<str>::from(fault.presentation().condition_label()),
    );
    let secondary = RuntimeDiagnosticLabel::new(
        fault.presentation().statement_span().clone(),
        Arc::<str>::from("assertion statement"),
    );
    RuntimeAssertionDiagnostic::new(
        message,
        Some(primary),
        vec![secondary],
        RuntimeAssertionDiagnosticIdentity::Session {
            mode: fault.identity().mode(),
            condition_index: condition,
        },
    )
}

/// Projects a persisted failure when no exact fresh-session inventory exists.
///
/// The optional span must come from persisted source-map evidence. This path
/// never claims a statement ID, condition index, or runtime assertion mode.
pub fn project_persisted_assertion_failure(
    failure: &RuntimeAssertionFailure,
    persisted_span: Option<SourceSpan>,
) -> RuntimeAssertionDiagnostic {
    let assertion = failure.assertion();
    let message = if assertion.message().is_empty() {
        "runtime assertion failed".to_owned()
    } else {
        assertion.message().to_owned()
    };
    let primary = persisted_span.map(|span| {
        let label = if assertion.condition().is_empty() {
            Arc::<str>::from("assertion condition")
        } else {
            Arc::<str>::from(assertion.condition())
        };
        RuntimeDiagnosticLabel::new(span, label)
    });
    RuntimeAssertionDiagnostic::new(
        message,
        primary,
        Vec::new(),
        RuntimeAssertionDiagnosticIdentity::PersistedOnly,
    )
}

fn materialized_failure_message(message: &str, condition_index: u8) -> String {
    if message.is_empty() {
        format!("assertion condition {condition_index} failed")
    } else {
        message.to_owned()
    }
}
