//! Observe a continuation over the finite rows owned by its residual binder.

use super::*;
use crate::effect_row::prepared::read::dependencies::{Dependency, Reachable, SchemeDependencies};

pub(crate) struct ResidualEffectClosure {
    graph: Arc<PreparedEffectDefinitions>,
    concrete: PrincipalEffectClosure,
    rows: BTreeMap<u64, GenericEffectReference>,
    domain: EffectPredicate,
}

impl PreparedEffectDefinitions {
    pub(in crate::effect_row::prepared) fn complete_residual_rows<C: DecisionControl>(
        self,
        ports: &[(&PreparedEffectUnknownRef, &GenericEffectReference)],
        control: &mut C,
    ) -> Result<ResidualEffectClosure, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        let mut rows = BTreeMap::new();
        let mut parameters = BTreeSet::new();
        for &(reference, parameter) in ports {
            work.charge(DecisionWork::Visit)?;
            let reference = FunctionRef::Unknown(reference.clone());
            let function = self.function(&reference)?;
            let Function::Unknown {
                eligibility: PreparedEffectUnknownEligibility::Future,
                origin: Some(origin),
                ..
            } = function
            else {
                return Err(PreparedEffectInvariant::FutureBinding.into());
            };
            if origin.variable() != parameter || !function.parameters().is_empty() {
                return Err(PreparedEffectInvariant::ParameterContract.into());
            }
            work.charge(DecisionWork::Node)?;
            if !parameters.insert(parameter.clone())
                || rows
                    .insert(reference.reference().ordinal, parameter.clone())
                    .is_some()
            {
                return Err(PreparedEffectInvariant::DuplicateFutureBinding.into());
            }
        }

        let mut concrete = Reachable::new(self.read(), SchemeDependencies::Bodies);
        for (&id, function) in &self.functions {
            work.charge(DecisionWork::Visit)?;
            if function.is_future() {
                if !rows.contains_key(&id) {
                    return Err(PreparedEffectInvariant::UnresolvedFuture.into());
                }
            } else {
                concrete.push(Dependency::Function(id), work.0)?;
            }
        }

        let mut concrete_obligations = Vec::new();
        let mut residual_obligations = Vec::new();
        for obligation in &self.obligations {
            let mut reach = Reachable::new(self.read(), SchemeDependencies::Bodies);
            for predicate in [&obligation.domain, &obligation.predicate] {
                reach.push(Dependency::Predicate(predicate), work.0)?;
            }
            let reach = reach.collect(work.0)?;
            if reach.functions.keys().any(|id| rows.contains_key(id)) {
                // A residual obligation can restrict the input domain only
                // after proving that it cannot change the least local tuple.
                if !reach.choices.is_empty()
                    || reach.functions.keys().any(|id| !rows.contains_key(id))
                {
                    return Err(PreparedEffectFailure::NonPointwiseObservation);
                }
                work.charge(DecisionWork::Node)?;
                residual_obligations.push(Arc::clone(obligation));
            } else {
                for predicate in [&obligation.domain, &obligation.predicate] {
                    concrete.push(Dependency::Predicate(predicate), work.0)?;
                }
                work.charge(DecisionWork::Node)?;
                concrete_obligations.push(Arc::clone(obligation));
            }
        }
        let concrete = concrete.collect(work.0)?;
        if concrete.functions.keys().any(|id| rows.contains_key(id)) {
            return Err(PreparedEffectFailure::NonPointwiseObservation);
        }
        let mut choices = BTreeSet::new();
        for &choice in &self.choices {
            work.charge(DecisionWork::Node)?;
            choices.insert(choice);
        }
        let retained = PreparedEffectDefinitions {
            issuer: Arc::clone(&self.issuer),
            choices,
            functions: concrete.functions,
            obligations: concrete_obligations,
            rows: concrete.rows,
            predicates: concrete.predicates,
        };

        let concrete = retained.close_principal(work.0)?;
        work.charge(DecisionWork::Node)?;
        let mut result = ResidualEffectClosure {
            graph: Arc::new(self),
            concrete,
            rows,
            domain: EffectPredicate::unconstrained(),
        };
        result.domain = result.complete_domain(parameters, &residual_obligations, &mut work)?;
        Ok(result)
    }
}

