//! Observations retain their own parameter/label support, separate from solving.

use super::*;

#[cfg(test)]
mod predicate_tests;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
enum Root<'a> {
    Row(&'a PreparedEffectRow),
    Predicate(&'a PreparedEffectPredicate),
}

struct ObservationSupport {
    labels: BTreeSet<EffectId>,
    parameters: BTreeSet<GenericEffectReference>,
}

struct ObservationLayout {
    labels: Box<[EffectId]>,
    parameters: Box<[GenericEffectReference]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EffectRowRegion {
    Vacuous,
    Rejected,
    MixedAdmission,
    Ambiguous,
    Invariant,
}

struct RowRegions {
    vacuous: Decision,
    rejected: Decision,
    mixed: Decision,
    ambiguous: Decision,
    invariant: Decision,
}

impl RowRegions {
    fn get(&self, region: EffectRowRegion) -> &Decision {
        match region {
            EffectRowRegion::Vacuous => &self.vacuous,
            EffectRowRegion::Rejected => &self.rejected,
            EffectRowRegion::MixedAdmission => &self.mixed,
            EffectRowRegion::Ambiguous => &self.ambiguous,
            EffectRowRegion::Invariant => &self.invariant,
        }
    }
}

/// Whole-valuation regions remain opaque. In particular they are not coerced
/// into the pointwise conjunction grammar of an EffectPredicate.
pub(crate) struct EffectRowObservation<'g> {
    _source: &'g PreparedEffectGraph,
    layout: ObservationLayout,
    regions: RowRegions,
    positive: Box<[Decision]>,
    negative: Box<[Decision]>,
    probe: Decision,
}

/// A canonical local extension has meaning only on its original invariant
/// region. Keeping the borrowed owner prevents losing that guard by conversion.
pub(crate) struct GuardedEffectRow<'o, 'g> {
    observation: &'o EffectRowObservation<'g>,
    row: EffectRow,
}

impl GuardedEffectRow<'_, '_> {
    pub(crate) fn row(&self) -> &EffectRow {
        &self.row
    }
    pub(crate) fn is_total(&self) -> bool {
        self.observation
            .region_is_universal(EffectRowRegion::Invariant)
    }
    pub(crate) fn into_total(self) -> Option<EffectRow> {
        self.is_total().then_some(self.row)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EffectPredicateRegion {
    Vacuous,
    True,
    False,
    Ambiguous,
}

pub(crate) struct EffectPredicateObservation<'g> {
    _source: &'g PreparedEffectGraph,
    layout: ObservationLayout,
    probe: Decision,
    invariant: Decision,
    vacuous: Decision,
    yes: Decision,
    no: Decision,
    ambiguous: Decision,
}

/// A pointwise conjunction representing the observed Boolean condition on
/// exactly the original invariant region. Losing the guard requires totality.
pub(crate) struct GuardedEffectPredicate<'o, 'g> {
    observation: &'o EffectPredicateObservation<'g>,
    predicate: EffectPredicate,
}

impl GuardedEffectPredicate<'_, '_> {
    pub(crate) fn predicate(&self) -> &EffectPredicate {
        &self.predicate
    }
    pub(crate) fn is_total(&self) -> bool {
        self.observation.invariant.is_constant(true)
    }
    pub(crate) fn into_total(self) -> Option<EffectPredicate> {
        self.is_total().then_some(self.predicate)
    }
}

impl<'g> EffectPredicateObservation<'g> {
    fn region(&self, region: EffectPredicateRegion) -> &Decision {
        match region {
            EffectPredicateRegion::Vacuous => &self.vacuous,
            EffectPredicateRegion::True => &self.yes,
            EffectPredicateRegion::False => &self.no,
            EffectPredicateRegion::Ambiguous => &self.ambiguous,
        }
    }
    pub(crate) fn region_is_empty(&self, region: EffectPredicateRegion) -> bool {
        self.region(region).is_constant(false)
    }
    pub(crate) fn region_is_universal(&self, region: EffectPredicateRegion) -> bool {
        self.region(region).is_constant(true)
    }
    pub(crate) fn total_truth(&self) -> Option<bool> {
        if self.yes.is_constant(true) {
            Some(true)
        } else if self.no.is_constant(true) {
            Some(false)
        } else {
            None
        }
    }

    pub(crate) fn canonical_extension<C: DecisionControl>(
        &self,
        control: &mut C,
    ) -> Result<Option<GuardedEffectPredicate<'_, 'g>>, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        work.charge(DecisionWork::Visit)?;
        if self.invariant.is_constant(false) {
            return Ok(None);
        }
        let predicate = if self.yes.is_constant(false) {
            EffectPredicate::impossible()
        } else {
            // The strongest pointwise conjunction accepting every positive
            // valuation permits each local input pattern occurring in one.
            // It is representable iff that conjunction rejects every negative.
            let default =
                self.layout
                    .local(&self.yes.and(&self.probe, &mut work)?, 0, &mut work)?;
            let mut violations = BTreeMap::new();
            for (index, label) in self.layout.labels.iter().enumerate() {
                let allowed = self.layout.local(&self.yes, index + 1, &mut work)?;
                work.charge(DecisionWork::Node)?;
                violations.insert(label.clone(), allowed.not(&mut work)?);
            }
            let violations =
                EffectFormula::try_from_decisions(default.not(&mut work)?, violations, &mut work)?;
            let empty = EffectFormula::from_set(&crate::effects::EffectSet::new(), &mut work)?;
            violations.subset(&empty, &mut work)?
        };
        let actual = self.layout.guard(&predicate, &mut work)?;
        if !self
            .yes
            .and(&actual.not(&mut work)?, &mut work)?
            .is_constant(false)
            || !self.no.and(&actual, &mut work)?.is_constant(false)
        {
            return Err(PreparedEffectFailure::NonPointwiseObservation);
        }
        Ok(Some(GuardedEffectPredicate {
            observation: self,
            predicate,
        }))
    }
}

impl ObservationLayout {
    fn local<C: DecisionControl>(
        &self,
        decision: &Decision,
        class: usize,
        work: &mut Work<'_, C>,
    ) -> Result<EffectDecision<GenericEffectReference>, PreparedEffectFailure<C::Error>> {
        let mut quantified = BTreeSet::new();
        for atom in decision.variables() {
            work.charge(DecisionWork::Visit)?;
            match atom {
                Atom::Formal {
                    owner: FormalOwner::Observation,
                    class: local,
                    ..
                } if *local == class => {}
                Atom::Formal {
                    owner: FormalOwner::Observation,
                    ..
                }
                | Atom::Presence {
                    owner: FormalOwner::Observation,
                    ..
                } => {
                    work.charge(DecisionWork::Node)?;
                    quantified.insert(atom.clone());
                }
                _ => return Err(PreparedEffectInvariant::UnclosedCell.into()),
            }
        }
        decision
            .exists(&quantified, work)?
            .map_references(work, &mut |atom, _| match atom {
                Atom::Formal {
                    owner: FormalOwner::Observation,
                    class: local,
                    parameter,
                } if *local == class => self
                    .parameters
                    .get(*parameter)
                    .cloned()
                    .ok_or_else(|| PreparedEffectInvariant::UnclosedCell.into()),
                _ => Err(PreparedEffectInvariant::UnclosedCell.into()),
            })
    }

    fn valuation(&self) -> FiniteRowValuation<'_> {
        FiniteRowValuation {
            classes: LabelClasses {
                labels: &self.labels,
            },
            owner: FormalOwner::Observation,
            parameters: &self.parameters,
        }
    }

    fn guard<C: DecisionControl>(
        &self,
        predicate: &EffectPredicate,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let valuation = self.valuation();
        let mut members = Vec::new();
        for class in 0..valuation.classes.classes() {
            let value = valuation
                .classes
                .at(predicate.membership(), class)
                .map_references(work, &mut |reference, _| {
                    self.parameters
                        .binary_search(reference)
                        .map(|parameter| Atom::Formal {
                            owner: FormalOwner::Observation,
                            class,
                            parameter,
                        })
                        .map_err(|_| {
                            PreparedEffectInvariant::UndeclaredParameter(reference.clone()).into()
                        })
                })?;
            work.charge(DecisionWork::Node)?;
            members.push(value);
        }
        valuation.whole(&members, work)
    }
}

