//! Typed, solver-neutral SMT representation used by Arcweft verification.
//!
//! The verifier core owns declarations, expression typing, proof polarity, and
//! deterministic SMT-LIB emission. Concrete process or library I/O stays in the
//! `arcweft-verify-z3` and `arcweft-verify-oxiz` adapter crates.

use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
};
use thiserror::Error;

/// Quantifier-free logics emitted by the current verifier lowering.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmtLogic {
    /// Quantifier-free propositional formulas.
    QfUf,
    /// Quantifier-free linear integer arithmetic.
    #[default]
    QfLia,
}

impl SmtLogic {
    /// SMT-LIB spelling for this logic.
    pub const fn as_smt_lib(self) -> &'static str {
        match self {
            Self::QfUf => "QF_UF",
            Self::QfLia => "QF_LIA",
        }
    }

    /// Selects the least arithmetic-capable logic needed by the symbols.
    pub fn for_symbols(symbols: &[SmtSymbol]) -> Self {
        if symbols.iter().any(|symbol| symbol.sort == SmtSort::Int) {
            Self::QfLia
        } else {
            Self::QfUf
        }
    }
}

/// Sorts supported by the first solver-backed Arcweft contract lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmtSort {
    Bool,
    Int,
}

impl SmtSort {
    /// SMT-LIB spelling for this sort.
    pub const fn as_smt_lib(self) -> &'static str {
        match self {
            Self::Bool => "Bool",
            Self::Int => "Int",
        }
    }
}

impl std::fmt::Display for SmtSort {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_smt_lib())
    }
}

/// Identifier for one declared SMT constant.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SmtSymbolId(String);

impl SmtSymbolId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Arcweft contract symbols intentionally use the portable SMT simple-symbol subset.
    pub fn is_valid(&self) -> bool {
        let mut characters = self.0.chars();
        characters
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
            && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
            && !matches!(
                self.0.as_str(),
                "Bool"
                    | "Int"
                    | "and"
                    | "assert"
                    | "check-sat"
                    | "declare-const"
                    | "exists"
                    | "false"
                    | "forall"
                    | "get-value"
                    | "ite"
                    | "let"
                    | "not"
                    | "or"
                    | "true"
            )
    }
}

impl From<&str> for SmtSymbolId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for SmtSymbolId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl std::fmt::Display for SmtSymbolId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One declared constant retained in the solver-neutral problem.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SmtSymbol {
    pub id: SmtSymbolId,
    pub sort: SmtSort,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_label: Option<String>,
}

impl SmtSymbol {
    pub fn new(id: impl Into<SmtSymbolId>, sort: SmtSort) -> Self {
        Self {
            id: id.into(),
            sort,
            source_label: None,
        }
    }

    #[must_use]
    pub fn with_source_label(mut self, source_label: impl Into<String>) -> Self {
        self.source_label = Some(source_label.into());
        self
    }
}

/// Solver-neutral proof expression.
///
/// This is the single SMT expression enum in `arcweft-verify`; adapters do not
/// introduce a parallel expression tree or an extension trait.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ProofExpr {
    Bool(bool),
    Int(i64),
    Var(SmtSymbolId),
    Not {
        expr: Box<Self>,
    },
    Neg {
        expr: Box<Self>,
    },
    And {
        exprs: Vec<Self>,
    },
    Or {
        exprs: Vec<Self>,
    },
    Implies {
        premise: Box<Self>,
        consequence: Box<Self>,
    },
    Eq {
        lhs: Box<Self>,
        rhs: Box<Self>,
    },
    Le {
        lhs: Box<Self>,
        rhs: Box<Self>,
    },
    Lt {
        lhs: Box<Self>,
        rhs: Box<Self>,
    },
    Ge {
        lhs: Box<Self>,
        rhs: Box<Self>,
    },
    Gt {
        lhs: Box<Self>,
        rhs: Box<Self>,
    },
    Add {
        terms: Vec<Self>,
    },
    Sub {
        lhs: Box<Self>,
        rhs: Box<Self>,
    },
    Mul {
        lhs: Box<Self>,
        rhs: Box<Self>,
    },
    Ite {
        condition: Box<Self>,
        then_expr: Box<Self>,
        else_expr: Box<Self>,
    },
}

