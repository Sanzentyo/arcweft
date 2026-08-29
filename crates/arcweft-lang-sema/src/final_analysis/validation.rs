//! Validation and work collection for one staged semantic generation.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_lang_hir::{
    expr::{HirCallCallee, HirPlaceholderKind},
    project::HirProjectEvaluationTopology,
    source_index::{
        HirExprSourceRole, HirLocalSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite,
        HirTypeSourceRole,
    },
};

use crate::{
    callable::{CallableCandidateId, CallableValidator, DialogueCallableId},
    types::{
        NoopTypeCompatibilityControl, TypeCompatibilityFailure, TypeCompatibilityForbidden,
        TypeCompatibilityPolicy,
    },
};

use super::type_rules::compact_numeric_element_type;
use super::{
    CallAnalysisOutcome, CallCalleeClassificationFact, CallTargetFacts, CallableDeclarationOwner,
    CallableDiagnosticSubject, CaptureId, CheckedBinding, CheckedBindingRole,
    CheckedCallArgumentSlotSource, CheckedCallCalleeExecution, CheckedCallResult,
    CheckedCharacterDialoguePatch, CheckedCharacterDialogueTarget, CheckedChoice,
    CheckedEntryReference, CheckedExpression, CheckedExpressionResolution,
    CheckedFunctionExecution, CheckedImplicitCallable, CheckedItem, CheckedItemRole,
    CheckedIteration, CheckedPatchOperation, CheckedPattern, CheckedPatternResolution, CheckedPipe,
    CheckedProjectCallable, CheckedProjectItem, CheckedProjectItemOwner, CheckedProjectNominal,
    CheckedSelectResolution, CheckedStatement, CheckedStatementPayload, CheckedTraitConformance,
    CheckedTryBoundary, CheckedTryCarrier, CheckedValueResolution, CheckedVariantOwner,
    CheckedVariantResolution, DeclarationIdentityFamily, ExprId, FinalSemanticAnalysisError,
    FinalSemanticAnalysisWork, HirExprKind, HirIdRef, HirItemKind, HirModule, HirModuleId,
    HirPatternKind, ItemId, LocalId, PatternId, PhysicalCandidateArgumentEvaluation,
    PostfixBracketResolution, ProjectNominalBody, ProjectSymbolTable, ResolvedCallable,
    ResolvedCallableOrigin, SemanticFactFamily, StmtId, TypeId, TypeKind, TypeResolutionReport,
};

/// Borrowed semantic fact maps validated and accounted as one generation.
#[derive(Clone, Copy)]
pub(super) struct SemanticFactInventory<'a> {
    pub(super) types: &'a BTreeMap<TypeId, TypeKind>,
    pub(super) locals: &'a BTreeMap<LocalId, CheckedBinding>,
    pub(super) captures: &'a BTreeMap<CaptureId, CheckedBinding>,
    pub(super) expressions: &'a BTreeMap<ExprId, CheckedExpression>,
    pub(super) patterns: &'a BTreeMap<PatternId, CheckedPattern>,
    pub(super) statements: &'a BTreeMap<StmtId, CheckedStatement>,
    pub(super) items: &'a BTreeMap<ItemId, CheckedItem>,
    pub(super) calls: &'a BTreeMap<ExprId, CallTargetFacts>,
}

pub(super) fn collect_work(
    inventory: SemanticFactInventory<'_>,
) -> Result<FinalSemanticAnalysisWork, FinalSemanticAnalysisError> {
    let SemanticFactInventory {
        types,
        locals,
        captures,
        expressions,
        patterns,
        statements,
        items,
        calls,
    } = inventory;
    let mut work = FinalSemanticAnalysisWork {
        type_facts: fact_count(types.len())?,
        local_facts: fact_count(locals.len())?,
        capture_facts: fact_count(captures.len())?,
        expression_facts: fact_count(expressions.len())?,
        pattern_facts: fact_count(patterns.len())?,
        statement_facts: fact_count(statements.len())?,
        item_facts: fact_count(items.len())?,
        call_facts: fact_count(calls.len())?,
        ..FinalSemanticAnalysisWork::default()
    };
    for call in calls.values() {
        let accounting = call.accounting();
        checked_add(
            &mut work.call_diagnostics,
            fact_count(call.diagnostics().len())?,
        )?;
        checked_add(
            &mut work.logical_argument_checks,
            accounting.logical_argument_checks(),
        )?;
        checked_add(
            &mut work.resolver_invocations,
            accounting.resolver_invocations(),
        )?;
        checked_add(
            &mut work.candidate_argument_probes,
            accounting.candidate_argument_probes(),
        )?;
        checked_add(
            &mut work.selected_replay_argument_visits,
            accounting.selected_replay_argument_visits(),
        )?;
        checked_add(
            &mut work.retained_argument_fact_publications,
            accounting.retained_argument_fact_publications(),
        )?;
    }
    Ok(work)
}

fn fact_count(count: usize) -> Result<u64, FinalSemanticAnalysisError> {
    u64::try_from(count).map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)
}

fn checked_add(target: &mut u64, amount: u64) -> Result<(), FinalSemanticAnalysisError> {
    *target = target
        .checked_add(amount)
        .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
    Ok(())
}

pub(super) fn collect_unique<K: Ord, V>(
    values: impl IntoIterator<Item = (K, V)>,
    family: SemanticFactFamily,
) -> Result<BTreeMap<K, V>, FinalSemanticAnalysisError> {
    let mut result = BTreeMap::new();
    for (key, value) in values {
        if result.insert(key, value).is_some() {
            return Err(FinalSemanticAnalysisError::DuplicateFact { family });
        }
    }
    Ok(result)
}

/// Computes the final semantic type inventory after each dot callee has one
/// accepted value-first/nominal-second classification.
///
/// `HirCallCallee::UnresolvedDot` deliberately retains both source-backed
/// candidates until semantic selection. A value selection does not turn the
/// alternative nominal receiver into a runtime type, while an associated-type
/// selection must retain complete nominal-resolution evidence for that exact
/// receiver. Calls in an unselected expression candidate have no semantic fact
/// and likewise cannot admit their alternative type subtree.
pub(super) fn accepted_type_owners(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    calls: &BTreeMap<ExprId, CallTargetFacts>,
) -> Result<BTreeSet<TypeId>, FinalSemanticAnalysisError> {
    let mut accepted = modules
        .values()
        .flat_map(|module| module.types().map(|(owner, _)| owner))
        .collect::<BTreeSet<_>>();

    // An Impl trait reference is a conformance identity, not a runtime value
    // type. Its final semantic consumer selects a typed trait identity from
    // the retained HIR path; publishing the same TypeId as a flattened
    // TypeKind would create a second, incorrect nominal-type authority.
    for owner in implementation_trait_reference_roots(modules) {
        accepted.remove(&owner);
    }

    for module in modules.values() {
        for (owner, expression) in module.expressions() {
            let HirExprKind::Call(call) = expression.kind() else {
                continue;
            };
            let HirCallCallee::UnresolvedDot {
                value_receiver,
                nominal_receiver,
                ..
            } = call.callee()
            else {
                continue;
            };
            let Some(nominal_receiver) = nominal_receiver.type_id() else {
                continue;
            };

            let Some(facts) = calls.get(&owner) else {
                if !expressions.contains_key(&owner)
                    || expressions.get(&owner).is_some_and(|expression| {
                        matches!(
                            expression.resolution(),
                            CheckedExpressionResolution::Effect(_)
                        )
                    })
                {
                    remove_type_subtree(module, nominal_receiver, &mut accepted)?;
                    continue;
                }
                return Err(FinalSemanticAnalysisError::CallFactMismatch);
            };
            match facts.outcome() {
                CallAnalysisOutcome::Selected(application) => {
                    if application.core().direct_type_receiver().is_none() {
                        remove_type_subtree(module, nominal_receiver, &mut accepted)?;
                    }
                }
                CallAnalysisOutcome::Ambiguous(evidence) => match evidence.callee() {
                    Some(CallCalleeClassificationFact::Value { expression })
                        if expression == *value_receiver =>
                    {
                        remove_type_subtree(module, nominal_receiver, &mut accepted)?;
                    }
                    Some(CallCalleeClassificationFact::AssociatedType { receiver, .. })
                        if receiver == nominal_receiver => {}
                    None if !expressions.contains_key(&owner)
                        || expressions.get(&owner).is_some_and(|expression| {
                            matches!(
                                expression.resolution(),
                                CheckedExpressionResolution::Effect(_)
                            )
                        }) =>
                    {
                        remove_type_subtree(module, nominal_receiver, &mut accepted)?;
                    }
                    Some(_) | None => return Err(FinalSemanticAnalysisError::CallFactMismatch),
                },
                CallAnalysisOutcome::Rejected(evidence) => match evidence.callee() {
                    Some(CallCalleeClassificationFact::Value { expression })
                        if expression == *value_receiver =>
                    {
                        remove_type_subtree(module, nominal_receiver, &mut accepted)?;
                    }
                    Some(CallCalleeClassificationFact::AssociatedType { receiver, .. })
                        if receiver == nominal_receiver => {}
                    None if !expressions.contains_key(&owner)
                        || expressions.get(&owner).is_some_and(|expression| {
                            matches!(
                                expression.resolution(),
                                CheckedExpressionResolution::Effect(_)
                            )
                        }) =>
                    {
                        remove_type_subtree(module, nominal_receiver, &mut accepted)?;
                    }
                    Some(_) | None => return Err(FinalSemanticAnalysisError::CallFactMismatch),
                },
                CallAnalysisOutcome::NonCallable(evidence) => match evidence.callee() {
                    Some(CallCalleeClassificationFact::Value { expression })
                        if expression == *value_receiver =>
                    {
                        remove_type_subtree(module, nominal_receiver, &mut accepted)?;
                    }
                    Some(CallCalleeClassificationFact::AssociatedType { receiver, .. })
                        if receiver == nominal_receiver => {}
                    Some(_) | None => return Err(FinalSemanticAnalysisError::CallFactMismatch),
                },
                CallAnalysisOutcome::Missing(evidence) => match evidence.callee() {
                    Some(CallCalleeClassificationFact::Value { expression })
                        if expression == *value_receiver =>
                    {
                        remove_type_subtree(module, nominal_receiver, &mut accepted)?;
                    }
                    Some(CallCalleeClassificationFact::AssociatedType { receiver, .. })
                        if receiver == nominal_receiver => {}
                    Some(_) | None => return Err(FinalSemanticAnalysisError::CallFactMismatch),
                },
            }
        }
    }

    Ok(accepted)
}

