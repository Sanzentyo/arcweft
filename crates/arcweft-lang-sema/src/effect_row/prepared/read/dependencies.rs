//! One dependency grammar for canonical closure and open source relations.

use super::*;

#[derive(Clone, Copy)]
pub(in crate::effect_row::prepared) enum SchemeDependencies {
    Bodies,
    Headers,
    Observation,
}

pub(in crate::effect_row::prepared) enum Dependency<'a> {
    Function(u64),
    Row(&'a PreparedEffectRow),
    Predicate(&'a PreparedEffectPredicate),
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
    pub(in crate::effect_row::prepared) functions: BTreeSet<u64>,
    pub(in crate::effect_row::prepared) rows: BTreeSet<u64>,
    pub(in crate::effect_row::prepared) predicates: BTreeSet<u64>,
    pub(in crate::effect_row::prepared) choices: BTreeSet<u64>,
    pending: Vec<Dependency<'a>>,
}

impl<'a> Reachable<'a> {
    pub(in crate::effect_row::prepared) fn new(
        view: PreparedEffectReadView<'a>,
        schemes: SchemeDependencies,
    ) -> Self {
        Self {
            view,
            schemes,
            functions: BTreeSet::new(),
            rows: BTreeSet::new(),
            predicates: BTreeSet::new(),
            choices: BTreeSet::new(),
            pending: Vec::new(),
        }
    }

    /// The bound snapshot has already followed every immutable term edge.
    /// Keep its closure only if the receiving view still exposes each exact
    /// definition. Named bodies enter through the normal function traversal.
    pub(super) fn retain_bound_terms<C: DecisionControl>(
        &mut self,
        bound: &BoundScopedEffectCompletion,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        fn retain<'view, T: 'view, C: DecisionControl>(
            definitions: &BTreeMap<u64, Arc<T>>,
            live: impl Fn(u64) -> Option<&'view Arc<T>>,
            ids: &mut BTreeSet<u64>,
            work: &mut Work<'_, C>,
        ) -> Result<(), PreparedEffectFailure<C::Error>> {
            for (&id, definition) in definitions {
                work.charge(DecisionWork::Visit)?;
                let current = live(id).ok_or(PreparedEffectInvariant::StaleReference)?;
                if !Arc::ptr_eq(current, definition) {
                    return Err(PreparedEffectInvariant::ImportedRelationOrigin.into());
                }
                work.charge(DecisionWork::Node)?;
                ids.insert(id);
            }
            Ok(())
        }

        let original = bound.definitions();
        let mut work = Work(control);
        retain(
            &original.rows,
            |id| self.view.row_definition(id),
            &mut self.rows,
            &mut work,
        )?;
        retain(
            &original.predicates,
            |id| self.view.predicate_definition(id),
            &mut self.predicates,
            &mut work,
        )?;
        for &id in &original.choices {
            work.charge(DecisionWork::Visit)?;
            if !self.view.contains_choice(id) {
                return Err(PreparedEffectInvariant::StaleReference.into());
            }
            work.charge(DecisionWork::Node)?;
            self.choices.insert(id);
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
        self.choices.insert(choice.0.ordinal);
        Ok(())
    }

    pub(in crate::effect_row::prepared) fn push<C: DecisionControl>(
        &mut self,
        value: Dependency<'a>,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        self.pending.push(value);
        Ok(())
    }

    pub(in crate::effect_row::prepared) fn collect<C: DecisionControl>(
        mut self,
        control: &mut C,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        while let Some(value) = self.pending.pop() {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            match value {
                Dependency::Function(id) => {
                    if self.functions.contains(&id) {
                        continue;
                    }
                    control
                        .charge(DecisionWork::Node)
                        .map_err(PreparedEffectFailure::Control)?;
                    self.functions.insert(id);
                    let function = self
                        .view
                        .function_definition(id)
                        .ok_or(PreparedEffectInvariant::StaleReference)?;
                    match function.as_ref() {
                        Function::Scheme(scheme)
                            if !matches!(self.schemes, SchemeDependencies::Headers) =>
                        {
                            let SchemeState::BodyPrepared { body, obligations } = &scheme.state
                            else {
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
                            for input in imported.relation.inputs.values() {
                                self.push(
                                    Dependency::Function(
                                        self.view.relation.validate(input.reference())?,
                                    ),
                                    control,
                                )?;
                            }
                        }
                        Function::Scheme(_)
                        | Function::SchemeInput(_)
                        | Function::Unknown { .. } => {}
                    }
                }
                Dependency::Row(PreparedEffectRow::Canonical(_))
                | Dependency::Predicate(PreparedEffectPredicate::Canonical(_)) => {}
                Dependency::Row(PreparedEffectRow::Term(reference)) => {
                    let id = self.view.relation.validate(&reference.0)?;
                    if self.rows.contains(&id) {
                        continue;
                    }
                    control
                        .charge(DecisionWork::Node)
                        .map_err(PreparedEffectFailure::Control)?;
                    self.rows.insert(id);
                    match self.view.row(reference)? {
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
                    }
                }
                Dependency::Predicate(PreparedEffectPredicate::Term(reference)) => {
                    let id = self.view.relation.validate(&reference.0)?;
                    if self.predicates.contains(&id) {
                        continue;
                    }
                    control
                        .charge(DecisionWork::Node)
                        .map_err(PreparedEffectFailure::Control)?;
                    self.predicates.insert(id);
                    match self.view.predicate(reference)? {
                        PredicateTerm::ImportedPrincipal(imported) => {
                            imported.validate(self.view, control)?;
                            for input in imported.inputs.values() {
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
                        PredicateTerm::Not(value) => {
                            self.push(Dependency::Predicate(value), control)?
                        }
                        PredicateTerm::Select { choice, yes, no } => {
                            self.choice(choice, control)?;
                            self.push(Dependency::Predicate(yes), control)?;
                            self.push(Dependency::Predicate(no), control)?;
                        }
                    }
                }
            }
        }
        Ok(self)
    }
}
