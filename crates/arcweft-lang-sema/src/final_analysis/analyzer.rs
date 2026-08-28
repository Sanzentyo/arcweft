//! Sole production construction path for [`FinalSemanticAnalysis`].
//!
//! This pass consumes the exact, accepted arena inventories.  It never opens
//! source text, constructs a detached HIR, or publishes a partial report.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::Arc,
};

use arcweft_lang_hir::{
    dialogue_application::{HirLinePlanItem, HirPostfixBracket, HirPostfixBracketCandidates},
    expr::{
        HirAssociatedSeparator, HirAwaitBranchKind, HirBinaryOp, HirBorrowKind, HirCallArgument,
        HirCallCallee, HirCallExpr, HirCallValue, HirChoiceCompactAction, HirChoiceItem,
        HirComputationBlockKind, HirExpr, HirExprKind, HirRecordField, HirRecoveredName,
        HirSelectedMember, HirUnaryOp,
    },
    identity::{ExprId, HirModuleId, ItemId, LocalId, PatternId, ScopeId, StmtId, TypeId},
    item::{
        HirFlowContractClause, HirFunctionBody, HirFunctionItem, HirGenericParameter,
        HirImplMember, HirItem, HirItemKind, HirPredicateBody, HirProofBody, HirStyleBodyItem,
        HirTraitMember,
    },
    leaf::{
        HirFloatLiteral, HirFloatWidth, HirIdRef, HirIdRefValue, HirIntegerLiteral, HirLiteral,
        HirName, HirPathRoot, HirPathSegment,
    },
    module::HirModule,
    pattern::{
        HirPatternBinding, HirPatternField, HirPatternKind, HirPatternRecordPath,
        HirPatternSequenceRest, HirVariantPattern, HirVariantPatternHead,
        HirVariantPatternHeadValue, HirVariantPatternName, HirVariantPatternPayload,
    },
    project::{HirExecutableProjectView, HirProjectEvaluationTopology, HirProjectView},
    scope::{HirScopeKind, HirScopeOwner, LocalLookup},
    source_index::{
        HirCallArgumentSourcePart, HirCallableEffectSourcePart, HirCallableSourceOwner,
        HirCallableSourceRole, HirExprSourceRole, HirFlowContractSourcePart, HirFlowSourceRole,
        HirItemSourceRole, HirPatternSourceRole, HirScopeSourceRole, HirSourcePresence,
        HirSourceQuery, HirSourceSite, HirTypeSourceRole,
    },
    stmt::{HirAssertionMode, HirStmtKind, HirTriggerPattern},
    symbol::{
        CallableDeclarationKey, CallableDeclarationOwner, ProjectSymbolTable, ProjectTypeTarget,
        ProjectValueLookup, ResolvedProjectSymbol,
        nominal::{ProjectNominalBody, ProjectNominalDeclaration},
    },
    type_ref::HirTypeKind,
};
use arcweft_lang_syntax::{ast::module_path::ModuleSegment, reference::BorrowKind};
use arcweft_source::SourceSpan;

use crate::{
    assertion::AssertionContext,
    callable::{
        CallCalleeClassificationFact, CallResolverAuthority, CallResolverContext,
        CallResolverRequest, CallTargetFacts, CallableAccess, CallableAuthorityRank,
        CallableCandidateId, CallableEffectContract, CallableEffectSchema, CallableGroupIndex,
        CallableInstantiation, CallableMethodRole, CharacterOwnerSource,
        CheckedCallArgumentSlotSource, CheckedCallableCatalog, CheckedCallableCatalogBuildError,
        CheckedCallableCatalogBuilder, CheckedCallableDeclaration, CheckedCallableExecution,
        CheckedCallableId, CheckedClosureId, EffectClauseSource, EffectItemSource,
        FinalCallCalleeFacts, MappedCallArgumentSlot, PreparedResolvedCallable, ResolveCallOutcome,
        ResolvedCallTarget, ResolvedCharacterOwner, ResolverWork, STANDARD_TRAIT_CATALOG_VERSION,
        map_call_arguments, prepare_final_call_callee, prepare_language_free_dot_path,
        resolve_call_target,
    },
    callable::{
        CallableLimits, CallableValidator, PRODUCTION_CALLABLE_LIMITS, SpreadArgumentPolicy,
    },
    checked_rich_text::RichTextAttributeChecker,
    effect_row::{EffectRow, EffectSubsetError, EffectSubstitution},
    effects::{EffectId, EffectSet},
    env::{EnumVariantPayload, EnvironmentEnumSchema, TypeCheckEnv},
    nominal::{
        BuiltinTypeConstructor, GenericTypeBinding, GenericTypeScope, NominalResolutionLimits,
        ResolvedTypeRefOutcome, SelfTypeScope, TypeNameResolution, TypeResolutionFailure,
        TypeResolutionInput, TypeResolutionReport, TypeSourceEvidence, resolve_type_ref,
    },
    registration::RegisteredSemanticWorld,
    semantic_coordinate::{AcceptedSemanticRootCatalog, SemanticCoordinateIndex},
    types::{
        ArrayLength, CharacterDialogueCharacterType, EntityKind, GenericParameterOwnerId,
        GenericTypeParameterId, IteratorStateKind, ProjectNominalType, TypeKind,
        TypeParameterSubstitutions,
    },
};

use super::{
    CandidateFactTransactionViolation, CharacterDialogueFieldCoordinate,
    CharacterDialoguePatchContext, CheckedAssertionDisposition, CheckedAwait,
    CheckedAwaitPendingObserver, CheckedBinding, CheckedBindingRole,
    CheckedCharacterDialogueFactory, CheckedCharacterDialoguePatch,
    CheckedCharacterDialoguePatchField, CheckedCharacterDialogueReconfigure,
    CheckedCharacterDialogueTarget, CheckedChoice, CheckedChoiceGoto, CheckedClosure,
    CheckedDialogueEffectOperation, CheckedDialogueEffectSite, CheckedDialogueEffectSiteOrdinal,
    CheckedDialogueEffectTrigger, CheckedDialogueLinePlan, CheckedDialogueMarkHandler,
    CheckedDialogueMarkOrdinal, CheckedEvaluatedEffect, CheckedExpression,
    CheckedExpressionResolution, CheckedFunctionExecution, CheckedImplicitCallable, CheckedItem,
    CheckedItemRole, CheckedIteration, CheckedIteratorFamily, CheckedPatchOperation,
    CheckedPattern, CheckedPatternResolution, CheckedPipe, CheckedProjectCallable,
    CheckedProjectItem, CheckedProjectNominal, CheckedSelectResolution, CheckedStageLook,
    CheckedStatement, CheckedStatementRole, CheckedStyleCallee, CheckedSuspensionRole,
    CheckedSuspensionStatement, CheckedTraitConformance, CheckedTraitIdentity, CheckedTry,
    CheckedTryBoundary, CheckedTryCarrier, CheckedTypeSelection, CheckedTypedBinding,
    CheckedValueResolution, CheckedVariantOwner, CheckedVariantResolution, CheckedViewCall,
    CheckedViewCallee, FinalSemanticAnalysis, FinalSemanticAnalysisControl,
    FinalSemanticAnalysisError, FinalSemanticAnalysisInput, PhysicalArgumentEvaluationKind,
    PhysicalCandidateArgumentEvaluation, PostfixBracketResolution, PreparedAssignmentStatement,
    PreparedEntryExpression, PreparedEntryReference, PreparedExpressionFact,
    PreparedExpressionShell, PreparedPatternFact, PreparedProjectVariantExpression,
    PreparedProjectVariantOwnerSeed, PreparedProjectVariantPattern, PreparedStatementFact,
    PreparedVariantCaseSeed, ProjectHirSymbolLookupError, ProjectSymbolResolutionError,
    RecursiveCallableContractEdge, RegisteredSemanticValueId, SemanticFactFamily,
};

/// Immutable catalogs used by the one accepted semantic pass.
#[derive(Clone, Copy)]
pub struct FinalSemanticCatalogs<'a> {
    world: &'a RegisteredSemanticWorld,
    callable_limits: CallableLimits,
}

impl<'a> FinalSemanticCatalogs<'a> {
    /// Creates the production catalog bundle with the compiled inclusive call
    /// limits.  The analyzer validates the exact symbol/world lease before it
    /// performs any work.
    pub const fn production(world: &'a RegisteredSemanticWorld) -> Self {
        Self {
            world,
            callable_limits: PRODUCTION_CALLABLE_LIMITS,
        }
    }

    pub const fn world(self) -> &'a RegisteredSemanticWorld {
        self.world
    }

    pub const fn callable_limits(self) -> CallableLimits {
        self.callable_limits
    }

    #[cfg(test)]
    pub(crate) const fn with_callable_limits(mut self, limits: CallableLimits) -> Self {
        self.callable_limits = limits;
        self
    }
}

/// Analyzes and atomically publishes one complete executable HIR generation.
///
/// This is the sole public constructor.  Staging types remain crate-private so
/// compiler, LSP, runtime, and tests cannot publish hand-assembled facts.
pub fn analyze_final_project(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    catalogs: FinalSemanticCatalogs<'_>,
    control: FinalSemanticAnalysisControl<'_>,
) -> Result<FinalSemanticAnalysis, super::FinalSemanticProjectError> {
    Analyzer::new(project, symbols, catalogs, control)?.analyze()
}

#[cfg(test)]
pub(super) fn freeze_checked_callables_for_test(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    catalogs: FinalSemanticCatalogs<'_>,
    input: &FinalSemanticAnalysisInput,
) -> Result<
    (
        Arc<HirProjectEvaluationTopology>,
        Arc<CheckedCallableCatalog>,
    ),
    FinalSemanticAnalysisError,
