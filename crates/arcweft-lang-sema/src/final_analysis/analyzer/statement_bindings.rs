//! Local binding inference in the expression owner's fact transaction.

use std::rc::Rc;

use super::{
    Analyzer, ExprId, FinalSemanticAnalysisError, HirExprKind, HirModule, HirStmtKind, PatternId,
    StmtId, TypeId, TypeKind,
    expression_error::{AnalyzerExpressionContext, AnalyzerExpressionError},
};

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
            _ => return Ok(()),
        };
        let context = AnalyzerExpressionContext::published(Rc::clone(&self.call_frames));
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
                    owner,
                    *pattern,
                    *initializer,
                    *annotation,
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
                self.seed_contextual_pattern_locals(module, statement.pattern(), ty)
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
                self.seed_contextual_pattern_locals(module, statement.pattern(), ty)
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
                    self.seed_contextual_pattern_locals(module, arm.pattern(), ty)
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
                let item = super::statements::iteration_item(iteration).clone();
                let module = self
                    .module(owner.module())
                    .map_err(AnalyzerExpressionError::fatal)?;
                self.seed_contextual_pattern_locals(module, statement.pattern(), &item)
                    .map_err(AnalyzerExpressionError::fatal)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn evaluate_initializer_binding(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        owner: StmtId,
        pattern: PatternId,
        initializer: ExprId,
        annotation: Option<TypeId>,
    ) -> Result<(), AnalyzerExpressionError> {
        let module = self
            .module(owner.module())
            .map_err(AnalyzerExpressionError::fatal)?;
        let authored_type = annotation.or(module
            .resolve_pattern(pattern)
            .map_err(|_| AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner))?
            .kind()
            .authored_type());
        let expected = authored_type
            .map(|annotation| {
                self.types.get(&annotation).cloned().ok_or_else(|| {
                    AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::TypeResolutionFailed { owner: annotation },
                    )
                })
            })
            .transpose()?;
        let actual = self.evaluate_expression(context, initializer, expected.as_ref())?;
        let actual_type = actual
            .value_type()
            .ok_or_else(|| AnalyzerExpressionError::rejected(initializer))?;
        let binding = match expected {
            Some(expected) if expected.accepts(actual_type) => expected,
            Some(_) => return Err(AnalyzerExpressionError::rejected(initializer)),
            None => dialogue_application_binding_type(module, initializer, actual_type)
                .unwrap_or_else(|| actual_type.clone()),
        };
        self.seed_contextual_pattern_locals(module, pattern, &binding)
            .map_err(AnalyzerExpressionError::fatal)
    }
}

fn dialogue_application_binding_type(
    module: &HirModule,
    owner: ExprId,
    ty: &TypeKind,
) -> Option<TypeKind> {
    let expression = module.resolve_expr(owner).ok()?;
    let HirExprKind::AttachedContentApplication(application) = expression.kind() else {
        return None;
    };
    if !matches!(
        application.family(),
        arcweft_lang_hir::dialogue_application::HirAttachedContentApplicationFamily::DialogueLine { .. }
    ) {
        return None;
    }
    let TypeKind::DialogueLine(result) = ty else {
        return None;
    };
    Some(result.as_ref().clone())
}