impl ObservationLayout {
    /// Quantify the lexical rows while retaining the whole valuation of the
    /// residual rows. A nonzero pattern may represent one or several labels.
    /// Its cardinality is unobservable in a canonical pointwise predicate.
    fn forall_except<C: DecisionControl>(
        self,
        retained: &BTreeSet<GenericEffectReference>,
        condition: Decision,
        work: &mut Work<'_, C>,
    ) -> Result<(Self, Decision), PreparedEffectFailure<C::Error>> {
        if self
            .parameters
            .iter()
            .all(|reference| retained.contains(reference))
        {
            return Ok((self, condition));
        }
        let mut parameters = Vec::new();
        let mut slots = Vec::new();
        for reference in &self.parameters {
            work.charge(DecisionWork::Visit)?;
            let slot = if retained.contains(reference) {
                work.charge(DecisionWork::Node)?;
                let slot = parameters.len();
                parameters.push(reference.clone());
                Some(slot)
            } else {
                None
            };
            work.charge(DecisionWork::Node)?;
            slots.push(slot);
        }
        let mut substitutions = BTreeMap::new();
        let mut quantified = BTreeSet::new();
        let mut groups: BTreeMap<usize, Vec<Atom>> = BTreeMap::new();
        for atom in condition.variables() {
            work.charge(DecisionWork::Visit)?;
            match *atom {
                Atom::Formal {
                    owner: FormalOwner::Observation,
                    class,
                    parameter,
                } => {
                    if substitutions.contains_key(atom) || quantified.contains(atom) {
                        continue;
                    }
                    work.charge(DecisionWork::Node)?;
                    if let Some(parameter) = slots[parameter] {
                        substitutions.insert(
                            atom.clone(),
                            Decision::variable(
                                Atom::Formal {
                                    owner: FormalOwner::Residual,
                                    class,
                                    parameter,
                                },
                                work,
                            )?,
                        );
                    } else {
                        quantified.insert(atom.clone());
                    }
                }
                Atom::Presence {
                    owner: FormalOwner::Observation,
                    mask,
                } => {
                    if quantified.contains(atom) {
                        continue;
                    }
                    let mut projection = 0;
                    for (index, slot) in slots.iter().enumerate() {
                        work.charge(DecisionWork::Visit)?;
                        if let Some(slot) = slot
                            && mask & (1usize << index) != 0
                        {
                            projection |= 1usize << slot;
                        }
                    }
                    work.charge(DecisionWork::Node)?;
                    quantified.insert(atom.clone());
                    if projection != 0 {
                        work.charge(DecisionWork::Node)?;
                        groups.entry(projection).or_default().push(atom.clone());
                    }
                }
                _ => return Err(PreparedEffectInvariant::UnclosedCell.into()),
            }
        }
        let refinements = 1usize
            .checked_shl(
                u32::try_from(slots.len() - parameters.len())
                    .map_err(|_| PreparedEffectInvariant::TruthTableOverflow)?,
            )
            .ok_or(PreparedEffectInvariant::TruthTableOverflow)?;
        let mut counterexample = condition.substitute(&substitutions, work)?.not(work)?;
        let mut singleton = Decision::constant(true);
        for (mask, observed) in groups {
            let requires_exact = observed.len() == refinements;
            let mut any = Decision::constant(false);
            for atom in observed {
                let present = Decision::variable(atom, work)?;
                // With only one available label there cannot be two distinct
                // refinements of the same nonzero residual pattern.
                singleton = singleton.and(&any.and(&present, work)?.not(work)?, work)?;
                any = any.or(&present, work)?;
            }
            let present = Decision::variable(
                Atom::Presence {
                    owner: FormalOwner::Residual,
                    mask,
                },
                work,
            )?;
            let absent = any.not(work)?;
            counterexample = counterexample.and(
                &present.conditional(
                    &if requires_exact {
                        any
                    } else {
                        Decision::constant(true)
                    },
                    &absent,
                    work,
                )?,
                work,
            )?;
        }
        let saturated = counterexample.exists(&quantified, work)?;
        let singleton = counterexample
            .and(&singleton, work)?
            .exists(&quantified, work)?;
        // Refining more labels can only admit more counterexamples. If the
        // two bounds agree, every legal multiplicity gives the same result.
        if saturated != singleton {
            return Err(PreparedEffectFailure::NonPointwiseObservation);
        }
        let condition = saturated
            .not(work)?
            .map_references(work, &mut |atom, _| match *atom {
                Atom::Formal {
                    owner: FormalOwner::Residual,
                    class,
                    parameter,
                } => Ok(Atom::Formal {
                    owner: FormalOwner::Observation,
                    class,
                    parameter,
                }),
                Atom::Presence {
                    owner: FormalOwner::Residual,
                    mask,
                } => Ok(Atom::Presence {
                    owner: FormalOwner::Observation,
                    mask,
                }),
                _ => Err(PreparedEffectInvariant::UnclosedCell.into()),
            })?;
        Ok((
            Self {
                labels: self.labels,
                parameters: parameters.into_boxed_slice(),
            },
            condition,
        ))
    }
}

