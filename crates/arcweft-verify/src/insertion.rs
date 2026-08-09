//! Revision-bound verifier insertion targets.
//!
//! The inventory is derived once from the accepted executable project. It
//! keeps the exact source revision for every module and never derives an edit
//! location from a display path or reparsed source string.

use std::collections::BTreeMap;

use arcweft_lang_hir::{identity::HirModuleId, project::HirExecutableProjectView};
use serde::{Deserialize, Serialize};

use crate::{ProofObligation, ProofObligationKind, SourceSpan, ToolActionApplicability};

/// Exact source insertion/replacement location owned by a verifier obligation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VerifierInsertionTarget {
    span: SourceSpan,
    policy: VerifierInsertionPolicy,
}

/// Policy describing what may be generated at a verifier insertion target.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "policy")]
pub enum VerifierInsertionPolicy {
    /// Insert a new top-level `proof name { ... }` item.
    TopLevelProofItem,
}

/// Per-module insertion inventory derived from the exact accepted HIR leases.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct VerifierInsertionInventory {
    proof_items: BTreeMap<HirModuleId, VerifierInsertionTarget>,
}

impl VerifierInsertionTarget {
    pub const fn new(span: SourceSpan, policy: VerifierInsertionPolicy) -> Self {
        Self { span, policy }
    }

    pub const fn top_level_proof_item(span: SourceSpan) -> Self {
        Self::new(span, VerifierInsertionPolicy::TopLevelProofItem)
    }

    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }

    pub const fn policy(&self) -> VerifierInsertionPolicy {
        self.policy
    }
}

impl VerifierInsertionInventory {
    pub(crate) fn from_project(project: HirExecutableProjectView<'_>) -> Self {
        let proof_items = project
            .modules()
            .map(|(_, module)| {
                let target = VerifierInsertionTarget::top_level_proof_item(SourceSpan::from_exact(
                    &module.provenance().document().end_span(),
                ));
                (module.module_id(), target)
            })
            .collect();
        Self { proof_items }
    }

    pub(crate) fn proof_target_for_kind(
        &self,
        module: HirModuleId,
        kind: ProofObligationKind,
    ) -> Option<VerifierInsertionTarget> {
        kind.owns_proof_insertion_span()
            .then(|| self.proof_items.get(&module).cloned())
            .flatten()
    }
}

/// Builds a verifier source edit replacement for a proof stub.
pub(crate) fn proof_stub_edit(
    obligation: &ProofObligation,
) -> Option<(SourceSpan, String, ToolActionApplicability)> {
    let target = obligation.insertion_target.as_ref()?;
    matches!(target.policy(), VerifierInsertionPolicy::TopLevelProofItem).then(|| {
        (
            target.span().clone(),
            proof_stub_replacement(obligation),
            ToolActionApplicability::HasPlaceholders,
        )
    })
}

fn proof_stub_replacement(obligation: &ProofObligation) -> String {
    let name = proof_name_for_obligation(&obligation.id);
    let message = sanitize_comment_text(&obligation.message);
    format!("\n\nproof {name}() {{\n    // TODO: prove {message}\n}}\n")
}

fn proof_name_for_obligation(obligation_id: &str) -> String {
    let suffix = obligation_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_owned();
    if suffix.is_empty() {
        "obligation".to_owned()
    } else {
        suffix
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
