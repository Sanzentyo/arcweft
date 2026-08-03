use super::*;

use crate::expr::{HirForSyntheticExpr, HirThreadFlowItem, HirThreadIssue};
use crate::identity::StmtId;
use crate::leaf::{HirIdRef, HirIdRefShape, HirIdRefValue};
use crate::source_index::{HirSourceRequirement, HirStmtSourceRole};
use crate::stmt::{
    HirConditionalElseBranch, HirSelectStmt, HirStmt, HirStmtChildRole, HirStmtMatchArmBody,
    HirStmtPoisonState, HirStmtRecoveryIssue, HirThreadStmtBodyRole, HirThreadStmtRecoveryIssue,
    HirUnsafeLifetimeBody,
};

fn block_statement(module: &HirModule, root: ExprId, ordinal: usize) -> (StmtId, &HirStmt) {
    let HirExprKind::Block(block) = expression(module, root).kind() else {
        panic!("fixture root must remain a value block");
    };
    let owner = block.statements()[ordinal];
    let statement = module
        .arenas()
        .statements()
        .resolve(module.slots(), owner)
        .expect("published block statement");
    (owner, statement)
}

fn assert_unsafe_insertion_manifest(
    parsed: &ParsedSource,
    module: &HirModule,
    owner: StmtId,
    requirement: HirSourceRequirement,
    status: HirSourceOwnerStatus,
    present: bool,
) {
    let query = HirSourceQuery::Stmt {
        owner,
        role: HirStmtSourceRole::UnsafeAuditInsertion,
    };
    assert_eq!(
        module.source_components().requirement(&query),
        Some(requirement)
    );
    let lookup = module
        .source_site(parsed.document().identity(), query)
        .expect("unsafe-audit insertion source query");
    assert_eq!(lookup.owner_status(), status);
    match (present, lookup.presence()) {
        (true, HirSourcePresence::Present(HirSourceSite::Insertion(_)))
        | (false, HirSourcePresence::AbsentOptional) => {}
        (_, actual) => panic!("unexpected unsafe-audit insertion presence: {actual:?}"),
    }
}

