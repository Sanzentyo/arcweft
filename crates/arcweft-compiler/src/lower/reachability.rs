use std::collections::BTreeSet;

use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::{ExprId, ItemId, StmtId},
    item::HirItemKind,
    project::{
        HirExecutableProjectView, HirRuntimeCallCalleeDisposition, HirRuntimeEmissionMode,
        HirRuntimeExecutableOwner, HirRuntimeExpressionProjection, HirRuntimeReachabilityEdge,
        HirRuntimeReachabilityEdgeKind, HirRuntimeReachabilityError, HirRuntimeReachabilityPath,
        HirRuntimeReachabilityRoot, HirRuntimeReachabilityRootKind, HirRuntimeReachabilitySite,
        HirRuntimeSemanticReachability, HirRuntimeSemanticReachabilityInput,
        HirRuntimeValueRetention,
    },
    scope::HirScopeOwner,
    stmt::HirStmtKind,
    symbol::{CallableDeclarationKey, ProjectSymbolTable},
};
use arcweft_lang_sema::{
    callable::{
        CallTargetFacts, CheckedCallArgumentSlotSource, CheckedCallCalleeExecution,
        CheckedCallSite, ResolvedCallableOrigin, ResolvedCallableState,
    },
    entry::{CheckedEntryBinding, CheckedEntryCatalog},
    final_analysis::{
        CheckedCallRuntimeCalleeDisposition, CheckedChoice, CheckedDropInvocation,
        CheckedEvaluatedEffect, CheckedEvaluatedEffectOperation, CheckedExpressionResolution,
        CheckedExpressionRuntimeDisposition, CheckedItemRole, CheckedOrdinaryFunctionEmission,
        CheckedStatementPayload, FinalSemanticAnalysis, FinalSemanticAnalysisError,
        PostfixBracketResolution,
    },
};
use thiserror::Error;

use crate::project::ProjectEntrySelection;

#[derive(Clone, Copy)]
pub enum RuntimeEmissionMode<'selection> {
    CheckAll,
    SelectedEntry(&'selection ProjectEntrySelection),
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeReachabilityProjectionError {
    #[error(transparent)]
    Generation(#[from] FinalSemanticAnalysisError),
    #[error(transparent)]
    ExpressionDisposition(
        #[from] arcweft_lang_sema::final_analysis::CheckedExpressionRuntimeDispositionError,
    ),
    #[error(transparent)]
    Hir(#[from] HirRuntimeReachabilityError),
    #[error("selected Entry has no accepted checked root")]
    MissingSelectedEntry,
    #[error("checked Call {owner:?} has no selected call authority")]
    MissingSelectedCallAuthority { owner: ExprId },
    #[error("checked expression {owner:?} has no matching final-HIR expression")]
    MissingHirExpression { owner: ExprId },
    #[error(
        "evaluated-effect carrier {owner:?} is invalid (dialogue application: {dialogue_application:?}): {reason}"
    )]
    InvalidEvaluatedEffectCarrier {
        owner: ExprId,
        dialogue_application: Option<ExprId>,
        reason: &'static str,
    },
    #[error("checked project executable edge is missing")]
    MissingCheckedEdge {
        site: HirRuntimeReachabilitySite,
        expected_target: Box<HirRuntimeExecutableOwner>,
    },
    #[error("checked project executable edge does not match its exact checked row")]
    MismatchedCheckedEdge {
        site: HirRuntimeReachabilitySite,
        expected: Box<HirRuntimeReachabilityEdge>,
        actual: Box<HirRuntimeReachabilityEdge>,
    },
    #[error("checked runtime reachability contains an unexpected executable edge")]
    UnexpectedCheckedEdge {
        site: HirRuntimeReachabilitySite,
        actual: Box<HirRuntimeReachabilityEdge>,
    },
    #[error("reachable executable has no checked item")]
    MissingCheckedItem { owner: ItemId },
    #[error("reachable executable {owner:?} has no deterministic root path")]
    MissingReachabilityPath { owner: HirRuntimeExecutableOwner },
    #[error(
        "reachable ordinary function {owner:?} cannot be emitted by the current runtime: {reason:?}"
    )]
    UnsupportedOrdinaryFunction {
        owner: ItemId,
        reason: CheckedOrdinaryFunctionEmission,
        path: Box<HirRuntimeReachabilityPath>,
        suspension_site: Option<ExprId>,
    },
}

