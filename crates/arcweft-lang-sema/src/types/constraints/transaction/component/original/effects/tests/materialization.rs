use super::*;
use crate::types::constraints::transaction::materialization::MaterializationInput;
use crate::types::constraints::transaction::{
    ActiveConstraintProbe, ConstraintMaterialization, ConstraintSourceId,
    PreparedConstraintSourceProjection, PreparedSourceOrdinal, SourceProbeObservation,
    StoredSourceSelection,
};
use crate::types::constraints::{
    ClosedMaterializationSubmission, SolvedCandidate, TypeConstraintFailure,
    TypeConstraintFailureInvariant, TypeConstraintSourceProtocolInvariant,
};

fn original_pair(
    context: &mut Context<'_>,
    graph: &mut PreparedEffectGraph,
) -> (
    ClosedOriginalConstraintForest<NoConstraintClient, ()>,
    [OriginalProductMember; 2],
) {
    let mut component = Component::new(context, graph).unwrap();
    let members = [
        root(
            &mut component,
            context,
            graph,
            EffectPredicate::unconstrained(),
            false,
        ),
        root(
            &mut component,
            context,
            graph,
            EffectPredicate::unconstrained(),
            false,
        ),
    ];
    for member in members {
        let value = component.values.get_mut(&member.producer).unwrap();
        let ValueDefinition::Constraint { frontiers } = &mut value.definition else {
            panic!("original root")
        };
        let path = &mut frontiers[member.frontier].transaction.frontier[member.path];
        path.lineage.probe_trace.push(ActiveConstraintProbe {
            source: ConstraintSourceId::new(path.root_id(), ()),
            source_ordinal: PreparedSourceOrdinal {
                application: path.root_id(),
                ordinal: 0,
            },
            branch: SourceProbeObservation::closed_for_test((), ()),
            selection: StoredSourceSelection::Unchecked,
            prepared_source_projection: PreparedConstraintSourceProjection::Scalar,
            value_expected: None,
            actual: context.fixture_operand(TypeKind::<PreparedEffects>::Bool),
        });
    }
    let forest = component
        .infer_original_products(context, graph)
        .unwrap()
        .close(&[], graph, context)
        .unwrap();
    (forest, members)
}

fn finish(
    owner: ConstraintMaterialization<NoConstraintClient>,
    context: &mut Context<'_>,
) -> SolvedCandidate<NoConstraintClient> {
    owner
        .finish(context, |_, _, _| Ok(true))
        .unwrap()
        .into_unique()
        .unwrap()
}

#[test]
fn standalone_dispatch_releases_the_original_path_before_the_selected_preparation_moves() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let mut component = Component::new(&mut context, &graph).unwrap();
    let member = root(
        &mut component,
        &mut context,
        &mut graph,
        EffectPredicate::unconstrained(),
        false,
    );
    let value = component.values.remove(&member.producer).unwrap();
    let ValueDefinition::Constraint { frontiers } = value.definition else {
        panic!("original root")
    };
    let mut frontier = frontiers.into_vec().remove(0);
    let path = &mut frontier.transaction.frontier[member.path];
    let source = ConstraintSourceId::new(path.root_id(), ());
    path.lineage.probe_trace.push(ActiveConstraintProbe {
        source,
        source_ordinal: PreparedSourceOrdinal {
            application: path.root_id(),
            ordinal: 0,
        },
        branch: SourceProbeObservation::closed_for_test((), ()),
        selection: StoredSourceSelection::Unchecked,
        prepared_source_projection: PreparedConstraintSourceProjection::Scalar,
        value_expected: None,
        actual: context.fixture_operand(TypeKind::<PreparedEffects>::Bool),
    });
    let mut owner = frontier
        .into_candidate_materialization(&[], &mut graph, &mut context)
        .unwrap();
    let mut ticket = owner
        .next_materialization_ticket(&mut context)
        .unwrap()
        .unwrap();
    assert_eq!(*ticket.requests().next().unwrap().source(), source);
    ticket.bind_callback().unwrap();
    let closed = ticket
        .bind_closed_submission(ClosedMaterializationSubmission::Sealed(()))
        .unwrap();
    owner.submit_closed_materialization(ticket, closed).unwrap();
    let solved = finish(owner, &mut context);
    assert_eq!(
        solved.component.sources().all()[0].actual(),
        &TypeKind::Bool
    );
    let Ok(preparations) = solved.source_preparations.into_owned(&mut context) else {
        panic!("the selected source preparation is still shared")
    };
    assert_eq!(preparations.len(), 1);
    assert_eq!(preparations[0].source, source);
}

