use arcweft_lang_syntax::assertion::AssertionMode;
use arcweft_lang_syntax::ast::line_plan::DeferOutcome;
use core::fmt::Debug;
use core::num::{NonZeroU32, NonZeroU64};

use super::{
    HirAssertionMode, HirConditionalElseBranch, HirContextualStmtBody, HirIfLetStmt, HirIfStmt,
    HirMatchStmt, HirStmt, HirStmtChildRole, HirStmtInvariantError, HirStmtKind, HirStmtMatchArm,
    HirStmtMatchArmBody, HirStmtPoisonState, HirStmtRecoveryIssue, HirThreadStmtChildRole,
    HirThreadStmtInvariantError, HirThreadStmtRecoveryIssue, HirTriggerPattern, HirUnsafeAudit,
    HirUnsafeLifetimeBody,
};
use crate::identity::{
    ExprId, HirDatabaseId, HirModuleId, HirTypedId, LocalId, PatternId, RawHirId, ScopeId, StmtId,
    TypeId,
};
use crate::leaf::{
    HirEntityReference, HirIdRef, HirIdRefIssue, HirIdRefRecovery, HirIdRefShape, HirIdRefValue,
    HirName,
};

fn module(database: u64) -> HirModuleId {
    HirModuleId::new(
        HirDatabaseId::from_raw_for_test(NonZeroU64::new(database).unwrap()),
        NonZeroU32::MIN,
    )
}

fn id<I: HirTypedId>(module: HirModuleId, slot: u32) -> I {
    I::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).unwrap(),
        I::KIND,
    ))
}

fn name(value: &str) -> HirName {
    HirName::try_new(value.into()).expect("valid HIR name")
}

fn ordinary_body(scope: ScopeId, statements: Box<[StmtId]>) -> HirContextualStmtBody {
    HirContextualStmtBody::try_ordinary(scope, statements).expect("valid ordinary body")
}

fn audit_id() -> HirIdRefValue {
    HirIdRefValue::Resolved(HirIdRef::absolute(
        HirEntityReference::try_new("unsafe.audit".into()).expect("valid absolute audit ID"),
    ))
}

fn assert_record_traits<T: Clone + Debug + Eq + PartialEq>() {}

#[test]
fn statement_records_expose_the_required_owned_traits() {
    assert_record_traits::<HirStmt>();
    assert_record_traits::<HirStmtKind>();
    assert_record_traits::<HirIfLetStmt>();
    assert_record_traits::<HirStmtMatchArm>();
    assert_record_traits::<HirStmtMatchArmBody>();
    assert_record_traits::<HirUnsafeAudit>();
    assert_record_traits::<HirUnsafeLifetimeBody>();
}

#[test]
fn if_let_retains_the_pattern_visibility_shape_and_typed_else_branch() {
    let owner_module = module(1);
    let owner_scope = id::<ScopeId>(owner_module, 1);
    let then_scope = id::<ScopeId>(owner_module, 2);
    let else_scope = id::<ScopeId>(owner_module, 3);
    let pattern = id::<PatternId>(owner_module, 4);
    let scrutinee = id::<ExprId>(owner_module, 5);
    let guard = id::<ExprId>(owner_module, 6);
    let first_then = id::<StmtId>(owner_module, 7);
    let second_then = id::<StmtId>(owner_module, 8);
    let first_local = id::<LocalId>(owner_module, 9);
    let second_local = id::<LocalId>(owner_module, 10);
    let else_statement = id::<StmtId>(owner_module, 11);

    let payload = HirIfLetStmt::try_new(
        pattern,
        scrutinee,
        Some(guard),
        ordinary_body(then_scope, Box::new([first_then, second_then])),
        Box::new([first_local, second_local]),
        Some(HirConditionalElseBranch::body(ordinary_body(
            else_scope,
            Box::new([else_statement]),
        ))),
    )
    .expect("same-module if-let payload");
    let statement = HirStmt::try_new(owner_scope, HirStmtKind::IfLet(payload))
        .expect("same-module if-let statement");

    assert_eq!(statement.scope(), owner_scope);
    let HirStmtKind::IfLet(payload) = statement.kind() else {
        panic!("dedicated if-let payload");
    };
    assert_eq!(payload.pattern(), pattern);
    assert_eq!(payload.scrutinee(), scrutinee);
    assert_eq!(payload.guard(), Some(guard));
    assert_eq!(payload.then_scope(), then_scope);
    assert_eq!(
        payload.then_body().ordinary_statements(),
        Some([first_then, second_then].as_slice())
    );
    assert_eq!(payload.locals(), [first_local, second_local]);
    assert!(matches!(
        payload.else_branch(),
        Some(HirConditionalElseBranch::Body(body))
            if body.scope() == else_scope
                && body.ordinary_statements() == Some([else_statement].as_slice())
    ));

    let nested = id::<StmtId>(owner_module, 12);
    let nested_else = HirIfLetStmt::try_new(
        pattern,
        scrutinee,
        None,
        ordinary_body(then_scope, Box::new([])),
        Box::new([first_local]),
        Some(HirConditionalElseBranch::else_if(nested)),
    )
    .expect("nested else-if keeps its statement identity");
    assert_eq!(
        nested_else.else_branch(),
        Some(&HirConditionalElseBranch::ElseIf(nested))
    );
}

