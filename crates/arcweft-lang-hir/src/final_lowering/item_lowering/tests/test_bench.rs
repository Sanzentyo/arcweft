use super::*;

use arcweft_lang_syntax::attachment::TypedItemNode;

use crate::identity::ExprId;
use crate::item::{HirBenchItem, HirTestItem, HirTestKind};
use crate::leaf::{HirEntityReference, HirIdRef, HirIdRefValue};
use crate::stmt::{HirStmt, HirStmtKind, HirStmtPoisonState};

fn test_item(
    module: &HirModule,
    ordinal: usize,
) -> (crate::identity::ItemId, &HirItem, &HirTestItem) {
    let owner = module.source_ordered_items()[ordinal];
    let item = resolve_item(module, ordinal);
    let HirItemKind::Test(test) = item.kind() else {
        panic!("source-ordered item {ordinal} must be a Test")
    };
    (owner, item, test)
}

fn bench_item(
    module: &HirModule,
    ordinal: usize,
) -> (crate::identity::ItemId, &HirItem, &HirBenchItem) {
    let owner = module.source_ordered_items()[ordinal];
    let item = resolve_item(module, ordinal);
    let HirItemKind::Bench(bench) = item.kind() else {
        panic!("source-ordered item {ordinal} must be a Bench")
    };
    (owner, item, bench)
}

fn assert_first_test_scope_and_goto(module: &HirModule) {
    let (owner, item, test) = test_item(module, 0);
    assert_eq!(
        item.prefix().documentation().unwrap().markdown(),
        "Scenario plan"
    );
    assert_eq!(item.prefix().attributes().len(), 1);
    let body_scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), test.scope())
        .unwrap();
    assert_eq!(body_scope.kind(), HirScopeKind::Block);
    assert_eq!(body_scope.parent(), Some(item.scope()));
    assert_eq!(body_scope.owner(), &HirScopeOwner::Item(owner));
    assert!(
        module
            .arenas()
            .scopes()
            .resolve(module.slots(), item.scope())
            .unwrap()
            .children()
            .contains(&test.scope())
    );
    assert_eq!(test.body().len(), 2);
    let goto = module
        .arenas()
        .statements()
        .resolve(module.slots(), test.body()[0])
        .unwrap();
    let HirStmtKind::Goto { target } = goto.kind() else {
        panic!("first plan statement must remain Goto")
    };
    assert_eq!(goto.scope(), test.scope());
    assert_eq!(goto.state(), &HirStmtPoisonState::Clean);
    assert_eq!(
        module
            .arenas()
            .expressions()
            .resolve(module.slots(), *target)
            .unwrap()
            .scope(),
        test.scope()
    );
    assert!(module.declaration_members().arena(owner).is_none());
}

fn assert_bench_scope(module: &HirModule) {
    let (_, item, bench) = bench_item(module, 5);
    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert_eq!(bench.body().len(), 3);
    assert_eq!(
        module
            .arenas()
            .scopes()
            .resolve(module.slots(), bench.scope())
            .unwrap()
            .parent(),
        Some(item.scope())
    );
}

#[test]
fn clean_test_and_bench_retain_typed_id_kind_statement_scope_and_goto() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-test-bench-clean",
        concat!(
            "/// Scenario plan\n",
            "#[tool.fixture]\n",
            "test @test.scenario scenario {\n",
            "    goto @flow.opening\n",
            "    true\n",
            "}\n",
            "test @test.visual visual {}\n",
            "test @test.audio audio {}\n",
            "test @test.fixture fixture {}\n",
            "test @test.custom headless {}\n",
            "bench @bench.score {\n",
            "    setup { true }\n",
            "    measure { false }\n",
            "    report { true }\n",
            "}\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );

    let plan_id_syntax = parsed
        .tree()
        .items()
        .unwrap()
        .into_iter()
        .filter_map(|item| match item {
            TypedItemNode::Test(test) => Some(test.semantics().unwrap().id().syntax().id()),
            TypedItemNode::Bench(bench) => Some(bench.semantics().unwrap().id().syntax().id()),
            _ => None,
        })
        .collect::<Vec<_>>();

    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    assert_eq!(module.source_ordered_items().len(), 6);

    for (ordinal, expected) in [
        HirTestKind::Scenario,
        HirTestKind::Visual,
        HirTestKind::Audio,
        HirTestKind::Fixture,
    ]
    .into_iter()
    .enumerate()
    {
        let (_, item, test) = test_item(&module, ordinal);
        assert_eq!(item.state(), &HirItemPoisonState::Clean);
        assert_eq!(test.kind(), &expected);
        assert!(matches!(test.id(), HirIdRefValue::Resolved(_)));
    }
    let (_, custom_item, custom) = test_item(&module, 4);
    assert_eq!(custom_item.state(), &HirItemPoisonState::Clean);
    assert!(matches!(
        custom.kind(),
        HirTestKind::Custom(name) if name.as_str() == "headless"
    ));
    assert_first_test_scope_and_goto(&module);
    assert_bench_scope(&module);
    for syntax in plan_id_syntax {
        assert_eq!(module.slots().prepared_source_owner::<ExprId>(syntax), None);
    }
}

