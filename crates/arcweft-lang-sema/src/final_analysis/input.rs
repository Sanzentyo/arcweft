//! Mutable staging input for final semantic publication.

use std::{collections::BTreeMap, sync::Arc};

use super::match_edges::{
    CheckedSelectedExpressionGraph, CheckedStructuralEdgeDraft, PreparedCallableJoins,
};
use super::{
    CallTargetFacts, CaptureId, CheckedBinding, CheckedItem, CheckedPattern, ExprId, ItemId,
    LocalId, PatternId, PhysicalCandidateArgumentEvaluation, PreparedExpressionFact,
    PreparedPatternFact, PreparedStatementFact, StmtId, TypeId, TypeKind,
};
#[cfg(test)]
use super::{CheckedExpression, CheckedStatement};

/// Mutable staging owner for the independent semantic passes.
///
/// This value is not accepted compiler state. Only
/// [`FinalSemanticAnalysis::try_new`] publishes its contents.
#[derive(Debug, Default)]
pub(crate) struct FinalSemanticAnalysisInput {
    pub(super) types: Vec<(TypeId, TypeKind)>,
    pub(super) locals: Vec<(LocalId, CheckedBinding)>,
    pub(super) captures: Vec<(CaptureId, CheckedBinding)>,
    pub(super) expressions: Vec<(ExprId, PreparedExpressionFact)>,
    pub(super) patterns: Vec<(PatternId, PreparedPatternFact)>,
    pub(super) statements: Vec<(StmtId, PreparedStatementFact)>,
    pub(super) items: Vec<(ItemId, CheckedItem)>,
    pub(super) calls: Vec<CallTargetFacts>,
    pub(super) callable_joins: PreparedCallableJoins,
    pub(super) selected_expressions: Option<CheckedSelectedExpressionGraph>,
    pub(super) structural_edges: Option<CheckedStructuralEdgeDraft>,
    pub(super) physical_candidate_argument_evaluations:
        BTreeMap<ExprId, Arc<[PhysicalCandidateArgumentEvaluation]>>,
}

impl FinalSemanticAnalysisInput {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn push_type(&mut self, owner: TypeId, ty: TypeKind) {
        self.types.push((owner, ty));
    }

    pub(crate) fn push_local(&mut self, owner: LocalId, binding: CheckedBinding) {
        self.locals.push((owner, binding));
    }

    pub(crate) fn push_capture(&mut self, owner: CaptureId, binding: CheckedBinding) {
        self.captures.push((owner, binding));
    }

    #[cfg(test)]
    pub(crate) fn push_expression(&mut self, owner: ExprId, expression: CheckedExpression) {
        self.push_prepared_expression(owner, expression.into());
    }

    pub(crate) fn push_prepared_expression(
        &mut self,
        owner: ExprId,
        expression: PreparedExpressionFact,
    ) {
        self.expressions.push((owner, expression));
    }

    pub(crate) fn push_pattern(&mut self, owner: PatternId, pattern: CheckedPattern) {
        self.push_prepared_pattern(owner, pattern.into());
    }

    pub(crate) fn push_prepared_pattern(&mut self, owner: PatternId, pattern: PreparedPatternFact) {
        self.patterns.push((owner, pattern));
    }

    #[cfg(test)]
    pub(crate) fn push_statement(&mut self, owner: StmtId, statement: CheckedStatement) {
        self.push_prepared_statement(owner, statement.into());
    }

    pub(crate) fn push_prepared_statement(
        &mut self,
        owner: StmtId,
        statement: PreparedStatementFact,
    ) {
        self.statements.push((owner, statement));
    }

    pub(crate) fn push_item(&mut self, owner: ItemId, item: CheckedItem) {
        self.items.push((owner, item));
    }

    pub(crate) fn push_call(&mut self, call: CallTargetFacts) {
        self.calls.push(call);
    }

    pub(crate) fn set_callable_joins(&mut self, joins: PreparedCallableJoins) {
        self.callable_joins = joins;
    }

    pub(super) fn set_structural_edges(
        &mut self,
        edges: CheckedStructuralEdgeDraft,
    ) -> Result<(), super::FinalSemanticAnalysisError> {
        if self.structural_edges.is_some() {
            return Err(super::FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        self.structural_edges = Some(edges);
        Ok(())
    }

    pub(super) fn set_selected_expressions(
        &mut self,
        selected: CheckedSelectedExpressionGraph,
    ) -> Result<(), super::FinalSemanticAnalysisError> {
        if self.selected_expressions.is_some() {
            return Err(super::FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        self.selected_expressions = Some(selected);
        Ok(())
    }

    pub(crate) fn set_physical_candidate_argument_evaluations(
        &mut self,
        evaluations: BTreeMap<ExprId, Arc<[PhysicalCandidateArgumentEvaluation]>>,
    ) {
        self.physical_candidate_argument_evaluations = evaluations;
    }
}
