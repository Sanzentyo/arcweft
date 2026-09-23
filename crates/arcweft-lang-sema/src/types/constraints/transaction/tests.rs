//! Candidate completion must stop before issuing source materialization work.
use crate::types::constraints::test_support::ConstraintTestSetup;

use std::{
    cell::Cell,
    sync::atomic::{AtomicBool, Ordering},
};

use super::*;
use crate::effect_row::{EffectConstraintEligibility, EffectConstraintVariable, EffectRow};
use crate::effects::EffectSet;
use crate::types::constraints::{
    NoConstraintClient, TypeConstraintFailureInvariant, TypeConstraintParameterEligibility,
    TypeConstraintParameterScope,
    context::{
        LocalConstraintAccounting, TypeConstraintContextIssuer, TypeConstraintEffectScope,
        TypeConstraintLimits, TypeConstraintWorkReport,
    },
};
use crate::types::{DetachedGenericOwnerId, GenericParameterOwnerId, GenericTypeParameterId};

struct ApplicationSourceDomain;

impl ConstraintDomain for ApplicationSourceDomain {
    type Application = u32;
    type Source = u8;
    type AlternativeIndex = u8;
    type EvidenceRule = ();
    type ObservedEvidence = ();
    type CheckedEvidence = ();
    type ProbeSemanticBranch = u8;
    type SealedBranchValue = ();
    type Projection = ();
    type SourceErrorCause = &'static str;
    type ClientInvariant = u8;

    fn evidence_accepts((): &(), (): &()) -> bool {
        true
    }
    fn project_checked_evidence((): &(), _: &TypeKind) -> Option<()> {
        Some(())
    }
    fn alternative_ordinal(index: &u8) -> u32 {
        u32::from(*index)
    }
    fn client_invariant_source(source: &u8) -> u8 {
        *source
    }
    fn empty_sealed_branch() {}
}

type ApplicationSourceContext<'a> =
    TypeConstraintContext<'a, LocalConstraintAccounting<'a>, ApplicationSourceDomain>;

fn repeated_source_component(
    cancellation: &AtomicBool,
    child_application: u32,
) -> (
    ApplicationSourceContext<'_>,
    TypeConstraintTransaction<ApplicationSourceDomain>,
) {
    // Prepare the child opening first so source precedence cannot accidentally
    // rely on monotonically issued application identities.
    let child_scope =
        ConstraintApplicationScope::new(child_application, TypeConstraintParameterScope::empty());
    let mut child_scope = Some(child_scope);
    let (mut context, parameters) =
        ConstraintTestSetup::<LocalConstraintAccounting<'_>, ApplicationSourceDomain>::new(
            TypeConstraintLimits::new(4096, 2048, 128, 128),
            cancellation,
        )
        .into_parts();
    let mut transaction =
        TypeConstraintTransaction::initialize(&mut context, 0, parameters, None).unwrap();
    for (application, ty) in [(0, TypeKind::I64), (child_application, TypeKind::String)] {
        if application != 0 {
            let scope = child_scope.take().unwrap();
            let id = scope.id();
            let path = context
                .admit_application(transaction.frontier.pop().unwrap(), scope)
                .unwrap();
            transaction =
                TypeConstraintTransaction::from_path(&mut context, id, path, None).unwrap();
        }
        transaction
            .begin_prepared_probe(
                &mut context,
                PreparedSourceConstraint::checked(
                    7,
                    PreparedConstraintSourceProjection::Scalar,
                    [],
                    super::super::PreparedSourceAlternative::new(0, (), ty.clone()),
                )
                .unwrap(),
                ConstraintAcceptance::PatternAcceptsActual,
            )
            .unwrap();
        let mut ticket = transaction.next_probe(&mut context).unwrap().unwrap();
        transaction
            .submit_probe(
                &mut context,
                ticket.input(),
                ProbeSubmission::Accepted(
                    ticket
                        .observe(SourceProbeResult::checked(ty, 0, 0, ()))
                        .expect("observed source branch"),
                ),
            )
            .unwrap();
        assert!(transaction.next_probe(&mut context).unwrap().is_none());
    }
    (context, transaction)
}

