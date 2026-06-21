use crate::{
    effect_model::{CallableId, EffectSite},
    effects::{EffectId, EffectSet},
};

/// Stable machine-readable diagnostic code.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EffectDiagnosticCode {
    MissingDeclaration,
    ForbiddenEffect,
    PureCallableEffect,
    ExplicitDeclarationRequired,
    UnknownLocalCallable,
    DynamicSignatureRequired,
    CapabilityUnavailable,
    OverdeclaredEffect,
}

/// Diagnostic severity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectSeverity {
    Error,
    Warning,
}

/// Structured effect diagnostic family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectDiagnosticKind {
    MissingDeclaration {
        missing: EffectSet,
        declared: EffectSet,
    },
    ForbiddenEffect {
        forbidden: EffectSet,
    },
    PureCallableEffect {
        inferred: EffectSet,
    },
    ExplicitDeclarationRequired {
        inferred: EffectSet,
    },
    UnknownLocalCallable {
        callee: CallableId,
    },
    DynamicSignatureRequired {
        target: String,
    },
    CapabilityUnavailable {
        unavailable: EffectSet,
    },
    OverdeclaredEffect {
        unused: EffectSet,
    },
}

/// One deterministic witness step explaining an inferred effect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectTraceStep {
    Call {
        caller: CallableId,
        callee: CallableId,
        site: EffectSite,
    },
    ExternalCall {
        caller: CallableId,
        callee: String,
        site: EffectSite,
    },
    DynamicCall {
        caller: CallableId,
        target: String,
        site: EffectSite,
    },
    Perform {
        callable: CallableId,
        effect: EffectId,
        site: EffectSite,
    },
}

/// Shortest deterministic witness path for one inferred effect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectTrace {
    effect: EffectId,
    steps: Vec<EffectTraceStep>,
}

/// One effect analysis diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectDiagnostic {
    code: EffectDiagnosticCode,
    severity: EffectSeverity,
    callable: CallableId,
    message: String,
    kind: EffectDiagnosticKind,
    trace: Option<EffectTrace>,
}

impl EffectDiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingDeclaration => "AWF-EFX-001",
            Self::ForbiddenEffect => "AWF-EFX-002",
            Self::PureCallableEffect => "AWF-EFX-003",
            Self::ExplicitDeclarationRequired => "AWF-EFX-004",
            Self::UnknownLocalCallable => "AWF-EFX-005",
            Self::DynamicSignatureRequired => "AWF-EFX-006",
            Self::CapabilityUnavailable => "AWF-EFX-007",
            Self::OverdeclaredEffect => "AWF-EFX-008",
        }
    }
}

impl EffectTrace {
    pub(crate) fn new(effect: EffectId, steps: Vec<EffectTraceStep>) -> Self {
        Self { effect, steps }
    }

    pub const fn effect(&self) -> &EffectId {
        &self.effect
    }

    pub fn steps(&self) -> &[EffectTraceStep] {
        &self.steps
    }
}

impl EffectDiagnostic {
    pub(crate) fn new(
        code: EffectDiagnosticCode,
        severity: EffectSeverity,
        callable: CallableId,
        message: String,
        kind: EffectDiagnosticKind,
        trace: Option<EffectTrace>,
    ) -> Self {
        Self {
            code,
            severity,
            callable,
            message,
            kind,
            trace,
        }
    }

    pub const fn code(&self) -> EffectDiagnosticCode {
        self.code
    }

    pub const fn severity(&self) -> EffectSeverity {
        self.severity
    }

    pub const fn callable(&self) -> &CallableId {
        &self.callable
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn kind(&self) -> &EffectDiagnosticKind {
        &self.kind
    }

    pub const fn trace(&self) -> Option<&EffectTrace> {
        self.trace.as_ref()
    }
}
