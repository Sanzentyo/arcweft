use arcweft_id::UnsafeAuditId;
use arcweft_lang_syntax::assertion::AssertionMode;
use arcweft_lang_syntax::ast::line_plan::DeferOutcome;
use core::fmt::Debug;
use core::num::{NonZeroU32, NonZeroU64};
use std::collections::BTreeSet;

use super::{
    HirAssertionMode, HirConditionalElseBranch, HirContextualStmtBody, HirForStmt, HirIfLetStmt,
    HirIfStmt, HirIncludeStmt, HirMatchStmt, HirScopeStmt, HirSelectBindingLocal, HirSelectBranch,
    HirSelectBranchHead, HirSelectStmt, HirSourceLocaleIssue, HirSourceLocaleStmt,
    HirSourceLocaleValue, HirStatementChildRole, HirStmt, HirStmtBindingPlanKind,
    HirStmtBranchPublicationKind, HirStmtChildRole, HirStmtEvaluationPlan,
    HirStmtEvaluationPublicationRole, HirStmtEvaluationStep, HirStmtInvariantError, HirStmtKind,
    HirStmtMatchArm, HirStmtMatchArmBody, HirStmtOrderedPairPlanKind, HirStmtPoisonState,
    HirStmtRecoveryIssue, HirStmtSelectEvaluationPlan, HirStmtSelectHeadEvaluation,
    HirStmtTriggerEvaluationPlan, HirStmtValuePlanKind, HirThreadStmtChildRole,
    HirThreadStmtInvariantError, HirThreadStmtRecoveryIssue, HirTrigger, HirUnsafeAudit,
    HirUnsafeAuditIdentity, HirUnsafeAuditIdentityIssue, HirUnsafeLifetimeBody, HirWhileLetStmt,
    HirWhileStmt,
};
use crate::expr::{HirThreadBody, HirThreadBodyOwner, HirThreadFlowItem};
use crate::identity::{
    ExprId, HirDatabaseId, HirModuleId, HirTypedId, LocalId, PatternId, RawHirId, ScopeId, StmtId,
    TypeId,
};
use crate::leaf::{HirEntityReference, HirIdRef, HirIdRefValue, HirName};

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

