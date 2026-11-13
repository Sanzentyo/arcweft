//! Iterative lowering of the retained relation into whole-row scalar clauses.

use super::*;

struct Row {
    formula: Formula,
    admitted: bool,
}

pub(super) struct Evaluator<'s, 'w, 'c, C: DecisionControl> {
    source: &'s ScopedEffectCompletion,
    work: &'w mut Work<'c, C>,
    arena: ScalarArena<Variable>,
    rows: BTreeMap<(ResidualRowScope, u64), Row>,
    predicates: BTreeMap<(ResidualRowScope, u64), ScalarId>,
}

impl<'s, 'w, 'c, C: DecisionControl> Evaluator<'s, 'w, 'c, C> {
    pub(super) fn new(
        source: &'s ScopedEffectCompletion,
        work: &'w mut Work<'c, C>,
    ) -> Result<Self, ReductionFailure<C::Error>> {
        Ok(Self {
            source,
            arena: ScalarArena::new(work)?,
            work,
            rows: BTreeMap::new(),
            predicates: BTreeMap::new(),
        })
    }

    fn check_variables(
        &mut self,
        variables: impl Iterator<Item = Variable>,
        scope: ResidualRowScope,
    ) -> Result<(), ReductionFailure<C::Error>> {
        for variable in variables {
            self.work.charge(DecisionWork::Visit)?;
            if let Variable::Substitution(reference) = variable {
                if scope.parameters(&self.source.graph).contains(&reference) {
                    return Err(ReductionFailure::Retain);
                }
                return Err(PreparedEffectInvariant::UndeclaredParameter(reference).into());
            }
        }
        Ok(())
    }

    fn formula(
        &mut self,
        value: &EffectFormula<GenericEffectReference>,
        arguments: &BTreeMap<Variable, Formula>,
        scope: ResidualRowScope,
    ) -> Result<Formula, ReductionFailure<C::Error>> {
        let value = value
            .map_references(self.work, &mut |reference, _| {
                Ok(Variable::Substitution(reference.clone()))
            })?
            .substitute(arguments, self.work)?;
        self.check_variables(value.variables().cloned(), scope)?;
        Ok(value)
    }

    fn canonical_predicate(
        &mut self,
        value: &EffectPredicate,
        arguments: &BTreeMap<Variable, Formula>,
        scope: ResidualRowScope,
    ) -> Result<Predicate, ReductionFailure<C::Error>> {
        let value = value
            .map_references(self.work, &mut |reference, _| {
                Ok(Variable::Substitution(reference.clone()))
            })?
            .substitute(arguments, self.work)?;
        self.check_variables(value.variables().cloned(), scope)?;
        Ok(value)
    }

    fn function(&mut self, id: u64) -> Result<Row, ReductionFailure<C::Error>> {
        let function = &self.source.graph.functions[&id];
        let admitted = match function.as_ref() {
            Function::Scheme(scheme) => {
                let domain = self.canonical_predicate(
                    &scheme.header.domain,
                    &BTreeMap::new(),
                    ResidualRowScope::Scheme(id),
                )?;
                if domain.is_unconstrained() {
                    true
                } else if domain.is_impossible() {
                    false
                } else {
                    return Err(ReductionFailure::Retain);
                }
            }
            Function::Unknown { .. } => true,
            Function::Imported(_) => return Err(ReductionFailure::Retain),
        };
        let formula = if let Some(row) = self.source.fixed.get(&id) {
            self.formula(row.known()?, &BTreeMap::new(), ResidualRowScope::Scheme(id))?
        } else {
            Formula::variable(Variable::Function(id), self.work)?
        };
        Ok(Row { formula, admitted })
    }

    fn arguments(
        &mut self,
        arguments: &BTreeMap<GenericEffectReference, PreparedEffectRow>,
        scope: ResidualRowScope,
    ) -> Result<(BTreeMap<Variable, Formula>, bool), ReductionFailure<C::Error>> {
        let mut values = BTreeMap::new();
        let mut admitted = true;
        for (reference, argument) in arguments {
            let value = self.row(argument, scope)?;
            admitted &= value.admitted;
            self.work.charge(DecisionWork::Node)?;
            values.insert(Variable::Substitution(reference.clone()), value.formula);
        }
        Ok((values, admitted))
    }

