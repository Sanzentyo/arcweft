use core::fmt::Debug;
use core::num::{NonZeroU32, NonZeroU64};

use arcweft_id::LocaleTag;

use super::{
    HirConditionalElseBranch, HirContextualStmtBody, HirForStmt, HirIfLetStmt, HirIfStmt,
    HirIncludeStmt, HirMatchStmt, HirScopeStmt, HirSelectBindingLocal, HirSelectBranch,
    HirSelectBranchHead, HirSelectStmt, HirSourceLocaleIssue, HirSourceLocaleStmt,
    HirSourceLocaleValue, HirStmtMatchArm, HirStmtMatchArmBody, HirThreadStmtInvariantError,
    HirThreadStmtRecoveryIssue, HirWhileLetStmt, HirWhileStmt,
};
use crate::expr::{HirThreadBody, HirThreadBodyOwner};
use crate::identity::{
    ExprId, HirDatabaseId, HirModuleId, HirTypedId, LocalId, PatternId, RawHirId, ScopeId, StmtId,
};
use crate::leaf::{HirEntityReference, HirIdRef, HirIdRefValue, HirName, HirNameInvariantError};

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

fn ordinary_body(module: HirModuleId, scope_slot: u32) -> HirContextualStmtBody {
    HirContextualStmtBody::try_ordinary(id(module, scope_slot), Box::new([]))
        .expect("same-module ordinary body")
}

fn thread_body(module: HirModuleId, scope_slot: u32) -> HirContextualStmtBody {
    let scope = id(module, scope_slot);
    HirContextualStmtBody::try_thread(
        HirThreadBody::try_new(HirThreadBodyOwner::NestedScope(scope), scope, Box::new([]))
            .expect("same-module Thread body"),
    )
    .expect("valid contextual Thread body")
}

fn name(value: &str) -> HirName {
    HirName::try_new(value.into()).expect("valid HIR name")
}

fn include_target() -> HirIdRefValue {
    HirIdRefValue::Resolved(HirIdRef::absolute(
        HirEntityReference::try_new("flow.chapter".into()).expect("valid Flow identity"),
    ))
}

fn assert_record_traits<T: Clone + Debug + Eq + Ord>() {}

#[test]
fn contextual_statement_body_is_one_typed_owner_for_both_execution_contexts() {
    assert_record_traits::<HirContextualStmtBody>();

    let owner = module(1);
    let ordinary_scope = id::<ScopeId>(owner, 1);
    let first = id::<StmtId>(owner, 2);
    let second = id::<StmtId>(owner, 3);
    let ordinary = HirContextualStmtBody::try_ordinary(ordinary_scope, Box::new([first, second]))
        .expect("same-module ordinary block");
    assert_eq!(ordinary.scope(), ordinary_scope);
    assert_eq!(
        ordinary.ordinary_statements(),
        Some([first, second].as_slice())
    );
    assert!(ordinary.thread_body().is_none());

    let thread = thread_body(owner, 4);
    assert!(thread.ordinary_statements().is_none());
    assert_eq!(thread.thread_body().unwrap().scope(), id(owner, 4));
    assert!(thread.thread_body_for_scope(id(owner, 4)).is_some());
    assert!(thread.thread_body_for_scope(id(owner, 5)).is_none());

    assert_eq!(
        HirContextualStmtBody::try_ordinary(ordinary_scope, Box::new([first, first])),
        Err(HirThreadStmtInvariantError::DuplicateChild)
    );
    let foreign = module(2);
    assert_eq!(
        HirContextualStmtBody::try_ordinary(ordinary_scope, Box::new([id::<StmtId>(foreign, 1)]),),
        Err(HirThreadStmtInvariantError::ForeignChild {
            expected: owner,
            actual: foreign,
        })
    );
}

