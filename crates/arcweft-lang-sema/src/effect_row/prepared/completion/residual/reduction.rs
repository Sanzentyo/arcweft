//! Exact nullary row elimination and whole-row residual scalar conditions.

use super::*;
use arcweft_core::effect_row::{
    EffectDecision, EffectFormula as SetFormula, EffectMembershipView,
    EffectPredicate as SetPredicate,
};
use std::cmp::Ordering;

mod evaluation;
use evaluation::Evaluator;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Variable {
    Function(u64),
    Substitution(GenericEffectReference),
}
type Formula = SetFormula<Variable>;
type Predicate = SetPredicate<Variable>;
type Decision = EffectDecision<Variable>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResidualFunctionVariable(PreparedEffectReference);
impl Ord for ResidualFunctionVariable {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.ordinal.cmp(&other.0.ordinal).then_with(|| {
            // Each admitted normal form contains only its original issuer.
            // Foreign operational references still need a lawful total order;
            // allocation identity never orders values within one program.
            (Arc::as_ptr(&self.0.issuer) as usize).cmp(&(Arc::as_ptr(&other.0.issuer) as usize))
        })
    }
}
impl PartialOrd for ResidualFunctionVariable {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

enum ReductionFailure<E> {
    Retain,
    Failure(PreparedEffectFailure<E>),
}
impl<E> From<PreparedEffectFailure<E>> for ReductionFailure<E> {
    fn from(value: PreparedEffectFailure<E>) -> Self {
        Self::Failure(value)
    }
}
impl<E> From<PreparedEffectInvariant> for ReductionFailure<E> {
    fn from(value: PreparedEffectInvariant) -> Self {
        Self::Failure(value.into())
    }
}
impl<E> From<EffectRowError> for ReductionFailure<E> {
    fn from(value: EffectRowError) -> Self {
        Self::Failure(value.into())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ScalarId(usize);
enum ScalarNode<V> {
    True,
    False,
    Pointwise(SetPredicate<V>),
    Coverage {
        actual: SetFormula<V>,
        permitted: SetFormula<V>,
    },
    And(Box<[ScalarId]>),
    Or(Box<[ScalarId]>),
    Not(ScalarId),
    Implies {
        domain: ScalarId,
        consequence: ScalarId,
    },
}
struct ScalarArena<V> {
    nodes: Vec<ScalarNode<V>>,
}

impl<V: Clone + Ord> ScalarArena<V> {
    fn new<C: DecisionControl>(
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        work.charge(DecisionWork::Node)?;
        work.charge(DecisionWork::Node)?;
        Ok(Self {
            nodes: vec![ScalarNode::True, ScalarNode::False],
        })
    }
    fn node<C: DecisionControl>(
        &mut self,
        value: ScalarNode<V>,
        work: &mut Work<'_, C>,
    ) -> Result<ScalarId, PreparedEffectFailure<C::Error>> {
        work.charge(DecisionWork::Node)?;
        let id = ScalarId(self.nodes.len());
        self.nodes.push(value);
        Ok(id)
    }
    fn boolean(value: bool) -> ScalarId {
        ScalarId(usize::from(!value))
    }
    fn pointwise<C: DecisionControl>(
        &mut self,
        value: SetPredicate<V>,
        work: &mut Work<'_, C>,
    ) -> Result<ScalarId, PreparedEffectFailure<C::Error>> {
        if value.is_unconstrained() {
            return Ok(ScalarId(0));
        }
        if value.is_impossible() {
            return Ok(ScalarId(1));
        }
        self.node(ScalarNode::Pointwise(value), work)
    }
    fn conjunction<C: DecisionControl>(
        &mut self,
        values: impl IntoIterator<Item = ScalarId>,
        work: &mut Work<'_, C>,
    ) -> Result<ScalarId, PreparedEffectFailure<C::Error>> {
        let mut result = Vec::new();
        for value in values {
            work.charge(DecisionWork::Visit)?;
            if value.0 == 1 {
                return Ok(value);
            }
            if value.0 == 0 {
                continue;
            }
            if let ScalarNode::And(children) = &self.nodes[value.0] {
                for child in children {
                    work.charge(DecisionWork::Node)?;
                    result.push(*child);
                }
            } else {
                work.charge(DecisionWork::Node)?;
                result.push(value);
            }
        }
        match result.as_slice() {
            [] => Ok(ScalarId(0)),
            [one] => Ok(*one),
            _ => self.node(ScalarNode::And(result.into_boxed_slice()), work),
        }
    }
    fn disjunction<C: DecisionControl>(
        &mut self,
        values: impl IntoIterator<Item = ScalarId>,
        work: &mut Work<'_, C>,
    ) -> Result<ScalarId, PreparedEffectFailure<C::Error>> {
        let mut result = Vec::new();
        for value in values {
            work.charge(DecisionWork::Visit)?;
            if value.0 == 0 {
                return Ok(value);
            }
            if value.0 == 1 {
                continue;
            }
            if let ScalarNode::Or(children) = &self.nodes[value.0] {
                for child in children {
                    work.charge(DecisionWork::Node)?;
                    result.push(*child);
                }
            } else {
                work.charge(DecisionWork::Node)?;
                result.push(value);
            }
        }
        match result.as_slice() {
            [] => Ok(ScalarId(1)),
            [one] => Ok(*one),
            _ => self.node(ScalarNode::Or(result.into_boxed_slice()), work),
        }
    }
    fn negate<C: DecisionControl>(
        &mut self,
        value: ScalarId,
        work: &mut Work<'_, C>,
    ) -> Result<ScalarId, PreparedEffectFailure<C::Error>> {
        match &self.nodes[value.0] {
            ScalarNode::True => Ok(ScalarId(1)),
            ScalarNode::False => Ok(ScalarId(0)),
            ScalarNode::Not(inner) => Ok(*inner),
            _ => self.node(ScalarNode::Not(value), work),
        }
    }
    fn implies<C: DecisionControl>(
        &mut self,
        domain: ScalarId,
        consequence: ScalarId,
        work: &mut Work<'_, C>,
    ) -> Result<ScalarId, PreparedEffectFailure<C::Error>> {
        if domain.0 == 1 || consequence.0 == 0 || domain == consequence {
            return Ok(ScalarId(0));
        }
        if domain.0 == 0 {
            return Ok(consequence);
        }
        if consequence.0 == 1 {
            return self.negate(domain, work);
        }
        self.node(
            ScalarNode::Implies {
                domain,
                consequence,
            },
            work,
        )
    }
    fn coverage<C: DecisionControl>(
        &mut self,
        actual: SetFormula<V>,
        permitted: SetFormula<V>,
        work: &mut Work<'_, C>,
    ) -> Result<ScalarId, PreparedEffectFailure<C::Error>> {
        if actual.subset(&permitted, work)?.is_unconstrained() {
            return Ok(ScalarId(0));
        }
        if let (Some(actual), Some(permitted)) = (actual.closed(work)?, permitted.closed(work)?) {
            for required in actual.iter() {
                let mut covered = false;
                for grant in permitted.iter() {
                    work.charge(DecisionWork::Visit)?;
                    if grant.covers(required) {
                        covered = true;
                        break;
                    }
                }
                if !covered {
                    return Ok(ScalarId(1));
                }
            }
            return Ok(ScalarId(0));
        }
        self.node(ScalarNode::Coverage { actual, permitted }, work)
    }
}

struct PointwiseMinimum {
    satisfiable: Predicate,
    principal: Predicate,
    lower: BTreeMap<Variable, Formula>,
    unique: bool,
}

fn decision_copy<C: DecisionControl>(
    value: &Decision,
    work: &mut Work<'_, C>,
) -> Result<Decision, PreparedEffectFailure<C::Error>> {
    value.map_references(work, &mut |reference, _| Ok(reference.clone()))
}

/// A universally interpreted predicate is the assertion that its finite row of
/// counterexamples is empty. If its default fails at all-zero inputs, infinitely
/// many labels violate it, so the whole predicate is false.
fn predicate_from_decisions<C: DecisionControl>(
    mut values: Vec<Decision>,
    labels: &[EffectId],
    work: &mut Work<'_, C>,
) -> Result<Predicate, PreparedEffectFailure<C::Error>> {
    let default = values.remove(0);
    if !default.evaluate_with_control(|_| false, work)? {
        return Ok(Predicate::impossible());
    }
    let mut overrides = BTreeMap::new();
    for (label, value) in labels.iter().zip(values) {
        work.charge(DecisionWork::Node)?;
        overrides.insert(label.clone(), value.not(work)?);
    }
    let bad = Formula::try_from_decisions(default.not(work)?, overrides, work)?;
    bad.subset(&Formula::empty(), work)
}

fn minimize_pointwise<C: DecisionControl>(
    source: &ScopedEffectCompletion,
    relation: &Predicate,
    work: &mut Work<'_, C>,
) -> Result<PointwiseMinimum, PreparedEffectFailure<C::Error>> {
    let mut locals = BTreeSet::new();
    let mut free = Vec::new();
    for (&id, function) in &source.graph.functions {
        work.charge(DecisionWork::Node)?;
        if function.is_future() {
            if !source.fixed.contains_key(&id) {
                free.push(Variable::Function(id));
            }
        } else {
            locals.insert(Variable::Function(id));
        }
    }
    let mut labels = Vec::new();
    for label in &source.support {
        work.charge(DecisionWork::Node)?;
        labels.push(label.clone());
    }
    let mut rows: BTreeMap<Variable, Vec<Decision>> = BTreeMap::new();
    for local in &locals {
        work.charge(DecisionWork::Node)?;
        rows.insert(local.clone(), Vec::new());
    }
    let mut satisfiable = Vec::new();
    let mut principal = Vec::new();
    let mut unique = true;
    for class in 0..=labels.len() {
        let mut predicate = decision_copy(
            if class == 0 {
                relation.membership().default_decision()
            } else {
                relation.membership().at(&labels[class - 1])
            },
            work,
        )?;
        if class == 0 {
            let mut supported = Decision::constant(false);
            for future in &free {
                supported = supported.or(&Decision::variable(future.clone(), work)?, work)?;
            }
            for local in &locals {
                predicate = predicate.and(
                    &Decision::variable(local.clone(), work)?.conditional(
                        &supported,
                        &Decision::constant(true),
                        work,
                    )?,
                    work,
                )?;
            }
        }
        let completed = predicate.complete_conditionally(&locals, work)?;
        let mut different = Decision::constant(false);
        for local in &locals {
            let lower = completed
                .lower_envelope()
                .get(local)
                .ok_or(PreparedEffectInvariant::UnclosedCell)?;
            let variable = Decision::variable(local.clone(), work)?;
            different =
                different.or(&variable.conditional(&lower.not(work)?, lower, work)?, work)?;
            work.charge(DecisionWork::Node)?;
            rows.get_mut(local)
                .expect("all local row functions were admitted")
                .push(decision_copy(lower, work)?);
        }
        unique &= predicate
            .and(&different, work)?
            .exists(&locals, work)?
            .is_constant(false);
        work.charge(DecisionWork::Node)?;
        satisfiable.push(decision_copy(completed.satisfiable(), work)?);
        work.charge(DecisionWork::Node)?;
        principal.push(decision_copy(completed.principal(), work)?);
    }
    let satisfiable = predicate_from_decisions(satisfiable, &labels, work)?;
    let principal = predicate_from_decisions(principal, &labels, work)?;
    let mut lower = BTreeMap::new();
    for (variable, mut values) in rows {
        let default = values.remove(0);
        let mut overrides = BTreeMap::new();
        for (label, value) in labels.iter().zip(values) {
            work.charge(DecisionWork::Node)?;
            overrides.insert(label.clone(), value);
        }
        let value = Formula::try_from_decisions(default, overrides, work)?;
        work.charge(DecisionWork::Node)?;
        lower.insert(variable, value);
    }
    Ok(PointwiseMinimum {
        satisfiable,
        principal,
        lower,
        unique,
    })
}

fn has_local<C: DecisionControl>(
    variables: impl Iterator<Item = Variable>,
    source: &ScopedEffectCompletion,
    work: &mut Work<'_, C>,
) -> Result<bool, PreparedEffectFailure<C::Error>> {
    let mut local = false;
    for variable in variables {
        work.charge(DecisionWork::Visit)?;
        match variable {
            Variable::Function(id) => local |= !source.graph.functions[&id].is_future(),
            Variable::Substitution(_) => return Err(PreparedEffectInvariant::UnclosedCell.into()),
        }
    }
    Ok(local)
}

impl ScalarArena<Variable> {
    fn local_flags<C: DecisionControl>(
        &self,
        source: &ScopedEffectCompletion,
        work: &mut Work<'_, C>,
    ) -> Result<Vec<bool>, PreparedEffectFailure<C::Error>> {
        let mut flags = Vec::new();
        for node in &self.nodes {
            let flag = match node {
                ScalarNode::True | ScalarNode::False => false,
                ScalarNode::Pointwise(value) => {
                    has_local(value.variables().cloned(), source, work)?
                }
                ScalarNode::Coverage { actual, permitted } => has_local(
                    actual.variables().chain(permitted.variables()).cloned(),
                    source,
                    work,
                )?,
                ScalarNode::And(values) | ScalarNode::Or(values) => {
                    values.iter().any(|id| flags[id.0])
                }
                ScalarNode::Not(id) => flags[id.0],
                ScalarNode::Implies {
                    domain,
                    consequence,
                } => flags[domain.0] || flags[consequence.0],
            };
            work.charge(DecisionWork::Node)?;
            flags.push(flag);
        }
        Ok(flags)
    }
    fn substitute<C: DecisionControl>(
        &self,
        root: ScalarId,
        values: &BTreeMap<Variable, Formula>,
        work: &mut Work<'_, C>,
    ) -> Result<(Self, ScalarId), PreparedEffectFailure<C::Error>> {
        let mut result = Self::new(work)?;
        let mut mapped: Vec<ScalarId> = Vec::new();
        for node in &self.nodes {
            let id = match node {
                ScalarNode::True => ScalarId(0),
                ScalarNode::False => ScalarId(1),
                ScalarNode::Pointwise(value) => {
                    result.pointwise(value.substitute(values, work)?, work)?
                }
                ScalarNode::Coverage { actual, permitted } => result.coverage(
                    actual.substitute(values, work)?,
                    permitted.substitute(values, work)?,
                    work,
                )?,
                ScalarNode::And(children) => {
                    result.conjunction(children.iter().map(|id| mapped[id.0]), work)?
                }
                ScalarNode::Or(children) => {
                    result.disjunction(children.iter().map(|id| mapped[id.0]), work)?
                }
                ScalarNode::Not(child) => result.negate(mapped[child.0], work)?,
                ScalarNode::Implies {
                    domain,
                    consequence,
                } => result.implies(mapped[domain.0], mapped[consequence.0], work)?,
            };
            work.charge(DecisionWork::Node)?;
            mapped.push(id);
        }
        Ok((result, mapped[root.0]))
    }
}

struct NormalForm {
    arena: ScalarArena<ResidualFunctionVariable>,
    satisfiable: ScalarId,
    principal: ScalarId,
    functions: BTreeMap<u64, SetFormula<ResidualFunctionVariable>>,
}

pub(crate) struct ReducedEffectCompletion<'a> {
    source: &'a ScopedEffectCompletion,
    normal: Option<NormalForm>,
}

impl ScopedEffectCompletion {
    pub(crate) fn reduce<C: DecisionControl>(
        &self,
        control: &mut C,
    ) -> Result<ReducedEffectCompletion<'_>, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        work.charge(DecisionWork::Visit)?;
        for function in self.graph.functions.values() {
            work.charge(DecisionWork::Visit)?;
            if !function.parameters().is_empty()
                || matches!(function.as_ref(), Function::Imported(_))
            {
                return Ok(ReducedEffectCompletion {
                    source: self,
                    normal: None,
                });
            }
        }
        if !self.graph.choices.is_empty() {
            return Ok(ReducedEffectCompletion {
                source: self,
                normal: None,
            });
        }
        let normal = match normalize(self, &mut work) {
            Ok(value) => Some(value),
            Err(ReductionFailure::Retain) => None,
            Err(ReductionFailure::Failure(error)) => return Err(error),
        };
        Ok(ReducedEffectCompletion {
            source: self,
            normal,
        })
    }
}

fn normalize<C: DecisionControl>(
    source: &ScopedEffectCompletion,
    work: &mut Work<'_, C>,
) -> Result<NormalForm, ReductionFailure<C::Error>> {
    let (mut arena, root) = Evaluator::new(source, work)?.relation()?;
    let flags = arena.local_flags(source, work)?;
    let mut todo = vec![root];
    let mut future = Vec::new();
    let mut other = Vec::new();
    let mut pointwise = Predicate::unconstrained();
    while let Some(id) = todo.pop() {
        work.charge(DecisionWork::Visit)?;
        if !flags[id.0] {
            work.charge(DecisionWork::Node)?;
            future.push(id);
            continue;
        }
        match &arena.nodes[id.0] {
            ScalarNode::And(children) => {
                for child in children {
                    work.charge(DecisionWork::Node)?;
                    todo.push(*child);
                }
            }
            ScalarNode::Pointwise(value) => pointwise = pointwise.and(value, work)?,
            _ => {
                work.charge(DecisionWork::Node)?;
                other.push(id);
            }
        }
    }
    let completed = minimize_pointwise(source, &pointwise, work)?;
    let (satisfiable, principal) = if other.is_empty() {
        let guard = arena.conjunction(future, work)?;
        let sat = arena.pointwise(completed.satisfiable, work)?;
        let principal = arena.pointwise(completed.principal, work)?;
        (
            arena.conjunction([guard, sat], work)?,
            arena.conjunction([guard, principal], work)?,
        )
    } else {
        if !completed.unique {
            return Err(ReductionFailure::Retain);
        }
        let (substituted, guard) = arena.substitute(root, &completed.lower, work)?;
        arena = substituted;
        if arena.local_flags(source, work)?[guard.0] {
            return Err(PreparedEffectInvariant::UnclosedCell.into());
        }
        (guard, guard)
    };
    let mut rows = completed.lower;
    for (&id, function) in &source.graph.functions {
        if function.is_future() {
            let value = if let Some(row) = source.fixed.get(&id) {
                row.known()?.map_references(work, &mut |reference, _| {
                    Err(PreparedEffectInvariant::UndeclaredParameter(reference.clone()).into())
                })?
            } else {
                Formula::variable(Variable::Function(id), work)?
            };
            work.charge(DecisionWork::Node)?;
            rows.insert(Variable::Function(id), value);
        }
    }
    let convert =
        |variable: &Variable| -> Result<ResidualFunctionVariable, PreparedEffectFailure<C::Error>> {
            let Variable::Function(id) = variable else {
                return Err(PreparedEffectInvariant::UnclosedCell.into());
            };
            if !source.graph.functions[id].is_future() || source.fixed.contains_key(id) {
                return Err(PreparedEffectInvariant::UnclosedCell.into());
            }
            Ok(ResidualFunctionVariable(source.graph.reference(*id)))
        };
    let (arena, satisfiable, principal) =
        convert_arena(&arena, satisfiable, principal, &convert, work)?;
    let mut functions = BTreeMap::new();
    for (variable, value) in rows {
        let Variable::Function(id) = variable else {
            return Err(PreparedEffectInvariant::UnclosedCell.into());
        };
        let value = value.map_references(work, &mut |variable, _| convert(variable))?;
        work.charge(DecisionWork::Node)?;
        functions.insert(id, value);
    }
    Ok(NormalForm {
        arena,
        satisfiable,
        principal,
        functions,
    })
}

fn convert_arena<C: DecisionControl>(
    source: &ScalarArena<Variable>,
    satisfiable: ScalarId,
    principal: ScalarId,
    convert: &impl Fn(&Variable) -> Result<ResidualFunctionVariable, PreparedEffectFailure<C::Error>>,
    work: &mut Work<'_, C>,
) -> Result<
    (ScalarArena<ResidualFunctionVariable>, ScalarId, ScalarId),
    PreparedEffectFailure<C::Error>,
> {
    let mut todo = vec![satisfiable, principal];
    let mut retained = BTreeSet::new();
    while let Some(id) = todo.pop() {
        work.charge(DecisionWork::Visit)?;
        if !retained.insert(id) {
            continue;
        }
        match &source.nodes[id.0] {
            ScalarNode::And(children) | ScalarNode::Or(children) => {
                for child in children {
                    work.charge(DecisionWork::Node)?;
                    todo.push(*child);
                }
            }
            ScalarNode::Not(child) => {
                work.charge(DecisionWork::Node)?;
                todo.push(*child);
            }
            ScalarNode::Implies {
                domain,
                consequence,
            } => {
                work.charge(DecisionWork::Node)?;
                todo.push(*domain);
                work.charge(DecisionWork::Node)?;
                todo.push(*consequence);
            }
            _ => {}
        }
    }
    let mut result = ScalarArena::new(work)?;
    let mut mapped = BTreeMap::new();
    for id in retained {
        let value = match &source.nodes[id.0] {
            ScalarNode::True => ScalarId(0),
            ScalarNode::False => ScalarId(1),
            ScalarNode::Pointwise(value) => result.pointwise(
                value.map_references(work, &mut |variable, _| convert(variable))?,
                work,
            )?,
            ScalarNode::Coverage { actual, permitted } => result.coverage(
                actual.map_references(work, &mut |variable, _| convert(variable))?,
                permitted.map_references(work, &mut |variable, _| convert(variable))?,
                work,
            )?,
            ScalarNode::And(children) => {
                result.conjunction(children.iter().map(|id| mapped[id]), work)?
            }
            ScalarNode::Or(children) => {
                result.disjunction(children.iter().map(|id| mapped[id]), work)?
            }
            ScalarNode::Not(child) => result.negate(mapped[child], work)?,
            ScalarNode::Implies {
                domain,
                consequence,
            } => result.implies(mapped[domain], mapped[consequence], work)?,
        };
        work.charge(DecisionWork::Node)?;
        mapped.insert(id, value);
    }
    Ok((result, mapped[&satisfiable], mapped[&principal]))
}

#[derive(Clone, Copy)]
enum ReducedExpression<'a> {
    Original(EffectResidualPredicate<'a>),
    Scalar {
        arena: &'a ScalarArena<ResidualFunctionVariable>,
        node: ScalarId,
    },
}

#[derive(Clone, Copy)]
pub(crate) struct ReducedEffectPredicate<'a> {
    source: &'a ScopedEffectCompletion,
    expression: ReducedExpression<'a>,
}

