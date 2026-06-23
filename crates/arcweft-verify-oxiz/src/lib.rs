//! Pure-Rust `OxiZ` adapter for Arcweft verification.
//!
//! All expression typing and SMT-LIB construction belongs to
//! `arcweft-verify::smt::SmtProblem`; this crate only executes the declared
//! problem through `OxiZ` and normalizes the result.

use arcweft_verify::smt::{
    ProofExpr, SmtBackend, SmtCheck, SmtError, SmtOutcome, SmtProblem, SmtSort, SmtSymbolId,
};
use oxiz_core::ast::TermId;
use oxiz_solver::{Context, SolverResult};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Errors specific to the `OxiZ` adapter.
#[derive(Debug, Error)]
pub enum OxizAdapterError {
    #[error("invalid SMT problem `{problem}`: {message}")]
    InvalidProblem { problem: String, message: String },
    #[error("OxiZ failed to execute SMT-LIB for `{problem}`: {message}")]
    Execute { problem: String, message: String },
    #[error("cannot lower SMT problem `{problem}` to OxiZ terms: {message}")]
    Lower { problem: String, message: String },
    #[error("OxiZ did not return sat/unsat/unknown for `{problem}`: {output}")]
    MissingOutcome { problem: String, output: String },
}

/// Pure-Rust solver adapter using the `OxiZ` context API.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OxizBackend;

impl OxizBackend {
    pub fn check_problem(&self, problem: &SmtProblem) -> Result<SmtCheck, OxizAdapterError> {
        let solver_problem = problem.split_ite_assertions();
        solver_problem
            .validate()
            .map_err(|error| OxizAdapterError::InvalidProblem {
                problem: problem.name.clone(),
                message: error.to_string(),
            })?;
        let mut context = Context::new();
        context.set_logic(solver_problem.logic.as_smt_lib());
        let symbols = declare_symbols(&mut context, &solver_problem);
        for assertion in &solver_problem.assertions {
            let term = lower_expr(&mut context, &symbols, assertion).map_err(|message| {
                OxizAdapterError::Lower {
                    problem: problem.name.clone(),
                    message,
                }
            })?;
            context.assert(term);
        }
        let outcome = match context.check_sat() {
            SolverResult::Sat => SmtOutcome::Sat,
            SolverResult::Unsat => SmtOutcome::Unsat,
            SolverResult::Unknown => SmtOutcome::Unknown,
        };
        let model = if outcome.is_counterexample() {
            collect_model(&context, &problem.model_symbols)
        } else {
            BTreeMap::new()
        };
        Ok(SmtCheck::new(outcome)
            .with_model(model)
            .with_raw_output(outcome.as_str()))
    }
}

impl SmtBackend for OxizBackend {
    fn name(&self) -> &'static str {
        "oxiz"
    }

    fn check(&self, problem: &SmtProblem) -> Result<SmtCheck, SmtError> {
        self.check_problem(problem)
            .map_err(|error| SmtError::new(error.to_string()))
    }
}

fn collect_model(context: &Context, requested_symbols: &[SmtSymbolId]) -> BTreeMap<String, String> {
    let requested = requested_symbols
        .iter()
        .map(SmtSymbolId::as_str)
        .collect::<BTreeSet<_>>();
    context
        .get_model()
        .unwrap_or_default()
        .into_iter()
        .filter(|(name, _, _)| requested.contains(name.as_str()))
        .map(|(name, _, value)| (name, value))
        .collect()
}

fn declare_symbols(context: &mut Context, problem: &SmtProblem) -> BTreeMap<SmtSymbolId, TermId> {
    problem
        .symbols
        .iter()
        .map(|symbol| {
            let sort = match symbol.sort {
                SmtSort::Bool => context.terms.sorts.bool_sort,
                SmtSort::Int => context.terms.sorts.int_sort,
            };
            let term = context.declare_const(symbol.id.as_str(), sort);
            (symbol.id.clone(), term)
        })
        .collect()
}