#[test]
fn conditional_loop_and_for_payloads_retain_only_typed_children() {
    assert_record_traits::<HirIfStmt>();
    assert_record_traits::<HirIfLetStmt>();
    assert_record_traits::<HirWhileStmt>();
    assert_record_traits::<HirWhileLetStmt>();
    assert_record_traits::<HirForStmt>();

    let owner = module(3);
    let condition = id::<ExprId>(owner, 1);
    let pattern = id::<PatternId>(owner, 2);
    let scrutinee = id::<ExprId>(owner, 3);
    let guard = id::<ExprId>(owner, 4);
    let local = id::<LocalId>(owner, 5);
    let if_payload = HirIfStmt::try_new(
        condition,
        ordinary_body(owner, 10),
        Some(HirConditionalElseBranch::body(thread_body(owner, 11))),
    )
    .expect("same-module if payload");
    assert_eq!(if_payload.condition(), condition);
    assert!(if_payload.else_branch().is_some());

    let if_let = HirIfLetStmt::try_new(
        pattern,
        scrutinee,
        Some(guard),
        thread_body(owner, 12),
        Box::new([local]),
        Some(HirConditionalElseBranch::else_if(id(owner, 13))),
    )
    .expect("same-module if-let payload");
    assert_eq!(if_let.pattern(), pattern);
    assert_eq!(if_let.scrutinee(), scrutinee);
    assert_eq!(if_let.guard(), Some(guard));
    assert_eq!(if_let.locals(), [local]);

    let while_payload = HirWhileStmt::try_new(condition, ordinary_body(owner, 15))
        .expect("same-module while payload");
    assert_eq!(while_payload.condition(), condition);

    let while_let = HirWhileLetStmt::try_new(
        pattern,
        scrutinee,
        Some(guard),
        Box::new([local]),
        thread_body(owner, 16),
    )
    .expect("same-module while-let payload");
    assert_eq!(while_let.locals(), [local]);

    let for_payload = HirForStmt::try_new(
        id(owner, 20),
        id(owner, 21),
        id(owner, 22),
        pattern,
        Box::new([local]),
        thread_body(owner, 17),
    )
    .expect("same-module for payload");
    assert_eq!(for_payload.source(), id(owner, 20));
    assert_eq!(for_payload.iterator(), id(owner, 21));
    assert_eq!(for_payload.next_value(), id(owner, 22));

    let foreign = module(4);
    assert!(matches!(
        HirWhileStmt::try_new(id(foreign, 1), ordinary_body(owner, 18)),
        Err(HirThreadStmtInvariantError::ForeignChild { .. })
    ));
}

#[test]
fn match_arms_keep_distinct_scopes_and_contextual_bodies() {
    assert_record_traits::<HirMatchStmt>();
    assert_record_traits::<HirStmtMatchArm>();
    assert_record_traits::<HirStmtMatchArmBody>();

    let owner = module(5);
    let first_scope = id::<ScopeId>(owner, 1);
    let second_scope = id::<ScopeId>(owner, 2);
    let first = HirStmtMatchArm::try_new(
        first_scope,
        id(owner, 3),
        Some(id(owner, 4)),
        HirStmtMatchArmBody::Body(
            HirContextualStmtBody::try_ordinary(first_scope, Box::new([])).unwrap(),
        ),
        Box::new([id(owner, 5)]),
    )
    .expect("same-scope match arm");
    let second = HirStmtMatchArm::try_new(
        second_scope,
        id(owner, 6),
        None,
        HirStmtMatchArmBody::Expression(id(owner, 7)),
        Box::new([]),
    )
    .expect("same-module expression arm");
    let payload = HirMatchStmt::try_new(id(owner, 8), Box::new([first.clone(), second]))
        .expect("source-ordered distinct match arms");
    assert_eq!(payload.arms().len(), 2);

    assert_eq!(
        HirStmtMatchArm::try_new(
            first_scope,
            id(owner, 9),
            None,
            HirStmtMatchArmBody::Body(ordinary_body(owner, 10)),
            Box::new([]),
        ),
        Err(HirThreadStmtInvariantError::MismatchedBodyScope)
    );
    assert_eq!(
        HirMatchStmt::try_new(id(owner, 11), Box::new([first.clone(), first])),
        Err(HirThreadStmtInvariantError::DuplicateChild)
    );
}

