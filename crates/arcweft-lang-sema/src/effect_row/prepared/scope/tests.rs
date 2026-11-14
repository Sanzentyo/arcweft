use super::*;
use crate::{
    effect_row::prepared::test_support::{Budget, Exhausted, budget, literal, open},
    effects::EffectSet,
    types::{GenericBinder, GenericScope},
};

struct Source {
    delta: PreparedEffectDelta,
    scheme: PreparedEffectSchemeRef,
    row: PreparedEffectRow,
    predicate: PreparedEffectPredicate,
    choice: PreparedEffectChoice,
    parameters: Vec<GenericEffectReference>,
    domain: EffectPredicate,
}

fn prepare_source(graph: &mut PreparedEffectGraph) -> Source {
    let mut work = budget();
    let binders = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let parameters = vec![
        binders.bound_effect(0, 0).unwrap(),
        binders.bound_effect(0, 1).unwrap(),
    ];
    let domain = EffectFormula::literal(EffectSet::new(), Some(parameters[0].clone()))
        .subset(
            &EffectFormula::literal(EffectSet::from_labels(["fs.read"]).unwrap(), None),
            &mut work,
        )
        .unwrap();
    let mut checkpoint = graph.checkpoint(&mut work).unwrap();
    let scheme = graph
        .declare(parameters.clone(), domain.clone(), &mut work)
        .unwrap();
    let row = graph
        .application(&scheme, parameters.iter().map(open).collect(), &mut work)
        .unwrap();
    let predicate = graph
        .relate(
            PreparedEffectRelation::Equal,
            row.clone(),
            open(&parameters[0]),
            &mut work,
        )
        .unwrap();
    let choice = graph.choice(&mut work).unwrap();
    graph
        .add_scoped_obligation(
            parameters.clone(),
            PreparedEffectPredicate::Canonical(domain.clone()),
            predicate.clone(),
            &mut work,
        )
        .unwrap();
    let delta = graph.extract_delta(&mut checkpoint, &mut work).unwrap();
    Source {
        delta,
        scheme,
        row,
        predicate,
        choice,
        parameters,
        domain,
    }
}

fn child<C: DecisionControl>(
    graph: &mut PreparedEffectGraph,
    scope: &PreparedEffectReadScope,
    source: &Source,
    control: &mut C,
) -> Result<(PreparedEffectDelta, PreparedEffectRow), PreparedEffectFailure<C::Error>> {
    let mut edit = graph.edit_scope(scope, control)?;
    let header = edit.read().declaration(&source.scheme, control)?;
    assert_eq!(header.parameters(), source.parameters);
    assert_eq!(header.domain(), &source.domain);
    edit.prepare_body(&source.scheme, open(&source.parameters[0]), vec![], control)?;
    let arguments = BTreeMap::from([
        (source.parameters[0].clone(), literal(&["fs.read"])),
        (source.parameters[1].clone(), literal(&["net.send"])),
    ]);
    let row = edit.substitute(&source.row, &arguments, control)?;
    let predicate = edit.substitute_predicate(&source.predicate, &arguments, control)?;
    let row = edit.select_row(&source.choice, row, literal(&[]), control)?;
    let equality = edit.relate(
        PreparedEffectRelation::Equal,
        row.clone(),
        literal(&["fs.read"]),
        control,
    )?;
    let predicate = edit.and(vec![predicate, equality], control)?;
    edit.add_scoped_obligation(
        vec![],
        PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        predicate,
        control,
    )?;
    Ok((edit.finish(control)?, row))
}

fn assert_base(graph: &PreparedEffectGraph, state: PreparedEffectStateId) {
    assert_eq!(graph.state, state);
    assert!(graph.functions.is_empty());
    assert_eq!(graph.retained_terms(), (0, 0));
    assert!(graph.obligations.is_empty());
    assert!(graph.choices.is_empty());
    assert!(graph.active_scope.is_none());
    assert!(graph.checkpoints.is_empty());
    assert!(graph.journal.is_empty());
}