impl RuntimeReachabilityProjectionError {
    pub const fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::UnsupportedOrdinaryFunction { reason, .. } => reason.diagnostic_code(),
            Self::Generation(_) => "compiler.runtime_reachability.stale_generation",
            Self::ExpressionDisposition(_) => {
                "compiler.runtime_reachability.expression_disposition"
            }
            Self::MissingSelectedEntry
            | Self::MissingCheckedItem { .. }
            | Self::MissingReachabilityPath { .. }
            | Self::MissingHirExpression { .. } => "compiler.runtime_reachability.invalid_root",
            Self::MissingSelectedCallAuthority { .. } => {
                "compiler.runtime_reachability.missing_selected_call_authority"
            }
            Self::InvalidEvaluatedEffectCarrier { .. } => {
                "compiler.runtime_reachability.invalid_evaluated_effect_carrier"
            }
            Self::MissingCheckedEdge { .. } => "compiler.runtime_reachability.missing_checked_edge",
            Self::MismatchedCheckedEdge { .. } | Self::UnexpectedCheckedEdge { .. } => {
                "compiler.runtime_reachability.mismatched_checked_edge"
            }
            Self::Hir(HirRuntimeReachabilityError::PresentationTarget { .. }) => {
                "compiler.runtime_reachability.presentation_target"
            }
            Self::Hir(HirRuntimeReachabilityError::LimitExceeded { .. }) => {
                "compiler.runtime_reachability.limit_exceeded"
            }
            Self::Hir(_) => "compiler.runtime_reachability.invalid_edge",
        }
    }
}

pub fn project_runtime_reachability<'project>(
    project: HirExecutableProjectView<'project>,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
    entries: &CheckedEntryCatalog,
    mode: RuntimeEmissionMode<'_>,
) -> Result<HirRuntimeSemanticReachability<'project>, RuntimeReachabilityProjectionError> {
    analysis.validate_generation(project, symbols)?;
    let (hir_mode, selected_entries) = match mode {
        RuntimeEmissionMode::CheckAll => (
            HirRuntimeEmissionMode::CheckAll,
            entries.entries().collect::<Vec<_>>(),
        ),
        RuntimeEmissionMode::SelectedEntry(selection) => (
            HirRuntimeEmissionMode::SelectedEntry,
            vec![
                entries
                    .get_public(selection.id())
                    .ok_or(RuntimeReachabilityProjectionError::MissingSelectedEntry)?,
            ],
        ),
    };
    let mut roots = if hir_mode == HirRuntimeEmissionMode::CheckAll {
        analysis
            .items()
            .filter(|(_, checked)| matches!(checked.role(), CheckedItemRole::Flow { .. }))
            .map(|(owner, _)| {
                HirRuntimeReachabilityRoot::new(
                    HirRuntimeReachabilityRootKind::CheckedFlow,
                    HirRuntimeExecutableOwner::Item(owner),
                )
            })
            .collect::<BTreeSet<_>>()
    } else {
        BTreeSet::new()
    };
    let entry_root_kind = match hir_mode {
        HirRuntimeEmissionMode::CheckAll => HirRuntimeReachabilityRootKind::CheckedEntry,
        HirRuntimeEmissionMode::SelectedEntry => HirRuntimeReachabilityRootKind::SelectedEntry,
    };
    roots.extend(selected_entries.iter().map(|entry| {
        HirRuntimeReachabilityRoot::new(
            entry_root_kind,
            HirRuntimeExecutableOwner::Item(entry.source_item()),
        )
    }));
    let mut edges = BTreeSet::new();
    for (call, facts) in analysis.calls() {
        if let Some(edge) = checked_call_edge(call, facts, symbols)? {
            edges.insert(edge);
        }
    }
    for (owner, checked) in analysis.expressions() {
        let CheckedExpressionResolution::Choice(choice) = checked.resolution() else {
            continue;
        };
        edges.extend(checked_choice_edges(owner, choice));
    }
    for (statement, checked) in analysis.statements() {
        let CheckedStatementPayload::Iteration(iteration) = checked.payload() else {
            continue;
        };
        edges.extend(checked_iteration_edges(statement, iteration));
    }
    for (owner, checked) in analysis.expressions() {
        let CheckedExpressionResolution::Closure(closure) = checked.resolution() else {
            continue;
        };
        edges.insert(checked_closure_execution_edge(owner, closure.owner()));
    }
    for entry in &selected_entries {
        append_entry_edges(entry, symbols, &mut edges)?;
    }
    let evaluated_effect_carriers =
        evaluated_effect_carriers(project, analysis).map_err(|error| {
            RuntimeReachabilityProjectionError::InvalidEvaluatedEffectCarrier {
                owner: error.owner(),
                dialogue_application: error.dialogue_application(),
                reason: error.reason(),
            }
        })?;
    let input = HirRuntimeSemanticReachabilityInput::try_new(
        hir_mode,
        symbols.world().clone(),
        *symbols.revision(),
        roots.into_iter().collect(),
        edges.into_iter().collect(),
    )?;
    let mut projection_error = None;
    let mut expression_projection = |owner| {
        if projection_error.is_some() {
            return None;
        }
        match runtime_expression_projection_for_owner(analysis, owner, &evaluated_effect_carriers) {
            Ok(projection) => Some(projection),
            Err(error) => {
                projection_error = Some(error);
                None
            }
        }
    };
    let reachability = project.runtime_semantic_reachability(
        input,
        analysis.hir_topology().as_ref(),
        |owner| {
            let expression = analysis.expression(owner)?;
            let arcweft_lang_sema::final_analysis::CheckedExpressionResolution::PostfixBracket(
                resolution,
            ) = expression.resolution()
            else {
                return None;
            };
            Some(resolution.candidate())
        },
        &mut expression_projection,
    );
    if let Some(error) = projection_error {
        return Err(error);
    }
    let reachability = reachability?;
    validate_checked_executable_edges(symbols, analysis, &selected_entries, &reachability)?;
    Ok(reachability)
}