> {
    let cancellation = std::sync::atomic::AtomicBool::new(false);
    let analyzer = Analyzer::new(
        project,
        symbols,
        catalogs,
        FinalSemanticAnalysisControl::new(&cancellation),
    )?;
    let topology = Arc::clone(&analyzer.topology);
    let callables = analyzer.freeze_checked_callables(input)?;
    Ok((topology, callables))
}

#[cfg(test)]
pub(super) fn analyze_final_project_with_physical_trace_for_test(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    catalogs: FinalSemanticCatalogs<'_>,
    control: FinalSemanticAnalysisControl<'_>,
) -> (
    Result<FinalSemanticAnalysis, FinalSemanticAnalysisError>,
    Vec<PhysicalCandidateArgumentEvaluation>,
) {
    let mut analyzer = match Analyzer::new(project, symbols, catalogs, control) {
        Ok(analyzer) => analyzer,
        Err(error) => return (Err(error), Vec::new()),
    };
    let result = analyzer.analyze_staged();
    let physical = match &result {
        Ok(report) => report
            .physical_candidate_argument_evaluations()
            .cloned()
            .collect(),
        Err(_) => analyzer
            .facts
            .physical_candidate_argument_evaluations()
            .values()
            .flat_map(|evaluations| evaluations.iter().cloned())
            .collect(),
    };
    (
        result.map_err(super::FinalSemanticProjectError::into_semantic_fixture_error),
        physical,
    )
}

struct Analyzer<'project, 'catalog, 'control> {
    executable: HirExecutableProjectView<'project>,
    project: HirProjectView<'project>,
    symbols: &'catalog ProjectSymbolTable,
    catalogs: FinalSemanticCatalogs<'catalog>,
    control: FinalSemanticAnalysisControl<'control>,
    topology: Arc<HirProjectEvaluationTopology>,
    modules: BTreeMap<HirModuleId, &'project HirModule>,
    style_value_kinds: BTreeMap<ExprId, arcweft_view::style::ViewStyleValueKind>,
    types: BTreeMap<TypeId, TypeKind>,
    type_reports: BTreeMap<TypeId, TypeResolutionReport>,
    facts: SemanticFactState,
    staged_callables: Option<StagedCheckedCallables>,
    call_frames: Rc<expression_error::CallFrameStack>,
    implicit_callable_stack: Vec<ImplicitCallableContext>,
    pipe_stack: Vec<PipeContext>,
    function_site_stack: Vec<FunctionSiteContext>,
}

struct ImplicitCallableContext {
    owner: ExprId,
    parameter: TypeKind,
    result: Option<TypeKind>,
    placeholders: Box<[ExprId]>,
}

struct PipeContext {
    owner: ExprId,
    left: ExprId,
    right: ExprId,
    value: TypeKind,
    placeholders: BTreeSet<ExprId>,
}

struct FunctionSiteContext {
    owner: ExprId,
    result: TypeKind,
}

struct StagedCheckedCallables {
    builder: CheckedCallableCatalogBuilder,
    bodies: Vec<StagedCallableBody>,
    effect_expressions: Vec<(ExprId, CheckedExpression)>,
    accepted: Arc<crate::callable::RegisteredCallableCatalog>,
}

#[derive(Clone)]
struct StagedCallableBody {
    id: CheckedCallableId,
    module: HirModuleId,
    item: ItemId,
    scope: ScopeId,
    owner: CallableDeclarationOwner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AcceptedCandidateRank {
    exact_matches: usize,
    declared_exact_matches: usize,
    unchecked_or_open: usize,
    omitted_parameters: usize,
    authority: Option<CallableAuthorityRank>,
}

enum CandidateSelection {
    Selected(usize),
    Ambiguous { primary: usize, tied: Vec<usize> },
    Rejected { primary: usize },
}

fn collect_style_value_kinds(
    modules: &BTreeMap<HirModuleId, &HirModule>,
) -> Result<BTreeMap<ExprId, arcweft_view::style::ViewStyleValueKind>, FinalSemanticAnalysisError> {
    let mut values = BTreeMap::new();
    for module in modules.values() {
        for (_, item) in module.items() {
            let HirItemKind::Style(style) = item.kind() else {
                continue;
            };
            collect_style_body_value_kinds(style.body(), &mut values)?;
        }
    }
    Ok(values)
}

fn collect_style_body_value_kinds(
    body: &[HirStyleBodyItem],
    values: &mut BTreeMap<ExprId, arcweft_view::style::ViewStyleValueKind>,
) -> Result<(), FinalSemanticAnalysisError> {
    for item in body {
        match item {
            HirStyleBodyItem::Rule(rule) => {
                for declaration in rule.declarations() {
                    let Some(property) = declaration.property().as_str() else {
                        continue;
                    };
                    let Some(property) =
                        arcweft_view::style::ViewPropertyKind::from_source_name(property)
                    else {
                        continue;
                    };
                    if values
                        .insert(declaration.value(), property.value_kind())
                        .is_some()
                    {
                        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                    }
                }
            }
            HirStyleBodyItem::Environment(environment) => {
                collect_style_body_value_kinds(environment.body(), values)?;
            }
            HirStyleBodyItem::Recovered(_) => {}
        }
    }
    Ok(())
}

impl<'project, 'catalog, 'control> Analyzer<'project, 'catalog, 'control> {
    fn new(
        executable: HirExecutableProjectView<'project>,
        symbols: &'catalog ProjectSymbolTable,
        catalogs: FinalSemanticCatalogs<'catalog>,
        control: FinalSemanticAnalysisControl<'control>,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        control.check()?;
        if !std::ptr::eq(symbols, catalogs.world.symbols())
            || symbols.world() != catalogs.world.environment().world()
            || symbols.revision() != catalogs.world.environment().symbol_revision()
        {
            return Err(FinalSemanticAnalysisError::CatalogGenerationMismatch);
        }
        let project = executable.project_view();
        let topology = executable
            .accept_symbol_generation(symbols)
            .map_err(|_| FinalSemanticAnalysisError::SymbolGenerationMismatch)?
            .into_evaluation_topology()
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let modules = executable
            .modules()
            .map(|(_, module)| (module.module_id(), module.as_ref()))
            .collect();
        let style_value_kinds = collect_style_value_kinds(&modules)?;
        Ok(Self {
            executable,
            project,
            symbols,
            catalogs,
            control,
            topology,
            modules,
            style_value_kinds,
            types: BTreeMap::new(),
            type_reports: BTreeMap::new(),
            facts: SemanticFactState::new(),
            staged_callables: None,
            call_frames: expression_error::CallFrameStack::new(
                catalogs.callable_limits.max_nested_calls(),
            )
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
            implicit_callable_stack: Vec::new(),
            pipe_stack: Vec::new(),
            function_site_stack: Vec::new(),
        })
    }

    fn analyze(mut self) -> Result<FinalSemanticAnalysis, super::FinalSemanticProjectError> {
        self.analyze_staged()
    }

    fn analyze_staged(
        &mut self,
    ) -> Result<FinalSemanticAnalysis, super::FinalSemanticProjectError> {
        self.resolve_all_types()?;
        self.seed_local_types()?;
        let staged_callables = self.stage_checked_callables()?;
        for (owner, fact) in &staged_callables.effect_expressions {
            self.facts
                .publish_new_expression(*owner, fact.clone())
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        }
        self.staged_callables = Some(staged_callables);
        self.infer_statement_bindings()?;
        self.validate_callable_body_results()?;
        self.analyze_all_expressions()?;
        self.finalize_unannotated_locals()?;
        if !self.facts.pending_implicit_capture_uses().is_empty() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
        }

