//! Exact finite-row valuations: named labels and outside-support patterns.

use super::*;
use arcweft_core::effect_row::EffectDecisionReplacement;

#[derive(Clone, Copy)]
pub(super) struct FiniteRowValuation<'a> {
    pub(super) classes: LabelClasses<'a>,
    pub(super) owner: FormalOwner,
    pub(super) parameters: &'a [GenericEffectReference],
}

impl FiniteRowValuation<'_> {
    fn pattern_count<E>(self) -> Result<usize, PreparedEffectFailure<E>> {
        let shift = u32::try_from(self.parameters.len())
            .map_err(|_| PreparedEffectInvariant::TruthTableOverflow)?;
        1usize
            .checked_shl(shift)
            .ok_or_else(|| PreparedEffectInvariant::TruthTableOverflow.into())
    }

    pub(super) fn formals<C: DecisionControl>(
        self,
        work: &mut Work<'_, C>,
    ) -> Result<BTreeSet<Atom>, PreparedEffectFailure<C::Error>> {
        let mut result = BTreeSet::new();
        for class in 0..self.classes.classes() {
            for parameter in 0..self.parameters.len() {
                work.charge(DecisionWork::Node)?;
                result.insert(Atom::Formal {
                    owner: self.owner,
                    class,
                    parameter,
                });
            }
        }
        for mask in 1..self.pattern_count()? {
            work.charge(DecisionWork::Node)?;
            result.insert(Atom::Presence {
                owner: self.owner,
                mask,
            });
        }
        Ok(result)
    }

    /// This quantifies labels in the same finite-row valuation, not the input
    /// rows themselves. Fold each atom before global Boolean guard operators.
    pub(super) fn outside<C: DecisionControl>(
        self,
        point: &Decision,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        if !point.variables().any(
            |atom| matches!(atom, Atom::Formal { owner, class: 0, .. } if *owner == self.owner),
        ) {
            return work.copy_decision(point);
        }
        let mut result = Decision::constant(true);
        for mask in 0..self.pattern_count()? {
            work.charge(DecisionWork::Visit)?;
            let value = point.restrict_references(
                |atom| match atom {
                    Atom::Formal {
                        owner,
                        class: 0,
                        parameter,
                    } if *owner == self.owner && *parameter < self.parameters.len() => {
                        Some(mask & (1usize << parameter) != 0)
                    }
                    _ => None,
                },
                work,
            )?;
            let value = if mask == 0 {
                value
            } else {
                Decision::variable(
                    Atom::Presence {
                        owner: self.owner,
                        mask,
                    },
                    work,
                )?
                .conditional(&value, &Decision::constant(true), work)?
            };
            result = result.and(&value, work)?;
        }
        Ok(result)
    }

    pub(super) fn whole<C: DecisionControl>(
        self,
        members: &[Decision],
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let (default, named) = members
            .split_first()
            .ok_or(PreparedEffectInvariant::UnclosedCell)?;
        let outside = self.outside(default, work)?;
        if named.is_empty() {
            return Ok(outside);
        }
        Decision::all(std::iter::once(&outside).chain(named), work)
    }

    /// A distinguished outside label must have a pattern actually present in
    /// the whole valuation. It is a probe, not another whole-row parameter.
    pub(super) fn probe_domain<C: DecisionControl>(
        self,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        Decision::select_table(
            self.parameters.len(),
            self.pattern_count()?,
            work,
            |parameter| {
                EffectDecisionReplacement::Variable(Atom::Formal {
                    owner: self.owner,
                    class: 0,
                    parameter,
                })
            },
            |mask, _| {
                Ok(if mask == 0 {
                    EffectDecisionReplacement::Constant(true)
                } else {
                    EffectDecisionReplacement::Variable(Atom::Presence {
                        owner: self.owner,
                        mask,
                    })
                })
            },
        )?
        .ok_or_else(|| PreparedEffectInvariant::TruthTableOverflow.into())
    }
}