/// Projects View handler value programs through an execution-owner inventory
/// that is disjoint from ordinary Flow/Entry roots. Captured View parameters
/// enter only this transaction and therefore cannot become Flow locals.
pub(crate) fn project_view_value_program_reachability<'project>(
    project: HirExecutableProjectView<'project>,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
    handler_closures: impl IntoIterator<Item = ExprId>,
) -> Result<HirRuntimeSemanticReachability<'project>, RuntimeReachabilityProjectionError> {
    analysis.validate_generation(project, symbols)?;
    let roots = handler_closures
        .into_iter()
        .map(|closure| {
            HirRuntimeReachabilityRoot::new(
                HirRuntimeReachabilityRootKind::CheckedViewValueProgram,
                HirRuntimeExecutableOwner::Closure(closure),
            )
        })
        .collect::<BTreeSet<_>>();
    let mut edges = BTreeSet::new();
    for (call, facts) in analysis.calls() {
        if let Some(edge) = checked_call_edge(call, facts, symbols)? {
            edges.insert(edge);
        }
    }
    for (owner, checked) in analysis.expressions() {
        let CheckedExpressionResolution::Choice(choice) = checked.resolution() else {
            continue;
        };
        edges.extend(checked_choice_edges(owner, choice));
    }
    for (statement, checked) in analysis.statements() {
        let CheckedStatementPayload::Iteration(iteration) = checked.payload() else {
            continue;
        };
        edges.extend(checked_iteration_edges(statement, iteration));
    }
    for (owner, checked) in analysis.expressions() {
        let CheckedExpressionResolution::Closure(closure) = checked.resolution() else {
            continue;
        };
        edges.insert(checked_closure_execution_edge(owner, closure.owner()));
    }
    let evaluated_effect_carriers =
        evaluated_effect_carriers(project, analysis).map_err(|error| {
            RuntimeReachabilityProjectionError::InvalidEvaluatedEffectCarrier {
                owner: error.owner(),
                dialogue_application: error.dialogue_application(),
                reason: error.reason(),
            }
        })?;
    let input = HirRuntimeSemanticReachabilityInput::try_new(
        HirRuntimeEmissionMode::CheckAll,
        symbols.world().clone(),
        *symbols.revision(),
        roots.into_iter().collect(),
        edges.into_iter().collect(),
    )?;
    let mut projection_error = None;
    let mut expression_projection = |owner| {
        if projection_error.is_some() {
            return None;
        }
        match runtime_expression_projection_for_owner(analysis, owner, &evaluated_effect_carriers) {
            Ok(projection) => Some(projection),
            Err(error) => {
                projection_error = Some(error);
                None
            }
        }
    };
    let reachability = project.runtime_semantic_reachability(
        input,
        analysis.hir_topology().as_ref(),
        |owner| {
            let expression = analysis.expression(owner)?;
            let CheckedExpressionResolution::PostfixBracket(resolution) = expression.resolution()
            else {
                return None;
            };
            Some(resolution.candidate())
        },
        &mut expression_projection,
    );
    if let Some(error) = projection_error {
        return Err(error);
    }
    let reachability = reachability?;
    validate_checked_executable_edges(symbols, analysis, &[], &reachability)?;
    Ok(reachability)
}

