//! On-demand body prerequisites for prepared function-value projections.

use std::{collections::BTreeSet, sync::Arc};

use crate::{
    callable::{
        CallableProjection, CallableTerminalEffectProjection, CheckedCallSite,
        CheckedCallableDeclaration, PreparedResolvedCallable,
    },
    effect_row::EffectRow,
    final_analysis::{
        FinalSemanticAnalysisError,
        statement_effects::{
            PreparedDeclarationExecutionEffectInput, prepare_declaration_execution_effects,
        },
    },
};

use super::{
    Analyzer,
    executable_ingress::{PreparedExecutableDeclarationInventory, PreparedExecutableIngressFacts},
};

impl Analyzer<'_, '_, '_> {
    /// Only a projection that needs an inferred terminal row enters this
    /// prerequisite. Ordinary terminal calls retain their typed effect edge.
    pub(super) fn prepare_pending_result_projection(
        &mut self,
        site: CheckedCallSite,
        candidate: &PreparedResolvedCallable,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let effects = self.source_callable_terminal_effects(candidate)?;
        let result = candidate
            .result_schema_for_group(candidate.call_group(), effects)
            .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
                owner: site.expression(),
            })?;
        if matches!(result, CallableProjection::Pending(_)) {
            self.prepare_callable_body_effects(site, candidate)?;
        }
        Ok(())
    }

    fn prepare_callable_body_effects(
        &mut self,
        site: CheckedCallSite,
        candidate: &PreparedResolvedCallable,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let checked = match self.source_callable_terminal_effects(candidate)? {
            CallableTerminalEffectProjection::Known(_) => return Ok(()),
            CallableTerminalEffectProjection::Pending(checked) => checked.clone(),
        };
        let CheckedCallableDeclaration::Project(declaration) = checked.declaration() else {
            return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
        };
        let declaration = declaration.clone();
        let reference = self
            .facts
            .request_effect_projection(site, candidate)
            .map_err(FinalSemanticAnalysisError::from)?;

        // A declaration body has its own lexical execution contexts. Inference
        // must not treat its parameter uses as captures of the requesting call.
        let implicit = std::mem::take(&mut self.implicit_callable_stack);
        let pipes = std::mem::take(&mut self.pipe_stack);
        let function_sites = std::mem::take(&mut self.function_site_stack);
        let prepared = self.prepare_callable_declaration_effects(&declaration);
        self.implicit_callable_stack = implicit;
        self.pipe_stack = pipes;
        self.function_site_stack = function_sites;
        let row = prepared?;
        self.facts
            .complete_effect_projection(&reference, &checked, row)
            .map_err(FinalSemanticAnalysisError::from)
    }

    fn prepare_callable_declaration_effects(
        &mut self,
        declaration: &arcweft_lang_hir::symbol::CallableDeclarationKey,
    ) -> Result<EffectRow, FinalSemanticAnalysisError> {
        let inventory = PreparedExecutableDeclarationInventory::build(self)?;
        let body = inventory
            .get(declaration)
            .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
        if body.contains_event_scrutinee(&self.modules)? {
            // Event-dependent declarations require their ingress authority;
            // an absent ingress type is never replaced with an inferred type.
            return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
        }
        self.complete_ingress_declaration(body, &PreparedExecutableIngressFacts::default())?;
        let graph = self
            .facts
            .prepared_calls()
            .map_err(FinalSemanticAnalysisError::from)?;
        let selected = self.executable.selected_declaration_expression_graph(
            &self.topology,
            declaration,
            |owner| self.facts.expressions().get(&owner)?.selected_postfix_candidate(),
            |owner| {
                let fact = self.facts.expressions().get(&owner)?;
                let Some(site) = fact.checked_call_site(owner) else {
                    return Some(arcweft_lang_hir::project::HirSelectedCallExpressionDisposition::Structural);
                };
                graph.project_site_payload(site,
                    super::calls::AnalyzerPreparedCallPrefix::selected_expression_inventory,
                    |unselected| Ok(unselected.selected_expression_inventory()))?
                    .ok().map(arcweft_lang_hir::project::HirSelectedCallExpressionDisposition::Callable)
            },
        ).map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let expressions = selected
            .expression_owners()
            .map(|owner| {
                self.facts
                    .expressions()
                    .get(&owner)
                    .cloned()
                    .map(|fact| (owner, fact))
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let statements = self.prepare_declaration_statement_effects(&selected)?;
        let prepared =
            prepare_declaration_execution_effects(PreparedDeclarationExecutionEffectInput {
                modules: &self.modules,
                topology: &self.topology,
                selected: &selected,
                expressions: &expressions,
                statements: &statements,
                control: self.control,
            })?;
        let mut effects = prepared
            .declaration_effects(declaration)
            .cloned()
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let evaluated = prepared
            .declaration_expressions(declaration)
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?
            .collect::<BTreeSet<_>>();
        let calls = self
            .facts
            .prepared_calls()
            .map_err(FinalSemanticAnalysisError::from)?
            .selected_nodes()
            .filter(|node| evaluated.contains(&node.site().expression()))
            .filter(|node| {
                node.prefix()
                    .application()
                    .selected()
                    .next_group_for(node.prefix().application().completed_group())
                    .is_none()
            })
            .map(|node| {
                (
                    node.site(),
                    Arc::clone(node.prefix().application().selected_shared()),
                )
            })
            .collect::<Vec<_>>();
        for (site, candidate) in calls {
            self.prepare_callable_body_effects(site, &candidate)?;
            let CallableTerminalEffectProjection::Known(row) =
                self.source_callable_terminal_effects(&candidate)?
            else {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            };
            effects.union_with(
                &row.closed_value()
                    .ok_or(FinalSemanticAnalysisError::OpenEffectRow)?,
            );
        }
        Ok(EffectRow::closed(effects))
    }
}
