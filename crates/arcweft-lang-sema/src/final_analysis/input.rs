//! Mutable staging input for final semantic publication.

use std::{collections::BTreeMap, sync::Arc};

use super::{
    CallTargetFacts, CaptureId, CheckedBinding, CheckedExpression, CheckedItem, CheckedPattern,
    CheckedStatement, ExprId, ItemId, LocalId, PatternId, PhysicalCandidateArgumentEvaluation,
    StmtId, TypeId, TypeKind,
};

/// Mutable staging owner for the independent semantic passes.
///
/// This value is not accepted compiler state. Only
/// [`FinalSemanticAnalysis::try_new`] publishes its contents.
#[derive(Clone, Debug, Default)]
pub(crate) struct FinalSemanticAnalysisInput {
    pub(super) types: Vec<(TypeId, TypeKind)>,
    pub(super) locals: Vec<(LocalId, CheckedBinding)>,
    pub(super) captures: Vec<(CaptureId, CheckedBinding)>,
    pub(super) expressions: Vec<(ExprId, CheckedExpression)>,
    pub(super) patterns: Vec<(PatternId, CheckedPattern)>,
    pub(super) statements: Vec<(StmtId, CheckedStatement)>,
    pub(super) items: Vec<(ItemId, CheckedItem)>,
    pub(super) calls: Vec<CallTargetFacts>,
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

    pub(crate) fn push_expression(&mut self, owner: ExprId, expression: CheckedExpression) {
        self.expressions.push((owner, expression));
    }

    pub(crate) fn push_pattern(&mut self, owner: PatternId, pattern: CheckedPattern) {
        self.patterns.push((owner, pattern));
    }

    pub(crate) fn push_statement(&mut self, owner: StmtId, statement: CheckedStatement) {
        self.statements.push((owner, statement));
    }

    pub(crate) fn push_item(&mut self, owner: ItemId, item: CheckedItem) {
        self.items.push((owner, item));
    }

    pub(crate) fn push_call(&mut self, call: CallTargetFacts) {
        self.calls.push(call);
    }

    pub(crate) fn set_physical_candidate_argument_evaluations(
        &mut self,
        evaluations: BTreeMap<ExprId, Arc<[PhysicalCandidateArgumentEvaluation]>>,
    ) {
        self.physical_candidate_argument_evaluations = evaluations;
    }
}
