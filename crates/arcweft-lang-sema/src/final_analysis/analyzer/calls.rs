//! Ordinary-call probing, accounting, and final semantic selection.

#[path = "calls/semantics.rs"]
mod semantics;

use semantics::{
    call_result_type, physical_evaluation_kind, physical_expected_type, provisional_call_effects,
    provisional_callable_effects, select_candidate_probes,
};
pub(super) use semantics::{
    callable_schema_type, callable_schema_type_with_effects, checked_project_nominal,
    final_call_effects, final_callable_effects, nominal_substitutions,
};

use super::expression_types::value_resolution_type;
use super::preparation::AssociatedReceiverTypeResolution;
use super::{
    Analyzer, BTreeMap, CallArgumentMapping, CallCalleeClassificationFact, CallPoison,
    CallResolverAuthority, CallResolverRequest, CallTargetFacts, CallTargetFactsInput,
    CallableGroupIndex, CallableInstantiation, CandidateEvaluationPass, CandidateProbe,
    CandidateScore, CandidateSelection, CandidateSemanticProjection, CharacterOwnerSource,
    CheckedCallArgumentFact, CheckedCallArgumentSlotFact, CheckedCallArgumentSlotInput,
    CheckedCallArgumentSlotSource, CheckedCallTarget, CheckedExpression,
    CheckedExpressionResolution, CheckedTypeSelection, CheckedValueResolution, EffectRow,
    EffectSet, EvaluatedCallArguments, ExprId, FinalCallCalleeFacts, FinalSemanticAnalysisError,
    HirAssociatedSeparator, HirCallArgument, HirCallCallee, HirCallExpr, HirExprKind,
    HirExprSourceRole, HirModule, HirSourcePresence, HirSourceQuery, HirSourceSite,
    PendingCallAnalysis, PhysicalCandidateArgument, PhysicalCandidateArgumentEvaluation,
    RegisteredSemanticValueId, ResolveCallOutcome, ResolvedCallTarget, ResolvedCallable,
    ResolvedCharacterOwner, ResolverWork, TypeKind, TypeParameterSubstitutions, map_call_arguments,
    map_unmapped_call_arguments, prepare_final_call_callee, prepare_language_free_dot_path,
    resolve_call_target,
};
use crate::callable::MappedCallArgument;
use crate::final_analysis::type_rules::compact_numeric_element_type as infer_compact_numeric_element_type;

#[derive(Clone, Copy)]
struct CallSource<'a> {
    module: &'a HirModule,
    owner: ExprId,
    call: &'a HirCallExpr,
    expected: Option<&'a TypeKind>,
}

struct ResolvedCallQuery {
    callee: CallCalleeClassificationFact,
    considered: Vec<ResolvedCallable>,
    function_value_type: Option<TypeKind>,
    current_group: CallableGroupIndex,
    work: ResolverWork,
    argument_count: u64,
}

struct AssociatedReceiverRecovery {
    receiver: arcweft_lang_hir::identity::TypeId,
    separator: HirAssociatedSeparator,
    result: TypeKind,
}

struct CandidateProbeBatch {
    probes: Vec<CandidateProbe>,
    singleton_checkpoint: Option<usize>,
}

#[derive(Clone, Copy)]
struct CandidateProbeRequest<'a> {
    module: &'a HirModule,
    owner: ExprId,
    arguments: &'a [HirCallArgument],
    candidate: &'a ResolvedCallable,
    current_group: CallableGroupIndex,
    expected_result: Option<&'a TypeKind>,
    pass: CandidateEvaluationPass,
}

#[derive(Clone, Copy)]
struct ArgumentEvaluationRequest<'a> {
    call: ExprId,
    authored: &'a [HirCallArgument],
    candidate: &'a ResolvedCallable,
    mapping: &'a CallArgumentMapping,
    pass: CandidateEvaluationPass,
    shape_rejected: bool,
}

#[derive(Clone, Copy)]
struct MappedArgumentEvaluationRequest<'a> {
    call: ExprId,
    authored: &'a HirCallArgument,
    mapped: &'a MappedCallArgument,
    candidate: &'a ResolvedCallable,
    pass: CandidateEvaluationPass,
    shape_rejected: bool,
    argument: arcweft_lang_hir::expr::HirCallArgumentOrdinal,
}

struct MappedArgumentEvaluation {
    fact: CheckedCallArgumentFact,
    hard_errors: usize,
    exact_matches: usize,
}

struct RecoveryCall<'a> {
    source: CallSource<'a>,
    callee: CallCalleeClassificationFact,
    candidates: &'a [ResolvedCallable],
    considered: &'a [ResolvedCallable],
    current_group: CallableGroupIndex,
    arguments: Vec<CheckedCallArgumentFact>,
    result: TypeKind,
    work: ResolverWork,
    ambiguous: bool,
}

