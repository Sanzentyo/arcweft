use crate::checked_rich_text::CheckedDuration;

use super::CheckedEvaluatedEffect;

/// Checked dialogue line-plan effects. Marker actions are retained in the
/// source-ordered checked rich-text tokens, so this record has no detached
/// mark or statement side table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedDialogueLinePlan {
    effect_sites: Box<[CheckedDialogueEffectSite]>,
}

impl CheckedDialogueLinePlan {
    pub(crate) fn new(effect_sites: impl Into<Box<[CheckedDialogueEffectSite]>>) -> Self {
        Self {
            effect_sites: effect_sites.into(),
        }
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
