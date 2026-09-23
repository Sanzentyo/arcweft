//! Finite-set effect formulas and universally interpreted effect predicates.
//!
//! Both use the same canonical label-membership representation. Their private
//! constructors enforce different laws: a row is empty outside the finite
//! support of its inputs; a predicate must hold at every label, including the
//! infinitely many labels outside that support.

use std::collections::{BTreeMap, BTreeSet};

use super::decision::{DecisionControl, DecisionEncoding, DecisionWork, EffectDecision};
use crate::effects::{EffectId, EffectSet};

#[cfg(test)]
mod tests;

pub(crate) trait MembershipEncoding<V>: DecisionEncoding<V> {
    fn effect(&mut self, effect: &EffectId) -> Result<(), Self::Error>;
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct Membership<V> {
    default: EffectDecision<V>,
    overrides: BTreeMap<EffectId, EffectDecision<V>>,
}

/// A symbolic finite effect set over scoped row references.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct EffectFormula<V>(Membership<V>);

/// A relation that must hold for every effect label under finite row valuations.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EffectPredicate<V = crate::types::GenericEffectReference>(Membership<V>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EffectCompletion<V> {
    pub(super) admissibility: EffectPredicate<V>,
    pub(super) least: Option<BTreeMap<V, EffectFormula<V>>>,
}

#[derive(Clone, Copy)]
enum Operation {
    Union,
    Intersection,
    Difference,
    Subset,
}

impl<V: Clone + Ord> Membership<V> {
    fn encode<E: MembershipEncoding<V>>(&self, encoder: &mut E) -> Result<(), E::Error> {
        self.default.encode(encoder)?;
        encoder.count(self.overrides.len())?;
        for (effect, decision) in &self.overrides {
            encoder.effect(effect)?;
            decision.encode(encoder)?;
        }
        Ok(())
    }
    fn constant(value: bool) -> Self {
        Self {
            default: EffectDecision::constant(value),
            overrides: BTreeMap::new(),
        }
    }

    fn at(&self, label: &EffectId) -> &EffectDecision<V> {
        self.overrides.get(label).unwrap_or(&self.default)
    }

    fn map_references<U: Clone + Ord, C: DecisionControl>(
        &self,
        control: &mut C,
        mapping: &mut impl FnMut(&V, &mut C) -> Result<U, C::Error>,
    ) -> Result<Membership<U>, C::Error> {
        let default = self.default.map_references(control, mapping)?;
        let mut membership = Membership {
            default,
            overrides: BTreeMap::new(),
        };
        for (label, decision) in &self.overrides {
            control.charge(DecisionWork::Visit)?;
            let decision = decision.map_references(control, mapping)?;
            membership.insert_override(label.clone(), decision, control)?;
        }
        Ok(membership)
    }

    fn insert_override<C: DecisionControl>(
        &mut self,
        label: EffectId,
        decision: EffectDecision<V>,
        control: &mut C,
    ) -> Result<(), C::Error> {
        control.charge(DecisionWork::Visit)?;
        if decision != self.default {
            self.overrides.insert(label, decision);
        }
        Ok(())
    }

    fn combine<C: DecisionControl>(
        &self,
        other: &Self,
        operation: Operation,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        let apply = |left: &EffectDecision<V>, right: &EffectDecision<V>, control: &mut C| {
            match operation {
                Operation::Union => left.or(right, control),
                Operation::Intersection => left.and(right, control),
                Operation::Difference => {
                    right.conditional(&EffectDecision::constant(false), left, control)
                }
                Operation::Subset => {
                    left.conditional(right, &EffectDecision::constant(true), control)
                }
            }
        };
        let default = apply(&self.default, &other.default, control)?;
        let mut result = Self {
            default,
            overrides: BTreeMap::new(),
        };
        let mut labels = BTreeSet::new();
        for label in self.overrides.keys().chain(other.overrides.keys()) {
            control.charge(DecisionWork::Visit)?;
            labels.insert(label);
        }
        for label in labels {
            let decision = apply(self.at(label), other.at(label), control)?;
            result.insert_override(label.clone(), decision, control)?;
        }
        Ok(result)
    }

    fn substitute<C: DecisionControl>(
        &self,
        replacements: &BTreeMap<V, EffectFormula<V>>,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        let mut labels = BTreeSet::new();
        let mut defaults = BTreeMap::new();
        for label in self.overrides.keys() {
            control.charge(DecisionWork::Visit)?;
            labels.insert(label);
        }
        for (variable, replacement) in replacements {
            control.charge(DecisionWork::Visit)?;
            defaults.insert(variable.clone(), replacement.0.default.clone());
            for label in replacement.0.overrides.keys() {
                control.charge(DecisionWork::Visit)?;
                labels.insert(label);
            }
        }
        let default = self.default.substitute(&defaults, control)?;
        let mut result = Self {
            default,
            overrides: BTreeMap::new(),
        };
        for label in labels {
            let mut at_label = BTreeMap::new();
            for (variable, replacement) in replacements {
                control.charge(DecisionWork::Visit)?;
                at_label.insert(variable.clone(), replacement.0.at(label).clone());
            }
            let decision = self.at(label).substitute(&at_label, control)?;
            result.insert_override(label.clone(), decision, control)?;
        }
        Ok(result)
    }
}

