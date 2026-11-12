use crate::types::constraints::test_support::ConstraintTestSetup;
use std::sync::atomic::AtomicBool;

use super::*;
use crate::types::constraints::transaction::{ProbeSubmission, TypeConstraintTransaction};
use crate::{
    effect_row::EffectRow,
    effects::EffectSet,
    types::{
        ArrayLength, DetachedGenericOwnerId, GenericBinder, GenericParameterOwnerId, GenericScope,
        GenericTypeParameterId,
        constraints::{
            ConstraintSourceContainerPolicy, NoConstraintClient,
            TypeConstraintParameterEligibility, TypeConstraintParameterScope,
            TypeConstraintRejection,
            context::{LocalConstraintAccounting, TypeConstraintLimits},
        },
    },
};

type TestContext<'a> = ConstraintTestSetup<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;

mod retained_preparation {
    use super::*;
    use crate::types::constraints::{SourceProbeResult, TypeConstraintProjectionClosure};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Facts {
        value: u32,
        drops: Arc<AtomicUsize>,
    }
    impl Drop for Facts {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::Relaxed);
        }
    }
    struct Domain;
    impl ConstraintDomain for Domain {
        type Application = ();
        type Source = ();
        type AlternativeIndex = ();
        type EvidenceRule = ();
        type ObservedEvidence = ();
        type CheckedEvidence = ();
        type ProbeSemanticBranch = ();
        type ProbePreparation = Facts;
        type SealedBranchValue = ();
        type Projection = ();
        type SourceErrorCause = ();
        type ClientInvariant = ();
        fn evidence_accepts((): &(), (): &()) -> bool {
            true
        }
        fn project_checked_evidence((): &(), _: &TypeKind) -> Option<()> {
            Some(())
        }
        fn alternative_ordinal((): &()) -> u32 {
            0
        }
        fn client_invariant_source((): &()) {}
        fn empty_sealed_branch() {}
    }

    #[test]
    fn forked_and_suspended_paths_share_the_original_non_clone_preparation() {
        let cancellation = AtomicBool::new(false);
        let (mut context, parameters) = ConstraintTestSetup::<_, Domain>::new(
            TypeConstraintLimits::new(4096, 2048, 128, 64),
            &cancellation,
        )
        .into_parts();
        let mut transaction =
            TypeConstraintTransaction::initialize_fixture(&mut context, (), parameters, None)
                .unwrap();
        transaction
            .begin_prepared_probe(
                &mut context,
                super::super::super::PreparedSourceConstraint::unchecked(
                    (),
                    PreparedConstraintSourceProjection::Scalar,
                ),
                super::super::super::ConstraintAcceptance::PatternAcceptsActual,
            )
            .unwrap();
        let mut ticket = transaction.next_probe(&mut context).unwrap().unwrap();
        let mut left = ticket
            .fork(&mut context)
            .unwrap()
            .observe(SourceProbeResult::unchecked(TypeKind::I32, ()))
            .unwrap();
        let right = ticket
            .observe(SourceProbeResult::unchecked(TypeKind::String, ()))
            .unwrap();
        left.append(right).unwrap();
        let drops = Arc::new(AtomicUsize::new(0));
        let contribution = left.close_preparation(Facts {
            value: 913,
            drops: Arc::clone(&drops),
        });
        transaction
            .submit_probe(
                &mut context,
                ticket.input(),
                ProbeSubmission::Accepted(contribution),
            )
            .unwrap();
        assert!(transaction.next_probe(&mut context).unwrap().is_none());
        transaction.request_projection(
            &mut context,
            (),
            &TypeKind::Unit,
            TypeConstraintProjectionClosure::Closed,
        );
        let suspended = transaction.suspend_result(&mut context, ()).unwrap();
        assert_eq!(drops.load(Ordering::Relaxed), 0);
        let resumed = suspended.resume();
        assert_eq!(resumed.frontier.len(), 2);
        let mut closed = Vec::new();
        for path in &resumed.frontier {
            closed.push(
                path.lineage.probe_trace[0]
                    .clone()
                    .close(
                        path.projection_view(),
                        &EffectSubstitution::new(),
                        &mut context,
                    )
                    .unwrap(),
            );
        }
        assert_eq!(closed[0].preparation().value, 913);
        assert!(std::ptr::eq(
            closed[0].preparation(),
            closed[1].preparation()
        ));
        assert_ne!(closed[0].actual(), closed[1].actual());
        drop(resumed);
        assert_eq!(drops.load(Ordering::Relaxed), 0);
        drop(closed);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn cancelled_preparation_admission_keeps_the_affine_input_unpublished() {
        let cancellation = AtomicBool::new(false);
        let (mut context, parameters) = ConstraintTestSetup::<_, Domain>::new(
            TypeConstraintLimits::new(4096, 2048, 128, 64),
            &cancellation,
        )
        .into_parts();
        let mut transaction =
            TypeConstraintTransaction::initialize_fixture(&mut context, (), parameters, None)
                .unwrap();
        transaction
            .begin_prepared_probe(
                &mut context,
                super::super::super::PreparedSourceConstraint::unchecked(
                    (),
                    PreparedConstraintSourceProjection::Scalar,
                ),
                super::super::super::ConstraintAcceptance::PatternAcceptsActual,
            )
            .unwrap();
        let mut ticket = transaction.next_probe(&mut context).unwrap().unwrap();
        let drops = Arc::new(AtomicUsize::new(0));
        let contribution = ticket
            .observe(SourceProbeResult::unchecked(TypeKind::I32, ()))
            .unwrap()
            .close_preparation(Facts {
                value: 7,
                drops: Arc::clone(&drops),
            });
        cancellation.store(true, Ordering::Release);
        assert!(matches!(
            transaction.submit_probe(
                &mut context,
                ticket.input(),
                ProbeSubmission::Accepted(contribution)
            ),
            Err(TypeConstraintError::Abort(
                crate::types::constraints::TypeConstraintAbort::Cancelled
            ))
        ));
        assert!(transaction.probe.as_ref().unwrap().active_input.is_some());
        assert!(transaction.frontier.is_empty());
        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }
}

