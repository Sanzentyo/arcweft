use super::*;
use crate::dialogue_application::HirPostfixBracketCandidates;
use crate::expr::{HirCallCallee, HirComputationBlockKind, HirNamedBlockName};
use crate::stmt::{
    HirAssertionMode, HirConditionalElseBranch, HirStmtChildRole, HirStmtMatchArmBody,
    HirStmtPoisonState, HirStmtRecoveryIssue, HirThreadStmtBodyRole, HirThreadStmtRecoveryIssue,
    HirUnsafeAuditIdentity, HirUnsafeAuditIdentityIssue, HirUnsafeLifetimeBody,
};
use arcweft_lang_syntax::assertion::AssertionMode;
use arcweft_lang_syntax::ast::line_plan::DeferOutcome;

fn index_candidate(module: &HirModule, owner: ExprId) -> (ExprId, &crate::expr::HirIndexExpr) {
    let HirExprKind::PostfixBracket(postfix) = expression(module, owner).kind() else {
        panic!("fixture root must remain the ambiguous E34 postfix");
    };
    let HirPostfixBracketCandidates::Ambiguous { index, .. } = postfix.candidates() else {
        panic!("ordinary index interpretation must remain typed");
    };
    let HirExprKind::Index(index_payload) = expression(module, *index).kind() else {
        panic!("ordinary interpretation must retain its Index root");
    };
    (*index, index_payload)
}

fn assert_candidate_origin<I: HirTypedId + std::fmt::Debug>(
    module: &HirModule,
    id: I,
    outer: ExprId,
    ordinal: u32,
) {
    let metadata = module.slots().resolve(id).expect("candidate slot metadata");
    assert!(
        matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Expr(outer)
                    && key.role() == SyntheticRole::PostfixIndexCandidateExpression
                    && key.ordinal() == ordinal
        ),
        "unexpected candidate origin for {id:?}: {:?}",
        metadata.origin()
    );
}

fn candidate_statement(
    module: &HirModule,
    outer: ExprId,
    ordinal: usize,
) -> (crate::identity::StmtId, &crate::stmt::HirStmt) {
    let (_, index) = index_candidate(module, outer);
    let HirExprKind::Block(block) = expression(module, index.index()).kind() else {
        panic!("candidate primary must remain a Block");
    };
    let owner = block.statements()[ordinal];
    let statement = module
        .arenas()
        .statements()
        .resolve(module.slots(), owner)
        .expect("candidate statement payload");
    (owner, statement)
}

fn associated_call_receiver(module: &HirModule, expression_id: ExprId) -> crate::identity::TypeId {
    let HirExprKind::Call(call) = expression(module, expression_id).kind() else {
        panic!("keyword payload must remain an associated Call");
    };
    let (receiver, _, member) = call
        .callee()
        .associated_parts()
        .expect("typed associated callee");
    assert_eq!(
        member.resolved().map(crate::leaf::HirName::as_str),
        Some("with_capacity")
    );
    receiver.type_id().expect("associated receiver type")
}

#[test]
fn value_block_candidate_owns_ordered_statements_locals_and_tail() {
    let parsed = parsed_source(
        "dialogue-candidate-value-block",
        &["items[{ let value = 1; value }]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let outer = owners[0];
    let (_, index) = index_candidate(&module, outer);
    let block_id = index.index();
    let HirExprKind::Block(block) = expression(&module, block_id).kind() else {
        panic!("candidate primary must retain its value Block");
    };
    let [statement_id] = block.statements() else {
        panic!("candidate Block must retain one source-ordered statement");
    };
    let statement = module
        .arenas()
        .statements()
        .resolve(module.slots(), *statement_id)
        .expect("candidate Let statement");
    let HirStmtKind::Let {
        pattern,
        initializer,
        locals,
        ..
    } = statement.kind()
    else {
        panic!("candidate statement must retain Let semantics");
    };
    assert_eq!(locals.len(), 1);
    let scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), block.scope())
        .expect("candidate Block scope");
    assert_eq!(scope.kind(), HirScopeKind::Block);
    assert_eq!(scope.owner(), &HirScopeOwner::Expr(block_id));
    assert_eq!(scope.locals(), locals.as_ref());
    assert_eq!(statement.scope(), block.scope());
    assert_eq!(expression(&module, *initializer).scope(), block.scope());
    assert_eq!(expression(&module, block.tail()).scope(), block.scope());

    assert_candidate_origin(&module, block_id, outer, 1);
    assert_candidate_origin(&module, *initializer, outer, 2);
    assert_candidate_origin(&module, block.tail(), outer, 3);
    assert_candidate_origin(&module, *statement_id, outer, 0);
    assert_candidate_origin(&module, *pattern, outer, 0);
    assert_candidate_origin(&module, block.scope(), outer, 0);
    assert_candidate_origin(&module, locals[0], outer, 0);
}

