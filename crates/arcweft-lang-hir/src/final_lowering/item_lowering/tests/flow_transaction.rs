use std::panic::{AssertUnwindSafe, catch_unwind};

use super::*;

use crate::expr::{HirExprKind, HirThreadFlowItem};
use crate::final_lowering::ProofReturnProjectModuleTransaction;
use crate::identity::{HirLimit, ItemId, LocalGeneration, LocalId, ScopeId};
use crate::lowering::{
    HirLowerFailure, HirLoweringCheckpoint, HirLoweringControl, HirModuleKey, LoweringRequest,
};
use crate::proof_return::{HirProofReturnProjectTransaction, HirProofReturnSemanticFactSet};
use crate::source_index::{
    HirFlowSourceRole, HirItemSourceRole, HirSourcePresence, HirSourceQuery,
};
use crate::stmt::HirStmtKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StagedFlowIds {
    item: ItemId,
    callable_scope: ScopeId,
    requires_scope: ScopeId,
    ensures_scope: ScopeId,
    body_scope: ScopeId,
    parameter: LocalId,
}

fn fixture(document_id: &str) -> (ParsedSource, HirModuleKey) {
    let parsed = parse(
        document_id,
        "flow transactional(value: I32) {\n    return unit\n}\n",
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "unexpected Flow fixture diagnostics: {:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    (parsed, key)
}

fn direct_dialogue_flow_fixture(
    document_id: &str,
    body_item_count: usize,
) -> (ParsedSource, HirModuleKey) {
    const HEADER: &str = "flow transactional(value: I32) {\n";
    const BODY_ITEM: &str = "    alice[こんにちは。]\n";
    const FOOTER: &str = "}\n";

    let mut source = String::with_capacity(
        HEADER.len() + BODY_ITEM.len().saturating_mul(body_item_count) + FOOTER.len(),
    );
    source.push_str(HEADER);
    for _ in 0..body_item_count {
        source.push_str(BODY_ITEM);
    }
    source.push_str(FOOTER);

    let parsed = parse(document_id, &source);
    assert!(
        parsed.diagnostics().is_empty(),
        "unexpected direct-Dialogue Flow diagnostics: {:?}",
        parsed.diagnostics().first()
    );
    let key = module_key(&parsed);
    (parsed, key)
}

fn thread_expression_flow_fixture(
    document_id: &str,
    body_item_count: usize,
) -> (ParsedSource, HirModuleKey) {
    const HEADER: &str = "flow transactional(value: I32) {\n    thread {\n";
    const BODY_ITEM: &str = "        alice[こんにちは。]\n";
    const FOOTER: &str = "    }\n}\n";

    let mut source = String::with_capacity(
        HEADER.len() + BODY_ITEM.len().saturating_mul(body_item_count) + FOOTER.len(),
    );
    source.push_str(HEADER);
    for _ in 0..body_item_count {
        source.push_str(BODY_ITEM);
    }
    source.push_str(FOOTER);

    let parsed = parse(document_id, &source);
    assert!(
        parsed.diagnostics().is_empty(),
        "unexpected Thread-expression Flow diagnostics: {:?}",
        parsed.diagnostics().first()
    );
    let items = parsed
        .items()
        .expect("Thread-expression fixture source items");
    let [TypedItemNode::Flow(flow)] = items.as_slice() else {
        panic!("Thread-expression fixture must contain one Flow")
    };
    let declaration = flow
        .semantics()
        .expect("Thread-expression fixture Flow semantics");
    let arcweft_lang_syntax::attachment::AttachedRequiredFlowBody::Present(body) =
        declaration.body()
    else {
        panic!("Thread-expression fixture Flow body must be present")
    };
    let [item] = body.items() else {
        panic!("Thread-expression fixture Flow body must contain one statement")
    };
    let statement = item
        .statement()
        .expect("direct Thread expression remains statement-backed");
    let statement = statement
        .cast::<arcweft_lang_syntax::attachment::node::ExpressionStatementKind>()
        .expect("direct Thread expression uses the ordinary expression-statement owner");
    let expression = statement
        .expression()
        .expect("direct Thread expression statement operand")
        .semantic()
        .expect("direct Thread expression semantic projection");
    assert!(matches!(
        expression.projection(),
        arcweft_lang_syntax::expressions::ExpressionProjection::Thread(_)
    ));
    let thread = expression
        .thread()
        .expect("direct Thread expression retains its exact typed owner");
    let arcweft_lang_syntax::attachment::AttachedRequiredThreadExpressionBody::Present(thread_body) =
        thread
            .statement_body()
            .expect("direct Thread expression body projection")
    else {
        panic!("direct Thread expression body must be present")
    };
    assert_ne!(
        body.syntax().id(),
        thread_body.syntax().id(),
        "Flow and nested Thread bodies must not alias one scope source owner"
    );
    assert_eq!(thread_body.items().len(), body_item_count);
    let key = module_key(&parsed);
    (parsed, key)
}

fn stage_flow<'source>(
    database: &HirDatabase,
    parsed: &'source ParsedSource,
    key: &HirModuleKey,
    control: HirLoweringControl,
) -> Result<HirProofReturnProjectTransaction<'source>, HirLowerFailure> {
    let world = ProjectSymbolWorldId::try_new(
        key.package().clone(),
        parsed.document().identity().id().clone(),
        "flow-transaction-test",
    )
    .expect("Flow transaction world");
    let revision = ProjectSymbolRevision::try_for_documents([parsed.document().identity()])
        .expect("Flow transaction revision");
    database.stage_proof_return_project(
        [LoweringRequest::try_new(key.clone(), parsed).expect("Flow lowering request")],
        world,
        revision,
        [parsed.document().identity()],
        control,
    )
}