fn future_parameter() -> GenericTypeParameterId {
    GenericTypeParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(719)),
        0,
    )
}

fn context(cancellation: &AtomicBool) -> TestContext<'_> {
    let scope = TypeConstraintParameterScope::new([(
        future_parameter(),
        TypeConstraintParameterEligibility::FutureEligible,
    )])
    .expect("one parameter for a later group");
    TestContext::with_scope(
        TypeConstraintLimits::new(4096, 2048, 128, 64),
        cancellation,
        scope,
    )
}

fn source(
    application: ConstraintApplicationId,
    actual: TypeKind,
) -> ConstraintProbe<NoConstraintClient> {
    ConstraintProbe::Active(ActiveConstraintProbe {
        source: ConstraintSourceId::new(application, ()),
        source_ordinal: PreparedSourceOrdinal {
            application,
            ordinal: 0,
        },
        branch: SourceProbeObservation::closed_for_test((), ()),
        selection: StoredSourceSelection::Unchecked,
        prepared_source_projection: PreparedConstraintSourceProjection::Scalar,
        value_expected: None,
        actual,
        result_origin: None,
    })
}

#[test]
fn an_unclosed_probe_preparation_cannot_issue_a_completed_source() {
    let cancellation = AtomicBool::new(false);
    let (mut context, path) = context(&cancellation).into_path();
    let mut probe = source(path.root_id(), TypeKind::I32);
    let ConstraintProbe::Active(active) = &mut probe else {
        unreachable!()
    };
    active.branch = SourceProbeObservation::new(());
    assert!(matches!(
        probe.close(
            path.projection_view(),
            &EffectSubstitution::new(),
            &mut context
        ),
        Err(TypeConstraintError::Invariant(
            super::super::super::TypeConstraintInvariant::SourceProtocol(
                TypeConstraintSourceProtocolInvariant::Outcome
            )
        ))
    ));
}