#[test]
fn thread_control_families_lower_in_exact_source_order() {
    let parsed = parsed_source(
        "thread-control-families",
        &[concat!(
            "thread {\n",
            "    loop {}\n",
            "    while ready {}\n",
            "    while let item = source when allowed {}\n",
            "    for item in source {}\n",
            "    select {\n",
            "        frame frame => {}\n",
            "        event .Back => {}\n",
            "        value = source? => {}\n",
            "    }\n",
            "    try await task with {\n",
            "        pending progress => {}\n",
            "        ready value => {}\n",
            "        error issue => {}\n",
            "        denied reason => {}\n",
            "    }\n",
            "}",
        )
        .into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let HirExprKind::Thread(thread) = expression(&module, owners[0]).kind() else {
        panic!("fixture root must remain a Thread expression");
    };
    assert!(matches!(
        thread.body().items()[0],
        HirThreadFlowItem::Loop(_)
    ));
    assert!(matches!(
        thread.body().items()[1],
        HirThreadFlowItem::While(_)
    ));
    assert!(matches!(
        thread.body().items()[2],
        HirThreadFlowItem::WhileLet(_)
    ));
    assert!(matches!(
        thread.body().items()[3],
        HirThreadFlowItem::For(_)
    ));
    assert!(matches!(
        thread.body().items()[4],
        HirThreadFlowItem::Select(_)
    ));
    assert!(matches!(
        thread.body().items()[5],
        HirThreadFlowItem::AwaitWith(_)
    ));

    let HirThreadFlowItem::For(statement_owner) = thread.body().items()[3] else {
        unreachable!();
    };
    let statement = module
        .arenas()
        .statements()
        .resolve(module.slots(), statement_owner)
        .expect("published For statement");
    let HirStmtKind::For(statement) = statement.kind() else {
        panic!("For item discriminant must match its statement payload");
    };
    assert!(matches!(
        expression(&module, statement.iterator()).kind(),
        HirExprKind::ForSynthetic(HirForSyntheticExpr::Iterator { source })
            if *source == statement.source()
    ));
    assert!(matches!(
        expression(&module, statement.next_value()).kind(),
        HirExprKind::ForSynthetic(HirForSyntheticExpr::NextValue { iterator })
            if *iterator == statement.iterator()
    ));
    for (owner, role) in [
        (statement.iterator(), SyntheticRole::ForIterator),
        (statement.next_value(), SyntheticRole::ForNextValue),
    ] {
        assert!(matches!(
            module.slots().resolve(owner).expect("For synthetic slot").origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Stmt(statement_owner)
                    && key.role() == role
                    && key.ordinal() == 0
        ));
    }
}

#[test]
fn thread_root_poison_is_rederived_from_the_attached_owner() {
    assert_expression_freeze_rejects(
        "thread-root-poison-freeze",
        "thread 1bad { loop {} }",
        |transaction, root| {
            let retained = {
                let (slots, arenas) = transaction.storage_mut();
                arenas
                    .expressions()
                    .resolve_staged(slots, root)
                    .expect("staged Thread expression")
                    .clone()
            };
            let replacement = HirExpr::try_new(
                retained.scope(),
                retained.kind().clone(),
                HirPoisonState::Poisoned(HirRecoveryIssue::InvalidThread(
                    HirThreadIssue::UnclosedBody,
                )),
            )
            .expect("same-module Thread poison substitution");
            let (slots, arenas) = transaction.storage_mut();
            arenas
                .expressions()
                .revise_finalized(slots, root, replacement)
                .expect("test-only Thread poison substitution");
        },
    );
}

#[test]
fn for_synthetic_edges_are_frozen_against_the_statement_owner() {
    assert_expression_freeze_rejects(
        "for-synthetic-edge-freeze",
        "thread { for item in source {} }",
        |transaction, root| {
            let (iterator, next_value) = {
                let (slots, arenas) = transaction.storage_mut();
                let root = arenas
                    .expressions()
                    .resolve_staged(slots, root)
                    .expect("staged Thread expression");
                let HirExprKind::Thread(thread) = root.kind() else {
                    panic!("fixture root must remain Thread");
                };
                let HirThreadFlowItem::For(owner) = thread.body().items()[0] else {
                    panic!("fixture body must retain For");
                };
                let statement = arenas
                    .statements()
                    .resolve_staged(slots, owner)
                    .expect("staged For statement");
                let HirStmtKind::For(statement) = statement.kind() else {
                    panic!("For item must retain its statement payload");
                };
                (statement.iterator(), statement.next_value())
            };
            let retained = {
                let (slots, arenas) = transaction.storage_mut();
                arenas
                    .expressions()
                    .resolve_staged(slots, iterator)
                    .expect("staged For iterator")
                    .clone()
            };
            let replacement = HirExpr::try_new(
                retained.scope(),
                HirExprKind::ForSynthetic(HirForSyntheticExpr::Iterator { source: next_value }),
                retained.state().clone(),
            )
            .expect("same-module For iterator edge substitution");
            let (slots, arenas) = transaction.storage_mut();
            arenas
                .expressions()
                .revise_finalized(slots, iterator, replacement)
                .expect("test-only For iterator edge substitution");
        },
    );
}

#[test]
fn assignment_and_lifetime_set_lower_through_the_typed_statement_owner() {
    let parsed = parsed_source(
        "typed-assignment-statements",
        &["{ marker; target = value; registry <- lease; () }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let (_, assignment) = block_statement(&module, owners[0], 1);
    let HirStmtKind::Assign { target, value } = assignment.kind() else {
        panic!("ordinary assignment must retain its final HIR family");
    };
    assert_eq!(expression(&module, *target).scope(), assignment.scope());
    assert_eq!(expression(&module, *value).scope(), assignment.scope());

    let (_, lifetime_set) = block_statement(&module, owners[0], 2);
    let HirStmtKind::LifetimeSet { target, value } = lifetime_set.kind() else {
        panic!("lifetime set must retain its final HIR family");
    };
    assert_eq!(expression(&module, *target).scope(), lifetime_set.scope());
    assert_eq!(expression(&module, *value).scope(), lifetime_set.scope());
}

#[test]
fn assignment_missing_operands_preserve_family_and_target_first_recovery() {
    let parsed = parsed_source(
        "typed-assignment-recovery",
        &[
            "{ marker; = value; () }".into(),
            "{ marker; target =; () }".into(),
            "{ marker; <- lease; () }".into(),
            "{ marker; registry <-; () }".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    for (position, role) in [
        HirStmtChildRole::Target,
        HirStmtChildRole::Initializer,
        HirStmtChildRole::Target,
        HirStmtChildRole::Initializer,
    ]
    .into_iter()
    .enumerate()
    {
        let (_, statement) = block_statement(&module, owners[position], 1);
        assert!(matches!(
            (position, statement.kind()),
            (0 | 1, HirStmtKind::Assign { .. }) | (2 | 3, HirStmtKind::LifetimeSet { .. })
        ));
        assert_eq!(
            statement.state(),
            &HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild { role })
        );
    }
}

#[test]
fn assignment_source_freeze_rejects_operand_reordering() {
    assert_expression_freeze_rejects(
        "typed-assignment-operand-order",
        "{ marker; target = value; () }",
        |transaction, root| {
            let statement = {
                let (slots, arenas) = transaction.storage_mut();
                let root = arenas
                    .expressions()
                    .resolve_staged(slots, root)
                    .expect("staged assignment block");
                let HirExprKind::Block(block) = root.kind() else {
                    panic!("fixture root must remain a value block");
                };
                block.statements()[1]
            };
            let retained = {
                let (slots, arenas) = transaction.storage_mut();
                arenas
                    .statements()
                    .resolve_staged(slots, statement)
                    .expect("staged assignment payload")
                    .clone()
            };
            let HirStmtKind::Assign { target, value } = retained.kind() else {
                panic!("selected statement must remain an assignment");
            };
            let replacement = HirStmt::try_new_with_state(
                retained.scope(),
                HirStmtKind::Assign {
                    target: *value,
                    value: *target,
                },
                retained.state().clone(),
            )
            .expect("same-module reordered assignment");
            let (slots, arenas) = transaction.storage_mut();
            arenas
                .statements()
                .revise_finalized(slots, statement, replacement)
                .expect("test-only assignment substitution");
        },
    );
}

#[test]
fn required_operand_statements_lower_with_exact_payloads_and_nested_select() {
    let parsed = parsed_source(
        "required-operand-statements",
        &["{ marker; return value; yield @entity.value; wait(target); close resource; select candidate.member; () }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let (_, returned) = block_statement(&module, owners[0], 1);
    assert!(matches!(returned.kind(), HirStmtKind::Return { .. }));
    let (_, yielded) = block_statement(&module, owners[0], 2);
    assert!(matches!(yielded.kind(), HirStmtKind::Yield { .. }));
    let (_, waited) = block_statement(&module, owners[0], 3);
    assert!(matches!(waited.kind(), HirStmtKind::Wait { .. }));
    let (_, closed) = block_statement(&module, owners[0], 4);
    assert!(matches!(closed.kind(), HirStmtKind::Close { .. }));
    let (_, selected) = block_statement(&module, owners[0], 5);
    let HirStmtKind::Select(HirSelectStmt::Operand(selected_member)) = selected.kind() else {
        panic!("ordinary Select statement family");
    };
    assert!(matches!(
        expression(&module, *selected_member).kind(),
        HirExprKind::Select(_)
    ));
}

#[test]
fn required_operand_recovery_preserves_family_and_wait_priority() {
    let parsed = parsed_source(
        "required-operand-recovery",
        &[
            "{ marker; return; () }".into(),
            "{ marker; yield; () }".into(),
            "{ marker; wait(); () }".into(),
            "{ marker; wait target; () }".into(),
            "{ marker; close; () }".into(),
            "{ marker; select; () }".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    for (position, expected_kind, expected_state) in [
        (
            0,
            "return",
            HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::Expression,
            },
        ),
        (
            1,
            "yield",
            HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::Expression,
            },
        ),
        (
            2,
            "wait",
            HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::Target,
            },
        ),
        (3, "wait", HirStmtRecoveryIssue::MalformedWait),
        (
            4,
            "close",
            HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::Target,
            },
        ),
        (
            5,
            "select",
            HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::Expression,
            },
        ),
    ] {
        let (_, statement) = block_statement(&module, owners[position], 1);
        assert!(
            matches!(
                (expected_kind, statement.kind()),
                ("return", HirStmtKind::Return { .. })
                    | ("yield", HirStmtKind::Yield { .. })
                    | ("wait", HirStmtKind::Wait { .. })
                    | ("close", HirStmtKind::Close { .. })
                    | ("select", HirStmtKind::Select(HirSelectStmt::Operand(_)))
            ),
            "unexpected retained family at fixture {position}: {:?}",
            statement.kind()
        );
        assert_eq!(
            statement.state(),
            &HirStmtPoisonState::Poisoned(expected_state)
        );
    }
}

#[test]
fn block_shaped_select_does_not_publish_an_ordinary_statement() {
    for (ordinal, source) in [
        "{ marker; select { marker; }; () }",
        "{ marker; select result { marker; }; () }",
        "{ marker; select scope named { marker; }; () }",
    ]
    .into_iter()
    .enumerate()
    {
        let parsed = parsed_source(&format!("flow-select-freeze-{ordinal}"), &[source.into()]);
        let attached = attached_expressions(&parsed).pop().unwrap();
        let database = HirDatabase::try_new().expect("HIR database");
        let mut transaction = stage(&database, &parsed);
        let scope = allocate_module_scope(&mut transaction, &parsed);
        assert_eq!(
            transaction.lower_attached_expression(&attached, scope),
            Err(HirLowerFailure::Invariant(
                HirInvariantFailure::InvalidArenaCommit
            ))
        );
        assert!(database.current(&module_key(&parsed)).is_none());
    }
}

#[test]
fn required_operand_source_freeze_rejects_statement_family_substitution() {
    assert_expression_freeze_rejects(
        "required-operand-family-substitution",
        "{ marker; return value; () }",
        |transaction, root| {
            let statement = {
                let (slots, arenas) = transaction.storage_mut();
                let root = arenas
                    .expressions()
                    .resolve_staged(slots, root)
                    .expect("staged required-operand block");
                let HirExprKind::Block(block) = root.kind() else {
                    panic!("fixture root must remain a value block");
                };
                block.statements()[1]
            };
            let retained = {
                let (slots, arenas) = transaction.storage_mut();
                arenas
                    .statements()
                    .resolve_staged(slots, statement)
                    .expect("staged Return payload")
                    .clone()
            };
            let HirStmtKind::Return { value } = retained.kind() else {
                panic!("selected statement must remain Return");
            };
            let replacement = HirStmt::try_new_with_state(
                retained.scope(),
                HirStmtKind::Yield { expression: *value },
                retained.state().clone(),
            )
            .expect("same-module statement-family substitution");
            let (slots, arenas) = transaction.storage_mut();
            arenas
                .statements()
                .revise_finalized(slots, statement, replacement)
                .expect("test-only statement-family substitution");
        },
    );
}

#[test]
fn keyword_statements_lower_through_one_typed_owner() {
    let parsed = parsed_source(
        "typed-keyword-statements",
        &["{ out 'exit value; goto next; defer cleanup(); signal changed <- payload; break 'loop result; continue 'loop; () }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let (_, out) = block_statement(&module, owners[0], 0);
    assert!(matches!(
        out.kind(),
        HirStmtKind::Out { label: Some(label), .. } if label.as_str() == "exit"
    ));

    let (_, goto) = block_statement(&module, owners[0], 1);
    assert!(matches!(goto.kind(), HirStmtKind::Goto { .. }));

    let (_, defer) = block_statement(&module, owners[0], 2);
    assert!(matches!(defer.kind(), HirStmtKind::Defer { .. }));

    let (_, signal) = block_statement(&module, owners[0], 3);
    assert!(matches!(signal.kind(), HirStmtKind::Signal { .. }));

    let (_, break_statement) = block_statement(&module, owners[0], 4);
    assert!(matches!(
        break_statement.kind(),
        HirStmtKind::Break {
            label: Some(label),
            value: Some(_),
        } if label.as_str() == "loop"
    ));

    let (_, continue_statement) = block_statement(&module, owners[0], 5);
    assert!(matches!(
        continue_statement.kind(),
        HirStmtKind::Continue { label: Some(label) } if label.as_str() == "loop"
    ));
}

#[test]
fn keyword_statement_recovery_keeps_family_specific_priority() {
    let parsed = parsed_source(
        "typed-keyword-statement-recovery",
        &[
            "{ out 'line.focus; () }".into(),
            "{ out; () }".into(),
            "{ goto; () }".into(),
            "{ defer; () }".into(),
            "{ signal <- value; () }".into(),
            "{ signal target <-; () }".into(),
            "{ signal target; () }".into(),
            "{ signal; () }".into(),
            "{ break; () }".into(),
            "{ break value +; () }".into(),
            "{ break 'line.focus value +; () }".into(),
            "{ continue extra; () }".into(),
            "{ continue 'events? extra; () }".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let expected = [
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::InvalidControlLabel(
            crate::leaf::HirNameInvariantError::InvalidIdentifier,
        )),
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::Initializer,
        }),
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::Target,
        }),
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::Initializer,
        }),
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::Target,
        }),
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::Initializer,
        }),
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::Initializer,
        }),
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::Target,
        }),
        HirStmtPoisonState::Clean,
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::Initializer,
        }),
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::InvalidControlLabel(
            crate::leaf::HirNameInvariantError::InvalidIdentifier,
        )),
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::MalformedContinue),
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::InvalidControlLabel(
            crate::leaf::HirNameInvariantError::InvalidIdentifier,
        )),
    ];
    for (position, expected) in expected.into_iter().enumerate() {
        let (_, statement) = block_statement(&module, owners[position], 0);
        assert_eq!(
            statement.state(),
            &expected,
            "unexpected recovery at fixture {position}: {:?}",
            statement.kind()
        );
    }
}

