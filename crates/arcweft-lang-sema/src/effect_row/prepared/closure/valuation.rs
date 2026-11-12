//! Exact finite-row valuations: named labels and outside-support patterns.

use super::*;

#[derive(Clone, Copy)]
pub(super) struct FiniteRowValuation<'a> {
    pub(super) layout: ClassLayout<'a>,
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
        for class in 0..self.layout.classes() {
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
            work.charge(DecisionWork::Visit)?;
            for _ in 0..point.node_count() {
                work.charge(DecisionWork::Node)?;
            }
            return Ok(point.clone());
        }
        let mut result = Decision::constant(true);
        for mask in 0..self.pattern_count()? {
            let mut bindings = BTreeMap::new();
            for parameter in 0..self.parameters.len() {
                work.charge(DecisionWork::Node)?;
                bindings.insert(
                    Atom::Formal {
                        owner: self.owner,
                        class: 0,
                        parameter,
                    },
                    Decision::constant(mask & (1usize << parameter) != 0),
                );
            }
            let value = point.substitute(&bindings, work)?;
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
        let mut result = self.outside(default, work)?;
        for member in named {
            result = result.and(member, work)?;
        }
        Ok(result)
    }

    /// A distinguished outside label must have a pattern actually present in
    /// the whole valuation. It is a probe, not another whole-row parameter.
    pub(super) fn probe_domain<C: DecisionControl>(
        self,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let mut frontier = Vec::new();
        for mask in 0..self.pattern_count()? {
            work.charge(DecisionWork::Node)?;
            frontier.push(if mask == 0 {
                Decision::constant(true)
            } else {
                Decision::variable(
                    Atom::Presence {
                        owner: self.owner,
                        mask,
                    },
                    work,
                )?
            });
        }
        for parameter in 0..self.parameters.len() {
            let bit = Decision::variable(
                Atom::Formal {
                    owner: self.owner,
                    class: 0,
                    parameter,
                },
                work,
            )?;
            let mut next = Vec::new();
            for pair in frontier.chunks_exact(2) {
                work.charge(DecisionWork::Node)?;
                next.push(bit.conditional(&pair[1], &pair[0], work)?);
            }
            frontier = next;
        }
        frontier
            .pop()
            .ok_or_else(|| PreparedEffectInvariant::TruthTableOverflow.into())
    }
}
