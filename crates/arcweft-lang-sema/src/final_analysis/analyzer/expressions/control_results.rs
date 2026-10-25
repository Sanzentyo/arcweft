//! Contextual result inference for finite control branches.
//! Probes retain only a type hint; all facts and selected-call projections are
//! rolled back before every branch is checked under the joined expectation.

use super::super::expression_error::AnalyzerExpressionRejection;
use super::super::state::{CandidateFactTransactionAction, CandidateFactTransactionOutcome};
use super::{
    Analyzer, AnalyzerExpressionContext, AnalyzerExpressionError, AnalyzerExpressionExpectation,
    ExprId, FinalSemanticAnalysisError, TypeKind, common_type,
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
        // A polymorphic function owns its binder. Its synthesized scheme
        // may join another branch, but is not a monomorphic function-use
        // requirement to reopen against every branch origin. Preserve each
        // original scheme and let the final common-type check join them.
        Ok(joined
            .filter(|ty| !matches!(ty, TypeKind::Function { binder, .. } if !binder.is_empty())))
    }

    pub(super) fn check_control_branch_pair(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        owner: ExprId,
        then_branch: ExprId,
        else_branch: ExprId,
        expectation: &AnalyzerExpressionExpectation<'_>,
    ) -> Result<super::CheckedExpression, AnalyzerExpressionError> {
        let inferred = self.infer_control_branch_type(
            context,
            owner,
            expectation,
            [then_branch, else_branch],
        )?;
        self.finish_control_branch_pair(
            context,
            owner,
            then_branch,
            else_branch,
            expectation,
            inferred.as_ref(),
        )
    }

    fn finish_control_branch_pair(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        owner: ExprId,
        then_branch: ExprId,
        else_branch: ExprId,
        expectation: &AnalyzerExpressionExpectation<'_>,
        inferred: Option<&TypeKind>,
    ) -> Result<super::CheckedExpression, AnalyzerExpressionError> {
        let branch_expectation = if let Some(inferred) = inferred {
            expectation.project_checked(owner, Some(inferred))?
        } else {
            expectation.clone()
        };
        let then_value = self.evaluate_expression_with_expectation(
            context,
            then_branch,
            branch_expectation.clone(),
        )?;
        let then_type = then_value.value_type().cloned().ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                owner: then_branch,
            })
        })?;
        let else_value = self.evaluate_expression_with_expectation(
            context,
            else_branch,
            branch_expectation.clone(),
        )?;
        let else_type = else_value.value_type().ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                owner: else_branch,
            })
        })?;
        let ty = common_type([&then_type, else_type], expectation.complete_type())
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        Ok(super::structural_expression(
            ty,
            super::CheckedTypeSelection::Inferred,
        ))
    }
}