#[test]
fn keyword_statement_source_freeze_rejects_signal_operand_reordering() {
    assert_expression_freeze_rejects(
        "typed-keyword-signal-order",
        "{ signal target <- value; () }",
        |transaction, root| {
            let statement = {
                let (slots, arenas) = transaction.storage_mut();
                let root = arenas
                    .expressions()
                    .resolve_staged(slots, root)
                    .expect("staged keyword block");
                let HirExprKind::Block(block) = root.kind() else {
                    panic!("fixture root must remain a value block");
                };
                block.statements()[0]
            };
            let retained = {
                let (slots, arenas) = transaction.storage_mut();
                arenas
                    .statements()
                    .resolve_staged(slots, statement)
                    .expect("staged Signal payload")
                    .clone()
            };
            let HirStmtKind::Signal { target, value } = retained.kind() else {
                panic!("selected statement must remain Signal");
            };
            let replacement = HirStmt::try_new_with_state(
                retained.scope(),
                HirStmtKind::Signal {
                    target: *value,
                    value: *target,
                },
                retained.state().clone(),
            )
            .expect("same-module reordered Signal");
            let (slots, arenas) = transaction.storage_mut();
            arenas
                .statements()
                .revise_finalized(slots, statement, replacement)
                .expect("test-only Signal substitution");
        },
    );
}