#[test]
fn application_local_source_coordinates_materialize_without_collisions() {
    let cancellation = AtomicBool::new(false);
    let (mut context, mut transaction) = repeated_source_component(&cancellation, 1);
    let equations = &transaction.test_single_path().equations;
    assert_eq!(equations.len(), 2);
    assert_ne!(equations[0].ordinal, equations[1].ordinal);
    let mut ticket = transaction
        .next_materialization_ticket(&mut context)
        .unwrap()
        .unwrap();
    let sources = ticket
        .requests()
        .map(|request| *request.source())
        .collect::<Vec<_>>();
    assert_eq!(
        sources
            .iter()
            .map(|source| source.local())
            .collect::<Vec<_>>(),
        [7, 7]
    );
    assert_ne!(sources[0].application(), sources[1].application());
    assert_eq!(
        ticket
            .requests()
            .map(|request| request.application_id())
            .collect::<Vec<_>>(),
        [0, 1]
    );
    for (request, expected) in ticket.requests().zip([TypeKind::I64, TypeKind::String]) {
        assert_eq!(request.actual(), &expected);
        assert_eq!(request.expected(), Some(&expected));
        assert!(std::ptr::eq(
            request.application(),
            request
                .component()
                .application(request.application_id())
                .unwrap(),
        ));
    }
    let component = ticket
        .requests()
        .next()
        .expect("completed source request")
        .component();
    assert_eq!(
        component
            .sources_for(0)
            .expect("root application is admitted")
            .map(|source| source.source().local())
            .collect::<Vec<_>>(),
        vec![7]
    );
    assert_eq!(
        component
            .sources_for(1)
            .expect("child application is admitted")
            .map(|source| source.source().local())
            .collect::<Vec<_>>(),
        vec![7]
    );
    assert!(component.sources_for(2).is_none());
    let binding = ticket.bind_callback().unwrap();
    assert!(binding.authorizes(&sources[0]));
    assert!(binding.authorizes(&sources[1]));
    let foreign = crate::types::constraints::test_support::source_id(7);
    assert!(!binding.authorizes(&foreign));
    let closed = ticket
        .bind_closed_submission(ClosedMaterializationSubmission::Sealed(()))
        .unwrap();
    transaction
        .submit_closed_materialization(ticket, closed)
        .unwrap();
    assert!(
        transaction
            .next_materialization_ticket(&mut context)
            .unwrap()
            .is_none()
    );
    let solved = transaction.finish(&mut context).unwrap();
    assert_eq!(solved.component.sources().all().len(), 2);
    let selected = solved.component.sources().selected().collect::<Vec<_>>();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].source(), sources[1]);
    assert_eq!(selected[0].actual(), &TypeKind::String);
}

#[test]
fn materialization_source_failures_keep_their_application() {
    for fatal in [false, true] {
        let cancellation = AtomicBool::new(false);
        let (mut context, mut transaction) = repeated_source_component(&cancellation, 1);
        let mut ticket = transaction
            .next_materialization_ticket(&mut context)
            .unwrap()
            .unwrap();
        let source = *ticket.requests().nth(1).unwrap().source();
        ticket.bind_callback().unwrap();
        let submission = if fatal {
            ClosedMaterializationSubmission::Fatal(SourceError::new(
                source,
                SourcePhase::Materialize,
                "child failure",
            ))
        } else {
            ClosedMaterializationSubmission::Rejected {
                source,
                cause: "child rejection",
            }
        };
        let closed = ticket.bind_closed_submission(submission).unwrap();
        transaction
            .submit_closed_materialization(ticket, closed)
            .unwrap();
        assert!(
            transaction
                .next_materialization_ticket(&mut context)
                .unwrap()
                .is_none()
        );
        match transaction.finish(&mut context).unwrap_err() {
            TypeConstraintFailure::FatalSource(error) if fatal => {
                assert_eq!(*error.source(), source);
            }
            TypeConstraintFailure::Rejected(TypeConstraintCandidateFailure::Source(error))
                if !fatal =>
            {
                assert_eq!(*error.source(), source);
            }
            other => panic!("unexpected completion: {other:?}"),
        }
    }
}

#[test]
fn completed_source_traces_compare_call_sites_across_fresh_openings() {
    let complete = |child_application| {
        let cancellation = AtomicBool::new(false);
        let (mut context, mut transaction) =
            repeated_source_component(&cancellation, child_application);
        let mut ticket = transaction
            .next_materialization_ticket(&mut context)
            .unwrap()
            .unwrap();
        ticket.bind_callback().unwrap();
        let closed = ticket
            .bind_closed_submission(ClosedMaterializationSubmission::Sealed(()))
            .unwrap();
        transaction
            .submit_closed_materialization(ticket, closed)
            .unwrap();
        assert!(
            transaction
                .next_materialization_ticket(&mut context)
                .unwrap()
                .is_none()
        );
        transaction.finish(&mut context).unwrap().component
    };
    let first = complete(1);
    let replay = complete(1);
    let other_call = complete(2);
    assert!(first == replay);
    assert!(first != other_call);
    assert_ne!(
        first.sources().all()[1].source(),
        replay.sources().all()[1].source()
    );
    let cancellation = AtomicBool::new(false);
    let mut accepted = false;
    for limit in 0..=128 {
        let (mut context, _) =
            ConstraintTestSetup::<LocalConstraintAccounting<'_>, ApplicationSourceDomain>::new(
                TypeConstraintLimits::new(4096, limit, 128, 128),
                &cancellation,
            )
            .into_parts();
        match first.equal_with(&replay, &mut context) {
            Err(TypeConstraintError::Abort(TypeConstraintAbort::NodeLimit {
                actual,
                limit: observed,
            })) => {
                assert_eq!((actual, observed), (limit + 1, limit));
            }
            Ok(true) => {
                accepted = true;
                break;
            }
            result => panic!("fresh openings retain one completed meaning: {result:?}"),
        }
    }
    assert!(
        accepted,
        "bounded comparison reaches the complete component"
    );
    cancellation.store(true, Ordering::Relaxed);
    let (mut context, _) = ConstraintTestSetup::<
        LocalConstraintAccounting<'_>,
        ApplicationSourceDomain,
    >::new(TypeConstraintLimits::new(4096, 0, 128, 128), &cancellation)
    .into_parts();
    assert!(matches!(
        first.equal_with(&other_call, &mut context),
        Err(TypeConstraintError::Abort(TypeConstraintAbort::Cancelled))
    ));
}

