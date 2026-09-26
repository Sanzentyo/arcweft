//! On-demand body prerequisites for prepared function-value projections.

use std::{collections::BTreeSet, sync::Arc};

use crate::{
    callable::{
        CallableCandidateId, CallableInstantiation, CallableProjection,
        CallableTerminalEffectProjection, CheckedCallSite, CheckedCallableDeclaration,
        PreparedCallableEffectProjectionSite, PreparedResolvedCallable,
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
    callable_effect_graph::prepared_fixed_call_effect_rows,
    executable_ingress::{PreparedExecutableDeclarationInventory, PreparedExecutableIngressFacts},
    expression_error::AnalyzerExpressionError,
    items::{inferred_callable_result_schema, result_schema_has_omitted_function_rows},
    state::CandidateFactTransactionAction,
};

impl Analyzer<'_, '_, '_> {
    /// A named value needs its full latent arrow, including inferred effects,
    /// without claiming that the expression invoked the declaration.
    pub(super) fn prepare_named_callable_value_type(
        &mut self,
        owner: arcweft_lang_hir::identity::ExprId,
        callable: &crate::final_analysis::CheckedProjectCallable,
    ) -> Result<crate::types::TypeKind, AnalyzerExpressionError> {
        let staged = self.staged_callables.as_ref().ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CheckedCallableCatalog)
        })?;
        let pending = staged
            .builder
            .pending_by_candidate(&CallableCandidateId::Project(
                callable.declaration().clone(),
            ))
            .map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CheckedCallableCatalog)
            })?;
        let record = Arc::clone(pending.record());
        let candidate = PreparedResolvedCallable::try_from_checked_record(
            pending.id().clone(),
            Arc::clone(&record),
            crate::callable::SignatureOrigin::Project {
                declaration: callable.declaration().clone(),
                binding: None,
            },
            CallableInstantiation::None,
            Vec::new(),
            Some(record.authority()),
            &self.catalogs.callable_limits,
        )
        .map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CheckedCallableCatalog)
        })?;
        let result = self.run_candidate_fact_transaction(
            |this, _, _| -> Result<_, AnalyzerExpressionError> {
                this.prepare_callable_body_effects(
                    PreparedCallableEffectProjectionSite::CallableValue(owner),
                    &candidate,
                )
                .map_err(AnalyzerExpressionError::fatal)?;
                let terminal = this
                    .source_callable_terminal_effects(&candidate)
                    .map_err(AnalyzerExpressionError::fatal)?;
                let ty = candidate.callable_value_type(terminal).map_err(|_| {
                    AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::CheckedCallableCatalog,
                    )
                })?;
                let CallableProjection::Ready(ty) = ty else {
                    return Err(AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::CheckedCallableCatalog,
                    ));
                };
                Ok(CandidateFactTransactionAction::Commit(ty))
            },
        )?;
        result
            .into_committed()
            .map_err(AnalyzerExpressionError::fact)
    }

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
        if matches!(result, CallableProjection::Pending(_))
            || (result_schema_has_omitted_function_rows(candidate.schema())
                && effects.inferred_result_schema().is_none())
        {
            self.prepare_callable_body_effects(site.into(), candidate)?;
        }
        Ok(())
    }

    fn prepare_callable_body_effects(
        &mut self,
        site: PreparedCallableEffectProjectionSite,
        candidate: &PreparedResolvedCallable,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let terminal = self.source_callable_terminal_effects(candidate)?;
        let needs_result = result_schema_has_omitted_function_rows(candidate.schema())
            && terminal.inferred_result_schema().is_none();
        let checked = match terminal {
            CallableTerminalEffectProjection::Known { .. } if !needs_result => return Ok(()),
            CallableTerminalEffectProjection::Pending(checked) => checked.clone(),
            CallableTerminalEffectProjection::Known { .. } => candidate
                .checked()
                .cloned()
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?,
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
        let result_schema = inferred_callable_result_schema(
            &self.modules,
            self.symbols
                .callable(&declaration)
                .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?,
            candidate.schema(),
            self.facts.expressions(),
        )?;
        self.facts
            .complete_effect_projection(&reference, &checked, row, result_schema)
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
        let call_effects = prepared_fixed_call_effect_rows(graph, self.control)?;
        let selected = self
            .executable
            .selected_declaration_expression_graph_with_select_target_disposition(
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
            |owner| {
                self.facts
                    .expressions()
                    .get(&owner)
                    .is_some_and(super::PreparedExpressionFact::is_variant_expression)
                    .then_some(
                        arcweft_lang_hir::project::HirSelectedSelectTargetDisposition::StaticVariantQualifier,
                    )
            },
        )
            .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
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
                call_effects: &call_effects,
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
                    node.prefix().application().effect_projection(),
                )
            })
            .collect::<Vec<_>>();
        for (site, candidate, effect_projection) in calls {
            self.prepare_callable_body_effects(site.into(), &candidate)?;
            let CallableTerminalEffectProjection::Known { effects: row, .. } =
                self.source_callable_terminal_effects(&candidate)?
            else {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            };
            let row = effect_projection.specialize(&row).map_err(|_| {
                FinalSemanticAnalysisError::CallResolutionFailed {
                    owner: site.expression(),
                }
            })?;
            effects = crate::final_analysis::statement_effects::union_effect_rows(
                &effects,
                &row,
                self.control,
            )?;
        }
        Ok(effects)
    }
}
