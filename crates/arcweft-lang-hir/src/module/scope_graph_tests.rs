use core::num::NonZeroU32;
use std::sync::Arc;

use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::incremental::{ParsedSource, SyntaxDatabase};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use crate::database::HirDatabase;
use crate::expr::{HirExpr, HirExprKind, HirPoisonState};
use crate::final_lowering::{
    StagedHirModuleTransaction, stage_unpublished_module_for_invariant_test,
};
use crate::identity::{
    HirIdKind, HirTypedId, ItemId, LocalId, RawHirId, ScopeId, SyntheticKey, SyntheticOwner,
    SyntheticRole,
};
use crate::item::HirItemKind;
use crate::lowering::{HirInvariantFailure, HirLowerFailure, HirModuleKey, LoweringRequest};
use crate::scope::{HirScope, HirScopeKind, HirScopeOwner};
use crate::source_index::HirSourceSite;
use crate::stmt::{HirStmt, HirStmtKind};
use crate::symbol::CallablePackageId;

fn parsed(document_id: &str, source: &str) -> ParsedSource {
    let name = SourceName::path("proof/scope-graph-freeze.arcw");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(document_id).unwrap(),
            name.clone(),
            source,
        )
        .unwrap(),
    );
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    syntax
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap()
}

fn module_key(parsed: &ParsedSource) -> HirModuleKey {
    HirModuleKey::new(
        CallablePackageId::try_new("proof-scope-graph-tests").unwrap(),
        CanonicalModulePath::crate_root(),
        parsed.document().identity().id().clone(),
    )
}

fn assert_graph_rejected(
    document_id: &str,
    source: &str,
    tamper: impl FnOnce(&ParsedSource, &mut StagedHirModuleTransaction<'_>, ScopeId, &[ItemId]),
) {
    let parsed = parsed(document_id, source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let request = LoweringRequest::try_new(key.clone(), &parsed).unwrap();
    let mut transaction = stage_unpublished_module_for_invariant_test(
        &database,
        request,
        crate::lowering::HirLoweringControl::new(),
    )
    .unwrap();
    let root = transaction.lower_parsed_source_items(&parsed).unwrap();
    let items = transaction.staged_source_ordered_items().to_vec();
    tamper(&parsed, &mut transaction, root, &items);

    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleArenaSnapshot
        ))
    ));
    assert!(database.current(&key).is_none());
}

fn action_scope(transaction: &mut StagedHirModuleTransaction<'_>, owner: ItemId) -> ScopeId {
    let (slots, arenas) = transaction.storage_mut();
    let item = arenas.items().resolve_staged(slots, owner).unwrap();
    let HirItemKind::Action(action) = item.kind() else {
        panic!("scope graph fixture requires an Action item");
    };
    action.callable_scope()
}

fn replace_scope(
    transaction: &mut StagedHirModuleTransaction<'_>,
    scope: ScopeId,
    replacement: HirScope,
) {
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .scopes()
        .revise_finalized(slots, scope, replacement)
        .unwrap();
}

fn scope_payload(transaction: &mut StagedHirModuleTransaction<'_>, scope: ScopeId) -> HirScope {
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .scopes()
        .resolve_staged(slots, scope)
        .unwrap()
        .clone()
}

fn action_source() -> &'static str {
    "action First(value: Value)\n"
}

fn two_action_source() -> &'static str {
    "action First(left: Value)\naction Second(right: Value)\n"
}

#[test]
fn central_scope_graph_preserves_valid_owner_and_source_ordered_membership() {
    let parsed = parsed(
        "arcweft-test://proof/scope-valid-source-order",
        two_action_source(),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let mut database = HirDatabase::try_new().unwrap();
    let request = LoweringRequest::try_new(module_key(&parsed), &parsed).unwrap();
    let mut transaction = stage_unpublished_module_for_invariant_test(
        &database,
        request,
        crate::lowering::HirLoweringControl::new(),
    )
    .unwrap();
    let root = transaction.lower_parsed_source_items(&parsed).unwrap();
    let items = transaction.staged_source_ordered_items().to_vec();
    let output = transaction.finish(&mut database).unwrap();
    let module = output.module();
    let expected_scopes = items
        .iter()
        .map(|&item_id| {
            let item = module
                .arenas()
                .items()
                .resolve(module.slots(), item_id)
                .unwrap();
            let HirItemKind::Action(action) = item.kind() else {
                panic!("scope graph fixture requires Action items");
            };
            action.callable_scope()
        })
        .collect::<Vec<_>>();
    let root_scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), root)
        .unwrap();
    assert_eq!(root_scope.children(), expected_scopes);

    for ((&item_id, &scope_id), item) in items
        .iter()
        .zip(&expected_scopes)
        .zip(module.source_ordered_items())
    {
        assert_eq!(item_id, *item);
        let scope = module
            .arenas()
            .scopes()
            .resolve(module.slots(), scope_id)
            .unwrap();
        let payload = module
            .arenas()
            .items()
            .resolve(module.slots(), item_id)
            .unwrap();
        let HirItemKind::Action(action) = payload.kind() else {
            unreachable!();
        };
        let expected_locals = action
            .parameters()
            .iter()
            .flat_map(|parameter| parameter.locals().iter().copied())
            .collect::<Vec<_>>();
        assert_eq!(scope.parent(), Some(root));
        assert_eq!(scope.owner(), &HirScopeOwner::Item(item_id));
        assert_eq!(scope.locals(), expected_locals);
    }
}

