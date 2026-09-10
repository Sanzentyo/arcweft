//! Atomic closure of contextual body results and selected execution roles.

use super::expression_error::{AnalyzerExpressionContext, AnalyzerExpressionError};
use super::items::function_body_roles;
use super::preparation::append_function_body_result_expectations;
use super::state::{
    CandidateFactOperationFailure, CandidateFactTransactionAction, CandidateSemanticProjection,
};
use super::{
    Analyzer, ExprId, FinalSemanticAnalysisError, HirFunctionBody, HirModule, ItemId, Rc, TypeKind,
};

/// The two existing function-body typing rules. A generator must declare a
/// Stream and contain a selected own-scope yield; a returned value must not.
#[derive(Clone, Copy)]
enum FunctionBodyInterpretation {
    ReturnValue,
    Generator,
}

impl FunctionBodyInterpretation {
    fn expected(self, result: &TypeKind) -> TypeKind {
        match self {
            Self::ReturnValue => result.clone(),
            Self::Generator => TypeKind::Unit,
        }
    }

    const fn accepts_yields(self, yields: u32) -> bool {
        match self {
            Self::ReturnValue => yields == 0,
            Self::Generator => yields != 0,
        }
    }
}

impl Analyzer<'_, '_, '_> {
    #[allow(
        clippy::result_large_err,
        reason = "body closure preserves the shared typed semantic error and its complete ownership evidence"
    )]
    pub(super) fn validate_function_body_interpretation(
        &mut self,
        module: &HirModule,
        owner: ItemId,
        body: &HirFunctionBody,
        result: TypeKind,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let HirFunctionBody::Block { tail, .. } = body else {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        };
        if !matches!(result, TypeKind::Stream { .. }) {
            let context = AnalyzerExpressionContext::published(Rc::clone(&self.call_frames));
            let yields = self
                .check_function_body_type(&context, module, body, result)
                .map_err(|error| error.into_public(*tail))?;
            return (yields == 0)
                .then_some(())
                .ok_or(FinalSemanticAnalysisError::InvalidFunctionExecution { owner });
        }
        let interpretations = [
            FunctionBodyInterpretation::ReturnValue,
            FunctionBodyInterpretation::Generator,
        ];
        let outcome = self
            .run_candidate_fact_transaction(
                |this, _, transaction| -> Result<_, CandidateFactOperationFailure> {
                    let mut selected = None;
                    let mut rejection = None;
                    for interpretation in interpretations {
                        match this.probe_function_body(module, body, &result, interpretation) {
                            Ok(Some(projection)) => {
                                if selected.replace(projection).is_some() {
                                    return Err(AnalyzerExpressionError::fatal(
                                        FinalSemanticAnalysisError::AmbiguousFunctionExecution {
                                            owner,
                                        },
                                    )
                                    .into());
                                }
                            }
                            Ok(None) => {}
                            Err(error @ AnalyzerExpressionError::Rejected(_)) => {
                                rejection = Some(error);
                            }
                            Err(error) => return Err(error.into()),
                        }
                    }
                    let projection = selected.ok_or_else(|| {
                        rejection.unwrap_or_else(|| {
                            AnalyzerExpressionError::fatal(
                                FinalSemanticAnalysisError::InvalidFunctionExecution { owner },
                            )
                        })
                    })?;
                    this.facts
                        .apply_candidate_projection(&transaction, projection)
                        .map_err(|failure| {
                            CandidateFactOperationFailure::Projection(Box::new(failure))
                        })?;
                    Ok(CandidateFactTransactionAction::Commit(()))
                },
            )
            .map_err(|error| error.into_public(*tail))?;
        outcome
            .into_committed()
            .map_err(|error| AnalyzerExpressionError::fact(error).into_public(*tail))
    }

    fn probe_function_body(
        &mut self,
        module: &HirModule,
        body: &HirFunctionBody,
        result: &TypeKind,
        interpretation: FunctionBodyInterpretation,
    ) -> Result<Option<CandidateSemanticProjection>, AnalyzerExpressionError> {
        let outcome = self.run_candidate_fact_transaction(
            |this, authority, _| -> Result<_, AnalyzerExpressionError> {
                let context =
                    AnalyzerExpressionContext::candidate(authority, Rc::clone(&this.call_frames));
                let yields = this.check_function_body_type(
                    &context,
                    module,
                    body,
                    interpretation.expected(result),
                )?;
                Ok(CandidateFactTransactionAction::Extract(
                    interpretation.accepts_yields(yields),
                ))
            },
        )?;
        let (accepted, projection) = outcome
            .into_extracted()
            .map_err(AnalyzerExpressionError::fact)?;
        Ok(accepted.then_some(projection))
    }

    fn check_function_body_type(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        body: &HirFunctionBody,
        expected: TypeKind,
    ) -> Result<u32, AnalyzerExpressionError> {
        let HirFunctionBody::Block { statements, .. } = body else {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::RecoveredOwner,
            ));
        };
        let mut expectations = Vec::<(ExprId, TypeKind)>::new();
        append_function_body_result_expectations(module, body, expected, &mut expectations)
            .map_err(AnalyzerExpressionError::fatal)?;
        for (owner, expected) in &expectations {
            let value = self.evaluate_expression(context, *owner, Some(expected))?;
            if value
                .value_type()
                .is_none_or(|actual| !expected.accepts(actual))
            {
                return Err(AnalyzerExpressionError::rejected(*owner));
            }
        }
        self.evaluate_block_statement_uses(context, module, statements)?;
        function_body_roles(module, body, self.facts.expressions())
            .map(|(yields, _)| yields)
            .map_err(AnalyzerExpressionError::fatal)
    }
}

#[cfg(test)]
mod tests {
    use crate::final_analysis::{CheckedFunctionExecution, CheckedItemRole};

    #[test]
    fn body_result_and_execution_role_close_with_tail_choices_and_existing_calls() {
        for (source, generators) in [
            ("fn value() -> i64 { [1i64, 2i64][0] }", 0),
            (
                "fn sink() {}\nfn pass(stream: Stream<i64, String>) -> Stream<i64, String> { sink(); stream }",
                0,
            ),
            (
                "fn generate() -> Stream<i64, String> { yield 1i64; [()][0] }",
                1,
            ),
            (
                "fn pass(stream: Stream<i64, String>) -> Stream<i64, String> { [stream][0] }",
                0,
            ),
            (
                "fn identity<T>(value: T) -> T { value }\nfn pass(stream: Stream<i64, String>) -> Stream<i64, String> { identity(stream) }",
                0,
            ),
        ] {
            let fixture = crate::final_analysis::tests::fixture(source, None);
            let report = crate::final_analysis::tests::analyze(&fixture)
                .unwrap_or_else(|error| panic!("{source}: {error:?}"));
            let actual = report
                .items()
                .filter(|(_, item)| {
                    matches!(
                        item.role(),
                        CheckedItemRole::Function {
                            execution: CheckedFunctionExecution::StreamFactory {
                                own_scope_yields: 1,
                                ..
                            },
                            ..
                        }
                    )
                })
                .count();
            assert_eq!(actual, generators);
        }
    }
}