impl<'g> EffectRowObservation<'g> {
    pub(crate) fn region_is_empty(&self, region: EffectRowRegion) -> bool {
        self.regions.get(region).is_constant(false)
    }
    pub(crate) fn region_is_universal(&self, region: EffectRowRegion) -> bool {
        self.regions.get(region).is_constant(true)
    }

    pub(crate) fn canonical_extension<C: DecisionControl>(
        &self,
        control: &mut C,
    ) -> Result<Option<GuardedEffectRow<'_, 'g>>, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        work.charge(DecisionWork::Visit)?;
        if self.region_is_empty(EffectRowRegion::Invariant) {
            return Ok(None);
        }
        let mut members = Vec::new();
        for (class, (positive, negative)) in self.positive.iter().zip(&self.negative).enumerate() {
            let mut positive = self.regions.invariant.and(positive, &mut work)?;
            let mut negative = self.regions.invariant.and(negative, &mut work)?;
            if class == 0 {
                positive = positive.and(&self.probe, &mut work)?;
                negative = negative.and(&self.probe, &mut work)?;
            }
            let positive = self.layout.local(&positive, class, &mut work)?;
            let negative = self.layout.local(&negative, class, &mut work)?;
            if !positive.and(&negative, &mut work)?.is_constant(false) {
                return Err(PreparedEffectFailure::NonPointwiseObservation);
            }
            work.charge(DecisionWork::Node)?;
            members.push(positive);
        }
        let mut members = members.into_iter();
        let default = members
            .next()
            .ok_or(PreparedEffectInvariant::UnclosedCell)?;
        let mut overrides = BTreeMap::new();
        for (label, member) in self.layout.labels.iter().zip(members) {
            work.charge(DecisionWork::Node)?;
            overrides.insert(label.clone(), member);
        }
        let formula = EffectFormula::try_from_decisions(default, overrides, &mut work)?;
        Ok(Some(GuardedEffectRow {
            observation: self,
            row: EffectRow {
                formula: Some(formula),
            },
        }))
    }
}

