use super::*;
use crate::callable::{CallableQueryLimitError, PRODUCTION_CALLABLE_LIMITS};
use crate::types::{ConstraintAcceptance, TypeKind};
use std::sync::atomic::AtomicBool;

#[test]
fn retained_alternatives_share_the_consumed_budget_without_recounting_source_work() {
    let mut work = ResolverWork::new(8);
    work.charge(2).unwrap();
    work.record_logical_argument_checks(2).unwrap();
    work.record_candidate_argument_probes(3).unwrap();
    let observed = work.call_accounting();
    let retained = RetainedResolverWork::retain(work).unwrap();
    let alternative = retained.clone();
    assert!(retained.same_ledger(&alternative));
    {
        let mut work = retained.borrow_work().unwrap();
        assert_eq!(work.remaining_budget().unwrap(), 5);
        work.charge(2).unwrap();
    }
    {
        let mut work = alternative.borrow_work().unwrap();
        assert_eq!(work.remaining_budget().unwrap(), 3);
        work.charge(3).unwrap();
        assert!(matches!(
            work.charge(1),
            Err(CallableQueryLimitError::Work {
                requested: 1,
                consumed: 8,
                limit: 8
            })
        ));
        assert_eq!(work.remaining_budget().unwrap(), 0);
    }
    assert_eq!(retained.report().unwrap(), observed);
    assert_eq!(alternative.report().unwrap(), observed);
}

#[test]
fn retained_work_has_one_checked_borrow_and_stable_preparation_observations() {
    let retained = RetainedResolverWork::retain(ResolverWork::new(16)).unwrap();
    let alias = retained.clone();
    let distinct = RetainedResolverWork::retain(ResolverWork::new(16)).unwrap();
    assert!(!retained.same_ledger(&distinct));
    assert_eq!(retained, distinct);
    let mut borrowed = retained.borrow_work().unwrap();
    borrowed.charge(2).unwrap();
    assert_eq!(
        alias.borrow_work().unwrap_err(),
        CallConstraintInvariant::RetainedCallWorkBorrowed
    );
    assert_eq!(
        alias.report().unwrap_err(),
        CallConstraintInvariant::RetainedCallWorkBorrowed
    );
    // Equality is immutable provenance, never a reentrant mutable borrow or
    // permission to exchange the actual ledgers.
    assert_eq!(retained, alias);
    drop(borrowed);
    assert_eq!(alias.borrow_work().unwrap().remaining_budget().unwrap(), 13);
    assert_eq!(retained.report().unwrap(), distinct.report().unwrap());
}

#[test]
fn retained_owner_admission_uses_the_original_inclusive_limit() {
    let exact = RetainedResolverWork::retain(ResolverWork::new(1)).unwrap();
    assert_eq!(exact.borrow_work().unwrap().remaining_budget().unwrap(), 0);
    assert!(matches!(
        RetainedResolverWork::retain(ResolverWork::new(0)),
        Err(TypeConstraintAbort::WorkLimit {
            requested: 1,
            consumed: 0,
            limit: 0
        })
    ));
}

#[test]
fn resumed_constraint_sessions_keep_original_work_and_physical_source_counts() {
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(256);
    work.charge(5).unwrap();
    work.record_logical_argument_checks(2).unwrap();
    work.record_candidate_argument_probes(2).unwrap();
    let observed = work.call_accounting();
    let retained = RetainedResolverWork::retain(work).unwrap();
    let mut previous = 250;
    let mut previous_constraint_work = 0;
    for owner in [retained.clone(), retained.clone()] {
        let mut work = owner.borrow_work().unwrap();
        let session = work
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
            .unwrap();
        session
            .with_fixture_driver::<crate::types::NoConstraintClient, _, _>(
                (),
                crate::callable::constraints::tests::no_constraint_initialization(),
                crate::types::NoConstraintClient,
                |mut driver| {
                    driver.constrain(
                        &TypeKind::I32,
                        &TypeKind::I32,
                        ConstraintAcceptance::PatternAcceptsActual,
                    );
                    driver.finish().unwrap()
                },
            )
            .unwrap();
        let remaining = work.remaining_budget().unwrap();
        let charged = work.type_constraint_report().work();
        assert!(remaining < previous);
        assert!(charged > previous_constraint_work);
        previous = remaining;
        previous_constraint_work = charged;
    }
    assert_eq!(retained.report().unwrap(), observed);
}

#[test]
fn a_root_frontier_outlives_its_driver_and_publication_reads_the_live_original_ledger() {
    let cancellation = AtomicBool::new(false);
    let retained = RetainedResolverWork::retain(ResolverWork::new(128)).unwrap();
    let equivalent_observation = RetainedResolverWork::retain(ResolverWork::new(128)).unwrap();
    let observed = retained.report().unwrap();
    let frontier = {
        let mut work = retained.borrow_work().unwrap();
        let session = work
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
            .unwrap();
        session
            .with_fixture_driver::<crate::types::NoConstraintClient, _, _>(
                (),
                crate::callable::constraints::tests::no_constraint_initialization(),
                crate::types::NoConstraintClient,
                |mut driver| {
                    driver.request_projection(
                        (),
                        &TypeKind::I32,
                        crate::types::constraints::TypeConstraintProjectionClosure::Closed,
                    );
                    driver.suspend_result(()).unwrap()
                },
            )
            .unwrap()
    };
    // The result has no borrow of the released driver or accounting session.
    assert_eq!(frontier.result().key(), &());
    assert!(retained.borrow_work().unwrap().remaining_budget().unwrap() < 127);
    assert_eq!(retained.report().unwrap(), observed);
    retained
        .borrow_work()
        .unwrap()
        .record_retained_argument_fact_publications(1)
        .unwrap();
    assert_ne!(retained.report().unwrap(), observed);
    // Semantic replay observes preparation, while publication reads the live
    // ledger. Neither comparison creates or replenishes an allowance.
    assert_eq!(retained, equivalent_observation);
}
