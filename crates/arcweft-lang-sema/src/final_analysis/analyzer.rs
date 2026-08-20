//! Sole production construction path for [`FinalSemanticAnalysis`].
//!
//! This pass consumes the exact, accepted arena inventories.  It never opens
//! source text, constructs a detached HIR, or publishes a partial report.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_lang_hir::{
    dialogue_application::{HirPostfixBracket, HirPostfixBracketCandidates},
    expr::{
        HirAssociatedSeparator, HirAwaitBranchKind, HirBinaryOp, HirBorrowKind, HirCallArgument,
        HirCallCallee, HirCallExpr, HirCallValue, HirComputationBlockKind, HirExpr, HirExprKind,
        HirRecordField, HirRecoveredName, HirSelectedMember, HirUnaryOp,
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
    project::{HirExecutableProjectView, HirProjectView},
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
        CallArgumentMapping, CallCalleeClassificationFact, CallPoison, CallResolverAuthority,
        CallResolverContext, CallResolverRequest, CallTargetFacts, CallTargetFactsInput,
        CallableAccess, CallableAuthorityRank, CallableCandidateId, CallableEffectContract,
        CallableEffectSchema, CallableGroupIndex, CallableInstantiation, CallableMethodRole,
        CallableQueryDepth, CharacterOwnerSource, CheckedCallArgumentFact,
        CheckedCallArgumentSlotFact, CheckedCallArgumentSlotInput, CheckedCallArgumentSlotSource,
        CheckedCallTarget, CheckedCallableCatalog, CheckedCallableCatalogBuildError,
        CheckedCallableCatalogBuilder, CheckedCallableDeclaration, CheckedCallableExecution,
        CheckedCallableId, CheckedClosureId, EffectClauseSource, EffectItemSource,
        FinalCallCalleeFacts, MappedCallArgumentSlot, ResolveCallOutcome, ResolvedCallTarget,
        ResolvedCallable, ResolvedCharacterOwner, ResolverWork, STANDARD_TRAIT_CATALOG_VERSION,
        map_call_arguments, map_unmapped_call_arguments, prepare_final_call_callee,
        prepare_language_free_dot_path, resolve_call_target,
    },
    callable::{
        CallableLimits, CallableParameterType, PRODUCTION_CALLABLE_LIMITS, SpreadArgumentPolicy,
    },
    checked_rich_text::RichTextAttributeChecker,
    effect_row::{EffectRow, EffectSubsetError},
    effects::{EffectId, EffectSet},
    env::{EnumVariantPayload, EnvironmentEnumSchema, TypeCheckEnv},
    nominal::{
        BuiltinTypeConstructor, GenericTypeBinding, GenericTypeScope, NominalResolutionLimits,
        ResolvedTypeRefOutcome, SelfTypeScope, TypeNameResolution, TypeResolutionFailure,
        TypeResolutionInput, TypeResolutionReport, TypeSourceEvidence, resolve_type_ref,
    },
    registration::RegisteredSemanticWorld,
    types::{
        ArrayLength, CharacterDialogueCharacterType, EntityKind, GenericTypeOwnerId,
        GenericTypeParameterId, IteratorStateKind, ProjectNominalType, TypeKind,
        TypeParameterSubstitutions,
    },
};

