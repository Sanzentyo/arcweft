//! Proof-specific re-derivation for final item publication.

use arcweft_lang_syntax::attachment::AttachedDeclarationPublicId;
use arcweft_lang_syntax::attachment::{AttachedProofBody, AttachedProofDeclaration};
use arcweft_lang_syntax::grammar::SyntaxKind;
use arcweft_lang_syntax::incremental::ParsedSource;

use crate::expr::{HirExpr, HirPoisonState};
use crate::identity::{ItemId, SyntheticKey, SyntheticOwner, SyntheticRole};
use crate::item::{
    HirDeclarationMemberArena, HirItem, HirItemIssue, HirItemKind, HirProof, HirProofBody,
};
use crate::scope::HirPatternBindingPolicy;
use crate::scope::HirScopeKind;
use crate::slot::{HirOrigin, SlotSnapshot};
use crate::source_index::block_projection::{
    BlockValidationArenas, ItemValueBlockRetained, missing_scope_tail_matches, proof_block_matches,
    source_expression_matches,
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
    name_issue, prefix_issue, required_name_matches, type_owner_matches, where_issue,
    where_predicates_match,
};

pub(super) fn payload_matches(
    source_index: &HirSourceIndex,
    owner: ItemId,
    attached: &AttachedProofDeclaration,
    item: &HirItem,
    members: Option<&HirDeclarationMemberArena>,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let HirItemKind::Proof(proof) = item.kind() else {
        return false;
    };
    if members.is_some()
        || !item.members().is_empty()
        || !item_prefix_matches(item, attached.prefix(), slots)
        || !proof_public_id_matches(proof.public_id(), attached.public_id())
        || !required_name_matches(proof.name(), attached.name())
        || !generic_parameters_match(proof.generic_parameters(), attached.generics(), slots)
        || !where_predicates_match(proof.where_predicates(), attached.where_clauses(), slots)
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
                callable: proof.callable_scope(),
                requires: proof.requires_scope(),
                ensures: proof.ensures_scope(),
            },
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
        proof.parameters(),
        proof.callable_scope(),
        HirPatternBindingPolicy::ProofParameter,
        slots,
        arenas,
        &block_arenas,
    ) else {
        return false;
    };
    let Some(return_state) =
        return_matches(source_index, owner, attached, proof, parsed, slots, arenas)
    else {
        return false;
    };
    let Some(contract_recovery) = contracts_match(
        attached.contracts(),
        proof.requires(),
        proof.ensures(),
        proof.requires_scope(),
        proof.ensures_scope(),
        slots,
        arenas,
    ) else {
        return false;
    };
    if !postcondition_result_matches(
        attached.postcondition_result_source_span(),
        proof.ensures_scope(),
        Some(proof.return_type()),
        parsed,
        slots,
        arenas,
    ) {
        return false;
    }

    let Some(body_issue) = proof_body_matches(
        attached,
        return_state.is_unit,
        ProofBodyValidation {
            owner,
            proof,
            parsed,
            slots,
            arenas,
            block_arenas: &block_arenas,
        },
    ) else {
        return false;
    };
    let expected_state = item_state(
        prefix_issue(attached.prefix(), item.prefix(), slots)
            .or_else(|| name_issue(attached.name()))
            .or_else(|| {
                matches!(
                    attached.public_id(),
                    AttachedDeclarationPublicId::Recovered { .. }
                )
                .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                generic_issue(attached.generics(), proof.generic_parameters(), slots)
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
                return_state
                    .missing_type
                    .then_some(HirItemIssue::MissingType)
            })
            .or_else(|| {
                return_state
                    .recovered
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                where_issue(attached.where_clauses(), proof.where_predicates(), slots)
                    .is_some()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| contract_recovery.then_some(HirItemIssue::Recovery))
            .or(body_issue)
            .or_else(|| {
                (!attached.trailing_recovery().is_empty()).then_some(HirItemIssue::Recovery)
            }),
    );
    item.state() == &expected_state
}

fn proof_public_id_matches(
    retained: Option<&arcweft_id::PublicId>,
    attached: &AttachedDeclarationPublicId,
) -> bool {
    match (retained, attached) {
        (None, AttachedDeclarationPublicId::Derived)
        | (None, AttachedDeclarationPublicId::Recovered { .. }) => true,
        (Some(retained), AttachedDeclarationPublicId::Explicit { value, .. }) => retained == value,
        _ => false,
    }
}