#[test]
fn materialization_failures_follow_trace_order_across_applications() {
    let cancellation = AtomicBool::new(false);
    let (mut context, mut transaction) = repeated_source_component(&cancellation, 1);
    let mut alternative = context.fork_path(&transaction.frontier[0]).unwrap();
    let source::ConstraintProbe::Active(probe) = alternative.probe_trace.last_mut().unwrap() else {
        panic!("active alternative");
    };
    probe.branch = Arc::new(1);
    transaction.frontier.push(alternative);
    let mut attempts = 0;
    let mut parent_source = None;
    while let Some(mut ticket) = transaction
        .next_materialization_ticket(&mut context)
        .unwrap()
    {
        let sources = ticket
            .requests()
            .map(|request| *request.source())
            .collect::<Vec<_>>();
        // The child opening was prepared before the parent, while the parent
        // source precedes the child in the component's trace.
        assert!(sources[1].application() < sources[0].application());
        parent_source = Some(sources[0]);
        let source = if attempts == 0 {
            sources[1]
        } else {
            sources[0]
        };
        attempts += 1;
        ticket.bind_callback().unwrap();
        let closed = ticket
            .bind_closed_submission(ClosedMaterializationSubmission::Fatal(SourceError::new(
                source,
                SourcePhase::Materialize,
                "source failure",
            )))
            .unwrap();
        transaction
            .submit_closed_materialization(ticket, closed)
            .unwrap();
    }
    assert_eq!(attempts, 2);
    let TypeConstraintFailure::FatalSource(error) = transaction.finish(&mut context).unwrap_err()
    else {
        panic!("fatal source must retain precedence");
    };
    assert_eq!(Some(*error.source()), parent_source);
}

#[test]
fn closing_a_source_rejects_a_foreign_application_or_ordinal() {
    for foreign_application in [false, true] {
        let cancellation = AtomicBool::new(false);
        let (mut context, mut transaction) = repeated_source_component(&cancellation, 1);
        let path = &mut transaction.frontier[0];
        let parent = path.applications.root_id();
        let source::ConstraintProbe::Active(probe) = path.probe_trace.last_mut().unwrap() else {
            panic!("unclosed child source");
        };
        if foreign_application {
            probe.source = crate::types::constraints::test_support::source_id(7);
        } else {
            probe.source_ordinal.application = parent;
        }
        let error = transaction
            .next_materialization_ticket(&mut context)
            .err()
            .expect("invalid source cannot materialize");
        let MaterializationImmediateFailure::Invariant(TypeConstraintFailureInvariant::Constraint(
            invariant,
        )) = error
        else {
            panic!("expected source ownership invariant");
        };
        if foreign_application {
            assert_eq!(
                invariant,
                TypeConstraintInvariant::ParameterScope(
                    super::super::TypeConstraintParameterScopeInvariant::ApplicationOutOfScope
                )
            );
        } else {
            assert_eq!(
                invariant,
                TypeConstraintInvariant::SourceProtocol(
                    TypeConstraintSourceProtocolInvariant::Outcome
                )
            );
        }
    }
}

fn scope() -> TypeConstraintParameterScope {
    TypeConstraintParameterScope::new([(parameter(), TypeConstraintParameterEligibility::Bindable)])
        .expect("one candidate parameter")
}

fn parameter() -> GenericTypeParameterId {
    GenericTypeParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(721)),
        0,
    )
}

fn effect_scope() -> (
    TypeConstraintParameterScope,
    crate::types::GenericEffectReference,
) {
    let key: crate::types::GenericEffectReference = crate::types::GenericEffectParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(721)),
        0,
    )
    .into();
    let effects = TypeConstraintEffectScope::seal_call_scope(
        [EffectConstraintVariable::new(
            key.clone(),
            EffectConstraintEligibility::Bindable,
        )],
        [],
    )
    .expect("one declared effect parameter");
    let parameters = TypeConstraintParameterScope::seal_call_scope(
        crate::types::GenericBinder::EMPTY,
        [],
        [],
        effects,
        [],
        [],
    )
    .expect("joint parameter scope");
    let variable = parameters.effect_reference(&key).expect("opened effect");
    (parameters, variable)
}

struct ForkedProbe<'a> {
    context: TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>,
    transaction: TypeConstraintTransaction<NoConstraintClient>,
    ticket: ProbeTicket<NoConstraintClient>,
    second: ProbeTicket<NoConstraintClient>,
    actual: TypeKind,
}