fn checked_call_edge(
    call: ExprId,
    facts: &arcweft_lang_sema::callable::CallTargetFacts,
    symbols: &ProjectSymbolTable,
) -> Result<Option<HirRuntimeReachabilityEdge>, RuntimeReachabilityProjectionError> {
    if facts.outcome().site() != CheckedCallSite::HirCall(call) {
        return Ok(None);
    }
    let Some(application) = facts.selected_application() else {
        return Ok(None);
    };
    if !matches!(
        application.core().callee(),
        CheckedCallCalleeExecution::Direct
    ) {
        return Ok(None);
    }
    let selected = application.core().candidates().selected();
    let ResolvedCallableOrigin::Project { declaration, .. } = selected.origin() else {
        return Ok(None);
    };
    let Some(target) = runtime_owner_for_declaration(symbols, declaration) else {
        return Ok(None);
    };
    let source = HirRuntimeReachabilitySite::Expression(call);
    let kind = match &target {
        HirRuntimeExecutableOwner::ImplMethod(method) => {
            let implementation = symbols
                .callable_symbols()
                .find(|symbol| symbol.declaration() == declaration)
                .map(arcweft_lang_hir::symbol::CallableSymbol::source_item)
                .ok_or_else(|| RuntimeReachabilityProjectionError::MissingCheckedEdge {
                    site: source,
                    expected_target: Box::new(target.clone()),
                })?;
            HirRuntimeReachabilityEdgeKind::CheckedTraitMethodCall {
                call,
                implementation,
                method: method.clone(),
            }
        }
        HirRuntimeExecutableOwner::Item(_) | HirRuntimeExecutableOwner::Closure(_) => {
            HirRuntimeReachabilityEdgeKind::CheckedProjectCall {
                call,
                declaration: declaration.clone(),
            }
        }
    };
    Ok(Some(HirRuntimeReachabilityEdge::new(source, target, kind)))
}

fn checked_choice_edges(
    owner: ExprId,
    choice: &CheckedChoice,
) -> BTreeSet<HirRuntimeReachabilityEdge> {
    let source = HirRuntimeReachabilitySite::Expression(owner);
    choice
        .gotos()
        .iter()
        .filter_map(|goto| goto.target().flow_owner())
        .map(|(declaration, target)| {
            HirRuntimeReachabilityEdge::new(
                source,
                HirRuntimeExecutableOwner::Item(target),
                HirRuntimeReachabilityEdgeKind::CheckedFlowTransfer {
                    source,
                    declaration: declaration.clone(),
                },
            )
        })
        .collect()
}

fn checked_iteration_edges(
    statement: StmtId,
    iteration: &arcweft_lang_sema::final_analysis::CheckedIteration,
) -> BTreeSet<HirRuntimeReachabilityEdge> {
    let source = HirRuntimeReachabilitySite::Statement(statement);
    iteration
        .witness_methods()
        .map(|(role, conformance, _)| {
            let method = conformance.declaration().clone();
            HirRuntimeReachabilityEdge::new(
                source,
                HirRuntimeExecutableOwner::ImplMethod(method.clone()),
                HirRuntimeReachabilityEdgeKind::CheckedIteratorWitnessMethod {
                    role,
                    implementation: conformance.implementation(),
                    member: conformance.method(),
                    method,
                },
            )
        })
        .collect()
}

