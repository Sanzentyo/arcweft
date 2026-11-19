//! On-demand body prerequisites for prepared function-value projections.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};

use crate::{
    callable::{
        CallableCandidateId, CallableInstantiation, CallableProjection,
        CallableTerminalEffectProjection, CheckedCallSite, CheckedCallableDeclaration,
        CheckedCallableId, EffectPermission, PreparedCallableEffectProjectionSite,
        PreparedResolvedCallable,
    },
    effect_row::EffectRow,
    final_analysis::{
        FinalSemanticAnalysisError,
        statement_effects::{
            PreparedDeclarationExecutionEffectInput, prepare_declaration_execution_effects,
        },
    },
    types::{PreparedEffects, PreparedTypeCompatibility, TypeKind},
};

use super::{
    Analyzer,
    callable_effect_graph::{
        CallableEffectCall, CallableEffectEquations, prepared_call_execution_rows,
    },
    executable_ingress::{
        PreparedExecutableDeclaration, PreparedExecutableDeclarationInventory,
        PreparedExecutableIngressFacts,
    },
    expression_error::AnalyzerExpressionError,
    items::{inferred_callable_result_schema, result_schema_has_omitted_function_rows},
    state::{CandidateFactTransactionAction, CandidateFactTransactionOutcome},
};

/// The exact source operands of a compatibility requirement or a decision
/// that used a provisional observation. Candidate closure and publication
/// check both with the completed effect relation.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct PreparedSourceTypeDecision {
    pub(super) owner: super::ExprId,
    pub(super) expected: TypeKind<PreparedEffects>,
    pub(super) actual: TypeKind<PreparedEffects>,
    pub(super) outcome: SourceTypeDecisionOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceTypeDecisionOutcome {
    Required,
    Observed { accepted: bool },
}

impl PreparedSourceTypeDecision {
    pub(super) fn expected_acceptance(&self) -> bool {
        self.outcome.expected_acceptance()
    }

    pub(super) fn publication_failure(&self, accepted: bool) -> Option<FinalSemanticAnalysisError> {
        if accepted == self.expected_acceptance() {
            return None;
        }
        Some(match self.outcome {
            SourceTypeDecisionOutcome::Required => {
                FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: self.owner }
            }
            SourceTypeDecisionOutcome::Observed { accepted } => {
                FinalSemanticAnalysisError::SourceTypeDecisionChanged {
                    owner: self.owner,
                    originally_accepted: accepted,
                }
            }
        })
    }
}

impl SourceTypeDecisionOutcome {
    fn expected_acceptance(self) -> bool {
        match self {
            Self::Required => true,
            Self::Observed { accepted } => accepted,
        }
    }
}

pub(super) enum SourceTypeDecisionUse {
    RequireAcceptance,
    AnnotatedBinding,
    SelectImplicitCallable,
}

