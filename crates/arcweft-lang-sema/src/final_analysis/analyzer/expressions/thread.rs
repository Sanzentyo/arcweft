//! Prepare the typed result of a single Thread expression.

use arcweft_lang_hir::{
    body_edges::HirBodyChild,
    expr::{HirThreadBody, HirThreadExpr},
};
use std::borrow::Cow;

use crate::final_analysis::{completion::ResultTypeJoin, prepared::PreparedThreadReturn};
use crate::types::TypeProjectionControl;

use super::{
    Analyzer, AnalyzerExpressionContext, AnalyzerExpressionError, AnalyzerExpressionExpectation,
    CheckedTypeSelection, EffectId, EffectSet, ExprId, FinalSemanticAnalysisError, HirModule,
    PreparedExpressionFact, PreparedExpressionShell, PreparedOwnerBoundExpression,
    PreparedOwnerBoundResolution, TypeKind,
};

impl Analyzer<'_, '_, '_> {
    pub(super) fn prepare_thread_expression(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        thread: &HirThreadExpr,
        expectation: &AnalyzerExpressionExpectation<'_>,
    ) -> Result<PreparedExpressionFact, AnalyzerExpressionError> {
        let expected = match expectation.contextual_shape() {
            Some(TypeKind::ThreadHandle(result)) => Some(result.as_ref()),
            Some(_) => return Err(AnalyzerExpressionError::rejected(owner)),
            None => None,
        };
        let expectation =
            self.project_expression_expectation(context, owner, expectation, expected)?;
        let hint = expectation.contextual_shape();
        if let Some(result) = hint {
            self.function_site_stack
                .push(super::super::FunctionSiteContext {
                    owner,
                    result: result.clone(),
                });
        }
        let result = (|| {
            self.evaluate_thread_body_uses(context, module, thread.body())?;
            let completion = self.thread_body_completion(module, thread.body(), owner)?;
            let mut work = context.work();
            let mut returns = Vec::new();
            for &statement in completion.returns() {
                work.visit_binding()
                    .map_err(AnalyzerExpressionError::fatal)?;
                returns.push(PreparedThreadReturn {
                    statement,
                    value: Self::return_operand(module, statement)?,
                });
            }
            let normal = completion.normal();
            let normal_type = TypeKind::Unit;
            let values = returns
                .iter()
                .map(|source| self.return_operand_type(source.value))
                .chain(normal.is_unit().then_some(Ok(&normal_type)));
            let mut joined = Cow::Owned(TypeKind::Never);
            for value in values {
                let value = value?;
                match ResultTypeJoin::for_values(&joined, value) {
                    Some(ResultTypeJoin::Left) => {}
                    Some(ResultTypeJoin::Right) => joined = Cow::Borrowed(value),
                    Some(ResultTypeJoin::Combined(ty)) => joined = Cow::Owned(ty),
                    None => {
                        if !joined
                            .may_equal_after_effect_closure(value, &mut work)
                            .map_err(|error| {
                                FinalSemanticAnalysisError::source_type_projection(
                                    owner.into(),
                                    error,
                                )
                            })
                            .map_err(AnalyzerExpressionError::fatal)?
                        {
                            return Self::reject_body_result_join(
                                module,
                                owner,
                                returns.iter().map(|source| source.value),
                                normal.is_unit(),
                            );
                        }
                    }
                }
            }
            let expected = expectation.complete_type();
            if let Some(expected) = expected {
                self.check_prepared_contextual_type(context, owner, expected, &joined)?;
            }
            let declarations = self
                .facts
                .effect_declarations()
                .map_err(AnalyzerExpressionError::fact)?;
            let source = self
                .source_type_preparation(work)
                .map_err(AnalyzerExpressionError::fatal)?;
            let result = source
                .retain_prepared(owner, &joined, declarations.read())
                .map_err(AnalyzerExpressionError::fatal)?;
            let expected = expected
                .map(|ty| source.retain_prepared(owner, ty, declarations.read()))
                .transpose()
                .map_err(AnalyzerExpressionError::fatal)?;
            let mut effects = EffectSet::new();
            effects.insert(
                EffectId::parse("control.spawn")
                    .expect("the language-owned Thread effect is a valid effect identity"),
            );
            let shell = PreparedExpressionShell::value(
                TypeKind::ThreadHandle(Box::new(result)),
                CheckedTypeSelection::Inferred,
                effects,
            );
            Ok(PreparedOwnerBoundExpression::new(
                shell,
                PreparedOwnerBoundResolution::thread(
                    thread,
                    returns.into_boxed_slice(),
                    normal,
                    expected,
                ),
            )
            .into())
        })();
        if hint.is_some() {
            self.function_site_stack
                .pop()
                .expect("Thread result context was just pushed");
        }
        result
    }

    pub(super) fn evaluate_thread_body_uses(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        body: &HirThreadBody,
    ) -> Result<(), AnalyzerExpressionError> {
        let projection = body.try_body_projection().map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::RecoveredOwner)
        })?;
        for edge in projection.children() {
            match edge.child() {
                HirBodyChild::Expression(expression) => {
                    self.evaluate_expression(context, expression, None)?;
                }
                HirBodyChild::Statement(statement) => {
                    self.evaluate_block_statement_uses(context, module, &[statement])?;
                }
            }
        }
        Ok(())
    }
}