use super::{
    CandidateEvaluationPass, CandidateExpectedType, CharacterDialogueFieldCoordinate,
    CharacterDialoguePatchContext, CheckedAssertionDisposition, CheckedAssignment,
    CheckedAssignmentPlace, CheckedAwait, CheckedAwaitPendingObserver, CheckedBinding,
    CheckedBindingRole, CheckedBuiltinVariantCase, CheckedCharacterDialogueFactory,
    CheckedCharacterDialoguePatch, CheckedCharacterDialoguePatchField,
    CheckedCharacterDialogueReconfigure, CheckedCharacterDialogueTarget, CheckedEntryReference,
    CheckedEvaluatedEffect, CheckedExpression, CheckedExpressionResolution,
    CheckedFunctionExecution, CheckedImplicitCallable, CheckedItem, CheckedItemRole,
    CheckedIteration, CheckedIteratorFamily, CheckedPatchOperation, CheckedPattern,
    CheckedPatternResolution, CheckedPipe, CheckedProjectCallable, CheckedProjectItem,
    CheckedProjectNominal, CheckedSelectResolution, CheckedStatement, CheckedStatementRole,
    CheckedStyleCallee, CheckedSuspensionRole, CheckedSuspensionStatement, CheckedTraitConformance,
    CheckedTraitIdentity, CheckedTry, CheckedTryBoundary, CheckedTryCarrier, CheckedTypeSelection,
    CheckedValueResolution, CheckedVariantOwner, CheckedVariantResolution, CheckedViewCall,
    CheckedViewCallee, FinalSemanticAnalysis, FinalSemanticAnalysisControl,
    FinalSemanticAnalysisError, FinalSemanticAnalysisInput, PhysicalArgumentEvaluationKind,
    PhysicalCandidateArgument, PhysicalCandidateArgumentEvaluation, PostfixBracketResolution,
    ProjectHirSymbolLookupError, ProjectSymbolResolutionError, RecursiveCallableContractEdge,
    RegisteredSemanticValueId, SemanticFactFamily,
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
) -> Result<FinalSemanticAnalysis, FinalSemanticAnalysisError> {
    Analyzer::new(project, symbols, catalogs, control)?.analyze()
}

#[cfg(test)]
pub(super) fn freeze_checked_callables_for_test(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    catalogs: FinalSemanticCatalogs<'_>,
    input: &FinalSemanticAnalysisInput,
) -> Result<Arc<CheckedCallableCatalog>, FinalSemanticAnalysisError> {
    let cancellation = std::sync::atomic::AtomicBool::new(false);
    Analyzer::new(
        project,
        symbols,
        catalogs,
        FinalSemanticAnalysisControl::new(&cancellation),
    )?
    .freeze_checked_callables(input)
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
            .physical_candidate_argument_evaluations
            .values()
            .flat_map(|evaluations| evaluations.iter().cloned())
            .collect(),
    };
    (result, physical)
}

struct Analyzer<'project, 'catalog, 'control> {
    executable: HirExecutableProjectView<'project>,
    project: HirProjectView<'project>,
    symbols: &'catalog ProjectSymbolTable,
    catalogs: FinalSemanticCatalogs<'catalog>,
    control: FinalSemanticAnalysisControl<'control>,
    modules: BTreeMap<HirModuleId, &'project HirModule>,
    style_value_kinds: BTreeMap<ExprId, arcweft_view::style::ViewStyleValueKind>,
    types: BTreeMap<TypeId, TypeKind>,
    type_reports: BTreeMap<TypeId, TypeResolutionReport>,
    iteration_facts: BTreeMap<ExprId, CheckedIteration>,
    facts: SemanticFactState,
    staged_callables: Option<StagedCheckedCallables>,
    physical_candidate_argument_evaluations:
        BTreeMap<ExprId, Vec<PhysicalCandidateArgumentEvaluation>>,
    callable_query_depth: CallableQueryDepth,
    physical_call_stack: Vec<ExprId>,
    implicit_callable_stack: Vec<ImplicitCallableContext>,
    pipe_stack: Vec<PipeContext>,
    function_site_stack: Vec<FunctionSiteContext>,
}

struct ImplicitCallableContext {
    owner: ExprId,
    parameter: TypeKind,
    result: Option<TypeKind>,
    members: BTreeSet<ExprId>,
    placeholders: BTreeSet<ExprId>,
}