#[test]
fn canonical_member_callbacks_keep_their_exact_ticket_issuer_without_a_graph_loan() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let (forest, members) = original_pair(&mut context, &mut graph);
    let mut completed = forest
        .products
        .into_vec()
        .remove(0)
        .result
        .unwrap()
        .into_vec()
        .remove(0)
        .completed
        .into_vec()
        .into_iter();
    let first = completed.next().unwrap();
    let second = completed.next().unwrap();
    assert_eq!(first.member, members[0]);
    assert_eq!(second.member, members[1]);
    let mut first = first.fixture_materialization(&graph, &mut context).unwrap();
    let mut second = second
        .fixture_materialization(&graph, &mut context)
        .unwrap();
    let retained = graph.retained_terms();
    first.validate_graph(graph.read()).unwrap();
    second.validate_graph(graph.read()).unwrap();
    let foreign = PreparedEffectGraph::default();
    assert!(matches!(
        first.validate_graph(foreign.read()),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::PreparedEffect(PreparedEffectFailure::Invariant(
                crate::effect_row::prepared::PreparedEffectInvariant::ForeignIssuer
            ))
        ))
    ));
    let mut a = first
        .next_materialization_ticket(&mut context)
        .unwrap()
        .unwrap();
    let mut b = second
        .next_materialization_ticket(&mut context)
        .unwrap()
        .unwrap();
    assert_eq!(
        first.validate_materialization_callback_begin(&b),
        Err(TypeConstraintSourceProtocolInvariant::Ticket)
    );
    assert_eq!(
        second.validate_materialization_callback_begin(&a),
        Err(TypeConstraintSourceProtocolInvariant::Ticket)
    );
    first.validate_materialization_callback_begin(&a).unwrap();
    second.validate_materialization_callback_begin(&b).unwrap();
    let binding_a = a.bind_callback().unwrap();
    let binding_b = b.bind_callback().unwrap();
    assert_eq!(
        a.validate_callback_binding(&binding_b),
        Err(TypeConstraintSourceProtocolInvariant::Ticket)
    );
    assert_eq!(
        b.validate_callback_binding(&binding_a),
        Err(TypeConstraintSourceProtocolInvariant::Ticket)
    );
    a.validate_callback_binding(&binding_a).unwrap();
    b.validate_callback_binding(&binding_b).unwrap();
    let closed_a = a
        .bind_closed_submission(ClosedMaterializationSubmission::Sealed(()))
        .unwrap();
    let closed_b = b
        .bind_closed_submission(ClosedMaterializationSubmission::Sealed(()))
        .unwrap();
    first.submit_closed_materialization(a, closed_a).unwrap();
    second.submit_closed_materialization(b, closed_b).unwrap();
    assert_eq!(
        finish(first, &mut context).component.sources().all()[0].actual(),
        &TypeKind::Bool
    );
    assert_eq!(
        finish(second, &mut context).component.sources().all()[0].actual(),
        &TypeKind::Bool
    );
    assert_eq!(graph.retained_terms(), retained);
}

#[test]
fn an_ignored_invalid_submission_cannot_publish_an_earlier_sealed_candidate() {
    use std::cell::Cell;
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let (forest, members) = original_pair(&mut context, &mut graph);
    let joint = &forest.products[0].result.as_ref().unwrap()[0];
    let mut owner =
        ConstraintMaterialization::new(forest.original.effect_owner.clone(), &mut context).unwrap();
    for member in members {
        let source = forest.original.original_path(member).unwrap();
        let input = MaterializationInput::from_closed_original(
            source,
            source.root_id(),
            &joint.branch,
            &joint.effects,
            graph.read(),
            &mut context,
        )
        .unwrap();
        owner.push_component(input, &mut context).unwrap();
    }
    let retained = graph.retained_terms();
    let mut a = owner
        .next_materialization_ticket(&mut context)
        .unwrap()
        .unwrap();
    let foreign_source = *a.requests().next().unwrap().source();
    a.bind_callback().unwrap();
    let closed = a
        .bind_closed_submission(ClosedMaterializationSubmission::Sealed(()))
        .unwrap();
    owner.submit_closed_materialization(a, closed).unwrap();
    assert!(!owner.records_are_empty());
    let mut b = owner
        .next_materialization_ticket(&mut context)
        .unwrap()
        .unwrap();
    assert_ne!(*b.requests().next().unwrap().source(), foreign_source);
    b.bind_callback().unwrap();
    let closed = b
        .bind_closed_submission(ClosedMaterializationSubmission::Rejected {
            source: foreign_source,
            cause: (),
        })
        .unwrap();
    assert!(owner.submit_closed_materialization(b, closed).is_err());
    assert!(
        owner
            .next_materialization_ticket(&mut context)
            .unwrap()
            .is_none()
    );
    let comparisons = Cell::new(0);
    let result = owner.finish(&mut context, |_, _, _| {
        comparisons.set(comparisons.get() + 1);
        Ok(true)
    });
    assert!(matches!(
        result,
        Err(TypeConstraintFailure::Invariant(
            TypeConstraintFailureInvariant::Constraint(TypeConstraintInvariant::SourceProtocol(
                TypeConstraintSourceProtocolInvariant::Ticket
            ))
        ))
    ));
    assert_eq!(comparisons.get(), 0);
    assert_eq!(graph.retained_terms(), retained);
}
