//! Registry availability in the selected ownership graph.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_core::effect::RuntimeRegistryReadKind;
use arcweft_id::{EffectId, LifetimeRegistryKey, LifetimeScopeKind};
use arcweft_lang_hir::identity::ExprId;

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
    DeferredWrite,
}

impl RegistryFact {
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::DeferredWrite, _) | (_, Self::DeferredWrite) => Self::DeferredWrite,
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
}

/// A fresh scope provides a shared absence default. Only deviations from the
/// default occupy rows, so entering a scope never materializes every read key.
#[derive(Clone, Default, Eq, PartialEq)]
pub(super) struct RegistryFacts {
    empty_scopes: BTreeSet<RegistryOwner>,
    entries: BTreeMap<RegistryAddress, RegistryFact>,
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
    }

    pub(super) fn forget(&mut self, site: RegistrySite) {
        for owner in site.owners() {
            self.empty_scopes.remove(&owner);
            self.entries.retain(|address, fact| {
                address.owner != owner || *fact == RegistryFact::DeferredWrite
            });
        }
    }

    pub(super) fn register(&mut self, address: &RegistryAddress, ty: &Arc<TypeKind>) {
        if *self.fact(address) != RegistryFact::DeferredWrite {
            self.set(address, RegistryFact::Present(Arc::clone(ty)));
        }
    }

    pub(super) fn invalidate_writes(
        &mut self,
        site: RegistrySite,
        effects: &[EffectId],
        deferred: bool,
        keys: &BTreeMap<RegistryAddress, EffectId>,
    ) {
        for (address, required) in keys {
            if !site.owners().any(|owner| owner == address.owner)
                || *self.fact(address) == RegistryFact::DeferredWrite
                || !effects.iter().any(|effect| effect.covers(required))
            {
                continue;
            }
            let fact = if deferred {
                RegistryFact::DeferredWrite
            } else {
                RegistryFact::Unknown
            };
            self.set(address, fact);
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
        };
        let keys = self
            .entries
            .keys()
            .chain(other.entries.keys())
            .collect::<BTreeSet<_>>();
        for key in keys {
            joined.set(key, self.fact(key).join(other.fact(key)));
        }
        *self = joined;
    }
}

/// Possible mutations by the children of one selected line activation.
/// A typed replacement can change absence to possible presence. It cannot
/// establish definite presence or remove an already registered value.
#[derive(Default)]
pub(super) struct RegistryInterference {
    typed: BTreeMap<RegistryAddress, Option<Arc<TypeKind>>>,
    opaque: BTreeMap<RegistryOwner, BTreeSet<EffectId>>,
    open: BTreeSet<RegistryOwner>,
}

impl RegistryInterference {
    pub(super) fn write(&mut self, address: &RegistryAddress, ty: &Arc<TypeKind>) {
        self.typed
            .entry(address.clone())
            .and_modify(|previous| {
                if previous.as_ref() != Some(ty) {
                    *previous = None;
                }
            })
            .or_insert_with(|| Some(Arc::clone(ty)));
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
        if fact == RegistryFact::DeferredWrite {
            return fact;
        }
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
            Some(Some(ty)) => fact.join(&RegistryFact::Present(Arc::clone(ty))),
            Some(None) => RegistryFact::Unknown,
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