    fn read_row(
        &mut self,
        value: &PreparedEffectRow,
        scope: ResidualRowScope,
    ) -> Result<Row, ReductionFailure<C::Error>> {
        match value {
            PreparedEffectRow::Canonical(value) => Ok(Row {
                formula: self.formula(value.known()?, &BTreeMap::new(), scope)?,
                admitted: true,
            }),
            PreparedEffectRow::Term(reference) => {
                let value = &self.rows[&(scope, reference.0.ordinal)];
                Ok(Row {
                    formula: value
                        .formula
                        .map_references(self.work, &mut |variable, _| Ok(variable.clone()))?,
                    admitted: value.admitted,
                })
            }
        }
    }

    fn row(
        &mut self,
        value: &PreparedEffectRow,
        scope: ResidualRowScope,
    ) -> Result<Row, ReductionFailure<C::Error>> {
        let mut todo = vec![value];
        let mut retained = BTreeSet::new();
        while let Some(value) = todo.pop() {
            self.work.charge(DecisionWork::Visit)?;
            if let PreparedEffectRow::Term(reference) = value {
                let node = self.source.graph.row(reference)?;
                if self.rows.contains_key(&(scope, reference.0.ordinal))
                    || !retained.insert(reference.0.ordinal)
                {
                    continue;
                }
                for child in node.children() {
                    self.work.charge(DecisionWork::Node)?;
                    todo.push(child);
                }
            }
        }
        for id in retained {
            let value = match self.source.graph.rows[&id].as_ref() {
                RowTerm::Application {
                    function,
                    arguments,
                } => {
                    let declared = self.source.graph.function(function)?;
                    if arguments.len() != declared.parameters().len() {
                        return Err(PreparedEffectInvariant::ApplicationArity.into());
                    }
                    let mut admitted = true;
                    for argument in arguments {
                        admitted &= self.read_row(argument, scope)?.admitted;
                    }
                    let mut value = self.function(function.reference().ordinal)?;
                    value.admitted &= admitted;
                    value
                }
                RowTerm::Substitution { formula, arguments } => {
                    let (arguments, admitted) = self.arguments(arguments, scope)?;
                    Row {
                        formula: self.formula(formula, &arguments, scope)?,
                        admitted,
                    }
                }
                RowTerm::Union(children) => {
                    let mut formula = Formula::empty();
                    let mut admitted = true;
                    for child in children {
                        let child = self.read_row(child, scope)?;
                        formula = formula.union(&child.formula, self.work)?;
                        admitted &= child.admitted;
                    }
                    Row { formula, admitted }
                }
                RowTerm::Select { .. } => return Err(ReductionFailure::Retain),
            };
            self.work.charge(DecisionWork::Node)?;
            self.rows.insert((scope, id), value);
        }
        self.read_row(value, scope)
    }

    fn read_predicate(
        &mut self,
        value: &PreparedEffectPredicate,
        scope: ResidualRowScope,
    ) -> Result<ScalarId, ReductionFailure<C::Error>> {
        match value {
            PreparedEffectPredicate::Canonical(value) => {
                let value = self.canonical_predicate(value, &BTreeMap::new(), scope)?;
                Ok(self.arena.pointwise(value, self.work)?)
            }
            PreparedEffectPredicate::Term(reference) => {
                Ok(self.predicates[&(scope, reference.0.ordinal)])
            }
        }
    }

