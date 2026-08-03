//! Ordinary-function re-derivation for final item publication.

use arcweft_lang_syntax::attachment::{
    AttachedFunctionBody, AttachedFunctionDeclaration, AttachedTypeFamily,
};
use arcweft_lang_syntax::grammar::SyntaxKind;
use arcweft_lang_syntax::incremental::ParsedSource;

use crate::identity::ItemId;
use crate::item::{
    HirDeclarationMemberArena, HirFunctionBody, HirFunctionItem, HirItem, HirItemIssue, HirItemKind,
};
use crate::scope::{HirPatternBindingPolicy, HirScopeKind};
use crate::slot::SlotSnapshot;
use crate::source_index::block_projection::{
    BlockValidationArenas, ItemValueBlockRetained, function_block_matches,
    missing_scope_tail_matches,
};

use super::callable::{
    CallableScopeIds, CallableScopeSource, contract_scopes_match, contracts_match,
    direct_children_are_exact, direct_contract_children_are_exact, function_parameter_groups_match,
    item_body_scope_matches, postcondition_result_matches,
};
use super::{
    ItemValidationArenas, generic_issue, generic_parameters_match, item_prefix_matches, item_state,
    name_issue, prefix_issue, required_name_matches, slot_is_poisoned, type_owner_matches,
    where_issue, where_predicates_match,
};

pub(super) fn payload_matches(
    owner: ItemId,
    attached: &AttachedFunctionDeclaration,
    item: &HirItem,
    members: Option<&HirDeclarationMemberArena>,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let HirItemKind::Function(function) = item.kind() else {
        return false;
    };
    if members.is_some()
        || !item.members().is_empty()
        || !item_prefix_matches(item, attached.prefix(), slots)
        || !required_name_matches(function.name(), attached.name())
        || !generic_parameters_match(function.generic_parameters(), attached.generics(), slots)
        || !where_predicates_match(function.where_predicates(), attached.where_clauses(), slots)
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
                callable: function.callable_scope(),
                requires: function.requires_scope(),
                ensures: function.ensures_scope(),
            },
            parsed,
            slots,
            arenas,
        )
    {
        return false;
    }

    let block_arenas = block_arenas(arenas);
    let Some(mut parameter_state) = function_parameter_groups_match(
        attached.parameter_groups(),
        function.parameter_groups(),
        function.callable_scope(),
        HirPatternBindingPolicy::CallableParameter,
        slots,
        arenas,
        &block_arenas,
    ) else {
        return false;
    };
    let Some(contract_recovery) = contracts_match(
        attached.contracts(),
        function.requires(),
        function.ensures(),
        function.requires_scope(),
        function.ensures_scope(),
        slots,
        arenas,
    ) else {
        return false;
    };
    let Some((return_missing_type, return_recovery)) =
        function_return_matches(attached, function, slots, arenas)
    else {
        return false;
    };
    parameter_state.recovered |= attached.has_parameter_shape_recovery();
    if !postcondition_result_matches(
        attached.postcondition_result_source_span(),
        function.ensures_scope(),
        function.return_type(),
        parsed,
        slots,
        arenas,
    ) {
        return false;
    }

    let Some(body_match) = function_body_matches(
        owner,
        attached,
        function,
        parsed,
        slots,
        arenas,
        &block_arenas,
    ) else {
        return false;
    };
    let expected_state = item_state(primary_function_issue(
        attached,
        function,
        item,
        slots,
        FunctionRecoveryState {
            parameter: typed_header_issue(parameter_state.missing_type, parameter_state.recovered),
            authored_return: typed_header_issue(return_missing_type, return_recovery),
            contracts: contract_recovery.then_some(HirItemIssue::Recovery),
            body: body_match.issue,
        },
    ));
    item.state() == &expected_state
}

#[derive(Clone, Copy)]
struct FunctionRecoveryState {
    parameter: Option<HirItemIssue>,
    authored_return: Option<HirItemIssue>,
    contracts: Option<HirItemIssue>,
    body: Option<HirItemIssue>,
}

const fn typed_header_issue(missing: bool, recovered: bool) -> Option<HirItemIssue> {
    if missing {
        Some(HirItemIssue::MissingType)
    } else if recovered {
        Some(HirItemIssue::MalformedHeader)
    } else {
        None
    }
}

