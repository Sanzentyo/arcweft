//! One dependency grammar for canonical closure and open source relations.

use super::*;

#[derive(Clone, Copy)]
pub(in crate::effect_row::prepared) enum SchemeDependencies {
    Bodies,
    Headers,
    Observation,
}

#[derive(Clone, Copy)]
pub(in crate::effect_row::prepared) enum Dependency<'a> {
    Function(u64),
    Row(&'a PreparedEffectRow),
    Predicate(&'a PreparedEffectPredicate),
}

enum PendingDependency<'a> {
    Function(u64, &'a Function),
    Row(&'a RowTerm),
    Predicate(&'a PredicateTerm),
}

#[derive(Default)]
pub(in crate::effect_row::prepared) struct ReachedDefinitions {
    pub(in crate::effect_row::prepared) functions: BTreeMap<u64, Arc<Function>>,
    pub(in crate::effect_row::prepared) rows: BTreeMap<u64, Arc<RowTerm>>,
    pub(in crate::effect_row::prepared) predicates: BTreeMap<u64, Arc<PredicateTerm>>,
    pub(in crate::effect_row::prepared) choices: BTreeSet<u64>,
}

impl PreparedEffectReadView<'_> {
    pub(crate) fn validate_query_roots<C: DecisionControl>(
        self,
        rows: &[&PreparedEffectRow],
        predicates: &[&PreparedEffectPredicate],
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        self.validate_reachable_query(rows, predicates, SchemeDependencies::Headers, control)
    }

    /// Check whether the source can be closed while keeping its original
    /// equations open for joint inference.
    pub(crate) fn require_query_bodies<C: DecisionControl>(
        self,
        rows: &[&PreparedEffectRow],
        predicates: &[&PreparedEffectPredicate],
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        self.validate_reachable_query(rows, predicates, SchemeDependencies::Observation, control)
    }

    fn validate_reachable_query<C: DecisionControl>(
        self,
        rows: &[&PreparedEffectRow],
        predicates: &[&PreparedEffectPredicate],
        schemes: SchemeDependencies,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        let mut reachable = Reachable::new(self, schemes);
        for row in rows {
            reachable.push(Dependency::Row(row), control)?;
        }
        for predicate in predicates {
            reachable.push(Dependency::Predicate(predicate), control)?;
        }
        reachable.collect(control)?;
        Ok(())
    }
}

pub(in crate::effect_row::prepared) struct Reachable<'a> {
    view: PreparedEffectReadView<'a>,
    schemes: SchemeDependencies,
    definitions: ReachedDefinitions,
    pending: Vec<PendingDependency<'a>>,
}

impl<'a> Reachable<'a> {
    pub(in crate::effect_row::prepared) fn new(
        view: PreparedEffectReadView<'a>,
        schemes: SchemeDependencies,
    ) -> Self {
        Self {
            view,
            schemes,
            definitions: ReachedDefinitions::default(),
            pending: Vec::new(),
        }
    }

    /// A verified original already contains the dependencies of this live
    /// function. A Named declaration may still need its body traversed.
    pub(super) fn retain_function<C: DecisionControl>(
        &mut self,
        id: u64,
        definition: &Arc<Function>,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        work.charge(DecisionWork::Node)?;
        self.definitions
            .functions
            .insert(id, Arc::clone(definition));
        Ok(())
    }

    /// The original snapshot has already followed every immutable term edge.
    /// Keep its closure only if the receiving view still exposes each exact
    /// definition. Named bodies enter through the normal function traversal.
    pub(super) fn retain_terms<C: DecisionControl>(
        &mut self,
        original: &PreparedEffectDefinitions,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        fn retain<'view, T: 'view, C: DecisionControl>(
            definitions: &BTreeMap<u64, Arc<T>>,
            live: impl Fn(u64) -> Option<&'view Arc<T>>,
            reached: &mut BTreeMap<u64, Arc<T>>,
            work: &mut Work<'_, C>,
        ) -> Result<(), PreparedEffectFailure<C::Error>> {
            for (&id, definition) in definitions {
                work.charge(DecisionWork::Visit)?;
                let current = live(id).ok_or(PreparedEffectInvariant::StaleReference)?;
                if !Arc::ptr_eq(current, definition) {
                    return Err(PreparedEffectInvariant::ImportedRelationOrigin.into());
                }
                work.charge(DecisionWork::Node)?;
                reached.insert(id, Arc::clone(current));
            }
            Ok(())
        }

        let mut work = Work(control);
        retain(
            &original.rows,
            |id| self.view.row_definition(id),
            &mut self.definitions.rows,
            &mut work,
        )?;
        retain(
            &original.predicates,
            |id| self.view.predicate_definition(id),
            &mut self.definitions.predicates,
            &mut work,
        )?;
        for &id in &original.choices {
            work.charge(DecisionWork::Visit)?;
            if !self.view.contains_choice(id) {
                return Err(PreparedEffectInvariant::StaleReference.into());
            }
            work.charge(DecisionWork::Node)?;
            self.definitions.choices.insert(id);
        }
        Ok(())
    }

    fn choice<C: DecisionControl>(
        &mut self,
        choice: &PreparedEffectChoice,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        self.view.validate_choice(choice)?;
        control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        self.definitions.choices.insert(choice.0.ordinal);
        Ok(())
    }

    pub(in crate::effect_row::prepared) fn push<C: DecisionControl>(
        &mut self,
        value: Dependency<'_>,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        work.charge(DecisionWork::Visit)?;
        let pending = match value {
            Dependency::Function(id) => Self::discover(
                &mut self.definitions.functions,
                id,
                || self.view.function_definition(id),
                &mut work,
            )?
            .map(|definition| PendingDependency::Function(id, definition)),
            Dependency::Row(PreparedEffectRow::Term(reference)) => {
                let id = self.view.relation.validate(&reference.0)?;
                Self::discover(
                    &mut self.definitions.rows,
                    id,
                    || self.view.row_definition(id),
                    &mut work,
                )?
                .map(PendingDependency::Row)
            }
            Dependency::Predicate(PreparedEffectPredicate::Term(reference)) => {
                let id = self.view.relation.validate(&reference.0)?;
                Self::discover(
                    &mut self.definitions.predicates,
                    id,
                    || self.view.predicate_definition(id),
                    &mut work,
                )?
                .map(PendingDependency::Predicate)
            }
            Dependency::Row(PreparedEffectRow::Canonical(_))
            | Dependency::Predicate(PreparedEffectPredicate::Canonical(_)) => return Ok(()),
        };
        if let Some(pending) = pending {
            self.pending.push(pending);
        }
        Ok(())
    }

    fn discover<T: 'a, C: DecisionControl>(
        reached: &mut BTreeMap<u64, Arc<T>>,
        id: u64,
        live: impl FnOnce() -> Option<&'a Arc<T>>,
        work: &mut Work<'_, C>,
    ) -> Result<Option<&'a T>, PreparedEffectFailure<C::Error>> {
        let std::collections::btree_map::Entry::Vacant(entry) = reached.entry(id) else {
            return Ok(None);
        };
        let definition = live().ok_or(PreparedEffectInvariant::StaleReference)?;
        // Admit both the retained definition and its one pending visit
        // before cloning or queueing. That same entry deduplicates edges.
        work.charge(DecisionWork::Node)?;
        work.charge(DecisionWork::Node)?;
        entry.insert(Arc::clone(definition));
        Ok(Some(definition))
    }

    pub(in crate::effect_row::prepared) fn collect<C: DecisionControl>(
        mut self,
        control: &mut C,
    ) -> Result<ReachedDefinitions, PreparedEffectFailure<C::Error>> {
        while let Some(value) = self.pending.pop() {
            match value {
                PendingDependency::Function(id, function) => match function {
                    Function::Scheme(scheme)
                        if !matches!(self.schemes, SchemeDependencies::Headers) =>
                    {
                        let SchemeState::BodyPrepared { body, obligations } = &scheme.state else {
                            if matches!(self.schemes, SchemeDependencies::Observation) {
                                return Err(PreparedEffectFailure::ObservationNeedsBody(
                                    PreparedEffectSchemeRef(self.view.relation.reference(id)),
                                ));
                            }
                            return Err(PreparedEffectInvariant::SchemeState.into());
                        };
                        self.push(Dependency::Row(body), control)?;
                        for predicate in obligations {
                            self.push(Dependency::Predicate(predicate), control)?;
                        }
                    }
                    Function::Imported(imported) => {
                        for input in imported.relation.inputs().values() {
                            self.push(
                                Dependency::Function(
                                    self.view.relation.validate(input.reference())?,
                                ),
                                control,
                            )?;
                        }
                    }
                    Function::Scheme(_) | Function::SchemeInput(_) | Function::Unknown { .. } => {}
                },
                PendingDependency::Row(row) => match row {
                    RowTerm::Application {
                        function,
                        arguments,
                    } => {
                        self.push(
                            Dependency::Function(
                                self.view.relation.validate(function.reference())?,
                            ),
                            control,
                        )?;
                        for row in arguments {
                            self.push(Dependency::Row(row), control)?;
                        }
                    }
                    RowTerm::Substitution { arguments, .. } => {
                        for row in arguments.values() {
                            self.push(Dependency::Row(row), control)?;
                        }
                    }
                    RowTerm::Union(rows) => {
                        for row in rows {
                            self.push(Dependency::Row(row), control)?;
                        }
                    }
                    RowTerm::Select { choice, yes, no } => {
                        self.choice(choice, control)?;
                        self.push(Dependency::Row(yes), control)?;
                        self.push(Dependency::Row(no), control)?;
                    }
                },
                PendingDependency::Predicate(predicate) => match predicate {
                    PredicateTerm::ImportedPrincipal(imported) => {
                        imported.validate(self.view, control)?;
                        for input in imported.inputs().values() {
                            self.push(
                                Dependency::Function(
                                    self.view.relation.validate(input.reference())?,
                                ),
                                control,
                            )?;
                        }
                    }
                    PredicateTerm::Substitution { arguments, .. } => {
                        for row in arguments.values() {
                            self.push(Dependency::Row(row), control)?;
                        }
                    }
                    PredicateTerm::Relation {
                        actual, permitted, ..
                    } => {
                        self.push(Dependency::Row(actual), control)?;
                        self.push(Dependency::Row(permitted), control)?;
                    }
                    PredicateTerm::And(values) | PredicateTerm::Or(values) => {
                        for predicate in values {
                            self.push(Dependency::Predicate(predicate), control)?;
                        }
                    }
                    PredicateTerm::Equivalent([left, right]) => {
                        self.push(Dependency::Predicate(left), control)?;
                        self.push(Dependency::Predicate(right), control)?;
                    }
                    PredicateTerm::Not(value) => {
                        self.push(Dependency::Predicate(value), control)?
                    }
                    PredicateTerm::Select { choice, yes, no } => {
                        self.choice(choice, control)?;
                        self.push(Dependency::Predicate(yes), control)?;
                        self.push(Dependency::Predicate(no), control)?;
                    }
                },
            }
        }
        Ok(self.definitions)
    }
}
