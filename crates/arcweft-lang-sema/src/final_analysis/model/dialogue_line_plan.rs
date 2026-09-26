use arcweft_lang_hir::identity::ExprId;

use super::{CheckedEvaluatedEffect, CheckedExecutableCapture};
use crate::callable::{CheckedCallApplicationDigest, CheckedCallApplicationSite};
use crate::checked_rich_text::CheckedDuration;
use crate::effects::EffectSet;
use crate::types::TypeKind;

/// Checked point-action sites for one content owner. Marker actions remain in
/// the source-ordered checked rich-text tokens, so this record has no detached
/// mark or statement side table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedDialogueEffectPlan {
    effect_sites: Box<[CheckedDialogueEffectSite]>,
}

impl CheckedDialogueEffectPlan {
    pub(crate) fn new(effect_sites: impl Into<Box<[CheckedDialogueEffectSite]>>) -> Self {
        Self {
            effect_sites: effect_sites.into(),
        }
    }

    pub const fn effect_sites(&self) -> &[CheckedDialogueEffectSite] {
        &self.effect_sites
    }
}

/// Source-ordered checked identity of one inline dialogue operation.
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

/// Closed operation family retained by a dialogue point-action site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedDialogueEffectOperation {
    EvaluatedEffect(Box<CheckedEvaluatedEffect>),
    Call {
        application: CheckedCallApplicationSite,
        application_digest: CheckedCallApplicationDigest,
        result: TypeKind,
    },
}

impl CheckedDialogueEffectOperation {
    pub const fn application(&self) -> &CheckedCallApplicationSite {
        match self {
            Self::EvaluatedEffect(effect) => effect.application(),
            Self::Call { application, .. } => application,
        }
    }

    pub const fn application_digest(&self) -> CheckedCallApplicationDigest {
        match self {
            Self::EvaluatedEffect(effect) => effect.application_digest(),
            Self::Call {
                application_digest, ..
            } => *application_digest,
        }
    }

    pub const fn result(&self) -> &TypeKind {
        match self {
            Self::EvaluatedEffect(effect) => effect.result(),
            Self::Call { result, .. } => result,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedDialogueEffectSite {
    id: CheckedDialogueEffectSiteOrdinal,
    trigger: CheckedDialogueEffectTrigger,
    root: ExprId,
    effects: EffectSet,
    operation: CheckedDialogueEffectOperation,
    captures: Box<[CheckedExecutableCapture]>,
}

impl CheckedDialogueEffectSite {
    pub(crate) const fn new(
        id: CheckedDialogueEffectSiteOrdinal,
        trigger: CheckedDialogueEffectTrigger,
        root: ExprId,
        effects: EffectSet,
        operation: CheckedDialogueEffectOperation,
        captures: Box<[CheckedExecutableCapture]>,
    ) -> Self {
        Self {
            id,
            trigger,
            root,
            effects,
            operation,
            captures,
        }
    }

    pub const fn root(&self) -> ExprId {
        self.root
    }

    pub const fn id(&self) -> CheckedDialogueEffectSiteOrdinal {
        self.id
    }

    pub const fn trigger(&self) -> &CheckedDialogueEffectTrigger {
        &self.trigger
    }

    /// Exact closed checked effect row executed by this reveal callback.
    pub const fn effects(&self) -> &EffectSet {
        &self.effects
    }

    pub const fn operation(&self) -> &CheckedDialogueEffectOperation {
        &self.operation
    }

    pub const fn captures(&self) -> &[CheckedExecutableCapture] {
        &self.captures
    }
}