pub(crate) enum ReducedEffectPredicateView<'a> {
    Original(EffectResidualPredicate<'a>),
    True,
    False,
    Pointwise(&'a SetPredicate<ResidualFunctionVariable>),
    Coverage {
        actual: ResidualRowExpression<'a>,
        permitted: ResidualRowExpression<'a>,
    },
    And(ReducedEffectPredicates<'a>),
    Or(ReducedEffectPredicates<'a>),
    Not(ReducedEffectPredicate<'a>),
    Implies {
        domain: ReducedEffectPredicate<'a>,
        consequence: ReducedEffectPredicate<'a>,
    },
}

pub(crate) struct ReducedEffectPredicates<'a> {
    source: &'a ScopedEffectCompletion,
    arena: &'a ScalarArena<ResidualFunctionVariable>,
    nodes: std::slice::Iter<'a, ScalarId>,
}
impl<'a> Iterator for ReducedEffectPredicates<'a> {
    type Item = ReducedEffectPredicate<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        Some(ReducedEffectPredicate::scalar(
            self.source,
            self.arena,
            *self.nodes.next()?,
        ))
    }
}

fn resolve_function<'a>(
    source: &'a ScopedEffectCompletion,
    variable: &ResidualFunctionVariable,
) -> Result<ResidualFunction<'a>, PreparedEffectInvariant> {
    let id = source.graph.validate(&variable.0)?;
    let function = source
        .graph
        .functions
        .get(&id)
        .ok_or(PreparedEffectInvariant::StaleReference)?;
    if !function.is_future() {
        return Err(PreparedEffectInvariant::FutureBinding);
    }
    Ok(ResidualFunction::at(source, id))
}