#[test]
fn unsafe_lifetime_retains_typed_audit_block_and_required_insertion() {
    let parsed = parsed_source(
        "unsafe-lifetime-canonical",
        &[concat!(
            "{\n",
            "  unsafe lifetime @unsafe.audit reason = \"bounded lifetime\" {\n",
            "    /// SAFETY: the test owns the retained value.\n",
            "    let inner = 1;\n",
            "    consume(inner)\n",
            "  };\n",
            "  ()\n",
            "}"
        )
        .into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let (owner, statement) = block_statement(&module, owners[0], 0);
    assert_eq!(statement.state(), &HirStmtPoisonState::Clean);
    let HirStmtKind::UnsafeLifetime { audit, body } = statement.kind() else {
        panic!("unsafe lifetime must retain its dedicated statement payload");
    };
    assert!(matches!(
        audit.id(),
        HirIdRefValue::Resolved(HirIdRef::Absolute(reference))
            if reference.as_str() == "unsafe.audit"
    ));
    assert!(audit.reason().is_some());
    assert!(audit.has_safety_doc());

    let HirUnsafeLifetimeBody::Block { scope, statements } = body else {
        panic!("authored unsafe lifetime body must retain a block");
    };
    assert_eq!(statements.len(), 2);
    let body_scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), *scope)
        .expect("unsafe lifetime body scope");
    assert_eq!(body_scope.kind(), HirScopeKind::Block);
    assert_eq!(body_scope.owner(), &HirScopeOwner::Stmt(owner));
    assert_eq!(body_scope.parent(), Some(statement.scope()));
    assert_eq!(body_scope.locals().len(), 1);

    assert_unsafe_insertion_manifest(
        &parsed,
        &module,
        owner,
        HirSourceRequirement::Required,
        HirSourceOwnerStatus::Clean,
        true,
    );
}

