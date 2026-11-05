//! A bounded finite proof for the checked assertion query fragment.

use std::collections::BTreeMap;

use crate::smt::{ProofExpr, SmtProblem, SmtSort, SmtSymbolId};

// Count actual expression visits across all branches. Every incomplete
// search leaves the condition unproved.
const MAX_EXPRESSION_VISITS: usize = 262_144;

pub(crate) fn proves_assertion(problem: &SmtProblem) -> bool {
    let [refutation] = problem.assertions.as_slice() else {
        return false;
    };
    let bindings = problem
        .symbols
        .iter()
        .enumerate()
        .map(|(index, symbol)| (&symbol.id, index))
        .collect::<BTreeMap<_, _>>();
    if bindings.len() != problem.symbols.len()
        || problem
            .symbols
            .iter()
            .any(|symbol| symbol.sort != SmtSort::Bool)
    {
        return false;
    }
    ProofSearch {
        bindings,
        assignment: vec![None; problem.symbols.len()],
        remaining: MAX_EXPRESSION_VISITS,
    }
    .refutes_all_assignments(refutation, 0)
    .unwrap_or(false)
}

struct ProofSearch<'a> {
    bindings: BTreeMap<&'a SmtSymbolId, usize>,
    assignment: Vec<Option<bool>>,
    remaining: usize,
}

impl ProofSearch<'_> {
    fn refutes_all_assignments(&mut self, expr: &ProofExpr, next: usize) -> Option<bool> {
        match self.evaluate(expr)? {
            Some(value) => Some(!value),
            None => {
                *self.assignment.get_mut(next)? = Some(false);
                let result = match self.refutes_all_assignments(expr, next + 1) {
                    Some(true) => {
                        self.assignment[next] = Some(true);
                        self.refutes_all_assignments(expr, next + 1)
                    }
                    outcome => outcome,
                };
                self.assignment[next] = None;
                result
            }
        }
    }

    fn evaluate(&mut self, expr: &ProofExpr) -> Option<Option<bool>> {
        self.remaining = self.remaining.checked_sub(1)?;
        Some(match expr {
            ProofExpr::Bool(value) => Some(*value),
            ProofExpr::Var(id) => self.assignment[*self.bindings.get(id)?],
            ProofExpr::Not { expr } => self.evaluate(expr)?.map(|value| !value),
            ProofExpr::And { exprs } => return self.junction(exprs, false),
            ProofExpr::Or { exprs } => return self.junction(exprs, true),
            ProofExpr::Implies {
                premise,
                consequence,
            } => match self.evaluate(premise)? {
                Some(false) => Some(true),
                Some(true) => self.evaluate(consequence)?,
                None => self.evaluate(consequence)?.filter(|value| *value),
            },
            ProofExpr::Eq { lhs, rhs } => self
                .evaluate(lhs)?
                .zip(self.evaluate(rhs)?)
                .map(|(lhs, rhs)| lhs == rhs),
            _ => return None,
        })
    }

    fn junction(&mut self, exprs: &[ProofExpr], decisive: bool) -> Option<Option<bool>> {
        let mut all_known = true;
        for expr in exprs {
            match self.evaluate(expr)? {
                Some(value) if value == decisive => return Some(Some(decisive)),
                Some(_) => {}
                None => all_known = false,
            }
        }
        Some(all_known.then_some(!decisive))
    }
}
