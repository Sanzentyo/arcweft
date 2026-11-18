//! Contextual function values retain their source type and one checked use.

use super::calls::{
    AnalyzerCallConstraintDomain, AnalyzerCallProjection, CallAnalysisFailure,
    CallAnalysisInvariant,
};
use super::expression_error::AnalyzerExpressionError;
use super::expressions::AnalyzerExpressionExpectation;
use super::preparation_work::AnalyzerPreparationWork;
use super::{Analyzer, ExprId, ResolverWork};
use crate::callable::{
    CallConstraintInvariant, CallableConstraintApplication, FunctionSpecializationFailure,
    FunctionSpecializationSealFailure,
};
use crate::final_analysis::{FinalSemanticAnalysisError, PreparedExpressionFact};
use crate::types::{
    TypeProjectionError,
    constraints::{CompletedResultProjectionView, TypeConstraintError, TypeConstraintFailure},
};

pub(super) fn specialization_invariant(
    owner: ExprId,
    error: CallConstraintInvariant,
) -> AnalyzerExpressionError {
    AnalyzerExpressionError::Call {
        owner,
        failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error)),
    }
}

fn specialization_failure(
    owner: ExprId,
    error: FunctionSpecializationFailure<AnalyzerCallConstraintDomain>,
) -> AnalyzerExpressionError {
    match error {
        FunctionSpecializationFailure::Prepared(error) => specialization_invariant(owner, error),
        FunctionSpecializationFailure::Constraint(TypeConstraintFailure::Rejected(_)) => {
            AnalyzerExpressionError::rejected(owner)
        }
        FunctionSpecializationFailure::Constraint(error) => {
            super::calls::terminal_lower_constraint_failure(owner, error)
        }
    }
}

pub(super) fn prepared_specialization_failure(
    owner: ExprId,
    error: FunctionSpecializationSealFailure<FinalSemanticAnalysisError>,
) -> AnalyzerExpressionError {
    match error {
        FunctionSpecializationSealFailure::Invariant(invariant) => {
            specialization_invariant(owner, invariant)
        }
        FunctionSpecializationSealFailure::Projection(error) => AnalyzerExpressionError::fatal(
            FinalSemanticAnalysisError::source_type_projection(owner.into(), error),
        ),
    }
}

impl Analyzer<'_, '_, '_> {
    /// Standalone values are already checked. Calls returning a scheme use
    /// their live Call result port before completion instead of this boundary.
    pub(super) fn specialize_checked_function_value(
        &mut self,
        owner: ExprId,
        checked: PreparedExpressionFact,
        expectation: &AnalyzerExpressionExpectation<'_>,
        work: &mut AnalyzerPreparationWork<'_>,
    ) -> Result<PreparedExpressionFact, AnalyzerExpressionError> {
        let Some(source) = checked.value_type() else {
            return Ok(checked);
        };
        let Some(expected) = expectation.specialization_target(Some(source)) else {
            return Ok(checked);
        };
        let module = self
            .module(owner.module())
            .map_err(AnalyzerExpressionError::fatal)?;
        let enclosing = self.enclosing_constraint_scope(module, owner, None)?;
        let graph = self
            .facts
            .prepared_calls_mut()
            .map_err(AnalyzerExpressionError::fact)?;
        let mut resolver_work = ResolverWork::new(self.catalogs.callable_limits.max_query_work());
        let session = resolver_work
            .begin_candidate_constraint_session(
                self.catalogs.callable_limits,
                self.control.cancellation(),
            )
            .map_err(|_| {
                AnalyzerExpressionError::Abort(
                    crate::types::constraints::TypeConstraintAbort::ArithmeticOverflow,
                )
            })?;
        let witness = session
            .specialize_root_function_value::<AnalyzerCallConstraintDomain, _, _>(
                graph,
                CallableConstraintApplication::Specialize(owner),
                source,
                expected,
                &enclosing,
                &self.catalogs.callable_limits,
                AnalyzerCallProjection::Result,
            )
            .map_err(|error| specialization_failure(owner, error))?;
        checked
            .with_function_specialization(owner, witness, work)
            .map_err(|error| prepared_specialization_failure(owner, error))
    }

    pub(super) fn seal_function_result_use(
        &self,
        owner: ExprId,
        result: &CompletedResultProjectionView<'_, AnalyzerCallConstraintDomain>,
        work: &mut ResolverWork,
    ) -> Result<
        std::sync::Arc<crate::callable::CheckedFunctionSpecialization>,
        AnalyzerExpressionError,
    > {
        let mut session = work
            .begin_candidate_constraint_session(
                self.catalogs.callable_limits,
                self.control.cancellation(),
            )
            .map_err(|_| {
                AnalyzerExpressionError::Abort(
                    crate::types::constraints::TypeConstraintAbort::ArithmeticOverflow,
                )
            })?;
        crate::callable::CheckedFunctionSpecialization::seal(result, &mut session).map_err(
            |error| match error {
                FunctionSpecializationSealFailure::Invariant(error) => {
                    specialization_invariant(owner, error)
                }
                FunctionSpecializationSealFailure::Projection(
                    TypeProjectionError::Instantiation(error),
                ) => specialization_invariant(owner, error.into()),
                FunctionSpecializationSealFailure::Projection(TypeProjectionError::Control(
                    TypeConstraintError::Abort(error),
                )) => AnalyzerExpressionError::Abort(error),
                FunctionSpecializationSealFailure::Projection(TypeProjectionError::Control(
                    error,
                )) => super::calls::terminal_lower_constraint_failure(owner, error.into()),
            },
        )
    }
}