impl ForkedProbe<'_> {
    fn observe(&mut self) -> super::SourceProbeContribution<NoConstraintClient> {
        let mut contribution = self
            .ticket
            .observe(SourceProbeResult::unchecked(
                TypeKind::Option(Box::new(self.actual.clone())),
                (),
            ))
            .unwrap();
        contribution
            .append(
                self.second
                    .observe(SourceProbeResult::unchecked(
                        TypeKind::Vec(Box::new(self.actual.clone())),
                        (),
                    ))
                    .unwrap(),
            )
            .unwrap();
        contribution
    }
}

fn forked_probe(cancellation: &AtomicBool) -> ForkedProbe<'_> {
    let (mut context, parameters) =
        ConstraintTestSetup::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(4096, 2048, 128, 128),
            cancellation,
            scope(),
        )
        .into_parts();
    let mut transaction =
        TypeConstraintTransaction::initialize(&mut context, (), parameters, None).unwrap();
    transaction
        .begin_prepared_probe(
            &mut context,
            PreparedSourceConstraint::unchecked((), PreparedConstraintSourceProjection::Scalar),
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .unwrap();
    let mut ticket = transaction.next_probe(&mut context).unwrap().unwrap();
    let variable = ticket
        .test_path()
        .applications
        .root_scope()
        .parameters()
        .type_reference(&parameter().into())
        .unwrap();
    let mut second = ticket.fork(&mut context).unwrap();
    for (branch, value) in [
        (&mut ticket, TypeKind::I64),
        (&mut second, TypeKind::String),
    ] {
        branch.path = context
            .add_binding(
                branch.path.take().unwrap(),
                variable.clone(),
                &value,
                value.constraint_shape(),
            )
            .unwrap();
    }
    ForkedProbe {
        context,
        transaction,
        ticket,
        second,
        actual: TypeKind::GenericParam(variable),
    }
}

#[test]
fn one_probe_submission_retains_each_correlated_path_and_actual_until_materialization() {
    let cancellation = AtomicBool::new(false);
    let mut fixture = forked_probe(&cancellation);
    let input = fixture.ticket.input();
    input.with_hint(|hint| assert!(matches!(hint, ExpectedHint::Unchecked)));
    let contribution = fixture.observe();
    fixture
        .transaction
        .submit_probe(
            &mut fixture.context,
            input,
            ProbeSubmission::Accepted(contribution),
        )
        .unwrap();
    assert!(
        fixture
            .transaction
            .next_probe(&mut fixture.context)
            .unwrap()
            .is_none()
    );
    assert_eq!(fixture.transaction.frontier.len(), 2);
    fixture.transaction.close(&mut fixture.context).unwrap();
    let actuals = fixture
        .transaction
        .materialization
        .iter()
        .map(|ticket| {
            let mut requests = ticket.requests();
            let actual = requests.next().unwrap().actual().clone();
            assert!(requests.next().is_none());
            actual
        })
        .collect::<Vec<_>>();
    assert_eq!(actuals.len(), 2);
    assert!(actuals.contains(&TypeKind::Option(Box::new(TypeKind::I64))));
    assert!(actuals.contains(&TypeKind::Vec(Box::new(TypeKind::String))));
}

#[test]
fn observed_probe_branches_cannot_be_reused_or_forked() {
    let cancellation = AtomicBool::new(false);
    let mut fixture = forked_probe(&cancellation);
    let contribution = fixture
        .ticket
        .observe(SourceProbeResult::unchecked(TypeKind::I64, ()))
        .unwrap();
    assert!(matches!(
        fixture
            .ticket
            .observe(SourceProbeResult::unchecked(TypeKind::String, ())),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::SourceProtocol(TypeConstraintSourceProtocolInvariant::Ticket)
        ))
    ));
    assert!(matches!(
        fixture.ticket.fork(&mut fixture.context),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::SourceProtocol(TypeConstraintSourceProtocolInvariant::Ticket)
        ))
    ));
    fixture
        .transaction
        .submit_probe(
            &mut fixture.context,
            fixture.ticket.input(),
            ProbeSubmission::Accepted(contribution),
        )
        .unwrap();
    assert!(
        fixture
            .transaction
            .next_probe(&mut fixture.context)
            .unwrap()
            .is_none()
    );
    assert_eq!(fixture.transaction.frontier.len(), 1);
}

#[test]
fn probe_forks_keep_the_shared_budget_even_when_discarded() {
    let cancellation = AtomicBool::new(false);
    let mut fixture = forked_probe(&cancellation);
    // Initialization and the fixture's second alternative own two branches.
    for _ in 2..128 {
        drop(fixture.ticket.fork(&mut fixture.context).unwrap());
    }
    assert!(matches!(
        fixture.ticket.fork(&mut fixture.context),
        Err(TypeConstraintError::Abort(
            TypeConstraintAbort::BranchLimit {
                actual: 129,
                limit: 128
            }
        ))
    ));
    cancellation.store(true, Ordering::Relaxed);
    assert!(matches!(
        fixture.ticket.fork(&mut fixture.context),
        Err(TypeConstraintError::Abort(TypeConstraintAbort::Cancelled))
    ));
    assert!(
        fixture.ticket.path.is_some(),
        "aborted forks do not consume their source"
    );
}

