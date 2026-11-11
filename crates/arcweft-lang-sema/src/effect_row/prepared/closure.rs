//! Joint closure of issuer-owned, parameterized effect schemes.

use arcweft_core::effect_row::{EffectDecision, EffectMembershipView};

use super::*;
use crate::effects::EffectId;

mod compilation;
mod reification;

use compilation::Compilation;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Atom {
    Cell {
        scheme: u64,
        class: usize,
        mask: usize,
    },
    Formal {
        scheme: u64,
        class: usize,
        parameter: usize,
    },
    Choice(u64),
    /// Temporary substitution key, eliminated before any relation is retained.
    Reference(GenericEffectReference),
}

type Decision = EffectDecision<Atom>;

struct Layout {
    labels: Box<[EffectId]>,
    tables: BTreeMap<u64, usize>,
    cells: BTreeSet<Atom>,
}

impl Layout {
    fn new<C: DecisionControl>(
        graph: &PreparedEffectGraph,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        let mut labels = BTreeSet::new();
        let mut support = |membership: EffectMembershipView<'_, GenericEffectReference>| -> Result<(), PreparedEffectFailure<C::Error>> {
            for (label, _) in membership.overrides() {
                work.charge(DecisionWork::Visit)?;
                labels.insert(label.clone());
            }
            Ok(())
        };
        for scheme in graph.schemes.values() {
            support(scheme.header.domain.membership())?;
        }
        // Include all retained alternatives, not just unconditional constants.
        for term in graph.rows.values() {
            match term.as_ref() {
                RowTerm::Substitution { formula, arguments } => {
                    support(formula.membership())?;
                    for row in arguments.values() {
                        if let PreparedEffectRow::Canonical(row) = row {
                            support(row.known()?.membership())?;
                        }
                    }
                }
                RowTerm::Application { arguments, .. } | RowTerm::Union(arguments) => {
                    for row in arguments {
                        if let PreparedEffectRow::Canonical(row) = row {
                            support(row.known()?.membership())?;
                        }
                    }
                }
                RowTerm::Select { yes, no, .. } => {
                    for row in [yes, no] {
                        if let PreparedEffectRow::Canonical(row) = row {
                            support(row.known()?.membership())?;
                        }
                    }
                }
            }
        }
        for scheme in graph.schemes.values() {
            if let SchemeState::BodyPrepared {
                body: PreparedEffectRow::Canonical(row),
                ..
            } = &scheme.state
            {
                support(row.known()?.membership())?;
            }
            if let SchemeState::BodyPrepared { obligations, .. } = &scheme.state {
                for predicate in obligations {
                    if let PreparedEffectPredicate::Canonical(predicate) = predicate {
                        support(predicate.membership())?;
                    }
                }
            }
        }
        for predicate in graph.predicates.values() {
            match predicate.as_ref() {
                PredicateTerm::Relation {
                    actual, permitted, ..
                } => {
                    for row in [actual, permitted] {
                        if let PreparedEffectRow::Canonical(row) = row {
                            support(row.known()?.membership())?;
                        }
                    }
                }
                PredicateTerm::And(predicates) | PredicateTerm::Or(predicates) => {
                    for predicate in predicates {
                        if let PreparedEffectPredicate::Canonical(predicate) = predicate {
                            support(predicate.membership())?;
                        }
                    }
                }
                PredicateTerm::Not(predicate) => {
                    if let PreparedEffectPredicate::Canonical(predicate) = predicate {
                        support(predicate.membership())?;
                    }
                }
                PredicateTerm::Select { yes, no, .. } => {
                    for predicate in [yes, no] {
                        if let PreparedEffectPredicate::Canonical(predicate) = predicate {
                            support(predicate.membership())?;
                        }
                    }
                }
            }
        }
        let classes = labels
            .len()
            .checked_add(1)
            .ok_or(PreparedEffectInvariant::TruthTableOverflow)?;
        let mut total = 0usize;
        // Admit the entire exponential shape before allocating any cell table.
        for scheme in graph.schemes.values() {
            work.charge(DecisionWork::Visit)?;
            let shift = u32::try_from(scheme.header.parameters.len())
                .map_err(|_| PreparedEffectInvariant::TruthTableOverflow)?;
            let table = 1usize
                .checked_shl(shift)
                .ok_or(PreparedEffectInvariant::TruthTableOverflow)?;
            total = total
                .checked_add(
                    table
                        .checked_mul(classes)
                        .ok_or(PreparedEffectInvariant::TruthTableOverflow)?,
                )
                .ok_or(PreparedEffectInvariant::TruthTableOverflow)?;
        }
        for _ in 0..total {
            work.charge(DecisionWork::Node)?;
        }
        let mut tables = BTreeMap::new();
        let mut cells = BTreeSet::new();
        for (&id, scheme) in &graph.schemes {
            let table = 1usize << scheme.header.parameters.len();
            tables.insert(id, table);
            for class in 0..classes {
                for mask in 0..table {
                    cells.insert(Atom::Cell {
                        scheme: id,
                        class,
                        mask,
                    });
                }
            }
        }
        Ok(Self {
            labels: labels.into_iter().collect(),
            tables,
            cells,
        })
    }

    fn classes(&self) -> usize {
        self.labels.len() + 1
    }

    fn at<'a>(
        &self,
        view: EffectMembershipView<'a, GenericEffectReference>,
        class: usize,
    ) -> &'a EffectDecision<GenericEffectReference> {
        if class == 0 {
            view.default_decision()
        } else {
            view.at(&self.labels[class - 1])
        }
    }
}