#[test]
fn if_let_rejects_every_foreign_child_category_before_publication() {
    let owner_module = module(2);
    let foreign_module = module(3);
    let scope = id::<ScopeId>(owner_module, 1);
    let pattern = id::<PatternId>(owner_module, 2);
    let scrutinee = id::<ExprId>(owner_module, 3);
    let body = id::<StmtId>(owner_module, 5);
    let local = id::<LocalId>(owner_module, 6);
    let foreign_pattern = id::<PatternId>(foreign_module, 2);
    let foreign_expr = id::<ExprId>(foreign_module, 3);
    let foreign_scope = id::<ScopeId>(foreign_module, 1);
    let foreign_stmt = id::<StmtId>(foreign_module, 5);
    let foreign_local = id::<LocalId>(foreign_module, 6);
    let error = HirThreadStmtInvariantError::ForeignChild {
        expected: owner_module,
        actual: foreign_module,
    };

    assert_eq!(
        HirIfLetStmt::try_new(
            foreign_pattern,
            scrutinee,
            None,
            ordinary_body(scope, Box::new([])),
            Box::new([]),
            None,
        ),
        Err(error)
    );
    assert_eq!(
        HirIfLetStmt::try_new(
            pattern,
            foreign_expr,
            None,
            ordinary_body(scope, Box::new([])),
            Box::new([]),
            None,
        ),
        Err(error)
    );
    assert_eq!(
        HirIfLetStmt::try_new(
            pattern,
            scrutinee,
            Some(foreign_expr),
            ordinary_body(scope, Box::new([])),
            Box::new([]),
            None,
        ),
        Err(error)
    );
    assert_eq!(
        HirContextualStmtBody::try_ordinary(scope, Box::new([foreign_stmt])),
        Err(error)
    );
    assert_eq!(
        HirIfLetStmt::try_new(
            pattern,
            scrutinee,
            None,
            ordinary_body(scope, Box::new([body])),
            Box::new([foreign_local]),
            None,
        ),
        Err(error)
    );
    assert_eq!(
        HirIfLetStmt::try_new(
            pattern,
            scrutinee,
            None,
            ordinary_body(scope, Box::new([body])),
            Box::new([local]),
            Some(HirConditionalElseBranch::body(ordinary_body(
                foreign_scope,
                Box::new([]),
            ))),
        ),
        Err(error)
    );
    assert_eq!(
        HirIfLetStmt::try_new(
            pattern,
            scrutinee,
            None,
            ordinary_body(scope, Box::new([body])),
            Box::new([local]),
            Some(HirConditionalElseBranch::else_if(foreign_stmt)),
        ),
        Err(error)
    );
}

#[test]
fn statement_owner_rejects_foreign_ids_across_nested_payloads() {
    let owner_module = module(4);
    let foreign_module = module(5);
    let owner_scope = id::<ScopeId>(owner_module, 1);
    let foreign_expr = id::<ExprId>(foreign_module, 2);
    let foreign_pattern = id::<PatternId>(foreign_module, 3);
    let expected = Err(HirStmtInvariantError::ForeignChild {
        expected: owner_module,
        actual: foreign_module,
    });

    assert_eq!(
        HirStmt::try_new(
            owner_scope,
            HirStmtKind::Let {
                pattern: foreign_pattern,
                annotation: None,
                initializer: id::<ExprId>(owner_module, 2),
                locals: Box::new([]),
            },
        ),
        expected
    );
    assert_eq!(
        HirStmt::try_new(
            owner_scope,
            HirStmtKind::On {
                trigger: HirTriggerPattern::Timeout(foreign_expr),
                scope: owner_scope,
                body: Box::new([]),
            },
        ),
        expected
    );
    assert_eq!(
        HirStmt::try_new(
            owner_scope,
            HirStmtKind::UnsafeLifetime {
                audit: HirUnsafeAudit::new(audit_id(), Some(foreign_expr), true),
                body: HirUnsafeLifetimeBody::Block {
                    scope: owner_scope,
                    statements: Box::new([]),
                },
            },
        ),
        expected
    );
}