#[test]
fn foreign_contribution_rejection_preserves_the_accepted_alternatives() {
    let cancellation = AtomicBool::new(false);
    let mut fixture = forked_probe(&cancellation);
    let mut foreign = forked_probe(&cancellation);
    let mut contribution = fixture.observe();
    assert_eq!(
        contribution.append(foreign.observe()),
        Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::Ticket
        ))
    );
    fixture
        .transaction
        .submit_probe(
            &mut fixture.context,
            fixture.ticket.input(),
            ProbeSubmission::Accepted(contribution),
        )
        .unwrap();
    assert!(
        fixture
            .transaction
            .next_probe(&mut fixture.context)
            .unwrap()
            .is_none()
    );
    assert_eq!(fixture.transaction.frontier.len(), 2);
}

#[test]
fn rejecting_a_probe_cannot_publish_any_of_its_retained_paths() {
    let cancellation = AtomicBool::new(false);
    let mut fixture = forked_probe(&cancellation);
    fixture
        .transaction
        .submit_probe(
            &mut fixture.context,
            fixture.ticket.input(),
            ProbeSubmission::Rejected(()),
        )
        .unwrap();
    assert!(
        fixture
            .transaction
            .next_probe(&mut fixture.context)
            .unwrap()
            .is_none()
    );
    assert!(fixture.transaction.frontier.is_empty());
    assert!(fixture.transaction.materialization.is_empty());
    assert!(matches!(
        fixture.transaction.finish(&mut fixture.context),
        Err(TypeConstraintFailure::Rejected(
            TypeConstraintCandidateFailure::Source(_)
        ))
    ));
}

#[test]
fn equal_source_coordinates_cannot_substitute_foreign_inputs_or_contributions() {
    let cancellation = AtomicBool::new(false);
    let mut fixture = forked_probe(&cancellation);
    let mut foreign = forked_probe(&cancellation);
    let foreign_input = foreign.ticket.input();
    let observation = foreign
        .ticket
        .observe(SourceProbeResult::unchecked(TypeKind::I64, ()))
        .unwrap();
    assert_eq!(
        fixture.transaction.submit_probe(
            &mut fixture.context,
            foreign_input,
            ProbeSubmission::Accepted(observation)
        ),
        Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::Ticket
        ))
    );
    let observation = foreign
        .second
        .observe(SourceProbeResult::unchecked(TypeKind::String, ()))
        .unwrap();
    assert_eq!(
        fixture.transaction.submit_probe(
            &mut fixture.context,
            fixture.ticket.input(),
            ProbeSubmission::Accepted(observation)
        ),
        Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::Ticket
        ))
    );
    let contribution = fixture.observe();
    fixture
        .transaction
        .submit_probe(
            &mut fixture.context,
            fixture.ticket.input(),
            ProbeSubmission::Accepted(contribution),
        )
        .unwrap();
    assert!(
        fixture
            .transaction
            .next_probe(&mut fixture.context)
            .unwrap()
            .is_none()
    );
    assert_eq!(fixture.transaction.frontier.len(), 2);
}

#[test]
fn an_outstanding_probe_cannot_be_skipped_by_advancing_or_closing() {
    let cancellation = AtomicBool::new(false);
    let mut fixture = forked_probe(&cancellation);
    assert!(matches!(
        fixture.transaction.next_probe(&mut fixture.context),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::SourceProtocol(TypeConstraintSourceProtocolInvariant::Ticket)
        ))
    ));
    assert_eq!(
        fixture.transaction.close(&mut fixture.context),
        Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::Outcome
        ))
    );
    assert!(fixture.transaction.materialization.is_empty());
    let contribution = fixture.observe();
    fixture
        .transaction
        .submit_probe(
            &mut fixture.context,
            fixture.ticket.input(),
            ProbeSubmission::Accepted(contribution),
        )
        .unwrap();
    assert!(
        fixture
            .transaction
            .next_probe(&mut fixture.context)
            .unwrap()
            .is_none()
    );
    fixture.transaction.close(&mut fixture.context).unwrap();
    assert_eq!(fixture.transaction.materialization.len(), 2);
}
#[test]
fn fixed_effect_evidence_survives_candidate_sealing() {
    let (parameters, variable) = effect_scope();
    let effect_key = parameters
        .effect_contract()
        .variables()
        .next()
        .expect("effect parameter")
        .variable()
        .clone();
    let cancellation = AtomicBool::new(false);
    let (mut context, test_parameters) =
        ConstraintTestSetup::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            // This test checks semantic preservation, not a fixed operation
            // count. Decision construction and completion are now metered too.
            TypeConstraintLimits::new(1_024, 1_024, 16, 16),
            &cancellation,
            parameters,
        )
        .into_parts();
    let mut transaction =
        TypeConstraintTransaction::initialize(&mut context, (), test_parameters, None).unwrap();
    let known = EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap());
    transaction.constrain_effect_equality(
        &mut context,
        &EffectRow::open(EffectSet::new(), variable),
        &known,
    );
    let solved = transaction
        .finish(&mut context)
        .expect("fixed effect solution");
    assert_eq!(
        solved
            .component
            .selected()
            .solution()
            .effect_bindings()
            .map(|(key, value)| (key.value(), value.value()))
            .collect::<Vec<_>>(),
        vec![(&effect_key, &known)],
    );
}