fn lower_expr(
    context: &mut Context,
    symbols: &BTreeMap<SmtSymbolId, TermId>,
    expr: &ProofExpr,
) -> Result<TermId, String> {
    Ok(match expr {
        ProofExpr::Bool(value) => context.terms.mk_bool(*value),
        ProofExpr::Int(value) => context.terms.mk_int(*value),
        ProofExpr::Var(id) => *symbols
            .get(id)
            .ok_or_else(|| format!("undeclared symbol `{id}`"))?,
        ProofExpr::Not { expr } => {
            let expr = lower_expr(context, symbols, expr)?;
            context.terms.mk_not(expr)
        }
        ProofExpr::Neg { expr } => {
            let expr = lower_expr(context, symbols, expr)?;
            let zero = context.terms.mk_int(0);
            context.terms.mk_sub(zero, expr)
        }
        ProofExpr::And { exprs } => {
            let terms = lower_exprs(context, symbols, exprs)?;
            context.terms.mk_and(terms)
        }
        ProofExpr::Or { exprs } => {
            let terms = lower_exprs(context, symbols, exprs)?;
            context.terms.mk_or(terms)
        }
        ProofExpr::Implies {
            premise,
            consequence,
        } => {
            let premise = lower_expr(context, symbols, premise)?;
            let consequence = lower_expr(context, symbols, consequence)?;
            context.terms.mk_implies(premise, consequence)
        }
        ProofExpr::Eq { lhs, rhs } => {
            let lhs = lower_expr(context, symbols, lhs)?;
            let rhs = lower_expr(context, symbols, rhs)?;
            context.terms.mk_eq(lhs, rhs)
        }
        ProofExpr::Le { lhs, rhs } => {
            let lhs = lower_expr(context, symbols, lhs)?;
            let rhs = lower_expr(context, symbols, rhs)?;
            context.terms.mk_le(lhs, rhs)
        }
        ProofExpr::Lt { lhs, rhs } => {
            let lhs = lower_expr(context, symbols, lhs)?;
            let rhs = lower_expr(context, symbols, rhs)?;
            context.terms.mk_lt(lhs, rhs)
        }
        ProofExpr::Ge { lhs, rhs } => {
            let lhs = lower_expr(context, symbols, lhs)?;
            let rhs = lower_expr(context, symbols, rhs)?;
            context.terms.mk_ge(lhs, rhs)
        }
        ProofExpr::Gt { lhs, rhs } => {
            let lhs = lower_expr(context, symbols, lhs)?;
            let rhs = lower_expr(context, symbols, rhs)?;
            context.terms.mk_gt(lhs, rhs)
        }
        ProofExpr::Add { terms } => {
            let terms = lower_exprs(context, symbols, terms)?;
            context.terms.mk_add(terms)
        }
        ProofExpr::Sub { lhs, rhs } => {
            let lhs = lower_expr(context, symbols, lhs)?;
            let rhs = lower_expr(context, symbols, rhs)?;
            context.terms.mk_sub(lhs, rhs)
        }
        ProofExpr::Mul { lhs, rhs } => {
            let lhs = lower_expr(context, symbols, lhs)?;
            let rhs = lower_expr(context, symbols, rhs)?;
            context.terms.mk_mul([lhs, rhs])
        }
        ProofExpr::Ite {
            condition,
            then_expr,
            else_expr,
        } => {
            let condition = lower_expr(context, symbols, condition)?;
            let then_expr = lower_expr(context, symbols, then_expr)?;
            let else_expr = lower_expr(context, symbols, else_expr)?;
            context.terms.mk_ite(condition, then_expr, else_expr)
        }
    })
}

fn lower_exprs(
    context: &mut Context,
    symbols: &BTreeMap<SmtSymbolId, TermId>,
    exprs: &[ProofExpr],
) -> Result<Vec<TermId>, String> {
    exprs
        .iter()
        .map(|expr| lower_expr(context, symbols, expr))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_verify::smt::SmtSymbol;

    #[test]
    fn boolean_identity_is_proven() {
        let problem = SmtProblem::counterexample(
            "identity",
            vec![SmtSymbol::new("p", SmtSort::Bool)],
            [ProofExpr::var("p")],
            ProofExpr::var("p"),
            Vec::new(),
        );
        let check = OxizBackend
            .check_problem(&problem)
            .expect("OxiZ checks declared Boolean problem");
        assert!(check.proves_claim());
    }

    #[test]
    fn integer_bug_returns_a_counterexample_model() {
        let problem = SmtProblem::counterexample(
            "bad_debit",
            vec![
                SmtSymbol::new("balance", SmtSort::Int),
                SmtSymbol::new("price", SmtSort::Int),
                SmtSymbol::new("result", SmtSort::Int),
            ],
            [
                ProofExpr::greater_equal(ProofExpr::var("price"), ProofExpr::int(0)),
                ProofExpr::greater_equal(ProofExpr::var("balance"), ProofExpr::var("price")),
                ProofExpr::equal(
                    ProofExpr::var("result"),
                    ProofExpr::subtract(
                        ProofExpr::subtract(ProofExpr::var("balance"), ProofExpr::var("price")),
                        ProofExpr::int(1),
                    ),
                ),
            ],
            ProofExpr::greater_equal(ProofExpr::var("result"), ProofExpr::int(0)),
            ["balance", "price", "result"]
                .into_iter()
                .map(SmtSymbolId::from)
                .collect(),
        );
        let check = OxizBackend
            .check_problem(&problem)
            .expect("OxiZ checks LIA problem");
        assert_eq!(check.outcome, SmtOutcome::Sat);
        assert!(!check.model.is_empty());
    }

    #[test]
    fn ite_body_relation_is_proven_after_core_split() {
        let result = ProofExpr::var("result");
        let value = ProofExpr::var("value");
        let problem = SmtProblem::counterexample(
            "clamp_lower_bound",
            vec![
                SmtSymbol::new("value", SmtSort::Int),
                SmtSymbol::new("result", SmtSort::Int),
            ],
            [ProofExpr::equal(
                result.clone(),
                ProofExpr::if_then_else(
                    ProofExpr::less_than(value.clone(), ProofExpr::int(0)),
                    ProofExpr::int(0),
                    value,
                ),
            )],
            ProofExpr::greater_equal(result, ProofExpr::int(0)),
            Vec::new(),
        );
        let check = OxizBackend
            .check_problem(&problem)
            .expect("OxiZ checks split ITE body relation");
        assert!(check.proves_claim());
    }

    #[test]
    fn monotone_addition_is_proven() {
        let result = ProofExpr::var("result");
        let progress = ProofExpr::var("progress");
        let step = ProofExpr::var("step");
        let problem = SmtProblem::counterexample(
            "monotone_addition",
            vec![
                SmtSymbol::new("progress", SmtSort::Int),
                SmtSymbol::new("step", SmtSort::Int),
                SmtSymbol::new("result", SmtSort::Int),
            ],
            [
                ProofExpr::greater_equal(progress.clone(), ProofExpr::int(0)),
                ProofExpr::greater_equal(step.clone(), ProofExpr::int(0)),
                ProofExpr::equal(result.clone(), ProofExpr::add([progress.clone(), step])),
            ],
            ProofExpr::greater_equal(result, progress),
            Vec::new(),
        );
        let check = OxizBackend
            .check_problem(&problem)
            .expect("OxiZ checks monotone addition");
        assert!(check.proves_claim(), "{check:?}");
    }
}