#[derive(Clone, Copy)]
pub(crate) struct ResidualRowExpression<'a> {
    source: &'a ScopedEffectCompletion,
    value: &'a SetFormula<ResidualFunctionVariable>,
}
impl<'a> ResidualRowExpression<'a> {
    pub(crate) fn membership(self) -> EffectMembershipView<'a, ResidualFunctionVariable> {
        self.value.membership()
    }
    pub(crate) fn function(
        self,
        variable: &ResidualFunctionVariable,
    ) -> Result<ResidualFunction<'a>, PreparedEffectInvariant> {
        resolve_function(self.source, variable)
    }
}

impl<'a> ReducedEffectPredicate<'a> {
    fn scalar(
        source: &'a ScopedEffectCompletion,
        arena: &'a ScalarArena<ResidualFunctionVariable>,
        node: ScalarId,
    ) -> Self {
        Self {
            source,
            expression: ReducedExpression::Scalar { arena, node },
        }
    }
    pub(crate) fn truth_value(self) -> Option<bool> {
        match self.expression {
            ReducedExpression::Scalar {
                node: ScalarId(0), ..
            } => Some(true),
            ReducedExpression::Scalar {
                node: ScalarId(1), ..
            } => Some(false),
            _ => None,
        }
    }
    pub(crate) fn function(
        self,
        variable: &ResidualFunctionVariable,
    ) -> Result<ResidualFunction<'a>, PreparedEffectInvariant> {
        resolve_function(self.source, variable)
    }
    pub(crate) fn view(self) -> ReducedEffectPredicateView<'a> {
        let ReducedExpression::Scalar { arena, node } = self.expression else {
            let ReducedExpression::Original(original) = self.expression else {
                unreachable!()
            };
            return ReducedEffectPredicateView::Original(original);
        };
        let child = |node| Self::scalar(self.source, arena, node);
        match &arena.nodes[node.0] {
            ScalarNode::True => ReducedEffectPredicateView::True,
            ScalarNode::False => ReducedEffectPredicateView::False,
            ScalarNode::Pointwise(value) => ReducedEffectPredicateView::Pointwise(value),
            ScalarNode::Coverage { actual, permitted } => ReducedEffectPredicateView::Coverage {
                actual: ResidualRowExpression {
                    source: self.source,
                    value: actual,
                },
                permitted: ResidualRowExpression {
                    source: self.source,
                    value: permitted,
                },
            },
            ScalarNode::And(nodes) => ReducedEffectPredicateView::And(ReducedEffectPredicates {
                source: self.source,
                arena,
                nodes: nodes.iter(),
            }),
            ScalarNode::Or(nodes) => ReducedEffectPredicateView::Or(ReducedEffectPredicates {
                source: self.source,
                arena,
                nodes: nodes.iter(),
            }),
            ScalarNode::Not(node) => ReducedEffectPredicateView::Not(child(*node)),
            ScalarNode::Implies {
                domain,
                consequence,
            } => ReducedEffectPredicateView::Implies {
                domain: child(*domain),
                consequence: child(*consequence),
            },
        }
    }
}

