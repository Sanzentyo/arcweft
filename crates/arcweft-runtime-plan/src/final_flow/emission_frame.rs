//! Exact construction frames for expressions inside retained executable bodies.

use super::{
    ControlLocals, FinalLoweringContext, RuntimeLocalSeedId, RuntimePlanLowerError,
    RuntimePlanSemanticFacts, RuntimeScopedExecutableSemanticFactView, defer, module_by_id,
};
use crate::semantic_facts::RuntimeExecutableSiteScope;
use arcweft_lang_hir::body_edges::{HirBodyChild, HirBodyChildEdge};
use arcweft_lang_hir::identity::{ExprId, LocalId};
use arcweft_lang_hir::project::HirAnalysisProjectView;
use arcweft_lang_hir::stmt::HirStmtEvaluationStep;
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct ExecutableFrameLocals<'a> {
    pub(super) control: &'a ControlLocals,
    pub(super) locals: &'a BTreeMap<LocalId, RuntimeLocalSeedId>,
    pub(super) specialized_operands: &'a BTreeMap<(ExprId, u32), RuntimeLocalSeedId>,
}

enum ExpressionFrameOwner {
    SemanticScope,
    DeferredBody(usize),
}

/// This index selects actual admitted frames; it stores no copied local table.
/// Semantic catalogs and physical deferred bodies have distinct ownership roles.
pub(super) struct ExecutableEmissionFrames<'a> {
    expressions: BTreeMap<(RuntimeExecutableSiteScope, ExprId), ExpressionFrameOwner>,
    deferred: &'a [defer::ReservedDeferDefinition<'a>],
}

impl<'a> ExecutableEmissionFrames<'a> {
    pub(super) fn empty() -> Self {
        Self {
            expressions: BTreeMap::new(),
            deferred: &[],
        }
    }

    pub(super) fn admit(
        project: HirAnalysisProjectView<'_>,
        facts: &RuntimePlanSemanticFacts,
        deferred: &'a [defer::ReservedDeferDefinition<'a>],
    ) -> Result<Self, RuntimePlanLowerError> {
        let mut expressions = BTreeMap::new();
        facts.visit_scoped_expression_owners(&mut |scope, expression| {
            expressions.insert(
                (scope.scope().into(), expression),
                ExpressionFrameOwner::SemanticScope,
            );
        });
        let roots = deferred
            .iter()
            .enumerate()
            .map(|(index, definition)| {
                let (scope, root) = definition.emission_root();
                (
                    (RuntimeExecutableSiteScope::from(scope.scope()), root),
                    index,
                )
            })
            .collect::<BTreeMap<_, _>>();
        if roots.len() != deferred.len() {
            return Err(RuntimePlanLowerError::new(
                "deferred executable repeats its scoped body root",
            ));
        }
        let mut admitted = Self {
            expressions,
            deferred,
        };
        for index in 0..deferred.len() {
            admitted.admit_deferred_body(project, &roots, index)?;
        }
        Ok(admitted)
    }

    fn admit_deferred_body(
        &mut self,
        project: HirAnalysisProjectView<'_>,
        roots: &BTreeMap<(RuntimeExecutableSiteScope, ExprId), usize>,
        index: usize,
    ) -> Result<(), RuntimePlanLowerError> {
        let (scope, root) = self.deferred[index].emission_root();
        let scope_key = RuntimeExecutableSiteScope::from(scope.scope());
        let module = module_by_id(project, root.module()).ok_or_else(|| {
            RuntimePlanLowerError::new("deferred executable body module is absent")
        })?;
        let mut pending = vec![HirBodyChild::Expression(root)];
        let mut seen = BTreeSet::new();
        while let Some(child) = pending.pop() {
            let expression = match child {
                HirBodyChild::Expression(expression) => expression,
                HirBodyChild::Statement(statement) => {
                    module
                        .resolve_stmt(statement)
                        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?
                        .kind()
                        .evaluation_plan()
                        .try_visit_evaluation_steps(|step| match step {
                            HirStmtEvaluationStep::Expression { expression, .. } => {
                                pending.push(HirBodyChild::Expression(expression));
                            }
                            HirStmtEvaluationStep::Statement { statement, .. } => {
                                pending.push(HirBodyChild::Statement(statement));
                            }
                            HirStmtEvaluationStep::ThreadBody { edge, .. } => {
                                pending.push(edge.child());
                            }
                            HirStmtEvaluationStep::Pattern { .. }
                            | HirStmtEvaluationStep::Type { .. }
                            | HirStmtEvaluationStep::Local { .. }
                            | HirStmtEvaluationStep::Publication { .. } => {}
                        })
                        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
                    continue;
                }
            };
            if !seen.insert(expression) {
                continue;
            }
            // A nested registration executes its body in that body's
            // own admitted frame, independently of the registering frame.
            if expression != root && roots.contains_key(&(scope_key.clone(), expression)) {
                continue;
            }
            let owner = self
                .expressions
                .get_mut(&(scope_key.clone(), expression))
                .ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "deferred expression {expression:?} has no admitted semantic owner"
                    ))
                })?;
            if matches!(owner, ExpressionFrameOwner::DeferredBody(other) if *other != index) {
                return Err(RuntimePlanLowerError::new(
                    "expression belongs to two distinct deferred execution frames",
                ));
            }
            *owner = ExpressionFrameOwner::DeferredBody(index);
            pending.extend(
                scope
                    .facts()
                    .expression_children(expression)
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "deferred expression {expression:?} has no selected child relation"
                        ))
                    })?
                    .iter()
                    .copied()
                    .map(HirBodyChild::Expression),
            );
            // Selected expression dependencies partition callable bodies,
            // while HIR body projections retain the statement-owned roots.
            // Both relations are needed for a physical construction frame.
            let kind = module
                .resolve_expr(expression)
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?
                .kind();
            if let Some(body) = kind
                .try_body_projection()
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?
            {
                pending.extend(body.children().iter().map(HirBodyChildEdge::child));
            }
            for body in kind
                .expression_owned_body_projections()
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?
            {
                pending.extend(body.children().iter().map(HirBodyChildEdge::child));
            }
        }
        Ok(())
    }

    pub(super) fn frame<'frame>(
        &'frame self,
        context: &'frame FinalLoweringContext<'_, '_>,
        scope: RuntimeScopedExecutableSemanticFactView<'_>,
        expression: ExprId,
    ) -> Result<ExecutableFrameLocals<'frame>, RuntimePlanLowerError> {
        match self.expressions.get(&(scope.scope().into(), expression)) {
            Some(ExpressionFrameOwner::SemanticScope) => Ok(ExecutableFrameLocals {
                control: context.executable_control_locals(scope.scope())?,
                locals: context.executable_locals(scope.scope())?,
                specialized_operands: context
                    .executable_specialized_operand_locals(scope.scope())?,
            }),
            Some(ExpressionFrameOwner::DeferredBody(index)) => {
                Ok(self.deferred[*index].emission_frame())
            }
            None => Err(RuntimePlanLowerError::new(format!(
                "expression {expression:?} has no admitted execution frame"
            ))),
        }
    }
}