#[test]
fn ordered_statement_bodies_arms_conditions_and_locals_are_not_reordered() {
    let owner_module = module(6);
    let owner_scope = id::<ScopeId>(owner_module, 1);
    let first_expr = id::<ExprId>(owner_module, 2);
    let second_expr = id::<ExprId>(owner_module, 3);
    let first_stmt = id::<StmtId>(owner_module, 4);
    let second_stmt = id::<StmtId>(owner_module, 5);
    let first_local = id::<LocalId>(owner_module, 6);
    let second_local = id::<LocalId>(owner_module, 7);
    let first_pattern = id::<PatternId>(owner_module, 8);
    let second_pattern = id::<PatternId>(owner_module, 9);
    let second_scope = id::<ScopeId>(owner_module, 10);

    let assertion = HirStmt::try_new(
        owner_scope,
        HirStmtKind::Assertion {
            mode: HirAssertionMode::Resolved(AssertionMode::Check),
            conditions: Box::new([second_expr, first_expr]),
        },
    )
    .expect("ordered assertion");
    let HirStmtKind::Assertion { mode, conditions } = assertion.kind() else {
        panic!("assertion payload");
    };
    assert_eq!(mode.resolved(), Some(AssertionMode::Check));
    assert_eq!(conditions.as_ref(), [second_expr, first_expr]);

    let first_arm = HirStmtMatchArm::try_new(
        owner_scope,
        first_pattern,
        None,
        HirStmtMatchArmBody::Body(ordinary_body(
            owner_scope,
            Box::new([second_stmt, first_stmt]),
        )),
        Box::new([second_local, first_local]),
    )
    .expect("first ordered arm");
    let second_arm = HirStmtMatchArm::try_new(
        second_scope,
        second_pattern,
        Some(first_expr),
        HirStmtMatchArmBody::Expression(second_expr),
        Box::new([]),
    )
    .expect("second ordered arm");
    let matched = HirStmt::try_new(
        owner_scope,
        HirStmtKind::Match(
            HirMatchStmt::try_new(first_expr, Box::new([first_arm, second_arm]))
                .expect("typed Match payload"),
        ),
    )
    .expect("ordered match");
    let HirStmtKind::Match(payload) = matched.kind() else {
        panic!("match payload");
    };
    let arms = payload.arms();
    assert_eq!(arms[0].pattern(), first_pattern);
    assert_eq!(
        arms[0].body(),
        &HirStmtMatchArmBody::Body(ordinary_body(
            owner_scope,
            Box::new([second_stmt, first_stmt]),
        ))
    );
    assert_eq!(arms[0].locals(), [second_local, first_local]);
    assert_eq!(arms[1].pattern(), second_pattern);
    assert_eq!(
        arms[1].body(),
        &HirStmtMatchArmBody::Expression(second_expr)
    );
}

#[test]
fn statement_match_arm_rejects_foreign_expression_and_block_bodies() {
    let owner_module = module(12);
    let foreign_module = module(13);
    let scope = id::<ScopeId>(owner_module, 1);
    let pattern = id::<PatternId>(owner_module, 2);
    let foreign_expression = id::<ExprId>(foreign_module, 3);
    let foreign_statement = id::<StmtId>(foreign_module, 4);
    let error = HirThreadStmtInvariantError::ForeignChild {
        expected: owner_module,
        actual: foreign_module,
    };

    assert_eq!(
        HirStmtMatchArm::try_new(
            scope,
            pattern,
            None,
            HirStmtMatchArmBody::Expression(foreign_expression),
            Box::new([]),
        ),
        Err(error)
    );
    assert_eq!(
        HirContextualStmtBody::try_ordinary(scope, Box::new([foreign_statement])),
        Err(error)
    );
}

#[test]
fn existing_assertion_and_defer_authorities_are_retained_directly() {
    let owner_module = module(7);
    let scope = id::<ScopeId>(owner_module, 1);
    let body = id::<StmtId>(owner_module, 2);

    let statement = HirStmt::try_new(
        scope,
        HirStmtKind::DeferBlock {
            outcome: DeferOutcome::Failed,
            scope,
            body: Box::new([body]),
        },
    )
    .expect("defer authority");
    assert!(matches!(
        statement.kind(),
        HirStmtKind::DeferBlock { outcome: DeferOutcome::Failed, body: statements, .. }
            if statements.as_ref() == [body]
    ));
}