#[test]
fn test_and_bench_recovery_preserves_missing_id_kind_and_body_without_defaults() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-test-bench-recovery",
        concat!(
            "test scenario {}\n",
            "test @test.no_kind {}\n",
            "test @test.no_body scenario\n",
            "bench {}\n",
            "bench @bench.no_body\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    let (missing_id_owner, missing_id_item, missing_id) = test_item(&module, 0);
    assert_eq!(
        missing_id_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingId)
    );
    assert!(missing_id.id().recovery_issue().is_some());
    assert!(matches!(missing_id.kind(), HirTestKind::Scenario));
    assert_item_owner_whole_recovery(&module, missing_id_owner);

    let (_, missing_kind_item, missing_kind) = test_item(&module, 1);
    assert_eq!(
        missing_kind_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingKind)
    );
    assert!(matches!(missing_kind.kind(), HirTestKind::Recovered(_)));

    let (_, missing_body_item, missing_body) = test_item(&module, 2);
    assert_eq!(
        missing_body_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingBody)
    );
    assert!(missing_body.body().is_empty());
    let scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), missing_body.scope())
        .unwrap();
    assert_eq!(scope.kind(), HirScopeKind::Block);
    assert!(scope.locals().is_empty());

    let (_, bench_missing_id_item, bench_missing_id) = bench_item(&module, 3);
    assert_eq!(
        bench_missing_id_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingId)
    );
    assert!(bench_missing_id.id().recovery_issue().is_some());
    let (_, bench_missing_body_item, bench_missing_body) = bench_item(&module, 4);
    assert_eq!(
        bench_missing_body_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingBody)
    );
    assert!(bench_missing_body.body().is_empty());
}

fn assert_test_freeze_rejects(case: &str, tamper: impl FnOnce(&HirTestItem) -> HirTestItem) {
    let parsed = parse(
        &format!("arcweft-test://proof/final-hir-test-freeze-{case}"),
        concat!(
            "test @test.freeze scenario {\n",
            "    goto @flow.first\n",
            "    goto @flow.second\n",
            "}\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction
        .lower_attached_source_file_items(&parsed.tree())
        .unwrap();
    let owner = transaction.source_ordered_items[0];
    let (slots, arenas) = transaction.storage_mut();
    let original = arenas.items().resolve_staged(slots, owner).unwrap().clone();
    let HirItemKind::Test(test) = original.kind() else {
        panic!("final Test item")
    };
    let replacement = HirItem::try_new_with_state(
        owner,
        original.scope(),
        original.prefix().clone(),
        HirItemKind::Test(tamper(test)),
        original.members().into(),
        *original.state(),
    )
    .unwrap();
    arenas
        .items()
        .revise_finalized(slots, owner, replacement)
        .unwrap();
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&key).is_none());
}

#[test]
fn test_freeze_rejects_id_substitution_and_statement_reordering() {
    assert_test_freeze_rejects("id", |test| {
        HirTestItem::new(
            HirIdRefValue::Resolved(HirIdRef::absolute(
                HirEntityReference::try_new("test.other".into()).unwrap(),
            )),
            test.kind().clone(),
            test.scope(),
            test.body().into(),
        )
    });
    assert_test_freeze_rejects("statement-order", |test| {
        let mut statements = test.body().to_vec();
        statements.swap(0, 1);
        HirTestItem::new(
            test.id().clone(),
            test.kind().clone(),
            test.scope(),
            statements.into_boxed_slice(),
        )
    });
}

#[test]
fn test_freeze_rejects_goto_target_substitution() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-test-freeze-goto-target",
        concat!(
            "test @test.freeze scenario {\n",
            "    goto @flow.first\n",
            "    goto @flow.second\n",
            "}\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction
        .lower_attached_source_file_items(&parsed.tree())
        .unwrap();
    let owner = transaction.source_ordered_items[0];
    let (first_statement, second_statement) = {
        let (slots, arenas) = transaction.storage_mut();
        let item = arenas.items().resolve_staged(slots, owner).unwrap();
        let HirItemKind::Test(test) = item.kind() else {
            panic!("final Test item")
        };
        (test.body()[0], test.body()[1])
    };
    let replacement = {
        let (slots, arenas) = transaction.storage_mut();
        let first = arenas
            .statements()
            .resolve_staged(slots, first_statement)
            .unwrap()
            .clone();
        let second = arenas
            .statements()
            .resolve_staged(slots, second_statement)
            .unwrap();
        let HirStmtKind::Goto { target } = second.kind() else {
            panic!("second plan statement must remain Goto")
        };
        HirStmt::try_new_with_state(
            first.scope(),
            HirStmtKind::Goto { target: *target },
            first.state().clone(),
        )
        .unwrap()
    };
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .statements()
        .revise_finalized(slots, first_statement, replacement)
        .unwrap();
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&key).is_none());
}