struct PipeContext {
    owner: ExprId,
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

#[derive(Clone)]
struct PendingCallAnalysis {
    expression: ExprId,
    expression_resolution: CheckedExpressionResolution,
    callee_expression: Option<ExprId>,
    enclosing_callable: Option<arcweft_lang_hir::symbol::CallableDeclarationKey>,
    callee: CallCalleeClassificationFact,
    selected: ResolvedCallable,
    considered: Vec<ResolvedCallable>,
    arguments: Vec<CheckedCallArgumentFact>,
    result: TypeKind,
    current_group: CallableGroupIndex,
    function_value_type: Option<TypeKind>,
    accounting: crate::callable::CallResolverAccountingReport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CandidateScore {
    hard_errors: usize,
    exact_matches: usize,
    unchecked_or_open: usize,
    omitted_parameters: usize,
    authority: Option<CallableAuthorityRank>,
}

struct CandidateProbe {
    candidate: ResolvedCallable,
    current_group: CallableGroupIndex,
    mapping: CallArgumentMapping,
    arguments: Vec<CheckedCallArgumentFact>,
    projection: CandidateSemanticProjection,
    result: TypeKind,
    score: CandidateScore,
    shape_rejected: bool,
}

struct EvaluatedCallArguments {
    arguments: Vec<CheckedCallArgumentFact>,
    hard_errors: usize,
    exact_matches: usize,
    substitutions: TypeParameterSubstitutions,
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
            modules,
            style_value_kinds,
            types: BTreeMap::new(),
            type_reports: BTreeMap::new(),
            iteration_facts: BTreeMap::new(),
            facts: SemanticFactState::new(),
            staged_callables: None,
            physical_candidate_argument_evaluations: BTreeMap::new(),
            callable_query_depth: CallableQueryDepth::new(catalogs.callable_limits),
            physical_call_stack: Vec::new(),
            implicit_callable_stack: Vec::new(),
            pipe_stack: Vec::new(),
            function_site_stack: Vec::new(),
        })
    }

    fn analyze(mut self) -> Result<FinalSemanticAnalysis, FinalSemanticAnalysisError> {
        self.analyze_staged()
    }

    fn analyze_staged(&mut self) -> Result<FinalSemanticAnalysis, FinalSemanticAnalysisError> {
        self.resolve_all_types()?;
        self.seed_local_types()?;
        let staged_callables = self.stage_checked_callables()?;
        for (owner, fact) in &staged_callables.effect_expressions {
            if self.facts.set_expression(*owner, fact.clone()) {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        }
        self.staged_callables = Some(staged_callables);
        self.infer_statement_bindings()?;
        self.validate_callable_body_results()?;
        self.analyze_all_expressions()?;
        self.finalize_unannotated_locals()?;

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
        for module in self.modules.values() {
            for (owner, capture) in module.captures() {
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
                input.push_capture(owner, CheckedBinding::with_role(ty, role));
            }
        }
        self.analyze_patterns(&mut input)?;
        self.analyze_statements(&mut input)?;
        self.analyze_items(&mut input)?;
        for (owner, fact) in self.facts.expressions() {
            input.push_expression(*owner, fact.clone());
        }
        let staged = self
            .staged_callables
            .take()
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let checked_callables = self.finish_checked_callables(staged, &input)?;
        self.finalize_call_facts(&checked_callables)?;
        input.expressions.clear();
        for (owner, fact) in self.facts.expressions() {
            input.push_expression(*owner, fact.clone());
        }
        self.validate_flow_effect_bounds(&input)?;
        for call in self.facts.calls().values() {
            input.push_call(call.clone());
        }
        input.set_physical_candidate_argument_evaluations(
            self.physical_candidate_argument_evaluations
                .iter()
                .map(|(root, evaluations)| (*root, Arc::from(evaluations.clone())))
                .collect(),
        );
        self.control.check()?;
        FinalSemanticAnalysis::try_new_with_control_and_type_resolutions(
            self.executable,
            self.symbols,
            checked_callables,
            input,
            std::mem::take(&mut self.type_reports),
            self.control,
        )
    }

    pub(super) fn record_physical_candidate_argument_evaluation(
        &mut self,
        evaluation: PhysicalCandidateArgumentEvaluation,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let root = self
            .physical_call_stack
            .first()
            .copied()
            .expect("physical evaluation occurs inside a Call analysis");
        let limit = self
            .catalogs
            .callable_limits
            .max_query_work()
            .checked_mul(
                u64::try_from(self.catalogs.callable_limits.max_nested_calls())
                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
            )
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
        let trace = self
            .physical_candidate_argument_evaluations
            .entry(root)
            .or_default();
        let observed = u64::try_from(trace.len())
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        if observed >= limit {
            return Err(FinalSemanticAnalysisError::CallResolutionFailed {
                owner: evaluation.call_expression(),
            });
        }
        trace.push(evaluation);
        Ok(())
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

#[path = "analyzer/callable_effect_graph.rs"]
mod callable_effect_graph;
#[path = "analyzer/calls.rs"]
mod calls;
#[path = "analyzer/entities.rs"]
mod entities;
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

use state::{CandidateSemanticProjection, SemanticFactState};
