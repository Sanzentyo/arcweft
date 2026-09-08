use arcweft_lang_hir::{dialogue_application::HirDialogueContentId, identity::ExprId};

use super::super::match_edges::{CheckedChildEdgeError, NestedPathEvidence};
use super::super::{
    CheckedCharacterDialoguePatch, CheckedCharacterDialogueTarget,
    CheckedDialogueEffectSiteOrdinal, CheckedDialogueEffectTrigger,
};
use super::{PreparedEvaluatedEffect, PreparedExpressionShell, TypeKind};
use crate::callable::ContentCallableIdentity;
use crate::checked_text_proxy::PreparedCheckedTextProxyApplication;

/// Closed producer disposition for one attached-content application.
///
/// `ContentResult` carries an exact checked `DialogueContent` result. The
/// remaining variants retain the typed producer payload until the C2 seal;
/// no runtime `Unit` placeholder is used for these non-value expressions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedContentEmission {
    ContentResult,
    ObjectSpan(PreparedCheckedTextProxyApplication),
    /// A language-owned content callable. The operation is already held by
    /// the non-value shell; the selected definition, schema, and
    /// mapped/defaulted operands remain owned by the final call fact.
    LanguageCallable(ContentCallableIdentity),
}

/// Private prepared fact for a generic attached-content expression.
///
/// The body is identified by its HIR content owner and is checked exactly
/// once into the facts-owned affine content catalog. The call graph owns the
/// invocation application; this carrier only joins that application with the
/// content emission disposition at the late seal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedContentApplication {
    owner: ExprId,
    shell: PreparedExpressionShell,
    content: Option<HirDialogueContentId>,
    emission: PreparedContentEmission,
}

impl PreparedContentApplication {
    /// Constructs the non-value shell reserved for a closed content emitter.
    /// The Object producer is the first such emitter; its checked application
    /// remains the sole owner of the concrete ObjectSpan payload.
    pub(crate) fn try_new_content_emission(
        owner: ExprId,
        effects: crate::effects::EffectSet,
        content: Option<HirDialogueContentId>,
        callable: ContentCallableIdentity,
        emission: PreparedContentEmission,
    ) -> Option<Self> {
        let valid = match (&emission, callable) {
            (
                PreparedContentEmission::ObjectSpan(_),
                ContentCallableIdentity::TextProxyObject { .. },
            ) => true,
            (
                PreparedContentEmission::LanguageCallable(identity),
                ContentCallableIdentity::Language { .. },
            ) if *identity == callable => true,
            (
                PreparedContentEmission::LanguageCallable(_),
                ContentCallableIdentity::Language { .. },
            ) => false,
            (
                PreparedContentEmission::LanguageCallable(_),
                ContentCallableIdentity::TextProxyObject { .. },
            ) => false,
            (PreparedContentEmission::ContentResult, _) => false,
            (PreparedContentEmission::ObjectSpan(_), _) => false,
        };
        if !valid {
            return None;
        }
        Self::try_new(
            owner,
            PreparedExpressionShell::content_emission(callable, effects),
            content,
            emission,
            None,
        )
    }

    pub(crate) fn try_new(
        owner: ExprId,
        shell: PreparedExpressionShell,
        content: Option<HirDialogueContentId>,
        emission: PreparedContentEmission,
        expected_dialogue_content: Option<&TypeKind>,
    ) -> Option<Self> {
        if content.is_some_and(|content| content.owner() != owner) {
            return None;
        }
        let valid_shell = match (shell.result(), &emission) {
            (
                super::PreparedExpressionResult::Value(value),
                PreparedContentEmission::ContentResult,
            ) => expected_dialogue_content == Some(value.ty()),
            (
                super::PreparedExpressionResult::NonValue(
                    super::PreparedNonValueExpressionResult::ContentEmission(callable),
                ),
                PreparedContentEmission::ObjectSpan(_),
            ) => matches!(callable, ContentCallableIdentity::TextProxyObject { .. }),
            (
                super::PreparedExpressionResult::NonValue(
                    super::PreparedNonValueExpressionResult::ContentEmission(callable),
                ),
                PreparedContentEmission::LanguageCallable(identity),
            ) => *callable == *identity,
            (
                super::PreparedExpressionResult::NonValue(_),
                PreparedContentEmission::ContentResult,
            ) => false,
            (super::PreparedExpressionResult::Value(_), PreparedContentEmission::ObjectSpan(_))
            | (
                super::PreparedExpressionResult::Value(_),
                PreparedContentEmission::LanguageCallable(_),
            ) => false,
        };
        if !valid_shell {
            return None;
        }
        Some(Self {
            owner,
            shell,
            content,
            emission,
        })
    }