#[test]
fn central_scope_graph_rejects_root_membership_kind_and_dead_owner_atomically() {
    assert_graph_rejected(
        "arcweft-test://proof/scope-root-cardinality",
        action_source(),
        |_, transaction, _, items| {
            let callable = action_scope(transaction, items[0]);
            let original = scope_payload(transaction, callable);
            let replacement = HirScope::try_new(
                callable.module(),
                original.kind(),
                None,
                *original.owner(),
                original.children().into(),
                original.locals().into(),
            )
            .unwrap();
            replace_scope(transaction, callable, replacement);
        },
    );

    assert_graph_rejected(
        "arcweft-test://proof/scope-parent-membership",
        action_source(),
        |_, transaction, root, _| {
            let original = scope_payload(transaction, root);
            let replacement = original
                .try_with_members(Box::new([]), original.locals().into())
                .unwrap();
            replace_scope(transaction, root, replacement);
        },
    );

    assert_graph_rejected(
        "arcweft-test://proof/scope-owner-kind",
        action_source(),
        |_, transaction, _, items| {
            let callable = action_scope(transaction, items[0]);
            let original = scope_payload(transaction, callable);
            let replacement = HirScope::try_new(
                callable.module(),
                HirScopeKind::Closure,
                original.parent(),
                *original.owner(),
                original.children().into(),
                original.locals().into(),
            )
            .unwrap();
            replace_scope(transaction, callable, replacement);
        },
    );

    assert_graph_rejected(
        "arcweft-test://proof/scope-dead-owner",
        action_source(),
        |_, transaction, _, items| {
            let callable = action_scope(transaction, items[0]);
            let original = scope_payload(transaction, callable);
            let dead = ItemId::from_raw(RawHirId::new(
                callable.module(),
                NonZeroU32::new(1_000_000).unwrap(),
                HirIdKind::Item,
            ));
            let replacement = HirScope::try_new(
                callable.module(),
                original.kind(),
                original.parent(),
                HirScopeOwner::Item(dead),
                original.children().into(),
                original.locals().into(),
            )
            .unwrap();
            replace_scope(transaction, callable, replacement);
        },
    );
}

#[test]
fn central_scope_graph_rejects_cross_item_subtree_splicing_atomically() {
    assert_graph_rejected(
        "arcweft-test://proof/scope-cross-item-splice",
        two_action_source(),
        |_, transaction, root, items| {
            let first = action_scope(transaction, items[0]);
            let second = action_scope(transaction, items[1]);
            let root_payload = scope_payload(transaction, root);
            let first_payload = scope_payload(transaction, first);
            let second_payload = scope_payload(transaction, second);

            let root_replacement = root_payload
                .try_with_members(Box::new([first]), root_payload.locals().into())
                .unwrap();
            let mut first_children = first_payload.children().to_vec();
            first_children.push(second);
            let first_replacement = first_payload
                .try_with_members(
                    first_children.into_boxed_slice(),
                    first_payload.locals().into(),
                )
                .unwrap();
            let second_replacement = HirScope::try_new(
                second.module(),
                second_payload.kind(),
                Some(first),
                *second_payload.owner(),
                second_payload.children().into(),
                second_payload.locals().into(),
            )
            .unwrap();

            replace_scope(transaction, root, root_replacement);
            replace_scope(transaction, first, first_replacement);
            replace_scope(transaction, second, second_replacement);
        },
    );
}

