use super::*;
use crate::types::{
    DetachedGenericOwnerId, GenericParameterOwnerId, GenericTypeParameterId, TypeKind,
    constraints::{
        ConstraintAcceptance, NoConstraintClient, PreparedConstraintSourceProjection,
        PreparedSourceConstraint, SourceProbeResult, TypeConstraintAbort, TypeConstraintInvariant,
        TypeConstraintParameterEligibility, TypeConstraintParameterScope,
        context::{LocalConstraintAccounting, TypeConstraintLimits},
        test_support::ConstraintTestSetup,
        transaction::ProbeSubmission,
    },
};
use std::sync::atomic::{AtomicBool, Ordering};

type Context<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;
type Transaction = TypeConstraintTransaction<NoConstraintClient>;

fn root<'a>(
    cancellation: &'a AtomicBool,
    parameters: TypeConstraintParameterScope,
    work: u64,
) -> (Context<'a>, Transaction) {
    let (mut context, parameters) = ConstraintTestSetup::with_scope(
        TypeConstraintLimits::new(work, 1024, 128, 128),
        cancellation,
        parameters,
    )
    .into_parts();
    let transaction = Transaction::initialize_fixture(&mut context, (), parameters, None).unwrap();
    (context, transaction)
}

fn result(context: &mut Context<'_>, transaction: &mut Transaction, ty: &TypeKind) {
    transaction.request_projection(
        context,
        (),
        ty,
        super::super::super::TypeConstraintProjectionClosure::Closed,
    );
}

#[test]
fn an_unbound_root_resumes_without_issuing_an_early_solution_or_replacing_its_opening() {
    let cancellation = AtomicBool::new(false);
    let parameter = GenericTypeParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(913)),
        0,
    );
    let parameters = TypeConstraintParameterScope::new([(
        parameter.clone(),
        TypeConstraintParameterEligibility::Bindable,
    )])
    .unwrap();
    let template = TypeKind::generic_parameter(parameter);
    let (mut context, mut transaction) = root(&cancellation, parameters, 1024);
    result(&mut context, &mut transaction, &template);
    let opening = transaction.application;
    let issuer = Arc::clone(&transaction.materialization_issuer);
    let equation = transaction.next_equation;
    let suspended = transaction.suspend_result(&mut context, ()).unwrap();
    assert_eq!(suspended.result().application(), opening);
    assert_eq!(suspended.result().key(), &());
    assert!(suspended.transaction.materialization.is_empty());
    assert!(suspended.transaction.materialized.is_empty());
    assert!(suspended.transaction.frontier[0].bindings.is_empty());
    let mut resumed = suspended.resume();
    assert!(Arc::ptr_eq(&issuer, &resumed.materialization_issuer));
    assert_eq!(resumed.next_equation, equation);
    resumed.constrain(
        &mut context,
        &template,
        &TypeKind::I32,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    assert_eq!(resumed.next_equation, equation + 1);
    let solved = resumed
        .finish(&mut context, |left, right, _| Ok(left == right))
        .unwrap();
    assert_eq!(
        solved
            .component
            .projection((), &())
            .unwrap()
            .projection()
            .value()
            .value(),
        &TypeKind::I32,
    );
}

#[test]
fn suspension_retains_the_original_source_trace_scope_and_source_ordinal() {
    let cancellation = AtomicBool::new(false);
    let (mut context, mut transaction) =
        root(&cancellation, TypeConstraintParameterScope::empty(), 1024);
    result(&mut context, &mut transaction, &TypeKind::I32);
    transaction
        .begin_prepared_probe(
            &mut context,
            PreparedSourceConstraint::unchecked((), PreparedConstraintSourceProjection::Scalar),
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .unwrap();
    let mut ticket = transaction.next_probe(&mut context).unwrap().unwrap();
    transaction
        .submit_probe(
            &mut context,
            ticket.input(),
            ProbeSubmission::Accepted(
                (ticket
                    .observe(SourceProbeResult::unchecked(TypeKind::I32, ()))
                    .unwrap())
                .close_preparation(()),
            ),
        )
        .unwrap();
    assert!(transaction.next_probe(&mut context).unwrap().is_none());
    let scopes = Arc::clone(&transaction.frontier[0].applications);
    let prepared = transaction.prepared_sources.clone();
    let ordinal = transaction.next_source_ordinal;
    let suspended = transaction.suspend_result(&mut context, ()).unwrap();
    let mut resumed = suspended.resume();
    assert!(Arc::ptr_eq(&scopes, &resumed.frontier[0].applications));
    assert_eq!(resumed.prepared_sources, prepared);
    assert_eq!(resumed.next_source_ordinal, ordinal);
    let materialization = resumed
        .next_materialization_ticket(&mut context)
        .unwrap()
        .unwrap();
    let requests = materialization.requests().collect::<Vec<_>>();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].actual(), &TypeKind::I32);
    assert!(prepared.contains(requests[0].source()));
}

