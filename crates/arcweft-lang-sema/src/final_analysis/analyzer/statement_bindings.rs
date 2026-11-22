//! Local binding inference in the expression owner's fact transaction.

use super::{
    Analyzer, ExprId, FinalSemanticAnalysisError, HirModule, HirStmtKind, PatternId,
    PreparedExpressionFact, StmtId, TypeId,
    expression_error::{AnalyzerExpressionContext, AnalyzerExpressionError},
    expressions::AnalyzerExpressionExpectation,
    pending_effects::SourceTypeDecisionUse,
};
use crate::final_analysis::prepared::InitializerBindingProjection;

impl Analyzer<'_, '_, '_> {
    /// Publication entrypoint for declaration and residual statement roots.
    pub(super) fn check_statement_bindings_published(
        &mut self,
        owner: StmtId,
        statement: &HirStmtKind,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let input = match statement {
            HirStmtKind::Let { initializer, .. } | HirStmtKind::LetElse { initializer, .. } => {
                *initializer
            }
            HirStmtKind::IfLet(statement) => statement.scrutinee(),
            HirStmtKind::WhileLet(statement) => statement.scrutinee(),
            HirStmtKind::Match(statement) => statement.scrutinee(),
            HirStmtKind::For(statement) => statement.source(),
            HirStmtKind::LifetimeSet { value, .. } => *value,
            _ => return Ok(()),
        };
        let context = self.published_expression_context();
        self.evaluate_statement_bindings(&context, owner, statement)
            .map_err(|error| error.into_public(input))
    }

    /// Nested blocks keep candidate visibility, rollback and call-frame scope.
    pub(super) fn evaluate_statement_bindings(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        owner: StmtId,
        statement: &HirStmtKind,
    ) -> Result<(), AnalyzerExpressionError> {
        self.control
            .check()
            .map_err(AnalyzerExpressionError::fatal)?;
        match statement {
            HirStmtKind::LifetimeSet { target, value } => {
                let module = self
                    .module(owner.module())
                    .map_err(AnalyzerExpressionError::fatal)?;
                let address = std::sync::Arc::new(
                    crate::final_analysis::CheckedRegistryAddress::from_statement(module, owner)
                        .map_err(AnalyzerExpressionError::fatal)?,
                );
                if let Some(previous) = self.facts.expressions().get(target) {
                    if !matches!(previous.checked_resolution(), Some(crate::final_analysis::CheckedExpressionResolution::RegistryAddress(previous)) if previous == &address)
                    {
                        return Err(AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        ));
                    }
                } else {
                    let target = *target;
                    self.run_candidate_fact_transaction::<_, AnalyzerExpressionError>(
                        context.work(), |this, _, _| {
                            this.facts
                                .publish_new_expression(
                                    target,
                                    crate::final_analysis::prepared::PreparedResolvedExpression::registry_address(
                                        address,
                                    ),
                                )
                                .map_err(|_| {
                                    AnalyzerExpressionError::fatal(
                                        FinalSemanticAnalysisError::WrongPayloadFamily,
                                    )
                                })?;
                            Ok(super::state::CandidateFactTransactionAction::Commit(()))
                        },
                    )?
                    .into_committed()
                    .map_err(AnalyzerExpressionError::fact)?;
                }
                // Later inferred bindings can use the checked producer. The
                // statement is still sealed with the same expression fact.
                self.evaluate_expression(context, *value, None)?;
            }
            HirStmtKind::Let {
                pattern,
                annotation,
                initializer,
                ..
            }
            | HirStmtKind::LetElse {
                pattern,
                annotation,
                initializer,
                ..
            } => {
                self.evaluate_initializer_binding(
                    context,
                    (owner, *pattern, *initializer),
                    *annotation,
                    None,
                )?;
            }
            HirStmtKind::IfLet(statement) => {
                let scrutinee = self.evaluate_expression(context, statement.scrutinee(), None)?;
                let ty = scrutinee
                    .value_type()
                    .ok_or_else(|| AnalyzerExpressionError::rejected(statement.scrutinee()))?;
                let module = self
                    .module(owner.module())
                    .map_err(AnalyzerExpressionError::fatal)?;
                self.seed_contextual_pattern_locals(context, module, statement.pattern(), ty)
                    .map_err(AnalyzerExpressionError::fatal)?;
            }
            HirStmtKind::WhileLet(statement) => {
                let scrutinee = self.evaluate_expression(context, statement.scrutinee(), None)?;
                let ty = scrutinee
                    .value_type()
                    .ok_or_else(|| AnalyzerExpressionError::rejected(statement.scrutinee()))?;
                let module = self
                    .module(owner.module())
                    .map_err(AnalyzerExpressionError::fatal)?;
                self.seed_contextual_pattern_locals(context, module, statement.pattern(), ty)
                    .map_err(AnalyzerExpressionError::fatal)?;
            }
            HirStmtKind::Match(statement) => {
                let scrutinee = self.evaluate_expression(context, statement.scrutinee(), None)?;
                let ty = scrutinee
                    .value_type()
                    .ok_or_else(|| AnalyzerExpressionError::rejected(statement.scrutinee()))?;
                let module = self
                    .module(owner.module())
                    .map_err(AnalyzerExpressionError::fatal)?;
                for arm in statement.arms() {
                    self.seed_contextual_pattern_locals(context, module, arm.pattern(), ty)
                        .map_err(AnalyzerExpressionError::fatal)?;
                }
            }
            HirStmtKind::For(statement) => {
                self.evaluate_expression(context, statement.source(), None)?;
                self.evaluate_expression(context, statement.iterator(), None)?;
                let iteration = self
                    .facts
                    .iteration_facts()
                    .get(&statement.iterator())
                    .ok_or_else(|| AnalyzerExpressionError::rejected(statement.iterator()))?;
                let item = {
                    let types = self
                        .source_type_preparation(context.work())
                        .map_err(AnalyzerExpressionError::fatal)?;
                    let graph = self
                        .facts
                        .effect_declarations()
                        .map_err(AnalyzerExpressionError::fact)?
                        .read();
                    types
                        .retain_prepared(statement.pattern(), iteration.item_type(), graph)
                        .map_err(AnalyzerExpressionError::fatal)?
                };
                let module = self
                    .module(owner.module())
                    .map_err(AnalyzerExpressionError::fatal)?;
                self.seed_contextual_pattern_locals(context, module, statement.pattern(), &item)
                    .map_err(AnalyzerExpressionError::fatal)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Check the binding before resolving a call through its local interface.
    /// A candidate can reuse the original checked initializer.
    pub(super) fn check_function_value_binding(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: StmtId,
        initializer: ExprId,
    ) -> Result<(), AnalyzerExpressionError> {
        let statement = module.resolve_stmt(owner).map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
        })?;
        let (HirStmtKind::Let {
            pattern,
            annotation,
            initializer: authored_initializer,
            ..
        }
        | HirStmtKind::LetElse {
            pattern,
            annotation,
            initializer: authored_initializer,
            ..
        }) = statement.kind()
        else {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::WrongPayloadFamily,
            ));
        };
        if initializer != *authored_initializer {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::InvalidOwner,
            ));
        }
        self.evaluate_initializer_binding(
            context,
            (owner, *pattern, initializer),
            *annotation,
            self.facts.expressions().get(&initializer).cloned(),
        )
    }

    fn evaluate_initializer_binding(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        (owner, pattern, initializer): (StmtId, PatternId, ExprId),
        annotation: Option<TypeId>,
        checked_initializer: Option<PreparedExpressionFact>,
    ) -> Result<(), AnalyzerExpressionError> {
        let module = self
            .module(owner.module())
            .map_err(AnalyzerExpressionError::fatal)?;
        let initializer_expression = module.resolve_expr(initializer).map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
        })?;
        let projection =
            InitializerBindingProjection::from_initializer(initializer_expression.kind());
        let authored_type = annotation.or(module
            .resolve_pattern(pattern)
            .map_err(|_| AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner))?
            .kind()
            .authored_type());
        let prepared_expected = authored_type
            .map(|annotation| {
                self.source_type_preparation(context.work())?
                    .annotation(initializer, annotation)
            })
            .transpose()
            .map_err(AnalyzerExpressionError::fatal)?;
        let initializer_expectation =
            prepared_expected.map(|ty| projection.initializer_expectation(ty));
        let actual = match checked_initializer {
            Some(checked) => checked,
            None => self.evaluate_expression_with_expectation(
                context,
                initializer,
                initializer_expectation.as_ref().map_or(
                    AnalyzerExpressionExpectation::Unconstrained,
                    AnalyzerExpressionExpectation::AnnotatedBinding,
                ),
            )?,
        };
        let actual_type = actual
            .value_type()
            .ok_or_else(|| AnalyzerExpressionError::rejected(initializer))?;
        let binding_type = projection.binding_type(actual_type).ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
        })?;
        let expected = initializer_expectation
            .as_ref()
            .and_then(|ty| projection.binding_type(ty));
        let binding = if let Some(expected) = expected {
            if !self.source_type_accepts(
                context,
                initializer,
                expected,
                binding_type,
                SourceTypeDecisionUse::AnnotatedBinding,
            )? {
                return Err(AnalyzerExpressionError::rejected(initializer));
            }
            self.source_type_preparation(context.work())
                .and_then(|source| {
                    source.complete_binding_effects(initializer, expected, binding_type)
                })
                .map_err(AnalyzerExpressionError::fatal)?
                .ok_or_else(|| AnalyzerExpressionError::rejected(initializer))?
        } else {
            let effects = self
                .facts
                .effect_declarations()
                .map_err(AnalyzerExpressionError::fact)?;
            self.source_type_preparation(context.work())
                .and_then(|source| source.retain_prepared(pattern, binding_type, effects.read()))
                .map_err(AnalyzerExpressionError::fatal)?
        };
        self.seed_contextual_pattern_locals(context, module, pattern, &binding)
            .map_err(AnalyzerExpressionError::fatal)
    }
}
