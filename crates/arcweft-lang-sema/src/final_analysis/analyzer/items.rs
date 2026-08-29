//! Item roles and checked callable-catalog publication.

use super::{
    Analyzer, Arc, BTreeMap, CallableAccess, CallableCandidateId, CallableEffectContract,
    CallableEffectSchema, CallableMethodRole, CheckedCallableCatalog,
    CheckedCallableCatalogBuildError, CheckedCallableCatalogBuilder, CheckedCallableDeclaration,
    CheckedCallableExecution, CheckedCallableId, CheckedExpression, CheckedExpressionResolution,
    CheckedFunctionExecution, CheckedItem, CheckedItemRole, CheckedSuspensionRole, EffectId,
    EffectRow, EffectSet, EffectSubsetError, ExprId, FinalSemanticAnalysisError,
    FinalSemanticAnalysisInput, HirCallCallee, HirCallableSourceOwner, HirExprKind,
    HirFlowContractClause, HirFlowContractSourcePart, HirFlowSourceRole, HirFunctionBody,
    HirImplMember, HirItem, HirItemKind, HirItemSourceRole, HirModule, HirPathSegment,
    HirPredicateBody, HirProofBody, HirSourceQuery, HirStmtKind, HirTraitMember, ItemId,
    ProjectSymbolTable, STANDARD_TRAIT_CATALOG_VERSION, ScopeId, SourceSpan, StagedCallableBody,
    StagedCheckedCallables, TypeId, TypeKind,
    callable_effect_graph::CallableEffectGraph,
    calls::{AnalyzerPreparedCallGraph, AnalyzerPreparedCallPrefix},
    statements::{checked_effect_expression, function_effect_contract, scope_span, source_span},
};
use crate::{
    callable::{
        CallTargetFacts, CheckedCallCalleeExecution, CheckedCallSite, EffectPermission,
        PreparedCallGraphSelectedNode, ResolvedCallableState,
    },
    semantic_coordinate::StableCheckedValueCoordinate,
};
use arcweft_lang_hir::{
    body_edges::{HirBodyChild, HirBodyProjection},
    expr::{
        HirComputationBlockKind, HirExpressionChildOwnership, HirExpressionChildRole,
        HirExpressionOwnedChild,
    },
    stmt::{HirStatementBodyRole, HirStatementChild, HirStatementChildRole},
};

fn prepared_call_node<'a>(
    prepared_calls: &'a AnalyzerPreparedCallGraph,
    owner: ExprId,
) -> Option<PreparedCallGraphSelectedNode<'a, AnalyzerPreparedCallPrefix>> {
    prepared_calls
        .selected_nodes()
        .find(|node| node.site() == CheckedCallSite::HirCall(owner))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EffectTraceCallDispatch {
    Direct,
    Value,
}

struct EffectTraceSelectedCall<'a> {
    dispatch: EffectTraceCallDispatch,
    producer: Option<ExprId>,
    argument_sources: Box<[ExprId]>,
    selected: &'a CallableCandidateId,
}

trait EffectTraceCallAuthority {
    fn selected_call(
        &self,
        owner: ExprId,
    ) -> Result<Option<EffectTraceSelectedCall<'_>>, FinalSemanticAnalysisError>;
}

struct PreparedEffectTraceCallAuthority<'a> {
    graph: &'a AnalyzerPreparedCallGraph,
}

impl<'a> PreparedEffectTraceCallAuthority<'a> {
    const fn new(graph: &'a AnalyzerPreparedCallGraph) -> Self {
        Self { graph }
    }
}

impl EffectTraceCallAuthority for PreparedEffectTraceCallAuthority<'_> {
    fn selected_call(
        &self,
        owner: ExprId,
    ) -> Result<Option<EffectTraceSelectedCall<'_>>, FinalSemanticAnalysisError> {
        let Some(node) = prepared_call_node(self.graph, owner) else {
            return Ok(None);
        };
        let prefix = node.prefix();
        let application = prefix.application();
        let record = prefix.record();
        let origin = record.function_value_origin();
        let producer = origin.and_then(|origin| match origin.producer() {
            crate::callable::PreparedFunctionValueOriginProducer::PreparedContinuation(site) => {
                Some(site.expression())
            }
            crate::callable::PreparedFunctionValueOriginProducer::Call(_)
            | crate::callable::PreparedFunctionValueOriginProducer::Lexical { .. }
            | crate::callable::PreparedFunctionValueOriginProducer::IndependentExpression {
                ..
            } => None,
        });
        let argument_sources = record.input_projection().expression_sources();
        Ok(Some(EffectTraceSelectedCall {
            dispatch: if origin.is_some() {
                EffectTraceCallDispatch::Value
            } else {
                EffectTraceCallDispatch::Direct
            },
            producer,
            argument_sources,
            selected: application.selected().id(),
        }))
    }
}

struct CheckedEffectTraceCallAuthority<'a> {
    calls: &'a [CallTargetFacts],
    sites: BTreeMap<StableCheckedValueCoordinate, ExprId>,
}

impl<'a> CheckedEffectTraceCallAuthority<'a> {
    fn seal(calls: &'a [CallTargetFacts]) -> Result<Self, FinalSemanticAnalysisError> {
        let mut sites = BTreeMap::new();
        for call in calls {
            let Some(application) = call.selected_application() else {
                continue;
            };
            if sites
                .insert(application.core().stable_site().clone(), call.expression())
                .is_some()
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        }
        Ok(Self { calls, sites })
    }
}