#[test]
fn central_scope_graph_requires_exact_local_membership_atomically() {
    assert_graph_rejected(
        "arcweft-test://proof/scope-missing-local",
        action_source(),
        |_, transaction, _, items| {
            let callable = action_scope(transaction, items[0]);
            let original = scope_payload(transaction, callable);
            assert_eq!(original.locals().len(), 1);
            let replacement = original
                .try_with_members(original.children().into(), Box::new([]))
                .unwrap();
            replace_scope(transaction, callable, replacement);
        },
    );

    assert_graph_rejected(
        "arcweft-test://proof/scope-cross-local",
        two_action_source(),
        |_, transaction, _, items| {
            let first = action_scope(transaction, items[0]);
            let second = action_scope(transaction, items[1]);
            let first_payload = scope_payload(transaction, first);
            let second_payload = scope_payload(transaction, second);
            let [second_local] = second_payload.locals() else {
                panic!("second Action owns one parameter local");
            };
            let mut first_locals = first_payload.locals().to_vec();
            first_locals.push(*second_local);
            let first_replacement = first_payload
                .try_with_members(
                    first_payload.children().into(),
                    first_locals.into_boxed_slice(),
                )
                .unwrap();
            let second_replacement = second_payload
                .try_with_members(second_payload.children().into(), Box::new([]))
                .unwrap();
            replace_scope(transaction, first, first_replacement);
            replace_scope(transaction, second, second_replacement);
        },
    );

    assert_graph_rejected(
        "arcweft-test://proof/scope-dead-local",
        action_source(),
        |_, transaction, _, items| {
            let callable = action_scope(transaction, items[0]);
            let original = scope_payload(transaction, callable);
            let dead = LocalId::from_raw(RawHirId::new(
                callable.module(),
                NonZeroU32::new(1_000_001).unwrap(),
                HirIdKind::Local,
            ));
            let replacement = original
                .try_with_members(original.children().into(), Box::new([dead]))
                .unwrap();
            replace_scope(transaction, callable, replacement);
        },
    );
}

#[derive(Clone, Copy)]
enum LexicalOwnerCase {
    Expression,
    Statement,
}

#[test]
fn expression_and_statement_owned_scopes_require_the_owner_lexical_parent() {
    for (document_id, owner_case) in [
        (
            "arcweft-test://proof/scope-expression-parent",
            LexicalOwnerCase::Expression,
        ),
        (
            "arcweft-test://proof/scope-statement-parent",
            LexicalOwnerCase::Statement,
        ),
    ] {
        assert_graph_rejected(
            document_id,
            action_source(),
            move |parsed, transaction, root, items| {
                let callable = action_scope(transaction, items[0]);
                let site = HirSourceSite::Span(parsed.root_syntax().source_span().clone());
                let (slots, arenas) = transaction.storage_mut();
                let expression = arenas
                    .expressions()
                    .allocate_source(
                        slots,
                        parsed.root_syntax().id(),
                        site.clone(),
                        HirExpr::try_new(root, HirExprKind::Unit, HirPoisonState::Clean).unwrap(),
                    )
                    .unwrap();
                let owner = match owner_case {
                    LexicalOwnerCase::Expression => HirScopeOwner::Expr(expression),
                    LexicalOwnerCase::Statement => {
                        let statement = arenas
                            .statements()
                            .allocate_source(
                                slots,
                                parsed.root_syntax().id(),
                                site.clone(),
                                HirStmt::try_new(root, HirStmtKind::Expression { expression })
                                    .unwrap(),
                            )
                            .unwrap();
                        HirScopeOwner::Stmt(statement)
                    }
                };
                let key = match owner {
                    HirScopeOwner::Expr(owner) => SyntheticKey::try_new(
                        SyntheticOwner::Expr(owner),
                        SyntheticRole::RecoveryOperand,
                        0,
                    )
                    .unwrap(),
                    HirScopeOwner::Stmt(owner) => SyntheticKey::try_new(
                        SyntheticOwner::Stmt(owner),
                        SyntheticRole::ForIterator,
                        0,
                    )
                    .unwrap(),
                    HirScopeOwner::Module(_) | HirScopeOwner::Item(_) => unreachable!(),
                };
                let reservation = arenas.scopes().reserve_synthetic(slots, key, site).unwrap();
                let child = reservation.id();
                arenas
                    .scopes()
                    .finalize(
                        slots,
                        reservation,
                        HirScope::try_new(
                            root.module(),
                            HirScopeKind::Block,
                            Some(callable),
                            owner,
                            Box::new([]),
                            Box::new([]),
                        )
                        .unwrap(),
                    )
                    .unwrap();
                let callable_payload = arenas
                    .scopes()
                    .resolve_staged(slots, callable)
                    .unwrap()
                    .clone();
                let mut children = callable_payload.children().to_vec();
                children.push(child);
                arenas
                    .scopes()
                    .revise_finalized(
                        slots,
                        callable,
                        callable_payload
                            .try_with_members(
                                children.into_boxed_slice(),
                                callable_payload.locals().into(),
                            )
                            .unwrap(),
                    )
                    .unwrap();
            },
        );
    }
}
