//! Borrow the current checked body into the canonical execution-effect fold.

use std::{cell::RefCell, collections::BTreeMap};

use super::super::statements::PreparedIteration;
use super::{Analyzer, ExprId, FinalSemanticAnalysisError, PreparedExpressionFact};
use crate::effect_row::{DecisionControl, DecisionWork};
use crate::final_analysis::{
    PreparedStatementPayload,
    statement_effects::{
        KnownDeclarationEffects, PreparedExpressionEffectSource, known_declaration_effects,
        prepare_expression_body_effects,
    },
};
use arcweft_lang_hir::{identity::StmtId, project::HirExpressionEvaluationEdge};

struct ExpressionBodyEffectSource<'a, 'project, 'symbols, 'control> {
    analyzer: &'a Analyzer<'project, 'symbols, 'control>,
    current: Option<(ExprId, &'a PreparedExpressionFact)>,
    charge_owner: Option<&'a dyn Fn() -> Result<(), FinalSemanticAnalysisError>>,
    // The query consumes only this borrowed prerequisite inventory. Final
    // statement sealing retains the original affine iteration facts.
    iterations: RefCell<BTreeMap<ExprId, PreparedIteration>>,
}

impl PreparedExpressionEffectSource for ExpressionBodyEffectSource<'_, '_, '_, '_> {
    fn enter_effect_owner(&self) -> Result<(), FinalSemanticAnalysisError> {
        self.charge_owner.map_or(Ok(()), |charge| charge())
    }

    fn expression(&self, owner: ExprId) -> Option<&PreparedExpressionFact> {
        self.current
            .filter(|(current, _)| *current == owner)
            .map(|(_, fact)| fact)
            .or_else(|| self.analyzer.facts.expressions().get(&owner))
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
            current: Some((owner, fact)),
            charge_owner: None,
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

    pub(in crate::final_analysis::analyzer) fn known_callable_body_effects(
        &self,
        declaration: &arcweft_lang_hir::symbol::CallableDeclarationKey,
        work: &mut impl DecisionControl<Error = FinalSemanticAnalysisError>,
    ) -> Result<KnownDeclarationEffects, FinalSemanticAnalysisError> {
        let graph = self
            .facts
            .prepared_calls()
            .map_err(FinalSemanticAnalysisError::from)?;
        for _ in graph.selected_nodes() {
            work.charge(DecisionWork::Visit)?;
        }
        let calls =
            super::super::callable_effect_graph::prepared_call_execution_rows(graph, self.control)?;
        let work = RefCell::new(work);
        let charge_owner = || work.borrow_mut().charge(DecisionWork::Visit);
        let source = ExpressionBodyEffectSource {
            analyzer: self,
            current: None,
            charge_owner: Some(&charge_owner),
            iterations: RefCell::new(self.facts.iteration_facts().clone()),
        };
        known_declaration_effects(
            &self.modules,
            &self.topology,
            &source,
            &calls,
            declaration,
            self.control,
        )
    }
}
