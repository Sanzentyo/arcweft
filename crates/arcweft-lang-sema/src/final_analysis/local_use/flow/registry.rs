//! Registry availability in the selected ownership graph.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_core::effect::RuntimeRegistryReadKind;
use arcweft_id::{EffectId, LifetimeRegistryKey, LifetimeScopeKind};
use arcweft_lang_hir::identity::ExprId;
use arcweft_lang_syntax::ast::line_plan::DeferOutcome;

use super::RegistryScopeId;
use crate::types::TypeKind;

/// A source activation in the ownership graph. Entering a Line site again
/// starts fresh facts even when the source site is reached through a loop.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::final_analysis::local_use) enum RegistryOwner {
    Flow,
    Line(ExprId),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::final_analysis::local_use) struct RegistrySite {
    line: Option<ExprId>,
}

impl RegistrySite {
    pub(in crate::final_analysis::local_use) const fn line(line: ExprId) -> Self {
        Self { line: Some(line) }
    }

    pub(in crate::final_analysis::local_use) const fn line_owner(self) -> Option<ExprId> {
        self.line
    }

    fn owners(self) -> impl Iterator<Item = RegistryOwner> {
        std::iter::once(RegistryOwner::Flow).chain(self.line.map(RegistryOwner::Line))
    }

    pub(super) fn reaches(self, address: &RegistryAddress) -> bool {
        self.owners().any(|owner| owner == address.owner)
    }

    pub(in crate::final_analysis::local_use) fn address(
        self,
        key: Arc<LifetimeRegistryKey>,
    ) -> Option<RegistryAddress> {
        let owner = match key.scope() {
            LifetimeScopeKind::Flow => RegistryOwner::Flow,
            LifetimeScopeKind::Line => RegistryOwner::Line(self.line?),
            _ => return None,
        };
        Some(RegistryAddress { owner, key })
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::final_analysis::local_use) struct RegistryAddress {
    owner: RegistryOwner,
    key: Arc<LifetimeRegistryKey>,
}

impl RegistryAddress {
    pub(super) fn key(&self) -> &Arc<LifetimeRegistryKey> {
        &self.key
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RegistryFact {
    Unknown,
    Absent,
    Present(Arc<TypeKind>),
    MaybePresent(Arc<TypeKind>),
}

impl RegistryFact {
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (left, right) if left == right => left.clone(),
            (Self::Absent, Self::Present(ty) | Self::MaybePresent(ty))
            | (Self::Present(ty) | Self::MaybePresent(ty), Self::Absent) => {
                Self::MaybePresent(Arc::clone(ty))
            }
            (Self::Present(left), Self::MaybePresent(right))
            | (Self::MaybePresent(left), Self::Present(right))
                if left == right =>
            {
                Self::MaybePresent(Arc::clone(left))
            }
            _ => Self::Unknown,
        }
    }

    pub(super) fn admits(&self, kind: RuntimeRegistryReadKind, expected: &TypeKind) -> bool {
        match (self, kind) {
            (Self::Present(item), _)
            | (Self::MaybePresent(item), RuntimeRegistryReadKind::Optional) => {
                item.as_ref() == expected
            }
            (Self::Absent, RuntimeRegistryReadKind::Optional) => true,
            _ => false,
        }
    }

    pub(super) fn admits_drop(&self, kind: RuntimeRegistryReadKind, expected: &TypeKind) -> bool {
        // Optional removal checks the reached item's type before committing.
        // A possible absence requires no proof of a prior registration.
        matches!(
            (self, kind),
            (Self::Unknown, RuntimeRegistryReadKind::Optional)
        ) || self.admits(kind, expected)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::final_analysis::local_use) enum RegistryDeferOwner {
    Line(ExprId),
    Lexical(Option<RegistryScopeId>),
}

#[derive(Clone, Copy)]
pub(super) enum RegistryUnwind {
    Completed,
    Any,
}

#[derive(Clone, Default, Eq, PartialEq)]
struct RegistryDeferredWrites {
    completed: BTreeSet<RegistryAddress>,
    interrupted: BTreeSet<RegistryAddress>,
}

/// A fresh scope provides a shared absence default. Only deviations from the
/// default occupy rows, so entering a scope never materializes every read key.
#[derive(Clone, Default, Eq, PartialEq)]
pub(super) struct RegistryFacts {
    empty_scopes: BTreeSet<RegistryOwner>,
    entries: BTreeMap<RegistryAddress, RegistryFact>,
    deferred: BTreeMap<RegistryDeferOwner, RegistryDeferredWrites>,
}

impl RegistryFacts {
    fn default_fact(&self, address: &RegistryAddress) -> &RegistryFact {
        if self.empty_scopes.contains(&address.owner) {
            &RegistryFact::Absent
        } else {
            &RegistryFact::Unknown
        }
    }

    pub(super) fn fact(&self, address: &RegistryAddress) -> &RegistryFact {
        self.entries
            .get(address)
            .unwrap_or_else(|| self.default_fact(address))
    }

    fn set(&mut self, address: &RegistryAddress, fact: RegistryFact) {
        if &fact == self.default_fact(address) {
            self.entries.remove(address);
        } else {
            self.entries.insert(address.clone(), fact);
        }
    }

    pub(super) fn enter(&mut self, owner: RegistryOwner) {
        self.leave(owner);
        self.empty_scopes.insert(owner);
    }

