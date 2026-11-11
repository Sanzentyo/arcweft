//! Capture-free simultaneous substitution through the one prepared term grammar.
//!
//! The owner rewrites immutable terms, never declaration bodies or source. A
//! replacement belongs to the destination scope and is not substituted again.

use super::*;
use crate::effect_row::EffectEqualityControl;

struct Substitution<'a, C> {
    graph: &'a mut PreparedEffectGraph,
    arguments: &'a BTreeMap<GenericEffectReference, PreparedEffectRow>,
    control: &'a mut C,
    rows: BTreeMap<u64, PreparedEffectRow>,
    predicates: BTreeMap<u64, PreparedEffectPredicate>,
}

impl<C: DecisionControl> Substitution<'_, C> {
    fn charge(&mut self, work: DecisionWork) -> Result<(), PreparedEffectFailure<C::Error>> {
        self.control
            .charge(work)
            .map_err(PreparedEffectFailure::Control)
    }

    fn copy_row(
        &mut self,
        row: &PreparedEffectRow,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        match row {
            PreparedEffectRow::Canonical(row) => row
                .encode(&mut EffectEqualityControl(self.control))
                .map_err(PreparedEffectFailure::Control)?,
            PreparedEffectRow::Term(reference) => {
                self.graph.row(reference)?;
                self.charge(DecisionWork::Visit)?;
            }
        }
        Ok(row.clone())
    }

    fn bindings<'a>(
        &mut self,
        variables: impl Iterator<Item = &'a GenericEffectReference>,
        mut mapped: BTreeMap<GenericEffectReference, PreparedEffectRow>,
    ) -> Result<BTreeMap<GenericEffectReference, PreparedEffectRow>, PreparedEffectFailure<C::Error>>
    {
        for variable in variables {
            self.charge(DecisionWork::Visit)?;
            if !mapped.contains_key(variable)
                && let Some(replacement) = self.arguments.get(variable)
            {
                let replacement = self.copy_row(replacement)?;
                self.charge(DecisionWork::Node)?;
                mapped.insert(variable.clone(), replacement);
            }
        }
        Ok(mapped)
    }

    fn read_row(
        &mut self,
        row: &PreparedEffectRow,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        match row {
            PreparedEffectRow::Canonical(row) => {
                let Some(formula) = row.formula() else {
                    return self.copy_row(&PreparedEffectRow::Canonical(row.clone()));
                };
                let arguments = self.bindings(formula.variables(), BTreeMap::new())?;
                formula
                    .encode(&mut EffectEqualityControl(self.control))
                    .map_err(PreparedEffectFailure::Control)?;
                if arguments.is_empty() {
                    Ok(PreparedEffectRow::Canonical(row.clone()))
                } else {
                    self.graph
                        .apply_formula(formula.clone(), arguments, self.control)
                }
            }
            PreparedEffectRow::Term(reference) => {
                self.graph.row(reference)?;
                let mapped = self
                    .rows
                    .get(&reference.0.ordinal)
                    .ok_or(PreparedEffectInvariant::StaleReference)?
                    .clone();
                self.copy_row(&mapped)
            }
        }
    }

    fn row(
        &mut self,
        root: &PreparedEffectRow,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        self.charge(DecisionWork::Node)?;
        let mut todo = vec![root];
        let mut reachable = BTreeSet::new();
        while let Some(row) = todo.pop() {
            self.control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            if let PreparedEffectRow::Term(reference) = row {
                let term = self.graph.row(reference)?;
                if !self.rows.contains_key(&reference.0.ordinal)
                    && reachable.insert(reference.0.ordinal)
                {
                    for child in term.children() {
                        self.control
                            .charge(DecisionWork::Node)
                            .map_err(PreparedEffectFailure::Control)?;
                        todo.push(child);
                    }
                }
            }
        }
        for id in reachable {
            self.charge(DecisionWork::Visit)?;
            let term = Arc::clone(&self.graph.rows[&id]);
            let mapped = match term.as_ref() {
                RowTerm::Application { scheme, arguments } => {
                    let mut mapped = Vec::new();
                    for argument in arguments {
                        self.charge(DecisionWork::Node)?;
                        mapped.push(self.read_row(argument)?);
                    }
                    self.graph.application(scheme, mapped, self.control)?
                }
                RowTerm::Union(children) => {
                    let mut mapped = Vec::new();
                    for child in children {
                        self.charge(DecisionWork::Node)?;
                        mapped.push(self.read_row(child)?);
                    }
                    self.graph.union(mapped, self.control)?
                }
                RowTerm::Select { choice, yes, no } => {
                    let yes = self.read_row(yes)?;
                    let no = self.read_row(no)?;
                    self.graph.select_row(choice, yes, no, self.control)?
                }
                RowTerm::Substitution { formula, arguments } => {
                    let mut mapped = BTreeMap::new();
                    for (parameter, argument) in arguments {
                        let argument = self.read_row(argument)?;
                        self.charge(DecisionWork::Node)?;
                        mapped.insert(parameter.clone(), argument);
                    }
                    let mapped = self.bindings(formula.variables(), mapped)?;
                    formula
                        .encode(&mut EffectEqualityControl(self.control))
                        .map_err(PreparedEffectFailure::Control)?;
                    self.graph
                        .apply_formula(formula.clone(), mapped, self.control)?
                }
            };
            self.charge(DecisionWork::Node)?;
            self.rows.insert(id, mapped);
        }
        self.read_row(root)
    }

    fn read_predicate(
        &mut self,
        predicate: &PreparedEffectPredicate,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        match predicate {
            PreparedEffectPredicate::Canonical(predicate) => {
                let mapped = self.bindings(predicate.variables(), BTreeMap::new())?;
                predicate
                    .encode(&mut EffectEqualityControl(self.control))
                    .map_err(PreparedEffectFailure::Control)?;
                if mapped.is_empty() {
                    Ok(PreparedEffectPredicate::Canonical(predicate.clone()))
                } else {
                    self.graph
                        .apply_predicate(predicate.clone(), mapped, self.control)
                }
            }
            PreparedEffectPredicate::Term(reference) => {
                self.graph.predicate(reference)?;
                self.charge(DecisionWork::Visit)?;
                Ok(self
                    .predicates
                    .get(&reference.0.ordinal)
                    .ok_or(PreparedEffectInvariant::StaleReference)?
                    .clone())
            }
        }
    }

    fn predicate(
        &mut self,
        root: &PreparedEffectPredicate,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        self.charge(DecisionWork::Node)?;
        let mut todo = vec![root];
        let mut reachable = BTreeSet::new();
        while let Some(predicate) = todo.pop() {
            self.control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            if let PreparedEffectPredicate::Term(reference) = predicate {
                let term = self.graph.predicate(reference)?;
                if !self.predicates.contains_key(&reference.0.ordinal)
                    && reachable.insert(reference.0.ordinal)
                {
                    match term {
                        PredicateTerm::And(children) | PredicateTerm::Or(children) => {
                            for child in children {
                                self.control
                                    .charge(DecisionWork::Node)
                                    .map_err(PreparedEffectFailure::Control)?;
                                todo.push(child);
                            }
                        }
                        PredicateTerm::Not(child) => {
                            self.control
                                .charge(DecisionWork::Node)
                                .map_err(PreparedEffectFailure::Control)?;
                            todo.push(child);
                        }
                        PredicateTerm::Select { yes, no, .. } => {
                            self.control
                                .charge(DecisionWork::Node)
                                .map_err(PreparedEffectFailure::Control)?;
                            todo.push(yes);
                            self.control
                                .charge(DecisionWork::Node)
                                .map_err(PreparedEffectFailure::Control)?;
                            todo.push(no);
                        }
                        PredicateTerm::Relation { .. } | PredicateTerm::Substitution { .. } => {}
                    }
                }
            }
        }
        for id in reachable {
            self.charge(DecisionWork::Visit)?;
            let term = Arc::clone(&self.graph.predicates[&id]);
            let mapped = match term.as_ref() {
                PredicateTerm::Relation {
                    relation,
                    actual,
                    permitted,
                } => {
                    let actual = self.row(actual)?;
                    let permitted = self.row(permitted)?;
                    self.graph
                        .relate(*relation, actual, permitted, self.control)?
                }
                PredicateTerm::And(children) | PredicateTerm::Or(children) => {
                    let mut mapped = Vec::new();
                    for child in children {
                        self.charge(DecisionWork::Node)?;
                        mapped.push(self.read_predicate(child)?);
                    }
                    if matches!(term.as_ref(), PredicateTerm::And(_)) {
                        self.graph.and(mapped, self.control)?
                    } else {
                        self.graph.or(mapped, self.control)?
                    }
                }
                PredicateTerm::Not(child) => {
                    let mapped = self.read_predicate(child)?;
                    self.graph.not(mapped, self.control)?
                }
                PredicateTerm::Select { choice, yes, no } => {
                    let yes = self.read_predicate(yes)?;
                    let no = self.read_predicate(no)?;
                    self.graph.select_predicate(choice, yes, no, self.control)?
                }
                PredicateTerm::Substitution {
                    predicate,
                    arguments,
                } => {
                    let mut mapped = BTreeMap::new();
                    for (parameter, argument) in arguments {
                        let argument = self.row(argument)?;
                        self.charge(DecisionWork::Node)?;
                        mapped.insert(parameter.clone(), argument);
                    }
                    let mapped = self.bindings(predicate.variables(), mapped)?;
                    predicate
                        .encode(&mut EffectEqualityControl(self.control))
                        .map_err(PreparedEffectFailure::Control)?;
                    self.graph
                        .apply_predicate(predicate.clone(), mapped, self.control)?
                }
            };
            self.charge(DecisionWork::Node)?;
            self.predicates.insert(id, mapped);
        }
        self.read_predicate(root)
    }
}