#[test]
fn source_operand_cannot_retain_an_unowned_future_type_reference() {
    let cancellation = AtomicBool::new(false);
    let (mut context, path) = context(&cancellation).into_path();
    let future = path
        .root_scope()
        .parameters()
        .type_reference(&(future_parameter()).clone().into())
        .expect("future slot");
    let probe = source(path.root_id(), TypeKind::GenericParam(future.clone()));
    assert_eq!(
        probe
            .close(
                path.projection_view(),
                &EffectSubstitution::new(),
                &mut context
            )
            .err()
            .expect("the already evaluated operand must close before materialization"),
        TypeConstraintError::Rejected(TypeConstraintRejection::IncompleteInstantiation {
            parameter: future.into(),
        }),
    );
}

#[test]
fn source_function_value_retains_its_own_type_and_constant_binder() {
    let binder = GenericBinder::new(1, 1, 0);
    let scope = GenericScope::default().with_binder(binder);
    let parameter = TypeKind::GenericParam(scope.bound_type(0, 0).expect("function type slot"));
    let function = TypeKind::function_with_binder(
        binder,
        [TypeKind::Array {
            item: Box::new(parameter.clone()),
            len: ArrayLength::Generic(scope.bound_const(0, 0).expect("function length slot")),
        }],
        parameter,
        EffectRow::closed(EffectSet::new()),
    );
    let cancellation = AtomicBool::new(false);
    let (mut context, path) = context(&cancellation).into_path();
    let closed = source(path.root_id(), function.clone())
        .close(
            path.projection_view(),
            &EffectSubstitution::new(),
            &mut context,
        )
        .unwrap_or_else(|error| panic!("an operand's own quantifiers must remain valid: {error}"));
    assert_eq!(closed.actual(), &function);
    assert!(closed.actual().semantic_identity_digest().is_ok());
    assert!(closed.final_expected().is_none());
    assert!(closed.selection().is_unchecked());
    assert_eq!(
        closed.source_projection(),
        &CheckedConstraintSourceProjection::Scalar
    );
    assert!(context.lexical_scope().binders().is_empty());
}

#[test]
fn closed_source_owns_the_normalized_actual_expectation_and_container() {
    let cancellation = AtomicBool::new(false);
    let (mut context, mut path) = context(&cancellation).into_path();
    let parameter = path
        .root_scope()
        .parameters()
        .type_reference(&(future_parameter()).clone().into())
        .expect("template slot");
    let item = TypeKind::GenericParam(parameter.clone());
    let probe = ConstraintProbe::Active(ActiveConstraintProbe {
        source: ConstraintSourceId::new(path.root_id(), ()),
        source_ordinal: PreparedSourceOrdinal {
            application: path.root_id(),
            ordinal: 3,
        },
        branch: SourceProbeObservation::closed_for_test((), ()),
        selection: StoredSourceSelection::Checked {
            alternative: (),
            evidence: Arc::new(()),
        },
        prepared_source_projection: PreparedConstraintSourceProjection::InferSpreadContainer {
            policy: ConstraintSourceContainerPolicy::Positional,
        },
        value_expected: Some(item.clone()),
        actual: TypeKind::Array {
            item: Box::new(item),
            len: ArrayLength::Const(4),
        },
        result_origin: None,
    });
    path.bindings.insert(parameter, TypeKind::I64);
    let closed = probe
        .close(
            path.projection_view(),
            &EffectSubstitution::new(),
            &mut context,
        )
        .unwrap_or_else(|error| panic!("normalized source: {error}"));
    let expected = TypeKind::Array {
        item: Box::new(TypeKind::I64),
        len: ArrayLength::Const(4),
    };
    assert_eq!(closed.actual(), &expected);
    assert_eq!(closed.final_expected(), Some(&expected));
    assert_eq!(closed.ordinal().ordinal, 3);
    assert_eq!(closed.selection().alternative(), Some(()));
    assert_eq!(closed.selection().evidence(), Some(&()));
    assert!(closed.source_projection().matches_actual(closed.actual()));
}

