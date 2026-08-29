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
    dialogue_application::{HirPostfixBracket, HirPostfixBracketCandidates},
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
        HirFloatLiteral, HirFloatWidth, HirIdRef, HirIntegerLiteral, HirLiteral, HirName,
        HirPathRoot, HirPathSegment,
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
    stmt::{HirAssertionMode, HirStmtKind},
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
    CheckedDialogueEffectSiteOrdinal, CheckedDialogueEffectTrigger, CheckedExpression,
    CheckedExpressionResolution, CheckedFunctionExecution, CheckedImplicitCallable, CheckedItem,
    CheckedItemRole, CheckedIteration, CheckedIteratorFamily, CheckedPatchOperation,
    CheckedPattern, CheckedPatternResolution, CheckedPipe, CheckedProjectCallable,
    CheckedProjectItem, CheckedProjectNominal, CheckedSelectResolution, CheckedStageLook,
    CheckedStyleCallee, CheckedSuspensionRole, CheckedSuspensionStatement, CheckedTraitConformance,
    CheckedTraitIdentity, CheckedTry, CheckedTryBoundary, CheckedTryCarrier, CheckedTypeSelection,
    CheckedTypedBinding, CheckedValueResolution, CheckedVariantOwner, CheckedVariantResolution,
    CheckedViewCall, CheckedViewCallee, FinalSemanticAnalysis, FinalSemanticAnalysisControl,
    FinalSemanticAnalysisError, FinalSemanticAnalysisInput, PhysicalArgumentEvaluationKind,
    PhysicalCandidateArgumentEvaluation, PostfixBracketResolution, PreparedAssignmentStatement,
    PreparedDialogueApplication, PreparedDialogueEffectSite, PreparedDialogueLinePlan,
    PreparedEntryExpression, PreparedEntryReference, PreparedExpressionFact,
    PreparedExpressionShell, PreparedPatternFact, PreparedProjectVariantExpression,
    PreparedProjectVariantOwnerSeed, PreparedProjectVariantPattern, PreparedStatementPayload,
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
    let expressions = input
        .expressions
        .iter()
        .cloned()
        .collect::<BTreeMap<_, _>>();
    if expressions.len() != input.expressions.len() {
        return Err(FinalSemanticAnalysisError::DuplicateFact {
            family: SemanticFactFamily::Expression,
        });
    }
    let selected = super::match_edges::CheckedSelectedExpressionGraph::seal_call_free_fixture(
        project,
        Arc::clone(&topology),
        &expressions,
    )?;
    let callables = analyzer.freeze_checked_callables(input, &selected)?;
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
    let result = analyzer.analyze_staged(NoPreparedStatementMutation);
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

#[cfg(test)]
pub(super) fn analyze_final_project_with_statement_mutation_for_test(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    catalogs: FinalSemanticCatalogs<'_>,
    control: FinalSemanticAnalysisControl<'_>,
    owner: StmtId,
    replacement: super::PreparedStatementPayload,
) -> Result<FinalSemanticAnalysis, FinalSemanticAnalysisError> {
    Analyzer::new(project, symbols, catalogs, control)?
        .analyze_staged(ExactPreparedStatementMutation { owner, replacement })
        .map_err(super::FinalSemanticProjectError::into_semantic_fixture_error)
}

#[cfg(test)]
pub(super) enum FinalAuthorityMutationForTest {
    EventDigest {
        statement: StmtId,
        replacement: crate::types::SemanticTypeDigest,
    },
    MissingCheckedCallable {
        declaration: arcweft_lang_hir::symbol::CallableDeclarationKey,
    },
    SubstituteCheckedCallable {
        declaration: arcweft_lang_hir::symbol::CallableDeclarationKey,
        replacement: CheckedCallableId,
    },
}

