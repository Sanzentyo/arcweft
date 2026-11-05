//! Bounded propositional proof for the checked assertion query fragment.

use std::collections::BTreeMap;

use crate::smt::{ProofExpr, SmtProblem, SmtSort, SmtSymbol, SmtSymbolId};

// Both CNF construction and search spend this budget. Incomplete search
// leaves the assertion unproved.
const MAX_PROOF_WORK: usize = 262_144;

pub(crate) fn proves_assertion(problem: &SmtProblem) -> bool {
    let [refutation] = problem.assertions.as_slice() else {
        return false;
    };
    prove_refutation(&problem.symbols, refutation, MAX_PROOF_WORK) == Some(true)
}

fn prove_refutation(symbols: &[SmtSymbol], refutation: &ProofExpr, budget: usize) -> Option<bool> {
    let mut encoding = Encoding::new(symbols, budget)?;
    let root = encoding.expression(refutation)?;
    encoding.clause([root])?;
    encoding.work.charge(encoding.variables)?;
    let mut search = ProofSearch {
        clauses: &encoding.clauses,
        assignment: vec![None; encoding.variables],
        trail: Vec::new(),
        work: encoding.work,
    };
    search.is_unsatisfiable()
}

#[derive(Clone, Copy)]
struct Literal {
    variable: usize,
    positive: bool,
}

impl Literal {
    // Variable zero is reserved for the Boolean constant.
    const TRUE: Self = Self {
        variable: 0,
        positive: true,
    };

    const fn negated(self) -> Self {
        Self {
            variable: self.variable,
            positive: !self.positive,
        }
    }
}

struct WorkBudget(usize);

impl WorkBudget {
    fn charge(&mut self, amount: usize) -> Option<()> {
        self.0 = self.0.checked_sub(amount)?;
        Some(())
    }
}

struct Encoding<'a> {
    bindings: BTreeMap<&'a SmtSymbolId, Literal>,
    variables: usize,
    clauses: Vec<Box<[Literal]>>,
    work: WorkBudget,
}

impl<'a> Encoding<'a> {
    fn new(symbols: &'a [SmtSymbol], budget: usize) -> Option<Self> {
        let mut encoding = Self {
            bindings: BTreeMap::new(),
            variables: 1,
            clauses: Vec::new(),
            work: WorkBudget(budget),
        };
        encoding.clause([Literal::TRUE])?;
        for symbol in symbols {
            if symbol.sort != SmtSort::Bool {
                return None;
            }
            let literal = encoding.new_variable()?;
            if encoding.bindings.insert(&symbol.id, literal).is_some() {
                return None;
            }
        }
        Some(encoding)
    }

    fn new_variable(&mut self) -> Option<Literal> {
        self.work.charge(1)?;
        let variable = self.variables;
        self.variables = self.variables.checked_add(1)?;
        Some(Literal {
            variable,
            positive: true,
        })
    }

    fn clause(&mut self, literals: impl IntoIterator<Item = Literal>) -> Option<()> {
        let literals = literals.into_iter();
        let count = literals.size_hint().1?;
        self.work.charge(count.checked_mul(2)?.checked_add(1)?)?;
        let mut signs = BTreeMap::new();
        let mut clause = Vec::with_capacity(count);
        for literal in literals {
            match signs.insert(literal.variable, literal.positive) {
                None => clause.push(literal),
                Some(existing) if existing != literal.positive => return Some(()),
                Some(_) => {}
            }
        }
        self.clauses.push(clause.into_boxed_slice());
        Some(())
    }

    fn expression(&mut self, expr: &ProofExpr) -> Option<Literal> {
        self.work.charge(1)?;
        match expr {
            ProofExpr::Bool(true) => Some(Literal::TRUE),
            ProofExpr::Bool(false) => Some(Literal::TRUE.negated()),
            ProofExpr::Var(id) => self.bindings.get(id).copied(),
            ProofExpr::Not { expr } => Some(self.expression(expr)?.negated()),
            ProofExpr::And { exprs } => self.junction(exprs, false),
            ProofExpr::Or { exprs } => self.junction(exprs, true),
            ProofExpr::Implies {
                premise,
                consequence,
            } => {
                let premise = self.expression(premise)?;
                let not_consequence = self.expression(consequence)?.negated();
                Some(self.conjunction(&[premise, not_consequence])?.negated())
            }
            ProofExpr::Eq { lhs, rhs } => {
                let lhs = self.expression(lhs)?;
                let rhs = self.expression(rhs)?;
                let equal = self.new_variable()?;
                self.clause([equal.negated(), lhs.negated(), rhs])?;
                self.clause([equal.negated(), lhs, rhs.negated()])?;
                self.clause([equal, lhs, rhs])?;
                self.clause([equal, lhs.negated(), rhs.negated()])?;
                Some(equal)
            }
            _ => None,
        }
    }