fn empty_proof_facts(
    transaction: &HirProofReturnProjectTransaction<'_>,
) -> Arc<HirProofReturnSemanticFactSet> {
    HirProofReturnSemanticFactSet::try_new(
        Arc::clone(transaction.generation()),
        transaction.headers().cloned(),
        [],
    )
    .expect("Flow fixture has no authored Proof return")
}

fn staged_flow_ids(transaction: &HirProofReturnProjectTransaction<'_>) -> StagedFlowIds {
    let [ProofReturnProjectModuleTransaction::Staged(module)] = transaction.modules.as_slice()
    else {
        panic!("Flow fixture must stage exactly one changed module")
    };
    let [item] = module.staged_source_ordered_items() else {
        panic!("Flow fixture must stage exactly one item")
    };
    let payload = module.staged_item(*item).expect("staged Flow item");
    let HirItemKind::Flow(flow) = payload.kind() else {
        panic!("staged fixture item must be Flow")
    };
    let [parameter] = flow.parameters() else {
        panic!("Flow fixture must retain one parameter")
    };
    let [parameter] = parameter.locals() else {
        panic!("Flow parameter must bind one local")
    };
    StagedFlowIds {
        item: *item,
        callable_scope: flow.callable_scope(),
        requires_scope: flow.requires_scope(),
        ensures_scope: flow.ensures_scope(),
        body_scope: flow.body_scope(),
        parameter: *parameter,
    }
}

fn publish_flow(
    database: &mut HirDatabase,
    transaction: HirProofReturnProjectTransaction<'_>,
) -> Result<Arc<HirModule>, HirLowerFailure> {
    let facts = empty_proof_facts(&transaction);
    let mut outputs = transaction.publish_with_semantic_facts(database, facts)?;
    let output = outputs.pop().expect("one Flow output");
    assert!(outputs.is_empty());
    Ok(output.into_module())
}

fn assert_published_once(
    database: &HirDatabase,
    parsed: &ParsedSource,
    key: &HirModuleKey,
    module: &Arc<HirModule>,
    expected: StagedFlowIds,
) {
    assert_eq!(module.source_ordered_items(), [expected.item]);
    let item = module
        .resolve_item(expected.item)
        .expect("published Flow item");
    let HirItemKind::Flow(flow) = item.kind() else {
        panic!("published fixture item must be Flow")
    };
    assert_eq!(flow.callable_scope(), expected.callable_scope);
    assert_eq!(flow.requires_scope(), expected.requires_scope);
    assert_eq!(flow.ensures_scope(), expected.ensures_scope);
    assert_eq!(flow.body_scope(), expected.body_scope);
    let local = module
        .arenas()
        .locals()
        .resolve(module.slots(), expected.parameter)
        .expect("published parameter local");
    assert_eq!(local.generation(), LocalGeneration::FIRST);
    let source = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Item {
                owner: expected.item,
                role: HirItemSourceRole::Flow(HirFlowSourceRole::Whole),
            },
        )
        .expect("published Flow source query");
    assert!(matches!(source.presence(), HirSourcePresence::Present(_)));
    let current = database.current(key).expect("retried Flow is current");
    assert!(Arc::ptr_eq(&current, module));
}