#[test]
fn fixed_effect_evidence_rejects_shrinking_and_expanding_function_relations() {
    for shrink in [true, false] {
        let (parameters, variable) = effect_scope();
        let cancellation = AtomicBool::new(false);
        let (mut context, test_parameters) =
            ConstraintTestSetup::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
                TypeConstraintLimits::new(1_024, 1_024, 16, 16),
                &cancellation,
                parameters,
            )
            .into_parts();
        let mut transaction =
            TypeConstraintTransaction::initialize(&mut context, (), test_parameters, None).unwrap();
        let projected = EffectRow::open(EffectSet::new(), variable);
        transaction.constrain_effect_equality(
            &mut context,
            &projected,
            &EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap()),
        );
        let projected = TypeKind::function_with_effects([], TypeKind::I64, projected);
        let incompatible = TypeKind::function_with_effects(
            [],
            TypeKind::I64,
            EffectRow::closed(if shrink {
                EffectSet::new()
            } else {
                EffectSet::from_labels(["fs.read", "fs.write"]).unwrap()
            }),
        );
        let (pattern, actual) = if shrink {
            (&incompatible, &projected)
        } else {
            (&projected, &incompatible)
        };
        transaction.constrain(
            &mut context,
            pattern,
            actual,
            ConstraintAcceptance::PatternAcceptsActual,
        );
        assert!(matches!(
            transaction.finish(&mut context),
            Err(TypeConstraintFailure::Rejected(
                TypeConstraintCandidateFailure::Constraint(TypeConstraintRejection::Mismatch)
            ))
        ));
    }
}

#[test]
fn fixed_effect_evidence_abort_cannot_publish_a_partial_equality() {
    let (parameters, variable) = effect_scope();
    let cancellation = AtomicBool::new(false);
    let (mut context, test_parameters) =
        ConstraintTestSetup::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(128, 1, 16, 16),
            &cancellation,
            parameters,
        )
        .into_parts();
    let mut transaction =
        TypeConstraintTransaction::initialize(&mut context, (), test_parameters, None).unwrap();
    transaction.constrain_effect_equality(
        &mut context,
        &EffectRow::open(EffectSet::new(), variable),
        &EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap()),
    );
    assert!(transaction.frontier.is_empty());
    assert!(transaction.materialization.is_empty());
    assert!(transaction.materialized.is_empty());
    assert!(matches!(
        transaction.finish(&mut context),
        Err(TypeConstraintFailure::Abort(
            TypeConstraintAbort::NodeLimit {
                actual: 2,
                limit: 1,
            }
        ))
    ));
}

fn source_frontier<A: TypeConstraintAccounting>(
    context: &mut TypeConstraintContext<'_, A, NoConstraintClient>,
    path: &ConstraintPath<NoConstraintClient>,
) -> TypeConstraintTransaction<NoConstraintClient> {
    let parameter = path
        .applications
        .root_scope()
        .parameters()
        .type_reference(&parameter().into())
        .expect("opened candidate parameter");
    let [first, second] = [(); 2].map(|()| {
        let mut path = context.fork_path(path).expect("admitted frontier row");
        path.bindings.insert(parameter.clone(), TypeKind::I64);
        path.probe_trace.push(source::ConstraintProbe::Active(
            source::ActiveConstraintProbe {
                source: ConstraintSourceId::new(path.applications.root_id(), ()),
                source_ordinal: PreparedSourceOrdinal {
                    application: path.applications.root_id(),
                    ordinal: 0,
                },
                branch: Arc::new(()),
                selection: StoredSourceSelection::Unchecked,
                prepared_source_projection: PreparedConstraintSourceProjection::Scalar,
                value_expected: None,
                actual: TypeKind::I64,
                result_origin: None,
            },
        ));
        path
    });
    let mut transaction =
        TypeConstraintTransaction::from_path(context, path.applications.root_id(), first, None)
            .unwrap();
    transaction.frontier.push(second);
    transaction
}