        let mut input = FinalSemanticAnalysisInput::new();
        for (owner, ty) in &self.types {
            input.push_type(*owner, ty.clone());
        }
        let dialogue_view_parameters = self
            .modules
            .values()
            .flat_map(|module| module.items())
            .filter_map(|(_, item)| match item.kind() {
                HirItemKind::View(view) => Some(view.parameters()),
                _ => None,
            })
            .flatten()
            .filter(|parameter| {
                self.types.get(&parameter.ty()).is_some_and(|ty| {
                    let TypeKind::Named(name) = ty else {
                        return false;
                    };
                    self.catalogs
                        .world
                        .environment()
                        .typecheck_env()
                        .dialogue_view_models()
                        .model(name)
                        .is_some()
                })
            })
            .flat_map(|parameter| parameter.locals().iter().copied())
            .collect::<BTreeSet<_>>();
        for (owner, ty) in self.facts.locals() {
            let role = if dialogue_view_parameters.contains(owner) {
                CheckedBindingRole::DialogueViewParameter
            } else {
                CheckedBindingRole::Ordinary
            };
            input.push_local(*owner, CheckedBinding::with_role(ty.clone(), role));
        }
        for module in self.topology.modules() {
            for capture in module.captures().rows() {
                let ty = self.facts.locals().get(&capture.local()).cloned().ok_or(
                    FinalSemanticAnalysisError::LocalTypeUnavailable {
                        owner: capture.local(),
                    },
                )?;
                let role = if dialogue_view_parameters.contains(&capture.local()) {
                    CheckedBindingRole::DialogueViewParameter
                } else {
                    CheckedBindingRole::Ordinary
                };
                input.push_capture(capture.capture(), CheckedBinding::with_role(ty, role));
            }
        }
        self.analyze_patterns(&mut input)?;
        self.analyze_statements(&mut input)?;
        self.analyze_items(&mut input)?;
        for (owner, fact) in self.facts.expressions() {
            input.push_prepared_expression(*owner, fact.clone());
        }
        let staged = self
            .staged_callables
            .take()
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let checked_callables = self.finish_checked_callables(staged, &input)?;
        let mut item_facts = BTreeMap::new();
        for (item, fact) in &input.items {
            if item_facts.insert(*item, fact).is_some() {
                return Err(FinalSemanticAnalysisError::DuplicateFact {
                    family: SemanticFactFamily::Item,
                }
                .into());
            }
        }
        let accepted_roots = Arc::new(AcceptedSemanticRootCatalog::seal(
            Arc::clone(&self.topology),
            &checked_callables,
            &item_facts,
        )?);
        let selected_expressions = super::match_edges::CheckedSelectedExpressionGraph::seal(
            self.executable,
            Arc::clone(&self.topology),
            self.facts.expressions(),
            self.facts
                .prepared_calls()
                .map_err(FinalSemanticAnalysisError::from)?,
        )?;
        let structural_edges = super::match_edges::CheckedStructuralEdgeDraft::seal(
            &selected_expressions,
            &self.modules,
            self.facts.expressions(),
        );
        let semantic_coordinates =
            SemanticCoordinateIndex::new(accepted_roots.as_ref(), &structural_edges);
        self.finalize_call_facts(&checked_callables, &semantic_coordinates)?;
        self.validate_view_modifier_handler_effects(&checked_callables)?;
        let callable_joins = super::match_edges::prepare_checked_callable_joins(
            self.facts.calls(),
            &checked_callables,
        );
        let method_selections = super::match_edges::prepare_checked_method_selections(
            &structural_edges,
            self.facts.expressions(),
            &callable_joins,
        )
        .map_err(|error| FinalSemanticAnalysisError::CheckedCallableJoin(Box::new(error)))?;
        for (owner, selection) in method_selections {
            let previous = self
                .facts
                .expressions()
                .get(&owner)
                .cloned()
                .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
            let super::PreparedExpressionFact::Method(previous) = previous else {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
            };
            let (shell, _diagnostic_name) = previous.into_parts();
            let (ty, type_selection, effects) = shell.into_parts();
            self.facts
                .replace_existing_expression(
                    owner,
                    CheckedExpression::new(
                        ty,
                        type_selection,
                        effects,
                        CheckedExpressionResolution::Select(CheckedSelectResolution::Method(
                            selection,
                        )),
                    ),
                )
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        }
        input.set_callable_joins(callable_joins);
        input.set_selected_expressions(selected_expressions)?;
        input.set_structural_edges(structural_edges)?;
        input.expressions.clear();
        for (owner, fact) in self.facts.expressions() {
            input.push_prepared_expression(*owner, fact.clone());
        }
        for call in self.facts.calls().values() {
            input.push_call(call.clone());
        }
        self.validate_flow_effect_bounds(&input)?;
        input.set_physical_candidate_argument_evaluations(
            self.facts
                .physical_candidate_argument_evaluations()
                .iter()
                .map(|(root, evaluations)| (*root, Arc::from(evaluations.clone())))
                .collect(),
        );
        self.control.check()?;
        let semantic_shapes =
            super::AcceptedSemanticShapeCatalog::build(self.catalogs.world.environment())?;
        FinalSemanticAnalysis::try_new_with_control_and_type_resolutions_and_catalog(
            self.executable,
            self.symbols,
            checked_callables,
            input,
            std::mem::take(&mut self.type_reports),
            accepted_roots,
            semantic_shapes,
            self.control,
        )
    }

    /// Reconciles contextual closure types with the accepted latent effect row
    /// after nested call effects have been closed project-wide.
    ///
    /// A callback closure is initially checked under its parameter contract,
    /// while project-call effect rows are sealed later. View modifiers are the
    /// first retained-call owner that requires the final row at admission, so
    /// this join consumes the checked closure catalog rather than trusting the
    /// earlier contextual shell.
    fn validate_view_modifier_handler_effects(
        &self,
        checked_callables: &crate::callable::CheckedCallableCatalog,
    ) -> Result<(), FinalSemanticAnalysisError> {
        for (owner, facts) in self.facts.calls() {
            let Some(application) = facts.selected_application() else {
                continue;
            };
            let selected = application.core().candidates().selected();
            let CallableValidator::ViewModifier(_) = selected.schema().validator() else {
                continue;
            };
            let [argument] = application.core().execution().arguments() else {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            };
            let [slot] = argument.slots() else {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            };
            let handler = slot.source().owner();
            let Some(checked_handler) = self
                .facts
                .expressions()
                .get(&handler)
                .and_then(PreparedExpressionFact::complete)
            else {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            };
            if !matches!(
                checked_handler.resolution(),
                CheckedExpressionResolution::Closure(_)
            ) {
                return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner: *owner });
            }
            let [group] = selected.schema().groups() else {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            };
            let [parameter] = group.parameters() else {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            };
            let Some(TypeKind::Function {
                effects: permitted, ..
            }) = parameter.declared_type()
            else {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            };
            let module = self.module(handler.module())?;
            let source = statements::expression_span(module, handler)?;
            let actual = checked_callables
                .closure_at_source(&source)
                .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            EffectRow::check_subset(actual, permitted, &mut EffectSubstitution::new())
                .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed { owner: *owner })?;
        }
        Ok(())
    }

    pub(super) fn record_physical_candidate_argument_evaluation(
        &mut self,
        evaluation: PhysicalCandidateArgumentEvaluation,
    ) -> Result<(), FinalSemanticAnalysisError> {
        // A lower candidate may probe a source and later materialize the same
        // source to extract its semantic projection.  The operational product
        // is the candidate/pass/argument-slot projection, so that second
        // callback phase is not a second candidate evaluation row.  This is a
        // monotonic admission check: no previously completed row is removed.
        let owner = evaluation.call_expression();
        let limit = self
            .catalogs
            .callable_limits
            .max_query_work()
            .checked_mul(
                u64::try_from(self.catalogs.callable_limits.max_nested_calls())
                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
            )
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
        match self
            .facts
            .record_physical_candidate_argument_evaluation(evaluation, limit)
            .map_err(
                |violation| FinalSemanticAnalysisError::CandidateFactTransaction { violation },
            )? {
            state::PhysicalCandidateEvaluationAdmission::Recorded
            | state::PhysicalCandidateEvaluationAdmission::Duplicate => Ok(()),
            state::PhysicalCandidateEvaluationAdmission::LimitReached => {
                Err(FinalSemanticAnalysisError::CallResolutionFailed { owner })
            }
        }
    }

    pub(super) fn module(
        &self,
        id: HirModuleId,
    ) -> Result<&'project HirModule, FinalSemanticAnalysisError> {
        self.modules
            .get(&id)
            .copied()
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)
    }
}

#[path = "analyzer/call_seal.rs"]
mod call_seal;
#[path = "analyzer/callable_effect_graph.rs"]
mod callable_effect_graph;
#[path = "analyzer/calls.rs"]
mod calls;
pub(super) use calls::AnalyzerPreparedCallGraph;
pub(crate) use calls::CallAnalysisFailure;
pub(crate) use expression_error::CallFrameInvariant;
#[path = "analyzer/entities.rs"]
mod entities;
#[path = "analyzer/expression_error.rs"]
mod expression_error;
#[path = "analyzer/expression_types.rs"]
mod expression_types;
#[path = "analyzer/expressions.rs"]
mod expressions;
#[path = "analyzer/items.rs"]
mod items;
#[path = "analyzer/patterns.rs"]
mod patterns;
#[path = "analyzer/preparation.rs"]
mod preparation;
#[path = "analyzer/state.rs"]
mod state;
#[path = "analyzer/statements.rs"]
mod statements;

pub(super) use expression_error::AnalyzerExpressionContext;
pub(in crate::final_analysis) use expression_error::PhysicalCallAttemptId;

use state::{CandidateSemanticProjection, SemanticFactState};

#[cfg(test)]
mod semantic_fact_transaction_tests {
    use std::{
        cell::RefCell,
        rc::Rc,
        sync::{Arc, atomic::AtomicBool},
    };

    use arcweft_lang_hir::{
        dialogue_application::HirPostfixBracketCandidates, expr::HirExprKind,
        project::HirLocalValueOrigin,
    };
    use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;

    use super::expression_error::AnalyzerExpressionError;
    use super::state::CandidateFactTransactionAction;
    use super::*;

    fn assert_outer_expression_failure_rolls_back_call_publication(
        fixture: &crate::final_analysis::tests::Fixture,
    ) {
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&CanonicalModulePath::crate_root())
            .expect("root module");
        let (call_owner, call) = module
            .expressions()
            .find_map(|(owner, expression)| match expression.kind() {
                HirExprKind::Call(call) => Some((owner, call)),
                _ => None,
            })
            .expect("nested call expression");
        let argument_owners = call
            .arguments()
            .iter()
            .map(HirCallArgument::value)
            .collect::<Vec<_>>();
        let local_owner = module
            .locals()
            .next()
            .map(|(owner, _)| owner)
            .expect("local");
        let pattern_owner = module
            .patterns()
            .next()
            .map(|(owner, _)| owner)
            .expect("pattern");
        let mut parents = BTreeMap::new();
        for (owner, expression) in module.expressions() {
            for child in expression.kind().direct_expression_children() {
                parents.insert(child, owner);
            }
        }
        let mut outer_owner = call_owner;
        while let Some(parent) = parents.get(&outer_owner).copied() {
            outer_owner = parent;
        }