/// A row expression stays attached to the component's principal guard. It is
/// inspectable before all Futures are fixed, but cannot issue a canonical row.
pub(crate) struct ReducedEffectFunction<'a> {
    source: &'a ScopedEffectCompletion,
    id: u64,
    value: Option<&'a SetFormula<ResidualFunctionVariable>>,
    principal: ReducedEffectPredicate<'a>,
}
impl<'a> ReducedEffectFunction<'a> {
    pub(crate) fn membership(&self) -> Option<EffectMembershipView<'a, ResidualFunctionVariable>> {
        self.value.map(SetFormula::membership)
    }
    pub(crate) fn principal(&self) -> ReducedEffectPredicate<'a> {
        self.principal
    }
    pub(crate) fn residual(&self) -> ResidualFunctionBody<'a> {
        ResidualFunctionBody::at(self.source, self.id)
    }
    pub(crate) fn function(
        &self,
        variable: &ResidualFunctionVariable,
    ) -> Result<ResidualFunction<'a>, PreparedEffectInvariant> {
        resolve_function(self.source, variable)
    }
}

impl ReducedEffectCompletion<'_> {
    pub(crate) fn satisfiable(&self) -> ReducedEffectPredicate<'_> {
        match &self.normal {
            Some(normal) => {
                ReducedEffectPredicate::scalar(self.source, &normal.arena, normal.satisfiable)
            }
            None => ReducedEffectPredicate {
                source: self.source,
                expression: ReducedExpression::Original(self.source.satisfiable()),
            },
        }
    }
    pub(crate) fn principal(&self) -> ReducedEffectPredicate<'_> {
        match &self.normal {
            Some(normal) => {
                ReducedEffectPredicate::scalar(self.source, &normal.arena, normal.principal)
            }
            None => ReducedEffectPredicate {
                source: self.source,
                expression: ReducedExpression::Original(self.source.principal()),
            },
        }
    }
    pub(crate) fn unknown(
        &self,
        reference: &PreparedEffectUnknownRef,
    ) -> Result<ReducedEffectFunction<'_>, PreparedEffectInvariant> {
        let reference = FunctionRef::Unknown(reference.clone());
        self.source.graph.function(&reference)?;
        Ok(self.function(reference.reference().ordinal))
    }
    pub(crate) fn scheme(
        &self,
        reference: &PreparedEffectSchemeRef,
    ) -> Result<ReducedEffectFunction<'_>, PreparedEffectInvariant> {
        self.source.graph.scheme(reference)?;
        Ok(self.function(reference.0.ordinal))
    }
    fn function(&self, id: u64) -> ReducedEffectFunction<'_> {
        ReducedEffectFunction {
            source: self.source,
            id,
            value: self
                .normal
                .as_ref()
                .and_then(|normal| normal.functions.get(&id)),
            principal: self.principal(),
        }
    }
}

#[cfg(test)]
mod tests;
