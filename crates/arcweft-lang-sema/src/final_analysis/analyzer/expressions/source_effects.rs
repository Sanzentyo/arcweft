//! Borrow the current checked body into the canonical execution-effect fold.

use std::{cell::RefCell, collections::BTreeMap};

use super::{Analyzer, ExprId, FinalSemanticAnalysisError, PreparedExpressionFact};
use crate::final_analysis::{
    CheckedIteration, PreparedStatementPayload,
    statement_effects::{PreparedExpressionEffectSource, prepare_expression_body_effects},
};
use arcweft_lang_hir::{identity::StmtId, project::HirExpressionEvaluationEdge};

struct ExpressionBodyEffectSource<'a, 'project, 'symbols, 'control> {
    analyzer: &'a Analyzer<'project, 'symbols, 'control>,
    current: (ExprId, &'a PreparedExpressionFact),
    // The query consumes only this borrowed prerequisite inventory. Final
    // statement sealing retains the original affine iteration facts.
    iterations: RefCell<BTreeMap<ExprId, CheckedIteration>>,
}

impl PreparedExpressionEffectSource for ExpressionBodyEffectSource<'_, '_, '_, '_> {
    fn expression(&self, owner: ExprId) -> Option<&PreparedExpressionFact> {
        if owner == self.current.0 {
            Some(self.current.1)
        } else {
            self.analyzer.facts.expressions().get(&owner)
        }
    }

    fn expression_edges(
        &self,
        owner: ExprId,
    ) -> Result<Box<[HirExpressionEvaluationEdge]>, FinalSemanticAnalysisError> {
        self.analyzer
            .prepared_body_expression_edges(owner, self.current)
    }

    fn statement(
        &self,
        owner: StmtId,
    ) -> Result<PreparedStatementPayload, FinalSemanticAnalysisError> {
        let module = self.analyzer.module(owner.module())?;
        let statement = module
            .resolve_stmt(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        self.analyzer.checked_statement_fact(
            module,
            owner,
            statement.scope(),
            statement.kind(),
            &mut self.iterations.borrow_mut(),
        )
    }
}

impl Analyzer<'_, '_, '_> {
    pub(super) fn prepared_expression_body_effects(
        &self,
        owner: ExprId,
        fact: &PreparedExpressionFact,
    ) -> Result<crate::effect_row::EffectRow, FinalSemanticAnalysisError> {
        let calls = super::super::callable_effect_graph::prepared_call_execution_rows(
            self.facts
                .prepared_calls()
                .map_err(FinalSemanticAnalysisError::from)?,
            self.control,
        )?;
        let source = ExpressionBodyEffectSource {
            analyzer: self,
            current: (owner, fact),
            iterations: RefCell::new(self.facts.iteration_facts().clone()),
        };
        prepare_expression_body_effects(
            &self.modules,
            &self.topology,
            &source,
            &calls,
            owner,
            self.control,
        )
    }
}