impl ProofExpr {
    pub const fn bool(value: bool) -> Self {
        Self::Bool(value)
    }

    pub const fn int(value: i64) -> Self {
        Self::Int(value)
    }

    pub fn var(id: impl Into<SmtSymbolId>) -> Self {
        Self::Var(id.into())
    }

    pub fn negated(expr: Self) -> Self {
        Self::Not {
            expr: Box::new(expr),
        }
    }

    /// Builds the logical refutation of this expression, simplifying standard
    /// Boolean and comparison forms along the way.
    #[must_use]
    pub fn refuted(self) -> Self {
        match self {
            Self::Bool(value) => Self::Bool(!value),
            Self::Not { expr } => *expr,
            Self::And { exprs } => Self::disjunction(exprs.into_iter().map(Self::refuted)),
            Self::Or { exprs } => Self::conjunction(exprs.into_iter().map(Self::refuted)),
            Self::Implies {
                premise,
                consequence,
            } => Self::conjunction([*premise, consequence.refuted()]),
            Self::Le { lhs, rhs } => Self::greater_than(*lhs, *rhs),
            Self::Lt { lhs, rhs } => Self::greater_equal(*lhs, *rhs),
            Self::Ge { lhs, rhs } => Self::less_than(*lhs, *rhs),
            Self::Gt { lhs, rhs } => Self::less_equal(*lhs, *rhs),
            expr => Self::negated(expr),
        }
    }

    pub fn arithmetic_negated(expr: Self) -> Self {
        Self::Neg {
            expr: Box::new(expr),
        }
    }

    pub fn conjunction(exprs: impl IntoIterator<Item = Self>) -> Self {
        Self::And {
            exprs: exprs.into_iter().collect(),
        }
    }

    pub fn disjunction(exprs: impl IntoIterator<Item = Self>) -> Self {
        Self::Or {
            exprs: exprs.into_iter().collect(),
        }
    }

    pub fn implies(premise: Self, consequence: Self) -> Self {
        Self::Implies {
            premise: Box::new(premise),
            consequence: Box::new(consequence),
        }
    }

