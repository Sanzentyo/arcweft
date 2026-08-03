//! Predicate-specific re-derivation for final item publication.

use arcweft_lang_syntax::attachment::{AttachedPredicateBody, AttachedPredicateDeclaration};
use arcweft_lang_syntax::incremental::ParsedSource;

use crate::expr::{HirExpr, HirPoisonState};
use crate::identity::{ItemId, SyntheticKey, SyntheticOwner, SyntheticRole};
use crate::item::{
    HirDeclarationMemberArena, HirItem, HirItemIssue, HirItemKind, HirPredicate, HirPredicateBody,
};
use crate::leaf::{HirPathRoot, HirPathSegment};
use crate::scope::HirPatternBindingPolicy;
use crate::scope::HirScopeKind;
use crate::slot::{HirOrigin, SlotSnapshot};
use crate::source_index::block_projection::{
    BlockValidationArenas, ItemValueBlockRetained, missing_scope_tail_matches,
    predicate_block_matches, source_expression_matches,
};
use crate::source_index::{HirSourceIndex, HirSourceSite};
use crate::type_ref::HirTypeKind;

use super::callable::{
    CallableScopeIds, CallableScopeSource, ParameterSurfacePolicy, contract_scopes_match,
    contracts_match, direct_children_are_exact, item_body_scope_matches, parameters_match,
    postcondition_result_matches,
};
use super::{
    ItemValidationArenas, generic_issue, generic_parameters_match, item_prefix_matches, item_state,
    name_issue, prefix_issue, required_name_matches, type_tree_is_unallocated, where_issue,
    where_predicates_match,
};