#[test]
fn candidate_let_else_owns_failure_scope_and_success_binding() {
    let parsed = parsed_source(
        "dialogue-candidate-let-else",
        &["items[{ let Some(value) = Some(source) else { return fallback; }; value }]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let outer = owners[0];
    let (statement_id, statement) = candidate_statement(&module, outer, 0);
    let HirStmtKind::LetElse {
        else_scope,
        else_body,
        locals,
        ..
    } = statement.kind()
    else {
        panic!("candidate statement must retain LetElse semantics")
    };
    assert_eq!(locals.len(), 1);
    assert_eq!(else_body.len(), 1);
    let scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), *else_scope)
        .expect("candidate LetElse failure scope");
    assert_eq!(scope.kind(), HirScopeKind::Block);
    assert_eq!(scope.owner(), &HirScopeOwner::Stmt(statement_id));
    assert!(scope.locals().is_empty());
}

#[test]
fn computation_and_named_candidates_use_final_tail_policy() {
    let parsed = parsed_source(
        "dialogue-candidate-block-families",
        &[
            "items[result { marker; }]".into(),
            "items[option { marker; }]".into(),
            "items[seq { }]".into(),
            "items[stream { }]".into(),
            "items[scope retry { marker }]".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    for (position, expected) in [
        HirComputationBlockKind::Result,
        HirComputationBlockKind::Option,
        HirComputationBlockKind::Seq,
        HirComputationBlockKind::Stream,
    ]
    .into_iter()
    .enumerate()
    {
        let (_, index) = index_candidate(&module, owners[position]);
        let HirExprKind::ComputationBlock(block) = expression(&module, index.index()).kind() else {
            panic!("candidate must retain its computation-block family");
        };
        assert_eq!(block.kind(), expected);
        match expected {
            HirComputationBlockKind::Result | HirComputationBlockKind::Option => assert_eq!(
                expression(&module, block.tail()).state(),
                &HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail)
            ),
            HirComputationBlockKind::Seq | HirComputationBlockKind::Stream => assert!(matches!(
                (
                    expression(&module, block.tail()).kind(),
                    expression(&module, block.tail()).state()
                ),
                (HirExprKind::Unit, HirPoisonState::Clean)
            )),
        }
    }

    let (_, index) = index_candidate(&module, owners[4]);
    let HirExprKind::NamedBlock(block) = expression(&module, index.index()).kind() else {
        panic!("candidate must retain its named-block family");
    };
    assert!(matches!(
        block.name(),
        HirNamedBlockName::Resolved(name) if name.as_str() == "retry"
    ));
}

#[test]
fn candidate_assertion_and_if_let_keep_statement_preorder_and_scopes() {
    let parsed = parsed_source(
        "dialogue-candidate-assertion-if-let",
        &[
            "items[{ assert.check(true, false); marker }]".into(),
            "items[{ if let value = source when ready { value; } else if fallback { marker; }; marker }]"
                .into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let (_, assertion_index) = index_candidate(&module, owners[0]);
    let HirExprKind::Block(assertion_block) = expression(&module, assertion_index.index()).kind()
    else {
        panic!("assertion candidate must remain a Block");
    };
    let [assertion] = assertion_block.statements() else {
        panic!("one assertion statement");
    };
    let assertion = module
        .arenas()
        .statements()
        .resolve(module.slots(), *assertion)
        .expect("candidate assertion");
    assert!(matches!(
        assertion.kind(),
        HirStmtKind::Assertion {
            mode: HirAssertionMode::Resolved(AssertionMode::Check),
            conditions,
        } if conditions.len() == 2
    ));

    let (_, conditional_index) = index_candidate(&module, owners[1]);
    let HirExprKind::Block(conditional_block) =
        expression(&module, conditional_index.index()).kind()
    else {
        panic!("if-let candidate must remain a Block");
    };
    let [conditional] = conditional_block.statements() else {
        panic!("one if-let statement");
    };
    let conditional_payload = module
        .arenas()
        .statements()
        .resolve(module.slots(), *conditional)
        .expect("candidate if-let");
    let HirStmtKind::IfLet(if_let) = conditional_payload.kind() else {
        panic!("dedicated if-let HIR");
    };
    let Some(HirConditionalElseBranch::ElseIf(nested)) = if_let.else_branch() else {
        panic!("else-if must retain a nested StmtId");
    };
    let nested_payload = module
        .arenas()
        .statements()
        .resolve(module.slots(), *nested)
        .expect("nested candidate if");
    assert!(matches!(nested_payload.kind(), HirStmtKind::If(_)));
    let then_scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), if_let.then_scope())
        .expect("if-let then scope");
    assert_eq!(then_scope.owner(), &HirScopeOwner::Stmt(*conditional));

    assert_candidate_origin(&module, *conditional, owners[1], 0);
    let then_statements = if_let
        .then_body()
        .ordinary_statements()
        .expect("candidate if-let must retain an ordinary body");
    assert_candidate_origin(&module, then_statements[0], owners[1], 1);
    assert_candidate_origin(&module, *nested, owners[1], 2);
}

#[test]
fn candidate_match_and_unsafe_keep_arm_and_audit_ownership() {
    let parsed = parsed_source(
        "dialogue-candidate-match-unsafe",
        &[
            "items[{ match subject { value when ready => value, _ => { marker; } }; marker }]"
                .into(),
            "items[{ unsafe lifetime @unsafe.audit reason = \"bounded\" { /// SAFETY: owned\n marker; }; marker }]"
                .into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let (_, match_index) = index_candidate(&module, owners[0]);
    let HirExprKind::Block(match_block) = expression(&module, match_index.index()).kind() else {
        panic!("Match candidate must remain a Block");
    };
    let [matched] = match_block.statements() else {
        panic!("one Match statement");
    };
    let matched_payload = module
        .arenas()
        .statements()
        .resolve(module.slots(), *matched)
        .expect("candidate Match");
    let HirStmtKind::Match(match_statement) = matched_payload.kind() else {
        panic!("typed Match statement");
    };
    let [first, second] = match_statement.arms() else {
        panic!("two typed Match arms");
    };
    assert!(matches!(first.body(), HirStmtMatchArmBody::Expression(_)));
    assert!(matches!(
        second.body(),
        HirStmtMatchArmBody::Body(body)
            if body.ordinary_statements().is_some_and(|statements| statements.len() == 1)
    ));
    for arm in match_statement.arms() {
        let scope = module
            .arenas()
            .scopes()
            .resolve(module.slots(), arm.scope())
            .expect("candidate Match arm scope");
        assert_eq!(scope.owner(), &HirScopeOwner::Stmt(*matched));
    }

    let (_, unsafe_index) = index_candidate(&module, owners[1]);
    let HirExprKind::Block(unsafe_block) = expression(&module, unsafe_index.index()).kind() else {
        panic!("unsafe candidate must remain a Block");
    };
    let [audit_statement] = unsafe_block.statements() else {
        panic!("one unsafe statement");
    };
    let audit_payload = module
        .arenas()
        .statements()
        .resolve(module.slots(), *audit_statement)
        .expect("candidate unsafe statement");
    let HirStmtKind::UnsafeLifetime { audit, body } = audit_payload.kind() else {
        panic!("typed unsafe-lifetime statement");
    };
    assert!(audit.has_safety_doc());
    assert!(audit.reason().is_some());
    let HirUnsafeLifetimeBody::Block { scope, statements } = body else {
        panic!("authored unsafe body");
    };
    assert_eq!(statements.len(), 1);
    let scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), *scope)
        .expect("unsafe body scope");
    assert_eq!(scope.owner(), &HirScopeOwner::Stmt(*audit_statement));
}

#[test]
fn candidate_statement_recovery_preserves_typed_families_and_priority() {
    let parsed = parsed_source(
        "dialogue-candidate-statement-recovery",
        &[
            "items[{ assert.assume(); marker }]".into(),
            "items[{ match subject; marker }]".into(),
            "items[{ match subject { value => }; marker }]".into(),
            "items[{ unsafe lifetime reason { marker; }; marker }]".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let (_, assertion) = candidate_statement(&module, owners[0], 0);
    assert!(matches!(
        assertion.kind(),
        HirStmtKind::Assertion {
            mode: HirAssertionMode::Recovered,
            conditions,
        } if conditions.is_empty()
    ));
    assert_eq!(
        assertion.state(),
        &HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::InvalidAssertionMode)
    );

    let (_, missing_match) = candidate_statement(&module, owners[1], 0);
    assert!(matches!(
        missing_match.kind(),
        HirStmtKind::Match(statement) if statement.arms().is_empty()
    ));
    assert_eq!(
        missing_match.state(),
        &HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::Thread(
            HirThreadStmtRecoveryIssue::MissingBody {
                role: HirThreadStmtBodyRole::Match,
            }
        ))
    );

    let (_, missing_arm) = candidate_statement(&module, owners[2], 0);
    let HirStmtKind::Match(matched) = missing_arm.kind() else {
        panic!("recovered candidate Match payload");
    };
    let [arm] = matched.arms() else {
        panic!("one recovered candidate Match arm");
    };
    let HirStmtMatchArmBody::Expression(body) = arm.body() else {
        panic!("missing candidate arm body must remain an expression recovery");
    };
    assert_eq!(
        expression(&module, *body).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail)
    );
    assert_eq!(
        missing_arm.state(),
        &HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::MatchArmBody { arm: 0 },
        })
    );

    let (_, unsafe_statement) = candidate_statement(&module, owners[3], 0);
    let HirStmtKind::UnsafeLifetime { audit, body } = unsafe_statement.kind() else {
        panic!("recovered candidate unsafe-lifetime payload");
    };
    assert!(matches!(
        audit.identity(),
        HirUnsafeAuditIdentity::Recovered(HirUnsafeAuditIdentityIssue::Missing)
    ));
    assert!(matches!(body, HirUnsafeLifetimeBody::Block { .. }));
    assert_eq!(
        unsafe_statement.state(),
        &HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::InvalidAuditId(
            HirUnsafeAuditIdentityIssue::Missing,
        ))
    );
}