#[test]
fn cancel_before_preflight() {
    let (parsed, key) = fixture("arcweft-test://proof/flow-cancel-before-preflight");
    let database = HirDatabase::try_new().unwrap();
    let before = database.test_state();
    let control = HirLoweringControl::new();
    control.cancel();

    assert!(matches!(
        stage_flow(&database, &parsed, &key, control),
        Err(HirLowerFailure::Cancelled)
    ));
    assert_eq!(database.test_state(), before);
}

#[test]
fn cancel_after_reservation() {
    let (parsed, key) = fixture("arcweft-test://proof/flow-cancel-after-reservation");
    let database = HirDatabase::try_new().unwrap();
    let before = database.test_state();
    let control = HirLoweringControl::cancel_at_for_test(HirLoweringCheckpoint::FlowScopesReserved);

    assert!(matches!(
        stage_flow(&database, &parsed, &key, control.clone()),
        Err(HirLowerFailure::Cancelled)
    ));
    assert_eq!(control.hit_count_for_test(), 1);
    assert_eq!(database.test_state(), before);
}

#[test]
fn cancel_before_commit() {
    let (parsed, key) = fixture("arcweft-test://proof/flow-cancel-before-commit");
    let mut database = HirDatabase::try_new().unwrap();
    let before = database.test_state();
    let control = HirLoweringControl::cancel_at_for_test(HirLoweringCheckpoint::BeforeCommit);
    let transaction =
        stage_flow(&database, &parsed, &key, control.clone()).expect("staged Flow transaction");

    assert!(matches!(
        publish_flow(&mut database, transaction),
        Err(HirLowerFailure::Cancelled)
    ));
    assert_eq!(control.hit_count_for_test(), 1);
    assert_eq!(database.test_state(), before);
}

#[test]
fn panic_during_child_lowering() {
    let (parsed, key) = fixture("arcweft-test://proof/flow-panic-child");
    let database = HirDatabase::try_new().unwrap();
    let before = database.test_state();
    let control = HirLoweringControl::panic_at_for_test(HirLoweringCheckpoint::ChildReserved);

    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = stage_flow(&database, &parsed, &key, control.clone());
    }));
    assert!(panic.is_err(), "test failpoint must propagate its panic");
    assert_eq!(control.hit_count_for_test(), 1);
    assert_eq!(database.test_state(), before);
}

#[test]
fn panic_during_source_freeze() {
    let (parsed, key) = fixture("arcweft-test://proof/flow-panic-source-freeze");
    let mut database = HirDatabase::try_new().unwrap();
    let before = database.test_state();
    let control = HirLoweringControl::panic_at_for_test(HirLoweringCheckpoint::SourceFreeze);
    let transaction =
        stage_flow(&database, &parsed, &key, control.clone()).expect("staged Flow transaction");

    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = publish_flow(&mut database, transaction);
    }));
    assert!(panic.is_err(), "test failpoint must propagate its panic");
    assert_eq!(control.hit_count_for_test(), 1);
    assert_eq!(database.test_state(), before);
}

#[test]
fn retry_cancelled() {
    let (parsed, key) = fixture("arcweft-test://proof/flow-retry-cancelled");
    let mut database = HirDatabase::try_new().unwrap();
    let before = database.test_state();
    let control = HirLoweringControl::cancel_at_for_test(HirLoweringCheckpoint::BeforeCommit);
    let cancelled =
        stage_flow(&database, &parsed, &key, control.clone()).expect("staged cancelled attempt");
    let cancelled_ids = staged_flow_ids(&cancelled);
    assert!(matches!(
        publish_flow(&mut database, cancelled),
        Err(HirLowerFailure::Cancelled)
    ));
    assert_eq!(control.hit_count_for_test(), 1);
    assert_eq!(database.test_state(), before);

    let retry = stage_flow(&database, &parsed, &key, HirLoweringControl::new())
        .expect("fresh retry stages");
    let retry_ids = staged_flow_ids(&retry);
    assert_eq!(retry_ids, cancelled_ids);
    let module = publish_flow(&mut database, retry).expect("fresh retry publishes");
    assert_published_once(&database, &parsed, &key, &module, retry_ids);
}