impl<'g> PrincipalEffectClosure<'g> {
    fn observation_support<C: DecisionControl>(
        &self,
        roots: &[Root<'_>],
        work: &mut Work<'_, C>,
    ) -> Result<(Box<[EffectId]>, Box<[GenericEffectReference]>), PreparedEffectFailure<C::Error>>
    {
        let support = ObservationSupport::gather(self.graph, &self.layout, roots, work)?;
        for _ in &support.labels {
            work.charge(DecisionWork::Node)?;
        }
        for _ in &support.parameters {
            work.charge(DecisionWork::Node)?;
        }
        Ok((
            support.labels.into_iter().collect(),
            support.parameters.into_iter().collect(),
        ))
    }

    fn project_cells<C: DecisionControl>(
        &self,
        decision: &Decision,
        layout: ClassLayout<'_>,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let mut replacements = BTreeMap::new();
        for atom in decision.variables() {
            work.charge(DecisionWork::Visit)?;
            if let Atom::Cell {
                scheme,
                class,
                mask,
            } = atom
            {
                let original = if *class == 0 {
                    0
                } else {
                    let label = layout
                        .labels
                        .get(*class - 1)
                        .ok_or(PreparedEffectInvariant::UnclosedCell)?;
                    self.layout
                        .labels
                        .binary_search(label)
                        .map_or(0, |index| index + 1)
                };
                let value = self
                    .completion
                    .lower_envelope()
                    .get(&Atom::Cell {
                        scheme: *scheme,
                        class: original,
                        mask: *mask,
                    })
                    .ok_or(PreparedEffectInvariant::UnclosedCell)?;
                if !value.is_constant(true) && !value.is_constant(false) {
                    return Err(PreparedEffectInvariant::UnclosedCell.into());
                }
                work.charge(DecisionWork::Node)?;
                replacements.insert(atom.clone(), Decision::constant(value.is_constant(true)));
            }
        }
        decision.substitute(&replacements, work)
    }

    fn require_whole<E>(decision: &Decision) -> Result<(), PreparedEffectFailure<E>> {
        if decision.variables().any(|atom| {
            matches!(
                atom,
                Atom::Formal { class: 0, .. } | Atom::Cell { .. } | Atom::Reference(_)
            )
        }) {
            return Err(PreparedEffectInvariant::UnclosedCell.into());
        }
        Ok(())
    }

    /// H is an observation scope only. It never constrains the minimized scheme
    /// relation; a later use's success must be observed rather than assumed.
    pub(crate) fn observe_row<C: DecisionControl>(
        &self,
        context: &PreparedEffectPredicate,
        row: &PreparedEffectRow,
        control: &mut C,
    ) -> Result<EffectRowObservation<'g>, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        let (labels, parameters) =
            self.observation_support(&[Root::Predicate(context), Root::Row(row)], &mut work)?;
        let layout = ClassLayout {
            labels: &labels,
            tables: &self.layout.tables,
        };
        let mut compilation =
            Compilation::new(self.graph, layout, FormalOwner::Observation, &parameters);
        let context = self.project_cells(
            &compilation.predicate(context, &mut work)?,
            layout,
            &mut work,
        )?;
        Self::require_whole(&context)?;
        let compiled = compilation.row(row, &mut work)?;
        let admitted = self.project_cells(&compiled.admitted, layout, &mut work)?;
        Self::require_whole(&admitted)?;
        let scoped = self.witnesses.and(&context, &mut work)?;
        let possible = scoped.exists(&self.choices, &mut work)?;
        let yes = scoped
            .and(&admitted, &mut work)?
            .exists(&self.choices, &mut work)?;
        let no = scoped
            .and(&admitted.not(&mut work)?, &mut work)?
            .exists(&self.choices, &mut work)?;
        let accepted = scoped.and(&admitted, &mut work)?;
        let mut positive = Vec::new();
        let mut negative = Vec::new();
        let mut ambiguous = Decision::constant(false);
        for (class, member) in compiled.membership.iter().enumerate() {
            let member = self.project_cells(member, layout, &mut work)?;
            let on = accepted
                .and(&member, &mut work)?
                .exists(&self.choices, &mut work)?;
            let off = accepted
                .and(&member.not(&mut work)?, &mut work)?
                .exists(&self.choices, &mut work)?;
            let mixed = on.and(&off, &mut work)?;
            let mixed = if class == 0 {
                compilation
                    .valuation()
                    .outside(&mixed.not(&mut work)?, &mut work)?
                    .not(&mut work)?
            } else {
                mixed
            };
            ambiguous = ambiguous.or(&mixed, &mut work)?;
            work.charge(DecisionWork::Node)?;
            positive.push(on);
            work.charge(DecisionWork::Node)?;
            negative.push(off);
        }
        let exclusively_admitted = yes.and(&no.not(&mut work)?, &mut work)?;
        let regions = RowRegions {
            vacuous: possible.not(&mut work)?,
            rejected: no.and(&yes.not(&mut work)?, &mut work)?,
            mixed: yes.and(&no, &mut work)?,
            ambiguous: exclusively_admitted.and(&ambiguous, &mut work)?,
            invariant: exclusively_admitted.and(&ambiguous.not(&mut work)?, &mut work)?,
        };
        let probe = compilation.valuation().probe_domain(&mut work)?;
        Ok(EffectRowObservation {
            _source: self.graph,
            layout: ObservationLayout { labels, parameters },
            regions,
            positive: positive.into_boxed_slice(),
            negative: negative.into_boxed_slice(),
            probe,
        })
    }