#[cfg(test)]
pub(super) fn analyze_final_project_with_authority_mutation_for_test(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    catalogs: FinalSemanticCatalogs<'_>,
    control: FinalSemanticAnalysisControl<'_>,
    mutation: FinalAuthorityMutationForTest,
) -> Result<FinalSemanticAnalysis, super::FinalSemanticProjectError> {
    Analyzer::new(project, symbols, catalogs, control)?.analyze_staged(mutation)
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

trait FinalAnalysisMutation {
    fn apply_prepared_input(
        &mut self,
        input: &mut FinalSemanticAnalysisInput,
    ) -> Result<(), FinalSemanticAnalysisError>;

    fn apply_checked_callables(
        &mut self,
        checked_callables: &mut Arc<crate::callable::CheckedCallableCatalog>,
    ) -> Result<(), FinalSemanticAnalysisError>;
}

struct NoPreparedStatementMutation;

impl FinalAnalysisMutation for NoPreparedStatementMutation {
    fn apply_prepared_input(
        &mut self,
        _input: &mut FinalSemanticAnalysisInput,
    ) -> Result<(), FinalSemanticAnalysisError> {
        Ok(())
    }

    fn apply_checked_callables(
        &mut self,
        _checked_callables: &mut Arc<crate::callable::CheckedCallableCatalog>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        Ok(())
    }
}

#[cfg(test)]
struct ExactPreparedStatementMutation {
    owner: StmtId,
    replacement: super::PreparedStatementPayload,
}

#[cfg(test)]
impl FinalAnalysisMutation for ExactPreparedStatementMutation {
    fn apply_prepared_input(
        &mut self,
        input: &mut FinalSemanticAnalysisInput,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let matches = input
            .statements
            .iter()
            .enumerate()
            .filter_map(|(index, (owner, _))| (*owner == self.owner).then_some(index))
            .collect::<Vec<_>>();
        let [index] = matches.as_slice() else {
            return Err(if matches.is_empty() {
                FinalSemanticAnalysisError::MissingFact {
                    family: SemanticFactFamily::Statement,
                }
            } else {
                FinalSemanticAnalysisError::DuplicateFact {
                    family: SemanticFactFamily::Statement,
                }
            });
        };
        let (_, payload) = input
            .statements
            .get_mut(*index)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let replacement = std::mem::replace(
            &mut self.replacement,
            super::PreparedStatementPayload::HirOwned,
        );
        *payload = replacement;
        Ok(())
    }

    fn apply_checked_callables(
        &mut self,
        _checked_callables: &mut Arc<crate::callable::CheckedCallableCatalog>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        Ok(())
    }
}

#[cfg(test)]
impl FinalAnalysisMutation for FinalAuthorityMutationForTest {
    fn apply_prepared_input(
        &mut self,
        input: &mut FinalSemanticAnalysisInput,
    ) -> Result<(), FinalSemanticAnalysisError> {
        match self {
            Self::EventDigest {
                statement,
                replacement,
            } => input
                .ingress_seal
                .as_mut()
                .ok_or(FinalSemanticAnalysisError::MissingFact {
                    family: SemanticFactFamily::Statement,
                })?
                .replace_event_digest_for_test(*statement, *replacement),
            Self::MissingCheckedCallable { .. } | Self::SubstituteCheckedCallable { .. } => Ok(()),
        }
    }

    fn apply_checked_callables(
        &mut self,
        checked_callables: &mut Arc<crate::callable::CheckedCallableCatalog>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        match self {
            Self::EventDigest { .. } => Ok(()),
            Self::MissingCheckedCallable { declaration } => Arc::get_mut(checked_callables)
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?
                .remove_project_candidate_for_test(declaration)
                .then_some(())
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog),
            Self::SubstituteCheckedCallable {
                declaration,
                replacement,
            } => Arc::get_mut(checked_callables)
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?
                .replace_project_candidate_for_test(declaration, replacement.clone())
                .then_some(())
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog),
        }
    }
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
        self.analyze_staged(NoPreparedStatementMutation)
    }

    fn analyze_staged<M: FinalAnalysisMutation>(
        &mut self,
        mut mutation: M,
    ) -> Result<FinalSemanticAnalysis, super::FinalSemanticProjectError> {
        self.resolve_all_types()?;
        self.seed_local_types()?;
        let entry_roots =
            crate::entry::prepare_entry_root_seeds(self.executable, self.symbols, &self.types)
                .map_err(|diagnostics| {
                    super::FinalSemanticProjectError::Entry(diagnostics.into())
                })?;
        let staged_callables = self.stage_checked_callables()?;
        for (owner, fact) in &staged_callables.effect_expressions {
            self.facts
                .publish_new_expression(*owner, fact.clone())
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        }
        self.staged_callables = Some(staged_callables);
        let ingress_seal = self.complete_contextual_declarations(entry_roots)?;
        self.infer_residual_statement_bindings()?;
        self.analyze_residual_expressions()?;
        self.finalize_residual_locals()?;
        if !self.facts.pending_implicit_capture_uses().is_empty() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
        }

        let mut input = FinalSemanticAnalysisInput::new();
        input.set_ingress_seal(ingress_seal)?;
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
        mutation.apply_prepared_input(&mut input)?;
        self.analyze_items(&mut input)?;
        for (owner, fact) in self.facts.expressions() {
            input.push_prepared_expression(*owner, fact.clone());
        }
        let selected_expressions = super::match_edges::CheckedSelectedExpressionGraph::seal(
            self.executable,
            Arc::clone(&self.topology),
            self.facts.expressions(),
            self.facts
                .prepared_calls()
                .map_err(FinalSemanticAnalysisError::from)?,
        )?;
        let staged = self
            .staged_callables
            .take()
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let (mut checked_callables, prepared_effects) =
            self.finish_checked_callables(staged, &input, &selected_expressions)?;
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
        let structural_edges = super::match_edges::CheckedStructuralEdgeDraft::seal(
            &selected_expressions,
            &self.modules,
            self.facts.expressions(),
        );
        let semantic_coordinates =
            SemanticCoordinateIndex::new(accepted_roots.as_ref(), &structural_edges);
        self.finalize_call_facts(&checked_callables, &semantic_coordinates)?;
        self.finalize_evaluated_effects(&mut input, &semantic_coordinates)?;
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
        self.validate_flow_effect_bounds(&input, &prepared_effects)?;
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
        mutation.apply_checked_callables(&mut checked_callables)?;
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
#[path = "analyzer/evaluated_effects.rs"]
mod evaluated_effects;
#[path = "analyzer/executable_ingress.rs"]
mod executable_ingress;
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
#[path = "analyzer/statement_scrutinee.rs"]
mod statement_scrutinee;
#[path = "analyzer/statements.rs"]
mod statements;

pub(crate) use executable_ingress::{
    PreparedEntryIngressSeal, PreparedExecutableIngressFacts, PreparedExecutableIngressSeal,
    PreparedStatementIngressSeal,
};

pub(super) use expression_error::AnalyzerExpressionContext;
pub(in crate::final_analysis) use expression_error::PhysicalCallAttemptId;

use state::{CandidateSemanticProjection, SemanticFactState};

#[cfg(test)]
#[path = "analyzer/tests.rs"]
mod semantic_fact_transaction_tests;