#[test]
fn scoped_edit_reads_original_sources_and_publishes_only_with_every_affine_permit() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let source = prepare_source(&mut graph);
    let base = graph.state;
    let scope = graph.read_scope([&source.delta], &mut work).unwrap();
    let (delta, row) = child(&mut graph, &scope, &source, &mut work).unwrap();
    assert_base(&graph, base);
    let view = graph.read_extracted(&delta, &mut work).unwrap();
    view.validate_retained_row(&row).unwrap();
    view.validate_retained_row(&source.row).unwrap();
    view.validate_retained_predicate(&source.predicate).unwrap();
    let header = view.declaration(&source.scheme, &mut work).unwrap();
    assert_eq!(header.parameters(), source.parameters);
    assert_eq!(header.domain(), &source.domain);
    let (error, delta) = graph
        .restore_delta(delta, &mut work)
        .unwrap_err()
        .into_parts();
    assert!(matches!(
        error,
        PreparedEffectFailure::Invariant(PreparedEffectInvariant::ReadScopeDependencies)
    ));
    assert_base(&graph, base);
    // Holding a read lease is harmless; publication consumes the real permits.
    let mut outer = graph.checkpoint(&mut work).unwrap();
    graph
        .restore_deltas(vec![delta, source.delta], &mut work)
        .unwrap();
    header.validate(&graph).unwrap();
    let closed = graph.solve_principal(&mut work).unwrap();
    assert_eq!(
        closed.resolve_row(&row, &mut work).unwrap(),
        EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap())
    );
    graph.rollback(&mut outer).unwrap();
    assert_base(&graph, base);
    assert_eq!(
        header.validate(&graph),
        Err(PreparedEffectInvariant::StaleReference)
    );
    assert!(graph.edit_scope(&scope, &mut work).is_ok());
}

#[test]
fn dropping_scoped_and_nested_term_edits_rolls_back_only_their_own_writes() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let source = prepare_source(&mut graph);
    let base = graph.state;
    let scope = graph.read_scope([&source.delta], &mut work).unwrap();
    {
        let mut edit = graph.edit_scope(&scope, &mut work).unwrap();
        let temporary;
        {
            let mut terms = edit.edit_terms();
            temporary = terms
                .map_row_parameters(&source.row, &mut work, &mut |_, _| {
                    Ok(Some(literal(&["fs.read"])))
                })
                .unwrap();
        }
        assert_eq!(
            edit.read().validate_retained_row(&temporary),
            Err(PreparedEffectInvariant::StaleReference)
        );
        edit.read().validate_retained_row(&source.row).unwrap();
        edit.prepare_body(
            &source.scheme,
            open(&source.parameters[0]),
            vec![],
            &mut work,
        )
        .unwrap();
        edit.declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Bindable,
            &mut work,
        )
        .unwrap();
        assert!(matches!(
            edit.graph.solve_principal(&mut work),
            Err(PreparedEffectFailure::Invariant(
                PreparedEffectInvariant::ReadScopeActive
            ))
        ));
    }
    assert_base(&graph, base);
    let view = graph.read_extracted(&source.delta, &mut work).unwrap();
    assert!(matches!(
        view.scheme(&source.scheme).unwrap().state,
        SchemeState::Declared
    ));
}

#[test]
fn scope_construction_edit_and_batch_publication_are_failure_atomic_at_every_budget() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let source = prepare_source(&mut graph);
    let base = graph.state;
    let mut measured = budget();
    let available = measured.0;
    let scope = graph.read_scope([&source.delta], &mut measured).unwrap();
    for remaining in 0..available - measured.0 {
        assert!(matches!(
            graph.read_scope([&source.delta], &mut Budget(remaining)),
            Err(PreparedEffectFailure::Control(Exhausted))
        ));
        assert_base(&graph, base);
    }
    let mut measured = budget();
    let available = measured.0;
    let (delta, _) = child(&mut graph, &scope, &source, &mut measured).unwrap();
    for remaining in 0..available - measured.0 {
        assert!(matches!(
            child(&mut graph, &scope, &source, &mut Budget(remaining)),
            Err(PreparedEffectFailure::Control(Exhausted))
        ));
        assert_base(&graph, base);
    }
    let mut permits = vec![source.delta, delta];
    let mut attempts = 0;
    loop {
        match graph.restore_deltas(permits, &mut Budget(attempts)) {
            Ok(()) => break,
            Err(failure) => {
                let (error, original) = failure.into_parts();
                assert!(matches!(error, PreparedEffectFailure::Control(Exhausted)));
                assert_eq!(original.len(), 2);
                permits = original;
                assert_base(&graph, base);
            }
        }
        attempts += 1;
        assert!(
            attempts < 10_000,
            "publication must finish within the measured test budget"
        );
    }
    assert!(attempts > 0);
    assert!(graph.solve_principal(&mut work).is_ok());
}

#[test]
fn incompatible_sibling_body_definitions_and_changed_or_foreign_bases_are_rejected() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let source = prepare_source(&mut graph);
    let base = graph.state;
    let scope = graph.read_scope([&source.delta], &mut work).unwrap();
    let (left, _) = child(&mut graph, &scope, &source, &mut work).unwrap();
    let (right, _) = child(&mut graph, &scope, &source, &mut work).unwrap();
    let (error, permits) = graph
        .restore_deltas(vec![source.delta, left, right], &mut work)
        .unwrap_err()
        .into_parts();
    assert!(matches!(
        error,
        PreparedEffectFailure::Invariant(PreparedEffectInvariant::ReadScopeConflict)
    ));
    assert_eq!(permits.len(), 3);
    assert_base(&graph, base);
    let mut foreign = PreparedEffectGraph::default();
    assert!(matches!(
        foreign.edit_scope(&scope, &mut work),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ForeignIssuer
        ))
    ));
    let mut checkpoint = graph.checkpoint(&mut work).unwrap();
    graph.choice(&mut work).unwrap();
    assert!(matches!(
        graph.edit_scope(&scope, &mut work),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::DeltaBase
        ))
    ));
    graph.rollback(&mut checkpoint).unwrap();
    assert!(graph.edit_scope(&scope, &mut work).is_ok());
}

