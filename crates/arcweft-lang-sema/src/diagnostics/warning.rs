use super::effect_trace::with_effect_trace_notes;
use crate::{effect_diagnostics::EffectDiagnostic, types::TypeKind};
use arcweft_source::{Diagnostic, DiagnosticSeverity};
use thiserror::Error;

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
    /// An unsuffixed numeric literal inside an inferred closure body fell back
    /// to a stable default primitive type.
    NumericFallbackInInferredClosure {
        literal_kind: String,
        fallback: TypeKind,
    },
    /// A real method was selected, but data-last callable fallback candidates
    /// with the same spelling were also viable. The selected method remains
    /// authoritative, but the API shape is worth surfacing to authors.
    ShadowedDataLastMethodFallback {
        method: String,
        receiver: TypeKind,
        selected: String,
        fallbacks: Vec<String>,
    },
    /// A structured warning from transitive effect analysis.
    Effect { diagnostic: EffectDiagnostic },
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

    pub(crate) fn numeric_fallback_in_inferred_closure(
        literal_kind: impl Into<String>,
        fallback: TypeKind,
    ) -> Self {
        let literal_kind = literal_kind.into();
        Self {
            message: format!(
                "unsuffixed {literal_kind} literal inside inferred closure body defaults to {fallback:?}; add a suffix or closure return type to make the contract explicit"
            ),
            kind: TypeCheckWarningKind::NumericFallbackInInferredClosure {
                literal_kind,
                fallback,
            },
        }
    }

    pub(crate) fn shadowed_data_last_method_fallback(
        method: impl Into<String>,
        receiver: TypeKind,
        selected: impl Into<String>,
        fallbacks: impl IntoIterator<Item = String>,
    ) -> Self {
        let method = method.into();
        let selected = selected.into();
        let fallbacks = fallbacks.into_iter().collect::<Vec<_>>();
        Self {
            message: format!(
                "method `{method}` on {} selects {selected}, shadowing data-last fallback candidate(s): {}",
                receiver.source_label(),
                fallbacks.join(", ")
            ),
            kind: TypeCheckWarningKind::ShadowedDataLastMethodFallback {
                method,
                receiver,
                selected,
                fallbacks,
            },
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

    /// Stable compiler-wide diagnostic code.
    pub fn stable_code(&self) -> String {
        typecheck_warning_code(&self.kind)
    }

    /// Builds the shared diagnostic representation for compiler, CLI, LSP, and Agent surfaces.
    pub fn diagnostic(&self) -> Diagnostic {
        let diagnostic = Diagnostic::new(DiagnosticSeverity::Warning, self.message.clone())
            .with_code(self.stable_code());
        match &self.kind {
            TypeCheckWarningKind::Effect { diagnostic: effect } => {
                with_effect_trace_notes(diagnostic, effect)
            }
            TypeCheckWarningKind::PublicAbiAnonymousSum { .. }
            | TypeCheckWarningKind::NumericFallbackInInferredClosure { .. }
            | TypeCheckWarningKind::ShadowedDataLastMethodFallback { .. } => diagnostic,
        }
    }
}

fn typecheck_warning_code(kind: &TypeCheckWarningKind) -> String {
    match kind {
        TypeCheckWarningKind::PublicAbiAnonymousSum { .. } => {
            "sema.public_abi.anonymous_sum".to_owned()
        }
        TypeCheckWarningKind::NumericFallbackInInferredClosure { .. } => {
            "sema.numeric.fallback_in_inferred_closure".to_owned()
        }
        TypeCheckWarningKind::ShadowedDataLastMethodFallback { .. } => {
            "sema.typecheck.shadowed_data_last_method_fallback".to_owned()
        }
        TypeCheckWarningKind::Effect { diagnostic } => diagnostic.code().as_str().to_owned(),
    }
}