#[test]
fn select_preserves_unary_and_source_ordered_branch_forms() {
    assert_record_traits::<HirSelectStmt>();
    assert_record_traits::<HirSelectBranch>();
    assert_record_traits::<HirSelectBranchHead>();

    let owner = module(6);
    let unary = HirSelectStmt::operand(id(owner, 1));
    assert!(matches!(unary, HirSelectStmt::Operand(_)));

    let bind = HirSelectBranch::try_new(
        HirSelectBranchHead::Bind {
            binding: HirSelectBindingLocal::Resolved(id(owner, 2)),
            source: id(owner, 3),
            propagates_error: true,
        },
        thread_body(owner, 10),
    )
    .expect("typed bind branch");
    let frame = HirSelectBranch::try_new(
        HirSelectBranchHead::Frame {
            pattern: id(owner, 4),
            locals: Box::new([id(owner, 5)]),
        },
        thread_body(owner, 11),
    )
    .expect("typed frame branch");
    let select_scope = id(owner, 12);
    let branches = HirSelectStmt::try_branches(select_scope, Box::new([bind, frame]))
        .expect("distinct branch scopes");
    let HirSelectStmt::Branches { scope, branches } = branches else {
        panic!("branch Select form")
    };
    assert_eq!(scope, select_scope);
    assert_eq!(branches.len(), 2);

    let duplicate_a =
        HirSelectBranch::try_new(HirSelectBranchHead::Recovered, ordinary_body(owner, 13)).unwrap();
    let duplicate_b =
        HirSelectBranch::try_new(HirSelectBranchHead::Recovered, ordinary_body(owner, 13)).unwrap();
    assert_eq!(
        HirSelectStmt::try_branches(id(owner, 14), Box::new([duplicate_a, duplicate_b])),
        Err(HirThreadStmtInvariantError::DuplicateChild)
    );
}

#[test]
fn locale_scope_and_include_payloads_keep_semantic_values_only() {
    assert_record_traits::<HirSourceLocaleStmt>();
    assert_record_traits::<HirSourceLocaleValue>();
    assert_record_traits::<HirScopeStmt>();
    assert_record_traits::<HirIncludeStmt>();

    let owner = module(7);
    let locale = HirSourceLocaleStmt::try_new(
        HirSourceLocaleValue::Resolved(LocaleTag::try_new("ja-JP").unwrap()),
        thread_body(owner, 1),
    )
    .expect("canonical locale payload");
    assert!(matches!(
        locale.locale(),
        HirSourceLocaleValue::Resolved(value) if value.as_str() == "ja-JP"
    ));
    let recovered = HirSourceLocaleStmt::try_new(
        HirSourceLocaleValue::Recovered(HirSourceLocaleIssue::Missing),
        ordinary_body(owner, 2),
    )
    .expect("typed missing-locale recovery");
    assert!(matches!(
        recovered.locale(),
        HirSourceLocaleValue::Recovered(HirSourceLocaleIssue::Missing)
    ));

    let scope = HirScopeStmt::try_new(Some(name("rain")), thread_body(owner, 3))
        .expect("named scope payload");
    assert_eq!(scope.name().unwrap().as_str(), "rain");
    let anonymous =
        HirScopeStmt::try_new(None, ordinary_body(owner, 4)).expect("anonymous scope payload");
    assert!(anonymous.name().is_none());

    let include = HirIncludeStmt::new(include_target());
    assert!(include.target().as_resolved().is_some());
}

#[test]
fn recovery_vocabulary_is_typed_and_source_independent() {
    assert_record_traits::<HirThreadStmtRecoveryIssue>();
    let issue =
        HirThreadStmtRecoveryIssue::InvalidScopeName(HirNameInvariantError::InvalidIdentifier);
    assert!(matches!(
        issue,
        HirThreadStmtRecoveryIssue::InvalidScopeName(_)
    ));
}