#[test]
fn advancing_materialization_preserves_the_queue_until_the_current_ticket_is_submitted() {
    let cancellation = AtomicBool::new(false);
    let (mut context, path) =
        ConstraintTestSetup::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(4096, 2048, 128, 128),
            &cancellation,
            scope(),
        )
        .into_path();
    let mut transaction = source_frontier(&mut context, &path);
    for remaining in [1, 0] {
        let mut ticket = transaction
            .next_materialization_ticket(&mut context)
            .unwrap()
            .expect("every retained path is materialized");
        assert!(matches!(
            transaction.next_materialization_ticket(&mut context),
            Err(MaterializationImmediateFailure::Invariant(
                TypeConstraintFailureInvariant::Constraint(
                    TypeConstraintInvariant::SourceProtocol(
                        TypeConstraintSourceProtocolInvariant::Ticket
                    )
                )
            ))
        ));
        assert_eq!(transaction.materialization.len(), remaining);
        transaction
            .validate_materialization_callback_begin(&ticket)
            .unwrap();
        let binding = ticket.bind_callback().unwrap();
        ticket.validate_callback_binding(&binding).unwrap();
        let closed = ticket
            .bind_closed_submission(ClosedMaterializationSubmission::Sealed(()))
            .unwrap();
        transaction
            .submit_closed_materialization(ticket, closed)
            .unwrap();
    }
    assert!(
        transaction
            .next_materialization_ticket(&mut context)
            .unwrap()
            .is_none()
    );
    transaction
        .finish(&mut context)
        .expect("both equivalent paths completed");
}

#[test]
fn a_completed_alternative_cannot_hide_an_outstanding_materialization() {
    let cancellation = AtomicBool::new(false);
    let (mut context, path) =
        ConstraintTestSetup::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(4096, 2048, 128, 128),
            &cancellation,
            scope(),
        )
        .into_path();
    let mut transaction = source_frontier(&mut context, &path);
    let mut first = transaction
        .next_materialization_ticket(&mut context)
        .unwrap()
        .unwrap();
    first.bind_callback().unwrap();
    let closed = first
        .bind_closed_submission(ClosedMaterializationSubmission::Sealed(()))
        .unwrap();
    transaction
        .submit_closed_materialization(first, closed)
        .unwrap();
    let _outstanding = transaction
        .next_materialization_ticket(&mut context)
        .unwrap()
        .unwrap();
    assert!(matches!(
        transaction.finish(&mut context),
        Err(TypeConstraintFailure::Invariant(
            TypeConstraintFailureInvariant::Constraint(TypeConstraintInvariant::SourceProtocol(
                TypeConstraintSourceProtocolInvariant::Outcome
            ))
        ))
    ));
}

#[test]
fn completion_and_comparison_limits_prevent_partial_materialization_ticket_issuance() {
    let cancellation = AtomicBool::new(false);
    let mut first_accepted_limit = None;
    for limit in 4..=256 {
        let (mut context, path) =
            ConstraintTestSetup::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
                TypeConstraintLimits::new(4096, limit, 16, 16),
                &cancellation,
                scope(),
            )
            .into_path();
        let mut transaction = source_frontier(&mut context, &path);
        let result = transaction.close(&mut context);
        match result {
            Err(TypeConstraintError::Abort(TypeConstraintAbort::NodeLimit {
                actual,
                limit: observed,
            })) => {
                assert_eq!(observed, limit);
                assert_eq!(actual, limit + 1);
                assert!(transaction.materialization.is_empty());
                assert!(transaction.materialized.is_empty());
                assert!(
                    transaction
                        .next_materialization_ticket(&mut context)
                        .unwrap()
                        .is_none()
                );
                assert!(
                    matches!(transaction.finish(&mut context), Err(TypeConstraintFailure::Abort(
                    TypeConstraintAbort::NodeLimit { actual, limit: observed }
                )) if actual == limit + 1 && observed == limit)
                );
            }
            Ok(()) => {
                assert_eq!(transaction.materialization.len(), 2);
                assert!(transaction.materialized.is_empty());
                first_accepted_limit = Some(limit);
                break;
            }
            Err(error) => panic!("unexpected completion failure at limit {limit}: {error:?}"),
        }
    }
    assert!(first_accepted_limit.is_some_and(|limit| limit > 5));
}

#[test]
fn incomplete_alternatives_are_rejected_before_any_materialization() {
    let cancellation = AtomicBool::new(false);
    for (retain_valid_alternative, source_demand) in
        [(false, false), (true, false), (false, true), (true, true)]
    {
        let (mut context, path) =
            ConstraintTestSetup::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
                TypeConstraintLimits::new(4096, 2048, 128, 128),
                &cancellation,
                scope(),
            )
            .into_path();
        let mut transaction = source_frontier(&mut context, &path);
        for path in transaction
            .frontier
            .iter_mut()
            .skip(usize::from(retain_valid_alternative))
        {
            if source_demand {
                let unresolved = path.bindings.keys().next().unwrap().clone();
                let source::ConstraintProbe::Active(probe) = &mut path.probe_trace[0] else {
                    panic!("the source has not been closed");
                };
                probe.actual = TypeKind::GenericParam(unresolved);
            }
            path.bindings.clear();
        }
        let ticket = transaction
            .next_materialization_ticket(&mut context)
            .unwrap();
        if retain_valid_alternative {
            let mut ticket = ticket.expect("only the complete alternative is materialized");
            assert_eq!(ticket.requests().len(), 1);
            ticket.bind_callback().unwrap();
            let closed = ticket
                .bind_closed_submission(ClosedMaterializationSubmission::Sealed(()))
                .unwrap();
            transaction
                .submit_closed_materialization(ticket, closed)
                .unwrap();
            assert!(
                transaction
                    .next_materialization_ticket(&mut context)
                    .unwrap()
                    .is_none()
            );
            transaction
                .finish(&mut context)
                .expect("complete alternative remains viable");
        } else {
            assert!(ticket.is_none());
            assert!(transaction.materialization.is_empty());
            assert!(transaction.materialized.is_empty());
            assert!(
                transaction
                    .next_materialization_ticket(&mut context)
                    .unwrap()
                    .is_none()
            );
            assert!(matches!(
                transaction.finish(&mut context),
                Err(TypeConstraintFailure::Rejected(
                    TypeConstraintCandidateFailure::Constraint(
                        TypeConstraintRejection::IncompleteInstantiation { .. }
                    )
                ))
            ));
        }
    }
}

