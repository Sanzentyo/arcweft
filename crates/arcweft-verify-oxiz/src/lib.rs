//! Pure-Rust `OxiZ` adapter boundary for Arcweft verification.
//!
//! Phase 1.5 keeps the verifier core solver-neutral. This adapter owns the
//! dependency on `OxiZ` and provides the API surface that later maps Arcweft
//! `ProofExpr` terms into `OxiZ` terms.

use arcweft_verify::{SmtBackend, SmtError, SmtOutcome, SmtProblem};
use oxiz_solver::Solver;
use thiserror::Error;

/// Errors specific to the `OxiZ` adapter.
#[derive(Debug, Error)]
pub enum OxizAdapterError {
    #[error("OxiZ term lowering is not implemented for problem `{0}`")]
    LoweringNotImplemented(String),
}

/// Pure-Rust solver adapter using `OxiZ` as the backend crate.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OxizBackend;

impl OxizBackend {
    /// Checks a problem. Empty assertion sets are trivially satisfiable; all
    /// non-empty problems currently return `Unknown` until ProofExpr-to-OxiZ
    /// term lowering is completed.
    pub fn check_problem(&self, problem: &SmtProblem) -> Result<SmtOutcome, OxizAdapterError> {
        let _solver = Solver::new();
        if problem.assertions.is_empty() {
            Ok(SmtOutcome::Sat)
        } else {
            Ok(SmtOutcome::Unknown)
        }
    }
}

impl SmtBackend for OxizBackend {
    fn name(&self) -> &'static str {
        "oxiz"
    }

    fn check(&self, problem: &SmtProblem) -> Result<SmtOutcome, SmtError> {
        self.check_problem(problem)
            .map_err(|error| SmtError::new(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_problem_is_sat() {
        let problem = SmtProblem {
            name: "empty".to_owned(),
            assertions: Vec::new(),
        };
        assert_eq!(
            OxizBackend
                .check_problem(&problem)
                .expect("empty problem checks"),
            SmtOutcome::Sat
        );
    }
}