#[test]
fn retry_panicked() {
    let (parsed, key) = fixture("arcweft-test://proof/flow-retry-panicked");
    let mut database = HirDatabase::try_new().unwrap();
    let before = database.test_state();
    let control = HirLoweringControl::panic_at_for_test(HirLoweringCheckpoint::SourceFreeze);
    let panicked =
        stage_flow(&database, &parsed, &key, control.clone()).expect("staged panicked attempt");
    let panicked_ids = staged_flow_ids(&panicked);
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = publish_flow(&mut database, panicked);
    }));
    assert!(panic.is_err(), "test failpoint must propagate its panic");
    assert_eq!(control.hit_count_for_test(), 1);
    assert_eq!(database.test_state(), before);

    let retry = stage_flow(&database, &parsed, &key, HirLoweringControl::new())
        .expect("fresh retry stages");
    let retry_ids = staged_flow_ids(&retry);
    assert_eq!(retry_ids, panicked_ids);
    let module = publish_flow(&mut database, retry).expect("fresh retry publishes");
    assert_published_once(&database, &parsed, &key, &module, retry_ids);
}

#[test]
#[ignore = "Tier 2: parses and lowers the real 65,536/65,537-item Flow boundary"]
fn thread_flow_items_exact_and_one_over_are_atomic_for_attached_flow() {
    let maximum = HirLimit::ThreadFlowItems.maximum();
    let (exact, exact_key) =
        direct_dialogue_flow_fixture("arcweft-test://proof/flow-thread-items-exact", maximum);
    let mut exact_database = HirDatabase::try_new().unwrap();
    let exact_transaction = stage_flow(
        &exact_database,
        &exact,
        &exact_key,
        HirLoweringControl::new(),
    )
    .expect("the inclusive ThreadFlowItems boundary stages");
    let exact_ids = staged_flow_ids(&exact_transaction);
    let exact_module = publish_flow(&mut exact_database, exact_transaction)
        .expect("the inclusive ThreadFlowItems boundary publishes");
    assert_published_once(
        &exact_database,
        &exact,
        &exact_key,
        &exact_module,
        exact_ids,
    );
    let exact_item = exact_module
        .resolve_item(exact_ids.item)
        .expect("published exact-boundary Flow item");
    let HirItemKind::Flow(exact_flow) = exact_item.kind() else {
        panic!("published exact-boundary item must be Flow")
    };
    assert_eq!(exact_flow.body().items().len(), maximum);

    let observed = maximum.checked_add(1).expect("one-over Flow limit");
    let (one_over, one_over_key) =
        direct_dialogue_flow_fixture("arcweft-test://proof/flow-thread-items-one-over", observed);
    let one_over_database = HirDatabase::try_new().unwrap();
    let before = one_over_database.test_state();
    let first_error = match stage_flow(
        &one_over_database,
        &one_over,
        &one_over_key,
        HirLoweringControl::new(),
    ) {
        Err(HirLowerFailure::Limit(error)) => error,
        Err(other) => panic!("one-over Flow failed with the wrong boundary: {other:?}"),
        Ok(_) => panic!("one-over ThreadFlowItems transaction must not stage"),
    };
    assert_eq!(first_error.limit(), HirLimit::ThreadFlowItems);
    assert_eq!(first_error.maximum(), maximum);
    assert_eq!(first_error.observed(), observed);
    assert_eq!(one_over_database.test_state(), before);
    assert!(one_over_database.current(&one_over_key).is_none());

    let retry_error = match stage_flow(
        &one_over_database,
        &one_over,
        &one_over_key,
        HirLoweringControl::new(),
    ) {
        Err(HirLowerFailure::Limit(error)) => error,
        Err(other) => panic!("one-over Flow retry changed failure boundary: {other:?}"),
        Ok(_) => panic!("one-over ThreadFlowItems retry must not stage"),
    };
    assert_eq!(retry_error, first_error);
    assert_eq!(one_over_database.test_state(), before);
    assert!(one_over_database.current(&one_over_key).is_none());
}

#[test]
fn empty_thread_expression_is_an_ordinary_flow_statement_with_an_expr_owned_body_scope() {
    let (parsed, key) =
        thread_expression_flow_fixture("arcweft-test://proof/empty-thread-expression-flow", 0);
    let mut database = HirDatabase::try_new().unwrap();
    let transaction = stage_flow(&database, &parsed, &key, HirLoweringControl::new())
        .expect("empty Thread expression Flow stages through the ordinary expression statement");
    let ids = staged_flow_ids(&transaction);
    let module = publish_flow(&mut database, transaction)
        .expect("empty Thread expression Flow publishes atomically");
    let item = module.resolve_item(ids.item).expect("published Flow item");
    let HirItemKind::Flow(flow) = item.kind() else {
        panic!("published item must be Flow")
    };
    let [HirThreadFlowItem::Statement(statement)] = flow.body().items() else {
        panic!("bare Thread expression must remain an ordinary Flow statement")
    };
    let statement = module
        .resolve_stmt(*statement)
        .expect("published expression statement");
    let HirStmtKind::Expression { expression } = statement.kind() else {
        panic!("bare Thread expression must use HirStmtKind::Expression")
    };
    let expression = *expression;
    let payload = module
        .resolve_expr(expression)
        .expect("published Thread expression");
    let HirExprKind::Thread(thread) = payload.kind() else {
        panic!("expression statement must retain the Thread payload")
    };
    assert!(thread.body().items().is_empty());
    let body_scope = module
        .resolve_scope(thread.scope())
        .expect("published Thread body scope");
    assert_eq!(body_scope.owner(), &HirScopeOwner::Expr(expression));
}

