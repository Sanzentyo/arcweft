//! Registry availability in the selected ownership graph.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_core::effect::RuntimeRegistryReadKind;
use arcweft_id::{EffectId, LifetimeRegistryKey, LifetimeScopeKind};

use crate::types::TypeKind;

#[derive(Clone, Debug, Eq, PartialEq)]
enum RegistryFact {
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
}

/// A fresh scope provides a shared absence default. Only deviations from the
/// default occupy rows, so entering a scope never materializes every read key.
#[derive(Clone, Default, Eq, PartialEq)]
pub(super) struct RegistryFacts {
    empty_scopes: BTreeSet<LifetimeScopeKind>,
    entries: BTreeMap<Arc<LifetimeRegistryKey>, RegistryFact>,
}

impl RegistryFacts {
    fn default_fact(&self, key: &LifetimeRegistryKey) -> &RegistryFact {
        if self.empty_scopes.contains(key.scope()) {
            &RegistryFact::Absent
        } else {
            &RegistryFact::Unknown
        }
    }

    fn fact(&self, key: &LifetimeRegistryKey) -> &RegistryFact {
        self.entries
            .get(key)
            .unwrap_or_else(|| self.default_fact(key))
    }

    fn set(&mut self, key: &Arc<LifetimeRegistryKey>, fact: RegistryFact) {
        if &fact == self.default_fact(key) {
            self.entries.remove(key);
        } else {
            self.entries.insert(Arc::clone(key), fact);
        }
    }

    pub(super) fn admits(
        &self,
        key: &LifetimeRegistryKey,
        kind: RuntimeRegistryReadKind,
        expected: &TypeKind,
    ) -> bool {
        match (self.fact(key), kind) {
            (RegistryFact::Present(item), _)
            | (RegistryFact::MaybePresent(item), RuntimeRegistryReadKind::Optional) => {
                item.as_ref() == expected
            }
            (RegistryFact::Absent, RuntimeRegistryReadKind::Optional) => true,
            _ => false,
        }
    }

    pub(super) fn enter(&mut self, scope: &LifetimeScopeKind) {
        self.empty_scopes.insert(scope.clone());
        self.entries
            .retain(|key, fact| key.scope() != scope || *fact == RegistryFact::DeferredWrite);
    }

    pub(super) fn forget(&mut self) {
        self.empty_scopes.clear();
        self.entries
            .retain(|_, fact| *fact == RegistryFact::DeferredWrite);
    }

    pub(super) fn register(&mut self, key: &Arc<LifetimeRegistryKey>, ty: &Arc<TypeKind>) {
        if *self.fact(key) != RegistryFact::DeferredWrite {
            self.set(key, RegistryFact::Present(Arc::clone(ty)));
        }
    }

    pub(super) fn invalidate_writes(
        &mut self,
        effects: &[EffectId],
        deferred: bool,
        keys: &BTreeMap<Arc<LifetimeRegistryKey>, EffectId>,
    ) {
        for (key, required) in keys {
            if *self.fact(key) == RegistryFact::DeferredWrite
                || !effects.iter().any(|effect| effect.covers(required))
            {
                continue;
            }
            let fact = if deferred {
                RegistryFact::DeferredWrite
            } else {
                RegistryFact::Unknown
            };
            self.set(key, fact);
        }
    }

    pub(super) fn join(&mut self, other: &Self) {
        let mut joined = Self {
            empty_scopes: self
                .empty_scopes
                .intersection(&other.empty_scopes)
                .cloned()
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