struct ReturnState {
    missing_type: bool,
    recovered: bool,
    is_unit: bool,
}

fn return_matches(
    source_index: &HirSourceIndex,
    owner: ItemId,
    attached: &AttachedProofDeclaration,
    proof: &HirProof,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<ReturnState> {
    let retained = proof.return_type();
    let payload = arenas.types.resolve_prepared(slots, retained).ok()?;
    if payload.scope() != proof.callable_scope() {
        return None;
    }
    match attached.authored_return() {
        Some(authored) => {
            if !type_owner_matches(retained, authored.ty(), slots) {
                return None;
            }
            Some(ReturnState {
                missing_type: payload.is_poisoned()
                    && authored.ty().syntax().kind() == SyntaxKind::MissingType,
                recovered: payload.is_poisoned(),
                is_unit: payload.kind().is_unit(),
            })
        }
        None => {
            let key = SyntheticKey::try_new(
                SyntheticOwner::Item(owner),
                SyntheticRole::ProofUnitReturn,
                0,
            )
            .ok()?;
            let site = HirSourceSite::from_attached_span(
                parsed.document(),
                &attached.implicit_return_source_span(),
            )
            .ok()?;
            let metadata = slots.resolve_prepared(retained).ok()?;
            (matches!(metadata.origin(), HirOrigin::Synthetic(actual) if *actual == key)
                && metadata.source_site() == &site
                && !source_index
                    .syntax_owners
                    .contains_key(&SyntheticOwner::Type(retained))
                && payload.state() == &HirPoisonState::Clean
                && matches!(payload.kind(), HirTypeKind::Tuple(elements) if elements.is_empty()))
            .then_some(ReturnState {
                missing_type: false,
                recovered: false,
                is_unit: true,
            })
        }
    }
}

struct ProofBodyValidation<'a, 'arena> {
    owner: ItemId,
    proof: &'a HirProof,
    parsed: &'a ParsedSource,
    slots: &'a SlotSnapshot,
    arenas: &'a ItemValidationArenas<'arena>,
    block_arenas: &'a BlockValidationArenas<'arena>,
}

fn proof_body_matches(
    attached: &AttachedProofDeclaration,
    return_is_unit: bool,
    validation: ProofBodyValidation<'_, '_>,
) -> Option<Option<HirItemIssue>> {
    let ProofBodyValidation {
        owner,
        proof,
        parsed,
        slots,
        arenas,
        block_arenas,
    } = validation;
    let body_has_recovery = attached.body().has_recovery();
    let body_syntax = attached.body().syntax();
    let callable = arenas
        .scopes
        .resolve_prepared(slots, proof.callable_scope())
        .ok()?;
    let item_body_scope = proof.body().scope();
    if !item_body_scope_matches(
        owner,
        proof.callable_scope(),
        item_body_scope,
        HirScopeKind::Proof,
        body_syntax.id(),
        body_syntax.source_span(),
        slots,
        arenas,
    ) {
        return None;
    }
    if !direct_children_are_exact(
        proof.callable_scope(),
        proof.requires_scope(),
        proof.ensures_scope(),
        item_body_scope,
        callable,
        slots,
        arenas,
    ) {
        return None;
    }
    match (attached.body(), proof.body()) {
        (
            AttachedProofBody::Expression {
                expression: attached,
                ..
            },
            HirProofBody::Expression {
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
            AttachedProofBody::Block { block, .. },
            HirProofBody::Block {
                scope,
                statements,
                tail,
            },
        ) => {
            if *scope != item_body_scope {
                return None;
            }
            let recovered = proof_block_matches(
                parsed,
                slots,
                block_arenas,
                ItemValueBlockRetained {
                    owner,
                    callable_scope: proof.callable_scope(),
                    scope: *scope,
                    statements,
                    tail: *tail,
                },
                return_is_unit,
                body_syntax.id(),
                body_syntax.source_span(),
                block,
            )?;
            Some((body_has_recovery || recovered).then_some(HirItemIssue::Recovery))
        }
        (
            AttachedProofBody::Missing {
                missing: attached, ..
            },
            HirProofBody::Error {
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
