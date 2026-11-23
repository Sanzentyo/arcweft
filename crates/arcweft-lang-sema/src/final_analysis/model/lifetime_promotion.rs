//! Source-backed operands of a checked lifetime promotion request.

use arcweft_id::{LifetimeScopeKind, UnsafeAuditId};
use arcweft_lang_hir::{
    identity::{ExprId, StmtId},
    symbol::CallableDeclarationKey,
};
use arcweft_source::{Diagnostic, DiagnosticLabel, SourceSpan};

/// A request with checked operands, a reached activation, and the applicable
/// receiver storage checks. Execution requires separate transfer evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedLifetimePromotion {
    pub(in crate::final_analysis) source: SourceSpan,
    pub(in crate::final_analysis) receiver: ExprId,
    pub(in crate::final_analysis) receiver_source: SourceSpan,
    pub(in crate::final_analysis) target: LifetimeScopeKind,
    pub(in crate::final_analysis) target_source: SourceSpan,
    pub(in crate::final_analysis) authorization: CheckedLifetimePromotionAuthorization,
}

/// The authored authorization attached to the checked request. Resolving a
/// proof declaration records its identity without discharging the proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedLifetimePromotionAuthorization {
    WithoutProof,
    Proof {
        declaration: CallableDeclarationKey,
        reference: SourceSpan,
        declaration_source: SourceSpan,
    },
    Audit {
        owner: StmtId,
        id: UnsafeAuditId,
        declaration_source: SourceSpan,
    },
}

impl CheckedLifetimePromotion {
    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }

    pub const fn receiver(&self) -> ExprId {
        self.receiver
    }

    pub const fn target(&self) -> &LifetimeScopeKind {
        &self.target
    }

    pub const fn authorization(&self) -> &CheckedLifetimePromotionAuthorization {
        &self.authorization
    }

    pub fn annotate_diagnostic(&self, diagnostic: Diagnostic) -> Diagnostic {
        let diagnostic = diagnostic
            .with_label(DiagnosticLabel::secondary(
                self.receiver_source.clone(),
                Some("promotion receiver".to_owned()),
            ))
            .with_label(DiagnosticLabel::secondary(
                self.target_source.clone(),
                Some(format!("reached '{}' activation", self.target.as_str())),
            ));
        match &self.authorization {
            CheckedLifetimePromotionAuthorization::WithoutProof => diagnostic,
            CheckedLifetimePromotionAuthorization::Proof {
                declaration,
                reference,
                declaration_source,
            } => diagnostic
                .with_label(DiagnosticLabel::secondary(
                    reference.clone(),
                    Some("authored proof reference".to_owned()),
                ))
                .with_label(DiagnosticLabel::secondary(
                    declaration_source.clone(),
                    Some(format!(
                        "resolved proof declaration '{}'",
                        declaration.qualified_name()
                    )),
                )),
            CheckedLifetimePromotionAuthorization::Audit {
                id,
                declaration_source,
                ..
            } => diagnostic.with_label(DiagnosticLabel::secondary(
                declaration_source.clone(),
                Some(format!("enclosing '{}' audit", id.as_public_id().as_str())),
            )),
        }
    }
}