fn checked_closure_execution_edge(source: ExprId, closure: ExprId) -> HirRuntimeReachabilityEdge {
    HirRuntimeReachabilityEdge::new(
        HirRuntimeReachabilitySite::Expression(source),
        HirRuntimeExecutableOwner::Closure(closure),
        HirRuntimeReachabilityEdgeKind::CheckedClosureExecution { closure },
    )
}

pub fn validate_reachable_runtime_callables(
    analysis: &FinalSemanticAnalysis,
    reachability: &HirRuntimeSemanticReachability<'_>,
) -> Result<(), RuntimeReachabilityProjectionError> {
    for executable in reachability.reachable_executables() {
        let HirRuntimeExecutableOwner::Item(owner) = executable else {
            continue;
        };
        let item = analysis
            .item(*owner)
            .ok_or(RuntimeReachabilityProjectionError::MissingCheckedItem { owner: *owner })?;
        let Some(reason) = item.role().ordinary_function_emission(item.effects()) else {
            continue;
        };
        if reason == CheckedOrdinaryFunctionEmission::PureDirectFrame {
            continue;
        }
        let path = reachability.first_path(executable).ok_or_else(|| {
            RuntimeReachabilityProjectionError::MissingReachabilityPath {
                owner: executable.clone(),
            }
        })?;
        return Err(
            RuntimeReachabilityProjectionError::UnsupportedOrdinaryFunction {
                owner: *owner,
                reason,
                path: Box::new(path.clone()),
                suspension_site: first_direct_suspension_site(analysis, reachability, *owner),
            },
        );
    }
    Ok(())
}

fn first_direct_suspension_site(
    analysis: &FinalSemanticAnalysis,
    reachability: &HirRuntimeSemanticReachability<'_>,
    owner: ItemId,
) -> Option<ExprId> {
    let project = reachability.project();
    let module = project.modules().find_map(|(_, module)| {
        (module.module_id() == owner.module()).then_some(module.as_ref())
    })?;
    let HirItemKind::Function(function) = module.resolve_item(owner).ok()?.kind() else {
        return None;
    };
    let callable_scope = function.callable_scope();
    module.expressions().find_map(|(expression, hir)| {
        (reachability.contains_expression(expression)
            && scope_is_direct_callable_descendant(module, hir.scope(), callable_scope)
            && analysis.expression(expression).is_some_and(|checked| {
                matches!(checked.resolution(), CheckedExpressionResolution::Await(_))
            }))
        .then_some(expression)
    })
}

fn scope_is_direct_callable_descendant(
    module: &arcweft_lang_hir::module::HirModule,
    mut scope: arcweft_lang_hir::identity::ScopeId,
    callable_scope: arcweft_lang_hir::identity::ScopeId,
) -> bool {
    loop {
        if scope == callable_scope {
            return true;
        }
        let Ok(record) = module.resolve_scope(scope) else {
            return false;
        };
        if let HirScopeOwner::Expr(expression) = record.owner()
            && module
                .resolve_expr(*expression)
                .is_ok_and(|expression| matches!(expression.kind(), HirExprKind::Closure(_)))
        {
            return false;
        }
        let Some(parent) = record.parent() else {
            return false;
        };
        scope = parent;
    }
}

