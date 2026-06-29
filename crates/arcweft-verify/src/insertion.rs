//! Typed source insertion targets for verifier repair actions.
//!
//! This module is intentionally Sans I/O. It receives typed source ranges from
//! syntax/HIR/sema and lowers them into verifier-owned source-edit payloads. It
//! does not read files, inspect editor state, or parse rendered diagnostics.

use crate::{ProofObligation, ProofObligationKind, SourceSpan, ToolActionApplicability};
use arcweft_lang_hir::model::HirModule;
use serde::{Deserialize, Serialize};

/// Exact source insertion/replacement location owned by a verifier obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VerifierInsertionTarget {
    span: SourceSpan,
    policy: VerifierInsertionPolicy,
}

/// Policy describing what may be generated at a verifier insertion target.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "policy")]
pub enum VerifierInsertionPolicy {
    /// Insert a new top-level `proof @proof.* { ... }` item.
    TopLevelProofItem,
    /// Replace the opening `{` of a braced `unsafe lifetime` block with
    /// metadata / SAFETY-doc scaffolding plus the original `{`.
    UnsafeAuditMetadata {
        has_reason: bool,
        has_safety_doc: bool,
    },
}

/// Source insertion inventory derived from HIR once per verification run.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct VerifierInsertionInventory {
    proof_item: Option<VerifierInsertionTarget>,
}

impl VerifierInsertionTarget {
    pub const fn new(span: SourceSpan, policy: VerifierInsertionPolicy) -> Self {
        Self { span, policy }
    }

    pub const fn top_level_proof_item(span: SourceSpan) -> Self {
        Self::new(span, VerifierInsertionPolicy::TopLevelProofItem)
    }

    pub const fn unsafe_audit_metadata(
        span: SourceSpan,
        has_reason: bool,
        has_safety_doc: bool,
    ) -> Self {
        Self::new(
            span,
            VerifierInsertionPolicy::UnsafeAuditMetadata {
                has_reason,
                has_safety_doc,
            },
        )
    }

    pub const fn span(self) -> SourceSpan {
        self.span
    }

    pub const fn policy(self) -> VerifierInsertionPolicy {
        self.policy
    }
}

impl VerifierInsertionInventory {
    pub(crate) fn from_module(module: &HirModule) -> Self {
        let proof_item = module
            .safe_top_level_insertion_range()
            .map(|range| SourceSpan {
                start: range.start(),
                end: range.end(),
            })
            .map(VerifierInsertionTarget::top_level_proof_item);
        Self { proof_item }
    }

    pub(crate) const fn proof_target_for_kind(
        self,
        kind: ProofObligationKind,
    ) -> Option<VerifierInsertionTarget> {
        if kind.owns_proof_insertion_span() {
            self.proof_item
        } else {
            None
        }
    }
}

/// Builds a verifier source edit replacement for a proof stub.
pub(crate) fn proof_stub_edit(
    obligation: &ProofObligation,
) -> Option<(SourceSpan, String, ToolActionApplicability)> {
    let target = obligation.insertion_target?;
    matches!(target.policy(), VerifierInsertionPolicy::TopLevelProofItem).then(|| {
        (
            target.span(),
            proof_stub_replacement(obligation),
            ToolActionApplicability::HasPlaceholders,
        )
    })
}

/// Builds a verifier source edit replacement for unsafe audit metadata.
pub(crate) fn unsafe_audit_edit(
    obligation: &ProofObligation,
) -> Option<(SourceSpan, String, ToolActionApplicability)> {
    let target = obligation.insertion_target?;
    let VerifierInsertionPolicy::UnsafeAuditMetadata {
        has_reason,
        has_safety_doc,
    } = target.policy()
    else {
        return None;
    };
    let replacement = unsafe_audit_replacement(has_reason, has_safety_doc)?;
    Some((
        target.span(),
        replacement,
        ToolActionApplicability::HasPlaceholders,
    ))
}

fn proof_stub_replacement(obligation: &ProofObligation) -> String {
    let id = proof_id_for_obligation(&obligation.id);
    let message = sanitize_comment_text(&obligation.message);
    format!("\n\nproof @{id} {{\n    // TODO: prove {message}\n    check _\n}}\n")
}

fn unsafe_audit_replacement(has_reason: bool, has_safety_doc: bool) -> Option<String> {
    match (has_reason, has_safety_doc) {
        (true, true) => None,
        (false, false) => Some(
            " reason = _\n{\n    /// SAFETY: TODO: justify this unsafe lifetime block.".to_owned(),
        ),
        (false, true) => Some(" reason = _\n{".to_owned()),
        (true, false) => {
            Some("{\n    /// SAFETY: TODO: justify this unsafe lifetime block.".to_owned())
        }
    }
}

fn proof_id_for_obligation(obligation_id: &str) -> String {
    let suffix = obligation_id
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_owned();
    if suffix.is_empty() {
        "proof.obligation".to_owned()
    } else {
        format!("proof.{suffix}")
    }
}

fn sanitize_comment_text(message: &str) -> String {
    message
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(160)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProofDischarge, ProofObligation};

    #[test]
    fn proof_stub_uses_empty_insertion_span_and_placeholders() {
        let obligation = ProofObligation {
            id: "obligation.0001".to_owned(),
            kind: ProofObligationKind::LifetimePromotion,
            message: "lifetime promotion requires proof".to_owned(),
            subject: Some("lifetime.promotion".to_owned()),
            source: None,
            insertion_target: Some(VerifierInsertionTarget::top_level_proof_item(SourceSpan {
                start: 42,
                end: 42,
            })),
            discharge: ProofDischarge::Missing,
            smt: None,
        };

        let (span, replacement, applicability) = proof_stub_edit(&obligation).expect("edit");

        assert_eq!(span, SourceSpan { start: 42, end: 42 });
        assert_eq!(applicability, ToolActionApplicability::HasPlaceholders);
        assert!(replacement.contains("proof @proof.obligation_0001"));
        assert!(replacement.contains("check _"));
    }

    #[test]
    fn unsafe_audit_replaces_open_brace_when_metadata_is_missing() {
        let obligation = ProofObligation {
            id: "obligation.0002".to_owned(),
            kind: ProofObligationKind::UnsafeLifetimeAudit,
            message: "unsafe lifetime block requires audit metadata".to_owned(),
            subject: Some("unsafe.cache".to_owned()),
            source: None,
            insertion_target: Some(VerifierInsertionTarget::unsafe_audit_metadata(
                SourceSpan { start: 21, end: 22 },
                false,
                false,
            )),
            discharge: ProofDischarge::Missing,
            smt: None,
        };

        let (span, replacement, applicability) = unsafe_audit_edit(&obligation).expect("edit");

        assert_eq!(span, SourceSpan { start: 21, end: 22 });
        assert_eq!(applicability, ToolActionApplicability::HasPlaceholders);
        assert!(replacement.starts_with(" reason = _\n{"));
        assert!(replacement.contains("/// SAFETY: TODO"));
    }
}