    pub(super) fn leave(&mut self, owner: RegistryOwner) {
        self.empty_scopes.remove(&owner);
        self.entries.retain(|address, _| address.owner != owner);
        if let RegistryOwner::Line(line) = owner {
            self.deferred.remove(&RegistryDeferOwner::Line(line));
        }
    }

    pub(super) fn forget(&mut self, site: RegistrySite) {
        for owner in site.owners() {
            self.empty_scopes.remove(&owner);
            self.entries.retain(|address, _| address.owner != owner);
        }
    }

    pub(super) fn register(&mut self, address: &RegistryAddress, ty: &Arc<TypeKind>) {
        self.set(address, RegistryFact::Present(Arc::clone(ty)));
    }

    pub(super) fn remove(&mut self, address: &RegistryAddress) {
        self.set(address, RegistryFact::Absent);
    }

    pub(super) fn invalidate_writes(
        &mut self,
        site: RegistrySite,
        effects: &[EffectId],
        keys: &BTreeMap<RegistryAddress, EffectId>,
    ) {
        for (address, required) in keys {
            if site.reaches(address) && effects.iter().any(|effect| effect.covers(required)) {
                self.set(address, RegistryFact::Unknown);
            }
        }
    }

    pub(super) fn defer_writes(
        &mut self,
        owner: RegistryDeferOwner,
        site: RegistrySite,
        outcome: DeferOutcome,
        effects: &[EffectId],
        keys: &BTreeMap<RegistryAddress, EffectId>,
    ) {
        for (address, required) in keys {
            if site.reaches(address) && effects.iter().any(|effect| effect.covers(required)) {
                let pending = self.deferred.entry(owner).or_default();
                let targets = match outcome {
                    DeferOutcome::Always | DeferOutcome::Completed => &mut pending.completed,
                    DeferOutcome::Cancelled | DeferOutcome::Failed => &mut pending.interrupted,
                };
                targets.insert(address.clone());
            }
        }
    }

    pub(super) fn has_lexical_defers(&self) -> bool {
        self.deferred
            .keys()
            .any(|owner| matches!(owner, RegistryDeferOwner::Lexical(Some(_))))
    }

    pub(super) fn unwind(&mut self, owner: RegistryDeferOwner, exit: RegistryUnwind) {
        let Some(mut writes) = self.deferred.remove(&owner) else {
            return;
        };
        if matches!(exit, RegistryUnwind::Any) {
            writes.completed.append(&mut writes.interrupted);
        }
        for address in &writes.completed {
            self.set(address, RegistryFact::Unknown);
        }
    }

    pub(super) fn join(&mut self, other: &Self) {
        let mut joined = Self {
            empty_scopes: self
                .empty_scopes
                .intersection(&other.empty_scopes)
                .copied()
                .collect(),
            entries: BTreeMap::new(),
            deferred: self.deferred.clone(),
        };
        let keys = self
            .entries
            .keys()
            .chain(other.entries.keys())
            .collect::<BTreeSet<_>>();
        for key in keys {
            joined.set(key, self.fact(key).join(other.fact(key)));
        }
        for (owner, writes) in &other.deferred {
            let pending = joined.deferred.entry(*owner).or_default();
            pending.completed.extend(writes.completed.iter().cloned());
            pending
                .interrupted
                .extend(writes.interrupted.iter().cloned());
        }
        *self = joined;
    }
}

/// Possible mutations by the children of one selected line activation.
/// The fact for a changed key joins all sibling write and removal outcomes.
#[derive(Default)]
pub(super) struct RegistryInterference {
    typed: BTreeMap<RegistryAddress, RegistryFact>,
    opaque: BTreeMap<RegistryOwner, BTreeSet<EffectId>>,
    open: BTreeSet<RegistryOwner>,
}

impl RegistryInterference {
    pub(super) fn write(&mut self, address: &RegistryAddress, ty: &Arc<TypeKind>) {
        self.record(address, RegistryFact::Present(Arc::clone(ty)));
    }

    pub(super) fn remove(&mut self, address: &RegistryAddress) {
        self.record(address, RegistryFact::Absent);
    }

    fn record(&mut self, address: &RegistryAddress, fact: RegistryFact) {
        self.typed
            .entry(address.clone())
            .and_modify(|previous| *previous = previous.join(&fact))
            .or_insert(fact);
    }

    pub(super) fn opaque_writes(&mut self, site: RegistrySite, effects: Option<&[EffectId]>) {
        if let Some(effects) = effects {
            for owner in site.owners() {
                self.opaque
                    .entry(owner)
                    .or_default()
                    .extend(effects.iter().cloned());
            }
        } else {
            self.open.extend(site.owners());
        }
    }

    pub(super) fn consider(
        &self,
        address: &RegistryAddress,
        required: &EffectId,
        fact: RegistryFact,
    ) -> RegistryFact {
        if self.open.contains(&address.owner)
            || self
                .opaque
                .get(&address.owner)
                .is_some_and(|effects| effects.iter().any(|effect| effect.covers(required)))
        {
            return RegistryFact::Unknown;
        }
        match self.typed.get(address) {
            None => fact,
            Some(possible) => fact.join(possible),
        }
    }

    pub(super) fn apply_to_flow(
        &self,
        facts: &mut RegistryFacts,
        keys: &BTreeMap<RegistryAddress, EffectId>,
    ) {
        for (address, required) in keys
            .iter()
            .filter(|(address, _)| address.owner == RegistryOwner::Flow)
        {
            let fact = self.consider(address, required, facts.fact(address).clone());
            facts.set(address, fact);
        }
    }
}