#[test]
fn keyword_statement_recovery_roles_are_family_closed() {
    let owner_module = module(15);
    let scope = id::<ScopeId>(owner_module, 1);
    let first = id::<ExprId>(owner_module, 2);
    let second = id::<ExprId>(owner_module, 3);
    let cases = [
        (
            HirStmtKind::Out {
                label: None,
                value: first,
            },
            HirStmtChildRole::Target,
        ),
        (
            HirStmtKind::Goto { target: first },
            HirStmtChildRole::Initializer,
        ),
        (
            HirStmtKind::Defer {
                outcome: DeferOutcome::Always,
                expression: first,
            },
            HirStmtChildRole::Target,
        ),
        (
            HirStmtKind::Signal {
                target: first,
                value: second,
            },
            HirStmtChildRole::Expression,
        ),
        (
            HirStmtKind::Break {
                label: None,
                value: Some(first),
            },
            HirStmtChildRole::Target,
        ),
        (
            HirStmtKind::Continue { label: None },
            HirStmtChildRole::Initializer,
        ),
    ];

    for (kind, role) in cases {
        assert_eq!(
            HirStmt::try_new_with_state(
                scope,
                kind,
                HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild { role }),
            ),
            Err(HirStmtInvariantError::InvalidPoisonState)
        );
    }
}

#[test]
fn unsafe_audit_retains_semantics_without_a_source_range() {
    let owner_module = module(8);
    let reason = id::<ExprId>(owner_module, 1);
    let audit = HirUnsafeAudit::new(audit_id(), Some(reason), true);

    assert!(matches!(
        audit.id(),
        HirIdRefValue::Resolved(HirIdRef::Absolute(id)) if id.as_str() == "unsafe.audit"
    ));
    assert_eq!(audit.reason(), Some(reason));
    assert!(audit.has_safety_doc());
}

#[test]
fn unsafe_lifetime_constructor_rejects_payload_and_poison_state_drift() {
    let owner_module = module(14);
    let owner_scope = id::<ScopeId>(owner_module, 1);
    let body_scope = id::<ScopeId>(owner_module, 2);
    let body = || HirUnsafeLifetimeBody::Block {
        scope: body_scope,
        statements: Box::new([]),
    };
    let missing_id = || {
        HirIdRefValue::Recovered(HirIdRefRecovery::new(
            HirIdRefShape::Missing,
            HirIdRefIssue::Missing,
        ))
    };

    assert_eq!(
        HirStmt::try_new(
            owner_scope,
            HirStmtKind::UnsafeLifetime {
                audit: HirUnsafeAudit::new(missing_id(), None, false),
                body: body(),
            },
        ),
        Err(HirStmtInvariantError::InvalidPoisonState)
    );
    assert_eq!(
        HirStmt::try_new(
            owner_scope,
            HirStmtKind::UnsafeLifetime {
                audit: HirUnsafeAudit::new(audit_id(), None, false),
                body: HirUnsafeLifetimeBody::Missing,
            },
        ),
        Err(HirStmtInvariantError::InvalidPoisonState)
    );
    assert_eq!(
        HirStmt::try_new_with_state(
            owner_scope,
            HirStmtKind::UnsafeLifetime {
                audit: HirUnsafeAudit::new(audit_id(), None, false),
                body: body(),
            },
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::MissingBody),
        ),
        Err(HirStmtInvariantError::InvalidPoisonState)
    );
    assert_eq!(
        HirStmt::try_new_with_state(
            owner_scope,
            HirStmtKind::UnsafeLifetime {
                audit: HirUnsafeAudit::new(audit_id(), None, false),
                body: body(),
            },
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::InvalidAuditId(
                HirIdRefIssue::Missing,
            )),
        ),
        Err(HirStmtInvariantError::InvalidPoisonState)
    );

    assert!(
        HirStmt::try_new_with_state(
            owner_scope,
            HirStmtKind::UnsafeLifetime {
                audit: HirUnsafeAudit::new(missing_id(), None, false),
                body: body(),
            },
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::InvalidAuditId(
                HirIdRefIssue::Missing,
            )),
        )
        .is_ok()
    );
    assert!(
        HirStmt::try_new_with_state(
            owner_scope,
            HirStmtKind::UnsafeLifetime {
                audit: HirUnsafeAudit::new(audit_id(), None, false),
                body: body(),
            },
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::UnclosedBody),
        )
        .is_ok()
    );
}