#[test]
fn unsafe_lifetime_omitted_reason_and_safety_policy_do_not_poison_hir() {
    let parsed = parsed_source(
        "unsafe-lifetime-policy-omission",
        &["{ unsafe lifetime @unsafe.audit { value; }; () }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let (owner, statement) = block_statement(&module, owners[0], 0);
    let HirStmtKind::UnsafeLifetime { audit, body } = statement.kind() else {
        panic!("unsafe lifetime payload");
    };
    assert_eq!(statement.state(), &HirStmtPoisonState::Clean);
    assert!(audit.reason().is_none());
    assert!(!audit.has_safety_doc());
    assert!(matches!(body, HirUnsafeLifetimeBody::Block { .. }));
    assert_unsafe_insertion_manifest(
        &parsed,
        &module,
        owner,
        HirSourceRequirement::Required,
        HirSourceOwnerStatus::Clean,
        true,
    );
}

#[test]
fn unsafe_lifetime_missing_identity_wins_over_reason_recovery_and_keeps_body() {
    let parsed = parsed_source(
        "unsafe-lifetime-missing-identity",
        &["{ unsafe lifetime reason { value; }; () }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let (owner, statement) = block_statement(&module, owners[0], 0);
    let HirStmtKind::UnsafeLifetime { audit, body } = statement.kind() else {
        panic!("recovered unsafe lifetime payload");
    };
    assert!(matches!(
        audit.id(),
        HirIdRefValue::Recovered(recovery)
            if recovery.shape() == HirIdRefShape::Missing
                && recovery.issue() == HirIdRefIssue::Missing
    ));
    let reason = audit
        .reason()
        .expect("missing authored reason gets one typed recovery child");
    assert!(matches!(
        module.slots().resolve(reason).expect("reason recovery slot").origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Stmt(owner)
                && key.role() == SyntheticRole::RecoveryOperand
    ));
    assert!(
        matches!(body, HirUnsafeLifetimeBody::Block { statements, .. } if statements.len() == 1)
    );
    assert_eq!(
        statement.state(),
        &HirStmtPoisonState::Poisoned(
            HirStmtRecoveryIssue::InvalidAuditId(HirIdRefIssue::Missing,)
        )
    );
    assert_unsafe_insertion_manifest(
        &parsed,
        &module,
        owner,
        HirSourceRequirement::Optional,
        HirSourceOwnerStatus::Poisoned,
        false,
    );
}

#[test]
fn unsafe_lifetime_missing_body_never_fabricates_a_scope_or_edit_anchor() {
    let parsed = parsed_source(
        "unsafe-lifetime-missing-body",
        &["{ unsafe lifetime @unsafe.audit value; () }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let (owner, statement) = block_statement(&module, owners[0], 0);
    let HirStmtKind::UnsafeLifetime { audit, body } = statement.kind() else {
        panic!("missing-body unsafe lifetime payload");
    };
    assert!(audit.reason().is_none());
    assert_eq!(body, &HirUnsafeLifetimeBody::Missing);
    assert_eq!(body.scope(), None);
    assert!(body.statements().is_empty());
    assert_eq!(
        statement.state(),
        &HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::MissingBody)
    );
    assert_unsafe_insertion_manifest(
        &parsed,
        &module,
        owner,
        HirSourceRequirement::Optional,
        HirSourceOwnerStatus::Poisoned,
        false,
    );
}

#[test]
fn unsafe_lifetime_body_recovery_retains_order_and_first_poisoned_ordinal() {
    let parsed = parsed_source(
        "unsafe-lifetime-body-recovery",
        &["{ unsafe lifetime @unsafe.audit { let missing =; consume(missing) }; () }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let (owner, statement) = block_statement(&module, owners[0], 0);
    let HirStmtKind::UnsafeLifetime { body, .. } = statement.kind() else {
        panic!("recovered unsafe lifetime payload");
    };
    assert!(
        matches!(body, HirUnsafeLifetimeBody::Block { statements, .. } if statements.len() == 2)
    );
    assert_eq!(
        statement.state(),
        &HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::BodyStatement { ordinal: 0 },
        })
    );
    assert_unsafe_insertion_manifest(
        &parsed,
        &module,
        owner,
        HirSourceRequirement::Optional,
        HirSourceOwnerStatus::Poisoned,
        false,
    );
}

#[test]
fn unsafe_lifetime_unclosed_body_retains_block_and_prior_child_recovery() {
    let parsed = parsed_source(
        "unsafe-lifetime-unclosed-body",
        &["{ unsafe lifetime @unsafe.audit { value; ".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let (owner, statement) = block_statement(&module, owners[0], 0);
    let HirStmtKind::UnsafeLifetime {
        body: HirUnsafeLifetimeBody::Block { statements, .. },
        ..
    } = statement.kind()
    else {
        panic!("unclosed unsafe lifetime payload");
    };
    assert_eq!(statements.len(), 2);
    let trailing_recovery = module
        .arenas()
        .statements()
        .resolve(module.slots(), statements[1])
        .expect("trailing unclosed-body recovery statement");
    assert!(matches!(trailing_recovery.kind(), HirStmtKind::Error));
    assert_eq!(
        statement.state(),
        &HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::BodyStatement { ordinal: 1 },
        })
    );
    assert_unsafe_insertion_manifest(
        &parsed,
        &module,
        owner,
        HirSourceRequirement::Optional,
        HirSourceOwnerStatus::Poisoned,
        false,
    );
}

#[test]
fn statement_if_let_owns_scoped_bindings_statement_blocks_and_nested_else_if() {
    let parsed = parsed_source(
        "statement-if-let-owner",
        &[concat!(
            "{ ",
            "if let .Some(value) = maybe when ready { ",
            "let inside = value; consume(inside) ",
            "} else if ready { fallback() } else { final_fallback() }; ",
            "() ",
            "}"
        )
        .into()],
    );
    let (module, owners, attached) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let HirExprKind::Block(block) = expression(&module, owners[0]).kind() else {
        panic!("fixture root must remain a value block");
    };
    assert_eq!(block.statements().len(), 1);
    let statement_id = block.statements()[0];
    let statement = module
        .arenas()
        .statements()
        .resolve(module.slots(), statement_id)
        .expect("published if-let statement");
    let HirStmtKind::IfLet(if_let) = statement.kind() else {
        panic!("statement-form if-let must retain its dedicated HIR payload");
    };

    let outer_scope = block.scope();
    let outer = module
        .arenas()
        .scopes()
        .resolve(module.slots(), outer_scope)
        .expect("outer value-block scope");
    assert!(outer.locals().is_empty(), "if-let bindings must not escape");
    assert_eq!(expression(&module, if_let.scrutinee()).scope(), outer_scope);

    let then_scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), if_let.then_scope())
        .expect("if-let then scope");
    assert_eq!(then_scope.kind(), HirScopeKind::Conditional);
    assert_eq!(then_scope.parent(), Some(outer_scope));
    assert_eq!(then_scope.owner(), &HirScopeOwner::Stmt(statement_id));
    assert_eq!(if_let.locals().len(), 1);
    assert_eq!(then_scope.locals().len(), 2);
    assert_eq!(then_scope.locals()[0], if_let.locals()[0]);
    let then_statements = if_let
        .then_body()
        .ordinary_statements()
        .expect("ordinary if-let statement body");
    assert_eq!(then_statements.len(), 2);

    let pattern = module
        .arenas()
        .patterns()
        .resolve(module.slots(), if_let.pattern())
        .expect("if-let pattern");
    assert_eq!(pattern.scope(), if_let.then_scope());
    let pattern_local = module
        .arenas()
        .locals()
        .resolve(module.slots(), if_let.locals()[0])
        .expect("if-let pattern local");
    assert_eq!(pattern_local.scope(), if_let.then_scope());
    assert_eq!(pattern_local.kind(), HirLocalKind::PatternBinding);
    assert_eq!(pattern_local.pattern(), Some(if_let.pattern()));
    assert_eq!(
        expression(&module, if_let.guard().expect("authored guard")).scope(),
        if_let.then_scope()
    );

    let final_then_statement = module
        .arenas()
        .statements()
        .resolve(module.slots(), then_statements[1])
        .expect("final then expression statement");
    assert!(matches!(
        final_then_statement.kind(),
        HirStmtKind::Expression { expression: child }
            if expression(&module, *child).scope() == if_let.then_scope()
    ));

    let Some(HirConditionalElseBranch::ElseIf(nested_id)) = if_let.else_branch() else {
        panic!("authored else-if must retain a separate statement identity");
    };
    let nested = module
        .arenas()
        .statements()
        .resolve(module.slots(), *nested_id)
        .expect("nested ordinary if statement");
    assert_eq!(nested.scope(), outer_scope);
    let HirStmtKind::If(nested_if) = nested.kind() else {
        panic!("nested else-if must retain its ordinary typed If payload");
    };
    let nested_then_statements = nested_if
        .then_body()
        .ordinary_statements()
        .expect("ordinary nested then body");
    let Some(HirConditionalElseBranch::Body(nested_else_body)) = nested_if.else_branch() else {
        panic!("nested ordinary if must retain its terminal else body");
    };
    let nested_else_statements = nested_else_body
        .ordinary_statements()
        .expect("ordinary nested else body");
    assert_eq!(
        expression(&module, nested_if.condition()).scope(),
        outer_scope
    );
    assert_eq!(nested_then_statements.len(), 1);
    assert_eq!(nested_else_statements.len(), 1);
    for branch_scope in [nested_if.then_scope(), nested_else_body.scope()] {
        let branch = module
            .arenas()
            .scopes()
            .resolve(module.slots(), branch_scope)
            .expect("nested branch scope");
        assert_eq!(branch.kind(), HirScopeKind::Conditional);
        assert_eq!(branch.parent(), Some(outer_scope));
        assert_eq!(branch.owner(), &HirScopeOwner::Stmt(*nested_id));
    }

    let outer_attached = &attached[0];
    let outer_block = outer_attached.block().expect("attached outer block");
    let conditional = outer_block
        .statements()
        .expect("attached outer statements")
        .into_iter()
        .next()
        .expect("attached if-let")
        .cast::<arcweft_lang_syntax::attachment::node::IfStatementKind>()
        .expect("exact attached if-let");
    assert!(
        conditional
            .then_branch()
            .expect("attached then block")
            .optional_tail()
            .expect("statement-block tail access")
            .is_none()
    );
}

#[test]
fn statement_if_let_retains_nested_if_let_terminal_else_and_omitted_else() {
    let parsed = parsed_source(
        "statement-if-let-else-shapes",
        &[concat!(
            "{ ",
            "if let first = 1 { consume(first) } ",
            "else if let second = 2 { consume(second) } ",
            "else { fallback() }; ",
            "if let lone = 3 { consume(lone) }; ",
            "() ",
            "}"
        )
        .into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let HirExprKind::Block(block) = expression(&module, owners[0]).kind() else {
        panic!("fixture root must remain a value block");
    };
    assert_eq!(block.statements().len(), 2);

    let first = module
        .arenas()
        .statements()
        .resolve(module.slots(), block.statements()[0])
        .expect("outer if-let");
    let HirStmtKind::IfLet(first) = first.kind() else {
        panic!("outer statement must retain IfLet");
    };
    let Some(HirConditionalElseBranch::ElseIf(nested_id)) = first.else_branch() else {
        panic!("nested if-let must retain a typed statement ID");
    };
    let nested = module
        .arenas()
        .statements()
        .resolve(module.slots(), *nested_id)
        .expect("nested if-let");
    let HirStmtKind::IfLet(nested) = nested.kind() else {
        panic!("nested statement must retain IfLet");
    };
    let Some(HirConditionalElseBranch::Body(body)) = nested.else_branch() else {
        panic!("terminal else must retain its source-backed block scope");
    };
    assert_eq!(
        body.ordinary_statements()
            .expect("ordinary terminal else body")
            .len(),
        1
    );
    let terminal_scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), body.scope())
        .expect("terminal else scope");
    assert_eq!(terminal_scope.kind(), HirScopeKind::Conditional);
    assert_eq!(terminal_scope.parent(), Some(block.scope()));
    assert_eq!(terminal_scope.owner(), &HirScopeOwner::Stmt(*nested_id));

    let omitted = module
        .arenas()
        .statements()
        .resolve(module.slots(), block.statements()[1])
        .expect("omitted-else if-let");
    let HirStmtKind::IfLet(omitted) = omitted.kind() else {
        panic!("second statement must retain IfLet");
    };
    assert!(omitted.else_branch().is_none());
}

#[test]
fn statement_if_let_freeze_rejects_pattern_to_body_generation_tampering() {
    assert_expression_local_freeze_rejects(
        "statement-if-let-local-generation",
        "{ if let value = 1 { let value = 2; consume(value) }; () }",
        |transaction, root| {
            let (slots, arenas) = transaction.storage_mut();
            let statement_id = {
                let root = arenas.expressions().resolve_staged(slots, root).unwrap();
                let HirExprKind::Block(block) = root.kind() else {
                    panic!("staged block owner")
                };
                block.statements()[0]
            };
            let body_statement = {
                let statement = arenas
                    .statements()
                    .resolve_staged(slots, statement_id)
                    .unwrap();
                let HirStmtKind::IfLet(if_let) = statement.kind() else {
                    panic!("staged statement IfLet owner")
                };
                if_let
                    .then_body()
                    .ordinary_statements()
                    .expect("ordinary staged if-let body")[0]
            };
            let body_let = arenas
                .statements()
                .resolve_staged(slots, body_statement)
                .unwrap();
            let HirStmtKind::Let { locals, .. } = body_let.kind() else {
                panic!("staged IfLet body Let")
            };
            locals[0]
        },
        LocalPayloadTamper::Generation(LocalGeneration::FIRST),
    );
}

#[test]
fn statement_match_owns_source_ordered_arms_scopes_and_locals() {
    let parsed = parsed_source(
        "statement-match-owner",
        &[concat!(
            "{ ",
            "match subject { ",
            "value when ready => consume(value), ",
            "_ => { let inner = 1; consume(inner) } ",
            "}; ",
            "() ",
            "}"
        )
        .into()],
    );
    let attached_root = attached_expressions(&parsed)
        .into_iter()
        .next()
        .expect("attached value-block root");
    let attached_match = attached_root
        .block()
        .expect("attached value block")
        .statements()
        .expect("attached value-block statements")
        .into_iter()
        .next()
        .expect("attached Match statement")
        .cast::<arcweft_lang_syntax::attachment::node::MatchStatementKind>()
        .expect("exact attached Match statement");
    let attached_body = attached_match
        .body_or_missing()
        .expect("typed attached Match body");
    let attached_arms = attached_body.arms().expect("typed attached Match arms");
    assert_eq!(attached_arms.len(), 2);
    for arm in &attached_arms {
        arm.pattern()
            .expect("typed Match arm pattern")
            .semantic()
            .expect("semantic Match arm pattern");
        if let Some(guard) = arm.guard().expect("typed Match arm guard") {
            match guard {
                arcweft_lang_syntax::attachment::MatchStatementExpressionNode::Expression(
                    expression,
                ) => {
                    expression
                        .semantic()
                        .expect("semantic Match arm guard expression");
                }
                arcweft_lang_syntax::attachment::MatchStatementExpressionNode::Missing(_) => {
                    panic!("canonical Match guard must be authored");
                }
            }
        }
        arm.body().expect("typed Match arm body");
    }
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let HirExprKind::Block(block) = expression(&module, owners[0]).kind() else {
        panic!("fixture root must remain a value block");
    };
    let (owner, statement) = block_statement(&module, owners[0], 0);
    assert_eq!(statement.state(), &HirStmtPoisonState::Clean);
    let HirStmtKind::Match(matched) = statement.kind() else {
        panic!("statement-form Match must retain its dedicated HIR payload");
    };
    let arms = matched.arms();
    assert_eq!(
        expression(&module, matched.scrutinee()).scope(),
        block.scope()
    );
    assert_eq!(arms.len(), 2);

    let first = &arms[0];
    let first_scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), first.scope())
        .expect("first Match arm scope");
    assert_eq!(first_scope.kind(), HirScopeKind::MatchArm);
    assert_eq!(first_scope.parent(), Some(block.scope()));
    assert_eq!(first_scope.owner(), &HirScopeOwner::Stmt(owner));
    assert_eq!(first.locals().len(), 1);
    assert_eq!(first_scope.locals(), first.locals());
    let first_local = module
        .arenas()
        .locals()
        .resolve(module.slots(), first.locals()[0])
        .expect("first Match binding");
    assert_eq!(first_local.scope(), first.scope());
    assert_eq!(first_local.kind(), HirLocalKind::MatchBinding);
    assert_eq!(first_local.pattern(), Some(first.pattern()));
    assert_eq!(
        expression(&module, first.guard().expect("authored Match guard")).scope(),
        first.scope()
    );
    let HirStmtMatchArmBody::Expression(first_body) = first.body() else {
        panic!("first Match arm must retain an expression body");
    };
    assert_eq!(expression(&module, *first_body).scope(), first.scope());

    let second = &arms[1];
    let second_scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), second.scope())
        .expect("second Match arm scope");
    assert_eq!(second_scope.kind(), HirScopeKind::MatchArm);
    assert_eq!(second_scope.parent(), Some(block.scope()));
    assert_eq!(second_scope.owner(), &HirScopeOwner::Stmt(owner));
    assert!(second.locals().is_empty());
    assert_eq!(second_scope.locals().len(), 1);
    let HirStmtMatchArmBody::Body(second_body) = second.body() else {
        panic!("second Match arm must retain a statement block");
    };
    assert_eq!(
        second_body
            .ordinary_statements()
            .expect("ordinary second Match arm body")
            .len(),
        2
    );
    assert_ne!(first.scope(), second.scope());
}