fn unsafe_audit_identity() -> HirUnsafeAuditIdentity {
    HirUnsafeAuditIdentity::Accepted(
        UnsafeAuditId::try_new("unsafe.audit").expect("valid unsafe-audit identity"),
    )
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
                trigger: HirTrigger::Timeout(foreign_expr),
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
                audit: HirUnsafeAudit::new(unsafe_audit_identity(), Some(foreign_expr), true),
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
    let HirStmtEvaluationPlan::Assertion { mode, conditions } = assertion.kind().evaluation_plan()
    else {
        panic!("assertion evaluation plan");
    };
    assert_eq!(mode.resolved(), Some(AssertionMode::Check));
    assert_eq!(conditions, [second_expr, first_expr]);

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
fn evaluation_plan_preserves_binding_visibility_and_statement_metadata() {
    let owner_module = module(14);
    let owner_scope = id::<ScopeId>(owner_module, 1);
    let else_scope = id::<ScopeId>(owner_module, 2);
    let pattern = id::<PatternId>(owner_module, 3);
    let initializer = id::<ExprId>(owner_module, 4);
    let success_local = id::<LocalId>(owner_module, 5);
    let else_statement = id::<StmtId>(owner_module, 6);
    let let_else = HirStmtKind::LetElse {
        pattern,
        annotation: None,
        initializer,
        else_scope,
        else_body: Box::new([else_statement]),
        locals: Box::new([success_local]),
    };
    let HirStmtEvaluationPlan::LetElse {
        initializer: planned_initializer,
        else_body,
        success_locals,
        ..
    } = let_else.evaluation_plan()
    else {
        panic!("let-else evaluation plan");
    };
    assert_eq!(planned_initializer, initializer);
    assert_eq!(else_body, [else_statement]);
    assert_eq!(success_locals, [success_local]);

    let locale = HirStmtKind::SourceLocale(
        HirSourceLocaleStmt::try_new(
            HirSourceLocaleValue::Recovered(HirSourceLocaleIssue::Missing),
            ordinary_body(owner_scope, Box::new([])),
        )
        .expect("source locale statement"),
    );
    let HirStmtEvaluationPlan::SourceLocale { locale, .. } = locale.evaluation_plan() else {
        panic!("source locale evaluation plan");
    };
    assert_eq!(
        locale,
        &HirSourceLocaleValue::Recovered(HirSourceLocaleIssue::Missing)
    );

    let scope = HirStmtKind::Scope(
        HirScopeStmt::try_new(
            Some(name("named")),
            ordinary_body(owner_scope, Box::new([])),
        )
        .expect("scope statement"),
    );
    let HirStmtEvaluationPlan::Scope {
        name: planned_name, ..
    } = scope.evaluation_plan()
    else {
        panic!("scope evaluation plan");
    };
    assert_eq!(planned_name.map(HirName::as_str), Some("named"));

    let select = HirStmtKind::Select(HirSelectStmt::operand(initializer));
    let HirStmtEvaluationPlan::Select { scope, plan } = select.evaluation_plan() else {
        panic!("select evaluation plan");
    };
    assert_eq!(scope, None);
    assert_eq!(
        plan,
        HirStmtSelectEvaluationPlan::Operand {
            expression: initializer
        }
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the 31-family matrix keeps every plan payload and visibility boundary auditable"
)]
fn evaluation_plan_matrix_covers_all_thirty_one_statement_families() {
    let owner_module = module(16);
    let scope = id::<ScopeId>(owner_module, 1);
    let body_scope = id::<ScopeId>(owner_module, 2);
    let branch_scope = id::<ScopeId>(owner_module, 3);
    let select_scope = id::<ScopeId>(owner_module, 4);
    let select_body_scope = id::<ScopeId>(owner_module, 5);
    let first_expr = id::<ExprId>(owner_module, 6);
    let second_expr = id::<ExprId>(owner_module, 7);
    let third_expr = id::<ExprId>(owner_module, 8);
    let fourth_expr = id::<ExprId>(owner_module, 9);
    let first_pattern = id::<PatternId>(owner_module, 10);
    let first_local = id::<LocalId>(owner_module, 12);
    let first_statement = id::<StmtId>(owner_module, 14);
    let second_statement = id::<StmtId>(owner_module, 15);
    let first_type = id::<TypeId>(owner_module, 16);
    let include_target = audit_id();

    let if_statement = HirIfStmt::try_new(
        first_expr,
        ordinary_body(body_scope, Box::new([first_statement])),
        None,
    )
    .expect("if payload");
    let if_let_statement = HirIfLetStmt::try_new(
        first_pattern,
        first_expr,
        Some(second_expr),
        ordinary_body(body_scope, Box::new([first_statement])),
        Box::new([first_local]),
        None,
    )
    .expect("if-let payload");
    let match_arm = HirStmtMatchArm::try_new(
        body_scope,
        first_pattern,
        Some(second_expr),
        HirStmtMatchArmBody::Expression(third_expr),
        Box::new([first_local]),
    )
    .expect("match arm payload");
    let match_statement =
        HirMatchStmt::try_new(first_expr, Box::new([match_arm])).expect("match payload");
    let while_statement = HirWhileStmt::try_new(
        first_expr,
        ordinary_body(body_scope, Box::new([first_statement])),
    )
    .expect("while payload");
    let while_let_statement = HirWhileLetStmt::try_new(
        first_pattern,
        first_expr,
        Some(second_expr),
        Box::new([first_local]),
        ordinary_body(body_scope, Box::new([first_statement])),
    )
    .expect("while-let payload");
    let for_statement = HirForStmt::try_new(
        first_expr,
        second_expr,
        third_expr,
        first_pattern,
        Box::new([first_local]),
        ordinary_body(body_scope, Box::new([first_statement])),
    )
    .expect("for payload");
    let select_branch = HirSelectBranch::try_new(
        HirSelectBranchHead::Bind {
            binding: HirSelectBindingLocal::Resolved(first_local),
            source: first_expr,
        },
        ordinary_body(select_body_scope, Box::new([second_statement])),
    )
    .expect("select branch payload");
    let select_statement = HirSelectStmt::try_branches(select_scope, Box::new([select_branch]))
        .expect("select payload");

    let rows = vec![
        (
            "Assertion",
            HirStmtKind::Assertion {
                mode: HirAssertionMode::Resolved(AssertionMode::Check),
                conditions: Box::new([first_expr, second_expr]),
            },
        ),
        (
            "Let",
            HirStmtKind::Let {
                pattern: first_pattern,
                annotation: Some(first_type),
                initializer: first_expr,
                locals: Box::new([first_local]),
            },
        ),
        (
            "Assign",
            HirStmtKind::Assign {
                target: first_expr,
                value: second_expr,
            },
        ),
        (
            "LetElse",
            HirStmtKind::LetElse {
                pattern: first_pattern,
                annotation: Some(first_type),
                initializer: first_expr,
                else_scope: branch_scope,
                else_body: Box::new([first_statement]),
                locals: Box::new([first_local]),
            },
        ),
        ("Return", HirStmtKind::Return { value: first_expr }),
        (
            "Out",
            HirStmtKind::Out {
                label: Some(name("out_label")),
                value: second_expr,
            },
        ),
        ("Goto", HirStmtKind::Goto { target: first_expr }),
        (
            "Defer",
            HirStmtKind::Defer {
                outcome: DeferOutcome::Cancelled,
                expression: first_expr,
            },
        ),
        (
            "Yield",
            HirStmtKind::Yield {
                expression: first_expr,
            },
        ),
        (
            "Signal",
            HirStmtKind::Signal {
                target: first_expr,
                value: second_expr,
            },
        ),
        (
            "LifetimeSet",
            HirStmtKind::LifetimeSet {
                target: first_expr,
                value: second_expr,
            },
        ),
        ("Wait", HirStmtKind::Wait { target: first_expr }),
        (
            "On",
            HirStmtKind::On {
                trigger: HirTrigger::Signal {
                    target: first_expr,
                    value: Some(first_pattern),
                },
                scope: body_scope,
                body: Box::new([first_statement]),
            },
        ),
        (
            "UnsafeLifetime",
            HirStmtKind::UnsafeLifetime {
                audit: HirUnsafeAudit::new(unsafe_audit_identity(), Some(second_expr), true),
                body: HirUnsafeLifetimeBody::Block {
                    scope: body_scope,
                    statements: Box::new([first_statement]),
                },
            },
        ),
        ("Choice", HirStmtKind::Choice { choice: first_expr }),
        ("If", HirStmtKind::If(if_statement)),
        ("IfLet", HirStmtKind::IfLet(if_let_statement)),
        ("Match", HirStmtKind::Match(match_statement)),
        ("While", HirStmtKind::While(while_statement)),
        ("WhileLet", HirStmtKind::WhileLet(while_let_statement)),
        ("For", HirStmtKind::For(for_statement)),
        ("Close", HirStmtKind::Close { target: first_expr }),
        ("Select", HirStmtKind::Select(select_statement)),
        (
            "SourceLocale",
            HirStmtKind::SourceLocale(
                HirSourceLocaleStmt::try_new(
                    HirSourceLocaleValue::Recovered(HirSourceLocaleIssue::Missing),
                    ordinary_body(scope, Box::new([first_statement])),
                )
                .expect("source locale payload"),
            ),
        ),
        (
            "Scope",
            HirStmtKind::Scope(
                HirScopeStmt::try_new(
                    Some(name("scope_name")),
                    ordinary_body(scope, Box::new([first_statement])),
                )
                .expect("scope payload"),
            ),
        ),
        (
            "Include",
            HirStmtKind::Include(HirIncludeStmt::new(include_target.clone())),
        ),
        (
            "Break",
            HirStmtKind::Break {
                label: Some(name("break_label")),
                value: Some(third_expr),
            },
        ),
        (
            "Continue",
            HirStmtKind::Continue {
                label: Some(name("continue_label")),
            },
        ),
        (
            "Expression",
            HirStmtKind::Expression {
                expression: fourth_expr,
            },
        ),
        ("ProofCall", HirStmtKind::ProofCall { call: first_expr }),
        ("Error", HirStmtKind::Error),
    ];

    assert_eq!(rows.len(), 31);
    let tags = rows
        .iter()
        .map(|(_, statement)| statement.semantic_transcript_tag())
        .collect::<Vec<_>>();
    assert_eq!(
        tags[..30],
        (0x0700_u16..=0x071D_u16).collect::<Vec<_>>(),
        "positive statement transcript tags must follow the owner enum order"
    );
    assert_eq!(tags[30], 0x071E, "Error must be the terminal statement tag");
    assert_eq!(
        tags.iter().collect::<BTreeSet<_>>().len(),
        tags.len(),
        "statement transcript tags must be unique"
    );
    for (ordinal, (family, statement)) in rows.into_iter().enumerate() {
        let plan = statement.evaluation_plan();
        let mut steps = Vec::new();
        plan.try_visit_evaluation_steps(|step| steps.push(step))
            .expect("evaluation step stream");
        let expected_expression_steps = match ordinal {
            0 => vec![
                (
                    HirStatementChildRole::AssertionCondition { ordinal: 0 },
                    first_expr,
                ),
                (
                    HirStatementChildRole::AssertionCondition { ordinal: 1 },
                    second_expr,
                ),
            ],
            1 | 3 => vec![(HirStatementChildRole::Initializer, first_expr)],
            2 | 9 | 10 => vec![
                (HirStatementChildRole::Target, first_expr),
                (HirStatementChildRole::Value, second_expr),
            ],
            14 => vec![(HirStatementChildRole::Input, first_expr)],
            4 | 7 | 8 | 26 | 28 | 29 => {
                let expression = if ordinal == 26 {
                    third_expr
                } else if ordinal == 28 {
                    fourth_expr
                } else {
                    first_expr
                };
                vec![(HirStatementChildRole::Value, expression)]
            }
            5 => vec![(HirStatementChildRole::Value, second_expr)],
            6 | 11 | 21 => vec![(HirStatementChildRole::Target, first_expr)],
            12 => vec![(HirStatementChildRole::TriggerSignalTarget, first_expr)],
            13 => vec![(HirStatementChildRole::UnsafeReason, second_expr)],
            15 | 18 => vec![(HirStatementChildRole::Condition, first_expr)],
            16 | 19 => vec![
                (HirStatementChildRole::Scrutinee, first_expr),
                (HirStatementChildRole::Guard, second_expr),
            ],
            17 => vec![
                (HirStatementChildRole::Scrutinee, first_expr),
                (HirStatementChildRole::MatchGuard { arm: 0 }, second_expr),
                (HirStatementChildRole::MatchValue { arm: 0 }, third_expr),
            ],
            20 => vec![
                (HirStatementChildRole::ForSource, first_expr),
                (HirStatementChildRole::ForIterator, second_expr),
                (HirStatementChildRole::ForNextValue, third_expr),
            ],
            22 => vec![(
                HirStatementChildRole::SelectSource { branch: 0 },
                first_expr,
            )],
            23..=25 | 27 | 30 => Vec::new(),
            _ => unreachable!("all statement family ordinals are covered"),
        };
        let actual_expression_steps = steps
            .iter()
            .filter_map(|step| match step {
                HirStmtEvaluationStep::Expression { role, expression } => {
                    Some((*role, *expression))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual_expression_steps, expected_expression_steps,
            "{family} expression evaluation roles"
        );
        match ordinal {
            1 => assert!(matches!(
                steps.as_slice(),
                [
                    HirStmtEvaluationStep::Type { ty, .. },
                    HirStmtEvaluationStep::Expression {
                        role: HirStatementChildRole::Initializer,
                        expression,
                    },
                    HirStmtEvaluationStep::Pattern { pattern, .. },
                    HirStmtEvaluationStep::Publication {
                        role: HirStmtEvaluationPublicationRole::Binding {
                            kind: HirStmtBindingPlanKind::Let,
                        },
                        locals,
                    },
                ] if *ty == first_type
                    && *expression == first_expr
                    && *pattern == first_pattern
                    && *locals == [first_local]
            )),
            3 => assert!(matches!(
                steps.as_slice(),
                [
                    HirStmtEvaluationStep::Type { ty, .. },
                    HirStmtEvaluationStep::Expression {
                        role: HirStatementChildRole::Initializer,
                        expression,
                    },
                    HirStmtEvaluationStep::Pattern { pattern, .. },
                    HirStmtEvaluationStep::Statement { statement, .. },
                    HirStmtEvaluationStep::Publication {
                        role: HirStmtEvaluationPublicationRole::LetElseSuccess,
                        locals,
                    },
                ] if *ty == first_type
                    && *expression == first_expr
                    && *pattern == first_pattern
                    && *statement == first_statement
                    && *locals == [first_local]
            )),
            12 => assert!(matches!(
                steps.as_slice(),
                [
                    HirStmtEvaluationStep::Expression {
                        role: HirStatementChildRole::TriggerSignalTarget,
                        expression: target,
                    },
                    HirStmtEvaluationStep::Pattern { pattern, .. },
                    HirStmtEvaluationStep::Publication {
                        role: HirStmtEvaluationPublicationRole::TriggerPattern {
                            pattern: published,
                        },
                        locals,
                    },
                    HirStmtEvaluationStep::Statement { .. },
                ] if *target == first_expr
                    && *pattern == first_pattern
                    && *published == first_pattern
                    && locals.is_empty()
            )),
            16 => assert!(matches!(
                    steps.as_slice(),
                    [
                        HirStmtEvaluationStep::Expression {
                            role: HirStatementChildRole::Scrutinee,
                            expression: scrutinee,
                        },
                        HirStmtEvaluationStep::Pattern { pattern, .. },
                        HirStmtEvaluationStep::Publication {
                            role: HirStmtEvaluationPublicationRole::Branch {
                                kind: HirStmtBranchPublicationKind::IfLet,
                            },
                            locals,
                        },
                        HirStmtEvaluationStep::Expression {
                            role: HirStatementChildRole::Guard,
                            expression: guard,
                        },
                        HirStmtEvaluationStep::Statement { .. },
                    ] if *scrutinee == first_expr
                        && *pattern == first_pattern
                        && *locals == [first_local]
                        && *guard == second_expr
            )),
            17 => assert!(matches!(
                steps.as_slice(),
                [
                    HirStmtEvaluationStep::Expression {
                        role: HirStatementChildRole::Scrutinee,
                        expression: scrutinee,
                    },
                    HirStmtEvaluationStep::Pattern { pattern, .. },
                    HirStmtEvaluationStep::Publication {
                        role: HirStmtEvaluationPublicationRole::Branch {
                            kind: HirStmtBranchPublicationKind::MatchArm { arm: 0 },
                        },
                        locals,
                    },
                    HirStmtEvaluationStep::Expression {
                        role: HirStatementChildRole::MatchGuard { arm: 0 },
                        expression: guard,
                    },
                    HirStmtEvaluationStep::Expression {
                        role: HirStatementChildRole::MatchValue { arm: 0 },
                        expression: value,
                    },
                ] if *scrutinee == first_expr
                    && *pattern == first_pattern
                    && *locals == [first_local]
                    && *guard == second_expr
                    && *value == third_expr
            )),
            19 => assert!(matches!(
                steps.as_slice(),
                [
                    HirStmtEvaluationStep::Expression {
                        role: HirStatementChildRole::Scrutinee,
                        expression: scrutinee,
                    },
                    HirStmtEvaluationStep::Pattern { pattern, .. },
                    HirStmtEvaluationStep::Publication {
                        role: HirStmtEvaluationPublicationRole::Branch {
                            kind: HirStmtBranchPublicationKind::WhileLet,
                        },
                        locals,
                    },
                    HirStmtEvaluationStep::Expression {
                        role: HirStatementChildRole::Guard,
                        expression: guard,
                    },
                    HirStmtEvaluationStep::Statement { .. },
                ] if *scrutinee == first_expr
                    && *pattern == first_pattern
                    && *locals == [first_local]
                    && *guard == second_expr
            )),
            22 => assert!(matches!(
                steps.as_slice(),
                [
                    HirStmtEvaluationStep::Expression {
                        role: HirStatementChildRole::SelectSource { branch: 0 },
                        expression: source,
                    },
                    HirStmtEvaluationStep::Local { local, .. },
                    HirStmtEvaluationStep::Publication {
                        role: HirStmtEvaluationPublicationRole::Branch {
                            kind: HirStmtBranchPublicationKind::SelectBranch { branch: 0 },
                        },
                        ..
                    },
                    HirStmtEvaluationStep::Statement { statement, .. },
                ] if *source == first_expr
                    && *local == first_local
                    && *statement == second_statement
            )),
            _ => {}
        }
        match (ordinal, plan) {
            (0, HirStmtEvaluationPlan::Assertion { mode, conditions }) => {
                assert_eq!(mode.resolved(), Some(AssertionMode::Check));
                assert_eq!(conditions, [first_expr, second_expr]);
            }
            (
                1,
                HirStmtEvaluationPlan::Binding {
                    kind: HirStmtBindingPlanKind::Let,
                    pattern,
                    annotation,
                    input,
                    locals,
                },
            ) => {
                assert_eq!(pattern, first_pattern);
                assert_eq!(annotation, Some(first_type));
                assert_eq!(input, first_expr);
                assert_eq!(locals, [first_local]);
            }
            (
                2,
                HirStmtEvaluationPlan::OrderedPair {
                    kind: HirStmtOrderedPairPlanKind::Assign,
                    first,
                    second,
                },
            )
            | (
                9,
                HirStmtEvaluationPlan::OrderedPair {
                    kind: HirStmtOrderedPairPlanKind::Signal,
                    first,
                    second,
                },
            )
            | (
                10,
                HirStmtEvaluationPlan::OrderedPair {
                    kind: HirStmtOrderedPairPlanKind::LifetimeSet,
                    first,
                    second,
                },
            ) => assert_eq!((first, second), (first_expr, second_expr)),
            (
                3,
                HirStmtEvaluationPlan::LetElse {
                    pattern,
                    annotation,
                    initializer,
                    else_scope,
                    else_body,
                    success_locals,
                },
            ) => {
                assert_eq!(pattern, first_pattern);
                assert_eq!(annotation, Some(first_type));
                assert_eq!(initializer, first_expr);
                assert_eq!(else_scope, branch_scope);
                assert_eq!(else_body, [first_statement]);
                assert_eq!(success_locals, [first_local]);
            }
            (
                4,
                HirStmtEvaluationPlan::Value {
                    kind: HirStmtValuePlanKind::Return,
                    expression: Some(value),
                    label: None,
                    outcome: None,
                },
            )
            | (
                6,
                HirStmtEvaluationPlan::Value {
                    kind: HirStmtValuePlanKind::Goto,
                    expression: Some(value),
                    label: None,
                    outcome: None,
                },
            )
            | (
                7,
                HirStmtEvaluationPlan::Value {
                    kind: HirStmtValuePlanKind::Defer,
                    expression: Some(value),
                    label: None,
                    outcome: Some(DeferOutcome::Cancelled),
                },
            )
            | (
                8,
                HirStmtEvaluationPlan::Value {
                    kind: HirStmtValuePlanKind::Yield,
                    expression: Some(value),
                    label: None,
                    outcome: None,
                },
            )
            | (
                11,
                HirStmtEvaluationPlan::Value {
                    kind: HirStmtValuePlanKind::Wait,
                    expression: Some(value),
                    label: None,
                    outcome: None,
                },
            )
            | (
                14,
                HirStmtEvaluationPlan::Value {
                    kind: HirStmtValuePlanKind::Choice,
                    expression: Some(value),
                    label: None,
                    outcome: None,
                },
            )
            | (
                21,
                HirStmtEvaluationPlan::Value {
                    kind: HirStmtValuePlanKind::Close,
                    expression: Some(value),
                    label: None,
                    outcome: None,
                },
            )
            | (
                29,
                HirStmtEvaluationPlan::Value {
                    kind: HirStmtValuePlanKind::ProofCall,
                    expression: Some(value),
                    label: None,
                    outcome: None,
                },
            ) => assert_eq!(value, first_expr),
            (
                5,
                HirStmtEvaluationPlan::Value {
                    kind: HirStmtValuePlanKind::Out,
                    expression: Some(value),
                    label: Some(label),
                    outcome: None,
                },
            ) => {
                assert_eq!(value, second_expr);
                assert_eq!(label.as_str(), "out_label");
            }
            (
                12,
                HirStmtEvaluationPlan::EventBody {
                    trigger:
                        HirStmtTriggerEvaluationPlan::Signal {
                            target,
                            value: Some(value),
                        },
                    scope,
                    body,
                },
            ) => {
                assert_eq!(target, first_expr);
                assert_eq!(value, first_pattern);
                assert_eq!(scope, body_scope);
                assert_eq!(body, [first_statement]);
            }
            (13, HirStmtEvaluationPlan::UnsafeLifetime { audit, body }) => {
                assert_eq!(audit.reason(), Some(second_expr));
                assert!(audit.has_safety_doc());
                assert_eq!(body.scope(), Some(body_scope));
                assert_eq!(body.statements(), [first_statement]);
            }
            (
                15,
                HirStmtEvaluationPlan::If {
                    condition,
                    then_body,
                    else_branch: None,
                },
            ) => {
                assert_eq!(condition, first_expr);
                assert_eq!(then_body.scope(), body_scope);
            }
            (
                16,
                HirStmtEvaluationPlan::IfLet {
                    pattern,
                    scrutinee,
                    guard,
                    branch_locals,
                    then_body,
                    else_branch: None,
                },
            ) => {
                assert_eq!(pattern, first_pattern);
                assert_eq!(scrutinee, first_expr);
                assert_eq!(guard, Some(second_expr));
                assert_eq!(branch_locals, [first_local]);
                assert_eq!(then_body.scope(), body_scope);
            }
            (17, HirStmtEvaluationPlan::Match { scrutinee, arms }) => {
                assert_eq!(scrutinee, first_expr);
                assert_eq!(arms.len(), 1);
                assert_eq!(arms[0].pattern(), first_pattern);
                assert_eq!(arms[0].guard(), Some(second_expr));
            }
            (18, HirStmtEvaluationPlan::While { condition, body }) => {
                assert_eq!(condition, first_expr);
                assert_eq!(body.scope(), body_scope);
            }
            (
                19,
                HirStmtEvaluationPlan::WhileLet {
                    pattern,
                    scrutinee,
                    guard,
                    branch_locals,
                    body,
                },
            ) => {
                assert_eq!(pattern, first_pattern);
                assert_eq!(scrutinee, first_expr);
                assert_eq!(guard, Some(second_expr));
                assert_eq!(branch_locals, [first_local]);
                assert_eq!(body.scope(), body_scope);
            }
            (
                20,
                HirStmtEvaluationPlan::For {
                    source,
                    iterator,
                    next_value,
                    pattern,
                    branch_locals,
                    body,
                },
            ) => {
                assert_eq!(
                    (source, iterator, next_value),
                    (first_expr, second_expr, third_expr)
                );
                assert_eq!(pattern, first_pattern);
                assert_eq!(branch_locals, [first_local]);
                assert_eq!(body.scope(), body_scope);
            }
            (
                22,
                HirStmtEvaluationPlan::Select {
                    scope: Some(scope),
                    plan: HirStmtSelectEvaluationPlan::Branches { branches },
                },
            ) => {
                assert_eq!(scope, select_scope);
                assert_eq!(branches.len(), 1);
                let mut entries = branches.entries();
                let branch = entries.next().expect("select branch");
                match branch.head() {
                    HirStmtSelectHeadEvaluation::Bind { binding, source } => {
                        assert_eq!(binding.resolved(), Some(first_local));
                        assert_eq!(source, first_expr);
                    }
                    other => panic!("unexpected select head: {other:?}"),
                }
                assert_eq!(branch.body().scope(), select_body_scope);
                assert!(entries.next().is_none());
            }
            (23, HirStmtEvaluationPlan::SourceLocale { locale, body }) => {
                assert_eq!(
                    locale,
                    &HirSourceLocaleValue::Recovered(HirSourceLocaleIssue::Missing)
                );
                assert_eq!(body.scope(), scope);
            }
            (24, HirStmtEvaluationPlan::Scope { name, body }) => {
                assert_eq!(name.map(HirName::as_str), Some("scope_name"));
                assert_eq!(body.scope(), scope);
            }
            (25, HirStmtEvaluationPlan::Include { target }) => {
                assert_eq!(target, &include_target);
            }
            (
                26,
                HirStmtEvaluationPlan::Value {
                    kind: HirStmtValuePlanKind::Break,
                    expression: Some(value),
                    label: Some(label),
                    outcome: None,
                },
            ) => {
                assert_eq!(value, third_expr);
                assert_eq!(label.as_str(), "break_label");
            }
            (27, HirStmtEvaluationPlan::Continue { label: Some(label) }) => {
                assert_eq!(label.as_str(), "continue_label");
            }
            (
                28,
                HirStmtEvaluationPlan::Value {
                    kind: HirStmtValuePlanKind::Expression,
                    expression: Some(value),
                    label: None,
                    outcome: None,
                },
            ) => assert_eq!(value, fourth_expr),
            (30, HirStmtEvaluationPlan::Recovered) => {}
            _ => panic!("{family} did not project its typed evaluation plan"),
        }
    }
}

#[test]
fn evaluation_steps_interleave_thread_else_if_match_and_select_bodies() {
    let owner_module = module(17);
    let thread_scope = id::<ScopeId>(owner_module, 2);
    let match_scope = id::<ScopeId>(owner_module, 3);
    let select_scope = id::<ScopeId>(owner_module, 4);
    let select_body_scope = id::<ScopeId>(owner_module, 5);
    let condition = id::<ExprId>(owner_module, 6);
    let scrutinee = id::<ExprId>(owner_module, 7);
    let guard = id::<ExprId>(owner_module, 8);
    let arm_value = id::<ExprId>(owner_module, 9);
    let source = id::<ExprId>(owner_module, 10);
    let pattern = id::<PatternId>(owner_module, 11);
    let local = id::<LocalId>(owner_module, 12);
    let thread_statement = id::<StmtId>(owner_module, 13);
    let else_if_statement = id::<StmtId>(owner_module, 14);

    assert_if_evaluation_steps(thread_scope, condition, thread_statement, else_if_statement);
    assert_match_evaluation_steps(
        match_scope,
        scrutinee,
        pattern,
        guard,
        arm_value,
        local,
        thread_statement,
    );
    assert_select_evaluation_steps(
        select_scope,
        select_body_scope,
        source,
        local,
        thread_statement,
    );
}

fn assert_if_evaluation_steps(
    thread_scope: ScopeId,
    condition: ExprId,
    thread_statement: StmtId,
    else_if_statement: StmtId,
) {
    let thread_body = HirThreadBody::try_new(
        HirThreadBodyOwner::NestedScope(thread_scope),
        thread_scope,
        Box::new([HirThreadFlowItem::Statement(thread_statement)]),
    )
    .expect("thread body");
    let if_statement = HirIfStmt::try_new(
        condition,
        HirContextualStmtBody::try_thread(thread_body).expect("thread body context"),
        Some(HirConditionalElseBranch::else_if(else_if_statement)),
    )
    .expect("thread if");
    let if_kind = HirStmtKind::If(if_statement);
    let if_steps = collect_evaluation_steps(&if_kind);
    assert!(matches!(
        if_steps.as_slice(),
        [
            HirStmtEvaluationStep::Expression { expression, .. },
            HirStmtEvaluationStep::ThreadBody { edge, .. },
            HirStmtEvaluationStep::Statement { role: HirStatementChildRole::ElseIf, statement },
        ] if *expression == condition
            && edge.child() == crate::body_edges::HirBodyChild::Statement(thread_statement)
            && *statement == else_if_statement
    ));
}

fn assert_match_evaluation_steps(
    match_scope: ScopeId,
    scrutinee: ExprId,
    pattern: PatternId,
    guard: ExprId,
    arm_value: ExprId,
    local: LocalId,
    thread_statement: StmtId,
) {
    let first_arm = HirStmtMatchArm::try_new(
        match_scope,
        pattern,
        Some(guard),
        HirStmtMatchArmBody::Body(
            HirContextualStmtBody::try_thread(
                HirThreadBody::try_new(
                    HirThreadBodyOwner::NestedScope(match_scope),
                    match_scope,
                    Box::new([HirThreadFlowItem::Statement(thread_statement)]),
                )
                .expect("match thread body"),
            )
            .expect("match thread body context"),
        ),
        Box::new([local]),
    )
    .expect("thread match arm");
    let second_pattern = id::<PatternId>(match_scope.module(), 16);
    let second_arm = HirStmtMatchArm::try_new(
        id::<ScopeId>(match_scope.module(), 15),
        second_pattern,
        None,
        HirStmtMatchArmBody::Expression(arm_value),
        Box::new([]),
    )
    .expect("ordinary match arm");
    let match_statement =
        HirMatchStmt::try_new(scrutinee, Box::new([first_arm, second_arm])).expect("mixed match");
    let match_kind = HirStmtKind::Match(match_statement);
    let match_steps = collect_evaluation_steps(&match_kind);
    assert!(matches!(
        match_steps.as_slice(),
        [
            HirStmtEvaluationStep::Expression { expression: first, .. },
            HirStmtEvaluationStep::Pattern { .. },
            HirStmtEvaluationStep::Publication {
                role: HirStmtEvaluationPublicationRole::Branch {
                    kind: HirStmtBranchPublicationKind::MatchArm { arm: 0 },
                },
                ..
            },
            HirStmtEvaluationStep::Expression { expression: first_guard, .. },
            HirStmtEvaluationStep::ThreadBody { .. },
            HirStmtEvaluationStep::Pattern { .. },
            HirStmtEvaluationStep::Publication {
                role: HirStmtEvaluationPublicationRole::Branch {
                    kind: HirStmtBranchPublicationKind::MatchArm { arm: 1 },
                },
                ..
            },
            HirStmtEvaluationStep::Expression { expression: second, .. },
        ] if *first == scrutinee && *first_guard == guard && *second == arm_value
    ));
}

fn assert_select_evaluation_steps(
    select_scope: ScopeId,
    select_body_scope: ScopeId,
    source: ExprId,
    local: LocalId,
    thread_statement: StmtId,
) {
    let select_branch = HirSelectBranch::try_new(
        HirSelectBranchHead::Bind {
            binding: HirSelectBindingLocal::Resolved(local),
            source,
        },
        ordinary_body(select_body_scope, Box::new([thread_statement])),
    )
    .expect("select branch");
    let select = HirStmtKind::Select(
        HirSelectStmt::try_branches(select_scope, Box::new([select_branch]))
            .expect("select statement"),
    );
    let select_steps = collect_evaluation_steps(&select);
    assert!(matches!(
        select_steps.as_slice(),
        [
            HirStmtEvaluationStep::Expression { expression, .. },
            HirStmtEvaluationStep::Local { local: published, .. },
            HirStmtEvaluationStep::Publication {
                role: HirStmtEvaluationPublicationRole::Branch {
                    kind: HirStmtBranchPublicationKind::SelectBranch { branch: 0 },
                },
                ..
            },
            HirStmtEvaluationStep::Statement { statement, .. },
        ] if *expression == source && *published == local && *statement == thread_statement
    ));
}

fn collect_evaluation_steps(statement: &HirStmtKind) -> Vec<HirStmtEvaluationStep<'_>> {
    let mut steps = Vec::new();
    statement
        .evaluation_plan()
        .try_visit_evaluation_steps(|step| steps.push(step))
        .expect("evaluation steps");
    steps
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
    let audit = HirUnsafeAudit::new(unsafe_audit_identity(), Some(reason), true);

    assert!(matches!(
        audit.identity(),
        HirUnsafeAuditIdentity::Accepted(id)
            if id.as_public_id().as_str() == "unsafe.audit"
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
    let missing_identity =
        || HirUnsafeAuditIdentity::Recovered(HirUnsafeAuditIdentityIssue::Missing);

    assert_eq!(
        HirStmt::try_new(
            owner_scope,
            HirStmtKind::UnsafeLifetime {
                audit: HirUnsafeAudit::new(missing_identity(), None, false),
                body: body(),
            },
        ),
        Err(HirStmtInvariantError::InvalidPoisonState)
    );
    assert_eq!(
        HirStmt::try_new(
            owner_scope,
            HirStmtKind::UnsafeLifetime {
                audit: HirUnsafeAudit::new(unsafe_audit_identity(), None, false),
                body: HirUnsafeLifetimeBody::Missing,
            },
        ),
        Err(HirStmtInvariantError::InvalidPoisonState)
    );
    assert_eq!(
        HirStmt::try_new_with_state(
            owner_scope,
            HirStmtKind::UnsafeLifetime {
                audit: HirUnsafeAudit::new(unsafe_audit_identity(), None, false),
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
                audit: HirUnsafeAudit::new(unsafe_audit_identity(), None, false),
                body: body(),
            },
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::InvalidAuditId(
                HirUnsafeAuditIdentityIssue::Missing,
            )),
        ),
        Err(HirStmtInvariantError::InvalidPoisonState)
    );

    assert!(
        HirStmt::try_new_with_state(
            owner_scope,
            HirStmtKind::UnsafeLifetime {
                audit: HirUnsafeAudit::new(missing_identity(), None, false),
                body: body(),
            },
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::InvalidAuditId(
                HirUnsafeAuditIdentityIssue::Missing,
            )),
        )
        .is_ok()
    );
    assert!(
        HirStmt::try_new_with_state(
            owner_scope,
            HirStmtKind::UnsafeLifetime {
                audit: HirUnsafeAudit::new(unsafe_audit_identity(), None, false),
                body: body(),
            },
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::UnclosedBody),
        )
        .is_ok()
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