#[test]
fn source_trace_cannot_skip_or_repeat_completion() {
    let cancellation = AtomicBool::new(false);
    let (mut context, path) = context(&cancellation).into_path();
    let active = source(path.root_id(), TypeKind::I64);
    let expected = protocol_error(TypeConstraintSourceProtocolInvariant::Outcome);
    assert_eq!(active.clone().into_closed().err(), Some(expected.clone()));
    let closed = active
        .close(
            path.projection_view(),
            &EffectSubstitution::new(),
            &mut context,
        )
        .unwrap_or_else(|error| panic!("complete source: {error}"));
    let trace = ConstraintProbe::Closed(closed);
    assert_eq!(
        trace
            .clone()
            .close(
                path.projection_view(),
                &EffectSubstitution::new(),
                &mut context
            )
            .err(),
        Some(expected),
    );
    assert_eq!(
        trace
            .into_closed()
            .unwrap_or_else(|error| panic!("closed trace: {error}"))
            .actual(),
        &TypeKind::I64
    );
}

#[derive(Eq, PartialEq)]
struct ObservedCase {
    owner: TypeKind,
    tag: u8,
}

#[derive(Debug, Eq, PartialEq)]
struct CheckedCase {
    owner: crate::types::SemanticTypeDigest,
    tag: u8,
}

struct EvidenceDomain;

impl ConstraintDomain for EvidenceDomain {
    type Application = ();
    type Source = u8;
    type AlternativeIndex = u8;
    type EvidenceRule = ();
    type ObservedEvidence = ObservedCase;
    type CheckedEvidence = CheckedCase;
    type ProbeSemanticBranch = ();
    type ProbePreparation = ();
    type SealedBranchValue = ();
    type Projection = u8;
    type SourceErrorCause = ();
    type ClientInvariant = u8;

    fn evidence_accepts(_: &(), _: &ObservedCase) -> bool {
        true
    }
    fn project_checked_evidence(observed: &ObservedCase, actual: &TypeKind) -> Option<CheckedCase> {
        Some(CheckedCase {
            owner: actual.semantic_identity_digest().ok()?,
            tag: observed.tag,
        })
    }
    fn alternative_ordinal(index: &u8) -> u32 {
        u32::from(*index)
    }
    fn client_invariant_source(source: &u8) -> u8 {
        *source
    }
    fn empty_sealed_branch() {}
}

#[test]
fn completion_transforms_observed_evidence_into_its_distinct_checked_type() {
    let cancellation = AtomicBool::new(false);
    let parameter = future_parameter();
    let scope = TypeConstraintParameterScope::new([(
        parameter.clone(),
        TypeConstraintParameterEligibility::Bindable,
    )])
    .expect("application scope");
    let reference = scope
        .type_reference(&(parameter).clone().into())
        .expect("opened parameter");
    let (mut context, mut path) =
        ConstraintTestSetup::<LocalConstraintAccounting<'_>, EvidenceDomain>::with_scope(
            TypeConstraintLimits::new(4096, 2048, 128, 64),
            &cancellation,
            scope,
        )
        .into_path();
    let actual = TypeKind::Option(Box::new(TypeKind::GenericParam(reference.clone())));
    let observed = ObservedCase {
        owner: actual.clone(),
        tag: 7,
    };
    assert!(observed.owner.semantic_identity_digest().is_err());
    let probe = ConstraintProbe::Active(ActiveConstraintProbe {
        source: ConstraintSourceId::new(path.root_id(), 2),
        source_ordinal: PreparedSourceOrdinal {
            application: path.root_id(),
            ordinal: 0,
        },
        branch: SourceProbeObservation::closed_for_test((), ()),
        selection: StoredSourceSelection::Checked {
            alternative: 0,
            evidence: Arc::new(observed),
        },
        prepared_source_projection: PreparedConstraintSourceProjection::Scalar,
        value_expected: Some(actual.clone()),
        actual,
        result_origin: None,
    });
    path.bindings.insert(reference, TypeKind::I64);
    let closed = probe
        .close(
            path.projection_view(),
            &EffectSubstitution::new(),
            &mut context,
        )
        .unwrap_or_else(|error| panic!("complete source evidence: {error}"));
    let expected = TypeKind::Option(Box::new(TypeKind::I64));
    assert_eq!(closed.actual(), &expected);
    assert_eq!(closed.final_expected(), Some(&expected));
    assert_eq!(
        closed.selection().evidence(),
        Some(&CheckedCase {
            owner: expected.semantic_identity_digest().expect("closed owner"),
            tag: 7,
        })
    );
}