#[test]
#[ignore = "Tier 2: parses and lowers the real 65,536/65,537-item Thread-expression boundary"]
fn thread_flow_items_exact_and_one_over_are_atomic_for_attached_thread_expression() {
    let maximum = HirLimit::ThreadFlowItems.maximum();
    let (exact, exact_key) = thread_expression_flow_fixture(
        "arcweft-test://proof/thread-expression-flow-items-exact",
        maximum,
    );
    let mut exact_database = HirDatabase::try_new().unwrap();
    let exact_transaction = stage_flow(
        &exact_database,
        &exact,
        &exact_key,
        HirLoweringControl::new(),
    )
    .expect("the inclusive Thread-expression item boundary stages");
    let exact_ids = staged_flow_ids(&exact_transaction);
    let exact_module = publish_flow(&mut exact_database, exact_transaction)
        .expect("the inclusive Thread-expression item boundary publishes");
    assert_published_once(
        &exact_database,
        &exact,
        &exact_key,
        &exact_module,
        exact_ids,
    );
    let exact_item = exact_module
        .resolve_item(exact_ids.item)
        .expect("published exact-boundary Flow item");
    let HirItemKind::Flow(exact_flow) = exact_item.kind() else {
        panic!("published exact-boundary item must be Flow")
    };
    let [HirThreadFlowItem::Statement(statement)] = exact_flow.body().items() else {
        panic!("exact-boundary Flow must contain one Thread expression statement")
    };
    let statement = exact_module
        .resolve_stmt(*statement)
        .expect("published Thread expression statement");
    let HirStmtKind::Expression {
        expression: thread_owner,
    } = statement.kind()
    else {
        panic!("published statement must retain the ordinary Thread expression owner")
    };
    let thread_owner = *thread_owner;
    let thread = exact_module
        .resolve_expr(thread_owner)
        .expect("published Thread expression");
    let HirExprKind::Thread(thread) = thread.kind() else {
        panic!("published expression must retain the Thread payload")
    };
    assert_eq!(thread.body().items().len(), maximum);
    let thread_scope = exact_module
        .resolve_scope(thread.scope())
        .expect("published Thread body scope");
    assert_eq!(thread_scope.owner(), &HirScopeOwner::Expr(thread_owner));

    let observed = maximum
        .checked_add(1)
        .expect("one-over Thread-expression item limit");
    let (one_over, one_over_key) = thread_expression_flow_fixture(
        "arcweft-test://proof/thread-expression-flow-items-one-over",
        observed,
    );
    let one_over_database = HirDatabase::try_new().unwrap();
    let before = one_over_database.test_state();
    let first_error = match stage_flow(
        &one_over_database,
        &one_over,
        &one_over_key,
        HirLoweringControl::new(),
    ) {
        Err(HirLowerFailure::Limit(error)) => error,
        Err(other) => {
            panic!("one-over Thread expression failed with the wrong boundary: {other:?}")
        }
        Ok(_) => panic!("one-over Thread-expression item transaction must not stage"),
    };
    assert_eq!(first_error.limit(), HirLimit::ThreadFlowItems);
    assert_eq!(first_error.maximum(), maximum);
    assert_eq!(first_error.observed(), observed);
    assert_eq!(one_over_database.test_state(), before);
    assert!(one_over_database.current(&one_over_key).is_none());

    let retry_error = match stage_flow(
        &one_over_database,
        &one_over,
        &one_over_key,
        HirLoweringControl::new(),
    ) {
        Err(HirLowerFailure::Limit(error)) => error,
        Err(other) => {
            panic!("one-over Thread-expression retry changed failure boundary: {other:?}")
        }
        Ok(_) => panic!("one-over Thread-expression item retry must not stage"),
    };
    assert_eq!(retry_error, first_error);
    assert_eq!(one_over_database.test_state(), before);
    assert!(one_over_database.current(&one_over_key).is_none());
}