fn append_entry_edges(
    entry: &CheckedEntryBinding,
    symbols: &ProjectSymbolTable,
    edges: &mut BTreeSet<HirRuntimeReachabilityEdge>,
) -> Result<(), RuntimeReachabilityProjectionError> {
    let source = entry.source_item();
    match entry {
        CheckedEntryBinding::Stateful(entry) => {
            append_entry_callable(source, entry.initializer().declaration(), symbols, edges)?;
            append_entry_callable(source, entry.reducer().declaration(), symbols, edges)?;
            let flow = symbols
                .flow_symbol_for_item(entry.initial_flow().source_item())
                .ok_or(RuntimeReachabilityProjectionError::MissingCheckedEdge {
                    site: HirRuntimeReachabilitySite::Item(source),
                    expected_target: Box::new(HirRuntimeExecutableOwner::Item(
                        entry.initial_flow().source_item(),
                    )),
                })?;
            append_entry_declaration(source, flow.declaration(), symbols, edges)?;
        }
        CheckedEntryBinding::Agent(entry) => {
            append_entry_callable(source, entry.controller().declaration(), symbols, edges)?;
        }
        CheckedEntryBinding::Existing(entry) => {
            entry.target().visit_flows(|target| {
                append_entry_declaration(
                    source,
                    &CallableDeclarationKey::Flow(target.declaration().clone()),
                    symbols,
                    edges,
                )
            })?;
        }
    }
    Ok(())
}

fn append_entry_callable(
    entry: ItemId,
    declaration: &arcweft_lang_hir::symbol::CallableDeclarationId,
    symbols: &ProjectSymbolTable,
    edges: &mut BTreeSet<HirRuntimeReachabilityEdge>,
) -> Result<(), RuntimeReachabilityProjectionError> {
    append_entry_declaration(
        entry,
        &CallableDeclarationKey::Existing(declaration.clone()),
        symbols,
        edges,
    )
}

fn append_entry_declaration(
    entry: ItemId,
    declaration: &CallableDeclarationKey,
    symbols: &ProjectSymbolTable,
    edges: &mut BTreeSet<HirRuntimeReachabilityEdge>,
) -> Result<(), RuntimeReachabilityProjectionError> {
    let target = runtime_owner_for_declaration(symbols, declaration).ok_or({
        RuntimeReachabilityProjectionError::MissingCheckedEdge {
            site: HirRuntimeReachabilitySite::Item(entry),
            expected_target: Box::new(HirRuntimeExecutableOwner::Item(entry)),
        }
    })?;
    edges.insert(HirRuntimeReachabilityEdge::new(
        HirRuntimeReachabilitySite::Item(entry),
        target,
        HirRuntimeReachabilityEdgeKind::CheckedEntryBinding {
            entry,
            declaration: declaration.clone(),
        },
    ));
    Ok(())
}

fn runtime_owner_for_declaration(
    symbols: &ProjectSymbolTable,
    declaration: &CallableDeclarationKey,
) -> Option<HirRuntimeExecutableOwner> {
    match declaration {
        CallableDeclarationKey::ImplMethod(method) => {
            Some(HirRuntimeExecutableOwner::ImplMethod(method.clone()))
        }
        CallableDeclarationKey::Existing(existing)
            if existing.owner().owns_runtime_executable_body() =>
        {
            symbols
                .callable_symbols()
                .find(|symbol| symbol.declaration() == declaration)
                .map(|symbol| HirRuntimeExecutableOwner::Item(symbol.source_item()))
        }
        CallableDeclarationKey::Flow(_) => symbols
            .callable_symbols()
            .find(|symbol| symbol.declaration() == declaration)
            .map(|symbol| HirRuntimeExecutableOwner::Item(symbol.source_item())),
        CallableDeclarationKey::Existing(_) => None,
        CallableDeclarationKey::TraitRequirement(_) => None,
    }
}