#[test]
fn candidate_unsafe_identity_uses_the_closed_absolute_projection() {
    let cases = [
        ("@.audit", HirUnsafeAuditIdentityIssue::NonAbsolute),
        ("@unsafe:.audit", HirUnsafeAuditIdentityIssue::NonAbsolute),
        ("@proof.audit", HirUnsafeAuditIdentityIssue::WrongFamily),
        ("@unsafe.", HirUnsafeAuditIdentityIssue::InvalidReference),
    ];

    for (ordinal, (reference, expected)) in cases.into_iter().enumerate() {
        let parsed = parsed_source(
            &format!("dialogue-candidate-invalid-unsafe-id-{ordinal}"),
            &[format!(
                "items[{{ unsafe lifetime {reference} {{ marker; }}; marker }}]"
            )],
        );
        let (module, owners, _) = lower_and_publish(&parsed);
        let (_, statement) = candidate_statement(&module, owners[0], 0);
        let HirStmtKind::UnsafeLifetime { audit, .. } = statement.kind() else {
            panic!("invalid candidate unsafe identity must retain its statement family");
        };
        assert_eq!(
            audit.identity(),
            &HirUnsafeAuditIdentity::Recovered(expected)
        );
        assert_eq!(
            statement.state(),
            &HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::InvalidAuditId(expected))
        );
    }
}

