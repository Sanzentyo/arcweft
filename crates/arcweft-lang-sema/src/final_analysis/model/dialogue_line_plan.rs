use arcweft_lang_hir::identity::{ExprId, LocalId};

use crate::checked_rich_text::CheckedDuration;
use crate::effects::EffectSet;
use crate::semantic_coordinate::StableCheckedBindingCoordinate;

use super::{CheckedEvaluatedEffect, TypeKind};

/// Checked effect sites for one content owner. Marker actions are retained in
/// the source-ordered checked rich-text tokens, so this record has no
/// detached mark or statement side table.
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
    effects: EffectSet,
    effect: Box<CheckedEvaluatedEffect>,
    captures: Box<[CheckedDialogueEffectCapture]>,
}

/// Exact source-ordered free-local capture for one reveal-time dialogue
/// effect callback.
///
/// `local` is generation-local lowering evidence. Stable identity and
/// transcripts use `origin` plus the exact semantic type instead.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedDialogueEffectCapture {
    local: LocalId,
    origin: StableCheckedBindingCoordinate,
    ty: TypeKind,
}

impl CheckedDialogueEffectCapture {
    pub(crate) const fn new(
        local: LocalId,
        origin: StableCheckedBindingCoordinate,
        ty: TypeKind,
    ) -> Self {
        Self { local, origin, ty }
    }

    pub const fn local(&self) -> LocalId {
        self.local
    }

    pub const fn origin(&self) -> &StableCheckedBindingCoordinate {
        &self.origin
    }

    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }
}

impl CheckedDialogueEffectSite {
    pub(crate) const fn new(
        id: CheckedDialogueEffectSiteOrdinal,
        trigger: CheckedDialogueEffectTrigger,
        effects: EffectSet,
        effect: Box<CheckedEvaluatedEffect>,
        captures: Box<[CheckedDialogueEffectCapture]>,
    ) -> Self {
        Self {
            id,
            trigger,
            effects,
            effect,
            captures,
        }
    }

    /// Expression owning this line-plan effect site.  The root is retained by
    /// the checked plan so compiler reachability never has to recover it from
    /// a raw statement or content HIR walk.
    pub const fn root(&self) -> ExprId {
        self.effect.site_root()
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

    pub const fn effect(&self) -> &CheckedEvaluatedEffect {
        &self.effect
    }

    pub const fn captures(&self) -> &[CheckedDialogueEffectCapture] {
        &self.captures
    }
}
