//! Validation and work collection for one staged semantic generation.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_lang_hir::expr::HirCallCallee;

use crate::{
    callable::UnknownCallKind,
    nominal::{ResolvedTypeRefOutcome, TypeNameResolution, TypeResolutionFailure},
};

use super::type_rules::compact_numeric_element_type;
use super::{
    CallCalleeClassificationFact, CallPoison, CallTargetFact, CallTargetFacts,
    CallableDeclarationOwner, CallableDiagnosticSubject, CallableInstantiation, CaptureId,
    CheckedBinding, CheckedBindingRole, CheckedCallArgumentSlotSource, CheckedEntryReference,
    CheckedExpression, CheckedExpressionResolution, CheckedFunctionExecution, CheckedItem,
    CheckedItemRole, CheckedIteration, CheckedPattern, CheckedPatternResolution,
    CheckedProjectCallable, CheckedProjectItem, CheckedProjectItemOwner, CheckedProjectNominal,
    CheckedSelectResolution, CheckedStatement, CheckedStatementRole, CheckedTraitConformance,
    CheckedValueResolution, CheckedVariantOwner, CheckedVariantResolution,
    DeclarationIdentityFamily, ExprId, FinalSemanticAnalysisError, FinalSemanticAnalysisWork,
    HirExecutableProjectView, HirExprKind, HirIdRef, HirItemKind, HirModule, HirModuleId,
    HirPatternKind, HirStmtKind, ItemId, LocalId, PatternId, PhysicalCandidateArgumentEvaluation,
    PostfixBracketResolution, ProjectNominalBody, ProjectSymbolTable, ResolvedCallable,
    SemanticFactFamily, SignatureOrigin, StmtId, TypeId, TypeKind, TypeResolutionReport,
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

pub(super) fn validate_symbol_generation(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
) -> Result<(), FinalSemanticAnalysisError> {
    if symbols.world().package() != project.package() {
        return Err(FinalSemanticAnalysisError::SymbolGenerationMismatch);
    }
    let project_modules = project.modules().collect::<BTreeMap<_, _>>();
    if project_modules.len() != symbols.modules().len() {
        return Err(FinalSemanticAnalysisError::SymbolGenerationMismatch);
    }
    for (path, module) in project_modules {
        if symbols.source_identity(path) != Some(module.provenance().source_identity()) {
            return Err(FinalSemanticAnalysisError::SymbolGenerationMismatch);
        }
    }
    Ok(())
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

            match calls.get(&owner).and_then(CallTargetFacts::callee) {
                Some(CallCalleeClassificationFact::Value { expression })
                    if expression == *value_receiver =>
                {
                    remove_type_subtree(module, nominal_receiver, &mut accepted)?;
                }
                Some(CallCalleeClassificationFact::AssociatedType { receiver, .. })
                    if receiver == nominal_receiver => {}
                None if !expressions.contains_key(&owner) => {
                    remove_type_subtree(module, nominal_receiver, &mut accepted)?;
                }
                Some(_) | None => return Err(FinalSemanticAnalysisError::CallFactMismatch),
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
    modules: &BTreeMap<HirModuleId, &HirModule>,
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
        modules
            .values()
            .flat_map(|module| module.captures().map(|(id, _)| id)),
        captures,
        SemanticFactFamily::Capture,
    )?;
    require_complete(
        checked_expression_owners(modules, expressions)?.into_iter(),
        expressions,
        SemanticFactFamily::Expression,
    )?;
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

/// Computes the expressions reachable after each bounded postfix ambiguity
/// has one checked interpretation. Both candidate trees remain immutable HIR
/// tooling data, but only the selected tree belongs to the executable semantic
/// inventory. Losing candidate facts must not leak from a probe transaction.
fn checked_expression_owners(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expressions: &BTreeMap<ExprId, CheckedExpression>,
) -> Result<BTreeSet<ExprId>, FinalSemanticAnalysisError> {
    let all = modules
        .values()
        .flat_map(|module| module.expressions().map(|(owner, _)| owner))
        .collect::<BTreeSet<_>>();
    let children = modules
        .values()
        .flat_map(|module| module.expressions().map(|(_, expression)| expression))
        .flat_map(|expression| expression.kind().direct_expression_children())
        .collect::<BTreeSet<_>>();
    let mut pending = all.difference(&children).copied().collect::<Vec<_>>();
    let mut reachable = BTreeSet::new();
    while let Some(owner) = pending.pop() {
        if !reachable.insert(owner) {
            continue;
        }
        let module = resolve_module(modules, owner.module())?;
        let expression = module
            .resolve_expr(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        match expression.kind() {
            HirExprKind::PostfixBracket(postfix) => {
                pending.push(postfix.target());
                let fact =
                    expressions
                        .get(&owner)
                        .ok_or(FinalSemanticAnalysisError::MissingFact {
                            family: SemanticFactFamily::Expression,
                        })?;
                let CheckedExpressionResolution::PostfixBracket(resolution) = fact.resolution()
                else {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                };
                let candidate = resolution.candidate();
                let admissible = matches!(
                    (postfix.candidates(), resolution),
                    (
                        arcweft_lang_hir::dialogue_application::HirPostfixBracketCandidates::Ambiguous {
                            index,
                            ..
                        },
                        PostfixBracketResolution::Index { candidate }
                    ) if index == candidate
                ) || matches!(
                    (postfix.candidates(), resolution),
                    (
                        arcweft_lang_hir::dialogue_application::HirPostfixBracketCandidates::Ambiguous {
                            dialogue,
                            ..
                        },
                        PostfixBracketResolution::Dialogue { candidate }
                    ) if dialogue == candidate
                );
                if !admissible {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                pending.push(candidate);
            }
            kind => pending.extend(kind.direct_expression_children()),
        }
    }
    Ok(reachable)
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
    calls: &BTreeMap<ExprId, CallTargetFacts>,
    type_resolutions: &BTreeMap<TypeId, TypeResolutionReport>,
) -> Result<(), FinalSemanticAnalysisError> {
    let recovered_receivers =
        associated_wrong_arity_receiver_ids(modules, calls, type_resolutions)?;
    for (&owner, ty) in types {
        let node = resolve_module(modules, owner.module())?
            .resolve_type(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if node.is_poisoned() {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        }
        if ty.contains_nominal_poison() && !recovered_receivers.contains(&owner) {
            return Err(FinalSemanticAnalysisError::PoisonedType);
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
    }
    for (&owner, fact) in captures {
        resolve_module(modules, owner.module())?
            .resolve_capture(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if fact.ty().contains_nominal_poison() {
            return Err(FinalSemanticAnalysisError::PoisonedType);
        }
    }
    Ok(())
}

pub(super) fn validate_expressions(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    calls: &BTreeMap<ExprId, CallTargetFacts>,
    type_resolutions: &BTreeMap<TypeId, TypeResolutionReport>,
) -> Result<(), FinalSemanticAnalysisError> {
    for (&owner, fact) in expressions {
        let expression = resolve_module(modules, owner.module())?
            .resolve_expr(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if expression.is_poisoned() {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        }
        let associated_wrong_arity = calls.get(&owner).is_some_and(|call| {
            let HirExprKind::Call(hir_call) = expression.kind() else {
                return false;
            };
            associated_wrong_arity_recovery(call, hir_call, type_resolutions)
        }) || associated_wrong_arity_fallback_receiver(
            owner,
            fact,
            modules,
            calls,
            type_resolutions,
        )?;
        if fact.ty().contains_nominal_poison() && !associated_wrong_arity {
            return Err(FinalSemanticAnalysisError::PoisonedType);
        }
        if !expression_resolution_matches(expression.kind(), fact.resolution())
            && !nominal_fallback_receiver_matches(owner, fact, modules, calls)?
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        validate_expression_resolution(symbols, modules, fact.resolution())?;
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
        if matches!(fact.resolution(), CheckedExpressionResolution::Call)
            != calls.contains_key(&owner)
        {
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
            let accepted = matches!(
                call_fact.callee(),
                Some(CallCalleeClassificationFact::AssociatedType { .. })
            ) || matches!(
                call_fact.callee(),
                Some(CallCalleeClassificationFact::Value { expression }) if expression == owner
            ) && free_path_call_target(call_fact);
            if accepted {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn associated_wrong_arity_fallback_receiver(
    owner: ExprId,
    fact: &CheckedExpression,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    calls: &BTreeMap<ExprId, CallTargetFacts>,
    type_resolutions: &BTreeMap<TypeId, TypeResolutionReport>,
) -> Result<bool, FinalSemanticAnalysisError> {
    if !matches!(fact.resolution(), CheckedExpressionResolution::Structural) {
        return Ok(false);
    }
    for call in calls.values() {
        let expression = resolve_module(modules, call.expression().module())?
            .resolve_expr(call.expression())
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirExprKind::Call(hir_call) = expression.kind() else {
            continue;
        };
        if matches!(
            hir_call.callee(),
            HirCallCallee::UnresolvedDot { value_receiver, .. } if *value_receiver == owner
        ) && associated_wrong_arity_recovery(call, hir_call, type_resolutions)
            && call.result() == Some(fact.ty())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn free_path_call_target(call: &CallTargetFacts) -> bool {
    let is_free = |candidate: &ResolvedCallable| {
        matches!(candidate.instantiation(), CallableInstantiation::None)
    };
    match call.target() {
        CallTargetFact::Selected { selected, .. } => is_free(selected),
        CallTargetFact::Ambiguous { candidates, .. } | CallTargetFact::Rejected { candidates } => {
            !candidates.is_empty() && candidates.iter().all(is_free)
        }
        CallTargetFact::Missing { .. } | CallTargetFact::NonCallable { .. } => false,
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
        | (HirExprKind::Path(_) | HirExprKind::Select(_), CheckedExpressionResolution::Effect(_))
        | (
            HirExprKind::Call(_),
            CheckedExpressionResolution::Call
            | CheckedExpressionResolution::ViewCall(_)
            | CheckedExpressionResolution::StyleValue(_),
        )
        | (
            HirExprKind::Path(_),
            CheckedExpressionResolution::ViewCallee(_)
            | CheckedExpressionResolution::StyleCallee(_),
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
        (kind, CheckedExpressionResolution::Structural) => structural_resolution_matches(kind),
        _ => false,
    }
}

const fn structural_resolution_matches(kind: &HirExprKind) -> bool {
    matches!(
        kind,
        HirExprKind::Unit
            | HirExprKind::LifetimePath(_)
            | HirExprKind::Placeholder(_)
            | HirExprKind::Tuple(_)
            | HirExprKind::BracketSequence(_)
            | HirExprKind::NumericBracketSequence(_)
            | HirExprKind::ArrayRepeat(_)
            | HirExprKind::Index(_)
            | HirExprKind::Pipe(_)
            | HirExprKind::Try(_)
            | HirExprKind::Await(_)
            | HirExprKind::Thread(_)
            | HirExprKind::Choice(_)
            | HirExprKind::Range(_)
            | HirExprKind::Binary(_)
            | HirExprKind::Borrow(_)
            | HirExprKind::Dereference(_)
            | HirExprKind::Closure(_)
            | HirExprKind::Unary(_)
            | HirExprKind::Block(_)
            | HirExprKind::ComputationBlock(_)
            | HirExprKind::NamedBlock(_)
            | HirExprKind::If(_)
            | HirExprKind::IfLet(_)
            | HirExprKind::Match(_)
            | HirExprKind::ForSynthetic(_)
    )
}

fn validate_expression_resolution(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    resolution: &CheckedExpressionResolution,
) -> Result<(), FinalSemanticAnalysisError> {
    match resolution {
        CheckedExpressionResolution::Value(value) => validate_value(symbols, modules, value),
        CheckedExpressionResolution::Select(select) => match select {
            CheckedSelectResolution::Field { nominal, .. }
            | CheckedSelectResolution::RecordElement { nominal, .. } => nominal
                .as_ref()
                .map(|nominal| validate_nominal(symbols, modules, nominal))
                .transpose()
                .map(|_| ()),
            CheckedSelectResolution::DialogueView { projection, name } => (projection.field()
                == name.as_str())
            .then_some(())
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily),
            CheckedSelectResolution::TupleElement { .. } => Ok(()),
        },
        CheckedExpressionResolution::Nominal(nominal) => {
            validate_nominal(symbols, modules, nominal)
        }
        CheckedExpressionResolution::Variant(variant) => {
            validate_variant(symbols, modules, variant)
        }
        CheckedExpressionResolution::DialogueApplication { character, .. } => {
            let item = resolve_item(modules, *character)?;
            matches!(item.kind(), HirItemKind::Character(_))
                .then_some(())
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
        }
        CheckedExpressionResolution::PostfixBracket(_)
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
        CheckedValueResolution::ProjectCallable(callable) => {
            validate_callable(symbols, modules, callable)
        }
        CheckedValueResolution::ProjectItem(item) => validate_project_item(symbols, modules, item),
        CheckedValueResolution::Entry(entry) => validate_entry_reference(modules, entry),
        CheckedValueResolution::Registered(_) | CheckedValueResolution::Constant(_) => Ok(()),
    }
}

fn validate_entry_reference(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    entry: &CheckedEntryReference,
) -> Result<(), FinalSemanticAnalysisError> {
    let item = resolve_item(modules, entry.owner())?;
    let HirItemKind::Entry(declaration) = item.kind() else {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    };
    let Some(HirIdRef::Absolute(public_id)) =
        declaration.id().value().and_then(|id| id.as_resolved())
    else {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    };
    (public_id.as_str() == entry.public_id().as_str())
        .then_some(())
        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
}

pub(super) fn validate_patterns(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
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
            (HirPatternKind::Record { .. }, CheckedPatternResolution::Nominal(nominal)) => {
                validate_nominal(symbols, modules, nominal).is_ok()
            }
            (
                HirPatternKind::Binding(_)
                | HirPatternKind::MutableBinding(_)
                | HirPatternKind::Discard
                | HirPatternKind::Tuple { .. }
                | HirPatternKind::BracketSequence { .. }
                | HirPatternKind::WholeBinding { .. }
                | HirPatternKind::Or { .. }
                | HirPatternKind::TypedBinding { .. },
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

fn validate_project_item(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    item: &CheckedProjectItem,
) -> Result<(), FinalSemanticAnalysisError> {
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
    statements: &BTreeMap<StmtId, CheckedStatement>,
) -> Result<(), FinalSemanticAnalysisError> {
    for (&owner, fact) in statements {
        let statement = resolve_module(modules, owner.module())?
            .resolve_stmt(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if statement.is_poisoned() {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        }
        let compatible = match fact.role() {
            CheckedStatementRole::Assertion(_) => {
                matches!(statement.kind(), HirStmtKind::Assertion { .. })
            }
            CheckedStatementRole::Iteration(_) => {
                matches!(statement.kind(), HirStmtKind::For(_))
            }
            CheckedStatementRole::Yield => matches!(statement.kind(), HirStmtKind::Yield { .. }),
            CheckedStatementRole::UnsafeAudit => {
                matches!(statement.kind(), HirStmtKind::UnsafeLifetime { .. })
            }
            CheckedStatementRole::Suspension => matches!(
                statement.kind(),
                HirStmtKind::Wait { .. } | HirStmtKind::AwaitWith(_) | HirStmtKind::LetAwait { .. }
            ),
            CheckedStatementRole::Ordinary => !matches!(
                statement.kind(),
                HirStmtKind::Assertion { .. }
                    | HirStmtKind::For(_)
                    | HirStmtKind::Yield { .. }
                    | HirStmtKind::UnsafeLifetime { .. }
                    | HirStmtKind::Wait { .. }
                    | HirStmtKind::AwaitWith(_)
                    | HirStmtKind::LetAwait { .. }
                    | HirStmtKind::Error
            ),
        };
        if !compatible {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        if let CheckedStatementRole::Iteration(iteration) = fact.role() {
            validate_iteration(modules, iteration)?;
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
    type_resolutions: &BTreeMap<TypeId, TypeResolutionReport>,
) -> Result<(), FinalSemanticAnalysisError> {
    for (&owner, call) in calls {
        let expression = resolve_module(modules, owner.module())?
            .resolve_expr(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirExprKind::Call(hir_call) = expression.kind() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let associated_wrong_arity = validate_call_acceptance(call, hir_call, type_resolutions)?;
        validate_call_callee(modules, call)?;
        validate_call_result(expressions, owner, call)?;
        validate_call_argument_slots(modules, expressions, call)?;
        validate_call_target(symbols, modules, call, associated_wrong_arity)?;
        validate_call_context(symbols, modules, call)?;
    }
    Ok(())
}

fn validate_call_acceptance(
    call: &CallTargetFacts,
    hir_call: &arcweft_lang_hir::expr::HirCallExpr,
    type_resolutions: &BTreeMap<TypeId, TypeResolutionReport>,
) -> Result<bool, FinalSemanticAnalysisError> {
    let associated_wrong_arity = associated_wrong_arity_recovery(call, hir_call, type_resolutions);
    let selected = matches!(call.target(), CallTargetFact::Selected { .. });
    let recovery = matches!(
        call.target(),
        CallTargetFact::Ambiguous { candidates, .. } if candidates.len() >= 2
    ) || matches!(
        call.target(),
        CallTargetFact::Rejected { candidates } if !candidates.is_empty()
    );
    let accepted_poison = (selected && call.poison() == CallPoison::Clean)
        || (recovery && call.poison() == CallPoison::Rejected)
        || associated_wrong_arity;
    let accepted_arguments = (!selected && !associated_wrong_arity)
        || call.arguments().iter().all(|argument| {
            argument.poison() == CallPoison::Clean
                && argument
                    .slots()
                    .iter()
                    .all(|slot| slot.poison() == CallPoison::Clean)
        });
    if !accepted_poison
        || call.arguments().len() != hir_call.arguments().len()
        || !accepted_arguments
    {
        return Err(FinalSemanticAnalysisError::UnacceptedCall);
    }
    Ok(associated_wrong_arity)
}

fn validate_call_callee(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    call: &CallTargetFacts,
) -> Result<(), FinalSemanticAnalysisError> {
    match call.callee() {
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
    if call.result() != Some(checked.ty())
        || !call.effects().is_known()
        || call.effects().concrete() != checked.effects()
    {
        return Err(FinalSemanticAnalysisError::CallFactMismatch);
    }
    matches!(
        call.effects().tail(),
        crate::effect_row::EffectRowTail::Closed
    )
    .then_some(())
    .ok_or(FinalSemanticAnalysisError::OpenEffectRow)
}

fn validate_call_argument_slots(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    call: &CallTargetFacts,
) -> Result<(), FinalSemanticAnalysisError> {
    for argument in call.arguments() {
        for slot in argument.slots() {
            let inferred_matches_source = match slot.source() {
                CheckedCallArgumentSlotSource::Expression(expression) => expressions
                    .get(&expression)
                    .is_some_and(|checked| slot.inferred() == Some(checked.ty())),
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
                    slot.inferred() == Some(&inferred)
                }
            };
            if !inferred_matches_source
                || slot
                    .expected()
                    .is_some_and(TypeKind::contains_nominal_poison)
            {
                return Err(FinalSemanticAnalysisError::CallFactMismatch);
            }
        }
    }
    Ok(())
}

fn validate_call_target(
    symbols: &ProjectSymbolTable,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    call: &CallTargetFacts,
    associated_wrong_arity: bool,
) -> Result<(), FinalSemanticAnalysisError> {
    match call.target() {
        CallTargetFact::Selected {
            selected,
            considered,
        } => {
            validate_resolved_callable(symbols, modules, selected)?;
            for candidate in considered.iter() {
                validate_resolved_callable(symbols, modules, candidate)?;
            }
        }
        CallTargetFact::Ambiguous { considered, .. } => {
            for candidate in considered.iter() {
                validate_resolved_callable(symbols, modules, candidate)?;
            }
        }
        CallTargetFact::Rejected { candidates } => {
            for candidate in candidates.iter() {
                validate_resolved_callable(symbols, modules, candidate)?;
            }
        }
        CallTargetFact::Missing { .. } if associated_wrong_arity => {}
        CallTargetFact::NonCallable { .. } | CallTargetFact::Missing { .. } => {
            return Err(FinalSemanticAnalysisError::UnacceptedCall);
        }
    }
    Ok(())
}

fn associated_wrong_arity_receiver_ids(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    calls: &BTreeMap<ExprId, CallTargetFacts>,
    type_resolutions: &BTreeMap<TypeId, TypeResolutionReport>,
) -> Result<BTreeSet<TypeId>, FinalSemanticAnalysisError> {
    let mut receivers = BTreeSet::new();
    for call in calls.values() {
        let expression = resolve_module(modules, call.expression().module())?
            .resolve_expr(call.expression())
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if let HirExprKind::Call(hir_call) = expression.kind()
            && associated_wrong_arity_recovery(call, hir_call, type_resolutions)
            && let Some(CallCalleeClassificationFact::AssociatedType { receiver, .. }) =
                call.callee()
        {
            receivers.insert(receiver);
        }
    }
    Ok(receivers)
}

fn associated_wrong_arity_recovery(
    call: &CallTargetFacts,
    hir_call: &arcweft_lang_hir::expr::HirCallExpr,
    type_resolutions: &BTreeMap<TypeId, TypeResolutionReport>,
) -> bool {
    let Some((hir_receiver, hir_separator, hir_member)) = hir_call.callee().associated_parts()
    else {
        return false;
    };
    let Some(hir_receiver) = hir_receiver.type_id() else {
        return false;
    };
    let Some(CallCalleeClassificationFact::AssociatedType {
        receiver,
        separator,
    }) = call.callee()
    else {
        return false;
    };
    let Some(report) = type_resolutions.get(&receiver) else {
        return false;
    };
    let root_wrong_arity = matches!(report.outcome(), ResolvedTypeRefOutcome::Poisoned(_))
        && report.outcome().product().root() == receiver
        && report.outcome().product().nodes().iter().any(|node| {
            node.node() == receiver
                && matches!(
                    node.outcome(),
                    TypeNameResolution::Failed(TypeResolutionFailure::WrongArity { .. })
                )
        });
    let accounting = call.accounting();
    let Ok(argument_count) = u64::try_from(call.arguments().len()) else {
        return false;
    };
    root_wrong_arity
        && hir_member.resolved().is_some()
        && hir_receiver == receiver
        && *hir_separator == separator
        && matches!(
            call.target(),
            CallTargetFact::Missing {
                kind: UnknownCallKind::AssociatedType
            }
        )
        && call.poison() == CallPoison::Recovered
        && call.result().is_some_and(TypeKind::contains_nominal_poison)
        && call.current_group() == crate::callable::CallableGroupIndex::ZERO
        && call.next_group().is_none()
        && call.function_value_type().is_none()
        && call.effects().concrete().is_empty()
        && accounting.logical_argument_checks() == argument_count
        && accounting.resolver_invocations() == 0
        && accounting.candidate_argument_probes() == 0
        && accounting.selected_replay_argument_visits() == 0
        && accounting.retained_argument_fact_publications() == argument_count
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
        CheckedVariantOwner::Project(nominal) => {
            validate_nominal(symbols, modules, nominal)?;
            let declaration = symbols
                .nominal(nominal.declaration())
                .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
            let ProjectNominalBody::Enum { variants } = declaration.body() else {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            };
            let selected = usize::try_from(variant.ordinal())
                .ok()
                .and_then(|ordinal| variants.get(ordinal))
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            if selected.name().as_str() != variant.name().as_str() {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        }
        CheckedVariantOwner::CharacterNominal { cases, .. } => {
            if usize::try_from(variant.ordinal())
                .ok()
                .and_then(|ordinal| cases.get(ordinal))
                .is_none_or(|name| name != variant.name().as_str())
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        }
        CheckedVariantOwner::BuiltinClosed { cases, .. } => {
            if usize::try_from(variant.ordinal())
                .ok()
                .and_then(|ordinal| cases.get(ordinal))
                .is_none_or(|case| case.name() != variant.name().as_str())
                || cases.iter().any(|case| {
                    case.payload()
                        .is_some_and(TypeKind::contains_nominal_poison)
                })
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        }
        CheckedVariantOwner::Option { item } => {
            if item.contains_nominal_poison()
                || !matches!(
                    (variant.ordinal(), variant.name().as_str()),
                    (0, "Some") | (1, "None")
                )
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        }
        CheckedVariantOwner::Result { ok, error } => {
            if ok.contains_nominal_poison()
                || error.contains_nominal_poison()
                || !matches!(
                    (variant.ordinal(), variant.name().as_str()),
                    (0, "Ok") | (1, "Err")
                )
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        }
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
    let SignatureOrigin::Project { declaration, .. } = callable.origin() else {
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