impl<V: Clone + Ord> EffectFormula<V> {
    pub(super) fn encode<E: MembershipEncoding<V>>(&self, encoder: &mut E) -> Result<(), E::Error> {
        self.0.encode(encoder)
    }
    /// Literal construction from already owned atoms and an optional reference.
    /// Algebraic operations use the controlled decision builder.
    pub(super) fn literal(effects: EffectSet, reference: Option<V>) -> Self {
        Self(Membership {
            default: reference.map_or_else(
                || EffectDecision::constant(false),
                EffectDecision::reference,
            ),
            overrides: effects
                .into_iter()
                .map(|effect| (effect, EffectDecision::constant(true)))
                .collect(),
        })
    }

    pub(super) fn variables(&self) -> impl Iterator<Item = &V> {
        self.0.default.variables().chain(
            self.0
                .overrides
                .values()
                .flat_map(EffectDecision::variables),
        )
    }

    pub(super) fn constant_effects(&self) -> EffectSet {
        self.0
            .overrides
            .iter()
            .filter(|(_, decision)| decision.is_constant(true))
            .map(|(effect, _)| effect.clone())
            .collect()
    }

    pub(super) fn single_reference(&self) -> Option<&V> {
        self.0
            .overrides
            .values()
            .all(|decision| decision.is_constant(true))
            .then(|| self.0.default.single_reference())
            .flatten()
    }

    pub(super) fn is_closed(&self) -> bool {
        self.variables().next().is_none()
    }
    pub(super) fn map_references<U: Clone + Ord, C: DecisionControl>(
        &self,
        control: &mut C,
        mapping: &mut impl FnMut(&V, &mut C) -> Result<U, C::Error>,
    ) -> Result<EffectFormula<U>, C::Error> {
        self.0.map_references(control, mapping).map(EffectFormula)
    }
    pub(super) fn empty() -> Self {
        Self(Membership::constant(false))
    }

    pub(super) fn from_set<C: DecisionControl>(
        effects: &EffectSet,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        let mut membership = Membership::constant(false);
        for effect in effects.iter() {
            membership.insert_override(effect.clone(), EffectDecision::constant(true), control)?;
        }
        Ok(Self(membership))
    }

    pub(super) fn variable<C: DecisionControl>(
        variable: V,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        Ok(Self(Membership {
            default: EffectDecision::variable(variable, control)?,
            overrides: BTreeMap::new(),
        }))
    }

    pub(super) fn union<C: DecisionControl>(
        &self,
        other: &Self,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        self.0
            .combine(&other.0, Operation::Union, control)
            .map(Self)
    }

    pub(super) fn intersection<C: DecisionControl>(
        &self,
        other: &Self,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        self.0
            .combine(&other.0, Operation::Intersection, control)
            .map(Self)
    }

    pub(super) fn difference<C: DecisionControl>(
        &self,
        other: &Self,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        self.0
            .combine(&other.0, Operation::Difference, control)
            .map(Self)
    }

    pub(super) fn subset<C: DecisionControl>(
        &self,
        permitted: &Self,
        control: &mut C,
    ) -> Result<EffectPredicate<V>, C::Error> {
        self.0
            .combine(&permitted.0, Operation::Subset, control)
            .map(EffectPredicate::normalized)
    }

    pub(super) fn substitute<C: DecisionControl>(
        &self,
        replacements: &BTreeMap<V, Self>,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        self.0.substitute(replacements, control).map(Self)
    }

    pub(super) fn closed<C: DecisionControl>(
        &self,
        control: &mut C,
    ) -> Result<Option<EffectSet>, C::Error> {
        if !self.0.default.is_constant(false) {
            return Ok(None);
        }
        let mut effects = EffectSet::new();
        for (label, decision) in &self.0.overrides {
            control.charge(DecisionWork::Visit)?;
            if !decision.is_constant(true) {
                return Ok(None);
            }
            effects.insert(label.clone());
        }
        Ok(Some(effects))
    }
}

impl<V: Clone + Ord> EffectPredicate<V> {
    pub(crate) fn equal_with<C: DecisionControl>(
        &self,
        other: &Self,
        control: &mut C,
    ) -> Result<bool, C::Error> {
        self.encode(&mut super::EffectEqualityControl(control))?;
        other.encode(&mut super::EffectEqualityControl(control))?;
        Ok(self == other)
    }

    pub(crate) fn encode<E: MembershipEncoding<V>>(&self, encoder: &mut E) -> Result<(), E::Error> {
        self.0.encode(encoder)
    }