    pub(crate) fn observe_predicate<C: DecisionControl>(
        &self,
        context: &PreparedEffectPredicate,
        predicate: &PreparedEffectPredicate,
        control: &mut C,
    ) -> Result<EffectPredicateObservation<'g>, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        let (labels, parameters) = self.observation_support(
            &[Root::Predicate(context), Root::Predicate(predicate)],
            &mut work,
        )?;
        let layout = ClassLayout {
            labels: &labels,
            tables: &self.layout.tables,
        };
        let mut compilation =
            Compilation::new(self.graph, layout, FormalOwner::Observation, &parameters);
        let context = self.project_cells(
            &compilation.predicate(context, &mut work)?,
            layout,
            &mut work,
        )?;
        Self::require_whole(&context)?;
        let predicate = self.project_cells(
            &compilation.predicate(predicate, &mut work)?,
            layout,
            &mut work,
        )?;
        Self::require_whole(&predicate)?;
        let scoped = self.witnesses.and(&context, &mut work)?;
        let possible = scoped.exists(&self.choices, &mut work)?;
        let yes = scoped
            .and(&predicate, &mut work)?
            .exists(&self.choices, &mut work)?;
        let no = scoped
            .and(&predicate.not(&mut work)?, &mut work)?
            .exists(&self.choices, &mut work)?;
        let ambiguous = yes.and(&no, &mut work)?;
        let exclusive_yes = yes.and(&no.not(&mut work)?, &mut work)?;
        let exclusive_no = no.and(&yes.not(&mut work)?, &mut work)?;
        let invariant = exclusive_yes.or(&exclusive_no, &mut work)?;
        let probe = compilation.valuation().probe_domain(&mut work)?;
        Ok(EffectPredicateObservation {
            _source: self.graph,
            layout: ObservationLayout { labels, parameters },
            probe,
            invariant,
            vacuous: possible.not(&mut work)?,
            ambiguous,
            yes: exclusive_yes,
            no: exclusive_no,
        })
    }

    pub(crate) fn resolve_row<C: DecisionControl>(
        &self,
        row: &PreparedEffectRow,
        control: &mut C,
    ) -> Result<EffectRow, PreparedEffectFailure<C::Error>> {
        let observation = self.observe_row(
            &PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            row,
            control,
        )?;
        if !observation.region_is_universal(EffectRowRegion::Invariant) {
            return Err(PreparedEffectFailure::ObservationNotTotal);
        }
        observation
            .canonical_extension(control)?
            .and_then(GuardedEffectRow::into_total)
            .ok_or(PreparedEffectInvariant::PrincipalConfirmation.into())
    }
}