    pub fn equal(lhs: Self, rhs: Self) -> Self {
        Self::Eq {
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    pub fn less_equal(lhs: Self, rhs: Self) -> Self {
        Self::Le {
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    pub fn less_than(lhs: Self, rhs: Self) -> Self {
        Self::Lt {
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    pub fn greater_equal(lhs: Self, rhs: Self) -> Self {
        Self::Ge {
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    pub fn greater_than(lhs: Self, rhs: Self) -> Self {
        Self::Gt {
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    pub fn add(terms: impl IntoIterator<Item = Self>) -> Self {
        Self::Add {
            terms: terms.into_iter().collect(),
        }
    }

    pub fn subtract(lhs: Self, rhs: Self) -> Self {
        Self::Sub {
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    pub fn multiply(lhs: Self, rhs: Self) -> Self {
        Self::Mul {
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    pub fn if_then_else(condition: Self, then_expr: Self, else_expr: Self) -> Self {
        Self::Ite {
            condition: Box::new(condition),
            then_expr: Box::new(then_expr),
            else_expr: Box::new(else_expr),
        }
    }

    /// Splits top-level Boolean `ite` expressions and equality-to-`ite` terms into
    /// guarded assertions.
    ///
    /// This keeps solver adapters from growing local expression rewrites while
    /// still allowing backends with limited `ite` support to consume equivalent
    /// Boolean formulas for ordinary function-body relations.
    pub fn split_ite_assertions(&self) -> Vec<Self> {
        self.split_ite_assertions_under(None)
    }

    /// Returns the expression sort and rejects ill-typed solver IR before emission.
    pub fn sort(&self, symbols: &BTreeMap<SmtSymbolId, SmtSort>) -> Result<SmtSort, SmtError> {
        match self {
            Self::Bool(_) => Ok(SmtSort::Bool),
            Self::Int(_) => Ok(SmtSort::Int),
            Self::Var(id) => symbols
                .get(id)
                .copied()
                .ok_or_else(|| SmtError::new(format!("undeclared SMT symbol `{id}`"))),
            Self::Not { expr } => {
                Self::require_sort(expr.sort(symbols)?, SmtSort::Bool, "not")?;
                Ok(SmtSort::Bool)
            }
            Self::Neg { expr } => {
                Self::require_sort(expr.sort(symbols)?, SmtSort::Int, "integer negation")?;
                Ok(SmtSort::Int)
            }
            Self::And { exprs } | Self::Or { exprs } => {
                for expr in exprs {
                    Self::require_sort(expr.sort(symbols)?, SmtSort::Bool, "Boolean connective")?;
                }
                Ok(SmtSort::Bool)
            }
            Self::Implies {
                premise,
                consequence,
            } => {
                Self::require_sort(premise.sort(symbols)?, SmtSort::Bool, "implication premise")?;
                Self::require_sort(
                    consequence.sort(symbols)?,
                    SmtSort::Bool,
                    "implication consequence",
                )?;
                Ok(SmtSort::Bool)
            }
            Self::Eq { lhs, rhs } => {
                let lhs_sort = lhs.sort(symbols)?;
                let rhs_sort = rhs.sort(symbols)?;
                if lhs_sort != rhs_sort {
                    return Err(SmtError::new(format!(
                        "equality compares {lhs_sort} with {rhs_sort}"
                    )));
                }
                Ok(SmtSort::Bool)
            }
            Self::Le { lhs, rhs }
            | Self::Lt { lhs, rhs }
            | Self::Ge { lhs, rhs }
            | Self::Gt { lhs, rhs } => {
                Self::require_integer_pair(lhs, rhs, symbols, "integer comparison")?;
                Ok(SmtSort::Bool)
            }
            Self::Add { terms } => {
                for term in terms {
                    Self::require_sort(term.sort(symbols)?, SmtSort::Int, "integer addition")?;
                }
                Ok(SmtSort::Int)
            }
            Self::Sub { lhs, rhs } | Self::Mul { lhs, rhs } => {
                Self::require_integer_pair(lhs, rhs, symbols, "integer arithmetic")?;
                Ok(SmtSort::Int)
            }
            Self::Ite {
                condition,
                then_expr,
                else_expr,
            } => {
                Self::require_sort(condition.sort(symbols)?, SmtSort::Bool, "ite condition")?;
                let then_sort = then_expr.sort(symbols)?;
                let else_sort = else_expr.sort(symbols)?;
                if then_sort != else_sort {
                    return Err(SmtError::new(format!(
                        "ite branches have different sorts: {then_sort} and {else_sort}"
                    )));
                }
                Ok(then_sort)
            }
        }
    }

    /// Deterministic SMT-LIB representation for this expression.
    pub fn emit_smt_lib(&self) -> String {
        match self {
            Self::Bool(value) => value.to_string(),
            Self::Int(value) if *value < 0 => format!("(- {})", value.unsigned_abs()),
            Self::Int(value) => value.to_string(),
            Self::Var(id) => id.to_string(),
            Self::Not { expr } => format!("(not {})", expr.emit_smt_lib()),
            Self::Neg { expr } => format!("(- {})", expr.emit_smt_lib()),
            Self::And { exprs } => Self::emit_nary("and", exprs, "true"),
            Self::Or { exprs } => Self::emit_nary("or", exprs, "false"),
            Self::Implies {
                premise,
                consequence,
            } => format!(
                "(=> {} {})",
                premise.emit_smt_lib(),
                consequence.emit_smt_lib()
            ),
            Self::Eq { lhs, rhs } => Self::emit_binary("=", lhs, rhs),
            Self::Le { lhs, rhs } => Self::emit_binary("<=", lhs, rhs),
            Self::Lt { lhs, rhs } => Self::emit_binary("<", lhs, rhs),
            Self::Ge { lhs, rhs } => Self::emit_binary(">=", lhs, rhs),
            Self::Gt { lhs, rhs } => Self::emit_binary(">", lhs, rhs),
            Self::Add { terms } => Self::emit_nary("+", terms, "0"),
            Self::Sub { lhs, rhs } => Self::emit_binary("-", lhs, rhs),
            Self::Mul { lhs, rhs } => Self::emit_binary("*", lhs, rhs),
            Self::Ite {
                condition,
                then_expr,
                else_expr,
            } => format!(
                "(ite {} {} {})",
                condition.emit_smt_lib(),
                then_expr.emit_smt_lib(),
                else_expr.emit_smt_lib()
            ),
        }
    }

    fn require_integer_pair(
        lhs: &Self,
        rhs: &Self,
        symbols: &BTreeMap<SmtSymbolId, SmtSort>,
        context: &str,
    ) -> Result<(), SmtError> {
        Self::require_sort(lhs.sort(symbols)?, SmtSort::Int, context)?;
        Self::require_sort(rhs.sort(symbols)?, SmtSort::Int, context)
    }

    fn require_sort(actual: SmtSort, expected: SmtSort, context: &str) -> Result<(), SmtError> {
        if actual == expected {
            Ok(())
        } else {
            Err(SmtError::new(format!(
                "{context} expected {expected}, found {actual}"
            )))
        }
    }

    fn emit_binary(operator: &str, lhs: &Self, rhs: &Self) -> String {
        format!("({operator} {} {})", lhs.emit_smt_lib(), rhs.emit_smt_lib())
    }

    fn emit_nary(operator: &str, exprs: &[Self], identity: &str) -> String {
        match exprs {
            [] => identity.to_owned(),
            [expr] => expr.emit_smt_lib(),
            _ => format!(
                "({operator} {})",
                exprs
                    .iter()
                    .map(Self::emit_smt_lib)
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        }
    }

    fn split_ite_assertions_under(&self, guard: Option<Self>) -> Vec<Self> {
        match self {
            Self::And { exprs } => exprs
                .iter()
                .flat_map(|expr| expr.split_ite_assertions_under(guard.clone()))
                .collect(),
            Self::Ite {
                condition,
                then_expr,
                else_expr,
            } => then_expr
                .split_ite_assertions_under(Some(Self::guard_conjunction(
                    guard.clone(),
                    condition.as_ref().clone(),
                )))
                .into_iter()
                .chain(
                    else_expr.split_ite_assertions_under(Some(Self::guard_conjunction(
                        guard,
                        Self::negated(condition.as_ref().clone()),
                    ))),
                )
                .collect(),
            Self::Eq { lhs, rhs } => {
                if matches!(lhs.as_ref(), Self::Ite { .. }) {
                    return Self::split_ite_equality(lhs, rhs, guard);
                }
                if matches!(rhs.as_ref(), Self::Ite { .. }) {
                    return Self::split_ite_equality(rhs, lhs, guard);
                }
                vec![Self::guarded_assertion(guard, self.clone())]
            }
            _ => vec![Self::guarded_assertion(guard, self.clone())],
        }
    }

    fn split_ite_equality(ite_side: &Self, other: &Self, guard: Option<Self>) -> Vec<Self> {
        match ite_side {
            Self::Ite {
                condition,
                then_expr,
                else_expr,
            } => Self::equal(other.clone(), then_expr.as_ref().clone())
                .split_ite_assertions_under(Some(Self::guard_conjunction(
                    guard.clone(),
                    condition.as_ref().clone(),
                )))
                .into_iter()
                .chain(
                    Self::equal(other.clone(), else_expr.as_ref().clone())
                        .split_ite_assertions_under(Some(Self::guard_conjunction(
                            guard,
                            Self::negated(condition.as_ref().clone()),
                        ))),
                )
                .collect(),
            _ => vec![Self::guarded_assertion(
                guard,
                Self::equal(other.clone(), ite_side.clone()),
            )],
        }
    }

    fn guard_conjunction(existing: Option<Self>, next: Self) -> Self {
        match existing {
            Some(existing) => Self::conjunction([existing, next]),
            None => next,
        }
    }

    fn guarded_assertion(guard: Option<Self>, assertion: Self) -> Self {
        match guard {
            Some(guard) => Self::disjunction([Self::negated(guard), assertion]),
            None => assertion,
        }
    }
}

/// Whether an emitted script only checks satisfiability or also requests values.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SmtEmission {
    #[default]
    CheckOnly,
    CounterexampleValues,
}

impl SmtEmission {
    pub const fn requests_values(self) -> bool {
        matches!(self, Self::CounterexampleValues)
    }
}

/// One complete, declared SMT problem.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SmtProblem {
    pub name: String,
    pub logic: SmtLogic,
    pub symbols: Vec<SmtSymbol>,
    pub assertions: Vec<ProofExpr>,
    #[serde(default)]
    pub model_symbols: Vec<SmtSymbolId>,
}

impl SmtProblem {
    /// Builds the standard proof-by-counterexample problem `assumptions ∧ ¬claim`.
    pub fn counterexample(
        name: impl Into<String>,
        symbols: Vec<SmtSymbol>,
        assumptions: impl IntoIterator<Item = ProofExpr>,
        claim: ProofExpr,
        model_symbols: Vec<SmtSymbolId>,
    ) -> Self {
        let logic = SmtLogic::for_symbols(&symbols);
        let assertions = assumptions
            .into_iter()
            .chain(std::iter::once(claim.refuted()))
            .collect();
        Self {
            name: name.into(),
            logic,
            symbols,
            assertions,
            model_symbols,
        }
    }

    /// Validates declarations, expression sorts, and model requests.
    pub fn validate(&self) -> Result<(), SmtError> {
        let mut seen = BTreeSet::new();
        let mut sorts = BTreeMap::new();
        for symbol in &self.symbols {
            if !symbol.id.is_valid() {
                return Err(SmtError::new(format!(
                    "invalid SMT symbol `{}` in problem `{}`",
                    symbol.id, self.name
                )));
            }
            if !seen.insert(symbol.id.clone()) {
                return Err(SmtError::new(format!(
                    "duplicate SMT symbol `{}` in problem `{}`",
                    symbol.id, self.name
                )));
            }
            sorts.insert(symbol.id.clone(), symbol.sort);
        }
        for assertion in &self.assertions {
            let sort = assertion.sort(&sorts)?;
            if sort != SmtSort::Bool {
                return Err(SmtError::new(format!(
                    "SMT assertion in `{}` has non-Boolean sort {sort}",
                    self.name
                )));
            }
        }
        for id in &self.model_symbols {
            if !sorts.contains_key(id) {
                return Err(SmtError::new(format!(
                    "model symbol `{id}` is not declared in `{}`",
                    self.name
                )));
            }
        }
        Ok(())
    }

    /// Emits a deterministic SMT-LIB 2 script for external solvers and artifacts.
    pub fn emit_smt_lib(&self, emission: SmtEmission) -> Result<String, SmtError> {
        self.validate()?;
        let mut output = format!(
            "; Arcweft obligation: {}\n(set-logic {})\n",
            self.name,
            self.logic.as_smt_lib()
        );
        if emission.requests_values() {
            output.push_str("(set-option :produce-models true)\n");
        }
        for symbol in &self.symbols {
            if let Some(source_label) = &symbol.source_label {
                let _ = writeln!(output, "; {} <= {}", symbol.id, source_label);
            }
            let _ = writeln!(
                output,
                "(declare-const {} {})",
                symbol.id,
                symbol.sort.as_smt_lib()
            );
        }
        for assertion in &self.assertions {
            output.push_str("(assert ");
            output.push_str(&assertion.emit_smt_lib());
            output.push_str(")\n");
        }
        output.push_str("(check-sat)\n");
        if emission.requests_values() && !self.model_symbols.is_empty() {
            output.push_str("(get-value (");
            output.push_str(
                &self
                    .model_symbols
                    .iter()
                    .map(SmtSymbolId::as_str)
                    .collect::<Vec<_>>()
                    .join(" "),
            );
            output.push_str("))\n");
        }
        Ok(output)
    }

    /// Returns an equivalent problem with top-level assertion `ite` forms split
    /// into guarded Boolean assertions.
    #[must_use]
    pub fn split_ite_assertions(&self) -> Self {
        let assertions = self
            .assertions
            .iter()
            .flat_map(ProofExpr::split_ite_assertions)
            .collect();
        Self {
            name: self.name.clone(),
            logic: self.logic,
            symbols: self.symbols.clone(),
            assertions,
            model_symbols: self.model_symbols.clone(),
        }
    }
}

/// Normalized solver outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmtOutcome {
    Sat,
    Unsat,
    Unknown,
}

impl SmtOutcome {
    /// For proof-by-counterexample obligations, only `unsat` proves the claim.
    pub const fn proves_claim(self) -> bool {
        matches!(self, Self::Unsat)
    }

    pub const fn is_counterexample(self) -> bool {
        matches!(self, Self::Sat)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sat => "sat",
            Self::Unsat => "unsat",
            Self::Unknown => "unknown",
        }
    }

    /// Finds the first solver result line in adapter output.
    pub fn from_solver_output(output: &str) -> Option<Self> {
        output.lines().find_map(|line| match line.trim() {
            "sat" => Some(Self::Sat),
            "unsat" => Some(Self::Unsat),
            "unknown" => Some(Self::Unknown),
            _ => None,
        })
    }
}

/// Structured result returned by every concrete SMT backend.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SmtCheck {
    pub outcome: SmtOutcome,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub model: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<String>,
}

impl SmtCheck {
    pub fn new(outcome: SmtOutcome) -> Self {
        Self {
            outcome,
            model: BTreeMap::new(),
            raw_output: None,
        }
    }

    #[must_use]
    pub fn with_model(mut self, model: BTreeMap<String, String>) -> Self {
        self.model = model;
        self
    }

    #[must_use]
    pub fn with_raw_output(mut self, raw_output: impl Into<String>) -> Self {
        self.raw_output = Some(raw_output.into());
        self
    }

    pub const fn proves_claim(&self) -> bool {
        self.outcome.proves_claim()
    }
}

/// Error returned by a solver adapter or SMT validation.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct SmtError {
    message: String,
}

impl SmtError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Sans-I/O-facing solver trait. Concrete adapters may perform I/O internally.
pub trait SmtBackend {
    fn name(&self) -> &'static str;
    fn check(&self, problem: &SmtProblem) -> Result<SmtCheck, SmtError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counterexample_problem_negates_the_claim_and_declares_symbols() {
        let problem = SmtProblem::counterexample(
            "bounds",
            vec![SmtSymbol::new("value", SmtSort::Int)],
            [ProofExpr::greater_equal(
                ProofExpr::var("value"),
                ProofExpr::int(0),
            )],
            ProofExpr::greater_equal(ProofExpr::var("value"), ProofExpr::int(0)),
            vec![SmtSymbolId::from("value")],
        );
        let script = problem
            .emit_smt_lib(SmtEmission::CounterexampleValues)
            .expect("problem emits");
        assert!(script.contains("(declare-const value Int)"));
        assert!(script.contains("(assert (< value 0))"));
        assert!(script.contains("(get-value (value))"));
    }

    #[test]
    fn outcome_owns_solver_output_parsing_and_proof_polarity() {
        assert_eq!(
            SmtOutcome::from_solver_output("success\nunsat\n"),
            Some(SmtOutcome::Unsat)
        );
        assert!(SmtOutcome::Unsat.proves_claim());
        assert!(SmtOutcome::Sat.is_counterexample());
    }

    #[test]
    fn ite_equality_splits_into_guarded_assertions() {
        let assertion = ProofExpr::equal(
            ProofExpr::var("result"),
            ProofExpr::if_then_else(
                ProofExpr::less_than(ProofExpr::var("value"), ProofExpr::int(0)),
                ProofExpr::int(0),
                ProofExpr::var("value"),
            ),
        );
        let split = assertion.split_ite_assertions();
        assert_eq!(split.len(), 2);
        assert_eq!(
            split[0].emit_smt_lib(),
            "(or (not (< value 0)) (= result 0))"
        );
        assert_eq!(
            split[1].emit_smt_lib(),
            "(or (not (not (< value 0))) (= result value))"
        );
    }
}