    fn predicate(
        &mut self,
        value: &PreparedEffectPredicate,
        scope: ResidualRowScope,
    ) -> Result<ScalarId, ReductionFailure<C::Error>> {
        let mut todo = vec![value];
        let mut retained = BTreeSet::new();
        while let Some(value) = todo.pop() {
            self.work.charge(DecisionWork::Visit)?;
            if let PreparedEffectPredicate::Term(reference) = value {
                let node = self.source.graph.predicate(reference)?;
                if self.predicates.contains_key(&(scope, reference.0.ordinal))
                    || !retained.insert(reference.0.ordinal)
                {
                    continue;
                }
                match node {
                    PredicateTerm::And(children) | PredicateTerm::Or(children) => {
                        for child in children {
                            self.work.charge(DecisionWork::Node)?;
                            todo.push(child);
                        }
                    }
                    PredicateTerm::Not(child) => {
                        self.work.charge(DecisionWork::Node)?;
                        todo.push(child);
                    }
                    PredicateTerm::Select { .. } => return Err(ReductionFailure::Retain),
                    _ => {}
                }
            }
        }
        for id in retained {
            let value = match self.source.graph.predicates[&id].as_ref() {
                PredicateTerm::Substitution {
                    predicate,
                    arguments,
                } => {
                    let (arguments, admitted) = self.arguments(arguments, scope)?;
                    let value = self.canonical_predicate(predicate, &arguments, scope)?;
                    if admitted {
                        self.arena.pointwise(value, self.work)?
                    } else {
                        ScalarId(1)
                    }
                }
                PredicateTerm::Relation {
                    relation,
                    actual,
                    permitted,
                } => {
                    let actual = self.row(actual, scope)?;
                    let permitted = self.row(permitted, scope)?;
                    if !actual.admitted || !permitted.admitted {
                        ScalarId(1)
                    } else {
                        match relation {
                            PreparedEffectRelation::Equal => {
                                let value =
                                    actual.formula.subset(&permitted.formula, self.work)?.and(
                                        &permitted.formula.subset(&actual.formula, self.work)?,
                                        self.work,
                                    )?;
                                self.arena.pointwise(value, self.work)?
                            }
                            PreparedEffectRelation::AdmittedSubset => {
                                self.arena
                                    .coverage(actual.formula, permitted.formula, self.work)?
                            }
                        }
                    }
                }
                PredicateTerm::And(children) | PredicateTerm::Or(children) => {
                    let mut values = Vec::new();
                    for child in children {
                        let value = self.read_predicate(child, scope)?;
                        self.work.charge(DecisionWork::Node)?;
                        values.push(value);
                    }
                    if matches!(
                        self.source.graph.predicates[&id].as_ref(),
                        PredicateTerm::And(_)
                    ) {
                        self.arena.conjunction(values, self.work)?
                    } else {
                        self.arena.disjunction(values, self.work)?
                    }
                }
                PredicateTerm::Not(child) => {
                    let child = self.read_predicate(child, scope)?;
                    self.arena.negate(child, self.work)?
                }
                PredicateTerm::Select { .. } => return Err(ReductionFailure::Retain),
            };
            self.work.charge(DecisionWork::Node)?;
            self.predicates.insert((scope, id), value);
        }
        self.read_predicate(value, scope)
    }

    pub(super) fn relation(
        mut self,
    ) -> Result<(ScalarArena<Variable>, ScalarId), ReductionFailure<C::Error>> {
        let mut values = Vec::new();
        for node in self.source.program.relation_nodes() {
            self.work.charge(DecisionWork::Visit)?;
            let value = match node {
                ResidualNode::True | ResidualNode::FiniteFunction(_) => ScalarId(0),
                ResidualNode::And(children) => self
                    .arena
                    .conjunction(children.iter().map(|id| values[id.index()]), self.work)?,
                ResidualNode::Not(child) => self.arena.negate(values[child.index()], self.work)?,
                ResidualNode::Implies {
                    domain,
                    consequence,
                } => self.arena.implies(
                    values[domain.index()],
                    values[consequence.index()],
                    self.work,
                )?,
                ResidualNode::ForAllFiniteRows { body, .. } => values[body.index()],
                ResidualNode::SourcePredicate(source) => match source.value(&self.source.graph)? {
                    ResidualSourcePredicate::Canonical(value) => {
                        let value =
                            self.canonical_predicate(value, &BTreeMap::new(), source.scope())?;
                        self.arena.pointwise(value, self.work)?
                    }
                    ResidualSourcePredicate::Prepared(value) => {
                        self.predicate(value, source.scope())?
                    }
                },
                ResidualNode::BodyEquation { scheme } => {
                    let body = self.row(
                        source_body(&self.source.graph, *scheme)?,
                        ResidualRowScope::Scheme(*scheme),
                    )?;
                    let function = self.function(*scheme)?;
                    let value = function.formula.subset(&body.formula, self.work)?.and(
                        &body.formula.subset(&function.formula, self.work)?,
                        self.work,
                    )?;
                    self.arena.pointwise(value, self.work)?
                }
                ResidualNode::BodyAdmitted { scheme } => ScalarArena::<Variable>::boolean(
                    self.row(
                        source_body(&self.source.graph, *scheme)?,
                        ResidualRowScope::Scheme(*scheme),
                    )?
                    .admitted,
                ),
                _ => return Err(PreparedEffectInvariant::UnclosedCell.into()),
            };
            self.work.charge(DecisionWork::Node)?;
            values.push(value);
        }
        Ok((self.arena, values[self.source.program.relation.index()]))
    }
}
