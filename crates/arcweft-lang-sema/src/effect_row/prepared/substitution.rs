//! Capture-free simultaneous substitution through the one prepared term grammar.
//!
//! The owner rewrites immutable terms, never declaration bodies or source. A
//! replacement belongs to the destination scope and is not substituted again.

use super::*;
use crate::effect_row::EffectEqualityControl;

struct Substitution<'a, C, P> {
    graph: &'a mut PreparedEffectGraph,
    project: &'a mut P,
    control: &'a mut C,
    parameters: BTreeMap<GenericEffectReference, Option<PreparedEffectRow>>,
    rows: BTreeMap<u64, PreparedEffectRow>,
    predicates: BTreeMap<u64, PreparedEffectPredicate>,
}

fn copy_annotation<C: DecisionControl>(
    row: &PreparedEffectRow,
    control: &mut C,
) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
    match row {
        PreparedEffectRow::Canonical(row) => row
            .encode(&mut EffectEqualityControl(control))
            .map_err(PreparedEffectFailure::Control)?,
        PreparedEffectRow::Term(_) => control
            .charge(DecisionWork::Visit)
            .map_err(PreparedEffectFailure::Control)?,
    }
    Ok(row.clone())
}

impl<C, P> Substitution<'_, C, P>
where
    C: DecisionControl,
    P: FnMut(
        &GenericEffectReference,
        &mut C,
    ) -> Result<Option<PreparedEffectRow>, PreparedEffectFailure<C::Error>>,
{
    fn charge(&mut self, work: DecisionWork) -> Result<(), PreparedEffectFailure<C::Error>> {
        self.control
            .charge(work)
            .map_err(PreparedEffectFailure::Control)
    }

    fn copy_row(
        &mut self,
        row: &PreparedEffectRow,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        if let PreparedEffectRow::Term(reference) = row {
            self.graph.row(reference)?;
        }
        copy_annotation(row, self.control)
    }

    fn bindings<'a>(
        &mut self,
        variables: impl Iterator<Item = &'a GenericEffectReference>,
        mut mapped: BTreeMap<GenericEffectReference, PreparedEffectRow>,
    ) -> Result<BTreeMap<GenericEffectReference, PreparedEffectRow>, PreparedEffectFailure<C::Error>>
    {
        for variable in variables {
            self.charge(DecisionWork::Visit)?;
            if mapped.contains_key(variable) {
                continue;
            }
            if !self.parameters.contains_key(variable) {
                self.charge(DecisionWork::Node)?;
                let replacement = (self.project)(variable, self.control)?;
                if let Some(row) = &replacement {
                    self.graph.validate_row(row)?;
                }
                self.parameters.insert(variable.clone(), replacement);
            }
            if let Some(Some(replacement)) = self.parameters.get(variable) {
                let replacement = copy_annotation(replacement, self.control)?;
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
                    row.encode(&mut EffectEqualityControl(self.control))
                        .map_err(PreparedEffectFailure::Control)?;
                    return Ok(PreparedEffectRow::Canonical(row.clone()));
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
                RowTerm::Application {
                    function,
                    arguments,
                } => {
                    let mut mapped = Vec::new();
                    for argument in arguments {
                        self.charge(DecisionWork::Node)?;
                        mapped.push(self.read_row(argument)?);
                    }
                    self.graph
                        .apply_function(function.clone(), mapped, self.control)?
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

/// An affine edit of terms only. Scheme/body state cannot change through this
/// borrow. Dropping an uncommitted edit removes every term it issued, including
/// earlier rows of a later-failed whole-type projection.
pub(crate) struct PreparedEffectTermEdit<'a> {
    graph: &'a mut PreparedEffectGraph,
    row_limit: u64,
    predicate_limit: u64,
    journal_start: usize,
    original_state: super::transaction::PreparedEffectStateId,
    state: TermEditState,
}

enum TermEditState {
    Open,
    Failed,
    Committed,
}

impl Drop for PreparedEffectTermEdit<'_> {
    fn drop(&mut self) {
        if !matches!(self.state, TermEditState::Committed) {
            while self
                .graph
                .rows
                .last_key_value()
                .is_some_and(|(id, _)| *id >= self.row_limit)
            {
                self.graph.rows.pop_last();
            }
            while self
                .graph
                .predicates
                .last_key_value()
                .is_some_and(|(id, _)| *id >= self.predicate_limit)
            {
                self.graph.predicates.pop_last();
            }
            self.graph.insertions.truncate(self.journal_start);
            self.graph.state = self.original_state;
        }
    }
}

impl PreparedEffectTermEdit<'_> {
    pub(crate) fn commit(mut self) -> Result<(), PreparedEffectInvariant> {
        if !matches!(self.state, TermEditState::Open) {
            return Err(PreparedEffectInvariant::TermEditState);
        }
        self.state = TermEditState::Committed;
        Ok(())
    }

    /// The projection owns admission of any new replacement value. This edit
    /// admits the parameter request first, then validates and memoizes it.
    pub(crate) fn map_row_parameters<C, P>(
        &mut self,
        row: &PreparedEffectRow,
        control: &mut C,
        project: &mut P,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>>
    where
        C: DecisionControl,
        P: FnMut(
            &GenericEffectReference,
            &mut C,
        ) -> Result<Option<PreparedEffectRow>, PreparedEffectFailure<C::Error>>,
    {
        if !matches!(self.state, TermEditState::Open) {
            return Err(PreparedEffectInvariant::TermEditState.into());
        }
        let result = Substitution {
            graph: self.graph,
            project,
            control,
            parameters: BTreeMap::new(),
            rows: BTreeMap::new(),
            predicates: BTreeMap::new(),
        }
        .row(row);
        if result.is_err() {
            self.state = TermEditState::Failed;
        }
        result
    }

    pub(crate) fn map_predicate_parameters<C, P>(
        &mut self,
        predicate: &PreparedEffectPredicate,
        control: &mut C,
        project: &mut P,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>>
    where
        C: DecisionControl,
        P: FnMut(
            &GenericEffectReference,
            &mut C,
        ) -> Result<Option<PreparedEffectRow>, PreparedEffectFailure<C::Error>>,
    {
        if !matches!(self.state, TermEditState::Open) {
            return Err(PreparedEffectInvariant::TermEditState.into());
        }
        let result = Substitution {
            graph: self.graph,
            project,
            control,
            parameters: BTreeMap::new(),
            rows: BTreeMap::new(),
            predicates: BTreeMap::new(),
        }
        .predicate(predicate);
        if result.is_err() {
            self.state = TermEditState::Failed;
        }
        result
    }
}

impl PreparedEffectGraph {
    pub(crate) fn edit_terms(&mut self) -> PreparedEffectTermEdit<'_> {
        PreparedEffectTermEdit {
            row_limit: self.next_row,
            predicate_limit: self.next_predicate,
            journal_start: self.insertions.len(),
            original_state: self.state,
            graph: self,
            state: TermEditState::Open,
        }
    }

    /// Bounded simultaneous substitution, including unfinished applications.
    pub(crate) fn substitute<C: DecisionControl>(
        &mut self,
        row: &PreparedEffectRow,
        arguments: &BTreeMap<GenericEffectReference, PreparedEffectRow>,
        control: &mut C,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        let mut edit = self.edit_terms();
        let result = edit.map_row_parameters(row, control, &mut |parameter, control| {
            arguments
                .get(parameter)
                .map(|row| copy_annotation(row, control))
                .transpose()
        })?;
        edit.commit()?;
        Ok(result)
    }

    pub(crate) fn substitute_predicate<C: DecisionControl>(
        &mut self,
        predicate: &PreparedEffectPredicate,
        arguments: &BTreeMap<GenericEffectReference, PreparedEffectRow>,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        let mut edit = self.edit_terms();
        let result =
            edit.map_predicate_parameters(predicate, control, &mut |parameter, control| {
                arguments
                    .get(parameter)
                    .map(|row| copy_annotation(row, control))
                    .transpose()
            })?;
        edit.commit()?;
        Ok(result)
    }
}
