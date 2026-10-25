//! Contextual result inference for finite control branches.
//! Probes retain only a type hint; all facts and selected-call projections are
//! rolled back before every branch is checked under the joined expectation.

use super::super::expression_error::AnalyzerExpressionRejection;
use super::super::state::{CandidateFactTransactionAction, CandidateFactTransactionOutcome};
use super::{
    Analyzer, AnalyzerExpressionContext, AnalyzerExpressionError, AnalyzerExpressionExpectation,
    ExprId, TypeKind, common_type,
};

impl Analyzer<'_, '_, '_> {
    pub(super) fn infer_control_branch_type(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        owner: ExprId,
        expectation: &AnalyzerExpressionExpectation<'_>,
        branches: impl IntoIterator<Item = ExprId>,
    ) -> Result<Option<TypeKind>, AnalyzerExpressionError> {
        if expectation.contextual_shape().is_some() {
            return Ok(None);
        }
        let mut joined = None;
        let mut saw_branch = false;
        for branch in branches {
            saw_branch = true;
            let probe = self.run_candidate_fact_transaction::<_, AnalyzerExpressionError>(
                |this, authority, _| {
                    let probe_context = context.child_candidate(authority);
                    let result = this.evaluate_expression_with_expectation(
                        &probe_context,
                        branch,
                        expectation.clone(),
                    );
                    drop(probe_context);
                    let ty = match result {
                        Ok(checked) => checked.value_type().cloned(),
                        // Lack of a synthesis result does not accept a branch.
                        // The final contextual check must validate it in full.
                        Err(AnalyzerExpressionError::Rejected(
                            AnalyzerExpressionRejection::Unavailable { .. },
                        )) => None,
                        Err(error) => return Err(error),
                    };
                    Ok(CandidateFactTransactionAction::Rollback(ty))
                },
            )?;
            let CandidateFactTransactionOutcome::RolledBack(ty) = probe else {
                return Err(AnalyzerExpressionError::fact(
                    crate::final_analysis::CandidateFactTransactionViolation::UnrecoverableLedger,
                ));
            };
            if let Some(ty) = ty {
                joined = Some(match joined {
                    None => ty,
                    Some(previous) => common_type([&previous, &ty], None)
                        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?,
                });
            }
        }
        if saw_branch && joined.is_none() {
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        Ok(joined)
    }
}