impl ObservationSupport {
    fn membership<'a, C: DecisionControl>(
        &mut self,
        view: EffectMembershipView<'a, GenericEffectReference>,
        parameters: impl IntoIterator<Item = &'a GenericEffectReference>,
        substituted: Option<&BTreeMap<GenericEffectReference, PreparedEffectRow>>,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        for (label, _) in view.overrides() {
            work.charge(DecisionWork::Visit)?;
            if !self.labels.contains(label) {
                work.charge(DecisionWork::Node)?;
                self.labels.insert(label.clone());
            }
        }
        for parameter in parameters {
            work.charge(DecisionWork::Visit)?;
            if !substituted.is_some_and(|arguments| arguments.contains_key(parameter))
                && !self.parameters.contains(parameter)
            {
                work.charge(DecisionWork::Node)?;
                self.parameters.insert(parameter.clone());
            }
        }
        Ok(())
    }

    fn gather<C: DecisionControl>(
        graph: &PreparedEffectGraph,
        base: &Layout,
        roots: &[Root<'_>],
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        let mut support = Self {
            labels: BTreeSet::new(),
            parameters: BTreeSet::new(),
        };
        for label in &base.labels {
            work.charge(DecisionWork::Node)?;
            support.labels.insert(label.clone());
        }
        let mut todo = Vec::new();
        for root in roots {
            work.charge(DecisionWork::Node)?;
            todo.push(*root);
        }
        let mut rows = BTreeSet::new();
        let mut predicates = BTreeSet::new();
        while let Some(root) = todo.pop() {
            work.charge(DecisionWork::Visit)?;
            match root {
                Root::Row(PreparedEffectRow::Canonical(row)) => {
                    let formula = row.known()?;
                    support.membership(formula.membership(), formula.variables(), None, work)?;
                }
                Root::Row(PreparedEffectRow::Term(reference)) => {
                    let term = graph.row(reference)?;
                    if rows.contains(&reference.0.ordinal) {
                        continue;
                    }
                    work.charge(DecisionWork::Node)?;
                    rows.insert(reference.0.ordinal);
                    if let RowTerm::Substitution { formula, arguments } = term {
                        support.membership(
                            formula.membership(),
                            formula.variables(),
                            Some(arguments),
                            work,
                        )?;
                    }
                    for child in term.children() {
                        work.charge(DecisionWork::Node)?;
                        todo.push(Root::Row(child));
                    }
                }
                Root::Predicate(PreparedEffectPredicate::Canonical(predicate)) => {
                    support.membership(
                        predicate.membership(),
                        predicate.variables(),
                        None,
                        work,
                    )?;
                }
                Root::Predicate(PreparedEffectPredicate::Term(reference)) => {
                    let term = graph.predicate(reference)?;
                    if predicates.contains(&reference.0.ordinal) {
                        continue;
                    }
                    work.charge(DecisionWork::Node)?;
                    predicates.insert(reference.0.ordinal);
                    match term {
                        PredicateTerm::Substitution {
                            predicate,
                            arguments,
                        } => {
                            support.membership(
                                predicate.membership(),
                                predicate.variables(),
                                Some(arguments),
                                work,
                            )?;
                            for row in arguments.values() {
                                work.charge(DecisionWork::Node)?;
                                todo.push(Root::Row(row));
                            }
                        }
                        PredicateTerm::Relation {
                            actual, permitted, ..
                        } => {
                            for row in [actual, permitted] {
                                work.charge(DecisionWork::Node)?;
                                todo.push(Root::Row(row));
                            }
                        }
                        PredicateTerm::And(children) | PredicateTerm::Or(children) => {
                            for child in children {
                                work.charge(DecisionWork::Node)?;
                                todo.push(Root::Predicate(child));
                            }
                        }
                        PredicateTerm::Not(child) => {
                            work.charge(DecisionWork::Node)?;
                            todo.push(Root::Predicate(child));
                        }
                        PredicateTerm::Select { yes, no, .. } => {
                            for child in [yes, no] {
                                work.charge(DecisionWork::Node)?;
                                todo.push(Root::Predicate(child));
                            }
                        }
                    }
                }
            }
        }
        Ok(support)
    }
}