pub(super) fn implementation_trait_reference_roots(
    modules: &BTreeMap<HirModuleId, &HirModule>,
) -> BTreeSet<TypeId> {
    modules
        .values()
        .flat_map(|module| module.items())
        .filter_map(|(_, item)| match item.kind() {
            HirItemKind::Impl(implementation) => implementation.trait_ref(),
            _ => None,
        })
        .collect()
}

fn remove_type_subtree(
    module: &HirModule,
    root: TypeId,
    accepted: &mut BTreeSet<TypeId>,
) -> Result<(), FinalSemanticAnalysisError> {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(owner) = pending.pop() {
        if !visited.insert(owner) {
            continue;
        }
        let ty = module
            .resolve_type(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        accepted.remove(&owner);
        pending.extend(ty.kind().direct_type_children());
    }
    Ok(())
}

pub(super) fn validate_complete_inventory(
    topology: &HirProjectEvaluationTopology,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    selected_expressions: &super::match_edges::CheckedSelectedExpressionGraph,
    inventory: SemanticFactInventory<'_>,
    type_resolutions: &BTreeMap<TypeId, TypeResolutionReport>,
) -> Result<(), FinalSemanticAnalysisError> {
    let SemanticFactInventory {
        types,
        locals,
        captures,
        expressions,
        patterns,
        statements,
        items,
        calls: _,
    } = inventory;
    if type_resolutions.is_empty() {
        require_complete(
            modules
                .values()
                .flat_map(|module| module.types().map(|(id, _)| id)),
            types,
            SemanticFactFamily::Type,
        )?;
    }
    require_complete(
        modules
            .values()
            .flat_map(|module| module.locals().map(|(id, _)| id)),
        locals,
        SemanticFactFamily::Local,
    )?;
    require_complete(
        topology
            .modules()
            .iter()
            .flat_map(|module| module.captures().rows().map(|capture| capture.capture())),
        captures,
        SemanticFactFamily::Capture,
    )?;
    let selected_expression_owners = selected_expressions.owners().collect::<BTreeSet<_>>();
    require_complete_expressions(selected_expression_owners.into_iter(), expressions)?;
    require_complete(
        modules
            .values()
            .flat_map(|module| module.patterns().map(|(id, _)| id)),
        patterns,
        SemanticFactFamily::Pattern,
    )?;
    require_complete(
        modules
            .values()
            .flat_map(|module| module.statements().map(|(id, _)| id)),
        statements,
        SemanticFactFamily::Statement,
    )?;
    require_complete(
        modules
            .values()
            .flat_map(|module| module.items().map(|(id, _)| id)),
        items,
        SemanticFactFamily::Item,
    )?;
    Ok(())
}

fn require_complete_expressions(
    expected: impl Iterator<Item = ExprId>,
    actual: &BTreeMap<ExprId, CheckedExpression>,
) -> Result<(), FinalSemanticAnalysisError> {
    let expected = expected.collect::<BTreeSet<_>>();
    if expected.iter().any(|owner| !actual.contains_key(owner)) {
        return Err(FinalSemanticAnalysisError::MissingFact {
            family: SemanticFactFamily::Expression,
        });
    }
    if let Some(owner) = actual.keys().find(|owner| !expected.contains(owner)) {
        return Err(FinalSemanticAnalysisError::UnexpectedExpressionFact { owner: *owner });
    }
    Ok(())
}

fn require_complete<K: Copy + Ord, V>(
    expected: impl Iterator<Item = K>,
    actual: &BTreeMap<K, V>,
    family: SemanticFactFamily,
) -> Result<(), FinalSemanticAnalysisError> {
    let mut observed = 0usize;
    for owner in expected {
        observed = observed.saturating_add(1);
        if !actual.contains_key(&owner) {
            return Err(FinalSemanticAnalysisError::MissingFact { family });
        }
    }
    if observed != actual.len() {
        return Err(FinalSemanticAnalysisError::InvalidOwner);
    }
    Ok(())
}

pub(super) fn validate_types(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    types: &BTreeMap<TypeId, TypeKind>,
) -> Result<(), FinalSemanticAnalysisError> {
    for (&owner, ty) in types {
        let node = resolve_module(modules, owner.module())?
            .resolve_type(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if node.is_poisoned() {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        }
        if ty.contains_nominal_poison() {
            return Err(FinalSemanticAnalysisError::PoisonedType);
        }
        if ty.contains_dialogue_line_operation() {
            return Err(FinalSemanticAnalysisError::DialogueLineEscape {
                escape_span: required_source(
                    resolve_module(modules, owner.module())?,
                    HirSourceQuery::Type {
                        owner,
                        role: HirTypeSourceRole::Whole,
                    },
                )?,
            });
        }
    }
    Ok(())
}

pub(super) fn validate_bindings(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    locals: &BTreeMap<LocalId, CheckedBinding>,
    captures: &BTreeMap<CaptureId, CheckedBinding>,
) -> Result<(), FinalSemanticAnalysisError> {
    for (&owner, fact) in locals {
        let local = resolve_module(modules, owner.module())?
            .resolve_local(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if fact.role() == CheckedBindingRole::DialogueViewParameter
            && local.kind() != arcweft_lang_hir::scope::HirLocalKind::Parameter
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        if fact.ty().contains_nominal_poison() {
            return Err(FinalSemanticAnalysisError::PoisonedType);
        }
        if fact.ty().contains_dialogue_line_operation() {
            return Err(FinalSemanticAnalysisError::DialogueLineEscape {
                escape_span: required_source(
                    resolve_module(modules, owner.module())?,
                    HirSourceQuery::Local {
                        owner,
                        role: HirLocalSourceRole::Name,
                    },
                )?,
            });
        }
    }
    for (&owner, fact) in captures {
        let capture = resolve_module(modules, owner.module())?
            .resolve_capture(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if fact.ty().contains_nominal_poison() {
            return Err(FinalSemanticAnalysisError::PoisonedType);
        }
        if fact.ty().contains_dialogue_line_operation() {
            return Err(FinalSemanticAnalysisError::DialogueLineEscape {
                escape_span: required_source(
                    resolve_module(modules, owner.module())?,
                    HirSourceQuery::Local {
                        owner: capture.local(),
                        role: HirLocalSourceRole::Name,
                    },
                )?,
            });
        }
    }
    Ok(())
}

pub(super) fn validate_expressions(
    symbols: &ProjectSymbolTable,
    topology: &Arc<HirProjectEvaluationTopology>,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    dialogue_lines: &arcweft_lang_hir::project::AcceptedDialogueLineInventory,
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    calls: &BTreeMap<ExprId, CallTargetFacts>,
) -> Result<(), FinalSemanticAnalysisError> {
    for (&owner, fact) in expressions {
        let expression = resolve_module(modules, owner.module())?
            .resolve_expr(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if expression.is_poisoned() {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        }
        if !matches!(expression.kind(), HirExprKind::Match(_)) && fact.match_fact().is_some() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let accepts_nested_path_evidence = matches!(
            (expression.kind(), fact.resolution()),
            (
                HirExprKind::Choice(_),
                CheckedExpressionResolution::Choice(_)
            ) | (
                HirExprKind::DialogueContentApplication(_),
                CheckedExpressionResolution::DialogueApplication { .. }
            )
        );
        if !accepts_nested_path_evidence && fact.nested_path_evidence().is_some() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        if fact.ty().contains_nominal_poison() {
            return Err(FinalSemanticAnalysisError::PoisonedType);
        }
        if fact.ty().contains_dialogue_line_operation()
            && !matches!(
                (fact.ty(), fact.resolution()),
                (
                    TypeKind::DialogueLine(_),
                    CheckedExpressionResolution::DialogueApplication { .. }
                        | CheckedExpressionResolution::PostfixBracket(
                            PostfixBracketResolution::Dialogue { .. }
                        )
                )
            )
        {
            return Err(FinalSemanticAnalysisError::DialogueLineEscape {
                escape_span: required_expression_source(
                    resolve_module(modules, owner.module())?,
                    owner,
                    HirExprSourceRole::Whole,
                )?,
            });
        }
        if !expression_resolution_matches(expression.kind(), fact.resolution())
            && !nominal_fallback_receiver_matches(owner, fact, modules, calls)?
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        validate_expression_resolution(
            symbols,
            topology,
            modules,
            dialogue_lines,
            expressions,
            owner,
            fact.ty(),
            fact.resolution(),
        )?;
        if let CheckedExpressionResolution::Value(CheckedValueResolution::ProjectItem(item)) =
            fact.resolution()
            && &item.ty() != fact.ty()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        if let CheckedExpressionResolution::Value(CheckedValueResolution::Entry(entry)) =
            fact.resolution()
            && &entry.ty() != fact.ty()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let call_backed = matches!(
            fact.resolution(),
            CheckedExpressionResolution::Call
                | CheckedExpressionResolution::CharacterDialogueFactory(_)
                | CheckedExpressionResolution::CharacterDialogueReconfigure(_)
                | CheckedExpressionResolution::DialogueApplication { .. }
        );
        if call_backed != calls.contains_key(&owner) {
            return Err(FinalSemanticAnalysisError::CallFactMismatch);
        }
    }
    Ok(())
}

fn nominal_fallback_receiver_matches(
    owner: ExprId,
    fact: &CheckedExpression,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    calls: &BTreeMap<ExprId, CallTargetFacts>,
) -> Result<bool, FinalSemanticAnalysisError> {
    if !matches!(fact.resolution(), CheckedExpressionResolution::Structural) {
        return Ok(false);
    }
    for call_fact in calls.values() {
        let expression = resolve_module(modules, call_fact.expression().module())?
            .resolve_expr(call_fact.expression())
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if let HirExprKind::Call(hir_call) = expression.kind()
            && matches!(
                hir_call.callee(),
                arcweft_lang_hir::expr::HirCallCallee::UnresolvedDot {
                    value_receiver,
                    ..
                } if *value_receiver == owner
            )
        {
            let accepted = match call_fact.outcome() {
                CallAnalysisOutcome::Selected(application) => application
                    .core()
                    .direct_type_receiver()
                    .is_some_and(|receiver| receiver == fact.ty()),
                CallAnalysisOutcome::Ambiguous(evidence) => {
                    matches!(
                        evidence.callee(),
                        Some(CallCalleeClassificationFact::AssociatedType { .. })
                    ) || (matches!(
                        evidence.callee(),
                        Some(CallCalleeClassificationFact::Value { expression })
                            if expression == owner
                    ) && free_path_call_target(call_fact))
                }
                CallAnalysisOutcome::Rejected(evidence) => {
                    matches!(
                        evidence.callee(),
                        Some(CallCalleeClassificationFact::AssociatedType { .. })
                    ) || (matches!(
                        evidence.callee(),
                        Some(CallCalleeClassificationFact::Value { expression })
                            if expression == owner
                    ) && free_path_call_target(call_fact))
                }
                CallAnalysisOutcome::NonCallable(_) | CallAnalysisOutcome::Missing(_) => false,
            };
            if accepted {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn free_path_call_target(call: &CallTargetFacts) -> bool {
    let is_free = |candidate: &ResolvedCallable| {
        matches!(
            candidate.instantiation(),
            crate::callable::ResolvedCallableBaseInstantiation::None
        )
    };
    match call.outcome() {
        CallAnalysisOutcome::Selected(application) => {
            is_free(application.core().candidates().selected().as_ref())
        }
        CallAnalysisOutcome::Ambiguous(evidence) => {
            !evidence.candidates().is_empty()
                && evidence
                    .candidates()
                    .iter()
                    .all(|candidate| is_free(candidate))
        }
        CallAnalysisOutcome::Rejected(evidence) => {
            !evidence.candidates().is_empty()
                && evidence
                    .candidates()
                    .iter()
                    .all(|candidate| is_free(candidate))
        }
        CallAnalysisOutcome::Missing(_) | CallAnalysisOutcome::NonCallable(_) => false,
    }
}

fn expression_resolution_matches(
    kind: &HirExprKind,
    resolution: &CheckedExpressionResolution,
) -> bool {
    match (kind, resolution) {
        (HirExprKind::Literal(authored), CheckedExpressionResolution::Literal(checked)) => {
            authored == checked
        }
        (
            HirExprKind::Path(_) | HirExprKind::EntityReference(_),
            CheckedExpressionResolution::Value(_),
        )
        | (HirExprKind::Select(_), CheckedExpressionResolution::Select(_))
        | (
            HirExprKind::Record(_) | HirExprKind::RecordLiteral(_),
            CheckedExpressionResolution::Nominal(_),
        )
        | (
            HirExprKind::Path(_) | HirExprKind::ShortVariant(_),
            CheckedExpressionResolution::Variant(_),
        )
        | (HirExprKind::ShortVariant(_), CheckedExpressionResolution::StageLook(_))
        | (
            HirExprKind::Path(_) | HirExprKind::Select(_) | HirExprKind::Call(_),
            CheckedExpressionResolution::Effect(_),
        )
        | (
            HirExprKind::EntityReference(_),
            CheckedExpressionResolution::DialogueLineReference(_)
            | CheckedExpressionResolution::DialogueLineCoordinate(_)
            | CheckedExpressionResolution::DialogueTextKeyCoordinate(_),
        )
        | (
            HirExprKind::Call(_),
            CheckedExpressionResolution::CharacterDialogueFactory(_)
            | CheckedExpressionResolution::CharacterDialogueReconfigure(_)
            | CheckedExpressionResolution::Call
            | CheckedExpressionResolution::ViewCall(_)
            | CheckedExpressionResolution::StyleValue(_),
        )
        | (
            HirExprKind::Path(_),
            CheckedExpressionResolution::ViewCallee(_)
            | CheckedExpressionResolution::StyleCallee(_),
        )
        | (HirExprKind::Await(_), CheckedExpressionResolution::Await(_))
        | (HirExprKind::Choice(_), CheckedExpressionResolution::Choice(_))
        | (
            HirExprKind::Placeholder(HirPlaceholderKind::PartialApplication),
            CheckedExpressionResolution::ImplicitParameter { .. },
        )
        | (
            HirExprKind::Placeholder(HirPlaceholderKind::PipeLeft),
            CheckedExpressionResolution::PipeLeft { .. },
        ) => true,
        (
            HirExprKind::DialogueContentApplication(application),
            CheckedExpressionResolution::DialogueApplication { rich_text, .. },
        ) => rich_text.content().id() == application.content().id() && rich_text.is_valid(),
        (
            HirExprKind::PostfixBracket(postfix),
            CheckedExpressionResolution::PostfixBracket(resolution),
        ) => match (postfix.candidates(), resolution) {
            (
                arcweft_lang_hir::dialogue_application::HirPostfixBracketCandidates::Ambiguous {
                    index: authored,
                    ..
                },
                PostfixBracketResolution::Index { candidate: checked },
            )
            | (
                arcweft_lang_hir::dialogue_application::HirPostfixBracketCandidates::Ambiguous {
                    dialogue: authored,
                    ..
                },
                PostfixBracketResolution::Dialogue { candidate: checked },
            ) => authored == checked,
            _ => false,
        },
        (HirExprKind::Try(authored), CheckedExpressionResolution::Try(checked)) => {
            authored.operand() == checked.operand()
        }
        (kind, CheckedExpressionResolution::ImplicitCallable(callable)) => {
            expression_resolution_matches(kind, callable.body_resolution())
        }
        (HirExprKind::Closure(_), CheckedExpressionResolution::Closure(_)) => true,
        (HirExprKind::Pipe(authored), CheckedExpressionResolution::Pipe(checked)) => {
            authored.left() == checked.left() && authored.right() == checked.right()
        }
        (kind, CheckedExpressionResolution::Structural) => structural_resolution_matches(kind),
        _ => false,
    }
}

const fn structural_resolution_matches(kind: &HirExprKind) -> bool {
    matches!(
        kind,
        HirExprKind::Unit
            | HirExprKind::LifetimePath(_)
            | HirExprKind::Tuple(_)
            | HirExprKind::BracketSequence(_)
            | HirExprKind::NumericBracketSequence(_)
            | HirExprKind::ArrayRepeat(_)
            | HirExprKind::Index(_)
            | HirExprKind::Await(_)
            | HirExprKind::Thread(_)
            | HirExprKind::Range(_)
            | HirExprKind::Binary(_)
            | HirExprKind::Borrow(_)
            | HirExprKind::Dereference(_)
            | HirExprKind::Unary(_)
            | HirExprKind::Block(_)
            | HirExprKind::ComputationBlock(_)
            | HirExprKind::NamedBlock(_)
            | HirExprKind::Loop(_)
            | HirExprKind::If(_)
            | HirExprKind::IfLet(_)
            | HirExprKind::Match(_)
            | HirExprKind::ForSynthetic(_)
    )
}

fn validate_expression_resolution(
    symbols: &ProjectSymbolTable,
    topology: &Arc<HirProjectEvaluationTopology>,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    dialogue_lines: &arcweft_lang_hir::project::AcceptedDialogueLineInventory,
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    owner: ExprId,
    ty: &TypeKind,
    resolution: &CheckedExpressionResolution,
) -> Result<(), FinalSemanticAnalysisError> {
    match resolution {
        CheckedExpressionResolution::Closure(closure) => {
            validate_explicit_closure(topology, owner, closure)
        }
        CheckedExpressionResolution::ImplicitCallable(callable) => validate_implicit_callable(
            symbols,
            topology,
            modules,
            dialogue_lines,
            expressions,
            owner,
            callable,
        ),
        CheckedExpressionResolution::ImplicitParameter { callable } => {
            validate_implicit_parameter(expressions, owner, *callable)
        }
        CheckedExpressionResolution::Pipe(pipe) => validate_pipe(modules, expressions, owner, pipe),
        CheckedExpressionResolution::PipeLeft { pipe } => {
            validate_pipe_left(expressions, owner, *pipe)
        }
        CheckedExpressionResolution::Value(value) => validate_value(symbols, modules, value),
        CheckedExpressionResolution::Select(select) => match select {
            CheckedSelectResolution::Method(method) => method
                .has_valid_receiver_identity()
                .then_some(())
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog),
            CheckedSelectResolution::AgentField { .. }
            | CheckedSelectResolution::ProgressField { .. } => Ok(()),
            CheckedSelectResolution::Field(selection) => {
                validate_field_selection(modules, expressions, owner, ty, selection)
            }
            CheckedSelectResolution::DialogueView { projection, field } => {
                validate_field_selection(modules, expressions, owner, ty, field)?;
                (match projection {
                    crate::dialogue_view::DialogueProjectionCoordinate::Character(character) => {
                        character.field()
                    }
                    other => other.field(),
                } == field.diagnostic_name().as_str())
                .then_some(())
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
            }
        },
        CheckedExpressionResolution::Nominal(nominal) => {
            validate_nominal(symbols, modules, nominal)
        }
        CheckedExpressionResolution::Variant(variant) => {
            validate_variant(symbols, modules, variant)
        }
        CheckedExpressionResolution::CharacterDialogueFactory(factory) => {
            validate_character_dialogue_call(
                symbols,
                modules,
                expressions,
                owner,
                factory.target(),
                factory.patch(),
            )
        }
        CheckedExpressionResolution::CharacterDialogueReconfigure(reconfigure) => {
            validate_character_dialogue_call(
                symbols,
                modules,
                expressions,
                owner,
                reconfigure.target(),
                reconfigure.patch(),
            )
        }
        CheckedExpressionResolution::DialogueApplication {
            target,
            application_patch,
            ..
        } => {
            validate_character_dialogue_application_target(
                symbols,
                modules,
                expressions,
                owner,
                target,
            )?;
            application_patch
                .as_ref()
                .map(|patch| {
                    validate_character_dialogue_patch(
                        modules,
                        expressions,
                        target.expression(),
                        patch,
                    )
                })
                .transpose()
                .map(|_| ())
        }
        CheckedExpressionResolution::DialogueLineReference(target)
        | CheckedExpressionResolution::DialogueLineCoordinate(target) => dialogue_lines
            .get(target)
            .map(|_| ())
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily),
        CheckedExpressionResolution::DialogueTextKeyCoordinate(target) => dialogue_lines
            .records()
            .iter()
            .any(|line| line.text_key() == target)
            .then_some(())
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily),
        CheckedExpressionResolution::Try(tried) => {
            let operand = expressions
                .get(&tried.operand())
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            let carrier_matches = match (tried.carrier(), operand.ty()) {
                (
                    CheckedTryCarrier::Result { success, residual },
                    TypeKind::Result { ok, error },
                ) => success == ok.as_ref() && residual.as_ref() == error.as_ref(),
                (CheckedTryCarrier::Option { success }, TypeKind::Option(item)) => {
                    success == item.as_ref()
                }
                _ => false,
            };
            if !carrier_matches {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            match tried.boundary() {
                CheckedTryBoundary::Infallible => matches!(
                    tried.carrier(),
                    CheckedTryCarrier::Result { residual, .. }
                        if matches!(residual.as_ref(), TypeKind::Never)
                )
                .then_some(())
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily),
                CheckedTryBoundary::CarrierBlock(boundary) => {
                    let boundary = resolve_module(modules, boundary.module())?
                        .resolve_expr(boundary)
                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                    let HirExprKind::ComputationBlock(block) = boundary.kind() else {
                        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                    };
                    matches!(
                        (block.kind(), tried.carrier()),
                        (
                            arcweft_lang_hir::expr::HirComputationBlockKind::Result,
                            CheckedTryCarrier::Result { .. }
                        ) | (
                            arcweft_lang_hir::expr::HirComputationBlockKind::Option,
                            CheckedTryCarrier::Option { .. }
                        )
                    )
                    .then_some(())
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
                }
                CheckedTryBoundary::FunctionSite(boundary) => {
                    let expression = resolve_module(modules, boundary.module())?
                        .resolve_expr(boundary)
                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                    (matches!(expression.kind(), HirExprKind::Closure(_))
                        || matches!(
                            expressions
                                .get(&boundary)
                                .map(CheckedExpression::resolution),
                            Some(CheckedExpressionResolution::ImplicitCallable(_))
                        ))
                    .then_some(())
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
                }
                CheckedTryBoundary::Callable(boundary) => {
                    resolve_module(modules, boundary.module())?
                        .resolve_item(boundary)
                        .map(|_| ())
                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)
                }
            }
        }
        CheckedExpressionResolution::Choice(choice) => {
            validate_choice(symbols, modules, owner, choice)
        }
        CheckedExpressionResolution::StageLook(look) => {
            let expression = resolve_module(modules, owner.module())?
                .resolve_expr(owner)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            let HirExprKind::ShortVariant(authored) = expression.kind() else {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            };
            let authored = authored
                .as_resolved()
                .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
            (look.matches_type(ty) && look.diagnostic_name() == authored)
                .then_some(())
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
        }
        CheckedExpressionResolution::Await(_)
        | CheckedExpressionResolution::PostfixBracket(_)
        | CheckedExpressionResolution::Effect(_)
        | CheckedExpressionResolution::ViewCall(_)
        | CheckedExpressionResolution::ViewCallee(_)
        | CheckedExpressionResolution::StyleValue(_)
        | CheckedExpressionResolution::StyleCallee(_)
        | CheckedExpressionResolution::Structural
        | CheckedExpressionResolution::Literal(_)
        | CheckedExpressionResolution::Call => Ok(()),
    }
}

fn validate_field_selection(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    owner: ExprId,
    ty: &TypeKind,
    selection: &super::CheckedFieldSelection,
) -> Result<(), FinalSemanticAnalysisError> {
    let expression = resolve_module(modules, owner.module())?
        .resolve_expr(owner)
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
    let HirExprKind::Select(select) = expression.kind() else {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    };
    let target = expressions.get(&select.target()).ok_or(
        FinalSemanticAnalysisError::ExpressionTypeUnavailable {
            owner: select.target(),
        },
    )?;
    let valid_family = match (selection.field(), selection.runtime_field(), target.ty()) {
        (
            crate::record_field::CheckedRecordFieldSemanticId::Project(_),
            Some(runtime_field),
            TypeKind::ProjectNominal(_),
        ) => runtime_field.zero_based() == selection.declaration_ordinal(),
        (
            crate::record_field::CheckedRecordFieldSemanticId::Environment(_),
            None,
            TypeKind::Named(_),
        ) => true,
        _ => false,
    };
    (valid_family
        && target.ty().semantic_identity_digest() == selection.owner_type()
        && ty.semantic_identity_digest() == selection.field_type())
    .then_some(())
    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
}

fn validate_choice(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    owner: ExprId,
    choice: &CheckedChoice,
) -> Result<(), FinalSemanticAnalysisError> {
    let expression = resolve_module(modules, owner.module())?
        .resolve_expr(owner)
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
    let HirExprKind::Choice(authored) = expression.kind() else {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    };
    if choice.option_ids().len() != authored.body().items().len()
        || choice
            .public_id()
            .is_some_and(|id| id.as_str().split('.').next() != Some("choice"))
        || choice
            .option_ids()
            .iter()
            .any(|id| id.as_str().split('.').next() != Some("choice"))
    {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }
    let mut previous = None;
    for goto in choice.gotos() {
        if previous.is_some_and(|previous| previous >= goto.arm()) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let index = usize::try_from(goto.arm())
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        if !matches!(
            authored.body().items().get(index),
            Some(arcweft_lang_hir::expr::HirChoiceItem::CompactArm(arm))
                if matches!(
                    arm.action(),
                    arcweft_lang_hir::expr::HirChoiceCompactAction::Goto(_)
                )
        ) || goto.target().family() != arcweft_id::DeclarationIdentityFamily::Flow
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        validate_project_item(symbols, modules, goto.target())?;
        previous = Some(goto.arm());
    }
    Ok(())
}

fn validate_implicit_callable(
    symbols: &ProjectSymbolTable,
    topology: &Arc<HirProjectEvaluationTopology>,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    dialogue_lines: &arcweft_lang_hir::project::AcceptedDialogueLineInventory,
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    owner: ExprId,
    callable: &CheckedImplicitCallable,
) -> Result<(), FinalSemanticAnalysisError> {
    let checked = expressions
        .get(&owner)
        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
    let TypeKind::Function {
        params,
        return_type,
        ..
    } = checked.ty()
    else {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    };
    if params.len() != 1
        || &params[0] != callable.parameter()
        || return_type.as_ref() != callable.result()
        || callable.placeholders().is_empty()
    {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }
    callable.validate_authority(topology, owner)?;
    for placeholder in callable.placeholders() {
        let expression = resolve_module(modules, placeholder.module())?
            .resolve_expr(*placeholder)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let (resolution, type_matches) = if *placeholder == owner {
            (callable.body_resolution(), true)
        } else {
            let fact = expressions
                .get(placeholder)
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            (fact.resolution(), fact.ty() == callable.parameter())
        };
        if !matches!(
            expression.kind(),
            HirExprKind::Placeholder(HirPlaceholderKind::PartialApplication)
        ) || !type_matches
            || !matches!(
                resolution,
                CheckedExpressionResolution::ImplicitParameter { callable } if *callable == owner
            )
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
    }
    validate_expression_resolution(
        symbols,
        topology,
        modules,
        dialogue_lines,
        expressions,
        owner,
        callable.result(),
        callable.body_resolution(),
    )
}

fn validate_explicit_closure(
    topology: &Arc<HirProjectEvaluationTopology>,
    owner: ExprId,
    checked: &super::CheckedClosure,
) -> Result<(), FinalSemanticAnalysisError> {
    checked.validate_authority(topology, owner)?;
    Ok(())
}

fn validate_implicit_parameter(
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    owner: ExprId,
    callable: ExprId,
) -> Result<(), FinalSemanticAnalysisError> {
    expressions
        .get(&callable)
        .and_then(|callable_fact| match callable_fact.resolution() {
            CheckedExpressionResolution::ImplicitCallable(implicit)
                if implicit.placeholders().contains(&owner) =>
            {
                Some(())
            }
            _ => None,
        })
        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
}

fn validate_pipe(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    owner: ExprId,
    pipe: &CheckedPipe,
) -> Result<(), FinalSemanticAnalysisError> {
    let authored = resolve_module(modules, owner.module())?
        .resolve_expr(owner)
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
    let HirExprKind::Pipe(authored) = authored.kind() else {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    };
    if authored.left() != pipe.left() || authored.right() != pipe.right() {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }
    for placeholder in pipe.placeholders() {
        let fact = expressions
            .get(placeholder)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        if !matches!(
            fact.resolution(),
            CheckedExpressionResolution::PipeLeft { pipe } if *pipe == owner
        ) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
    }
    Ok(())
}

fn validate_pipe_left(
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    owner: ExprId,
    pipe: ExprId,
) -> Result<(), FinalSemanticAnalysisError> {
    expressions
        .get(&pipe)
        .and_then(|pipe_fact| match pipe_fact.resolution() {
            CheckedExpressionResolution::Pipe(checked)
                if checked.placeholders().contains(&owner) =>
            {
                Some(())
            }
            _ => None,
        })
        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
}

fn validate_value(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    value: &CheckedValueResolution,
) -> Result<(), FinalSemanticAnalysisError> {
    match value {
        CheckedValueResolution::Local(local) => resolve_module(modules, local.module())?
            .resolve_local(*local)
            .map(|_| ())
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner),
        CheckedValueResolution::CharacterField { receiver, .. } => {
            validate_value(symbols, modules, receiver)
        }
        CheckedValueResolution::ProjectCallable(callable) => {
            validate_callable(symbols, modules, callable)
        }
        CheckedValueResolution::ProjectItem(item) => validate_project_item(symbols, modules, item),
        CheckedValueResolution::Entry(entry) => validate_entry_reference(modules, entry),
        CheckedValueResolution::LineContext
        | CheckedValueResolution::Registered(_)
        | CheckedValueResolution::Constant(_) => Ok(()),
    }
}

fn validate_entry_reference(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    entry: &CheckedEntryReference,
) -> Result<(), FinalSemanticAnalysisError> {
    let item = resolve_item(modules, entry.lookup_owner())?;
    let HirItemKind::Entry(declaration) = item.kind() else {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    };
    let Some(HirIdRef::Absolute(public_id)) =
        declaration.id().value().and_then(|id| id.as_resolved())
    else {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    };
    (public_id.as_str() == entry.diagnostic_public_id().as_str())
        .then_some(())
        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
}

pub(super) fn validate_patterns(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    types: &BTreeMap<TypeId, TypeKind>,
    patterns: &BTreeMap<PatternId, CheckedPattern>,
) -> Result<(), FinalSemanticAnalysisError> {
    for (&owner, fact) in patterns {
        let pattern = resolve_module(modules, owner.module())?
            .resolve_pattern(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if pattern.is_poisoned() {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        }
        if fact.ty().contains_nominal_poison() {
            return Err(FinalSemanticAnalysisError::PoisonedType);
        }
        let matches = match (pattern.kind(), fact.resolution()) {
            (HirPatternKind::Literal(authored), CheckedPatternResolution::Literal(checked)) => {
                authored == checked
            }
            (HirPatternKind::EntityReference(_), CheckedPatternResolution::Entity(item)) => {
                validate_project_item(symbols, modules, item).is_ok() && &item.ty() == fact.ty()
            }
            (HirPatternKind::Variant(_), CheckedPatternResolution::Variant(variant)) => {
                validate_variant(symbols, modules, variant).is_ok()
            }
            (HirPatternKind::Record { .. }, CheckedPatternResolution::Record(record)) => {
                let exact_owner = match record.owner() {
                    super::CheckedRecordPatternOwner::Project { nominal, .. } => {
                        validate_nominal(symbols, modules, nominal).is_ok()
                    }
                    super::CheckedRecordPatternOwner::Environment { record } => {
                        record.semantic_type() == fact.ty().semantic_identity_digest()
                    }
                    super::CheckedRecordPatternOwner::VariantPayload { payload, .. } => {
                        fact.ty() == &TypeKind::VariantPayload(Box::new(payload.clone()))
                    }
                };
                exact_owner
                    && record.owner().semantic_type() == fact.ty().semantic_identity_digest()
            }
            (
                HirPatternKind::TypedBinding { ty: annotation, .. },
                CheckedPatternResolution::TypedBinding(checked),
            ) => {
                let mut compatibility_control = NoopTypeCompatibilityControl;
                let relation = match fact.ty() {
                    // Choice patterns retain the scrutinee-to-annotation
                    // direction. Reversing this would turn a multi-
                    // alternative scrutinee into an apparent single-choice
                    // success and discard the unique-injection rule.
                    TypeKind::Choice(_) => fact.ty().accepts_with(
                        checked.annotation(),
                        TypeCompatibilityPolicy::Invariant,
                        &mut compatibility_control,
                    ),
                    _ => checked.annotation().accepts_with(
                        fact.ty(),
                        TypeCompatibilityPolicy::Invariant,
                        &mut compatibility_control,
                    ),
                };
                let compatible = match relation {
                    Ok(compatible) => compatible,
                    Err(TypeCompatibilityFailure::Forbidden { kind, .. }) => {
                        return Err(match kind {
                            TypeCompatibilityForbidden::Error
                            | TypeCompatibilityForbidden::ArrayLengthError => {
                                FinalSemanticAnalysisError::PoisonedType
                            }
                            TypeCompatibilityForbidden::Projection
                            | TypeCompatibilityForbidden::Placeholder
                            | TypeCompatibilityForbidden::ArrayLengthInferred
                            | TypeCompatibilityForbidden::UnknownEffectTail => {
                                FinalSemanticAnalysisError::WrongPayloadFamily
                            }
                        });
                    }
                    Err(TypeCompatibilityFailure::Control(error)) => match error {},
                };
                types.get(annotation) == Some(checked.annotation())
                    && checked.has_valid_semantic_identity()
                    && compatible
            }
            (
                HirPatternKind::Binding(_)
                | HirPatternKind::MutableBinding(_)
                | HirPatternKind::Discard
                | HirPatternKind::Tuple { .. }
                | HirPatternKind::BracketSequence { .. }
                | HirPatternKind::WholeBinding { .. }
                | HirPatternKind::Or { .. },
                CheckedPatternResolution::Structural,
            ) => true,
            _ => false,
        };
        if !matches {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
    }
    Ok(())
}

fn validate_character_dialogue_target(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    target: &CheckedCharacterDialogueTarget,
) -> Result<(), FinalSemanticAnalysisError> {
    let checked = expressions
        .get(&target.expression())
        .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
    match target {
        CheckedCharacterDialogueTarget::Character {
            item: Some(item),
            character,
            ..
        } => {
            if item.family() != DeclarationIdentityFamily::Character {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            if item.character().as_ref() != character.exact()
                || !checked
                    .ty()
                    .is_entity_ref_kind(&crate::types::EntityKind::Character)
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            validate_project_item(symbols, modules, item)
        }
        CheckedCharacterDialogueTarget::Character {
            item: None,
            character,
            ..
        } => (character.exact().is_none()
            && checked
                .ty()
                .is_entity_ref_kind(&crate::types::EntityKind::Character))
        .then_some(())
        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily),
        CheckedCharacterDialogueTarget::Dialogue { ty, .. } => (checked.ty()
            == &TypeKind::CharacterDialogue(ty.clone()))
            .then_some(())
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily),
    }
}

fn validate_character_dialogue_call(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    owner: ExprId,
    target: &CheckedCharacterDialogueTarget,
    patch: &CheckedCharacterDialoguePatch,
) -> Result<(), FinalSemanticAnalysisError> {
    let module = resolve_module(modules, owner.module())?;
    let expression = module
        .resolve_expr(owner)
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
    let HirExprKind::Call(call) = expression.kind() else {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    };
    if !matches!(call.callee(), HirCallCallee::Value { value } if *value == target.expression()) {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }
    validate_character_dialogue_target(symbols, modules, expressions, target)?;
    validate_character_dialogue_patch(modules, expressions, owner, patch)
}

fn validate_character_dialogue_application_target(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    owner: ExprId,
    target: &CheckedCharacterDialogueTarget,
) -> Result<(), FinalSemanticAnalysisError> {
    let expression = resolve_module(modules, owner.module())?
        .resolve_expr(owner)
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
    let HirExprKind::DialogueContentApplication(application) = expression.kind() else {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    };
    if application.target() != target.expression() {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }
    validate_character_dialogue_target(symbols, modules, expressions, target)
}

fn validate_character_dialogue_patch(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    owner: ExprId,
    patch: &CheckedCharacterDialoguePatch,
) -> Result<(), FinalSemanticAnalysisError> {
    let module = resolve_module(modules, owner.module())?;
    let expression = module
        .resolve_expr(owner)
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
    let HirExprKind::Call(call) = expression.kind() else {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    };
    let whole = required_expression_source(module, owner, HirExprSourceRole::Whole)?;
    if patch.source() != &whole || patch.fields().len() > call.arguments().len() {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }

    let mut argument_sources = Vec::with_capacity(call.arguments().len());
    for index in 0..call.arguments().len() {
        let argument = arcweft_lang_hir::expr::HirCallArgumentOrdinal::try_from_usize(index)
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        argument_sources.push(required_expression_source(
            module,
            owner,
            HirExprSourceRole::CallArgument {
                argument,
                part: arcweft_lang_hir::source_index::HirCallArgumentSourcePart::Whole,
            },
        )?);
    }

    let mut last_argument = None;
    let mut coordinates = BTreeSet::new();
    for field in patch.fields() {
        if !coordinates.insert(field.coordinate()) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let argument_index = argument_sources
            .iter()
            .position(|source| source == field.source())
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        if last_argument.is_some_and(|previous| argument_index <= previous) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        last_argument = Some(argument_index);
        let argument = &call.arguments()[argument_index];
        let checked = expressions
            .get(&argument.value())
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        match field.operation() {
            CheckedPatchOperation::Set { value, ty } => {
                if *value != argument.value() || checked.ty() != ty {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
            }
            CheckedPatchOperation::Clear => {
                if !matches!(
                    checked.resolution(),
                    CheckedExpressionResolution::Variant(variant)
                        if matches!(variant.owner(), CheckedVariantOwner::Option { .. })
                            && variant.ordinal() == 1
                ) {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
            }
        }
    }
    Ok(())
}

fn required_expression_source(
    module: &HirModule,
    owner: ExprId,
    role: HirExprSourceRole,
) -> Result<arcweft_source::SourceSpan, FinalSemanticAnalysisError> {
    required_source(module, HirSourceQuery::Expr { owner, role })
}

fn required_source(
    module: &HirModule,
    query: HirSourceQuery,
) -> Result<arcweft_source::SourceSpan, FinalSemanticAnalysisError> {
    let lookup = module
        .source_site(module.provenance().source_identity(), query)
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
    match lookup.presence() {
        HirSourcePresence::Present(HirSourceSite::Span(span)) => Ok(span.clone()),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => Err(FinalSemanticAnalysisError::InvalidOwner),
    }
}

fn validate_project_item(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    item: &CheckedProjectItem,
) -> Result<(), FinalSemanticAnalysisError> {
    if !item.has_valid_semantic_identity() {
        return Err(FinalSemanticAnalysisError::InvalidOwner);
    }
    match item.owner() {
        CheckedProjectItemOwner::Retained(owner) => {
            let accepted = symbols
                .retained(item.public_id())
                .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
            if accepted.owner() != *owner || accepted.family() != item.family() {
                return Err(FinalSemanticAnalysisError::InvalidOwner);
            }
            let owner = resolve_item(modules, *owner)?;
            let family_matches = matches!(
                (accepted.family(), owner.kind()),
                (
                    DeclarationIdentityFamily::Character,
                    HirItemKind::Character(_)
                ) | (DeclarationIdentityFamily::View, HirItemKind::View(_))
                    | (DeclarationIdentityFamily::Action, HirItemKind::Action(_))
                    | (
                        DeclarationIdentityFamily::Activity,
                        HirItemKind::Activity(_)
                    )
                    | (DeclarationIdentityFamily::Signal, HirItemKind::Signal(_))
                    | (DeclarationIdentityFamily::Metric, HirItemKind::Metric(_))
                    | (DeclarationIdentityFamily::Layer, HirItemKind::Layer(_))
            );
            family_matches
                .then_some(())
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
        }
        CheckedProjectItemOwner::Flow {
            declaration,
            item: owner,
        } => {
            let arcweft_lang_hir::symbol::CallableDeclarationKey::Flow(flow) = declaration else {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            };
            if item.family() != DeclarationIdentityFamily::Flow
                || item.public_id() != flow.public_id()
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            let symbol = symbols
                .callable(declaration)
                .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
            if symbol.source_item() != *owner || symbol.owner() != CallableDeclarationOwner::Flow {
                return Err(FinalSemanticAnalysisError::InvalidOwner);
            }
            matches!(resolve_item(modules, *owner)?.kind(), HirItemKind::Flow(_))
                .then_some(())
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
        }
        CheckedProjectItemOwner::External(declaration) => {
            let character = item
                .character()
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            if item.family() != DeclarationIdentityFamily::Character
                || character.as_str() != item.public_id().as_str()
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            let external = symbols
                .external(*declaration)
                .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
            (external.canonical_path().canonical_string() == character.as_str())
                .then_some(())
                .ok_or(FinalSemanticAnalysisError::InvalidOwner)
        }
    }
}

pub(super) fn validate_statements(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    locals: &BTreeMap<LocalId, CheckedBinding>,
    statements: &BTreeMap<StmtId, CheckedStatement>,
    calls: &BTreeMap<ExprId, CallTargetFacts>,
) -> Result<(), FinalSemanticAnalysisError> {
    for (&owner, fact) in statements {
        let statement = resolve_module(modules, owner.module())?
            .resolve_stmt(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if statement.is_poisoned() {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        }
        match fact.payload() {
            CheckedStatementPayload::Assignment(assignment) => {
                let place = assignment.place();
                if locals.get(&place.local()).map(CheckedBinding::ty) != Some(&place.nominal().ty())
                    || place.field_type() != assignment.value_type()
                {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
            }
            CheckedStatementPayload::EvaluatedEffect(effect) => {
                let site = effect.application().raw();
                let call = calls
                    .get(&site.expression())
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let Some(application) = call.selected_application() else {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                };
                let selected = application.core().candidates().selected();
                if application.core().site() != site
                    || application.core().application_site() != effect.application()
                    || selected.schema().evaluated_effect() != Some(effect.disposition())
                    || !matches!(application.result(), CheckedCallResult::Value(_))
                {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
            }
            CheckedStatementPayload::Iteration(iteration) => {
                validate_iteration(modules, iteration)?;
            }
            CheckedStatementPayload::Structural
            | CheckedStatementPayload::Assertion(_)
            | CheckedStatementPayload::Defer(_)
            | CheckedStatementPayload::ControlTransfer(_)
            | CheckedStatementPayload::Trigger(_)
            | CheckedStatementPayload::UnsafeAudit(_)
            | CheckedStatementPayload::Select(_)
            | CheckedStatementPayload::SourceLocale(_)
            | CheckedStatementPayload::Scope(_)
            | CheckedStatementPayload::Include(_)
            | CheckedStatementPayload::Suspension(_)
            | CheckedStatementPayload::Yield => {}
        }
    }
    Ok(())
}

fn validate_iteration(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    iteration: &CheckedIteration,
) -> Result<(), FinalSemanticAnalysisError> {
    match iteration {
        CheckedIteration::Builtin { item, .. } => (!item.contains_nominal_poison())
            .then_some(())
            .ok_or(FinalSemanticAnalysisError::PoisonedType),
        CheckedIteration::Witness {
            source,
            item,
            into_iter,
            into_iterator,
            iterator,
        } => {
            if source.contains_nominal_poison()
                || item.contains_nominal_poison()
                || into_iter.contains_nominal_poison()
            {
                return Err(FinalSemanticAnalysisError::PoisonedType);
            }
            validate_conformance(modules, into_iterator)?;
            validate_conformance(modules, iterator)
        }
        CheckedIteration::IteratorWitness {
            source,
            item,
            iterator,
        } => {
            if source.contains_nominal_poison() || item.contains_nominal_poison() {
                return Err(FinalSemanticAnalysisError::PoisonedType);
            }
            validate_conformance(modules, iterator)
        }
    }
}

fn validate_conformance(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    conformance: &CheckedTraitConformance,
) -> Result<(), FinalSemanticAnalysisError> {
    let implementation = resolve_item(modules, conformance.implementation())?;
    let HirItemKind::Impl(implementation) = implementation.kind() else {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    };
    let method = implementation
        .members()
        .get(usize::from(conformance.method()));
    let Some(arcweft_lang_hir::item::HirImplMember::Function(method)) = method else {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    };
    if conformance.declaration().kind() != arcweft_lang_hir::symbol::ImplMethodKind::Trait
        || method
            .name()
            .resolved()
            .map(arcweft_lang_hir::leaf::HirName::as_str)
            != Some(conformance.declaration().method().as_str())
    {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }
    let expected_method = match conformance.trait_identity() {
        super::CheckedTraitIdentity::Project(trait_item) => {
            if !matches!(
                resolve_item(modules, *trait_item)?.kind(),
                HirItemKind::Trait(_)
            ) {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            None
        }
        super::CheckedTraitIdentity::StandardIterator => Some("next"),
        super::CheckedTraitIdentity::StandardIntoIterator => Some("into_iter"),
    };
    if expected_method.is_some_and(|expected| {
        method
            .name()
            .resolved()
            .map(arcweft_lang_hir::leaf::HirName::as_str)
            != Some(expected)
    }) {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }
    Ok(())
}

pub(super) fn validate_items(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    items: &BTreeMap<ItemId, CheckedItem>,
) -> Result<(), FinalSemanticAnalysisError> {
    for (&owner, fact) in items {
        let item = resolve_item(modules, owner)?;
        if item.is_poisoned() {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        }
        if item.kind().family() != fact.role().family() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        if let (HirItemKind::Flow(flow), CheckedItemRole::Flow { identity }) =
            (item.kind(), fact.role())
            && flow.identity() != identity
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        if let CheckedItemRole::Function {
            execution:
                CheckedFunctionExecution::StreamFactory {
                    item,
                    error,
                    own_scope_yields,
                },
            ..
        } = fact.role()
            && (*own_scope_yields == 0
                || item.contains_nominal_poison()
                || error.contains_nominal_poison())
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
    }
    Ok(())
}

pub(super) fn validate_calls(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    calls: &BTreeMap<ExprId, CallTargetFacts>,
) -> Result<(), FinalSemanticAnalysisError> {
    for (&owner, call) in calls {
        let expression = resolve_module(modules, owner.module())?
            .resolve_expr(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        match expression.kind() {
            HirExprKind::Call(hir_call) => validate_call_acceptance(call, hir_call)?,
            HirExprKind::DialogueContentApplication(application) => {
                validate_dialogue_application_call_acceptance(call, application)?;
            }
            _ => return Err(FinalSemanticAnalysisError::WrongPayloadFamily),
        }
        validate_call_callee(modules, call)?;
        validate_call_result(expressions, owner, call)?;
        validate_call_argument_slots(modules, expressions, call)?;
        validate_call_target(symbols, modules, call)?;
        validate_call_context(symbols, modules, call)?;
    }
    Ok(())
}

fn validate_dialogue_application_call_acceptance(
    call: &CallTargetFacts,
    application: &arcweft_lang_hir::dialogue_application::HirDialogueContentApplication,
) -> Result<(), FinalSemanticAnalysisError> {
    let Some(application_facts) = call.selected_application() else {
        return Err(FinalSemanticAnalysisError::UnacceptedCall);
    };
    let core = application_facts.core();
    let selected = core.candidates().selected();
    let execution = application_facts.core().execution();
    let semantic_operands = execution.semantic_operands();
    let structural_sources_match = matches!(
        semantic_operands,
        [target, content]
            if matches!(target.source(), crate::callable::CheckedCallSemanticOperandSource::DialogueTarget(source)
                if source.raw() == crate::callable::CheckedCallArgumentSlotSource::Expression(application.target()))
                && matches!(content.source(), crate::callable::CheckedCallSemanticOperandSource::DialogueContent { .. })
                && application.plan().is_none()
    ) || matches!(
        semantic_operands,
        [target, content, line_plan]
            if matches!(target.source(), crate::callable::CheckedCallSemanticOperandSource::DialogueTarget(source)
                if source.raw() == crate::callable::CheckedCallArgumentSlotSource::Expression(application.target()))
                && matches!(content.source(), crate::callable::CheckedCallSemanticOperandSource::DialogueContent { .. })
                && matches!(line_plan.source(), crate::callable::CheckedCallSemanticOperandSource::DialogueLinePlan { .. })
                && application.plan().is_some()
    );
    if !matches!(core.callee(), CheckedCallCalleeExecution::Direct)
        || !execution.arguments().is_empty()
        || !structural_sources_match
        || core.current_group() != crate::callable::CallableGroupIndex::ZERO
        || !matches!(application_facts.result(), CheckedCallResult::Value(_))
        || core.candidates().candidates().len() != 1
        || selected.id() != &CallableCandidateId::Dialogue(DialogueCallableId::ContentApplication)
        || selected.schema().validator()
            != &CallableValidator::Dialogue(DialogueCallableId::ContentApplication)
        || selected.schema().result() != application_facts.result().ty()
    {
        return Err(FinalSemanticAnalysisError::UnacceptedCall);
    }
    Ok(())
}

fn validate_call_acceptance(
    call: &CallTargetFacts,
    hir_call: &arcweft_lang_hir::expr::HirCallExpr,
) -> Result<(), FinalSemanticAnalysisError> {
    match call.outcome() {
        CallAnalysisOutcome::Selected(application) => {
            let arguments = application.core().execution().arguments();
            if arguments.len() != hir_call.arguments().len() {
                return Err(FinalSemanticAnalysisError::UnacceptedCall);
            }
            Ok(())
        }
        CallAnalysisOutcome::Ambiguous(evidence) if evidence.candidates().len() >= 2 => Ok(()),
        CallAnalysisOutcome::Rejected(evidence) if !evidence.candidates().is_empty() => Ok(()),
        CallAnalysisOutcome::NonCallable(_) | CallAnalysisOutcome::Missing(_) => {
            Err(FinalSemanticAnalysisError::UnacceptedCall)
        }
        CallAnalysisOutcome::Ambiguous(_) | CallAnalysisOutcome::Rejected(_) => {
            Err(FinalSemanticAnalysisError::UnacceptedCall)
        }
    }
}

fn validate_call_callee(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    call: &CallTargetFacts,
) -> Result<(), FinalSemanticAnalysisError> {
    let callee = match call.outcome() {
        CallAnalysisOutcome::Selected(_) => return Ok(()),
        CallAnalysisOutcome::Ambiguous(evidence) => evidence.callee(),
        CallAnalysisOutcome::Rejected(evidence) => evidence.callee(),
        CallAnalysisOutcome::NonCallable(evidence) => evidence.callee(),
        CallAnalysisOutcome::Missing(evidence) => evidence.callee(),
    };
    match callee {
        Some(CallCalleeClassificationFact::Value { expression }) => {
            resolve_module(modules, expression.module())?
                .resolve_expr(expression)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        }
        Some(CallCalleeClassificationFact::AssociatedType { receiver, .. }) => {
            resolve_module(modules, receiver.module())?
                .resolve_type(receiver)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        }
        None => {}
    }
    Ok(())
}

fn validate_call_result(
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    owner: ExprId,
    call: &CallTargetFacts,
) -> Result<(), FinalSemanticAnalysisError> {
    let checked = expressions
        .get(&owner)
        .ok_or(FinalSemanticAnalysisError::MissingFact {
            family: SemanticFactFamily::Expression,
        })?;
    let Some(application) = call.selected_application() else {
        // Unselected evidence deliberately owns no result/effect projection.
        return Ok(());
    };
    let effects = application.core().effects();
    if application.result().ty() != checked.ty()
        || !effects.is_known()
        || !effects.concrete().is_subset(checked.effects())
    {
        return Err(FinalSemanticAnalysisError::CallFactMismatch);
    }
    matches!(effects.tail(), crate::effect_row::EffectRowTail::Closed)
        .then_some(())
        .ok_or(FinalSemanticAnalysisError::OpenEffectRow)
}

fn validate_call_argument_slots(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    call: &CallTargetFacts,
) -> Result<(), FinalSemanticAnalysisError> {
    let Some(application) = call.selected_application() else {
        // Argument execution rows are selected-only; do not rebuild recovery
        // projections for an unselected outcome.
        return Ok(());
    };
    for argument in application.core().execution().arguments() {
        for slot in argument.slots() {
            let source = slot.source().raw();
            let inferred_matches_source = match source {
                // The C sealer has already consumed a checked-base projection
                // token and compared FrozenSolution::apply(projected raw) with
                // this exact inferred row. Final validation only rechecks that
                // the stable source coordinate still names a live fact.
                CheckedCallArgumentSlotSource::Expression(expression) => {
                    expressions.contains_key(&expression)
                }
                CheckedCallArgumentSlotSource::CompactNumericElement { sequence, ordinal } => {
                    let module = resolve_module(modules, sequence.module())?;
                    let expression = module
                        .resolve_expr(sequence)
                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                    let HirExprKind::NumericBracketSequence(sequence) = expression.kind() else {
                        return Err(FinalSemanticAnalysisError::InvalidOwner);
                    };
                    let ordinal = usize::try_from(ordinal)
                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                    if sequence.elements().get(ordinal).is_none() {
                        return Err(FinalSemanticAnalysisError::InvalidOwner);
                    }
                    let inferred =
                        compact_numeric_element_type(sequence.common_suffix(), slot.expected());
                    slot.inferred() == &inferred
                }
            };
            if !inferred_matches_source
                || slot
                    .expected()
                    .is_some_and(TypeKind::contains_nominal_poison)
            {
                return Err(FinalSemanticAnalysisError::CallFactMismatch);
            }
            if slot.inferred().contains_dialogue_line_operation() {
                let owner = match source {
                    CheckedCallArgumentSlotSource::Expression(expression) => expression,
                    CheckedCallArgumentSlotSource::CompactNumericElement { sequence, .. } => {
                        sequence
                    }
                };
                return Err(FinalSemanticAnalysisError::DialogueLineEscape {
                    escape_span: required_expression_source(
                        resolve_module(modules, owner.module())?,
                        owner,
                        HirExprSourceRole::Whole,
                    )?,
                });
            }
        }
    }
    Ok(())
}

fn validate_call_target(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    call: &CallTargetFacts,
) -> Result<(), FinalSemanticAnalysisError> {
    match call.outcome() {
        CallAnalysisOutcome::Selected(application) => {
            for candidate in application.core().candidates().candidates() {
                validate_resolved_callable(symbols, modules, candidate)?;
            }
        }
        CallAnalysisOutcome::Ambiguous(evidence) => {
            for candidate in evidence.considered() {
                validate_resolved_callable(symbols, modules, candidate)?;
            }
        }
        CallAnalysisOutcome::Rejected(evidence) => {
            for candidate in evidence.candidates() {
                validate_resolved_callable(symbols, modules, candidate)?;
            }
        }
        CallAnalysisOutcome::NonCallable(_) | CallAnalysisOutcome::Missing(_) => {
            return Err(FinalSemanticAnalysisError::UnacceptedCall);
        }
    }
    Ok(())
}

fn validate_call_context(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    call: &CallTargetFacts,
) -> Result<(), FinalSemanticAnalysisError> {
    if let Some(declaration) = call.enclosing_callable() {
        let symbol = symbols
            .callable(declaration)
            .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
        resolve_item(modules, symbol.source_item())?;
    }
    for diagnostic in call.diagnostics() {
        if let CallableDiagnosticSubject::Argument(expression) = diagnostic.subject() {
            resolve_module(modules, expression.module())?
                .resolve_expr(*expression)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        }
        validate_diagnostic_span(modules, diagnostic.span())?;
        for related in diagnostic.related() {
            validate_diagnostic_span(modules, related.span())?;
        }
    }
    Ok(())
}

pub(super) fn validate_physical_candidate_argument_evaluations(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    traces: &BTreeMap<ExprId, Arc<[PhysicalCandidateArgumentEvaluation]>>,
) -> Result<(), FinalSemanticAnalysisError> {
    for (&root, evaluations) in traces {
        let root_expression = resolve_module(modules, root.module())?
            .resolve_expr(root)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if !matches!(root_expression.kind(), HirExprKind::Call(_)) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        for evaluation in evaluations.iter() {
            let expression = resolve_module(modules, evaluation.call_expression().module())?
                .resolve_expr(evaluation.call_expression())
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            let HirExprKind::Call(call) = expression.kind() else {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            };
            if usize::from(evaluation.argument().get()) >= call.arguments().len()
                || evaluation.source().owner().module() != evaluation.call_expression().module()
            {
                return Err(FinalSemanticAnalysisError::InvalidOwner);
            }
            if let CheckedCallArgumentSlotSource::CompactNumericElement { sequence, ordinal } =
                evaluation.source()
            {
                let sequence = resolve_module(modules, sequence.module())?
                    .resolve_expr(sequence)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                let HirExprKind::NumericBracketSequence(sequence) = sequence.kind() else {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                };
                let ordinal = usize::try_from(ordinal)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                if sequence.elements().get(ordinal).is_none() {
                    return Err(FinalSemanticAnalysisError::InvalidOwner);
                }
            }
        }
    }
    Ok(())
}

fn validate_nominal(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    nominal: &CheckedProjectNominal,
) -> Result<(), FinalSemanticAnalysisError> {
    let item = resolve_item(modules, nominal.owner())?;
    if !matches!(
        item.kind(),
        HirItemKind::Struct(_) | HirItemKind::Enum(_) | HirItemKind::TypeAlias(_)
    ) {
        return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
    }
    let declaration = symbols
        .nominal(nominal.declaration())
        .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
    if declaration.owner() != nominal.owner() {
        return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
    }
    Ok(())
}

fn validate_variant(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    variant: &CheckedVariantResolution,
) -> Result<(), FinalSemanticAnalysisError> {
    match variant.owner() {
        CheckedVariantOwner::Project { nominal, .. } => {
            validate_nominal(symbols, modules, nominal)?;
            let declaration = symbols
                .nominal(nominal.declaration())
                .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
            if !matches!(declaration.body(), ProjectNominalBody::Enum { .. }) {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        }
        CheckedVariantOwner::CharacterNominal { .. }
        | CheckedVariantOwner::BuiltinClosed { .. }
        | CheckedVariantOwner::RuntimeBuiltin { .. }
        | CheckedVariantOwner::Option { .. }
        | CheckedVariantOwner::Result { .. } => {}
    }
    if !variant.owner().has_valid_case_rows() || variant.owner().case(variant.ordinal()).is_none() {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }
    Ok(())
}

fn validate_callable(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    callable: &CheckedProjectCallable,
) -> Result<(), FinalSemanticAnalysisError> {
    let item = resolve_item(modules, callable.owner())?;
    let expected_family = match callable.declaration().owner() {
        CallableDeclarationOwner::Function => matches!(item.kind(), HirItemKind::Function(_)),
        CallableDeclarationOwner::ExternCapability => {
            matches!(item.kind(), HirItemKind::ExternCapability(_))
        }
        CallableDeclarationOwner::View => matches!(item.kind(), HirItemKind::View(_)),
        CallableDeclarationOwner::Predicate => matches!(item.kind(), HirItemKind::Predicate(_)),
        CallableDeclarationOwner::Proof => matches!(item.kind(), HirItemKind::Proof(_)),
        CallableDeclarationOwner::TraitRequirement => {
            matches!(item.kind(), HirItemKind::Trait(_))
        }
        CallableDeclarationOwner::TraitImplementation
        | CallableDeclarationOwner::InherentMethod => {
            matches!(item.kind(), HirItemKind::Impl(_))
        }
        CallableDeclarationOwner::Flow => matches!(item.kind(), HirItemKind::Flow(_)),
    };
    let symbol = symbols
        .callable(callable.declaration())
        .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
    if !expected_family
        || symbol.source_item() != callable.owner()
        || symbol.source_snapshot()
            != resolve_module(modules, callable.owner().module())?.snapshot_id()
    {
        return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
    }
    Ok(())
}

fn validate_resolved_callable(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    callable: &ResolvedCallable,
) -> Result<(), FinalSemanticAnalysisError> {
    let ResolvedCallableOrigin::Project { declaration, .. } = callable.origin() else {
        return Ok(());
    };
    let symbol = symbols
        .callable(declaration)
        .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
    let owner = symbol.source_item();
    let item = resolve_item(modules, owner)?;
    if item.is_poisoned()
        || symbol.source_snapshot() != resolve_module(modules, owner.module())?.snapshot_id()
    {
        return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
    }
    Ok(())
}

fn validate_diagnostic_span(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    span: Option<&arcweft_source::SourceSpan>,
) -> Result<(), FinalSemanticAnalysisError> {
    let Some(span) = span else {
        return Ok(());
    };
    modules
        .values()
        .any(|module| module.provenance().source_identity() == span.source())
        .then_some(())
        .ok_or(FinalSemanticAnalysisError::DiagnosticSourceMismatch)
}

fn resolve_module<'a>(
    modules: &'a BTreeMap<HirModuleId, &'a HirModule>,
    owner: HirModuleId,
) -> Result<&'a HirModule, FinalSemanticAnalysisError> {
    modules
        .get(&owner)
        .copied()
        .ok_or(FinalSemanticAnalysisError::InvalidOwner)
}

fn resolve_item<'a>(
    modules: &'a BTreeMap<HirModuleId, &'a HirModule>,
    owner: ItemId,
) -> Result<&'a arcweft_lang_hir::item::HirItem, FinalSemanticAnalysisError> {
    resolve_module(modules, owner.module())?
        .resolve_item(owner)
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)
}