#[test]
fn statement_match_freeze_rejects_pattern_to_body_generation_tampering() {
    assert_expression_source_freeze_rejects(
        "statement-match-local-generation",
        "{ match subject { value => { let value = 2; consume(value) } }; () }",
        |transaction, root| {
            let (slots, arenas) = transaction.storage_mut();
            let statement_id = {
                let root = arenas.expressions().resolve_staged(slots, root).unwrap();
                let HirExprKind::Block(block) = root.kind() else {
                    panic!("staged block owner")
                };
                block.statements()[0]
            };
            let body_statement = {
                let statement = arenas
                    .statements()
                    .resolve_staged(slots, statement_id)
                    .unwrap();
                let HirStmtKind::Match(matched) = statement.kind() else {
                    panic!("staged statement Match owner")
                };
                let HirStmtMatchArmBody::Body(body) = matched.arms()[0].body() else {
                    panic!("staged Match arm statement block")
                };
                body.ordinary_statements()
                    .expect("ordinary staged Match arm body")[0]
            };
            let body_let = arenas
                .statements()
                .resolve_staged(slots, body_statement)
                .unwrap();
            let HirStmtKind::Let { locals, .. } = body_let.kind() else {
                panic!("staged Match arm body Let")
            };
            let body_local = locals[0];
            tamper_local_payload(
                transaction,
                body_local,
                LocalPayloadTamper::Generation(LocalGeneration::FIRST),
            );
        },
    );
}