struct CancelBeforeComparison<'a> {
    cancellation: &'a AtomicBool,
    nodes: u64,
}

#[derive(Default)]
struct AccountingReceipt {
    commits: Cell<u32>,
    branches: Cell<u64>,
}

struct ContextLifetimeAccounting<'a> {
    cancellation: &'a AtomicBool,
    receipt: &'a AccountingReceipt,
    proposed: TypeConstraintWorkReport,
    committed: bool,
}

impl TypeConstraintAccounting for ContextLifetimeAccounting<'_> {
    fn charge_constraint(
        &mut self,
        delta: &TypeConstraintWorkReport,
        _limits: TypeConstraintLimits,
    ) -> Result<(), TypeConstraintError> {
        self.proposed = self.proposed.checked_add(delta)?;
        Ok(())
    }

    fn commit(&mut self) {
        if !self.committed {
            self.receipt.branches.set(self.proposed.branches);
            self.receipt.commits.set(self.receipt.commits.get() + 1);
            self.committed = true;
        }
    }
}

impl<'a> TypeConstraintContextIssuer<'a> for ContextLifetimeAccounting<'a> {
    fn context_limits(&self) -> TypeConstraintLimits {
        TypeConstraintLimits::new(4096, 2048, 1, 128)
    }

    fn context_cancellation(&self) -> &'a AtomicBool {
        self.cancellation
    }
}

#[test]
fn transaction_completion_cannot_release_or_reset_the_surrounding_context() {
    let cancellation = AtomicBool::new(false);
    for complete in [false, true] {
        let receipt = AccountingReceipt::default();
        let mut context = TypeConstraintContext::with_accounting(ContextLifetimeAccounting {
            cancellation: &cancellation,
            receipt: &receipt,
            proposed: TypeConstraintWorkReport::default(),
            committed: false,
        });
        let first = TypeConstraintTransaction::<NoConstraintClient>::initialize(
            &mut context,
            (),
            TypeConstraintParameterScope::empty(),
            None,
        )
        .unwrap();
        if complete {
            first.finish(&mut context).unwrap();
        } else {
            drop(first);
        }
        assert_eq!(receipt.commits.get(), 0);

        assert!(matches!(
            TypeConstraintTransaction::<NoConstraintClient>::initialize(
                &mut context,
                (),
                TypeConstraintParameterScope::empty(),
                None,
            ),
            Err(super::super::TypeConstraintInitializationFailure::Abort(
                TypeConstraintAbort::BranchLimit {
                    actual: 2,
                    limit: 1
                }
            ))
        ));
        assert_eq!(receipt.commits.get(), 0);
        drop(context);
        assert_eq!(receipt.commits.get(), 1);
        assert_eq!(receipt.branches.get(), 1);
    }
}

impl TypeConstraintAccounting for CancelBeforeComparison<'_> {
    fn charge_constraint(
        &mut self,
        delta: &TypeConstraintWorkReport,
        _limits: TypeConstraintLimits,
    ) -> Result<(), TypeConstraintError> {
        self.nodes += delta.nodes();
        if self.nodes == 4 {
            self.cancellation.store(true, Ordering::Release);
        }
        Ok(())
    }

    fn commit(&mut self) {}
}

impl<'a> TypeConstraintContextIssuer<'a> for CancelBeforeComparison<'a> {
    fn context_limits(&self) -> TypeConstraintLimits {
        TypeConstraintLimits::new(128, 128, 16, 16)
    }

    fn context_cancellation(&self) -> &'a AtomicBool {
        self.cancellation
    }
}

#[test]
fn comparison_cancellation_prevents_materialization_ticket_issuance() {
    let cancellation = AtomicBool::new(false);
    let (mut context, path) = ConstraintTestSetup::<_, NoConstraintClient>::with_accounting(
        CancelBeforeComparison {
            cancellation: &cancellation,
            nodes: 0,
        },
        scope(),
    )
    .into_path();
    let mut transaction = source_frontier(&mut context, &path);
    assert_eq!(
        transaction.close(&mut context),
        Err(TypeConstraintError::Abort(TypeConstraintAbort::Cancelled))
    );
    assert!(transaction.materialization.is_empty());
    assert!(transaction.materialized.is_empty());
}