    pub(crate) const fn shell(&self) -> &PreparedExpressionShell {
        &self.shell
    }

    pub(crate) const fn content(&self) -> Option<HirDialogueContentId> {
        self.content
    }

    pub(crate) const fn emission(&self) -> &PreparedContentEmission {
        &self.emission
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        ExprId,
        PreparedExpressionShell,
        Option<HirDialogueContentId>,
        PreparedContentEmission,
    ) {
        (self.owner, self.shell, self.content, self.emission)
    }
}

/// One source-ordered inline dialogue effect awaiting the final callable
/// application seal.  The callable-owned preparation is kept private and is
/// consumed into the public site only after its checked application is
/// available.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedDialogueEffectSite {
    id: CheckedDialogueEffectSiteOrdinal,
    trigger: CheckedDialogueEffectTrigger,
    effect: PreparedEvaluatedEffect,
}

impl PreparedDialogueEffectSite {
    pub(crate) const fn new(
        id: CheckedDialogueEffectSiteOrdinal,
        trigger: CheckedDialogueEffectTrigger,
        effect: PreparedEvaluatedEffect,
    ) -> Self {
        Self {
            id,
            trigger,
            effect,
        }
    }

    pub(crate) const fn root(&self) -> ExprId {
        self.effect.root()
    }

    pub(crate) const fn id(&self) -> CheckedDialogueEffectSiteOrdinal {
        self.id
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        CheckedDialogueEffectSiteOrdinal,
        CheckedDialogueEffectTrigger,
        PreparedEvaluatedEffect,
    ) {
        (self.id, self.trigger, self.effect)
    }
}

/// Private content-owner-local effect-plan carrier. Marker actions remain part
/// of the checked rich text content; only effect sites retain callable
/// preparation until the project-wide call seal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedDialogueEffectPlan {
    effect_sites: Box<[PreparedDialogueEffectSite]>,
}

impl PreparedDialogueEffectPlan {
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
    content: HirDialogueContentId,
    line_result: TypeKind,
    nested_path_evidence: Option<Result<NestedPathEvidence, CheckedChildEdgeError>>,
}

impl PreparedDialogueApplication {
    pub(crate) fn try_new(
        shell: PreparedExpressionShell,
        target: CheckedCharacterDialogueTarget,
        application_patch: Option<CheckedCharacterDialoguePatch>,
        content: HirDialogueContentId,
        line_result: TypeKind,
        nested_path_evidence: Option<Result<NestedPathEvidence, CheckedChildEdgeError>>,
    ) -> Option<Self> {
        let expected_shell_type = TypeKind::DialogueLine(Box::new(line_result.clone()));
        if shell.value_type() != Some(&expected_shell_type) {
            return None;
        }
        Some(Self {
            shell,
            target,
            application_patch,
            content,
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

    pub(crate) const fn content(&self) -> HirDialogueContentId {
        self.content
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
        HirDialogueContentId,
        TypeKind,
        Option<Result<NestedPathEvidence, CheckedChildEdgeError>>,
    ) {
        (
            self.shell,
            self.target,
            self.application_patch,
            self.content,
            self.line_result,
            self.nested_path_evidence,
        )
    }
}