    fn junction(&mut self, exprs: &[ProofExpr], disjunction: bool) -> Option<Literal> {
        self.work.charge(exprs.len())?;
        let mut atoms = Vec::with_capacity(exprs.len());
        for expr in exprs {
            let atom = self.expression(expr)?;
            atoms.push(if disjunction { atom.negated() } else { atom });
        }
        let conjunction = self.conjunction(&atoms)?;
        Some(if disjunction {
            conjunction.negated()
        } else {
            conjunction
        })
    }

    fn conjunction(&mut self, atoms: &[Literal]) -> Option<Literal> {
        match atoms {
            [] => Some(Literal::TRUE),
            [atom] => Some(*atom),
            _ => {
                let all = self.new_variable()?;
                for &atom in atoms {
                    self.clause([all.negated(), atom])?;
                }
                self.clause(
                    std::iter::once(all).chain(atoms.iter().copied().map(Literal::negated)),
                )?;
                Some(all)
            }
        }
    }
}

struct ProofSearch<'a> {
    clauses: &'a [Box<[Literal]>],
    assignment: Vec<Option<bool>>,
    trail: Vec<usize>,
    work: WorkBudget,
}

enum SearchState {
    Contradiction,
    Satisfied,
    Decide(usize),
}

enum ClauseState {
    Contradiction,
    Satisfied,
    Unit(Literal),
    Open(usize),
}

impl ProofSearch<'_> {
    fn is_unsatisfiable(&mut self) -> Option<bool> {
        let mark = self.trail.len();
        let outcome = match self.propagate() {
            Some(SearchState::Contradiction) => Some(true),
            Some(SearchState::Satisfied) => Some(false),
            Some(SearchState::Decide(variable)) => {
                let branch = self.trail.len();
                self.assign(Literal {
                    variable,
                    positive: false,
                });
                match self.is_unsatisfiable() {
                    Some(true) => {
                        self.rollback(branch);
                        self.assign(Literal {
                            variable,
                            positive: true,
                        });
                        self.is_unsatisfiable()
                    }
                    result => result,
                }
            }
            None => None,
        };
        self.rollback(mark);
        outcome
    }

    fn propagate(&mut self) -> Option<SearchState> {
        loop {
            let mut inferred = false;
            let mut undecided = None;
            for clause in self.clauses {
                match inspect_clause(clause, &self.assignment, &mut self.work)? {
                    ClauseState::Contradiction => return Some(SearchState::Contradiction),
                    ClauseState::Satisfied => {}
                    ClauseState::Unit(literal) => {
                        self.assign(literal);
                        inferred = true;
                    }
                    ClauseState::Open(variable) => {
                        undecided.get_or_insert(variable);
                    }
                }
            }
            if !inferred {
                return Some(undecided.map_or(SearchState::Satisfied, SearchState::Decide));
            }
        }
    }

    fn assign(&mut self, literal: Literal) {
        debug_assert!(self.assignment[literal.variable].is_none());
        self.assignment[literal.variable] = Some(literal.positive);
        self.trail.push(literal.variable);
    }

    fn rollback(&mut self, mark: usize) {
        for variable in self.trail.drain(mark..) {
            self.assignment[variable] = None;
        }
    }
}

fn inspect_clause(
    clause: &[Literal],
    assignment: &[Option<bool>],
    work: &mut WorkBudget,
) -> Option<ClauseState> {
    let mut first = None;
    let mut several = false;
    for &literal in clause {
        work.charge(1)?;
        match assignment[literal.variable] {
            Some(value) if value == literal.positive => return Some(ClauseState::Satisfied),
            Some(_) => {}
            None if first.is_some() => several = true,
            None => first = Some(literal),
        }
    }
    Some(match first {
        None => ClauseState::Contradiction,
        Some(first) if several => ClauseState::Open(first.variable),
        Some(first) => ClauseState::Unit(first),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branching_proves_a_refutation_only_when_every_assignment_is_impossible() {
        let symbols = ["p", "q", "r"].map(|name| SmtSymbol::new(name, SmtSort::Bool));
        let p = ProofExpr::var("p");
        let q = ProofExpr::var("q");
        let r = ProofExpr::var("r");
        let different = |lhs, rhs| ProofExpr::negated(ProofExpr::equal(lhs, rhs));

        // Three variables cannot all differ from their neighbors in a cycle.
        // No authored variable has a unit assignment at the root.
        let refutation = ProofExpr::conjunction([
            different(p.clone(), q.clone()),
            different(q.clone(), r.clone()),
            different(r.clone(), p.clone()),
        ]);
        assert_eq!(
            prove_refutation(&symbols, &refutation, MAX_PROOF_WORK),
            Some(true)
        );
        assert_eq!(prove_refutation(&symbols, &refutation, 1), None);

        // Deleting one edge leaves a real assignment, p = r != q.
        let satisfiable = ProofExpr::conjunction([different(p, q.clone()), different(q, r)]);
        assert_eq!(
            prove_refutation(&symbols, &satisfiable, MAX_PROOF_WORK),
            Some(false)
        );
    }
}
