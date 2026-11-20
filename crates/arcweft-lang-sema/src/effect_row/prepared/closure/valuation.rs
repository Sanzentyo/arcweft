//! Exact finite-row valuations: named labels and outside-support patterns.

use super::*;
use arcweft_core::effect_row::{EffectDecisionReplacement, EffectDecisionRequirement};

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

    /// Eliminates this valuation's input references. A decision already
    /// expressed in the receiving cells and choices can keep its graph.
    pub(super) fn quantify_inputs<C: DecisionControl>(
        self,
        predicate: &Decision,
        work: &mut Work<'_, C>,
    ) -> Result<Option<Decision>, PreparedEffectFailure<C::Error>> {
        if self.parameters.is_empty() || predicate.node_count() == 0 {
            return Ok(None);
        }
        let patterns = self.pattern_count()?;
        let mut inputs = BTreeSet::new();
        for atom in predicate.variables() {
            work.charge(DecisionWork::Visit)?;
            let belongs = match atom {
                Atom::Formal {
                    owner,
                    class,
                    parameter,
                } => {
                    *owner == self.owner
                        && *class < self.classes.classes()
                        && *parameter < self.parameters.len()
                }
                Atom::Presence { owner, mask } => {
                    *owner == self.owner && (1..patterns).contains(mask)
                }
                Atom::Cell { .. } | Atom::Choice(_) => false,
            };
            if belongs && !inputs.contains(atom) {
                work.charge(DecisionWork::Node)?;
                inputs.insert(atom.clone());
            }
        }
        if inputs.is_empty() {
            return Ok(None);
        }
        predicate.forall(&inputs, work).map(Some)
    }

    /// This quantifies labels in the same finite-row valuation, not the input
    /// rows themselves. Fold each atom before global Boolean guard operators.
    pub(super) fn outside<C: DecisionControl>(
        self,
        point: &Decision,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        self.outside_requirement(EffectDecisionRequirement::Predicate(point), work)
    }

    pub(super) fn outside_requirement<C: DecisionControl>(
        self,
        requirement: EffectDecisionRequirement<'_, Atom>,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let Some(last_parameter) = self.parameters.len().checked_sub(1) else {
            return requirement.decision(work);
        };
        if !requirement.variables().any(
            |atom| matches!(atom, Atom::Formal { owner, class: 0, .. } if *owner == self.owner),
        ) {
            return requirement.decision(work);
        }
        // A point uses the current scope. Only its outside-label formals
        // precede the named classes, presence tests, cells and choices.
        let through = Atom::Formal {
            owner: self.owner,
            class: 0,
            parameter: last_parameter,
        };
        requirement.all_guarded_prefixes(
            &through,
            0..self.pattern_count()?,
            work,
            |mask, atom| match atom {
                Atom::Formal {
                    owner,
                    class: 0,
                    parameter,
                } if *owner == self.owner => Ok(mask & (1usize << parameter) != 0),
                _ => Err(PreparedEffectInvariant::UnclosedCell.into()),
            },
            |mask| {
                (mask != 0).then_some(Atom::Presence {
                    owner: self.owner,
                    mask,
                })
            },
        )
    }

    fn validate_class_point<'a, C: DecisionControl>(
        self,
        class: usize,
        variables: impl Iterator<Item = &'a Atom>,
        work: &mut Work<'_, C>,
    ) -> Result<bool, PreparedEffectFailure<C::Error>> {
        let mut uses_parameters = false;
        for reference in variables {
            work.charge(DecisionWork::Visit)?;
            match reference {
                Atom::Formal {
                    owner,
                    class: member_class,
                    parameter,
                } if *owner == self.owner
                    && *member_class == class
                    && *parameter < self.parameters.len() =>
                {
                    uses_parameters = true;
                }
                Atom::Formal { .. } | Atom::Presence { .. } => {
                    return Err(PreparedEffectInvariant::UnclosedCell.into());
                }
                _ => {}
            }
        }
        Ok(uses_parameters)
    }

    /// Evaluate a membership point with each input row empty. The common
    /// parameter-independent case can continue to borrow its decision.
    pub(super) fn empty_point<'a, C: DecisionControl>(
        self,
        class: usize,
        point: &'a Decision,
        work: &mut Work<'_, C>,
    ) -> Result<Cow<'a, Decision>, PreparedEffectFailure<C::Error>> {
        if self.parameters.is_empty()
            || !self.validate_class_point(class, point.variables(), work)?
        {
            return Ok(Cow::Borrowed(point));
        }
        point
            .restrict_references(
                |atom| matches!(atom, Atom::Formal { .. }).then_some(false),
                work,
            )
            .map(Cow::Owned)
    }

    /// In an unrestricted scope any assignment can occur at this class,
    /// including at a fresh label outside the named support.
    pub(super) fn all_class_inputs<C: DecisionControl>(
        self,
        class: usize,
        requirement: EffectDecisionRequirement<'_, Atom>,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        if !self.validate_class_point(class, requirement.variables(), work)? {
            return requirement.decision(work);
        }
        let through = Atom::Formal {
            owner: self.owner,
            class,
            parameter: self.parameters.len() - 1,
        };
        self.pattern_count()?;
        requirement.forall_prefix(&through, work)
    }

    /// Over an unrestricted scope every membership mask can occur at each
    /// label class, including a fresh label outside the named support.
    pub(super) fn scheme_equation<C: DecisionControl>(
        self,
        scheme: u64,
        members: &[Decision],
        fixed: &FixedCells,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        if members.len() != self.classes.classes() {
            return Err(PreparedEffectInvariant::UnclosedCell.into());
        }
        let mut classes = Vec::new();
        for (class, point) in members.iter().enumerate() {
            self.validate_class_point(class, point.variables(), work)?;
            let coefficient = |mask| Atom::Cell {
                scheme,
                class,
                mask,
            };
            let equation = match self.parameters.len().checked_sub(1) {
                Some(parameter) => point.matches_prefix_table(
                    &Atom::Formal {
                        owner: self.owner,
                        class,
                        parameter,
                    },
                    0..self.pattern_count()?,
                    work,
                    |mask, reference| match reference {
                        Atom::Formal { parameter, .. } => Ok(mask & (1usize << parameter) != 0),
                        _ => Err(PreparedEffectInvariant::UnclosedCell.into()),
                    },
                    |mask| fixed.operand(coefficient(mask)),
                )?,
                None => Decision::all_equalities(
                    [Ok([
                        EffectDecisionReplacement::Decision(point),
                        fixed.operand(coefficient(0)),
                    ])],
                    work,
                )?,
            };
            work.charge(DecisionWork::Node)?;
            classes.push(equation);
        }
        Decision::all(&classes, work)
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
        Self::join_named(outside, named, work)
    }

    pub(super) fn join_named<C: DecisionControl>(
        outside: Decision,
        named: &[Decision],
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect_row::prepared::test_support::{Budget, budget, parameter};
    use crate::types::{GenericBinder, GenericScope};

    #[test]
    fn coverage_path_groups_include_scopes_without_including_other_operations() {
        let mut labels: Vec<EffectId> = [
            "fs.read(file)",
            "fs.read",
            "fs.read(file.extra)",
            "fs.read.meta",
            "fs.read_more",
            "fs.write",
        ]
        .into_iter()
        .map(|value| value.parse().unwrap())
        .collect();
        labels.sort();
        let classes = LabelClasses { labels: &labels };
        let required = "fs.read(file.extra)".parse().unwrap();
        assert_eq!(
            labels[classes.with_path(&required)]
                .iter()
                .map(EffectId::as_str)
                .collect::<Vec<_>>(),
            ["fs.read", "fs.read(file)", "fs.read(file.extra)"],
        );
    }

    #[test]
    fn input_quantification_visits_only_reachable_formals_and_presence() {
        let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 20));
        let parameters: Vec<_> = (0..20).map(|index| parameter(&scope, index)).collect();
        let owner = FormalOwner::Observation;
        let valuation = FiniteRowValuation {
            classes: LabelClasses { labels: &[] },
            owner,
            parameters: &parameters,
        };
        let mut setup = budget();
        let formal = Decision::variable(
            Atom::Formal {
                owner,
                class: 0,
                parameter: 17,
            },
            &mut setup,
        )
        .unwrap();
        let presence = Decision::variable(Atom::Presence { owner, mask: 7 }, &mut setup).unwrap();
        let foreign = Decision::variable(
            Atom::Presence {
                owner: FormalOwner::Scheme(2),
                mask: 7,
            },
            &mut setup,
        )
        .unwrap();
        let cell = Decision::variable(
            Atom::Cell {
                class: 0,
                mask: 0,
                scheme: 1,
            },
            &mut setup,
        )
        .unwrap();
        let decision = Decision::all(
            [
                &formal.or(&cell, &mut setup).unwrap(),
                &presence.or(&cell, &mut setup).unwrap(),
                &foreign,
            ],
            &mut setup,
        )
        .unwrap();
        let expected = foreign.and(&cell, &mut setup).unwrap();
        let projected = valuation
            .quantify_inputs(&decision, &mut Work(&mut Budget(256)))
            .unwrap();
        assert_eq!(projected.as_ref(), Some(&expected));
        assert!(
            valuation
                .quantify_inputs(&expected, &mut Work(&mut Budget(expected.node_count())))
                .unwrap()
                .is_none()
        );
    }
}
