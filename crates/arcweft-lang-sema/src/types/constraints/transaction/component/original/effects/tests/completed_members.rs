use super::*;
use crate::effect_row::prepared::{PreparedEffectPredicate, PreparedEffectUnknownEligibility};
use crate::types::constraints::transaction::operand::ConstraintEffectPhase;
use crate::types::constraints::{ConstraintAcceptance, TypeConstraintSourceProtocolInvariant};
use crate::types::{
    GenericBinder,
    constraints::transaction::{
        ActiveConstraintProbe, ConstraintEquationId, ConstraintEquationTiming, ConstraintOperand,
        ConstraintSourceId, PendingEquation, PreparedConstraintSourceProjection,
        PreparedSourceOrdinal, SourceProbeObservation, StoredSourceSelection,
    },
};

fn path_mut(
    component: &mut Component,
    member: OriginalProductMember,
) -> &mut ConstraintPath<NoConstraintClient> {
    let value = component.values.get_mut(&member.producer).unwrap();
    let ValueDefinition::Constraint { frontiers } = &mut value.definition else {
        panic!("original root")
    };
    &mut frontiers[member.frontier].transaction.frontier[member.path]
}

fn operand(
    value: &TypeKind<PreparedEffects>,
    path: &ConstraintPath<NoConstraintClient>,
    graph: &mut PreparedEffectGraph,
    context: &mut Context<'_>,
) -> ConstraintOperand<NoConstraintClient> {
    ConstraintOperand::from_prepared_original_in(
        value,
        Arc::new(path.lineage.original.gamma().clone()),
        path.lineage.original.clone(),
        &mut ConstraintEffectPhase::new(graph),
        context,
    )
    .unwrap()
}

fn function(row: PreparedEffectRow) -> TypeKind<PreparedEffects> {
    TypeKind::function_with_contract(
        GenericBinder::EMPTY,
        PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        [],
        TypeKind::I64,
        row,
    )
}

fn materialize(
    mut owner: crate::types::constraints::transaction::ConstraintMaterialization<
        NoConstraintClient,
    >,
    context: &mut Context<'_>,
) -> crate::types::constraints::SolvedCandidate<NoConstraintClient> {
    while let Some(mut ticket) = owner.next_materialization_ticket(context).unwrap() {
        owner
            .validate_materialization_callback_begin(&ticket)
            .unwrap();
        let binding = ticket.bind_callback().unwrap();
        ticket.validate_callback_binding(&binding).unwrap();
        let closed = ticket
            .bind_closed_submission(
                crate::types::constraints::ClosedMaterializationSubmission::Sealed(()),
            )
            .unwrap();
        owner.submit_closed_materialization(ticket, closed).unwrap();
    }
    owner
        .finish(context, None, |_, _, _| Ok(true))
        .unwrap()
        .into_unique()
        .unwrap()
}

