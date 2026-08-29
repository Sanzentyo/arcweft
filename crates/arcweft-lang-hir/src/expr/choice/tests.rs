use super::*;
use crate::expr::{
    HirExpr, HirExprInvariantError, HirExprKind, HirExpressionRecoveryIssue, HirGenericExprIssue,
    HirPoisonState, HirRecoveryIssue, HirThreadBodyOwner, HirThreadFlowItem,
};
use crate::identity::{HirDatabaseId, HirTypedId, RawHirId, ScopeId, StmtId};
use crate::leaf::{HirEntityReference, HirIdRef};
use core::fmt::Debug;
use core::hash::Hash;
use core::num::{NonZeroU32, NonZeroU64};

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

fn static_id(value: &str) -> HirIdRefValue {
    HirIdRefValue::Resolved(HirIdRef::absolute(
        HirEntityReference::try_new(value.into()).expect("valid absolute ID"),
    ))
}

fn assert_owned_traits<T: Clone + Debug + Eq + Hash + Ord + PartialOrd>() {}

#[test]
fn choice_payload_owns_static_id_and_shared_statement_bodies() {
    let owner_module = module(1);
    let expression_scope = id::<ScopeId>(owner_module, 1);
    let choice_scope = id::<ScopeId>(owner_module, 2);
    let option_scope = id::<ScopeId>(owner_module, 3);
    let select_scope = id::<ScopeId>(owner_module, 4);
    let timeout_scope = id::<ScopeId>(owner_module, 5);
    let option_id = id::<ExprId>(owner_module, 6);
    let duration = id::<ExprId>(owner_module, 7);
    let select_statement = id::<StmtId>(owner_module, 8);
    let timeout_statement = id::<StmtId>(owner_module, 9);

    let select_body = HirThreadBody::try_new(
        HirThreadBodyOwner::NestedScope(select_scope),
        select_scope,
        vec![HirThreadFlowItem::Statement(select_statement)].into_boxed_slice(),
    )
    .expect("valid select body");
    let timeout_body = HirThreadBody::try_new(
        HirThreadBodyOwner::NestedScope(timeout_scope),
        timeout_scope,
        vec![HirThreadFlowItem::Statement(timeout_statement)].into_boxed_slice(),
    )
    .expect("valid timeout body");
    let option = HirChoiceOption::new(
        option_id,
        HirChoiceOptionBody::new(
            option_scope,
            vec![HirChoiceOptionField::Select(select_body)].into_boxed_slice(),
        ),
    );
    let choice = HirChoiceExpr::new(
        Some(static_id("choice.test")),
        HirChoiceBody::new(
            choice_scope,
            vec![HirChoiceItem::Option(option)].into_boxed_slice(),
        ),
        Some(HirChoicePlan::new(
            vec![HirChoicePlanItem::Timeout {
                duration,
                body: timeout_body,
            }]
            .into_boxed_slice(),
        )),
    );

    assert!(matches!(choice.id(), Some(HirIdRefValue::Resolved(_))));
    let HirChoiceItem::Option(option) = &choice.body().items()[0] else {
        panic!("expected option candidate");
    };
    let HirChoiceOptionField::Select(body) = &option.body().fields()[0] else {
        panic!("expected shared select body");
    };
    assert_eq!(body.scope(), select_scope);

    HirExpr::try_new(
        expression_scope,
        HirExprKind::Choice(choice),
        HirPoisonState::Clean,
    )
    .expect("same-module Choice payload");
}

#[test]
fn choice_expression_rejects_a_foreign_dynamic_option_id() {
    let owner_module = module(1);
    let foreign_module = module(2);
    let expression_scope = id::<ScopeId>(owner_module, 1);
    let choice_scope = id::<ScopeId>(owner_module, 2);
    let option_scope = id::<ScopeId>(owner_module, 3);
    let foreign_option_id = id::<ExprId>(foreign_module, 4);
    let choice = HirChoiceExpr::new(
        None,
        HirChoiceBody::new(
            choice_scope,
            vec![HirChoiceItem::Option(HirChoiceOption::new(
                foreign_option_id,
                HirChoiceOptionBody::new(option_scope, Box::new([])),
            ))]
            .into_boxed_slice(),
        ),
        None,
    );

    assert_eq!(
        HirExpr::try_new(
            expression_scope,
            HirExprKind::Choice(choice),
            HirPoisonState::Clean,
        ),
        Err(HirExprInvariantError::ForeignChild {
            expected: owner_module,
            actual: foreign_module,
        })
    );
}

#[test]
fn choice_expression_rejects_foreign_trigger_children() {
    let owner_module = module(1);
    let foreign_module = module(2);
    let expression_scope = id::<ScopeId>(owner_module, 1);
    let choice_scope = id::<ScopeId>(owner_module, 2);
    let cancel_scope = id::<ScopeId>(owner_module, 3);
    let foreign_trigger = id::<ExprId>(foreign_module, 4);
    let cancel_body = HirThreadBody::try_new(
        HirThreadBodyOwner::NestedScope(cancel_scope),
        cancel_scope,
        Box::new([]),
    )
    .expect("valid empty cancel body");
    let choice = HirChoiceExpr::new(
        None,
        HirChoiceBody::new(choice_scope, Box::new([])),
        Some(HirChoicePlan::new(
            vec![HirChoicePlanItem::Cancel {
                trigger: HirTrigger::Expression(foreign_trigger),
                body: cancel_body,
            }]
            .into_boxed_slice(),
        )),
    );

    assert_eq!(
        HirExpr::try_new(
            expression_scope,
            HirExprKind::Choice(choice),
            HirPoisonState::Clean,
        ),
        Err(HirExprInvariantError::ForeignChild {
            expected: owner_module,
            actual: foreign_module,
        })
    );
}

#[test]
fn choice_recovery_rows_require_outer_expression_poison() {
    let owner_module = module(1);
    let expression_scope = id::<ScopeId>(owner_module, 1);
    let choice_scope = id::<ScopeId>(owner_module, 2);
    let choice = HirChoiceExpr::new(
        None,
        HirChoiceBody::new(choice_scope, vec![HirChoiceItem::Error].into_boxed_slice()),
        None,
    );

    assert_eq!(
        HirExpr::try_new(
            expression_scope,
            HirExprKind::Choice(choice.clone()),
            HirPoisonState::Clean,
        ),
        Err(HirExprInvariantError::CleanRecoveryPayload)
    );
    HirExpr::try_new(
        expression_scope,
        HirExprKind::Choice(choice),
        HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
            HirExpressionRecoveryIssue::Generic(HirGenericExprIssue::TransactionalChildFailure),
        )),
    )
    .expect("recovered Choice payload with outer poison");
}

#[test]
fn choice_records_expose_the_owned_semantic_trait_family() {
    assert_owned_traits::<HirChoiceExpr>();
    assert_owned_traits::<HirChoiceBody>();
    assert_owned_traits::<HirChoiceItem>();
    assert_owned_traits::<HirChoiceOptionField>();
    assert_owned_traits::<HirChoicePlanItem>();
}
