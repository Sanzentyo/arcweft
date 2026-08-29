use arcweft_lang_hir::identity::ExprId;

use crate::checked_rich_text::PreparedCheckedRichTextReport;

use super::super::match_edges::{CheckedChildEdgeError, NestedPathEvidence};
use super::super::{
    CheckedCharacterDialoguePatch, CheckedCharacterDialogueTarget,
    CheckedDialogueEffectSiteOrdinal, CheckedDialogueEffectTrigger,
};
use super::{PreparedEvaluatedEffect, PreparedExpressionShell, TypeKind};

/// One source-ordered inline dialogue effect awaiting the final callable
/// application seal.  The callable-owned preparation is kept private and is
/// consumed into the public site only after its checked application is
/// available.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedDialogueEffectSite {
    id: CheckedDialogueEffectSiteOrdinal,
    trigger: CheckedDialogueEffectTrigger,
    /// The authored expression which owns this source-ordered line-plan
    /// site.  This remains private because it is structural evidence used by
    /// the final call seal (not a public runtime identity).
    expression: ExprId,
    effect: PreparedEvaluatedEffect,
}

impl PreparedDialogueEffectSite {
    pub(crate) const fn new(
        id: CheckedDialogueEffectSiteOrdinal,
        trigger: CheckedDialogueEffectTrigger,
        expression: ExprId,
        effect: PreparedEvaluatedEffect,
    ) -> Self {
        Self {
            id,
            trigger,
            expression,
            effect,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        CheckedDialogueEffectSiteOrdinal,
        CheckedDialogueEffectTrigger,
        ExprId,
        PreparedEvaluatedEffect,
    ) {
        (self.id, self.trigger, self.expression, self.effect)
    }
}

/// Private line-plan carrier. Marker actions remain part of the checked rich
/// text content; only effect sites retain callable preparation until the
/// project-wide call seal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedDialogueLinePlan {
    effect_sites: Box<[PreparedDialogueEffectSite]>,
}

impl PreparedDialogueLinePlan {
    pub(crate) fn new(effect_sites: impl Into<Box<[PreparedDialogueEffectSite]>>) -> Self {
        Self {
            effect_sites: effect_sites.into(),
        }
    }

    pub(crate) fn into_parts(self) -> Box<[PreparedDialogueEffectSite]> {
        self.effect_sites
    }
}

/// Private expression carrier for one checked dialogue content application.
/// Its shell preserves the type-selection/effect facts produced while the
/// application call is prepared.  The line-plan effect sites are sealed only
/// after the project-wide call graph has produced checked applications.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedDialogueApplication {
    shell: PreparedExpressionShell,
    target: CheckedCharacterDialogueTarget,
    application_patch: Option<CheckedCharacterDialoguePatch>,
    rich_text: Box<PreparedCheckedRichTextReport>,
    line_plan: PreparedDialogueLinePlan,
    line_result: TypeKind,
    nested_path_evidence: Option<Result<NestedPathEvidence, CheckedChildEdgeError>>,
}

impl PreparedDialogueApplication {
    pub(crate) fn try_new(
        shell: PreparedExpressionShell,
        target: CheckedCharacterDialogueTarget,
        application_patch: Option<CheckedCharacterDialoguePatch>,
        rich_text: Box<PreparedCheckedRichTextReport>,
        line_plan: PreparedDialogueLinePlan,
        line_result: TypeKind,
        nested_path_evidence: Option<Result<NestedPathEvidence, CheckedChildEdgeError>>,
    ) -> Option<Self> {
        if shell.ty() != &TypeKind::DialogueLine(Box::new(line_result.clone())) {
            return None;
        }
        Some(Self {
            shell,
            target,
            application_patch,
            rich_text,
            line_plan,
            line_result,
            nested_path_evidence,
        })
    }

    pub(crate) const fn shell(&self) -> &PreparedExpressionShell {
        &self.shell
    }

    pub(crate) const fn target(&self) -> &CheckedCharacterDialogueTarget {
        &self.target
    }

    pub(crate) const fn application_patch(&self) -> Option<&CheckedCharacterDialoguePatch> {
        self.application_patch.as_ref()
    }

    pub(crate) const fn line_result(&self) -> &TypeKind {
        &self.line_result
    }

    pub(crate) const fn nested_path_evidence(
        &self,
    ) -> Option<&Result<NestedPathEvidence, CheckedChildEdgeError>> {
        self.nested_path_evidence.as_ref()
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        PreparedExpressionShell,
        CheckedCharacterDialogueTarget,
        Option<CheckedCharacterDialoguePatch>,
        Box<PreparedCheckedRichTextReport>,
        PreparedDialogueLinePlan,
        TypeKind,
        Option<Result<NestedPathEvidence, CheckedChildEdgeError>>,
    ) {
        (
            self.shell,
            self.target,
            self.application_patch,
            self.rich_text,
            self.line_plan,
            self.line_result,
            self.nested_path_evidence,
        )
    }
}