#[test]
fn a_closed_member_contains_canonical_sources_and_keeps_original_c_unchanged() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let named = graph
        .declare(
            vec![input()],
            EffectPredicate::unconstrained(),
            &mut context,
        )
        .unwrap();
    let named_row = graph
        .application(
            &named,
            vec![PreparedEffectRow::Canonical(EffectRow::open(
                EffectSet::new(),
                input(),
            ))],
            &mut context,
        )
        .unwrap();
    let future = graph
        .declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Future,
            &mut context,
        )
        .unwrap();
    let future_row = graph
        .unknown_application(&future, vec![], &mut context)
        .unwrap();
    let actual = function(
        graph
            .union(vec![named_row, future_row], &mut context)
            .unwrap(),
    );
    let expected = function(PreparedEffectRow::Canonical(EffectRow::open(
        EffectSet::from_labels(["named.body", "future.actual", "allowed.extra"]).unwrap(),
        input(),
    )));
    let mut component = Component::new(&mut context, &graph).unwrap();
    let member = root(
        &mut component,
        &mut context,
        &mut graph,
        EffectPredicate::unconstrained(),
        false,
    );
    let path = path_mut(&mut component, member);
    let actual = operand(&actual, path, &mut graph, &mut context);
    let expected = operand(&expected, path, &mut graph, &mut context);
    let before_actual = actual.clone();
    let before_pattern = expected.clone();
    let application = path.root_id();
    let ordinal = PreparedSourceOrdinal {
        application,
        ordinal: 0,
    };
    path.equations.push(PendingEquation {
        ordinal: ConstraintEquationId {
            application,
            ordinal: 0,
        },
        direction: ConstraintAcceptance::PatternAcceptsActual,
        pattern: expected.clone(),
        actual: actual.clone(),
        source_ordinal: Some(ordinal),
        timing: ConstraintEquationTiming::Immediate,
    });
    path.lineage.probe_trace.push(ActiveConstraintProbe {
        source: ConstraintSourceId::new(application, ()),
        source_ordinal: ordinal,
        branch: SourceProbeObservation::closed_for_test((), ()),
        selection: StoredSourceSelection::Checked {
            alternative: (),
            evidence: Arc::new(()),
        },
        prepared_source_projection: PreparedConstraintSourceProjection::Scalar,
        value_expected: Some(expected),
        actual,
    });
    let forest = component
        .infer_original_products(&mut context, &mut graph)
        .unwrap();
    graph
        .prepare_body(
            &named,
            PreparedEffectRow::Canonical(EffectRow::open(
                EffectSet::from_labels(["named.body"]).unwrap(),
                input(),
            )),
            vec![],
            &mut context,
        )
        .unwrap();
    let actual_future = EffectRow::closed(EffectSet::from_labels(["future.actual"]).unwrap());
    let closed = forest
        .close(&[(&future, &actual_future)], &mut graph, &mut context)
        .unwrap();
    let joint = closed
        .products
        .into_vec()
        .remove(0)
        .result
        .unwrap()
        .into_vec()
        .remove(0);
    assert_eq!(joint.completed.len(), 1);
    let completed = joint.completed.into_vec().remove(0);
    assert_eq!(completed.member, member);
    let solved = materialize(completed.materialization, &mut context);
    let sources = solved.component.sources().all();
    assert_eq!(sources.len(), 1);
    sources[0].require_completion(&joint.effects).unwrap();
    let TypeKind::Function { effects: row, .. } = sources[0].actual() else {
        panic!("canonical source")
    };
    assert_eq!(
        row,
        &EffectRow::open(
            EffectSet::from_labels(["named.body", "future.actual"]).unwrap(),
            input()
        )
    );
    assert!(sources[0].final_expected().is_some());
    assert_eq!(
        solved.component.selected().projections()[0]
            .to_quantified_type()
            .unwrap(),
        TypeKind::I64
    );
    let original = closed.original.original_path(member).unwrap();
    assert!(std::ptr::eq(
        original.lineage.probe_trace[0].actual.value(),
        before_actual.value()
    ));
    assert!(std::ptr::eq(
        original.equations[0].actual.value(),
        before_actual.value()
    ));
    assert!(std::ptr::eq(
        original.equations[0].pattern.value(),
        before_pattern.value()
    ));
}

#[test]
fn a_late_member_failure_returns_the_same_forest_without_partial_components() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let mut component = Component::new(&mut context, &graph).unwrap();
    let first = root(
        &mut component,
        &mut context,
        &mut graph,
        EffectPredicate::unconstrained(),
        false,
    );
    let second = root(
        &mut component,
        &mut context,
        &mut graph,
        EffectPredicate::unconstrained(),
        false,
    );
    let observation = SourceProbeObservation::new(());
    let path = path_mut(&mut component, second);
    let actual = operand(&TypeKind::Bool, path, &mut graph, &mut context);
    let application = path.root_id();
    path.lineage.probe_trace.push(ActiveConstraintProbe {
        source: ConstraintSourceId::new(application, ()),
        source_ordinal: PreparedSourceOrdinal {
            application,
            ordinal: 0,
        },
        branch: Arc::clone(&observation),
        selection: StoredSourceSelection::Unchecked,
        prepared_source_projection: PreparedConstraintSourceProjection::Scalar,
        value_expected: None,
        actual,
    });
    let forest = component
        .infer_original_products(&mut context, &mut graph)
        .unwrap();
    let members = Arc::clone(&forest.products[0].members);
    let retained = graph.retained_terms();
    let frames = context.lexical_frames().clone();
    let (error, forest) = forest
        .close(&[], &mut graph, &mut context)
        .err()
        .unwrap()
        .into_parts();
    assert!(matches!(
        error,
        PreparedValueFailure::Constraint(TypeConstraintError::Invariant(
            TypeConstraintInvariant::SourceProtocol(TypeConstraintSourceProtocolInvariant::Outcome)
        ))
    ));
    assert!(Arc::ptr_eq(&members, &forest.products[0].members));
    assert_eq!(forest.products[0].members.as_ref(), &[first, second]);
    assert_eq!(graph.retained_terms(), retained);
    assert_eq!(context.lexical_frames(), &frames);
    // Finish the original callback's one retained preparation, then retry the
    // same affine forest. The first member's staged component was not published.
    observation.retain_owned(()).unwrap();
    let closed = forest.close(&[], &mut graph, &mut context).unwrap();
    let completed = closed
        .products
        .into_vec()
        .remove(0)
        .result
        .unwrap()
        .into_vec()
        .remove(0)
        .completed
        .into_vec();
    assert_eq!(completed.len(), 2);
    assert_eq!(completed[0].member, first);
    assert_eq!(completed[1].member, second);
    let mut values = completed.into_iter();
    let _first = materialize(values.next().unwrap().materialization, &mut context);
    let second = materialize(values.next().unwrap().materialization, &mut context);
    assert_eq!(
        second.component.sources().all()[0].actual(),
        &TypeKind::Bool
    );
}