impl EffectTraceCallAuthority for CheckedEffectTraceCallAuthority<'_> {
    fn selected_call(
        &self,
        owner: ExprId,
    ) -> Result<Option<EffectTraceSelectedCall<'_>>, FinalSemanticAnalysisError> {
        let mut matching = self.calls.iter().filter(|call| call.expression() == owner);
        let Some(call) = matching.next() else {
            return Ok(None);
        };
        if matching.next().is_some() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let Some(application) = call.selected_application() else {
            return Ok(None);
        };
        let selected = application.core().candidates().selected();
        let (dispatch, producer) = match (application.core().callee(), selected.state()) {
            (CheckedCallCalleeExecution::Direct, ResolvedCallableState::Base) => {
                (EffectTraceCallDispatch::Direct, None)
            }
            (CheckedCallCalleeExecution::Value { .. }, ResolvedCallableState::Base) => {
                let producer = match selected.base().authority().stable() {
                    crate::callable::ResolvedCallableStableIdentity::FunctionValue(identity) => {
                        self.sites
                            .get(&StableCheckedValueCoordinate::Expression(
                                identity.expression().clone(),
                            ))
                            .copied()
                    }
                    crate::callable::ResolvedCallableStableIdentity::Catalog(_)
                    | crate::callable::ResolvedCallableStableIdentity::Language(_)
                    | crate::callable::ResolvedCallableStableIdentity::Lexical(_) => None,
                };
                (EffectTraceCallDispatch::Value, producer)
            }
            (
                CheckedCallCalleeExecution::Value { .. },
                ResolvedCallableState::Continuation(continuation),
            ) => {
                let producer = self
                    .sites
                    .get(continuation.prefix_application_site())
                    .copied()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                (EffectTraceCallDispatch::Value, Some(producer))
            }
            (CheckedCallCalleeExecution::Direct, ResolvedCallableState::Continuation(_)) => {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        };
        let argument_sources = application
            .core()
            .execution()
            .arguments()
            .iter()
            .flat_map(|argument| argument.slots())
            .map(|slot| slot.source().owner())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(Some(EffectTraceSelectedCall {
            dispatch,
            producer,
            argument_sources,
            selected: selected.id(),
        }))
    }
}

fn callable_label(module: &HirModule, owner: ItemId) -> Result<String, FinalSemanticAnalysisError> {
    let item = module
        .resolve_item(owner)
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
    match item.kind() {
        HirItemKind::Flow(flow) => match flow.identity().public_id() {
            Some(arcweft_lang_hir::leaf::HirIdRef::Absolute(reference)) => {
                Ok(reference.as_str().to_owned())
            }
            _ => flow
                .identity()
                .name()
                .map(|name| name.as_str().to_owned())
                .ok_or(FinalSemanticAnalysisError::RecoveredOwner),
        },
        HirItemKind::Function(function) => function
            .name()
            .resolved()
            .map(|name| name.as_str().to_owned())
            .ok_or(FinalSemanticAnalysisError::RecoveredOwner),
        HirItemKind::Predicate(predicate) => predicate
            .name()
            .resolved()
            .map(|name| name.as_str().to_owned())
            .ok_or(FinalSemanticAnalysisError::RecoveredOwner),
        HirItemKind::Proof(proof) => proof
            .name()
            .resolved()
            .map(|name| name.as_str().to_owned())
            .ok_or(FinalSemanticAnalysisError::RecoveredOwner),
        _ => Err(FinalSemanticAnalysisError::InvalidCallableOwner),
    }
}

#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "effect trace construction joins the accepted module, scope, staged calls, symbols, modules, and missing effect without a side table"
)]
fn effect_trace_notes(
    module: &HirModule,
    owner: ItemId,
    input: &FinalSemanticAnalysisInput,
    call_authority: &impl EffectTraceCallAuthority,
    root_expressions: &std::collections::BTreeSet<ExprId>,
    prepared_effects: &crate::final_analysis::statement_effects::PreparedExecutionEffectCatalog,
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<super::HirModuleId, &HirModule>,
    missing: &EffectSet,
) -> Result<Vec<String>, FinalSemanticAnalysisError> {
    let callable = callable_label(module, owner)?;
    let mut notes = Vec::new();
    for effect in missing {
        notes.push(format!("effect trace for `{effect}`:"));
        let mut direct_perform = false;
        let trace = function_value_effect_trace(
            module,
            input,
            call_authority,
            root_expressions,
            symbols,
            modules,
            effect,
        )?;
        for (owner, checked) in &input.expressions {
            if !root_expressions.contains(owner) {
                continue;
            }
            let expression = module
                .resolve_expr(*owner)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            match expression.kind() {
                HirExprKind::Await(_) if checked.effects().contains(effect) => {
                    notes.push(format!("`{callable}` performs `{effect}` via await"));
                    direct_perform = true;
                }
                HirExprKind::Call(call) => {
                    let Some(label) = call_label(module, call) else {
                        continue;
                    };
                    if trace.function_value_calls.contains(owner) {
                        notes.push(format!("function value call `{label}`"));
                        direct_perform = true;
                    }
                    if trace.returned_calls.contains(owner) {
                        notes.push(format!("returned function value from `{label}`"));
                    }
                    let selected = call_authority.selected_call(*owner)?;
                    if checked.effects().contains(effect)
                        && selected
                            .is_some_and(|call| call.dispatch == EffectTraceCallDispatch::Direct)
                    {
                        notes.push(format!("call `{label}`"));
                        direct_perform = true;
                    }
                }
                _ => {}
            }
        }
        for callback in &trace.callback_closures {
            let callback_module = modules
                .get(&callback.module())
                .copied()
                .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
            let row = prepared_effects
                .closure(*callback)
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            for candidate in row.expressions() {
                let Some(checked) = input
                    .expressions
                    .iter()
                    .find_map(|(owner, checked)| (*owner == candidate).then_some(checked))
                else {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                        owner: candidate,
                    });
                };
                if !checked.effects().contains(effect) {
                    continue;
                }
                let expression = callback_module
                    .resolve_expr(candidate)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                let HirExprKind::Call(call) = expression.kind() else {
                    continue;
                };
                let Some(label) = call_label(callback_module, call) else {
                    continue;
                };
                notes.push(format!("call `{label}`"));
                direct_perform = true;
            }
        }
        for returned in &trace.returned_closures {
            let returned_module = modules
                .get(&returned.module())
                .copied()
                .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
            let row = prepared_effects
                .closure(*returned)
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            let closure_performs = row.expressions().any(|candidate| {
                input.expressions.iter().any(|(owner, checked)| {
                    *owner == candidate && checked.effects().contains(effect)
                })
            });
            if !closure_performs {
                continue;
            }
            let checked_closure = input
                .expressions
                .iter()
                .find_map(|(owner, checked)| {
                    (*owner == *returned).then(|| match checked.checked_resolution() {
                        Some(super::CheckedExpressionResolution::Closure(closure)) => Some(closure),
                        _ => None,
                    })?
                })
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            for capture in checked_closure.captures() {
                let local = returned_module
                    .resolve_local(capture.local())
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                notes.push(format!(
                    "higher-order argument `{}` captured by returned closure",
                    local.name().as_str()
                ));
            }
        }
        if !direct_perform && !notes.iter().any(|note| note == &format!("call `{effect}`")) {
            notes.push(format!("`{callable}` performs `{effect}`"));
        }
    }
    notes.dedup();
    Ok(notes)
}