fn primary_function_issue(
    attached: &AttachedFunctionDeclaration,
    function: &HirFunctionItem,
    item: &HirItem,
    slots: &SlotSnapshot,
    recovery: FunctionRecoveryState,
) -> Option<HirItemIssue> {
    prefix_issue(attached.prefix(), item.prefix(), slots)
        .or_else(|| name_issue(attached.name()))
        .or_else(|| {
            generic_issue(attached.generics(), function.generic_parameters(), slots)
                .is_some()
                .then_some(HirItemIssue::MalformedHeader)
        })
        .or(recovery.parameter)
        .or(recovery.authored_return)
        .or_else(|| {
            where_issue(attached.where_clauses(), function.where_predicates(), slots)
                .is_some()
                .then_some(HirItemIssue::MalformedHeader)
        })
        .or(recovery.contracts)
        .or(recovery.body)
        .or_else(|| (!attached.trailing_recovery().is_empty()).then_some(HirItemIssue::Recovery))
}

fn function_return_matches(
    attached: &AttachedFunctionDeclaration,
    function: &HirFunctionItem,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<(bool, bool)> {
    match (attached.authored_return(), function.return_type()) {
        (None, None) => Some((false, false)),
        (Some(attached), Some(retained)) => {
            if !type_owner_matches(retained, attached.ty(), slots)
                || !arenas
                    .types
                    .resolve_prepared(slots, retained)
                    .is_ok_and(|payload| payload.scope() == function.callable_scope())
            {
                return None;
            }
            let recovered = attached.has_recovery() || slot_is_poisoned(slots, retained);
            let missing = recovered
                && attached.ty().family() == AttachedTypeFamily::Recovery
                && attached.ty().syntax().kind() == SyntaxKind::MissingType;
            Some((missing, recovered))
        }
        _ => None,
    }
}

fn function_body_matches(
    owner: ItemId,
    attached: &AttachedFunctionDeclaration,
    function: &HirFunctionItem,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
    block_arenas: &BlockValidationArenas<'_>,
) -> Option<FunctionBodyMatch> {
    let callable = arenas
        .scopes
        .resolve_prepared(slots, function.callable_scope())
        .ok()?;
    match (attached.body(), function.body()) {
        (
            AttachedFunctionBody::Block { block, .. },
            HirFunctionBody::Block {
                scope,
                statements,
                tail,
            },
        ) => {
            if !item_body_scope_matches(
                owner,
                function.callable_scope(),
                *scope,
                HirScopeKind::Block,
                attached.body().syntax().id(),
                attached.body().syntax().source_span(),
                slots,
                arenas,
            ) || !direct_children_are_exact(
                function.callable_scope(),
                function.requires_scope(),
                function.ensures_scope(),
                *scope,
                callable,
                slots,
                arenas,
            ) {
                return None;
            }
            let recovered = function_block_matches(
                parsed,
                slots,
                block_arenas,
                ItemValueBlockRetained {
                    owner,
                    callable_scope: function.callable_scope(),
                    scope: *scope,
                    statements,
                    tail: *tail,
                },
                attached.body().syntax().id(),
                attached.body().syntax().source_span(),
                block,
            )?;
            Some(FunctionBodyMatch {
                issue: (attached.body().has_recovery() || recovered)
                    .then_some(HirItemIssue::Recovery),
            })
        }
        (
            AttachedFunctionBody::Missing {
                missing: attached, ..
            },
            HirFunctionBody::Error(retained),
        ) => {
            if !direct_contract_children_are_exact(
                function.callable_scope(),
                function.requires_scope(),
                function.ensures_scope(),
                callable,
                slots,
                arenas,
            ) || !missing_scope_tail_matches(
                parsed,
                slots,
                arenas.expressions,
                function.callable_scope(),
                *retained,
                attached.source_span(),
            ) {
                return None;
            }
            Some(FunctionBodyMatch {
                issue: Some(HirItemIssue::MissingBody),
            })
        }
        _ => None,
    }
}

struct FunctionBodyMatch {
    issue: Option<HirItemIssue>,
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