fn statement_variant_ordinal(kind: &HirStmtKind) -> u8 {
    match kind {
        HirStmtKind::Assertion { .. } => 0,
        HirStmtKind::Let { .. } => 1,
        HirStmtKind::Assign { .. } => 2,
        HirStmtKind::LetElse { .. } => 3,
        HirStmtKind::LetChoice { .. } => 4,
        HirStmtKind::LetScope { .. } => 5,
        HirStmtKind::LetLoop { .. } => 6,
        HirStmtKind::LetActionReceive { .. } => 7,
        HirStmtKind::Return { .. } => 8,
        HirStmtKind::Out { .. } => 9,
        HirStmtKind::Goto { .. } => 10,
        HirStmtKind::DeferBlock { .. } => 11,
        HirStmtKind::Defer { .. } => 12,
        HirStmtKind::Yield { .. } => 13,
        HirStmtKind::Signal { .. } => 14,
        HirStmtKind::LifetimeSet { .. } => 15,
        HirStmtKind::Wait { .. } => 16,
        HirStmtKind::On { .. } => 17,
        HirStmtKind::UnsafeLifetime { .. } => 18,
        HirStmtKind::Choice { .. } => 19,
        HirStmtKind::If(_) => 20,
        HirStmtKind::IfLet(_) => 21,
        HirStmtKind::Match(_) => 22,
        HirStmtKind::Loop(_) => 23,
        HirStmtKind::While(_) => 24,
        HirStmtKind::WhileLet(_) => 25,
        HirStmtKind::For(_) => 26,
        HirStmtKind::Close { .. } => 27,
        HirStmtKind::Select(_) => 28,
        HirStmtKind::SourceLocale(_) => 29,
        HirStmtKind::Scope(_) => 30,
        HirStmtKind::Include(_) => 31,
        HirStmtKind::Break { .. } => 32,
        HirStmtKind::Continue { .. } => 33,
        HirStmtKind::Expression { .. } => 34,
        HirStmtKind::ProofCall { .. } => 35,
        HirStmtKind::Error => 36,
    }
}

#[test]
fn statement_inventory_is_the_closed_typed_contract() {
    assert_eq!(statement_variant_ordinal(&HirStmtKind::Error), 36);
    assert_eq!(
        statement_variant_ordinal(&HirStmtKind::Continue {
            label: Some(name("outer")),
        }),
        33
    );
}

#[test]
fn ordinary_if_rejects_a_foreign_contextual_else_body_before_publication() {
    let owner_module = module(9);
    let foreign_module = module(10);
    let then_scope = id::<ScopeId>(owner_module, 1);
    let else_scope = id::<ScopeId>(foreign_module, 1);
    let foreign_else = HirConditionalElseBranch::body(ordinary_body(
        else_scope,
        Box::new([id::<StmtId>(foreign_module, 2)]),
    ));

    assert_eq!(
        HirIfStmt::try_new(
            id::<ExprId>(owner_module, 2),
            ordinary_body(then_scope, Box::new([])),
            Some(foreign_else),
        ),
        Err(HirThreadStmtInvariantError::ForeignChild {
            expected: owner_module,
            actual: foreign_module,
        })
    );
}

#[test]
fn ordinary_if_accepts_only_its_typed_recovery_vocabulary() {
    let owner_module = module(10);
    let owner_scope = id::<ScopeId>(owner_module, 1);
    let payload = HirIfStmt::try_new(
        id::<ExprId>(owner_module, 2),
        ordinary_body(id::<ScopeId>(owner_module, 3), Box::new([])),
        None,
    )
    .expect("same-module if payload");

    assert!(
        HirStmt::try_new_with_state(
            owner_scope,
            HirStmtKind::If(payload.clone()),
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::Thread(
                HirThreadStmtRecoveryIssue::RecoveredChild {
                    role: HirThreadStmtChildRole::Condition,
                },
            )),
        )
        .is_ok()
    );
    assert_eq!(
        HirStmt::try_new_with_state(
            owner_scope,
            HirStmtKind::If(payload),
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::Thread(
                HirThreadStmtRecoveryIssue::EmptySelect,
            )),
        ),
        Err(HirStmtInvariantError::InvalidPoisonState)
    );
}

#[test]
fn optional_type_children_are_checked_by_the_statement_owner() {
    let owner_module = module(10);
    let foreign_module = module(11);
    let scope = id::<ScopeId>(owner_module, 1);
    assert_eq!(
        HirStmt::try_new(
            scope,
            HirStmtKind::Let {
                pattern: id::<PatternId>(owner_module, 2),
                annotation: Some(id::<TypeId>(foreign_module, 3)),
                initializer: id::<ExprId>(owner_module, 4),
                locals: Box::new([]),
            },
        ),
        Err(HirStmtInvariantError::ForeignChild {
            expected: owner_module,
            actual: foreign_module,
        })
    );
}