impl ResidualEffectClosure {
    pub(crate) fn domain(&self) -> &EffectPredicate {
        &self.domain
    }

    pub(in crate::effect_row::prepared) fn definitions(&self) -> &PreparedEffectDefinitions {
        &self.graph
    }

    fn compilation<'a>(&'a self, layout: &'a ObservationLayout) -> Compilation<'a> {
        let basis = self.concrete.layout.view();
        Compilation::new(
            &self.graph,
            ClassLayout {
                labels: &layout.labels,
                ..basis
            },
            FormalOwner::Observation,
            &layout.parameters,
            ApplicationCells::Solved {
                basis,
                coefficients: &self.concrete.coefficients,
            },
        )
        .with_residual_rows(&self.rows)
    }

    fn complete_domain<C: DecisionControl>(
        &self,
        parameters: BTreeSet<GenericEffectReference>,
        obligations: &[Arc<ScopedObligation>],
        work: &mut Work<'_, C>,
    ) -> Result<EffectPredicate, PreparedEffectFailure<C::Error>> {
        if obligations.is_empty() {
            return Ok(EffectPredicate::unconstrained());
        }
        let mut result = EffectPredicate::unconstrained();
        for obligation in obligations {
            let layout = self.layout(
                &[
                    Root::Predicate(&obligation.domain),
                    Root::Predicate(&obligation.predicate),
                ],
                work,
            )?;
            let mut compilation = self.compilation(&layout);
            let domain = compilation.predicate(&obligation.domain, work)?;
            let consequence = compilation.predicate(&obligation.predicate, work)?;
            let condition = domain.conditional(&consequence, &Decision::constant(true), work)?;
            for reference in &layout.parameters {
                work.charge(DecisionWork::Visit)?;
                if !parameters.contains(reference) && !obligation.parameters.contains(reference) {
                    return Err(
                        PreparedEffectInvariant::UndeclaredParameter(reference.clone()).into(),
                    );
                }
            }
            let (layout, condition) = layout.forall_except(&parameters, condition, work)?;
            let negative = condition.not(work)?;
            let probe = layout.valuation().probe_domain(work)?;
            let predicate = layout.canonical_guard(&condition, &negative, &probe, work)?;
            result = result.and(&predicate, work)?;
        }
        if result.is_impossible() {
            return Err(PreparedEffectFailure::Unsatisfiable);
        }
        Ok(result)
    }

    fn layout<C: DecisionControl>(
        &self,
        roots: &[Root<'_>],
        work: &mut Work<'_, C>,
    ) -> Result<ObservationLayout, PreparedEffectFailure<C::Error>> {
        let mut support =
            ObservationSupport::gather(&self.graph, &self.concrete.layout, roots, work)?;
        for reference in self.rows.values() {
            work.charge(DecisionWork::Node)?;
            support.parameters.insert(reference.clone());
        }
        for _ in &support.labels {
            work.charge(DecisionWork::Node)?;
        }
        for _ in &support.parameters {
            work.charge(DecisionWork::Node)?;
        }
        Ok(ObservationLayout {
            labels: support.labels.into_iter().collect(),
            parameters: support.parameters.into_iter().collect(),
        })
    }

    pub(crate) fn resolve_row<C: DecisionControl>(
        &self,
        row: &PreparedEffectRow,
        control: &mut C,
    ) -> Result<EffectRow, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        let domain = PreparedEffectPredicate::Canonical(self.domain.copy_with_control(&mut work)?);
        let layout = self.layout(&[Root::Row(row), Root::Predicate(&domain)], &mut work)?;
        let mut compilation = self.compilation(&layout);
        let compiled = compilation.row(row, &mut work)?;
        let admitted = compilation.predicate(&domain, &mut work)?;
        let witnesses = &self.concrete.witnesses;
        let accepted = admitted.and(witnesses.relation(), &mut work)?;
        if !accepted
            .and(&compiled.admitted.not(&mut work)?, &mut work)?
            .is_constant(false)
        {
            return Err(PreparedEffectFailure::ObservationNotTotal);
        }
        if witnesses.choices().is_empty() {
            let mut members = Vec::new();
            for (class, member) in compiled.membership.iter().enumerate() {
                work.charge(DecisionWork::Node)?;
                members.push(layout.decode_member(member, class, &mut work)?);
            }
            return layout.assemble_row(members, &mut work);
        }
        let probe = layout.valuation().probe_domain(&mut work)?;
        let mut members = Vec::new();
        for (class, member) in compiled.membership.iter().enumerate() {
            let region = if class == 0 {
                accepted.and(&probe, &mut work)?
            } else {
                work.copy_decision(&accepted)?
            };
            let yes = region
                .and(member, &mut work)?
                .exists(witnesses.choices(), &mut work)?;
            let no = region
                .and(&member.not(&mut work)?, &mut work)?
                .exists(witnesses.choices(), &mut work)?;
            let yes = layout.local(&yes, class, &mut work)?;
            let no = layout.local(&no, class, &mut work)?;
            if !yes.and(&no, &mut work)?.is_constant(false) {
                return Err(PreparedEffectFailure::NonPointwiseObservation);
            }
            work.charge(DecisionWork::Node)?;
            members.push(yes);
        }
        layout.assemble_row(members, &mut work)
    }