impl Analyzer<'_, '_, '_> {
    pub(super) fn source_type_accepts(
        &mut self,
        context: &super::expression_error::AnalyzerExpressionContext<'_>,
        owner: super::ExprId,
        expected: &TypeKind<PreparedEffects>,
        actual: &TypeKind<PreparedEffects>,
        use_at: SourceTypeDecisionUse,
    ) -> Result<bool, AnalyzerExpressionError> {
        let mut work = context.work();
        if matches!(expected, TypeKind::Bool) {
            return actual
                .accepts_as_condition_with(&mut work)
                .map_err(AnalyzerExpressionError::fatal);
        }
        match expected
            .check_prepared_compatibility(actual, &mut work)
            .map_err(AnalyzerExpressionError::fatal)?
        {
            PreparedTypeCompatibility::Accepted => return Ok(true),
            PreparedTypeCompatibility::Rejected => return Ok(false),
            PreparedTypeCompatibility::NeedsEffectObservation => {}
        }
        let outcome = self.run_candidate_fact_transaction::<_, AnalyzerExpressionError>(
            work.clone(),
            |this, _, _| {
                let outcome = if matches!(use_at, SourceTypeDecisionUse::AnnotatedBinding) {
                    // The annotation supplies the local interface. Its
                    // compatibility with the initializer is a requirement,
                    // even when the initializer refers to the body in progress.
                    SourceTypeDecisionOutcome::Required
                } else {
                    let (expect_observed, actual_observed) = this
                        .observe_source_with_dependencies(context, owner, |source, graph| {
                            source.observe_compatibility_inputs(owner, expected, actual, graph)
                        })
                        .map_err(AnalyzerExpressionError::fatal)?;
                    let accepted = expect_observed
                        .accepts_recovery_with(&actual_observed, &mut work)
                        .map_err(AnalyzerExpressionError::fatal)?;
                    if !accepted && matches!(use_at, SourceTypeDecisionUse::RequireAcceptance) {
                        return Ok(CandidateFactTransactionAction::Rollback(false));
                    }
                    SourceTypeDecisionOutcome::Observed { accepted }
                };
                let graph = this
                    .facts
                    .effect_declarations()
                    .map_err(AnalyzerExpressionError::fact)?
                    .read();
                let source = this
                    .source_type_preparation(work)
                    .map_err(AnalyzerExpressionError::fatal)?;
                let decision = PreparedSourceTypeDecision {
                    owner,
                    expected: source
                        .retain_prepared(owner, expected, graph)
                        .map_err(AnalyzerExpressionError::fatal)?,
                    actual: source
                        .retain_prepared(owner, actual, graph)
                        .map_err(AnalyzerExpressionError::fatal)?,
                    outcome,
                };
                source
                    .visit_binding()
                    .map_err(AnalyzerExpressionError::fatal)?;
                drop(source);
                this.facts
                    .record_source_type_decision(decision)
                    .map_err(AnalyzerExpressionError::fact)?;
                Ok(CandidateFactTransactionAction::Commit(
                    outcome.expected_acceptance(),
                ))
            },
        )?;
        match outcome {
            CandidateFactTransactionOutcome::Committed(accepted)
            | CandidateFactTransactionOutcome::RolledBack(accepted) => Ok(accepted),
            CandidateFactTransactionOutcome::Extracted { .. } => {
                Err(AnalyzerExpressionError::fact(
                    super::CandidateFactTransactionViolation::ProjectionUnavailable,
                ))
            }
        }
    }

    pub(super) fn observe_source_type_with_dependencies(
        &mut self,
        context: &super::expression_error::AnalyzerExpressionContext<'_>,
        owner: arcweft_lang_hir::identity::ExprId,
        ty: &crate::types::TypeKind<crate::types::PreparedEffects>,
    ) -> Result<crate::types::TypeKind, FinalSemanticAnalysisError> {
        self.observe_source_with_dependencies(context, owner, |source, graph| {
            source.observe_canonical(owner, ty, graph)
        })
    }

    pub(super) fn prepare_source_effect_bodies(
        &mut self,
        context: &super::AnalyzerExpressionContext<'_>,
        owner: super::ExprId,
        roots: &crate::types::PreparedTypeEffectRoots<'_>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let mut work = context.work();
        self.observe_source_with_dependencies(context, owner, |_, graph| {
            graph
                .require_query_bodies(roots.rows(), roots.predicates(), &mut work)
                .map_err(FinalSemanticAnalysisError::from)
        })
    }

    pub(super) fn prepare_specialization_effect_bodies<'a>(
        &mut self,
        context: &super::AnalyzerExpressionContext<'_>,
        owner: super::ExprId,
        types: impl IntoIterator<Item = &'a TypeKind<PreparedEffects>>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let roots = crate::types::PreparedTypeEffectRoots::collect(types, [], &mut context.work())
            .map_err(|error| {
                FinalSemanticAnalysisError::source_type_projection(owner.into(), error)
            })?;
        self.prepare_source_effect_bodies(context, owner, &roots)
    }

    fn observe_source_with_dependencies<T>(
        &mut self,
        context: &super::expression_error::AnalyzerExpressionContext<'_>,
        owner: super::ExprId,
        mut observe: impl FnMut(
            super::expression_types::SourceTypePreparation<'_>,
            crate::effect_row::prepared::PreparedEffectReadView<'_>,
        ) -> Result<T, FinalSemanticAnalysisError>,
    ) -> Result<T, FinalSemanticAnalysisError> {
        loop {
            let declarations = self.facts.effect_declarations()?;
            let result = observe(
                self.source_type_preparation(context.work())?,
                declarations.read(),
            );
            match result {
                Ok(observed) => return Ok(observed),
                Err(error) => {
                    let Some(dependency) = error.effect_observation_dependency() else {
                        return Err(error);
                    };
                    self.prepare_type_observation_dependency(context, owner, dependency)?;
                }
            }
        }
    }

    pub(super) fn prepare_type_observation_dependency(
        &mut self,
        context: &super::expression_error::AnalyzerExpressionContext<'_>,
        operand: arcweft_lang_hir::identity::ExprId,
        dependency: &crate::effect_row::prepared::PreparedEffectSchemeRef,
    ) -> Result<(), FinalSemanticAnalysisError> {
        use crate::effect_row::prepared::PreparedEffectFailure;
        let checked = self
            .facts
            .effect_declarations()?
            .observation_dependency_owner(dependency, &mut context.work())?;
        let staged = self
            .staged_callables
            .as_ref()
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let pending = staged
            .builder
            .pending_by_id(checked)
            .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let candidate = PreparedResolvedCallable::try_from_checked_record(
            checked.clone(),
            Arc::clone(pending.record()),
            crate::callable::PreparedCatalogCallee::Direct {
                project_binding: None,
            },
            CallableInstantiation::None,
            Vec::new(),
            &self.catalogs.callable_limits,
        )
        .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        self.prepare_callable_body_effects(
            PreparedCallableEffectProjectionSite::CallableValue(operand),
            &candidate,
        )?;
        if !self
            .facts
            .effect_declarations()?
            .read()
            .scheme_body_is_prepared(dependency)
            .map_err(PreparedEffectFailure::Invariant)?
        {
            return Err(PreparedEffectFailure::ObservationNeedsBody(dependency.clone()).into());
        }
        Ok(())
    }

    /// A named value needs its full latent arrow, including inferred effects,
    /// without claiming that the expression invoked the declaration.
    pub(super) fn prepare_named_callable_value_type(
        &mut self,
        context: &super::expression_error::AnalyzerExpressionContext<'_>,
        callable: &crate::final_analysis::CheckedProjectCallable,
    ) -> Result<crate::types::TypeKind<crate::types::PreparedEffects>, AnalyzerExpressionError>
    {
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
        if let Some(ty) = crate::types::CompileTimeCallableType::for_project(callable.declaration())
        {
            return Ok(crate::types::TypeKind::CompileTimeCallable(ty));
        }
        let candidate = PreparedResolvedCallable::try_from_checked_record(
            pending.id().clone(),
            Arc::clone(&record),
            crate::callable::PreparedCatalogCallee::Direct {
                project_binding: None,
            },
            CallableInstantiation::None,
            Vec::new(),
            &self.catalogs.callable_limits,
        )
        .map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CheckedCallableCatalog)
        })?;
        self.facts
            .ensure_healthy()
            .map_err(AnalyzerExpressionError::fact)?;
        self.facts
            .prepared_calls_mut()
            .map_err(AnalyzerExpressionError::fact)?
            .prepare_declaration_value(&staged.builder, &candidate, &mut context.work())
            .map_err(FinalSemanticAnalysisError::from)
            .map_err(AnalyzerExpressionError::fatal)
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

    pub(super) fn prepare_callable_body_effects(
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
        let prepared = self.prepare_callable_declaration_effects(&checked);
        self.implicit_callable_stack = implicit;
        self.pipe_stack = pipes;
        self.function_site_stack = function_sites;
        let row = prepared?;
        let result_schema = if needs_result {
            let symbol = self
                .symbols
                .callable(&declaration)
                .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
            let returns = self.callable_return_owners(symbol)?;
            let context = self.published_expression_context();
            inferred_callable_result_schema(
                candidate.schema(),
                &returns,
                &mut context.work(),
                |owner, work| {
                    let actual = self
                        .facts
                        .expressions()
                        .get(&owner)
                        .and_then(super::PreparedExpressionFact::value_type)
                        .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                    let graph = self.facts.effect_declarations()?.read();
                    let actual = self
                        .source_type_preparation(work.clone())?
                        .retain_prepared(owner, actual, graph)?;
                    self.observe_source_type_with_dependencies(&context, owner, &actual)
                },
            )?
        } else {
            None
        };
        self.facts
            .complete_effect_projection(&reference, &checked, row, result_schema)
            .map_err(FinalSemanticAnalysisError::from)
    }

    fn prepare_callable_declaration_effects(
        &mut self,
        root: &CheckedCallableId,
    ) -> Result<EffectRow, FinalSemanticAnalysisError> {
        let inventory = PreparedExecutableDeclarationInventory::build(self)?;
        let mut pending = VecDeque::from([root.clone()]);
        let mut scheduled = BTreeSet::from([root.clone()]);
        let mut rows = BTreeMap::new();
        let mut boundary = BTreeMap::new();
        let mut equations = CallableEffectEquations::default();
        while let Some(owner) = pending.pop_front() {
            self.control.check()?;
            let CheckedCallableDeclaration::Project(declaration) = owner.declaration() else {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            };
            let body = inventory
                .get(declaration)
                .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
            let (row, calls) = self.prepare_declaration_effect_equation(body)?;
            let mut applications = Vec::with_capacity(calls.len());
            for call in calls {
                self.control.check()?;
                let target = call.target();
                let contract = self
                    .staged_callables
                    .as_ref()
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?
                    .builder
                    .pending_by_id(target)
                    .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?
                    .body_contract()
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                match contract.permission() {
                    EffectPermission::Bounded(effects) => {
                        if let Some(previous) = boundary.insert(target.clone(), effects.clone())
                            && previous != *effects
                        {
                            return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
                        }
                    }
                    EffectPermission::UnboundedInference => {
                        if !self
                            .facts
                            .effect_declarations()?
                            .declaration_body_is_prepared(target)
                            .map_err(
                                crate::effect_row::prepared::PreparedEffectFailure::Invariant,
                            )?
                            && scheduled.insert(target.clone())
                        {
                            pending.push_back(target.clone());
                        }
                    }
                }
                applications.push(call.expression());
                equations.insert_call(call)?;
            }
            equations.insert_body(owner.clone(), applications)?;
            rows.insert(owner, row);
        }
        equations.prepare_bodies(
            &mut rows,
            &boundary,
            &self
                .staged_callables
                .as_ref()
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?
                .builder,
            self.facts
                .effect_declarations_mut()
                .map_err(FinalSemanticAnalysisError::from)?,
            self.catalogs.callable_limits.max_catalog_build_work(),
            self.control,
        )?;
        rows.remove(root)
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)
    }

    fn prepare_declaration_effect_equation(
        &mut self,
        body: &PreparedExecutableDeclaration,
    ) -> Result<(EffectRow, Vec<CallableEffectCall>), FinalSemanticAnalysisError> {
        let declaration = body.declaration();
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
        let call_effects = prepared_call_execution_rows(graph, self.control)?;
        let selected = self.prepared_declaration_expression_graph(declaration)?;
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
        let effects = prepared
            .declaration_effects(declaration)
            .cloned()
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let evaluated = prepared
            .declaration_effect_expressions(declaration)
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?
            .collect::<BTreeSet<_>>();
        let calls = self
            .facts
            .prepared_calls()
            .map_err(FinalSemanticAnalysisError::from)?
            .selected_nodes()
            .filter(|node| evaluated.contains(&node.site().expression()))
            .filter_map(|node| {
                CallableEffectCall::selected(node.site(), node.prefix().application())
            })
            .collect::<Vec<_>>();
        Ok((effects, calls))
    }
}