#[test]
fn replay_keeps_original_read_inputs_fixed_and_compares_the_actual_new_relation() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let source = prepare_source(&mut graph);
    let scope = graph.read_scope([&source.delta], &mut work).unwrap();
    let (left, left_row) = child(&mut graph, &scope, &source, &mut work).unwrap();
    let (right, right_row) = child(&mut graph, &scope, &source, &mut work).unwrap();
    assert_eq!(
        graph
            .replay_delta_mismatch(
                &left,
                &right,
                [PreparedEffectReplayRoot::Row(&left_row, &right_row)],
                &mut work
            )
            .unwrap(),
        None
    );
    let other = prepare_source(&mut graph);
    let other_scope = graph.read_scope([&other.delta], &mut work).unwrap();
    let (other_child, other_row) = child(&mut graph, &other_scope, &other, &mut work).unwrap();
    assert_eq!(
        graph
            .replay_delta_mismatch(
                &left,
                &other_child,
                [PreparedEffectReplayRoot::Row(&left_row, &other_row)],
                &mut work
            )
            .unwrap(),
        Some(PreparedEffectReplayMismatch::ReadScopeOrigin)
    );
}

#[derive(Debug, thiserror::Error)]
#[error("original scoped operation cancellation")]
struct Cancelled;

struct Cancellation<'a>(&'a std::cell::Cell<bool>);
impl DecisionControl for Cancellation<'_> {
    type Error = Cancelled;
    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        if self.0.get() { Err(Cancelled) } else { Ok(()) }
    }
}

#[test]
fn cancellation_preserves_its_original_cause_and_all_source_publication_permits() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let source = prepare_source(&mut graph);
    let base = graph.state;
    let cancelled = std::cell::Cell::new(true);
    let mut control = Cancellation(&cancelled);
    assert!(matches!(
        graph.read_scope([&source.delta], &mut control),
        Err(PreparedEffectFailure::Control(Cancelled))
    ));
    cancelled.set(false);
    let scope = graph.read_scope([&source.delta], &mut control).unwrap();
    let mut edit = graph.edit_scope(&scope, &mut control).unwrap();
    edit.prepare_body(
        &source.scheme,
        open(&source.parameters[0]),
        vec![],
        &mut control,
    )
    .unwrap();
    cancelled.set(true);
    assert!(matches!(
        edit.finish(&mut control),
        Err(PreparedEffectFailure::Control(Cancelled))
    ));
    assert_base(&graph, base);
    cancelled.set(false);
    let (delta, row) = child(&mut graph, &scope, &source, &mut control).unwrap();
    cancelled.set(true);
    let (error, permits) = graph
        .restore_deltas(vec![source.delta, delta], &mut control)
        .unwrap_err()
        .into_parts();
    assert!(matches!(error, PreparedEffectFailure::Control(Cancelled)));
    assert_base(&graph, base);
    graph.restore_deltas(permits, &mut work).unwrap();
    assert_eq!(
        graph
            .solve_principal(&mut work)
            .unwrap()
            .resolve_row(&row, &mut work)
            .unwrap(),
        EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap())
    );
}

#[test]
fn transitive_read_layers_require_every_original_permit_and_drop_without_an_owned_chain() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let source = prepare_source(&mut graph);
    let base = graph.state;
    let mut scope = graph.read_scope([&source.delta], &mut work).unwrap();
    let mut permits = Vec::new();
    let mut row = source.row.clone();
    for _ in 0..256 {
        let mut edit = graph.edit_scope(&scope, &mut work).unwrap();
        row = edit
            .select_row(&source.choice, row, literal(&[]), &mut work)
            .unwrap();
        let delta = edit.finish(&mut work).unwrap();
        scope = graph.read_scope([&delta], &mut work).unwrap();
        permits.push(delta);
        assert_base(&graph, base);
    }
    let (delta, _) = child(&mut graph, &scope, &source, &mut work).unwrap();
    let (error, delta) = graph
        .restore_delta(delta, &mut work)
        .unwrap_err()
        .into_parts();
    assert!(matches!(
        error,
        PreparedEffectFailure::Invariant(PreparedEffectInvariant::ReadScopeDependencies)
    ));
    assert_base(&graph, base);
    permits.push(source.delta);
    permits.push(delta);
    graph.restore_deltas(permits, &mut work).unwrap();
    assert!(graph.solve_principal(&mut work).is_ok());
    drop(scope);
}