        let cancellation = AtomicBool::new(false);
        let mut analyzer = Analyzer::new(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            FinalSemanticCatalogs::production(&fixture.registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");
        let first_error = analyzer
            .analyze_staged()
            .expect_err("the outer expression fails after the inner call");
        assert!(
            analyzer
                .facts
                .physical_candidate_argument_evaluations()
                .values()
                .flatten()
                .all(|evaluation| evaluation.call_expression() != call_owner),
            "candidate-neutral recovery must not publish a synthetic physical candidate trace; error: {first_error:?}"
        );
        assert!(!analyzer.facts.calls().contains_key(&call_owner));
        assert!(!analyzer.facts.expressions().contains_key(&call_owner));
        assert!(
            !analyzer
                .facts
                .prepared_calls()
                .expect("prepared call graph")
                .selected_nodes()
                .any(|node| node.site() == crate::callable::CheckedCallSite::HirCall(call_owner))
        );
        assert_eq!(
            analyzer
                .facts
                .prepared_calls()
                .expect("prepared call graph")
                .selected_nodes()
                .count(),
            0,
            "rolled-back call publication must not retain prepared graph nodes"
        );
        for argument in &argument_owners {
            assert!(
                !analyzer.facts.expressions().contains_key(argument),
                "retained argument facts must roll back with call publication"
            );
        }

        let baseline_iteration = CheckedIteration::Builtin {
            family: CheckedIteratorFamily::Seq,
            item: TypeKind::I64,
        };
        analyzer
            .facts
            .set_local_type(local_owner, TypeKind::I64)
            .expect("baseline local");
        analyzer
            .facts
            .set_pattern_type(pattern_owner, TypeKind::U64)
            .expect("baseline pattern");
        analyzer
            .facts
            .set_iteration_fact(call_owner, baseline_iteration.clone())
            .expect("baseline iteration");
        let retry_error = analyzer
            .run_candidate_fact_transaction(|this, _authority, _transaction| {
                this.facts
                    .set_local_type(local_owner, TypeKind::Bool)
                    .map_err(AnalyzerExpressionError::fact)?;
                this.facts
                    .set_pattern_type(pattern_owner, TypeKind::I16)
                    .map_err(AnalyzerExpressionError::fact)?;
                this.facts
                    .set_iteration_fact(
                        call_owner,
                        CheckedIteration::Builtin {
                            family: CheckedIteratorFamily::Array,
                            item: TypeKind::Bool,
                        },
                    )
                    .map_err(AnalyzerExpressionError::fact)?;
                this.check_expression_published(outer_owner, None)
                    .map(|_| CandidateFactTransactionAction::Commit(()))
                    .map_err(AnalyzerExpressionError::fatal)
            })
            .map(|_| ())
            .map_err(|error| error.into_public(outer_owner))
            .expect_err("retry reaches the authored outer failure again");
        assert!(!matches!(
            first_error,
            crate::final_analysis::FinalSemanticProjectError::Semantic(error)
                if matches!(error.as_ref(), FinalSemanticAnalysisError::ExpressionCycle { .. })
        ));
        assert!(!matches!(
            retry_error,
            FinalSemanticAnalysisError::ExpressionCycle { .. }
        ));
        assert!(!analyzer.facts.calls().contains_key(&call_owner));
        assert!(!analyzer.facts.expressions().contains_key(&call_owner));
        assert!(
            !analyzer
                .facts
                .prepared_calls()
                .expect("prepared call graph")
                .selected_nodes()
                .any(|node| node.site() == crate::callable::CheckedCallSite::HirCall(call_owner))
        );
        assert_eq!(
            analyzer
                .facts
                .prepared_calls()
                .expect("prepared call graph")
                .selected_nodes()
                .count(),
            0,
            "retry rollback must not retain prepared graph nodes"
        );
        assert_eq!(
            analyzer.facts.locals().get(&local_owner),
            Some(&TypeKind::I64)
        );
        assert_eq!(
            analyzer.facts.patterns().get(&pattern_owner),
            Some(&TypeKind::U64)
        );
        assert_eq!(
            analyzer.facts.iteration_facts().get(&call_owner),
            Some(&baseline_iteration)
        );
    }

    #[test]
    fn outer_failure_rolls_back_inner_selected_call_publication() {
        let fixture = crate::final_analysis::tests::fixture(
            concat!(
                "fn consume(value: i64) -> i64 { value }\n",
                "fn caller(seed: i64) { (consume(1), missing); }\n",
            ),
            None,
        );
        assert_outer_expression_failure_rolls_back_call_publication(&fixture);
    }

    #[test]
    fn outer_failure_rolls_back_inner_ambiguous_call_publication() {
        let fixture = crate::final_analysis::tests::environment_overload_fixture(
            "fn caller(seed: i64) { (choose(1), missing); }\n",
        );
        assert_outer_expression_failure_rolls_back_call_publication(&fixture);
    }

    #[test]
    fn outer_failure_rolls_back_inner_rejected_call_publication() {
        let fixture = crate::final_analysis::tests::environment_overload_fixture(
            "fn caller(seed: i64) { (choose(\"no\"), missing); }\n",
        );
        assert_outer_expression_failure_rolls_back_call_publication(&fixture);
    }

    #[test]
    fn candidate_call_keeps_nested_ordinary_call_on_candidate_context() {
        let fixture = crate::final_analysis::tests::fixture(
            concat!(
                "fn inner(value: i64) -> i64 { value }\n",
                "fn consume(value: i64) -> i64 { value }\n",
                "fn caller(seed: i64) { consume(inner(seed)); }\n",
            ),
            None,
        );
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&CanonicalModulePath::crate_root())
            .expect("root module");
        let (outer_owner, inner_owner) = module
            .expressions()
            .find_map(|(owner, expression)| {
                if !matches!(expression.kind(), HirExprKind::Call(_)) {
                    return None;
                }
                expression
                    .kind()
                    .direct_expression_children()
                    .into_iter()
                    .find(|child| {
                        module.resolve_expr(*child).is_ok_and(|expression| {
                            matches!(expression.kind(), HirExprKind::Call(_))
                        })
                    })
                    .map(|child| (owner, child))
            })
            .expect("nested ordinary calls");
        let cancellation = AtomicBool::new(false);
        let mut analyzer = Analyzer::new(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            FinalSemanticCatalogs::production(&fixture.registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");
        analyzer.resolve_all_types().expect("types");
        analyzer.seed_local_types().expect("locals");
        let staged = analyzer
            .stage_checked_callables()
            .expect("checked callables");
        for (owner, fact) in &staged.effect_expressions {
            analyzer
                .facts
                .publish_new_expression(*owner, fact.clone())
                .expect("effect expression");
        }
        analyzer.staged_callables = Some(staged);
        analyzer.infer_statement_bindings().expect("bindings");

        analyzer
            .facts
            .publish_new_expression(
                inner_owner,
                CheckedExpression::new(
                    TypeKind::String,
                    CheckedTypeSelection::Inferred,
                    EffectSet::new(),
                    CheckedExpressionResolution::Structural,
                ),
            )
            .expect("unstable published cache");

        let outcome =
            analyzer
                .run_candidate_fact_transaction(|this, authority, _transaction| {
                    let context = AnalyzerExpressionContext::candidate(
                        authority,
                        Rc::clone(&this.call_frames),
                    );
                    let checked = this.evaluate_expression(&context, outer_owner, None)?;
                    drop(context);
                    Ok::<
                        CandidateFactTransactionAction<PreparedExpressionFact>,
                        AnalyzerExpressionError,
                    >(CandidateFactTransactionAction::Commit(checked))
                })
                .expect("candidate call transaction");
        let checked = outcome.into_committed().expect("candidate call result");
        assert_eq!(checked.ty(), &TypeKind::I64);
        let selected_sites = analyzer
            .facts
            .prepared_calls()
            .expect("prepared call graph")
            .selected_nodes()
            .map(|node| node.site())
            .collect::<BTreeSet<_>>();
        assert!(selected_sites.contains(&crate::callable::CheckedCallSite::HirCall(outer_owner,)));
        assert!(selected_sites.contains(&crate::callable::CheckedCallSite::HirCall(inner_owner,)));
    }

    #[test]
    fn selected_call_publishes_one_prepared_graph_node() {
        let fixture = crate::final_analysis::tests::fixture(
            "fn consume(value: i64) -> i64 { value }\nfn caller(value: i64) { consume(value); }\n",
            None,
        );
        let cancellation = AtomicBool::new(false);
        let mut analyzer = Analyzer::new(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            FinalSemanticCatalogs::production(&fixture.registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");
        let analysis = analyzer.analyze_staged().expect("simple selected call");
        let calls = analysis.calls().collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        let (owner, facts) = calls[0];
        let application = facts
            .selected_application()
            .expect("selected call application");
        assert_eq!(
            application.core().site(),
            crate::callable::CheckedCallSite::HirCall(owner)
        );
        assert!(matches!(
            application.core().callee(),
            crate::callable::CheckedCallCalleeExecution::Direct
        ));
        assert!(matches!(
            application.core().candidates().selected().state(),
            crate::callable::ResolvedCallableState::Base
        ));
        assert!(matches!(
            application.result(),
            crate::callable::CheckedCallResult::Value(TypeKind::I64)
        ));
        assert_eq!(application.result().ty(), &TypeKind::I64);
    }

    #[test]
    fn multi_group_function_values_use_initializer_origin_and_shared_dependency() {
        let fixture = crate::final_analysis::tests::fixture(
            concat!(
                "fn make(first: i64)(second: i64) -> i64 { second }\n",
                "fn caller() { let partial = make(1i64); partial(2i64); partial(3i64); }\n",
            ),
            None,
        );
        let cancellation = AtomicBool::new(false);
        let mut analyzer = Analyzer::new(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            FinalSemanticCatalogs::production(&fixture.registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");
        let analysis = analyzer
            .analyze_staged()
            .expect("multi-group function-value analysis");
        let applications = analysis
            .calls()
            .filter_map(|(_, facts)| facts.selected_application())
            .collect::<Vec<_>>();
        assert_eq!(applications.len(), 3, "partial and both shared local calls");
        let origin = applications
            .iter()
            .find(|application| {
                matches!(
                    application.result(),
                    crate::callable::CheckedCallResult::Continuation(_)
                )
            })
            .expect("initializer partial application");
        assert_eq!(origin.core().current_group().get(), 0);
        assert!(matches!(
            origin.core().candidates().selected().state(),
            crate::callable::ResolvedCallableState::Base
        ));
        let dependents = applications
            .iter()
            .filter(|application| {
                matches!(
                    application.core().callee(),
                    crate::callable::CheckedCallCalleeExecution::Value { .. }
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            dependents.len(),
            2,
            "both local uses retain the origin authority"
        );
        for dependent in dependents {
            assert!(matches!(
                dependent.core().candidates().selected().state(),
                crate::callable::ResolvedCallableState::Continuation(_)
            ));
            assert!(matches!(
                dependent.result(),
                crate::callable::CheckedCallResult::Value(TypeKind::I64)
            ));
            let crate::callable::ResolvedCallableState::Continuation(continuation) =
                dependent.core().candidates().selected().state()
            else {
                unreachable!("function-value dependent has a continuation state");
            };
            assert_eq!(
                continuation.prefix_application_core(),
                origin.core().digest()
            );
            assert_eq!(
                continuation.prefix_application_site(),
                origin.core().stable_site()
            );
            assert_eq!(
                continuation.inherited_solution().digest(),
                origin.core().solution().digest()
            );
        }
    }

    #[test]
    fn direct_nested_function_value_call_uses_inner_graph_site() {
        let fixture = crate::final_analysis::tests::fixture(
            concat!(
                "fn make(first: i64)(second: i64) -> i64 { second }\n",
                "fn caller() { make(1i64)(2i64); }\n",
            ),
            None,
        );
        let cancellation = AtomicBool::new(false);
        let mut analyzer = Analyzer::new(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            FinalSemanticCatalogs::production(&fixture.registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");
        let analysis = analyzer
            .analyze_staged()
            .expect("direct nested function-value analysis");
        let applications = analysis
            .calls()
            .filter_map(|(_, facts)| facts.selected_application())
            .collect::<Vec<_>>();
        assert_eq!(applications.len(), 2, "inner and outer applications");
        let inner = applications
            .iter()
            .find(|application| {
                matches!(
                    application.result(),
                    crate::callable::CheckedCallResult::Continuation(_)
                )
            })
            .expect("inner application");
        let outer = applications
            .iter()
            .find(|application| {
                matches!(
                    application.core().callee(),
                    crate::callable::CheckedCallCalleeExecution::Value { .. }
                )
            })
            .expect("outer application");
        assert_eq!(inner.core().current_group().get(), 0);
        assert!(matches!(
            inner.core().candidates().selected().state(),
            crate::callable::ResolvedCallableState::Base
        ));
        assert!(matches!(
            outer.core().callee(),
            crate::callable::CheckedCallCalleeExecution::Value { .. }
        ));
        assert!(matches!(
            outer.core().candidates().selected().state(),
            crate::callable::ResolvedCallableState::Continuation(_)
        ));
        assert!(matches!(
            inner.result(),
            crate::callable::CheckedCallResult::Continuation(_)
        ));
        assert!(matches!(
            outer.result(),
            crate::callable::CheckedCallResult::Value(TypeKind::I64)
        ));
        let crate::callable::ResolvedCallableState::Continuation(continuation) =
            outer.core().candidates().selected().state()
        else {
            unreachable!("outer function-value application has a continuation state");
        };
        assert_eq!(
            continuation.prefix_application_core(),
            inner.core().digest()
        );
        assert_eq!(
            continuation.prefix_application_site(),
            inner.core().stable_site()
        );
        assert_eq!(
            continuation.inherited_solution().digest(),
            inner.core().solution().digest()
        );
    }

    #[test]
    fn independent_function_parameter_value_applies_without_prepared_dependency() {
        let fixture = crate::final_analysis::tests::fixture(
            concat!("fn caller(callback: (i64) -> i64 effects {}) { callback(1i64); }\n",),
            None,
        );
        let cancellation = AtomicBool::new(false);
        let mut analyzer = Analyzer::new(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            FinalSemanticCatalogs::production(&fixture.registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");
        let analysis = analyzer
            .analyze_staged()
            .expect("independent function parameter analysis");
        let applications = analysis
            .calls()
            .filter_map(|(_, facts)| facts.selected_application())
            .collect::<Vec<_>>();
        assert_eq!(applications.len(), 1, "independent callback call");
        let independent = applications[0];
        assert!(matches!(
            independent.core().callee(),
            crate::callable::CheckedCallCalleeExecution::Value { .. }
        ));
        assert!(matches!(
            independent.core().candidates().selected().state(),
            crate::callable::ResolvedCallableState::Base
        ));
        assert!(matches!(
            independent
                .core()
                .candidates()
                .selected()
                .base()
                .authority()
                .stable(),
            crate::callable::ResolvedCallableStableIdentity::Lexical(_)
        ));
        assert!(matches!(
            independent.result(),
            crate::callable::CheckedCallResult::Value(TypeKind::I64)
        ));
    }

    #[test]
    fn terminal_function_result_enters_independent_origin_without_dependency() {
        let fixture = crate::final_analysis::tests::fixture(
            concat!(
                "fn make_loader() -> ((Unit) -> Unit effects {}) {\n",
                "    |_unit: Unit| -> Unit {}\n",
                "}\n",
                "fn caller() { let loader = make_loader(); loader(()); }\n",
            ),
            None,
        );
        let cancellation = AtomicBool::new(false);
        let mut analyzer = Analyzer::new(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            FinalSemanticCatalogs::production(&fixture.registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");
        let analysis = analyzer
            .analyze_staged()
            .expect("terminal function result analysis");
        let applications = analysis
            .calls()
            .filter_map(|(_, facts)| facts.selected_application())
            .collect::<Vec<_>>();
        assert_eq!(
            applications.len(),
            2,
            "factory and independent function-value call"
        );
        let producer = applications
            .iter()
            .find(|application| {
                matches!(
                    application.result(),
                    crate::callable::CheckedCallResult::Value(TypeKind::Function { .. })
                )
            })
            .expect("terminal function producer");
        let consumer = applications
            .iter()
            .find(|application| {
                matches!(
                    application.core().callee(),
                    crate::callable::CheckedCallCalleeExecution::Value { .. }
                )
            })
            .expect("independent function-value consumer");
        assert!(matches!(
            producer.core().callee(),
            crate::callable::CheckedCallCalleeExecution::Direct
        ));
        assert!(matches!(
            producer.core().candidates().selected().state(),
            crate::callable::ResolvedCallableState::Base
        ));
        assert!(matches!(
            consumer.core().candidates().selected().state(),
            crate::callable::ResolvedCallableState::Base
        ));
        assert!(matches!(
            consumer
                .core()
                .candidates()
                .selected()
                .base()
                .authority()
                .stable(),
            crate::callable::ResolvedCallableStableIdentity::FunctionValue(_)
        ));
        assert!(matches!(
            consumer.result(),
            crate::callable::CheckedCallResult::Value(TypeKind::Unit)
        ));
    }

    #[test]
    fn three_group_function_values_follow_prepared_adjacency() {
        let fixture = crate::final_analysis::tests::fixture(
            concat!(
                "fn make(first: i64)(second: i64)(third: i64) -> i64 { third }\n",
                "fn caller() { let first = make(1i64); let second = first(2i64); second(3i64); }\n",
            ),
            None,
        );
        let cancellation = AtomicBool::new(false);
        let mut analyzer = Analyzer::new(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            FinalSemanticCatalogs::production(&fixture.registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");
        let analysis = analyzer
            .analyze_staged()
            .expect("three-group function-value analysis");
        let mut applications = analysis
            .calls()
            .filter_map(|(_, facts)| facts.selected_application())
            .collect::<Vec<_>>();
        applications.sort_by_key(|application| application.core().current_group());
        assert_eq!(applications.len(), 3, "three selected applications");
        let origin = applications[0];
        let middle = applications[1];
        let terminal = applications[2];
        assert_eq!(origin.core().current_group().get(), 0);
        assert_eq!(middle.core().current_group().get(), 1);
        assert_eq!(terminal.core().current_group().get(), 2);
        assert!(matches!(
            origin.core().candidates().selected().state(),
            crate::callable::ResolvedCallableState::Base
        ));
        assert!(matches!(
            middle.core().candidates().selected().state(),
            crate::callable::ResolvedCallableState::Continuation(_)
        ));
        assert!(matches!(
            terminal.core().candidates().selected().state(),
            crate::callable::ResolvedCallableState::Continuation(_)
        ));
        assert!(matches!(
            origin.result(),
            crate::callable::CheckedCallResult::Continuation(_)
        ));
        assert!(matches!(
            middle.result(),
            crate::callable::CheckedCallResult::Continuation(_)
        ));
        assert!(matches!(
            terminal.result(),
            crate::callable::CheckedCallResult::Value(TypeKind::I64)
        ));
        assert!(matches!(
            middle.core().callee(),
            crate::callable::CheckedCallCalleeExecution::Value { .. }
        ));
        assert!(matches!(
            terminal.core().callee(),
            crate::callable::CheckedCallCalleeExecution::Value { .. }
        ));
        let crate::callable::ResolvedCallableState::Continuation(middle_continuation) =
            middle.core().candidates().selected().state()
        else {
            unreachable!("middle function-value application has a continuation state");
        };
        let crate::callable::ResolvedCallableState::Continuation(terminal_continuation) =
            terminal.core().candidates().selected().state()
        else {
            unreachable!("terminal function-value application has a continuation state");
        };
        assert_eq!(
            middle_continuation.prefix_application_core(),
            origin.core().digest()
        );
        assert_eq!(
            middle_continuation.inherited_solution().digest(),
            origin.core().solution().digest()
        );
        assert_eq!(
            terminal_continuation.prefix_application_core(),
            middle.core().digest()
        );
        assert_eq!(
            terminal_continuation.inherited_solution().digest(),
            middle.core().solution().digest()
        );
    }

    #[test]
    fn rolled_back_prepared_continuation_is_stale_without_independent_fallback() {
        let fixture = crate::final_analysis::tests::fixture(
            concat!(
                "fn make(first: i64)(second: i64) -> i64 { second }\n",
                "fn caller() { make(1i64); }\n",
            ),
            None,
        );
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&CanonicalModulePath::crate_root())
            .expect("root module");
        let call_owner = module
            .expressions()
            .find_map(|(owner, expression)| {
                matches!(expression.kind(), HirExprKind::Call(_)).then_some(owner)
            })
            .expect("call expression");
        let cancellation = AtomicBool::new(false);
        let mut analyzer = Analyzer::new(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            FinalSemanticCatalogs::production(&fixture.registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");
        analyzer.resolve_all_types().expect("types");
        analyzer.seed_local_types().expect("locals");
        let staged = analyzer.stage_checked_callables().expect("callables");
        for (owner, fact) in &staged.effect_expressions {
            analyzer
                .facts
                .publish_new_expression(*owner, fact.clone())
                .expect("effect expression");
        }
        analyzer.staged_callables = Some(staged);
        analyzer.infer_statement_bindings().expect("bindings");

        let captured = Rc::new(RefCell::new(None));
        let captured_for_transaction = Rc::clone(&captured);
        let captured_actual = Rc::new(RefCell::new(None));
        let captured_actual_for_transaction = Rc::clone(&captured_actual);
        let outcome = analyzer
            .run_candidate_fact_transaction(|this, authority, _transaction| {
                let context =
                    AnalyzerExpressionContext::candidate(authority, Rc::clone(&this.call_frames));
                this.evaluate_expression(&context, call_owner, None)?;
                drop(context);
                let actual = this
                    .facts
                    .prepared_calls().expect("prepared call graph")
                    .selected_nodes()
                    .find(|node| {
                        node.site() == crate::callable::CheckedCallSite::HirCall(call_owner)
                    })
                    .expect("candidate call graph node")
                    .prefix()
                    .application()
                    .result_type()
                    .expect("candidate call result type");
                *captured_actual_for_transaction.borrow_mut() = Some(actual.clone());
                let reference = match crate::callable::PreparedCallGraphIngress::new(
                    this.facts.prepared_calls().expect("prepared call graph"),
                )
                .continuation_at(
                    crate::callable::CheckedCallSite::HirCall(call_owner),
                    &actual,
                )
                {
                    Ok(crate::callable::PreparedCallSiteContinuation::Prepared(reference)) => {
                        reference
                    }
                    Ok(crate::callable::PreparedCallSiteContinuation::Independent) => {
                        return Err(AnalyzerExpressionError::Call {
                            owner: call_owner,
                            failure: CallAnalysisFailure::Invariant(
                                super::calls::CallAnalysisInvariant::Constraint(
                                    crate::callable::CallConstraintInvariant::InvalidPreparedNodeState,
                                ),
                            ),
                        });
                    }
                    Err(invariant) => {
                        return Err(AnalyzerExpressionError::Call {
                            owner: call_owner,
                            failure: CallAnalysisFailure::Invariant(
                                super::calls::CallAnalysisInvariant::Constraint(invariant),
                            ),
                        });
                    }
                };
                *captured_for_transaction.borrow_mut() = Some(reference.clone());
                Ok::<_, AnalyzerExpressionError>(CandidateFactTransactionAction::Rollback(
                    reference,
                ))
            })
            .expect("candidate rollback");
        assert!(matches!(
            outcome,
            super::state::CandidateFactTransactionOutcome::RolledBack(_)
        ));
        assert_eq!(
            analyzer
                .facts
                .prepared_calls()
                .expect("prepared call graph")
                .selected_nodes()
                .count(),
            0,
            "rollback removes the issued continuation node"
        );
        let stale = captured.borrow_mut().take().expect("captured continuation");
        let result =
            crate::callable::PreparedCallContinuationAuthority::resolve_prepared_continuation(
                analyzer
                    .facts
                    .prepared_calls()
                    .expect("prepared call graph"),
                &stale,
                &captured_actual
                    .borrow_mut()
                    .take()
                    .expect("captured call result type"),
            );
        assert!(matches!(
            result,
            Err(crate::callable::CallConstraintInvariant::MissingOrStalePreparedNode)
        ));
    }

    #[test]
    fn postfix_both_fail_rolls_back_both_candidate_subgraphs_and_guard() {
        let fixture = crate::final_analysis::tests::fixture("fn caller() { 1[true]; }\n", None);
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&CanonicalModulePath::crate_root())
            .expect("root module");
        let (owner, postfix) = module
            .expressions()
            .find_map(|(owner, expression)| match expression.kind() {
                HirExprKind::PostfixBracket(postfix) => Some((owner, postfix)),
                _ => None,
            })
            .expect("postfix expression");
        let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = postfix.candidates()
        else {
            panic!("postfix retains both interpretation candidates");
        };
        let candidate_owners = [*index, *dialogue];
        let target = postfix.target();
        let cancellation = AtomicBool::new(false);
        let mut analyzer = Analyzer::new(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            FinalSemanticCatalogs::production(&fixture.registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");

        for _ in 0..2 {
            assert!(matches!(
                analyzer.check_expression_published(owner, None),
                Err(FinalSemanticAnalysisError::UnresolvedPostfixBracket {
                    owner: failed
                }) if failed == owner
            ));
            assert!(!analyzer.facts.expressions().contains_key(&owner));
            assert!(!analyzer.facts.expressions().contains_key(&target));
            for candidate in candidate_owners {
                assert!(!analyzer.facts.expressions().contains_key(&candidate));
                assert!(!analyzer.facts.calls().contains_key(&candidate));
            }
        }
    }

    #[test]
    fn postfix_ambiguous_rolls_back_both_successful_candidate_rows() {
        let fixture = crate::final_analysis::tests::fixture(
            "fn caller(items: Seq<i64>, key: usize) { items[key]; }\n",
            None,
        );
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&CanonicalModulePath::crate_root())
            .expect("root module");
        let (owner, postfix) = module
            .expressions()
            .find_map(|(owner, expression)| match expression.kind() {
                HirExprKind::PostfixBracket(postfix) => Some((owner, postfix)),
                _ => None,
            })
            .expect("postfix expression");
        let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = postfix.candidates()
        else {
            panic!("postfix retains both interpretation candidates");
        };
        let index = *index;
        let dialogue = *dialogue;
        let cancellation = AtomicBool::new(false);
        let mut analyzer = Analyzer::new(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            FinalSemanticCatalogs::production(&fixture.registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");

        // The closed type algebra intentionally has no value that is both an
        // index source and a dialogue target. Exercise the defensive
        // ambiguity branch with two already-checked candidate rows, but mint
        // and roll them back through the real semantic-fact transaction.
        let outcome = analyzer.run_candidate_fact_transaction(|this, authority, _transaction| {
            this.facts
                .publish_new_expression(
                    index,
                    CheckedExpression::new(
                        TypeKind::I64,
                        CheckedTypeSelection::Inferred,
                        EffectSet::new(),
                        CheckedExpressionResolution::Structural,
                    ),
                )
                .map_err(|_| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
                })?;
            this.facts
                .publish_new_expression(
                    dialogue,
                    CheckedExpression::new(
                        TypeKind::I64,
                        CheckedTypeSelection::Inferred,
                        EffectSet::new(),
                        CheckedExpressionResolution::Structural,
                    ),
                )
                .map_err(|_| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
                })?;
            let context =
                AnalyzerExpressionContext::candidate(authority, Rc::clone(&this.call_frames));
            let _ = this.evaluate_expression(&context, owner, None)?;
            drop(context);
            Ok::<CandidateFactTransactionAction<()>, AnalyzerExpressionError>(
                CandidateFactTransactionAction::Commit(()),
            )
        });
        let outcome = outcome
            .map(|_| ())
            .map_err(|error| error.into_public(owner));
        assert!(matches!(
            outcome,
            Err(FinalSemanticAnalysisError::AmbiguousPostfixBracket {
                owner: failed
            }) if failed == owner
        ));
        assert!(!analyzer.facts.expressions().contains_key(&owner));
        assert!(!analyzer.facts.expressions().contains_key(&index));
        assert!(!analyzer.facts.expressions().contains_key(&dialogue));
        assert!(!matches!(
            analyzer.check_expression_published(owner, None),
            Err(FinalSemanticAnalysisError::ExpressionCycle { .. })
        ));
    }

    #[test]
    fn contextual_literal_cache_rewrite_rolls_back_and_retry_replaces_baseline() {
        let fixture = crate::final_analysis::tests::fixture("fn caller() { 1; }\n", None);
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&CanonicalModulePath::crate_root())
            .expect("root module");
        let owner = module
            .expressions()
            .find_map(|(owner, expression)| {
                matches!(expression.kind(), HirExprKind::Literal(_)).then_some(owner)
            })
            .expect("literal expression");
        let cancellation = AtomicBool::new(false);
        let mut analyzer = Analyzer::new(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            FinalSemanticCatalogs::production(&fixture.registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");
        let baseline = analyzer
            .check_expression_published(owner, None)
            .expect("default literal fact");

        let outcome = analyzer.run_candidate_fact_transaction(|this, _authority, _transaction| {
            let contextual = this
                .check_expression_published(owner, Some(&TypeKind::I64))
                .map_err(AnalyzerExpressionError::fatal)?;
            assert_eq!(contextual.ty(), &TypeKind::I64);
            Err::<CandidateFactTransactionAction<()>, _>(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::CheckedCallableCatalog,
            ))
        });
        let outcome = outcome
            .map(|_| ())
            .map_err(|error| error.into_public(owner));
        assert!(matches!(
            outcome,
            Err(FinalSemanticAnalysisError::CheckedCallableCatalog)
        ));
        assert_eq!(analyzer.facts.expressions().get(&owner), Some(&baseline));

        let retry = analyzer
            .check_expression_published(owner, Some(&TypeKind::U64))
            .expect("contextual retry");
        assert_eq!(retry.ty(), &TypeKind::U64);
        assert_eq!(analyzer.facts.expressions().get(&owner), Some(&retry));
    }

    #[test]
    fn uncached_expression_failure_retry_cleans_structured_guard() {
        let fixture = crate::final_analysis::tests::fixture("fn caller() { missing; }\n", None);
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&CanonicalModulePath::crate_root())
            .expect("root module");
        let owner = module
            .expressions()
            .find_map(|(owner, expression)| {
                matches!(expression.kind(), HirExprKind::Path(_)).then_some(owner)
            })
            .expect("unresolved path expression");
        let cancellation = AtomicBool::new(false);
        let mut analyzer = Analyzer::new(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            FinalSemanticCatalogs::production(&fixture.registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");

        for _ in 0..2 {
            assert!(matches!(
                analyzer.check_expression_published(owner, None),
                Err(FinalSemanticAnalysisError::ValueResolutionFailed {
                    owner: failed
                }) if failed == owner
            ));
            assert!(!analyzer.facts.expressions().contains_key(&owner));
        }
    }

    #[test]
    fn function_value_origin_query_resumes_exact_checked_owner() {
        let fixture = crate::final_analysis::tests::fixture(
            concat!(
                "fn make(first: i64)(second: i64) -> i64 { second }\n",
                "fn caller() { let first = make(1i64); let alias = first; alias(2i64); }\n",
            ),
            None,
        );
        let cancellation = AtomicBool::new(false);
        let mut analyzer = Analyzer::new(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            FinalSemanticCatalogs::production(&fixture.registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");
        analyzer
            .analyze_staged()
            .expect("alias function-value analysis");
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&CanonicalModulePath::crate_root())
            .expect("root HIR module");
        let topology = Arc::clone(&analyzer.topology);
        let local_origins = topology
            .module(module.module_id())
            .expect("module topology")
            .local_origins();
        let facts = analyzer.facts.expressions();
        let candidate = module
            .expressions()
            .filter_map(|(owner, _expression)| {
                let checked = facts.get(&owner)?;
                let Some(CheckedExpressionResolution::Value(CheckedValueResolution::Local(local))) =
                    checked.checked_resolution()
                else {
                    return None;
                };
                if !matches!(checked.ty(), TypeKind::Function { .. }) {
                    return None;
                }
                let HirLocalValueOrigin::DirectInitializer(initializer) =
                    local_origins.origin(*local)?
                else {
                    return None;
                };
                let HirExprKind::Path(_) = module.resolve_expr(initializer).ok()?.kind() else {
                    return None;
                };
                Some((owner, checked.clone()))
            })
            .next()
            .expect("aliased function-value path");

        let mut progress = crate::callable::prepare_function_value_origin_query(
            Arc::clone(&topology),
            module,
            candidate.0,
            &BTreeMap::new(),
        )
        .expect("origin query starts");
        let mut needs = 0_u32;
        loop {
            match progress {
                crate::callable::PreparedFunctionValueOriginProgress::Need(need) => {
                    needs = needs.checked_add(1).expect("query depth");
                    let owner = need.expression();
                    let checked = facts
                        .get(&owner)
                        .expect("query owner has checked expression");
                    progress = need
                        .resume(owner, checked, module)
                        .expect("exact checked owner resumes query");
                }
                crate::callable::PreparedFunctionValueOriginProgress::Ready(evidence) => {
                    assert!(needs >= 2, "the alias chain should cross two local origins");
                    assert!(matches!(
                        evidence.producer(),
                        crate::callable::PreparedFunctionValueOriginProducer::Call(
                            crate::callable::CheckedCallSite::HirCall(_)
                        )
                    ));
                    break;
                }
            }
        }

        let wrong_query = crate::callable::prepare_function_value_origin_query(
            Arc::clone(&topology),
            module,
            candidate.0,
            &BTreeMap::new(),
        )
        .expect("wrong-owner query starts");
        let crate::callable::PreparedFunctionValueOriginProgress::Need(need) = wrong_query else {
            panic!("the initial path must require its checked fact");
        };
        let wrong_owner = module
            .expressions()
            .map(|(owner, _)| owner)
            .find(|owner| *owner != need.expression())
            .expect("foreign expression owner");
        let wrong_checked = facts.get(&need.expression()).expect("query owner fact");
        let error = need.resume(wrong_owner, wrong_checked, module);
        assert!(matches!(
            error,
            Err(crate::callable::PreparedFunctionValueOriginQueryError::Invalid)
        ));
    }

    #[test]
    fn function_value_origin_query_classifies_independent_parameters_and_cycles() {
        let independent_fixture = crate::final_analysis::tests::fixture(
            "fn caller(callback: (i64) -> i64 effects {}) { callback(1i64); }\n",
            None,
        );
        let independent_module = independent_fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&CanonicalModulePath::crate_root())
            .expect("root HIR module");
        let independent_topology = independent_fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .accept_symbol_generation(&independent_fixture.symbols)
            .expect("accepted HIR generation")
            .into_evaluation_topology()
            .expect("evaluation topology");
        let independent_local = independent_module
            .locals()
            .find_map(|(owner, local)| (local.name().as_str() == "callback").then_some(owner))
            .expect("callback local");
        let independent_expression = independent_module
            .expressions()
            .find_map(|(owner, expression)| {
                let HirExprKind::Path(path) = expression.kind() else {
                    return None;
                };
                (path.as_resolved().and_then(|path| path.lexical_name()) == Some("callback"))
                    .then_some(owner)
            })
            .expect("callback path");
        let independent_checked = CheckedExpression::new(
            TypeKind::function([TypeKind::I64], TypeKind::I64),
            CheckedTypeSelection::Inferred,
            EffectSet::new(),
            CheckedExpressionResolution::Value(CheckedValueResolution::Local(independent_local)),
        );
        let independent_facts =
            BTreeMap::from([(independent_expression, independent_checked.into())]);
        let independent = crate::callable::prepare_function_value_origin_query(
            Arc::clone(&independent_topology),
            independent_module,
            independent_expression,
            &independent_facts,
        )
        .expect("independent origin query");
        assert!(matches!(
            independent,
            crate::callable::PreparedFunctionValueOriginProgress::Ready(evidence)
                if matches!(
                    evidence.producer(),
                    crate::callable::PreparedFunctionValueOriginProducer::Lexical { .. }
                        | crate::callable::PreparedFunctionValueOriginProducer::IndependentExpression {
                            ..
                        }
                )
        ));

        let cycle_fixture =
            crate::final_analysis::tests::fixture("fn caller() { let x = x; x(1i64); }\n", None);
        let cycle_module = cycle_fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&CanonicalModulePath::crate_root())
            .expect("root HIR module");
        let cycle_topology = cycle_fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .accept_symbol_generation(&cycle_fixture.symbols)
            .expect("accepted HIR generation")
            .into_evaluation_topology()
            .expect("evaluation topology");
        let cycle_local = cycle_module
            .locals()
            .find_map(|(owner, local)| (local.name().as_str() == "x").then_some(owner))
            .expect("cycle local");
        let cycle_expression = cycle_module
            .expressions()
            .find_map(|(owner, expression)| {
                let HirExprKind::Path(path) = expression.kind() else {
                    return None;
                };
                (path.as_resolved().and_then(|path| path.lexical_name()) == Some("x"))
                    .then_some(owner)
            })
            .expect("cycle path");
        let cycle_checked = CheckedExpression::new(
            TypeKind::function([TypeKind::I64], TypeKind::I64),
            CheckedTypeSelection::Inferred,
            EffectSet::new(),
            CheckedExpressionResolution::Value(CheckedValueResolution::Local(cycle_local)),
        );
        let cycle_checked: PreparedExpressionFact = cycle_checked.into();
        let cycle_facts = BTreeMap::from([(cycle_expression, cycle_checked.clone())]);
        let progress = crate::callable::prepare_function_value_origin_query(
            Arc::clone(&cycle_topology),
            cycle_module,
            cycle_expression,
            &cycle_facts,
        )
        .expect("cycle query begins with a checked path");
        let crate::callable::PreparedFunctionValueOriginProgress::Need(need) = progress else {
            panic!("cycle query must request the direct initializer fact");
        };
        let error = match need.resume(cycle_expression, &cycle_checked, cycle_module) {
            Ok(_) => panic!("revisiting the same local must be a typed cycle"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            crate::callable::PreparedFunctionValueOriginQueryError::Cycle
        ));
    }

    #[test]
    fn lexical_function_parameter_issues_top_level_prepared_identity() {
        let fixture = crate::final_analysis::tests::fixture(
            "fn caller(callback: (i64) -> i64 effects {}) { callback(1i64); }\n",
            None,
        );
        let cancellation = AtomicBool::new(false);
        let mut analyzer = Analyzer::new(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            FinalSemanticCatalogs::production(&fixture.registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");
        analyzer.analyze_staged().expect("lexical function call");
        let selected = analyzer
            .facts
            .calls()
            .values()
            .find_map(|call| call.selected_application())
            .map(|application| application.core().candidates().selected())
            .expect("selected lexical candidate");
        assert!(matches!(
            selected.base().authority().stable(),
            crate::callable::ResolvedCallableStableIdentity::Lexical(_)
        ));
    }

    #[test]
    fn evaluator_records_implicit_and_explicit_capture_modes_on_terminal_facts() {
        let fixture = crate::final_analysis::tests::fixture(
            concat!(
                "fn caller() {\n",
                "    let mut outer = 1i64;\n",
                "    let implicit_read = _ + outer;\n",
                "    result { outer; outer = _; () };\n",
                "    let explicit_read = || { outer };\n",
                "    let explicit_write = || { outer; outer = 2i64; () };\n",
                "}\n",
            ),
            None,
        );
        let cancellation = AtomicBool::new(false);
        let mut analyzer = Analyzer::new(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            FinalSemanticCatalogs::production(&fixture.registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");
        analyzer.resolve_all_types().expect("resolved types");
        analyzer.seed_local_types().expect("seeded locals");
        let staged = analyzer
            .stage_checked_callables()
            .expect("staged checked callables");
        for (owner, fact) in &staged.effect_expressions {
            analyzer
                .facts
                .publish_new_expression(*owner, fact.clone())
                .expect("published effect expression");
        }
        analyzer.staged_callables = Some(staged);
        analyzer
            .infer_statement_bindings()
            .expect("inferred statement bindings");
        assert!(
            analyzer
                .topology
                .modules()
                .iter()
                .flat_map(|module| module.expression_uses().rows())
                .any(|row| row.capture_access()
                    == arcweft_lang_hir::scope::CaptureAccess::Reassign),
            "typed statement target must be classified as Reassign"
        );
        let implicit_write = analyzer
            .modules
            .values()
            .flat_map(|module| module.expressions())
            .find_map(|(owner, expression)| {
                matches!(expression.kind(), HirExprKind::ComputationBlock(_)).then_some(owner)
            })
            .expect("implicit reassign producer");
        let context = AnalyzerExpressionContext::published(Rc::clone(&analyzer.call_frames));
        analyzer
            .evaluate_expression(
                &context,
                implicit_write,
                Some(&TypeKind::function(
                    [TypeKind::I64],
                    TypeKind::Result {
                        ok: Box::new(TypeKind::Unit),
                        error: Box::new(TypeKind::Never),
                    },
                )),
            )
            .expect("contextual implicit reassign producer");

        let mut implicit = Vec::new();
        let mut explicit = Vec::new();
        for checked in analyzer.facts.expressions().values() {
            match checked.checked_resolution() {
                Some(CheckedExpressionResolution::ImplicitCallable(callable)) => {
                    assert!(Arc::ptr_eq(callable.topology(), &analyzer.topology));
                    let [capture] = callable.captures() else {
                        continue;
                    };
                    implicit.push(capture.mode());
                }
                Some(CheckedExpressionResolution::Closure(closure)) => {
                    assert!(Arc::ptr_eq(closure.topology(), &analyzer.topology));
                    let [capture] = closure.captures() else {
                        continue;
                    };
                    explicit.push(capture.mode());
                }
                _ => {}
            }
        }
        implicit.sort();
        explicit.sort();
        let expected = vec![
            arcweft_lang_hir::scope::CaptureAccess::Read,
            arcweft_lang_hir::scope::CaptureAccess::Reassign,
        ];
        assert_eq!(implicit, expected);
        assert_eq!(explicit, expected);
    }

    #[test]
    fn function_value_origin_retains_terminal_captures_through_aliases() {
        for (source, mode) in [
            (
                "fn caller() { let outer = 1i64; let producer = || -> i64 { outer }; let alias = producer; alias(); }\n",
                arcweft_lang_hir::scope::CaptureAccess::Read,
            ),
            (
                "fn caller() { let mut outer = 1i64; let producer: (i64) -> Result<Unit, Never> = result { outer; outer = _; () }; let alias = producer; alias(2i64); }\n",
                arcweft_lang_hir::scope::CaptureAccess::Reassign,
            ),
        ] {
            let fixture = crate::final_analysis::tests::fixture(source, None);
            let module = fixture
                .project
                .executable_view()
                .expect("executable HIR")
                .module(&CanonicalModulePath::crate_root())
                .expect("root module");
            let outer = module
                .locals()
                .find_map(|(id, local)| (local.name().as_str() == "outer").then_some(id))
                .expect("outer local");
            let alias_use = module
                .expressions()
                .find_map(|(id, expression)| {
                    let HirExprKind::Path(path) = expression.kind() else {
                        return None;
                    };
                    (path.as_resolved().and_then(|path| path.lexical_name()) == Some("alias"))
                        .then_some(id)
                })
                .expect("alias use");
            let cancellation = AtomicBool::new(false);
            let mut analyzer = Analyzer::new(
                fixture.project.executable_view().expect("executable HIR"),
                &fixture.symbols,
                FinalSemanticCatalogs::production(&fixture.registered),
                FinalSemanticAnalysisControl::new(&cancellation),
            )
            .expect("analyzer");
            analyzer.resolve_all_types().expect("resolved types");
            analyzer.seed_local_types().expect("seeded locals");
            let staged = analyzer
                .stage_checked_callables()
                .expect("staged checked callables");
            for (owner, fact) in &staged.effect_expressions {
                analyzer
                    .facts
                    .publish_new_expression(*owner, fact.clone())
                    .expect("published effect expression");
            }
            analyzer.staged_callables = Some(staged);
            analyzer
                .infer_statement_bindings()
                .expect("inferred statement bindings");
            analyzer
                .validate_callable_body_results()
                .expect("validated callable results");
            analyzer
                .analyze_all_expressions()
                .expect("analyzed expression facts");
            let producer = analyzer
                .facts
                .expressions()
                .iter()
                .find_map(|(owner, checked)| {
                    matches!(
                        checked.checked_resolution(),
                        Some(
                            CheckedExpressionResolution::Closure(_)
                                | CheckedExpressionResolution::ImplicitCallable(_)
                        )
                    )
                    .then_some(*owner)
                })
                .expect("terminal function-value producer");
            let mut progress = crate::callable::prepare_function_value_origin_query(
                Arc::clone(&analyzer.topology),
                module,
                alias_use,
                analyzer.facts.expressions(),
            )
            .expect("origin query");
            let evidence = loop {
                match progress {
                    crate::callable::PreparedFunctionValueOriginProgress::Ready(evidence) => {
                        break evidence;
                    }
                    crate::callable::PreparedFunctionValueOriginProgress::Need(need) => {
                        let owner = need.expression();
                        progress = need
                            .resume(
                                owner,
                                analyzer
                                    .facts
                                    .expressions()
                                    .get(&owner)
                                    .expect("queried fact"),
                                module,
                            )
                            .expect("resume origin query");
                    }
                }
            };
            assert_eq!(evidence.captures().len(), 1);
            assert_eq!(evidence.captures()[0].local(), outer);
            assert_eq!(evidence.captures()[0].mode(), mode);
            assert!(matches!(
                evidence.producer(),
                crate::callable::PreparedFunctionValueOriginProducer::IndependentExpression {
                    producer: actual
                } if *actual == producer
            ));

            let foreign_topology = fixture
                .project
                .executable_view()
                .expect("executable HIR")
                .accept_symbol_generation(&fixture.symbols)
                .expect("accepted HIR generation")
                .into_evaluation_topology()
                .expect("foreign allocation");
            assert!(!Arc::ptr_eq(&foreign_topology, &analyzer.topology));
            let mut foreign_progress = crate::callable::prepare_function_value_origin_query(
                foreign_topology,
                module,
                alias_use,
                analyzer.facts.expressions(),
            )
            .expect("foreign query starts before terminal fact");
            loop {
                match foreign_progress {
                    crate::callable::PreparedFunctionValueOriginProgress::Need(need) => {
                        let owner = need.expression();
                        match need.resume(
                            owner,
                            analyzer
                                .facts
                                .expressions()
                                .get(&owner)
                                .expect("foreign query owner fact"),
                            module,
                        ) {
                            Err(
                                crate::callable::PreparedFunctionValueOriginQueryError::CaptureTopologyMismatch(
                                    crate::final_analysis::CheckedCaptureAuthorityViolation::TopologyMismatch
                                )
                            ) => break,
                            Err(error) => {
                                panic!("unexpected foreign topology result: {error:?}")
                            }
                            Ok(progress) => foreign_progress = progress,
                        }
                    }
                    crate::callable::PreparedFunctionValueOriginProgress::Ready(_) => {
                        panic!("foreign topology query unexpectedly succeeded")
                    }
                }
            }

            let wrong_producer = module
                .expressions()
                .find_map(|(owner, expression)| {
                    (owner != producer && matches!(expression.kind(), HirExprKind::Literal(_)))
                        .then_some(owner)
                })
                .expect("independent wrong producer");
            let terminal = analyzer
                .facts
                .expressions()
                .get(&producer)
                .expect("terminal checked fact")
                .clone();
            assert!(matches!(
                crate::callable::prepare_function_value_origin_query(
                    Arc::clone(&analyzer.topology),
                    module,
                    wrong_producer,
                    &BTreeMap::from([(wrong_producer, terminal)]),
                ),
                Err(
                    crate::callable::PreparedFunctionValueOriginQueryError::CaptureProducerMismatch(
                        crate::final_analysis::CheckedCaptureAuthorityViolation::ProducerMismatch {
                            expected,
                            actual,
                        }
                    )
                ) if expected == wrong_producer && actual == producer
            ));
        }
    }
}