impl PreparedEffectGraph {
    /// Bounded simultaneous substitution, including unfinished applications.
    /// On failure, newly built terms are removed without reusing their IDs.
    pub(crate) fn substitute<C: DecisionControl>(
        &mut self,
        row: &PreparedEffectRow,
        arguments: &BTreeMap<GenericEffectReference, PreparedEffectRow>,
        control: &mut C,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        let row_limit = self.next_row;
        let result = Substitution {
            graph: self,
            arguments,
            control,
            rows: BTreeMap::new(),
            predicates: BTreeMap::new(),
        }
        .row(row);
        if result.is_err() {
            self.rows.retain(|id, _| *id < row_limit);
        }
        result
    }

    pub(crate) fn substitute_predicate<C: DecisionControl>(
        &mut self,
        predicate: &PreparedEffectPredicate,
        arguments: &BTreeMap<GenericEffectReference, PreparedEffectRow>,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        let row_limit = self.next_row;
        let predicate_limit = self.next_predicate;
        let result = Substitution {
            graph: self,
            arguments,
            control,
            rows: BTreeMap::new(),
            predicates: BTreeMap::new(),
        }
        .predicate(predicate);
        if result.is_err() {
            self.rows.retain(|id, _| *id < row_limit);
            self.predicates.retain(|id, _| *id < predicate_limit);
        }
        result
    }
}