    pub(crate) fn resolve_predicate<C: DecisionControl>(
        &self,
        predicate: &PreparedEffectPredicate,
        control: &mut C,
    ) -> Result<EffectPredicate, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        let domain = PreparedEffectPredicate::Canonical(self.domain.copy_with_control(&mut work)?);
        let layout = self.layout(
            &[Root::Predicate(predicate), Root::Predicate(&domain)],
            &mut work,
        )?;
        let mut compilation = self.compilation(&layout);
        let value = compilation.predicate(predicate, &mut work)?;
        let admitted = compilation.predicate(&domain, &mut work)?;
        let witnesses = &self.concrete.witnesses;
        let accepted = admitted.and(witnesses.relation(), &mut work)?;
        let yes = accepted
            .and(&value, &mut work)?
            .exists(witnesses.choices(), &mut work)?;
        let no = accepted
            .and(&value.not(&mut work)?, &mut work)?
            .exists(witnesses.choices(), &mut work)?;
        if !yes.and(&no, &mut work)?.is_constant(false) {
            return Err(PreparedEffectFailure::ObservationNotTotal);
        }
        let probe = layout.valuation().probe_domain(&mut work)?;
        layout.canonical_guard(&yes, &no, &probe, &mut work)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect_row::prepared::test_support::budget;
    use crate::types::{GenericBinder, GenericScope};

    #[test]
    fn quantifying_lexical_rows_preserves_presence_and_checks_cardinality() {
        let mut control = budget();
        let mut work = Work(&mut control);
        let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
        let residual = scope.bound_effect(0, 0).unwrap();
        let lexical = scope.bound_effect(0, 1).unwrap();
        let retained = BTreeSet::from([residual.clone()]);
        let layout = || ObservationLayout {
            labels: Box::new([]),
            parameters: Box::new([residual.clone(), lexical.clone()]),
        };
        let presence = |mask| Atom::Presence {
            owner: FormalOwner::Observation,
            mask,
        };
        let outside_lexical = Decision::variable(presence(1), &mut work).unwrap();
        let inside_lexical = Decision::variable(presence(3), &mut work).unwrap();
        let present = outside_lexical.or(&inside_lexical, &mut work).unwrap();
        let (projected, present) = layout()
            .forall_except(&retained, present, &mut work)
            .unwrap();
        assert_eq!(&*projected.parameters, std::slice::from_ref(&residual));
        assert_eq!(present, outside_lexical);

        let no_proper_split = outside_lexical
            .and(&inside_lexical, &mut work)
            .unwrap()
            .not(&mut work)
            .unwrap();
        assert!(matches!(
            layout().forall_except(&retained, no_proper_split, &mut work),
            Err(PreparedEffectFailure::NonPointwiseObservation)
        ));

        let lexical_only = Decision::variable(presence(2), &mut work).unwrap();
        let exclusive = lexical_only
            .conditional(
                &inside_lexical.not(&mut work).unwrap(),
                &inside_lexical,
                &mut work,
            )
            .unwrap();
        let (_, result) = layout()
            .forall_except(&retained, exclusive, &mut work)
            .unwrap();
        assert!(result.is_constant(false));
    }
}
