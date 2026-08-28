use arcweft_id::PublicId;
use arcweft_lang_hir::identity::StmtId;

use crate::checked_rich_text::CheckedDuration;

use super::CheckedEvaluatedEffect;

/// Source-ordered checked mark coordinate owned by one dialogue content
/// application. The ordinal is sealed against that application's checked
/// `RichText` mark catalog and is never reconstructed from a runtime label.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedDialogueMarkOrdinal(u32);

impl CheckedDialogueMarkOrdinal {
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One final-HIR line-plan statement bound to an exact checked content mark.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedDialogueMarkHandler {
    statement: StmtId,
    mark: CheckedDialogueMarkOrdinal,
}

impl CheckedDialogueMarkHandler {
    pub(crate) const fn new(statement: StmtId, mark: CheckedDialogueMarkOrdinal) -> Self {
        Self { statement, mark }
    }

    pub const fn statement(&self) -> StmtId {
        self.statement
    }

    pub const fn mark(&self) -> CheckedDialogueMarkOrdinal {
        self.mark
    }
}

/// Checked content-local line-plan coordinate catalog. Public mark identities
/// remain useful to semantic tooling; executable consumers use only the exact
/// statement-to-ordinal rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedDialogueLinePlan {
    marks: Box<[PublicId]>,
    mark_handlers: Box<[CheckedDialogueMarkHandler]>,
    effect_sites: Box<[CheckedDialogueEffectSite]>,
}

impl CheckedDialogueLinePlan {
    pub(crate) fn new(
        marks: impl Into<Box<[PublicId]>>,
        mark_handlers: impl Into<Box<[CheckedDialogueMarkHandler]>>,
        effect_sites: impl Into<Box<[CheckedDialogueEffectSite]>>,
    ) -> Self {
        Self {
            marks: marks.into(),
            mark_handlers: mark_handlers.into(),
            effect_sites: effect_sites.into(),
        }
    }

    pub const fn marks(&self) -> &[PublicId] {
        &self.marks
    }

    pub const fn mark_handlers(&self) -> &[CheckedDialogueMarkHandler] {
        &self.mark_handlers
    }

    pub const fn effect_sites(&self) -> &[CheckedDialogueEffectSite] {
        &self.effect_sites
    }
}

/// Source-ordered checked identity of one inline dialogue effect boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedDialogueEffectSiteOrdinal(u32);

impl CheckedDialogueEffectSiteOrdinal {
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedDialogueEffectTrigger {
    Content,
    Delay(CheckedDuration),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedDialogueEffectSite {
    id: CheckedDialogueEffectSiteOrdinal,
    trigger: CheckedDialogueEffectTrigger,
    effect: Box<CheckedEvaluatedEffect>,
}

impl CheckedDialogueEffectSite {
    pub(crate) const fn new(
        id: CheckedDialogueEffectSiteOrdinal,
        trigger: CheckedDialogueEffectTrigger,
        effect: Box<CheckedEvaluatedEffect>,
    ) -> Self {
        Self {
            id,
            trigger,
            effect,
        }
    }

    pub const fn id(&self) -> CheckedDialogueEffectSiteOrdinal {
        self.id
    }

    pub const fn trigger(&self) -> &CheckedDialogueEffectTrigger {
        &self.trigger
    }

    pub const fn effect(&self) -> &CheckedEvaluatedEffect {
        &self.effect
    }
}