#[test]
fn candidate_assertion_exact_condition_limit_commits_without_duplicate_accounting() {
    let conditions = (0..HirLimit::AssertionConditions.maximum())
        .map(|ordinal| format!("condition_{ordinal}"))
        .collect::<Vec<_>>()
        .join(", ");
    let parsed = parsed_source(
        "dialogue-candidate-assertion-exact-limit",
        &[format!("items[{{ assert.check({conditions}); marker }}]")],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);
    let (_, assertion) = candidate_statement(&module, owners[0], 0);
    assert!(matches!(
        assertion.kind(),
        HirStmtKind::Assertion { conditions, .. }
            if conditions.len() == HirLimit::AssertionConditions.maximum()
    ));
}

#[test]
fn candidate_assignment_and_lifetime_set_keep_typed_operands_and_preorder() {
    let parsed = parsed_source(
        "dialogue-candidate-assignment-statements",
        &["items[{ marker; target = value; registry <- lease; marker }]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let outer = owners[0];
    let (assignment_owner, assignment) = candidate_statement(&module, outer, 1);
    let HirStmtKind::Assign { target, value } = assignment.kind() else {
        panic!("candidate assignment family");
    };
    assert_candidate_origin(&module, assignment_owner, outer, 1);
    assert_candidate_origin(&module, *target, outer, 3);
    assert_candidate_origin(&module, *value, outer, 4);

    let (lifetime_owner, lifetime_set) = candidate_statement(&module, outer, 2);
    let HirStmtKind::LifetimeSet { target, value } = lifetime_set.kind() else {
        panic!("candidate lifetime-set family");
    };
    assert_candidate_origin(&module, lifetime_owner, outer, 2);
    assert_candidate_origin(&module, *target, outer, 5);
    assert_candidate_origin(&module, *value, outer, 6);
}

#[test]
fn candidate_assignment_missing_operands_preserve_family_and_priority() {
    let parsed = parsed_source(
        "dialogue-candidate-assignment-recovery",
        &[
            "items[{ marker; = value; marker }]".into(),
            "items[{ marker; target =; marker }]".into(),
            "items[{ marker; <- lease; marker }]".into(),
            "items[{ marker; registry <-; marker }]".into(),
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
        let (_, statement) = candidate_statement(&module, owners[position], 1);
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
fn candidate_required_operands_and_ordinary_wait_call_keep_global_preorder() {
    let parsed = parsed_source(
        "dialogue-candidate-required-operands",
        &["items[{ marker; return value; yield @entity.value; wait(target); close resource; select candidate.member; marker }]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let outer = owners[0];
    let (_, returned) = candidate_statement(&module, outer, 1);
    let HirStmtKind::Return { value } = returned.kind() else {
        panic!("candidate Return family");
    };
    assert_candidate_origin(&module, *value, outer, 3);

    let (_, yielded) = candidate_statement(&module, outer, 2);
    let HirStmtKind::Yield {
        expression: yielded,
    } = yielded.kind()
    else {
        panic!("candidate Yield family");
    };
    assert_candidate_origin(&module, *yielded, outer, 4);

    let (_, waited) = candidate_statement(&module, outer, 3);
    let HirStmtKind::Expression { expression: waited } = waited.kind() else {
        panic!("ordinary candidate wait call must remain an expression statement");
    };
    assert_candidate_origin(&module, *waited, outer, 5);
    let HirExprKind::Call(wait) = expression(&module, *waited).kind() else {
        panic!("ordinary candidate wait must remain a Call expression");
    };
    let HirCallCallee::Value { value: callee } = wait.callee() else {
        panic!("ordinary candidate wait must retain its value callee");
    };
    assert_candidate_origin(&module, *callee, outer, 6);
    assert_candidate_origin(&module, wait.arguments()[0].value(), outer, 7);

    let (_, closed) = candidate_statement(&module, outer, 4);
    let HirStmtKind::Close { target } = closed.kind() else {
        panic!("candidate Close family");
    };
    assert_candidate_origin(&module, *target, outer, 8);

    let (_, selected) = candidate_statement(&module, outer, 5);
    let HirStmtKind::Select(crate::stmt::HirSelectStmt::Operand(selected)) = selected.kind() else {
        panic!("candidate ordinary Select family");
    };
    assert_candidate_origin(&module, *selected, outer, 9);
    assert!(matches!(
        expression(&module, *selected).kind(),
        HirExprKind::Select(_)
    ));
}

#[test]
fn candidate_required_operand_recovery_excludes_ordinary_wait_calls() {
    let parsed = parsed_source(
        "dialogue-candidate-required-operand-recovery",
        &[
            "items[{ marker; return; marker }]".into(),
            "items[{ marker; yield; marker }]".into(),
            "items[{ marker; close; marker }]".into(),
            "items[{ marker; select; marker }]".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    for (position, expected) in [
        HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::Expression,
        },
        HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::Expression,
        },
        HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::Target,
        },
        HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::Expression,
        },
    ]
    .into_iter()
    .enumerate()
    {
        let (_, statement) = candidate_statement(&module, owners[position], 1);
        assert_eq!(statement.state(), &HirStmtPoisonState::Poisoned(expected));
    }
}

#[test]
fn candidate_keyword_statements_keep_typed_payloads_and_source_order() {
    let parsed = parsed_source(
        "dialogue-candidate-keyword-statements",
        &["items[{ out 'exit Vec<Int>.with_capacity(1); goto Vec<Int>.with_capacity(2); defer Vec<Int>.with_capacity(3); signal Vec<Int>.with_capacity(4) <- Vec<Int>.with_capacity(5); break 'loop Vec<Int>.with_capacity(6); continue 'loop; marker }]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let outer = owners[0];
    let (_, out) = candidate_statement(&module, outer, 0);
    let HirStmtKind::Out {
        label: Some(label),
        value: out_value,
    } = out.kind()
    else {
        panic!("candidate Out payload");
    };
    assert_eq!(label.as_str(), "exit");
    let (_, goto) = candidate_statement(&module, outer, 1);
    let HirStmtKind::Goto {
        target: goto_target,
    } = goto.kind()
    else {
        panic!("candidate Goto payload");
    };
    let (_, defer) = candidate_statement(&module, outer, 2);
    let HirStmtKind::Defer {
        outcome: DeferOutcome::Always,
        expression: defer_value,
    } = defer.kind()
    else {
        panic!("candidate Defer payload");
    };
    let (_, signal) = candidate_statement(&module, outer, 3);
    let HirStmtKind::Signal {
        target: signal_target,
        value: signal_value,
    } = signal.kind()
    else {
        panic!("candidate Signal payload");
    };
    let (_, break_statement) = candidate_statement(&module, outer, 4);
    let HirStmtKind::Break {
        label: Some(label),
        value: Some(break_value),
    } = break_statement.kind()
    else {
        panic!("candidate Break payload");
    };
    assert_eq!(label.as_str(), "loop");
    let (_, continue_statement) = candidate_statement(&module, outer, 5);
    assert!(matches!(
        continue_statement.kind(),
        HirStmtKind::Continue { label: Some(label) } if label.as_str() == "loop"
    ));

    for (ordinal, expression) in [
        *out_value,
        *goto_target,
        *defer_value,
        *signal_target,
        *signal_value,
        *break_value,
    ]
    .into_iter()
    .enumerate()
    {
        let receiver = associated_call_receiver(&module, expression);
        assert_candidate_origin(
            &module,
            receiver,
            outer,
            u32::try_from(ordinal * 2).expect("bounded candidate type ordinal"),
        );
    }
}

#[test]
fn candidate_keyword_recovery_matches_central_priority() {
    let parsed = parsed_source(
        "dialogue-candidate-keyword-recovery",
        &[
            "items[{ out 'line.focus; marker }]".into(),
            "items[{ goto; marker }]".into(),
            "items[{ defer; marker }]".into(),
            "items[{ signal <- value; marker }]".into(),
            "items[{ signal target; marker }]".into(),
            "items[{ signal; marker }]".into(),
            "items[{ break; marker }]".into(),
            "items[{ break value +; marker }]".into(),
            "items[{ break 'line.focus value +; marker }]".into(),
            "items[{ continue extra; marker }]".into(),
            "items[{ continue 'events? extra; marker }]".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let expected = [
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::InvalidControlLabel(
            crate::leaf::HirNameInvariantError::InvalidIdentifier,
        )),
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
        let (_, statement) = candidate_statement(&module, owners[position], 0);
        assert_eq!(
            statement.state(),
            &expected,
            "unexpected candidate recovery at fixture {position}: {:?}",
            statement.kind()
        );
    }
}

#[test]
fn candidate_block_shaped_select_never_publishes_as_ordinary_select() {
    for (ordinal, source) in [
        "items[{ marker; select { marker; }; marker }]",
        "items[{ marker; select result { marker; }; marker }]",
        "items[{ marker; select scope named { marker; }; marker }]",
    ]
    .into_iter()
    .enumerate()
    {
        let parsed = parsed_source(
            &format!("dialogue-candidate-flow-select-freeze-{ordinal}"),
            &[source.into()],
        );
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