fn runtime_expression_projection_for_owner(
    analysis: &FinalSemanticAnalysis,
    owner: ExprId,
    evaluated_effect_carriers: &BTreeSet<ExprId>,
) -> Result<HirRuntimeExpressionProjection, RuntimeReachabilityProjectionError> {
    let checked = analysis
        .expression(owner)
        .ok_or(RuntimeReachabilityProjectionError::MissingHirExpression { owner })?;
    let value = if evaluated_effect_carriers.contains(&owner)
        || matches!(
            checked.resolution(),
            CheckedExpressionResolution::DialogueApplication { .. }
                | CheckedExpressionResolution::PostfixBracket(
                    PostfixBracketResolution::Dialogue { .. }
                )
        ) {
        HirRuntimeValueRetention::Omit
    } else {
        HirRuntimeValueRetention::Retain
    };
    match analysis.runtime_expression_disposition(owner)? {
        CheckedExpressionRuntimeDisposition::Structural => {
            Ok(HirRuntimeExpressionProjection::Structural { value })
        }
        CheckedExpressionRuntimeDisposition::Call(disposition) => {
            let callee = match disposition {
                CheckedCallRuntimeCalleeDisposition::Static => {
                    HirRuntimeCallCalleeDisposition::Static
                }
                CheckedCallRuntimeCalleeDisposition::RuntimeReceiver => {
                    HirRuntimeCallCalleeDisposition::RuntimeReceiver
                }
            };
            Ok(HirRuntimeExpressionProjection::Call {
                result: value,
                callee,
            })
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(super) enum EvaluatedEffectCarrierError {
    #[error("evaluated-effect application has no selected call authority")]
    MissingSelectedCall {
        owner: ExprId,
        dialogue_application: Option<ExprId>,
    },
    #[error("evaluated-effect metadata has no expression source")]
    InvalidMetadataSource {
        owner: ExprId,
        dialogue_application: Option<ExprId>,
    },
}

impl EvaluatedEffectCarrierError {
    pub(super) const fn owner(self) -> ExprId {
        match self {
            Self::MissingSelectedCall { owner, .. } | Self::InvalidMetadataSource { owner, .. } => {
                owner
            }
        }
    }

    pub(super) const fn dialogue_application(self) -> Option<ExprId> {
        match self {
            Self::MissingSelectedCall {
                dialogue_application,
                ..
            }
            | Self::InvalidMetadataSource {
                dialogue_application,
                ..
            } => dialogue_application,
        }
    }

    pub(super) const fn reason(self) -> &'static str {
        match self {
            Self::MissingSelectedCall { .. } => {
                "evaluated-effect application has no selected call authority"
            }
            Self::InvalidMetadataSource { .. } => {
                "evaluated-effect metadata has no expression source"
            }
        }
    }
}

pub(super) fn evaluated_effect_carriers(
    project: HirExecutableProjectView<'_>,
    analysis: &FinalSemanticAnalysis,
) -> Result<BTreeSet<ExprId>, EvaluatedEffectCarrierError> {
    let mut carriers = BTreeSet::new();
    for (statement, checked) in analysis.statements() {
        let CheckedStatementPayload::EvaluatedEffect(effect) = checked.payload() else {
            continue;
        };
        let statement_expression = resolve_hir_statement_expression(project, statement).ok_or(
            EvaluatedEffectCarrierError::InvalidMetadataSource {
                owner: effect.application().raw().expression(),
                dialogue_application: None,
            },
        )?;
        extend_evaluated_effect_carriers(effect, analysis, None, &mut carriers)?;
        carriers.insert(statement_expression);
    }
    for (owner, expression) in analysis.expressions() {
        let CheckedExpressionResolution::DialogueApplication { line_plan, .. } =
            expression.resolution()
        else {
            continue;
        };
        for site in line_plan.effect_sites() {
            extend_evaluated_effect_carriers(site.effect(), analysis, Some(owner), &mut carriers)?;
        }
    }
    Ok(carriers)
}

fn extend_evaluated_effect_carriers(
    effect: &CheckedEvaluatedEffect,
    analysis: &FinalSemanticAnalysis,
    dialogue_application: Option<ExprId>,
    carriers: &mut BTreeSet<ExprId>,
) -> Result<(), EvaluatedEffectCarrierError> {
    let mut current = Some(effect.application().raw().expression());
    while let Some(owner) = current {
        if !carriers.insert(owner) {
            break;
        }
        let application = analysis
            .call(owner)
            .and_then(CallTargetFacts::selected_application)
            .ok_or(EvaluatedEffectCarrierError::MissingSelectedCall {
                owner,
                dialogue_application,
            })?;
        current = match application.core().candidates().selected().state() {
            ResolvedCallableState::Base => None,
            ResolvedCallableState::Continuation(continuation) => {
                Some(continuation.prefix_call_site().expression())
            }
        };
    }
    let CheckedEvaluatedEffectOperation::Drop {
        invocation: CheckedDropInvocation::DropWithPolicy { source, .. },
        ..
    } = effect.operation()
    else {
        return Ok(());
    };
    let CheckedCallArgumentSlotSource::Expression(owner) = source.operand().source().raw() else {
        return Err(EvaluatedEffectCarrierError::InvalidMetadataSource {
            owner: effect.application().raw().expression(),
            dialogue_application,
        });
    };
    if analysis.call(owner).is_some()
        && analysis
            .call(owner)
            .and_then(CallTargetFacts::selected_application)
            .is_none()
    {
        return Err(EvaluatedEffectCarrierError::MissingSelectedCall {
            owner,
            dialogue_application,
        });
    }
    carriers.insert(owner);
    Ok(())
}

fn resolve_hir_statement_expression(
    project: HirExecutableProjectView<'_>,
    owner: StmtId,
) -> Option<ExprId> {
    let statement = project
        .modules()
        .find(|(_, module)| module.module_id() == owner.module())?
        .1
        .resolve_stmt(owner)
        .ok()?;
    let HirStmtKind::Expression { expression } = statement.kind() else {
        return None;
    };
    Some(*expression)
}

fn validate_checked_executable_edges(
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
    selected_entries: &[&CheckedEntryBinding],
    reachability: &HirRuntimeSemanticReachability<'_>,
) -> Result<(), RuntimeReachabilityProjectionError> {
    for (call, facts) in analysis.calls() {
        if !reachability.contains_expression(call) {
            continue;
        }
        let Some(edge) = checked_call_edge(call, facts, symbols)? else {
            continue;
        };
        validate_exact_edge_set(
            reachability,
            HirRuntimeReachabilitySite::Expression(call),
            &BTreeSet::from([edge]),
        )?;
    }
    for (owner, checked) in analysis.expressions() {
        if !reachability.contains_expression(owner) {
            continue;
        }
        let expected = match checked.resolution() {
            CheckedExpressionResolution::Choice(choice) => checked_choice_edges(owner, choice),
            CheckedExpressionResolution::Closure(closure) => {
                BTreeSet::from([checked_closure_execution_edge(owner, closure.owner())])
            }
            _ => continue,
        };
        validate_exact_edge_set(
            reachability,
            HirRuntimeReachabilitySite::Expression(owner),
            &expected,
        )?;
    }
    for (statement, checked) in analysis.statements() {
        if !reachability.contains_statement(statement) {
            continue;
        }
        let CheckedStatementPayload::Iteration(iteration) = checked.payload() else {
            continue;
        };
        validate_exact_edge_set(
            reachability,
            HirRuntimeReachabilitySite::Statement(statement),
            &checked_iteration_edges(statement, iteration),
        )?;
    }
    for entry in selected_entries {
        let site = HirRuntimeReachabilitySite::Item(entry.source_item());
        if !reachability
            .contains_runtime_owner(&HirRuntimeExecutableOwner::Item(entry.source_item()))
        {
            continue;
        }
        let mut expected = BTreeSet::new();
        append_entry_edges(entry, symbols, &mut expected)?;
        validate_exact_edge_set(reachability, site, &expected)?;
    }
    Ok(())
}

fn validate_exact_edge_set(
    reachability: &HirRuntimeSemanticReachability<'_>,
    site: HirRuntimeReachabilitySite,
    expected: &BTreeSet<HirRuntimeReachabilityEdge>,
) -> Result<(), RuntimeReachabilityProjectionError> {
    let actual = reachability
        .edge_from(site)
        .cloned()
        .collect::<BTreeSet<_>>();
    if let Some(missing) = expected.difference(&actual).next() {
        if let Some(unexpected) = actual.difference(expected).next() {
            return Err(RuntimeReachabilityProjectionError::MismatchedCheckedEdge {
                site,
                expected: Box::new(missing.clone()),
                actual: Box::new(unexpected.clone()),
            });
        }
        return Err(RuntimeReachabilityProjectionError::MissingCheckedEdge {
            site,
            expected_target: Box::new(missing.target().clone()),
        });
    }
    if let Some(unexpected) = actual.difference(expected).next() {
        return Err(RuntimeReachabilityProjectionError::UnexpectedCheckedEdge {
            site,
            actual: Box::new(unexpected.clone()),
        });
    }
    Ok(())
}