#[test]
fn statement_match_missing_scrutinee_and_guard_keep_distinct_stmt_recovery_slots() {
    let parsed = parsed_source(
        "statement-match-missing-heads",
        &["{ match { value when => result }; () }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let (owner, statement) = block_statement(&module, owners[0], 0);
    let HirStmtKind::Match(matched) = statement.kind() else {
        panic!("recovered Match statement payload");
    };
    let arms = matched.arms();
    assert_eq!(arms.len(), 1);
    assert_eq!(
        statement.state(),
        &HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::Scrutinee,
        })
    );
    assert!(matches!(
        module
            .slots()
            .resolve(matched.scrutinee())
            .expect("missing scrutinee slot")
            .origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Stmt(owner)
                && key.role() == SyntheticRole::RecoveryOperand
                && key.ordinal() == 0
    ));

    let guard = arms[0].guard().expect("recognized missing guard slot");
    assert!(matches!(
        module.slots().resolve(guard).expect("missing guard slot").origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Stmt(owner)
                && key.role() == SyntheticRole::RecoveryOperand
                && key.ordinal() == 1
    ));
    assert_eq!(expression(&module, guard).scope(), arms[0].scope());
}

#[test]
fn statement_match_missing_body_uses_scope_owned_required_tail() {
    let parsed = parsed_source(
        "statement-match-missing-arm-body",
        &["{ match subject { value => }; () }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let (_, statement) = block_statement(&module, owners[0], 0);
    let HirStmtKind::Match(matched) = statement.kind() else {
        panic!("recovered Match statement payload");
    };
    let arms = matched.arms();
    assert_eq!(arms.len(), 1);
    assert_eq!(
        statement.state(),
        &HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::MatchArmBody { arm: 0 },
        })
    );
    let HirStmtMatchArmBody::Expression(body) = arms[0].body() else {
        panic!("missing arm body must retain one typed expression recovery");
    };
    assert!(matches!(
        module
            .slots()
            .resolve(*body)
            .expect("missing arm body slot")
            .origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Scope(arms[0].scope())
                && key.role() == SyntheticRole::MissingRequiredTail
                && key.ordinal() == 0
    ));
    assert_eq!(expression(&module, *body).scope(), arms[0].scope());
}

#[test]
fn statement_match_missing_braced_body_remains_typed_and_poisoned() {
    let parsed = parsed_source(
        "statement-match-missing-body",
        &["{ match subject; () }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let (_, statement) = block_statement(&module, owners[0], 0);
    let HirStmtKind::Match(matched) = statement.kind() else {
        panic!("missing-body Match must retain its recognized family");
    };
    assert!(matched.arms().is_empty());
    assert_eq!(
        statement.state(),
        &HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::Thread(
            HirThreadStmtRecoveryIssue::MissingBody {
                role: HirThreadStmtBodyRole::Match,
            }
        ))
    );
}