    pub(crate) fn variables(&self) -> impl Iterator<Item = &V> {
        self.0.default.variables().chain(
            self.0
                .overrides
                .values()
                .flat_map(EffectDecision::variables),
        )
    }

    pub(crate) fn map_references<U: Clone + Ord, C: DecisionControl>(
        &self,
        control: &mut C,
        mapping: &mut impl FnMut(&V, &mut C) -> Result<U, C::Error>,
    ) -> Result<EffectPredicate<U>, C::Error> {
        self.0
            .map_references(control, mapping)
            .map(EffectPredicate::normalized)
    }
    fn normalized(membership: Membership<V>) -> Self {
        // All finite inputs are empty at infinitely many unmentioned labels.
        // Failure at that all-zero valuation is therefore globally impossible.
        // A permanently false explicit label is likewise impossible.
        if membership.default.evaluate(|_| false) {
            Self(membership)
        } else {
            Self(Membership::constant(false))
        }
    }

    pub(super) fn unconstrained() -> Self {
        Self(Membership::constant(true))
    }

    pub(super) fn impossible() -> Self {
        Self(Membership::constant(false))
    }

    pub(crate) fn is_unconstrained(&self) -> bool {
        self.0.default.is_constant(true) && self.0.overrides.is_empty()
    }

    pub(super) fn is_impossible(&self) -> bool {
        self.0.default.is_constant(false)
            || self.0.overrides.values().any(|row| row.is_constant(false))
    }

    /// Concrete label classes whose projected relation has no witness.
    pub(super) fn rejected_labels<C: DecisionControl>(
        &self,
        control: &mut C,
    ) -> Result<EffectSet, C::Error> {
        let mut labels = EffectSet::new();
        for (label, decision) in &self.0.overrides {
            control.charge(DecisionWork::Visit)?;
            if decision.is_constant(false) {
                labels.insert(label.clone());
            }
        }
        Ok(labels)
    }

    pub(super) fn project<C: DecisionControl>(
        &self,
        quantified: &BTreeSet<V>,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        let default = self.0.default.exists(quantified, control)?;
        let mut membership = Membership {
            default,
            overrides: BTreeMap::new(),
        };
        for (label, relation) in &self.0.overrides {
            control.charge(DecisionWork::Visit)?;
            membership.insert_override(
                label.clone(),
                relation.exists(quantified, control)?,
                control,
            )?;
        }
        Ok(Self::normalized(membership))
    }

    pub(super) fn and<C: DecisionControl>(
        &self,
        other: &Self,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        self.0
            .combine(&other.0, Operation::Intersection, control)
            .map(Self::normalized)
    }

    pub(super) fn substitute<C: DecisionControl>(
        &self,
        replacements: &BTreeMap<V, EffectFormula<V>>,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        self.0
            .substitute(replacements, control)
            .map(Self::normalized)
    }

    pub(super) fn complete<C: DecisionControl>(
        &self,
        quantified: &BTreeSet<V>,
        control: &mut C,
    ) -> Result<EffectCompletion<V>, C::Error> {
        let default = self.0.default.complete(quantified, control)?;
        let mut admissibility = Membership {
            default: default.admissibility,
            overrides: BTreeMap::new(),
        };
        let mut least = default.least.map(|rows| {
            rows.into_iter()
                .map(|(variable, default)| {
                    (
                        variable,
                        Membership {
                            default,
                            overrides: BTreeMap::new(),
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>()
        });
        for (label, relation) in &self.0.overrides {
            control.charge(DecisionWork::Visit)?;
            let completed = relation.complete(quantified, control)?;
            admissibility.insert_override(label.clone(), completed.admissibility, control)?;
            match (&mut least, completed.least) {
                (Some(rows), Some(completed)) => {
                    for (variable, decision) in completed {
                        control.charge(DecisionWork::Visit)?;
                        rows.get_mut(&variable)
                            .expect("every completion retains the quantified inventory")
                            .insert_override(label.clone(), decision, control)?;
                    }
                }
                _ => least = None,
            }
        }
        let admissibility = Self::normalized(admissibility);
        let least = if admissibility.is_impossible() {
            // The empty tuple of valuations witnesses nothing; retaining empty
            // row terms avoids misclassifying unsatisfiability as ambiguity.
            let mut rows = BTreeMap::new();
            for variable in quantified {
                control.charge(DecisionWork::Visit)?;
                rows.insert(variable.clone(), EffectFormula::empty());
            }
            Some(rows)
        } else {
            least.map(|rows| {
                rows.into_iter()
                    .map(|(variable, membership)| {
                        debug_assert!(!membership.default.evaluate(|_| false));
                        (variable, EffectFormula(membership))
                    })
                    .collect()
            })
        };
        Ok(EffectCompletion {
            admissibility,
            least,
        })
    }
}