#[test]
fn a_root_cannot_be_attached_as_an_unrelated_parent_source_or_suspended_after_closure() {
    let cancellation = AtomicBool::new(false);
    let (mut context, mut transaction) =
        root(&cancellation, TypeConstraintParameterScope::empty(), 1024);
    result(&mut context, &mut transaction, &TypeKind::I32);
    let suspended = transaction.suspend_result(&mut context, ()).unwrap();
    assert!(matches!(
        suspended.into_child(&mut context),
        Err(TypeConstraintFailure::Invariant(
            super::super::super::TypeConstraintFailureInvariant::Constraint(
                TypeConstraintInvariant::SourceProtocol(
                    TypeConstraintSourceProtocolInvariant::WrongPhase
                )
            )
        ))
    ));
    let (mut context, mut transaction) =
        root(&cancellation, TypeConstraintParameterScope::empty(), 1024);
    result(&mut context, &mut transaction, &TypeKind::I32);
    assert!(
        transaction
            .next_materialization_ticket(&mut context)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        transaction.suspend_result(&mut context, ()),
        Err(TypeConstraintFailure::Invariant(_))
    ));
}

#[test]
fn suspension_checks_unique_projections_cancellation_and_the_original_inclusive_budget() {
    let cancellation = AtomicBool::new(false);
    for count in [0, 2] {
        let (mut context, mut transaction) =
            root(&cancellation, TypeConstraintParameterScope::empty(), 1024);
        for _ in 0..count {
            result(&mut context, &mut transaction, &TypeKind::I32);
        }
        assert!(matches!(
            transaction.suspend_result(&mut context, ()),
            Err(TypeConstraintFailure::Invariant(_))
        ));
    }
    for (limit, succeeds) in [(3, true), (2, false)] {
        // Keep the suspension budget independent of the separately tested
        // source-admission budget by supplying an already admitted fixture.
        let (mut context, parameters) = ConstraintTestSetup::new(
            TypeConstraintLimits::new(limit, 1024, 128, 128),
            &cancellation,
        )
        .into_parts();
        let original =
            super::super::super::OriginalProducerContext::ground_fixture(Default::default());
        let mut transaction =
            Transaction::initialize(&mut context, original, (), parameters, None).unwrap();
        result(&mut context, &mut transaction, &TypeKind::I32);
        let suspended = transaction.suspend_result(&mut context, ());
        assert_eq!(suspended.is_ok(), succeeds);
        if !succeeds {
            assert!(matches!(
                suspended,
                Err(TypeConstraintFailure::Abort(
                    TypeConstraintAbort::WorkLimit {
                        requested: 1,
                        consumed: 2,
                        limit: 2
                    }
                ))
            ));
        }
    }
    let (mut context, mut transaction) =
        root(&cancellation, TypeConstraintParameterScope::empty(), 1024);
    result(&mut context, &mut transaction, &TypeKind::I32);
    cancellation.store(true, Ordering::Release);
    assert!(matches!(
        transaction.suspend_result(&mut context, ()),
        Err(TypeConstraintFailure::Abort(TypeConstraintAbort::Cancelled))
    ));
}

#[test]
fn independent_roots_with_equal_result_shapes_keep_distinct_projection_authorities() {
    let cancellation = AtomicBool::new(false);
    let mut roots = Vec::new();
    for _ in 0..2 {
        let (mut context, mut transaction) =
            root(&cancellation, TypeConstraintParameterScope::empty(), 1024);
        result(&mut context, &mut transaction, &TypeKind::I32);
        roots.push(transaction.suspend_result(&mut context, ()).unwrap());
    }
    assert!(roots[0].result() != roots[1].result());
}