impl Analyzer<'_, '_, '_> {
    pub(super) fn check_call_expression(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        call: &HirCallExpr,
        expected: Option<&TypeKind>,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let is_root = !self.callable_query_depth.is_active();
        self.callable_query_depth
            .try_enter()
            .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed { owner })?;
        if is_root {
            self.physical_candidate_argument_evaluations
                .entry(owner)
                .or_default();
        }
        self.physical_call_stack.push(owner);
        let result = self.check_call_expression_inner(module, owner, call, expected);
        let popped = self
            .physical_call_stack
            .pop()
            .expect("Call stack push and pop are paired");
        assert_eq!(popped, owner, "nested Call stack exits LIFO");
        self.callable_query_depth.leave();
        result
    }

    fn check_call_expression_inner(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        call: &HirCallExpr,
        expected: Option<&TypeKind>,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let source = CallSource {
            module,
            owner,
            call,
            expected,
        };
        let argument_count = u64::try_from(source.call.arguments().len())
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        let mut work = ResolverWork::new(self.catalogs.callable_limits.max_query_work());
        work.record_logical_argument_checks(argument_count)
            .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
                owner: source.owner,
            })?;
        if let Some(recovery) = self.stage_call_callee_children(source.module, source.call)? {
            return self.publish_associated_receiver_recovery(source, recovery, work);
        }
        let mut resolution = self.resolve_call_query(source, work, argument_count)?;
        let probes = self.probe_resolved_call(source, &mut resolution)?;
        match select_candidate_probes(&probes.probes) {
            CandidateSelection::Selected(selected) => {
                self.publish_selected_call(source, resolution, probes, selected)
            }
            CandidateSelection::Ambiguous { primary, tied } => {
                self.publish_ambiguous_call(source, resolution, probes, primary, tied)
            }
            CandidateSelection::Rejected { primary } => {
                self.publish_rejected_call(source, resolution, probes, primary)
            }
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "call-query resolution keeps preparation, charged resolver execution, and checked fact publication atomic"
    )]
    fn resolve_call_query(
        &mut self,
        source: CallSource<'_>,
        mut work: ResolverWork,
        argument_count: u64,
    ) -> Result<ResolvedCallQuery, FinalSemanticAnalysisError> {
        let authority = CallResolverAuthority::accepted(
            self.project,
            source.module,
            self.symbols,
            self.catalogs.world,
        );
        let enum_variants = BTreeMap::new();
        let prepared = prepare_final_call_callee(
            authority,
            source.owner,
            FinalCallCalleeFacts::new(
                self.facts.expressions(),
                self.facts.calls(),
                &self.type_reports,
                &enum_variants,
            ),
            &self.catalogs.callable_limits,
        )
        .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
            owner: source.owner,
        })?;

        let staged = self
            .staged_callables
            .as_ref()
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let request = CallResolverRequest::try_new(
            prepared.as_borrowed(),
            &super::CallResolverContext {
                authority,
                checked: (&staged.builder).into(),
                expected: source.expected,
                call_group: CallableGroupIndex::ZERO,
                expression: source.owner,
                cancellation: self.control.cancellation(),
                limits: &self.catalogs.callable_limits,
            },
            &mut work,
        )
        .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
            owner: source.owner,
        })?;
        let callee = request.classification();
        let outcome = resolve_call_target(request);
        let (mut considered, function_value_type, current_group) = match outcome {
            ResolveCallOutcome::Resolved(ResolvedCallTarget::Candidates(candidates)) => (
                candidates.as_slice().to_vec(),
                None,
                candidates.first().call_group(),
            ),
            ResolveCallOutcome::Resolved(ResolvedCallTarget::FunctionValue(value)) => (
                vec![value.callable().clone()],
                Some(value.function_type().clone()),
                value.current_group(),
            ),
            ResolveCallOutcome::Missing(target) => {
                let name = target
                    .path()
                    .map(crate::callable::CallablePath::dotted_name)
                    .or_else(|| target.method().map(|method| method.as_str().to_owned()))
                    .unwrap_or_else(|| "<recovered>".to_owned());
                let lookup = source
                    .module
                    .source_site(
                        source.module.provenance().source_identity(),
                        HirSourceQuery::Expr {
                            owner: source.owner,
                            role: HirExprSourceRole::CallCallee,
                        },
                    )
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                let HirSourcePresence::Present(HirSourceSite::Span(span)) = lookup.presence()
                else {
                    return Err(FinalSemanticAnalysisError::RecoveredOwner);
                };
                return Err(FinalSemanticAnalysisError::UnknownCallTarget {
                    owner: source.owner,
                    kind: target.kind(),
                    name,
                    call_source: span.clone(),
                });
            }
            ResolveCallOutcome::Resolved(ResolvedCallTarget::NonCallable(_))
            | ResolveCallOutcome::Rejected(_) => {
                return Err(FinalSemanticAnalysisError::CallResolutionFailed {
                    owner: source.owner,
                });
            }
        };
        self.specialize_presentation_character_candidates(
            source.module,
            source.owner,
            source.call.arguments(),
            &mut considered,
        )?;
        Ok(ResolvedCallQuery {
            callee,
            considered,
            function_value_type,
            current_group,
            work,
            argument_count,
        })
    }

    fn publish_associated_receiver_recovery(
        &mut self,
        source: CallSource<'_>,
        recovery: AssociatedReceiverRecovery,
        mut work: ResolverWork,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let arguments = self.check_candidate_neutral_arguments(source, &mut work)?;
        work.record_retained_argument_fact_publications(
            u64::try_from(arguments.len())
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
        )
        .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
            owner: source.owner,
        })?;
        let callee = CallCalleeClassificationFact::AssociatedType {
            receiver: recovery.receiver,
            separator: recovery.separator,
        };
        self.stage_associated_receiver_recovery_expression(
            source.module,
            source.call,
            &recovery.result,
        )?;
        let checked =
            CheckedCallTarget::associated_receiver_recovery(arguments, recovery.result.clone());
        let facts = CallTargetFacts::try_new(
            CallTargetFactsInput {
                expression: source.owner,
                enclosing_callable: None,
                callee: Some(callee),
                checked,
                diagnostics: Vec::new(),
                accounting: work.call_accounting(),
            },
            &self.catalogs.callable_limits,
        )
        .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
            owner: source.owner,
        })?;
        if self.facts.set_call_fact(source.owner, facts) {
            return Err(FinalSemanticAnalysisError::CallResolutionFailed {
                owner: source.owner,
            });
        }
        let selection = if source
            .expected
            .is_some_and(|expected| expected.accepts(&recovery.result))
        {
            CheckedTypeSelection::Expected
        } else {
            CheckedTypeSelection::Inferred
        };
        Ok(CheckedExpression::new(
            recovery.result,
            selection,
            EffectSet::new(),
            CheckedExpressionResolution::Call,
        ))
    }

    fn check_candidate_neutral_arguments(
        &mut self,
        source: CallSource<'_>,
        work: &mut ResolverWork,
    ) -> Result<Vec<CheckedCallArgumentFact>, FinalSemanticAnalysisError> {
        let mapping = map_unmapped_call_arguments(source.call.arguments()).ok_or(
            FinalSemanticAnalysisError::CallResolutionFailed {
                owner: source.owner,
            },
        )?;
        let mut arguments = Vec::with_capacity(source.call.arguments().len());
        for (index, mapped) in mapping.arguments().iter().enumerate() {
            let argument = arcweft_lang_hir::expr::HirCallArgumentOrdinal::try_from_usize(index)
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
            let mut slots = Vec::with_capacity(mapped.slots().len());
            for slot in mapped.slots() {
                self.control.check()?;
                work.charge_type_check(1).map_err(|_| {
                    FinalSemanticAnalysisError::CallResolutionFailed {
                        owner: source.owner,
                    }
                })?;
                let inferred = self.check_call_argument_slot(slot.source(), None)?;
                slots.push(CheckedCallArgumentSlotFact::new(
                    CheckedCallArgumentSlotInput {
                        slot: slot.slot(),
                        source: slot.source(),
                        mapped: None,
                        inferred: Some(inferred),
                        expected: None,
                        poison: CallPoison::Clean,
                    },
                ));
            }
            arguments.push(CheckedCallArgumentFact::new(
                argument,
                slots,
                CallPoison::Clean,
            ));
        }
        Ok(arguments)
    }

    fn stage_associated_receiver_recovery_expression(
        &mut self,
        module: &HirModule,
        call: &HirCallExpr,
        receiver_type: &TypeKind,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let HirCallCallee::UnresolvedDot { value_receiver, .. } = call.callee() else {
            return Ok(());
        };
        module
            .resolve_expr(*value_receiver)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if !self.facts.expressions().contains_key(value_receiver) {
            self.facts.set_expression(
                *value_receiver,
                CheckedExpression::new(
                    receiver_type.clone(),
                    CheckedTypeSelection::Inferred,
                    EffectSet::new(),
                    CheckedExpressionResolution::Structural,
                ),
            );
        }
        Ok(())
    }

    fn probe_resolved_call(
        &mut self,
        source: CallSource<'_>,
        resolution: &mut ResolvedCallQuery,
    ) -> Result<CandidateProbeBatch, FinalSemanticAnalysisError> {
        let singleton = resolution.considered.len() == 1;
        let mut singleton_checkpoint = None;
        let mut probes = Vec::with_capacity(resolution.considered.len());
        for candidate in &resolution.considered {
            self.control.check()?;
            resolution
                .work
                .record_candidate_argument_probes(resolution.argument_count)
                .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
                    owner: source.owner,
                })?;
            resolution
                .work
                .charge_argument_mapping(resolution.argument_count)
                .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
                    owner: source.owner,
                })?;
            let candidate_group = if resolution.function_value_type.is_some() {
                resolution.current_group
            } else {
                candidate.call_group()
            };
            let checkpoint = self.facts.begin_candidate_transaction();
            let probe = self.probe_call_candidate(
                CandidateProbeRequest {
                    module: source.module,
                    owner: source.owner,
                    arguments: source.call.arguments(),
                    candidate,
                    current_group: candidate_group,
                    expected_result: source.expected,
                    pass: CandidateEvaluationPass::Probe,
                },
                &mut resolution.work,
            );
            let mut probe = match probe {
                Ok(probe) => probe,
                Err(error) => {
                    self.facts.rollback_candidate_transaction(checkpoint);
                    return Err(error);
                }
            };
            probe.projection = self.facts.capture_candidate_projection(checkpoint);
            if singleton {
                singleton_checkpoint = Some(checkpoint);
            } else {
                self.facts.rollback_candidate_transaction(checkpoint);
            }
            probes.push(probe);
        }
        Ok(CandidateProbeBatch {
            probes,
            singleton_checkpoint,
        })
    }

    #[expect(
        clippy::too_many_lines,
        reason = "selected-call publication validates the complete semantic candidate and argument-accounting record"
    )]
    fn publish_selected_call(
        &mut self,
        source: CallSource<'_>,
        mut resolution: ResolvedCallQuery,
        mut batch: CandidateProbeBatch,
        selected_index: usize,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let selected = batch.probes[selected_index].candidate.clone();
        let current_group = batch.probes[selected_index].current_group;
        let arguments = if let Some(checkpoint) = batch.singleton_checkpoint.take() {
            self.facts.commit_candidate_transaction(checkpoint);
            std::mem::take(&mut batch.probes[selected_index].arguments)
        } else {
            resolution
                .work
                .record_selected_replay_argument_visits(resolution.argument_count)
                .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
                    owner: source.owner,
                })?;
            let checkpoint = self.facts.begin_candidate_transaction();
            let replay = self.commit_call_arguments(
                source.owner,
                source.call.arguments(),
                &selected,
                &batch.probes[selected_index].mapping,
                CandidateEvaluationPass::SelectedReplay,
                &mut resolution.work,
            );
            match replay {
                Ok(arguments) => {
                    self.facts.commit_candidate_transaction(checkpoint);
                    arguments
                }
                Err(error) => {
                    self.facts.rollback_candidate_transaction(checkpoint);
                    return Err(error);
                }
            }
        };
        resolution
            .work
            .record_retained_argument_fact_publications(resolution.argument_count)
            .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
                owner: source.owner,
            })?;

        let result = batch.probes[selected_index].result.clone();
        let effects = provisional_call_effects(&selected, current_group)?;
        let callee_expression = self.stage_resolved_callee_expression(
            source.module,
            source.call,
            &selected,
            &provisional_callable_effects(&selected),
        )?;
        let mut checked_target = CheckedCallTarget::selected(
            &selected,
            &resolution.considered,
            arguments.clone(),
            result.clone(),
            effects.clone(),
            current_group,
            CallPoison::Clean,
        );
        if let Some(function_value_type) = &resolution.function_value_type {
            checked_target = checked_target.with_function_value_type(function_value_type.clone());
        }
        let pending = PendingCallAnalysis {
            expression: source.owner,
            callee_expression,
            enclosing_callable: None,
            callee: resolution.callee,
            selected,
            considered: resolution.considered,
            arguments,
            result: result.clone(),
            current_group,
            function_value_type: resolution.function_value_type,
            accounting: resolution.work.call_accounting(),
        };
        let facts = CallTargetFacts::try_new(
            CallTargetFactsInput {
                expression: pending.expression,
                enclosing_callable: pending.enclosing_callable.clone(),
                callee: Some(pending.callee),
                checked: checked_target,
                diagnostics: Vec::new(),
                accounting: pending.accounting,
            },
            &self.catalogs.callable_limits,
        )
        .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
            owner: source.owner,
        })?;
        if self.facts.set_pending_call(source.owner, pending)
            || self.facts.set_call_fact(source.owner, facts)
        {
            return Err(FinalSemanticAnalysisError::CallResolutionFailed {
                owner: source.owner,
            });
        }
        Ok(CheckedExpression::new(
            result,
            if source.expected.is_some() {
                CheckedTypeSelection::Expected
            } else {
                CheckedTypeSelection::Inferred
            },
            effects.concrete().clone(),
            CheckedExpressionResolution::Call,
        ))
    }

    fn publish_ambiguous_call(
        &mut self,
        source: CallSource<'_>,
        resolution: ResolvedCallQuery,
        mut batch: CandidateProbeBatch,
        primary: usize,
        tied: Vec<usize>,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let ResolvedCallQuery {
            callee,
            considered,
            work,
            ..
        } = resolution;
        if let Some(checkpoint) = batch.singleton_checkpoint.take() {
            self.facts.rollback_candidate_transaction(checkpoint);
        }
        let projection = std::mem::take(&mut batch.probes[primary].projection);
        self.facts.apply_candidate_projection(projection);
        let candidates = tied
            .into_iter()
            .map(|index| batch.probes[index].candidate.clone())
            .collect::<Vec<_>>();
        let arguments = std::mem::take(&mut batch.probes[primary].arguments);
        let result = batch.probes[primary].result.clone();
        self.publish_recovery_call(RecoveryCall {
            source,
            callee,
            candidates: &candidates,
            considered: &considered,
            current_group: batch.probes[primary].current_group,
            arguments,
            result,
            work,
            ambiguous: true,
        })
    }

    fn publish_rejected_call(
        &mut self,
        source: CallSource<'_>,
        mut resolution: ResolvedCallQuery,
        mut batch: CandidateProbeBatch,
        primary: usize,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let arguments = if let Some(checkpoint) = batch.singleton_checkpoint.take() {
            self.facts.rollback_candidate_transaction(checkpoint);
            let checkpoint = self.facts.begin_candidate_transaction();
            let replay = self.replay_rejected_call_arguments(
                source.owner,
                source.call.arguments(),
                &batch.probes[primary].candidate,
                &batch.probes[primary].mapping,
                batch.probes[primary].shape_rejected,
                &mut resolution.work,
            );
            match replay {
                Ok(arguments) => {
                    self.facts.commit_candidate_transaction(checkpoint);
                    arguments
                }
                Err(error) => {
                    self.facts.rollback_candidate_transaction(checkpoint);
                    return Err(error);
                }
            }
        } else {
            let projection = std::mem::take(&mut batch.probes[primary].projection);
            self.facts.apply_candidate_projection(projection);
            std::mem::take(&mut batch.probes[primary].arguments)
        };
        let result = batch.probes[primary].result.clone();
        self.publish_recovery_call(RecoveryCall {
            source,
            callee: resolution.callee,
            candidates: &resolution.considered,
            considered: &resolution.considered,
            current_group: batch.probes[primary].current_group,
            arguments,
            result,
            work: resolution.work,
            ambiguous: false,
        })
    }

    fn publish_recovery_call(
        &mut self,
        recovery: RecoveryCall<'_>,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let RecoveryCall {
            source,
            callee,
            candidates,
            considered,
            current_group,
            arguments,
            result,
            mut work,
            ambiguous,
        } = recovery;
        let primary =
            candidates
                .first()
                .ok_or(FinalSemanticAnalysisError::CallResolutionFailed {
                    owner: source.owner,
                })?;
        work.record_retained_argument_fact_publications(
            u64::try_from(arguments.len())
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
        )
        .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
            owner: source.owner,
        })?;
        self.stage_resolved_callee_expression(
            source.module,
            source.call,
            primary,
            &provisional_callable_effects(primary),
        )?;
        let checked = if ambiguous {
            CheckedCallTarget::ambiguous(
                candidates,
                considered,
                arguments,
                result.clone(),
                current_group,
            )
        } else {
            CheckedCallTarget::rejected(candidates, arguments, result.clone(), current_group)
        };
        let facts = CallTargetFacts::try_new(
            CallTargetFactsInput {
                expression: source.owner,
                enclosing_callable: None,
                callee: Some(callee),
                checked,
                diagnostics: Vec::new(),
                accounting: work.call_accounting(),
            },
            &self.catalogs.callable_limits,
        )
        .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
            owner: source.owner,
        })?;
        if self.facts.set_call_fact(source.owner, facts) {
            return Err(FinalSemanticAnalysisError::CallResolutionFailed {
                owner: source.owner,
            });
        }
        let selection = if source
            .expected
            .is_some_and(|expected| expected.accepts(&result))
        {
            CheckedTypeSelection::Expected
        } else {
            CheckedTypeSelection::Inferred
        };
        Ok(CheckedExpression::new(
            result,
            selection,
            EffectSet::new(),
            CheckedExpressionResolution::Call,
        ))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "callee child staging exhaustively follows the final-HIR callee variants and their typed owners"
    )]
    fn stage_call_callee_children(
        &mut self,
        module: &HirModule,
        call: &HirCallExpr,
    ) -> Result<Option<AssociatedReceiverRecovery>, FinalSemanticAnalysisError> {
        match call.callee() {
            HirCallCallee::Value { value } => {
                let expression = module
                    .resolve_expr(*value)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                if !matches!(expression.kind(), HirExprKind::Path(_)) {
                    self.check_expression(*value, None)?;
                } else if let HirExprKind::Path(path) = expression.kind() {
                    let path = path
                        .as_resolved()
                        .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
                    if let Ok(Some(resolution)) =
                        self.resolve_path_value(module, *value, expression.scope(), path)
                    {
                        let ty = match &resolution {
                            CheckedValueResolution::Local(local) => {
                                Some(self.facts.locals().get(local).cloned().ok_or(
                                    FinalSemanticAnalysisError::LocalTypeUnavailable {
                                        owner: *local,
                                    },
                                )?)
                            }
                            CheckedValueResolution::ProjectCallable(_) => None,
                            _ => Some(
                                value_resolution_type(self.catalogs.world, &resolution).ok_or(
                                    FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                                        owner: *value,
                                    },
                                )?,
                            ),
                        };
                        if let Some(ty) = ty
                            && !self.facts.expressions().contains_key(value)
                        {
                            self.facts.set_expression(
                                *value,
                                CheckedExpression::new(
                                    ty,
                                    CheckedTypeSelection::Inferred,
                                    EffectSet::new(),
                                    CheckedExpressionResolution::Value(resolution),
                                ),
                            );
                        }
                    }
                }
            }
            HirCallCallee::UnresolvedDot {
                value_receiver,
                nominal_receiver,
                separator,
                member,
            } => {
                let expression = module
                    .resolve_expr(*value_receiver)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                if !matches!(expression.kind(), HirExprKind::Path(_)) {
                    self.check_expression(*value_receiver, None)?;
                } else if let HirExprKind::Path(path) = expression.kind() {
                    let path = path
                        .as_resolved()
                        .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
                    let member = member
                        .resolved()
                        .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
                    let full_path = path.with_terminal_member(member);
                    let full_resolution = self.resolve_path_value(
                        module,
                        *value_receiver,
                        expression.scope(),
                        &full_path,
                    )?;
                    match full_resolution {
                        Some(resolution) => {
                            if let Some(ty) =
                                value_resolution_type(self.catalogs.world, &resolution)
                                && !self.facts.expressions().contains_key(value_receiver)
                            {
                                self.facts.set_expression(
                                    *value_receiver,
                                    CheckedExpression::new(
                                        ty,
                                        CheckedTypeSelection::Inferred,
                                        EffectSet::new(),
                                        CheckedExpressionResolution::Value(resolution),
                                    ),
                                );
                            }
                        }
                        None => {
                            match self.resolve_path_value(
                                module,
                                *value_receiver,
                                expression.scope(),
                                path,
                            )? {
                                Some(resolution) => {
                                    if let Some(ty) =
                                        value_resolution_type(self.catalogs.world, &resolution)
                                        && !self.facts.expressions().contains_key(value_receiver)
                                    {
                                        self.facts.set_expression(
                                            *value_receiver,
                                            CheckedExpression::new(
                                                ty,
                                                CheckedTypeSelection::Inferred,
                                                EffectSet::new(),
                                                CheckedExpressionResolution::Value(resolution),
                                            ),
                                        );
                                    }
                                }
                                None => {
                                    if prepare_language_free_dot_path(
                                        *value_receiver,
                                        expression,
                                        member,
                                        &self.catalogs.callable_limits,
                                    )
                                    .map_err(|_| {
                                        FinalSemanticAnalysisError::CallResolutionFailed {
                                            owner: *value_receiver,
                                        }
                                    })?
                                    .is_none()
                                    {
                                        let receiver = nominal_receiver.type_id().ok_or(
                                            FinalSemanticAnalysisError::CallResolutionFailed {
                                                owner: *value_receiver,
                                            },
                                        )?;
                                        match self.resolve_associated_receiver_type(receiver)? {
                                            AssociatedReceiverTypeResolution::Complete(_) => {}
                                            AssociatedReceiverTypeResolution::WrongArity(
                                                result,
                                            ) => {
                                                return Ok(Some(AssociatedReceiverRecovery {
                                                    receiver,
                                                    separator: *separator,
                                                    result,
                                                }));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            HirCallCallee::Associated {
                receiver,
                separator,
                ..
            } => {
                let receiver = receiver
                    .type_id()
                    .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
                match self.resolve_associated_receiver_type(receiver)? {
                    AssociatedReceiverTypeResolution::Complete(_) => {}
                    AssociatedReceiverTypeResolution::WrongArity(result) => {
                        return Ok(Some(AssociatedReceiverRecovery {
                            receiver,
                            separator: *separator,
                            result,
                        }));
                    }
                }
            }
        }
        Ok(None)
    }

    fn specialize_presentation_character_candidates(
        &mut self,
        module: &HirModule,
        call: ExprId,
        arguments: &[HirCallArgument],
        candidates: &mut [ResolvedCallable],
    ) -> Result<(), FinalSemanticAnalysisError> {
        for candidate in candidates {
            let Some(owner) = self.presentation_character_owner(module, arguments, candidate)
            else {
                continue;
            };
            *candidate = candidate
                .try_with_presentation_character_owner(
                    owner,
                    self.catalogs.world.environment(),
                    &self.catalogs.callable_limits,
                )
                .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed { owner: call })?;
        }
        Ok(())
    }

    fn presentation_character_owner(
        &mut self,
        module: &HirModule,
        arguments: &[HirCallArgument],
        candidate: &ResolvedCallable,
    ) -> Option<ResolvedCharacterOwner> {
        let group = candidate.schema().group(candidate.call_group())?;
        let parameter = group.parameters().iter().find(|parameter| {
            parameter
                .name()
                .is_some_and(|name| name.as_str() == "character")
        })?;
        let mapping = map_call_arguments(
            module,
            candidate.schema(),
            candidate.call_group(),
            arguments,
            None,
        )?;
        let argument = mapping
            .arguments()
            .iter()
            .position(|argument| {
                argument.slots().iter().any(|slot| {
                    slot.coordinate().is_some_and(|coordinate| {
                        coordinate.group() == group.index()
                            && coordinate.parameter() == parameter.index()
                    })
                })
            })
            .and_then(|index| arguments.get(index))?;
        let checked = self.check_expression(argument.value(), None).ok()?;
        let CheckedExpressionResolution::Value(CheckedValueResolution::ProjectItem(item)) =
            checked.resolution()
        else {
            return None;
        };
        let character = item.character()?;
        self.catalogs
            .world
            .environment()
            .character_manifest(&character)?;
        Some(ResolvedCharacterOwner::new(
            character,
            CharacterOwnerSource::EntityReference,
        ))
    }

    fn probe_call_candidate(
        &mut self,
        request: CandidateProbeRequest<'_>,
        work: &mut ResolverWork,
    ) -> Result<CandidateProbe, FinalSemanticAnalysisError> {
        let CandidateProbeRequest {
            module,
            owner,
            arguments,
            candidate,
            current_group,
            expected_result,
            pass,
        } = request;
        let implicit = match candidate.instantiation() {
            CallableInstantiation::DataLast {
                group, parameter, ..
            } if *group == current_group => Some(*parameter),
            _ => None,
        };
        let mapped = map_call_arguments(
            module,
            candidate.schema(),
            current_group,
            arguments,
            implicit,
        );
        let shape_rejected = mapped.is_none();
        let mapping = mapped
            .or_else(|| map_unmapped_call_arguments(arguments))
            .ok_or(FinalSemanticAnalysisError::CallResolutionFailed { owner })?;
        let mut score = CandidateScore {
            hard_errors: usize::from(shape_rejected),
            exact_matches: 0,
            unchecked_or_open: mapping.unchecked_or_open_slots(),
            omitted_parameters: mapping.omitted_parameters(),
            authority: candidate.authority(),
        };
        let evaluated = self.evaluate_call_arguments(
            ArgumentEvaluationRequest {
                call: owner,
                authored: arguments,
                candidate,
                mapping: &mapping,
                pass,
                shape_rejected,
            },
            work,
        )?;
        score.exact_matches = score
            .exact_matches
            .checked_add(evaluated.exact_matches)
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
        score.hard_errors = score
            .hard_errors
            .checked_add(evaluated.hard_errors)
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
        let result = evaluated.substitutions.apply(
            &call_result_type(candidate, current_group)
                .ok_or(FinalSemanticAnalysisError::CallResolutionFailed { owner })?,
        );
        if let Some(expected) = expected_result {
            if expected.accepts(&result) {
                if expected == &result {
                    score.exact_matches = score
                        .exact_matches
                        .checked_add(1)
                        .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
                }
            } else {
                score.hard_errors = score
                    .hard_errors
                    .checked_add(1)
                    .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
            }
        }
        Ok(CandidateProbe {
            candidate: candidate.clone(),
            current_group,
            mapping,
            arguments: evaluated.arguments,
            projection: CandidateSemanticProjection::default(),
            result,
            score,
            shape_rejected,
        })
    }

    fn check_call_argument_slot(
        &mut self,
        source: CheckedCallArgumentSlotSource,
        expected: Option<&TypeKind>,
    ) -> Result<TypeKind, FinalSemanticAnalysisError> {
        match source {
            CheckedCallArgumentSlotSource::Expression(expression) => self
                .check_expression(expression, expected)
                .map(|checked| checked.ty().clone()),
            CheckedCallArgumentSlotSource::CompactNumericElement { sequence, ordinal } => {
                self.compact_numeric_element_type(sequence, ordinal, expected)
            }
        }
    }

    fn evaluate_call_arguments(
        &mut self,
        request: ArgumentEvaluationRequest<'_>,
        work: &mut ResolverWork,
    ) -> Result<EvaluatedCallArguments, FinalSemanticAnalysisError> {
        let ArgumentEvaluationRequest {
            call,
            authored,
            candidate,
            mapping,
            pass,
            shape_rejected,
        } = request;
        if mapping.arguments().len() != authored.len() {
            return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner: call });
        }
        let mut arguments = Vec::with_capacity(authored.len());
        let mut hard_errors = 0usize;
        let mut exact_matches = 0usize;
        let mut substitutions = TypeParameterSubstitutions::default();
        for (argument_index, (authored, mapped)) in
            authored.iter().zip(mapping.arguments()).enumerate()
        {
            let argument =
                arcweft_lang_hir::expr::HirCallArgumentOrdinal::try_from_usize(argument_index)
                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
            let evaluated = self.evaluate_mapped_call_argument(
                MappedArgumentEvaluationRequest {
                    call,
                    authored,
                    mapped,
                    candidate,
                    pass,
                    shape_rejected,
                    argument,
                },
                work,
                &mut substitutions,
            )?;
            hard_errors = hard_errors
                .checked_add(evaluated.hard_errors)
                .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
            exact_matches = exact_matches
                .checked_add(evaluated.exact_matches)
                .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
            arguments.push(evaluated.fact);
        }
        Ok(EvaluatedCallArguments {
            arguments,
            hard_errors,
            exact_matches,
            substitutions,
        })
    }

    fn evaluate_mapped_call_argument(
        &mut self,
        request: MappedArgumentEvaluationRequest<'_>,
        work: &mut ResolverWork,
        substitutions: &mut TypeParameterSubstitutions,
    ) -> Result<MappedArgumentEvaluation, FinalSemanticAnalysisError> {
        let MappedArgumentEvaluationRequest {
            call,
            authored,
            mapped,
            candidate,
            pass,
            shape_rejected,
            argument,
        } = request;
        let mut slots = Vec::with_capacity(mapped.slots().len());
        let mut hard_errors = 0usize;
        let mut exact_matches = 0usize;
        let mut argument_poison = if shape_rejected {
            CallPoison::Rejected
        } else {
            CallPoison::Clean
        };
        for slot in mapped.slots() {
            self.control.check_physical_slot_boundary()?;
            work.charge_type_check(1)
                .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed { owner: call })?;
            self.facts.prepare_physical_slot_evaluation(slot.source());
            let declared_expected = slot.expected();
            let expected = declared_expected.map(|expected| substitutions.apply(expected));
            let physical_expected = physical_expected_type(
                expected.as_ref(),
                slot.coordinate().is_some(),
                shape_rejected,
            );
            let kind = physical_evaluation_kind(
                authored,
                slot,
                shape_rejected,
                candidate.schema().argument_policy().spread(),
            );
            self.record_physical_candidate_argument_evaluation(
                PhysicalCandidateArgumentEvaluation::new(
                    call,
                    candidate.id().clone(),
                    pass,
                    PhysicalCandidateArgument::new(
                        argument,
                        slot.slot(),
                        slot.source(),
                        kind,
                        physical_expected,
                    ),
                ),
            )?;
            let inferred = self.check_call_argument_slot(slot.source(), expected.as_ref())?;
            let substitution_conflict = declared_expected
                .is_some_and(|declared| !substitutions.observe(declared, &inferred));
            let retained_expected = declared_expected.map(|expected| substitutions.apply(expected));
            let mismatch = substitution_conflict
                || retained_expected
                    .as_ref()
                    .is_some_and(|expected| !expected.accepts(&inferred));
            if mismatch {
                hard_errors = hard_errors
                    .checked_add(1)
                    .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
            } else if retained_expected.as_ref() == Some(&inferred) {
                exact_matches = exact_matches
                    .checked_add(1)
                    .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
            }
            let poison = if shape_rejected || mismatch {
                CallPoison::Rejected
            } else {
                CallPoison::Clean
            };
            argument_poison = argument_poison.merge(poison);
            slots.push(CheckedCallArgumentSlotFact::new(
                CheckedCallArgumentSlotInput {
                    slot: slot.slot(),
                    source: slot.source(),
                    mapped: slot.coordinate(),
                    inferred: Some(inferred),
                    expected: retained_expected,
                    poison,
                },
            ));
        }
        Ok(MappedArgumentEvaluation {
            fact: CheckedCallArgumentFact::new(argument, slots, argument_poison),
            hard_errors,
            exact_matches,
        })
    }

    fn compact_numeric_element_type(
        &self,
        owner: ExprId,
        ordinal: u32,
        expected: Option<&TypeKind>,
    ) -> Result<TypeKind, FinalSemanticAnalysisError> {
        let module = self.module(owner.module())?;
        let expression = module
            .resolve_expr(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirExprKind::NumericBracketSequence(sequence) = expression.kind() else {
            return Err(FinalSemanticAnalysisError::InvalidOwner);
        };
        let ordinal =
            usize::try_from(ordinal).map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        sequence
            .elements()
            .get(ordinal)
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        Ok(infer_compact_numeric_element_type(
            sequence.common_suffix(),
            expected,
        ))
    }

    fn commit_call_arguments(
        &mut self,
        call: ExprId,
        arguments: &[HirCallArgument],
        candidate: &ResolvedCallable,
        mapping: &CallArgumentMapping,
        pass: CandidateEvaluationPass,
        work: &mut ResolverWork,
    ) -> Result<Vec<CheckedCallArgumentFact>, FinalSemanticAnalysisError> {
        let evaluated = self.evaluate_call_arguments(
            ArgumentEvaluationRequest {
                call,
                authored: arguments,
                candidate,
                mapping,
                pass,
                shape_rejected: false,
            },
            work,
        )?;
        if evaluated.hard_errors != 0 {
            return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner: call });
        }
        Ok(evaluated.arguments)
    }

    fn replay_rejected_call_arguments(
        &mut self,
        call: ExprId,
        arguments: &[HirCallArgument],
        candidate: &ResolvedCallable,
        mapping: &CallArgumentMapping,
        shape_rejected: bool,
        work: &mut ResolverWork,
    ) -> Result<Vec<CheckedCallArgumentFact>, FinalSemanticAnalysisError> {
        self.evaluate_call_arguments(
            ArgumentEvaluationRequest {
                call,
                authored: arguments,
                candidate,
                mapping,
                pass: CandidateEvaluationPass::RejectedRecoveryReplay,
                shape_rejected,
            },
            work,
        )
        .map(|evaluated| evaluated.arguments)
    }

    fn stage_resolved_callee_expression(
        &mut self,
        module: &HirModule,
        call: &HirCallExpr,
        selected: &ResolvedCallable,
        callable_effects: &EffectRow,
    ) -> Result<Option<ExprId>, FinalSemanticAnalysisError> {
        let (value, nominal_receiver) = match call.callee() {
            HirCallCallee::Value { value } => (*value, false),
            HirCallCallee::UnresolvedDot { value_receiver, .. }
                if !self.facts.expressions().contains_key(value_receiver) =>
            {
                (*value_receiver, true)
            }
            HirCallCallee::UnresolvedDot { .. } | HirCallCallee::Associated { .. } => {
                return Ok(None);
            }
        };
        let ty = if nominal_receiver {
            match selected.instantiation() {
                CallableInstantiation::TypeReceiver { receiver } => receiver.receiver().clone(),
                _ => selected.schema().result().clone(),
            }
        } else {
            callable_schema_type_with_effects(selected.schema(), callable_effects)
                .ok_or(FinalSemanticAnalysisError::CallResolutionFailed { owner: value })?
        };
        let resolution = if let Some(existing) = self.facts.expressions().get(&value) {
            if nominal_receiver {
                return Ok(None);
            }
            match existing.resolution() {
                CheckedExpressionResolution::Value(
                    CheckedValueResolution::ProjectCallable(_)
                    | CheckedValueResolution::Registered(_),
                ) => existing.resolution().clone(),
                _ => return Ok(None),
            }
        } else if nominal_receiver {
            CheckedExpressionResolution::Structural
        } else if let crate::callable::CallableCandidateId::Project(declaration) = selected.id() {
            let symbol = self
                .symbols
                .callable(declaration)
                .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
            CheckedExpressionResolution::Value(CheckedValueResolution::ProjectCallable(
                super::CheckedProjectCallable::new(declaration.clone(), symbol.source_item()),
            ))
        } else {
            CheckedExpressionResolution::Value(CheckedValueResolution::Registered(
                RegisteredSemanticValueId::from_bytes(
                    *selected.schema().semantic_digest().as_bytes(),
                ),
            ))
        };
        let _ = module
            .resolve_expr(value)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        self.facts.set_expression(
            value,
            CheckedExpression::new(
                ty,
                CheckedTypeSelection::Inferred,
                EffectSet::new(),
                resolution,
            ),
        );
        Ok((!nominal_receiver).then_some(value))
    }
}