pub(super) fn payload_matches(
    source_index: &HirSourceIndex,
    owner: ItemId,
    attached: &AttachedPredicateDeclaration,
    item: &HirItem,
    members: Option<&HirDeclarationMemberArena>,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let HirItemKind::Predicate(predicate) = item.kind() else {
        return false;
    };
    if members.is_some()
        || !item.members().is_empty()
        || !item_prefix_matches(item, attached.prefix(), slots)
        || !required_name_matches(predicate.name(), attached.name())
        || !generic_parameters_match(predicate.generic_parameters(), attached.generics(), slots)
        || !where_predicates_match(
            predicate.where_predicates(),
            attached.where_clauses(),
            slots,
        )
        || !contract_scopes_match(
            owner,
            CallableScopeSource {
                syntax: attached.syntax().id(),
                item: &attached.syntax().source_span(),
                requires: &attached.requires_scope_source_span(),
                ensures: &attached.ensures_scope_source_span(),
            },
            CallableScopeIds {
                item: item.scope(),
                callable: predicate.callable_scope(),
                requires: predicate.requires_scope(),
                ensures: predicate.ensures_scope(),
            },
            parsed,
            slots,
            arenas,
        )
        || !predicate_bool_return_matches(
            source_index,
            owner,
            attached,
            predicate,
            parsed,
            slots,
            arenas,
        )
    {
        return false;
    }

    let block_arenas = block_arenas(arenas);
    let Some(parameter_state) = parameters_match(
        attached.parameter_group().parameters().iter(),
        attached.parameter_group().has_recovery(),
        ParameterSurfacePolicy::FixedOnly,
        predicate.parameters(),
        predicate.callable_scope(),
        HirPatternBindingPolicy::PredicateParameter,
        slots,
        arenas,
        &block_arenas,
    ) else {
        return false;
    };
    let Some(contract_recovery) = contracts_match(
        attached.contracts(),
        predicate.requires(),
        predicate.ensures(),
        predicate.requires_scope(),
        predicate.ensures_scope(),
        slots,
        arenas,
    ) else {
        return false;
    };
    if !postcondition_result_matches(
        attached.postcondition_result_source_span(),
        predicate.ensures_scope(),
        Some(predicate.return_type()),
        parsed,
        slots,
        arenas,
    ) || !attached
        .authored_return()
        .is_none_or(|authored| type_tree_is_unallocated(authored.ty(), slots))
    {
        return false;
    }

    let Some(body_issue) = predicate_body_matches(
        owner,
        attached,
        predicate,
        parsed,
        slots,
        arenas,
        &block_arenas,
    ) else {
        return false;
    };
    let expected_state = item_state(
        prefix_issue(attached.prefix(), item.prefix(), slots)
            .or_else(|| name_issue(attached.name()))
            .or_else(|| {
                generic_issue(attached.generics(), predicate.generic_parameters(), slots)
                    .is_some()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                parameter_state
                    .missing_type
                    .then_some(HirItemIssue::MissingType)
            })
            .or_else(|| {
                parameter_state
                    .recovered
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                where_issue(
                    attached.where_clauses(),
                    predicate.where_predicates(),
                    slots,
                )
                .is_some()
                .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| contract_recovery.then_some(HirItemIssue::Recovery))
            .or_else(|| {
                attached
                    .authored_return()
                    .is_some()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or(body_issue)
            .or_else(|| {
                (!attached.trailing_recovery().is_empty()).then_some(HirItemIssue::Recovery)
            }),
    );
    item.state() == &expected_state
}

fn predicate_bool_return_matches(
    source_index: &HirSourceIndex,
    owner: ItemId,
    attached: &AttachedPredicateDeclaration,
    predicate: &HirPredicate,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let return_type = predicate.return_type();
    let Ok(key) = SyntheticKey::try_new(
        SyntheticOwner::Item(owner),
        SyntheticRole::PredicateBoolReturn,
        0,
    ) else {
        return false;
    };
    let Ok(site) = HirSourceSite::from_attached_span(
        parsed.document(),
        &attached.parameter_group().end_source_span(),
    ) else {
        return false;
    };
    let Ok(metadata) = slots.resolve_prepared(return_type) else {
        return false;
    };
    let Ok(payload) = arenas.types.resolve_prepared(slots, return_type) else {
        return false;
    };
    matches!(metadata.origin(), HirOrigin::Synthetic(actual) if *actual == key)
        && metadata.source_site() == &site
        && !source_index
            .syntax_owners
            .contains_key(&SyntheticOwner::Type(return_type))
        && payload.scope() == predicate.callable_scope()
        && payload.state() == &HirPoisonState::Clean
        && matches!(
            payload.kind(),
            HirTypeKind::Path(path)
                if path.root() == HirPathRoot::ImplicitCrate
                    && matches!(path.segments(), [HirPathSegment::Identifier(name)] if name.as_str() == "Bool")
        )
}

fn predicate_body_matches(
    owner: ItemId,
    attached: &AttachedPredicateDeclaration,
    predicate: &HirPredicate,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
    block_arenas: &BlockValidationArenas<'_>,
) -> Option<Option<HirItemIssue>> {
    let body_has_recovery = attached.body().has_recovery();
    let body_syntax = attached.body().syntax();
    let callable = arenas
        .scopes
        .resolve_prepared(slots, predicate.callable_scope())
        .ok()?;
    let item_body_scope = predicate.body().scope();
    if !item_body_scope_matches(
        owner,
        predicate.callable_scope(),
        item_body_scope,
        HirScopeKind::Predicate,
        body_syntax.id(),
        body_syntax.source_span(),
        slots,
        arenas,
    ) {
        return None;
    }
    if !direct_children_are_exact(
        predicate.callable_scope(),
        predicate.requires_scope(),
        predicate.ensures_scope(),
        item_body_scope,
        callable,
        slots,
        arenas,
    ) {
        return None;
    }
    match (attached.body(), predicate.body()) {
        (
            AttachedPredicateBody::Expression {
                expression: attached,
                ..
            },
            HirPredicateBody::Expression {
                scope,
                expression: retained,
            },
        ) => {
            if *scope != item_body_scope {
                return None;
            }
            if !source_expression_matches(
                slots,
                arenas.expressions,
                *retained,
                attached,
                item_body_scope,
            ) {
                return None;
            }
            let recovered = body_has_recovery
                || arenas
                    .expressions
                    .resolve_prepared(slots, *retained)
                    .is_ok_and(HirExpr::is_poisoned);
            Some(recovered.then_some(HirItemIssue::Recovery))
        }
        (
            AttachedPredicateBody::Block { block, .. },
            HirPredicateBody::Block {
                scope,
                statements,
                tail,
            },
        ) => {
            if *scope != item_body_scope {
                return None;
            }
            let recovered = predicate_block_matches(
                parsed,
                slots,
                block_arenas,
                ItemValueBlockRetained {
                    owner,
                    callable_scope: predicate.callable_scope(),
                    scope: *scope,
                    statements,
                    tail: *tail,
                },
                body_syntax.id(),
                body_syntax.source_span(),
                block,
            )?;
            Some((body_has_recovery || recovered).then_some(HirItemIssue::Recovery))
        }
        (
            AttachedPredicateBody::Missing {
                missing: attached, ..
            },
            HirPredicateBody::Error {
                scope,
                expression: retained,
            },
        ) => {
            if *scope != item_body_scope
                || !missing_scope_tail_matches(
                    parsed,
                    slots,
                    arenas.expressions,
                    item_body_scope,
                    *retained,
                    attached.source_span(),
                )
            {
                return None;
            }
            Some(Some(HirItemIssue::MissingBody))
        }
        _ => None,
    }
}

fn block_arenas<'arena>(arenas: &ItemValidationArenas<'arena>) -> BlockValidationArenas<'arena> {
    BlockValidationArenas {
        expressions: arenas.expressions,
        statements: arenas.statements,
        scopes: arenas.scopes,
        locals: arenas.locals,
        patterns: arenas.patterns,
    }
}