/// A closure borrows its immutable issuer. Pending handles can be projected only
/// through this owner; no partially solved row is a canonical type certificate.
pub(crate) struct ClosedEffectComponent<'a> {
    graph: &'a PreparedEffectGraph,
    layout: Layout,
    schemes: BTreeMap<u64, EffectRow>,
    choices: BTreeMap<u64, bool>,
}

impl PreparedEffectGraph {
    pub(crate) fn close<C: DecisionControl>(
        &self,
        control: &mut C,
    ) -> Result<ClosedEffectComponent<'_>, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        for scheme in self.schemes.values() {
            work.charge(DecisionWork::Visit)?;
            if matches!(scheme.state, SchemeState::Declared) {
                return Err(PreparedEffectInvariant::SchemeState.into());
            }
        }
        let layout = Layout::new(self, &mut work)?;
        let mut relation = Decision::constant(true);
        for (&id, scheme) in &self.schemes {
            let SchemeState::BodyPrepared { body, obligations } = &scheme.state else {
                return Err(PreparedEffectInvariant::SchemeState.into());
            };
            let mut compilation = Compilation::new(self, &layout, id, &scheme.header.parameters);
            let body = compilation.row(body, &mut work)?;
            let parameters = compilation.parameters(&mut work)?;
            let own = compilation.application(id, &parameters, &mut work)?;
            let mut equation = Decision::constant(true);
            for (left, right) in own.iter().zip(&body.membership) {
                equation = equation.and(
                    &left.conditional(right, &right.not(&mut work)?, &mut work)?,
                    &mut work,
                )?;
            }
            equation = equation.and(&body.admitted, &mut work)?;
            for predicate in obligations {
                equation =
                    equation.and(&compilation.predicate(predicate, &mut work)?, &mut work)?;
            }
            let domain = compilation.canonical_predicate(&scheme.header.domain, &mut work)?;
            equation = domain.conditional(&equation, &Decision::constant(true), &mut work)?;
            // Choices stay outside this universal quantifier: one candidate
            // must work for every row-parameter valuation and every label.
            equation = equation.forall(&compilation.formals(&mut work)?, &mut work)?;
            relation = relation.and(&equation, &mut work)?;
            // Finite row functions are false outside support on empty inputs.
            relation = relation.and(
                &Decision::variable(
                    Atom::Cell {
                        scheme: id,
                        class: 0,
                        mask: 0,
                    },
                    &mut work,
                )?
                .not(&mut work)?,
                &mut work,
            )?;
        }
        let mut choices = BTreeSet::new();
        for &id in &self.choices {
            work.charge(DecisionWork::Visit)?;
            choices.insert(Atom::Choice(id));
        }
        let projected = relation.exists(&choices, &mut work)?;
        let completion = projected.complete_conditionally(&layout.cells, &mut work)?;
        if !completion.satisfiable().is_constant(true) {
            return Err(PreparedEffectFailure::Unsatisfiable);
        }
        if !completion.principal().is_constant(true) {
            return Err(PreparedEffectFailure::NonPrincipal);
        }
        let choice_relation = relation.substitute(completion.lower_envelope(), &mut work)?;
        let mut selected = BTreeMap::new();
        for &id in &self.choices {
            let variable = Decision::variable(Atom::Choice(id), &mut work)?;
            let yes = choice_relation
                .and(&variable, &mut work)?
                .exists(&choices, &mut work)?;
            let no = choice_relation
                .and(&variable.not(&mut work)?, &mut work)?
                .exists(&choices, &mut work)?;
            let value = match (yes.is_constant(true), no.is_constant(true)) {
                (true, false) => true,
                (false, true) => false,
                (true, true) => return Err(PreparedEffectFailure::AmbiguousChoice),
                (false, false) => return Err(PreparedEffectFailure::Unsatisfiable),
            };
            selected.insert(id, value);
        }
        let mut schemes = BTreeMap::new();
        for (&id, scheme) in &self.schemes {
            let mut compilation = Compilation::new(self, &layout, id, &scheme.header.parameters);
            let parameters = compilation.parameters(&mut work)?;
            let function = compilation.application(id, &parameters, &mut work)?;
            let mut output = Vec::new();
            for decision in function {
                let decision = decision.substitute(completion.lower_envelope(), &mut work)?;
                let mapped = decision.map_references(&mut work, &mut |atom, _| match atom {
                    Atom::Formal {
                        scheme: owner,
                        parameter,
                        ..
                    } if *owner == id => Ok(scheme.header.parameters[*parameter].clone()),
                    _ => Err(PreparedEffectInvariant::UnclosedCell.into()),
                })?;
                work.charge(DecisionWork::Node)?;
                output.push(mapped);
            }
            let mut output = output.into_iter();
            let default = output.next().ok_or(PreparedEffectInvariant::UnclosedCell)?;
            let mut overrides = BTreeMap::new();
            for (label, decision) in layout.labels.iter().zip(output) {
                work.charge(DecisionWork::Node)?;
                overrides.insert(label.clone(), decision);
            }
            let formula = EffectFormula::try_from_decisions(default, overrides, &mut work)?;
            schemes.insert(
                id,
                EffectRow {
                    formula: Some(formula),
                },
            );
        }
        let closed = ClosedEffectComponent {
            graph: self,
            layout,
            schemes,
            choices: selected,
        };
        // Recheck the original typed grammar after cell elimination. This is an
        // independent canonical check, never a substitute for joint constraints.
        closed.confirm(&closed.layout, &mut work)?;
        Ok(closed)
    }
}