#[derive(Default)]
struct FunctionValueEffectTrace {
    function_value_calls: std::collections::BTreeSet<ExprId>,
    returned_calls: std::collections::BTreeSet<ExprId>,
    callback_closures: std::collections::BTreeSet<ExprId>,
    returned_closures: std::collections::BTreeSet<ExprId>,
}

fn function_value_effect_trace(
    module: &HirModule,
    input: &FinalSemanticAnalysisInput,
    call_authority: &impl EffectTraceCallAuthority,
    root_expressions: &std::collections::BTreeSet<ExprId>,
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<super::HirModuleId, &HirModule>,
    effect: &super::EffectId,
) -> Result<FunctionValueEffectTrace, FinalSemanticAnalysisError> {
    let mut trace = FunctionValueEffectTrace::default();
    for (owner, checked) in &input.expressions {
        if !root_expressions.contains(owner) || !checked.effects().contains(effect) {
            continue;
        }
        let expression = module
            .resolve_expr(*owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirExprKind::Call(_) = expression.kind() else {
            continue;
        };
        let Some(call) = call_authority.selected_call(*owner)? else {
            continue;
        };
        if call.dispatch != EffectTraceCallDispatch::Value {
            continue;
        }
        trace.function_value_calls.insert(*owner);
        let Some(origin) = call.producer else {
            continue;
        };
        let origin_call = call_authority
            .selected_call(origin)?
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        trace.returned_calls.insert(origin);
        for argument in origin_call.argument_sources {
            let argument_module = modules
                .get(&argument.module())
                .copied()
                .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
            let argument_expression = argument_module
                .resolve_expr(argument)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            if matches!(argument_expression.kind(), HirExprKind::Closure(_)) {
                trace.callback_closures.insert(argument);
            }
        }
        let CallableCandidateId::Project(declaration) = origin_call.selected else {
            continue;
        };
        let Some(symbol) = symbols.callable(declaration) else {
            return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
        };
        let target_module = modules
            .get(&symbol.source_item().module())
            .copied()
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        let target = target_module
            .resolve_item(symbol.source_item())
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirItemKind::Function(function) = target.kind() else {
            continue;
        };
        let HirFunctionBody::Block { tail, .. } = function.body() else {
            continue;
        };
        if let Some(returned) = returned_closure_expression(target_module, *tail)? {
            trace.returned_closures.insert(returned);
        }
    }
    Ok(trace)
}

fn returned_closure_expression(
    module: &HirModule,
    mut owner: ExprId,
) -> Result<Option<ExprId>, FinalSemanticAnalysisError> {
    loop {
        let expression = module
            .resolve_expr(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        match expression.kind() {
            HirExprKind::Closure(_) => return Ok(Some(owner)),
            HirExprKind::Block(block) => owner = block.tail(),
            HirExprKind::Unit
            | HirExprKind::Literal(_)
            | HirExprKind::Path(_)
            | HirExprKind::EntityReference(_)
            | HirExprKind::LifetimePath(_)
            | HirExprKind::ShortVariant(_)
            | HirExprKind::Placeholder(_)
            | HirExprKind::Tuple(_)
            | HirExprKind::BracketSequence(_)
            | HirExprKind::NumericBracketSequence(_)
            | HirExprKind::ArrayRepeat(_)
            | HirExprKind::Call(_)
            | HirExprKind::Select(_)
            | HirExprKind::Index(_)
            | HirExprKind::Pipe(_)
            | HirExprKind::Try(_)
            | HirExprKind::Await(_)
            | HirExprKind::Thread(_)
            | HirExprKind::Choice(_)
            | HirExprKind::Range(_)
            | HirExprKind::Record(_)
            | HirExprKind::RecordLiteral(_)
            | HirExprKind::Binary(_)
            | HirExprKind::Borrow(_)
            | HirExprKind::Dereference(_)
            | HirExprKind::Unary(_)
            | HirExprKind::ComputationBlock(_)
            | HirExprKind::NamedBlock(_)
            | HirExprKind::Loop(_)
            | HirExprKind::If(_)
            | HirExprKind::IfLet(_)
            | HirExprKind::Match(_)
            | HirExprKind::DialogueContentApplication(_)
            | HirExprKind::PostfixBracket(_)
            | HirExprKind::Error(_)
            | HirExprKind::ForSynthetic(_) => return Ok(None),
        }
    }
}

fn call_label(module: &HirModule, call: &arcweft_lang_hir::expr::HirCallExpr) -> Option<String> {
    match call.callee() {
        HirCallCallee::Value { value } => expression_label(module, *value),
        HirCallCallee::UnresolvedDot {
            value_receiver,
            member,
            ..
        } => {
            let mut label = expression_label(module, *value_receiver)?;
            label.push('.');
            label.push_str(member.resolved()?.as_str());
            Some(label)
        }
        HirCallCallee::Associated { member, .. } => Some(member.resolved()?.as_str().to_owned()),
    }
}

fn expression_label(module: &HirModule, owner: ExprId) -> Option<String> {
    let expression = module.resolve_expr(owner).ok()?;
    match expression.kind() {
        HirExprKind::Path(path) => {
            let path = path.as_resolved()?;
            Some(
                path.segments()
                    .iter()
                    .map(|segment| match segment {
                        HirPathSegment::Identifier(name) => name.as_str(),
                        HirPathSegment::ProjectSymbol(name) => name.as_str(),
                    })
                    .collect::<Vec<_>>()
                    .join("."),
            )
        }
        HirExprKind::Select(select) => {
            let target = module.resolve_expr(select.target()).ok()?;
            let HirExprKind::Path(path) = target.kind() else {
                return None;
            };
            let mut label = path
                .as_resolved()?
                .segments()
                .iter()
                .map(|segment| match segment {
                    HirPathSegment::Identifier(name) => name.as_str(),
                    HirPathSegment::ProjectSymbol(name) => name.as_str(),
                })
                .collect::<Vec<_>>()
                .join(".");
            let arcweft_lang_hir::expr::HirSelectedMember::Name(member) = select.member() else {
                return None;
            };
            label.push('.');
            label.push_str(member.as_str());
            Some(label)
        }
        _ => None,
    }
}

impl Analyzer<'_, '_, '_> {
    pub(super) fn analyze_items(
        &self,
        input: &mut FinalSemanticAnalysisInput,
    ) -> Result<(), FinalSemanticAnalysisError> {
        for module in self.modules.values().copied() {
            for (owner, item) in module.items() {
                if item.is_poisoned() {
                    return Err(FinalSemanticAnalysisError::RecoveredOwner);
                }
                let role = item_role(module, owner, item, &self.types, self.facts.expressions())?;
                let effects = match item.kind() {
                    HirItemKind::Flow(flow) => flow
                        .contracts()
                        .iter()
                        .filter_map(|clause| clause.admitted_effect_operands())
                        .flatten()
                        .map(|owner| {
                            let checked = self
                                .facts
                                .expressions()
                                .get(owner)
                                .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
                            match checked.checked_resolution() {
                                Some(CheckedExpressionResolution::Effect(effect)) => {
                                    Ok(effect.clone())
                                }
                                _ => Err(FinalSemanticAnalysisError::WrongPayloadFamily),
                            }
                        })
                        .collect::<Result<EffectSet, _>>()?,
                    _ => EffectSet::new(),
                };
                input.push_item(owner, CheckedItem::new(effects, role));
            }
        }
        Ok(())
    }

    pub(super) fn stage_checked_callables(
        &self,
    ) -> Result<StagedCheckedCallables, FinalSemanticAnalysisError> {
        self.control.check()?;
        let accepted = Arc::clone(self.catalogs.world.environment().callable_catalog_arc());
        let mut builder = CheckedCallableCatalogBuilder::for_registered(
            Arc::clone(&accepted),
            Arc::clone(self.topology.generation()),
            STANDARD_TRAIT_CATALOG_VERSION,
        )
        .map_err(checked_catalog_error)?;
        let mut bodies = Vec::new();
        let mut effect_expressions = Vec::new();
        for module in self.modules.values().copied() {
            effect_expressions.extend(module_effect_expression_facts(module)?);
        }

        for record in builder
            .registered_records()
            .map_err(checked_catalog_error)?
        {
            self.control.check()?;
            let execution = fixed_record_execution(&record);
            let id = match record.schema().effects() {
                CallableEffectSchema::Fixed(_) => builder
                    .insert_fixed_shell(Arc::clone(&record), execution)
                    .map_err(checked_catalog_error)?,
                CallableEffectSchema::Project { declaration }
                    if matches!(
                        record.id(),
                        CallableCandidateId::Project(candidate) if candidate == declaration
                    ) =>
                {
                    let symbol = self
                        .symbols
                        .callable(declaration)
                        .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
                    let module = self.module(symbol.source_item().module())?;
                    if symbol.source_snapshot() != module.snapshot_id() {
                        return Err(FinalSemanticAnalysisError::CatalogGenerationMismatch);
                    }
                    match source_callable_shell(
                        module,
                        symbol,
                        &self.types,
                        self.facts.expressions(),
                    )? {
                        SourceCallableShell::Body {
                            scope,
                            execution,
                            contract,
                        } => {
                            let body_source = scope_span(module, scope)?;
                            let id = builder
                                .insert_body_shell(
                                    Arc::clone(&record),
                                    execution,
                                    *contract,
                                    &body_source,
                                )
                                .map_err(checked_catalog_error)?;
                            bodies.push(StagedCallableBody {
                                id: id.clone(),
                                module: module.module_id(),
                                item: symbol.source_item(),
                                owner: symbol.declaration().owner(),
                            });
                            id
                        }
                        SourceCallableShell::BodylessTraitRequirement { name } => {
                            let contract = CallableEffectContract::omitted_bodyless_trait(name);
                            builder
                                .insert_bodyless_trait_shell(
                                    Arc::clone(&record),
                                    CheckedCallableExecution::DispatchContract,
                                    contract,
                                )
                                .map_err(checked_catalog_error)?
                        }
                    }
                }
                CallableEffectSchema::Project { .. } | CallableEffectSchema::Detached { .. } => {
                    return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
                }
            };

            if let Some(method) = record.extension_method_name() {
                builder
                    .stage_extension_candidate(method, id)
                    .map_err(checked_catalog_error)?;
            } else if let Some(key) = record.receiver_method_key()
                && !matches!(record.access(), CallableAccess::TraitImplementation)
            {
                builder
                    .stage_method_candidate(&key, id)
                    .map_err(checked_catalog_error)?;
            }
        }

        builder.begin_inference().map_err(checked_catalog_error)?;
        Ok(StagedCheckedCallables {
            builder,
            bodies,
            effect_expressions,
            accepted,
        })
    }

    pub(super) fn finish_checked_callables(
        &self,
        mut staged: StagedCheckedCallables,
        input: &FinalSemanticAnalysisInput,
        selected: &crate::final_analysis::match_edges::CheckedSelectedExpressionGraph,
    ) -> Result<
        (
            Arc<CheckedCallableCatalog>,
            crate::final_analysis::statement_effects::PreparedExecutionEffectCatalog,
        ),
        FinalSemanticAnalysisError,
    > {
        let mut prepared_effects =
            crate::final_analysis::statement_effects::prepare_execution_effects(
                crate::final_analysis::statement_effects::PreparedExecutionEffectInput {
                    modules: &self.modules,
                    topology: self.topology.as_ref(),
                    selected,
                    expressions: &input.expressions,
                    statements: &input.statements,
                    control: self.control,
                },
            )?;
        let mut rows = BTreeMap::<CheckedCallableId, EffectSet>::new();
        for body in &staged.bodies {
            self.control.check()?;
            let CheckedCallableDeclaration::Project(declaration) = body.id.declaration() else {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            };
            rows.insert(
                body.id.clone(),
                prepared_effects
                    .declaration_effects(declaration)
                    .cloned()
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?,
            );
        }
        let graph = CallableEffectGraph::build(
            &staged.bodies,
            self.facts
                .prepared_calls()
                .map_err(FinalSemanticAnalysisError::from)?,
            &prepared_effects,
            self.control,
        )?;
        graph.reject_recursive_contracts(self.control)?;
        let mut bounded_call_effect_rows = BTreeMap::new();
        for body in &staged.bodies {
            let contract = staged
                .builder
                .pending_by_id(&body.id)
                .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?
                .body_contract()
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            if let EffectPermission::Bounded(row) = contract.permission()
                && bounded_call_effect_rows
                    .insert(body.id.clone(), row.concrete().clone())
                    .is_some()
            {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            }
        }
        graph.close_effect_rows(&mut rows, &bounded_call_effect_rows, self.control)?;

        // Body rows remain the authority for validating each callable's own
        // contract. Calls execute the callable's exposed interface instead:
        // an authored upper bound is observable even when this project build
        // can prove that the current body performs fewer effects. Closure
        // latent rows must therefore close project-call edges with exposed
        // rows, exactly like final call applications do.
        let call_effect_rows = staged
            .bodies
            .iter()
            .map(|body| {
                let inferred = rows
                    .get(&body.id)
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                let contract = staged
                    .builder
                    .pending_by_id(&body.id)
                    .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?
                    .body_contract()
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                let exposed = match contract.permission() {
                    EffectPermission::UnboundedInference => inferred,
                    EffectPermission::Bounded(row) => row.concrete(),
                };
                Ok((body.id.clone(), exposed.clone()))
            })
            .collect::<Result<BTreeMap<_, _>, FinalSemanticAnalysisError>>()?;

        for item in prepared_effects.item_owners().collect::<Vec<_>>() {
            let expressions = prepared_effects
                .item_expressions(item)
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?
                .collect::<Vec<_>>();
            let base = prepared_effects
                .item_effects(item)
                .cloned()
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            let closed = graph.close_selected_expression_effects(
                expressions,
                &base,
                &call_effect_rows,
                self.control,
            )?;
            prepared_effects.replace_item_effects(item, closed)?;
        }

        for body in &staged.bodies {
            self.control.check()?;
            let row = rows
                .get(&body.id)
                .cloned()
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            staged
                .builder
                .assign_inferred_row(&body.id, EffectRow::closed(row))
                .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            let module = self.module(body.module)?;
            let CheckedCallableDeclaration::Project(declaration) = body.id.declaration() else {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            };
            for closure in prepared_effects.closures(declaration) {
                let id = super::CheckedClosureId::from_checked_expression(
                    body.id.clone(),
                    super::statements::expression_span(module, closure.owner())?,
                )
                .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                let row = EffectRow::closed(graph.close_selected_expression_effects(
                    closure.expressions(),
                    closure.effects(),
                    &call_effect_rows,
                    self.control,
                )?);
                staged
                    .builder
                    .insert_closure_row(id, row)
                    .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            }
        }
        staged
            .builder
            .begin_validation()
            .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        for body in &staged.bodies {
            self.control.check()?;
            let contract = staged
                .builder
                .pending_by_id(&body.id)
                .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?
                .body_contract()
                .cloned()
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            match staged.builder.validate_body_contract(&body.id) {
                Ok(()) => {}
                Err(CheckedCallableCatalogBuildError::EffectSubset(error)) => {
                    let EffectSubsetError::MissingEffects { missing } = *error else {
                        return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
                    };
                    let module = self.module(body.module)?;
                    let call_authority = PreparedEffectTraceCallAuthority::new(
                        self.facts
                            .prepared_calls()
                            .map_err(FinalSemanticAnalysisError::from)?,
                    );
                    let CheckedCallableDeclaration::Project(declaration) = body.id.declaration()
                    else {
                        return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
                    };
                    let root_expressions = prepared_effects
                        .declaration_expressions(declaration)
                        .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?
                        .collect::<std::collections::BTreeSet<_>>();
                    return Err(FinalSemanticAnalysisError::EffectUpperBoundExceeded {
                        owner: body.item,
                        callable: callable_label(module, body.item)?,
                        trace_notes: effect_trace_notes(
                            module,
                            body.item,
                            input,
                            &call_authority,
                            &root_expressions,
                            &prepared_effects,
                            self.symbols,
                            &self.modules,
                            &missing,
                        )?
                        .into_boxed_slice(),
                        missing,
                        contract_source: contract.source().anchor().clone(),
                    });
                }
                Err(_) => return Err(FinalSemanticAnalysisError::CheckedCallableCatalog),
            }
        }
        let checked = staged
            .builder
            .finish()
            .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        checked
            .validate_registered_authority(
                staged.accepted.as_ref(),
                self.topology.generation().as_ref(),
            )
            .map_err(|_| FinalSemanticAnalysisError::CatalogGenerationMismatch)?;
        Ok((checked, prepared_effects))
    }

    /// Validates authored Flow effect upper bounds after call facts have been
    /// finalized. Flows are structural execution owners rather than ordinary
    /// callable symbols, so they deliberately do not enter the checked
    /// callable catalog. Their bodies nevertheless consume the same final
    /// expression effects and typed effect identities as ordinary functions.
    pub(super) fn validate_flow_effect_bounds(
        &self,
        input: &FinalSemanticAnalysisInput,
        prepared_effects: &crate::final_analysis::statement_effects::PreparedExecutionEffectCatalog,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let call_authority = CheckedEffectTraceCallAuthority::seal(&input.calls)?;
        for module in self.modules.values().copied() {
            for (owner, item) in module.items() {
                self.control.check()?;
                let HirItemKind::Flow(flow) = item.kind() else {
                    continue;
                };
                let Some(contract_source) = flow
                    .contracts()
                    .iter()
                    .enumerate()
                    .find_map(|(ordinal, clause)| {
                        matches!(clause, HirFlowContractClause::Effects(_)).then_some(ordinal)
                    })
                    .map(|ordinal| {
                        let ordinal = u16::try_from(ordinal)
                            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
                        source_span(
                            module,
                            HirSourceQuery::Item {
                                owner,
                                role: HirItemSourceRole::Flow(HirFlowSourceRole::ContractClause {
                                    ordinal,
                                    part: HirFlowContractSourcePart::Whole,
                                }),
                            },
                        )
                    })
                    .transpose()?
                else {
                    // An omitted Flow effects clause means body inference,
                    // exactly like an omitted ordinary-function clause.
                    continue;
                };
                let mut permitted = input
                    .items
                    .iter()
                    .find_map(|(candidate, checked)| {
                        (*candidate == owner).then_some(checked.effects())
                    })
                    .ok_or(FinalSemanticAnalysisError::MissingFact {
                        family: super::SemanticFactFamily::Item,
                    })?
                    .clone();
                permitted.insert(EffectId::control_suspend());
                let actual = prepared_effects
                    .item_effects(owner)
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                let missing = actual.effects_not_covered_by(&permitted);
                if missing.is_empty() {
                    continue;
                }
                let root_expressions = prepared_effects
                    .item_expressions(owner)
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?
                    .collect::<std::collections::BTreeSet<_>>();
                return Err(FinalSemanticAnalysisError::EffectUpperBoundExceeded {
                    owner,
                    callable: callable_label(module, owner)?,
                    trace_notes: effect_trace_notes(
                        module,
                        owner,
                        input,
                        &call_authority,
                        &root_expressions,
                        prepared_effects,
                        self.symbols,
                        &self.modules,
                        &missing,
                    )?
                    .into_boxed_slice(),
                    missing,
                    contract_source,
                });
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn freeze_checked_callables(
        &self,
        input: &FinalSemanticAnalysisInput,
        selected: &crate::final_analysis::match_edges::CheckedSelectedExpressionGraph,
    ) -> Result<Arc<CheckedCallableCatalog>, FinalSemanticAnalysisError> {
        self.finish_checked_callables(self.stage_checked_callables()?, input, selected)
            .map(|(catalog, _)| catalog)
    }
}

pub(super) enum SourceCallableShell {
    Body {
        scope: ScopeId,
        execution: CheckedCallableExecution,
        contract: Box<CallableEffectContract>,
    },
    BodylessTraitRequirement {
        name: SourceSpan,
    },
}

pub(super) fn checked_catalog_error<E>(_: E) -> FinalSemanticAnalysisError {
    FinalSemanticAnalysisError::CheckedCallableCatalog
}

fn fixed_record_execution(record: &crate::callable::CallableRecord) -> CheckedCallableExecution {
    if record
        .method_role()
        .is_some_and(CallableMethodRole::is_dispatch_contract)
    {
        CheckedCallableExecution::DispatchContract
    } else {
        CheckedCallableExecution::Runtime(CheckedFunctionExecution::DirectFrame)
    }
}

fn module_effect_expression_facts(
    module: &HirModule,
) -> Result<Vec<(ExprId, CheckedExpression)>, FinalSemanticAnalysisError> {
    let mut facts = Vec::new();
    for (_, item) in module.items() {
        for effect in item.kind().effect_expression_roots() {
            facts.extend(checked_effect_expression(module, effect)?.1);
        }
    }
    Ok(facts)
}

fn source_callable_shell(
    module: &HirModule,
    symbol: &arcweft_lang_hir::symbol::CallableSymbol,
    types: &BTreeMap<TypeId, TypeKind>,
    expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
) -> Result<SourceCallableShell, FinalSemanticAnalysisError> {
    let item = module
        .resolve_item(symbol.source_item())
        .map_err(|_| FinalSemanticAnalysisError::InvalidCallableOwner)?;
    let inferred_body = |scope, execution| {
        let anchor = scope_span(module, scope)?;
        let contract =
            CallableEffectContract::body_inference(anchor, EffectSet::new(), Box::new([]))
                .map_err(checked_catalog_error)?;
        Ok(SourceCallableShell::Body {
            scope,
            execution,
            contract: Box::new(contract),
        })
    };
    if symbol.owner() == arcweft_lang_hir::symbol::CallableDeclarationOwner::View {
        return view_source_callable_shell(module, symbol, item);
    }
    match (symbol.source_owner(), item.kind()) {
        (HirCallableSourceOwner::Item, HirItemKind::Flow(flow)) => inferred_body(
            flow.body_scope(),
            CheckedCallableExecution::Runtime(CheckedFunctionExecution::DirectFrame),
        ),
        (HirCallableSourceOwner::Item, HirItemKind::Function(function)) => {
            let CheckedItemRole::Function { execution, .. } =
                item_role(module, symbol.source_item(), item, types, expressions)?
            else {
                return Err(FinalSemanticAnalysisError::InvalidFunctionExecution {
                    owner: symbol.source_item(),
                });
            };
            match function.body() {
                HirFunctionBody::Block { scope, .. } => function_effect_contract(
                    module,
                    symbol.source_item(),
                    function,
                    *scope,
                    CheckedCallableExecution::Runtime(execution),
                ),
                HirFunctionBody::Error(_) => Err(FinalSemanticAnalysisError::RecoveredOwner),
            }
        }
        (HirCallableSourceOwner::Item, HirItemKind::Predicate(predicate)) => {
            match predicate.body() {
                HirPredicateBody::Expression { scope, .. }
                | HirPredicateBody::Block { scope, .. } => inferred_body(
                    *scope,
                    CheckedCallableExecution::Runtime(CheckedFunctionExecution::DirectFrame),
                ),
                HirPredicateBody::Error { .. } => Err(FinalSemanticAnalysisError::RecoveredOwner),
            }
        }
        (HirCallableSourceOwner::Item, HirItemKind::Proof(proof)) => match proof.body() {
            HirProofBody::Expression { scope, .. } | HirProofBody::Block { scope, .. } => {
                inferred_body(
                    *scope,
                    CheckedCallableExecution::Runtime(CheckedFunctionExecution::DirectFrame),
                )
            }
            HirProofBody::Error { .. } => Err(FinalSemanticAnalysisError::RecoveredOwner),
        },
        (HirCallableSourceOwner::TraitFunction { member }, HirItemKind::Trait(trait_item)) => {
            let Some(HirTraitMember::Function(function)) =
                trait_item.members().get(usize::from(member))
            else {
                return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
            };
            if function.body().is_some() {
                return Err(FinalSemanticAnalysisError::UnsupportedCallableBody {
                    owner: symbol.source_item(),
                });
            }
            Ok(SourceCallableShell::BodylessTraitRequirement {
                name: symbol.name_span().clone(),
            })
        }
        (HirCallableSourceOwner::ImplFunction { member }, HirItemKind::Impl(impl_item)) => {
            let Some(HirImplMember::Function(function)) =
                impl_item.members().get(usize::from(member))
            else {
                return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
            };
            let Some(HirFunctionBody::Block { scope, .. }) = function.body() else {
                return Err(FinalSemanticAnalysisError::UnsupportedCallableBody {
                    owner: symbol.source_item(),
                });
            };
            let (yield_count, _) =
                function_body_roles(module, function.body().expect("checked above"), expressions)?;
            let execution = checked_function_execution(
                symbol.source_item(),
                function.return_type(),
                types,
                yield_count,
            )?;
            inferred_body(*scope, CheckedCallableExecution::Runtime(execution))
        }
        _ => Err(FinalSemanticAnalysisError::InvalidCallableOwner),
    }
}

fn view_source_callable_shell(
    module: &HirModule,
    symbol: &arcweft_lang_hir::symbol::CallableSymbol,
    item: &HirItem,
) -> Result<SourceCallableShell, FinalSemanticAnalysisError> {
    let (HirCallableSourceOwner::ViewItem, HirItemKind::View(view)) =
        (symbol.source_owner(), item.kind())
    else {
        return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
    };
    let anchor = scope_span(module, view.callable_scope())?;
    let contract = CallableEffectContract::body_inference(anchor, EffectSet::new(), Box::new([]))
        .map_err(checked_catalog_error)?;
    Ok(SourceCallableShell::Body {
        scope: view.callable_scope(),
        execution: CheckedCallableExecution::Runtime(CheckedFunctionExecution::DirectFrame),
        contract: Box::new(contract),
    })
}

fn item_role(
    module: &HirModule,
    owner: ItemId,
    item: &HirItem,
    types: &BTreeMap<TypeId, TypeKind>,
    expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
) -> Result<CheckedItemRole, FinalSemanticAnalysisError> {
    Ok(match item.kind() {
        HirItemKind::Module(_) => CheckedItemRole::Module,
        HirItemKind::Use(_) => CheckedItemRole::Use,
        HirItemKind::Flow(flow) => CheckedItemRole::Flow {
            identity: flow.identity().clone(),
        },
        HirItemKind::Function(function) => {
            let (yield_count, suspension) =
                function_body_roles(module, function.body(), expressions)?;
            let execution =
                checked_function_execution(owner, function.return_type(), types, yield_count)?;
            CheckedItemRole::Function {
                execution,
                suspension: if suspension {
                    CheckedSuspensionRole::MaySuspend
                } else {
                    CheckedSuspensionRole::NonSuspending
                },
            }
        }
        HirItemKind::Predicate(_) => CheckedItemRole::Predicate,
        HirItemKind::Proof(_) => CheckedItemRole::Proof,
        HirItemKind::Trait(_) => CheckedItemRole::Trait,
        HirItemKind::Impl(_) => CheckedItemRole::Impl,
        HirItemKind::Enum(_) => CheckedItemRole::Enum,
        HirItemKind::Struct(_) => CheckedItemRole::Struct,
        HirItemKind::TypeAlias(_) => CheckedItemRole::TypeAlias,
        HirItemKind::Resource(_) => CheckedItemRole::Resource,
        HirItemKind::Character(_) => CheckedItemRole::Character,
        HirItemKind::View(_) => CheckedItemRole::View,
        HirItemKind::Action(_) => CheckedItemRole::Action,
        HirItemKind::Activity(_) => CheckedItemRole::Activity,
        HirItemKind::Signal(_) => CheckedItemRole::Signal,
        HirItemKind::Metric(_) => CheckedItemRole::Metric,
        HirItemKind::Layer(_) => CheckedItemRole::Layer,
        HirItemKind::Entry(_) => CheckedItemRole::Entry,
        HirItemKind::ExternCapability(_) => CheckedItemRole::ExternCapability,
        HirItemKind::Test(_) => CheckedItemRole::Test,
        HirItemKind::Bench(_) => CheckedItemRole::Bench,
        HirItemKind::Style(_) => CheckedItemRole::Style,
        HirItemKind::Error(_) => return Err(FinalSemanticAnalysisError::RecoveredOwner),
    })
}

pub(super) fn function_body_roles(
    module: &HirModule,
    body: &HirFunctionBody,
    expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
) -> Result<(u32, bool), FinalSemanticAnalysisError> {
    let body = body
        .try_body_projection()
        .map_err(|_| FinalSemanticAnalysisError::RecoveredOwner)?;
    let mut fold = FunctionBodyRoleFold {
        module,
        expressions,
        statements: std::collections::BTreeSet::new(),
        visited_expressions: std::collections::BTreeSet::new(),
        yields: 0,
        suspension: false,
    };
    fold.fold_body(&body)?;
    Ok((fold.yields, fold.suspension))
}

/// Typed top-level execution traversal used only for function execution-role
/// classification. It follows the same eager/latent boundaries as the final
/// expression/statement effect fold and never infers membership from scopes.
struct FunctionBodyRoleFold<'a> {
    module: &'a HirModule,
    expressions: &'a BTreeMap<ExprId, super::PreparedExpressionFact>,
    statements: std::collections::BTreeSet<super::StmtId>,
    visited_expressions: std::collections::BTreeSet<ExprId>,
    yields: u32,
    suspension: bool,
}

impl FunctionBodyRoleFold<'_> {
    fn fold_body(&mut self, body: &HirBodyProjection) -> Result<(), FinalSemanticAnalysisError> {
        for edge in body.children() {
            match edge.child() {
                HirBodyChild::Expression(owner) => self.fold_expression(owner)?,
                HirBodyChild::Statement(owner) => self.fold_statement(owner)?,
            }
        }
        Ok(())
    }

    fn fold_statement(&mut self, owner: super::StmtId) -> Result<(), FinalSemanticAnalysisError> {
        if !self.statements.insert(owner) {
            return Ok(());
        }
        let statement = self
            .module
            .resolve_stmt(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if statement.is_poisoned() {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        }
        let kind = statement.kind();
        match kind {
            HirStmtKind::Yield { .. } => {
                self.yields = self
                    .yields
                    .checked_add(1)
                    .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
                self.suspension = true;
            }
            HirStmtKind::Wait { .. } => self.suspension = true,
            HirStmtKind::Assertion { .. }
            | HirStmtKind::Let { .. }
            | HirStmtKind::Assign { .. }
            | HirStmtKind::LetElse { .. }
            | HirStmtKind::Return { .. }
            | HirStmtKind::Out { .. }
            | HirStmtKind::Goto { .. }
            | HirStmtKind::Defer { .. }
            | HirStmtKind::Signal { .. }
            | HirStmtKind::LifetimeSet { .. }
            | HirStmtKind::On { .. }
            | HirStmtKind::UnsafeLifetime { .. }
            | HirStmtKind::Choice { .. }
            | HirStmtKind::If(_)
            | HirStmtKind::IfLet(_)
            | HirStmtKind::Match(_)
            | HirStmtKind::While(_)
            | HirStmtKind::WhileLet(_)
            | HirStmtKind::For(_)
            | HirStmtKind::Close { .. }
            | HirStmtKind::Select(_)
            | HirStmtKind::SourceLocale(_)
            | HirStmtKind::Scope(_)
            | HirStmtKind::Include(_)
            | HirStmtKind::Break { .. }
            | HirStmtKind::Continue { .. }
            | HirStmtKind::Expression { .. }
            | HirStmtKind::ProofCall { .. } => {}
            HirStmtKind::Error => return Err(FinalSemanticAnalysisError::RecoveredOwner),
        }
        for edge in kind
            .try_child_edges()
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
        {
            match edge.child() {
                HirStatementChild::Expression(owner) => self.fold_expression(owner)?,
                HirStatementChild::Statement(owner)
                    if !matches!(edge.role(), HirStatementChildRole::BodyItem { .. }) =>
                {
                    self.fold_statement(owner)?;
                }
                HirStatementChild::Statement(_)
                | HirStatementChild::Pattern(_)
                | HirStatementChild::Type(_)
                | HirStatementChild::Local(_) => {}
            }
        }
        for body in kind
            .body_projections()
            .map_err(|_| FinalSemanticAnalysisError::RecoveredOwner)?
        {
            if body.role() != &HirStatementBodyRole::On {
                self.fold_body(body.projection())?;
            }
        }
        Ok(())
    }

    fn fold_expression(&mut self, owner: ExprId) -> Result<(), FinalSemanticAnalysisError> {
        if !self.visited_expressions.insert(owner) {
            return Ok(());
        }
        let expression = self
            .module
            .resolve_expr(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if expression.is_poisoned() {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        }
        let kind = expression.kind();
        if matches!(kind, HirExprKind::Await(_)) {
            self.suspension = true;
        }
        let latent_callable = matches!(kind, HirExprKind::Closure(_))
            || matches!(
                self.expressions
                    .get(&owner)
                    .and_then(super::PreparedExpressionFact::checked_resolution),
                Some(super::CheckedExpressionResolution::ImplicitCallable(_))
            );
        let independent_computation = matches!(
            kind,
            HirExprKind::ComputationBlock(expression)
                if matches!(
                    expression.kind(),
                    HirComputationBlockKind::Seq | HirComputationBlockKind::Stream
                )
        );
        for edge in kind
            .try_child_edges()
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
        {
            if edge.ownership() != HirExpressionChildOwnership::Owning
                || matches!(edge.role(), HirExpressionChildRole::ClosureBody)
                || latent_callable
                || independent_computation
            {
                continue;
            }
            self.fold_expression(edge.child())?;
        }
        if let Some(body) = kind
            .try_body_projection()
            .map_err(|_| FinalSemanticAnalysisError::RecoveredOwner)?
            && !matches!(kind, HirExprKind::Thread(_))
            && !latent_callable
            && !independent_computation
        {
            self.fold_body(&body)?;
        }
        if matches!(kind, HirExprKind::Await(_)) {
            for edge in kind
                .expression_owned_child_edges()
                .map_err(|_| FinalSemanticAnalysisError::RecoveredOwner)?
            {
                match edge.child() {
                    HirExpressionOwnedChild::Pattern(_) => {}
                    HirExpressionOwnedChild::Statement(owner) => self.fold_statement(owner)?,
                    HirExpressionOwnedChild::Body(edge) => match edge.child() {
                        HirBodyChild::Expression(owner) => self.fold_expression(owner)?,
                        HirBodyChild::Statement(owner) => self.fold_statement(owner)?,
                    },
                }
            }
        }
        Ok(())
    }
}

fn checked_function_execution(
    owner: ItemId,
    return_type: Option<TypeId>,
    types: &BTreeMap<TypeId, TypeKind>,
    yield_count: u32,
) -> Result<CheckedFunctionExecution, FinalSemanticAnalysisError> {
    if yield_count == 0 {
        return Ok(CheckedFunctionExecution::DirectFrame);
    }
    let return_type = return_type
        .and_then(|id| types.get(&id))
        .ok_or(FinalSemanticAnalysisError::InvalidFunctionExecution { owner })?;
    let TypeKind::Stream { item, error } = return_type else {
        return Err(FinalSemanticAnalysisError::InvalidFunctionExecution { owner });
    };
    Ok(CheckedFunctionExecution::StreamFactory {
        item: (**item).clone(),
        error: (**error).clone(),
        own_scope_yields: yield_count,
    })
}
