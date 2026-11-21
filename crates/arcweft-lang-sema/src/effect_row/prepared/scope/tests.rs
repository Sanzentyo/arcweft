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
    assert!(graph.relation.functions.is_empty());
    assert_eq!(graph.retained_terms(), (0, 0));
    assert!(graph.relation.obligations.is_empty());
    assert!(graph.relation.choices.is_empty());
    assert!(graph.active_scope.is_none());
    assert!(graph.checkpoints.is_empty());
    assert!(graph.journal.is_empty());
}

#[test]
fn nested_checkpoints_inherit_selected_reads_and_restore_their_parent_on_close() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let base = graph.state;
    let parent = prepare_source(&mut graph);
    let parent_reads = graph.read_scope([&parent.delta], &mut work).unwrap();
    let child = prepare_source(&mut graph);
    let child_reads = graph.read_scope([&child.delta], &mut work).unwrap();

    let mut parent_checkpoint = graph
        .checkpoint_in_read_scope(&parent_reads, &mut work)
        .unwrap();
    assert_eq!(
        graph.commit(&parent_checkpoint),
        Err(PreparedEffectInvariant::ReadScopeActive)
    );
    assert!(matches!(
        graph.checkpoint_in_read_scope(&child_reads, &mut work),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ReadScopeConflict
        ))
    ));
    graph
        .prepare_body(
            &parent.scheme,
            open(&parent.parameters[0]),
            vec![],
            &mut work,
        )
        .unwrap();
    let parent_state = graph.state;
    let mut ordinary = graph.checkpoint(&mut work).unwrap();
    graph.choice(&mut work).unwrap();
    let ordinary = graph.extract_delta(&mut ordinary, &mut work).unwrap();
    graph.read_extracted(&ordinary, &mut work).unwrap();
    assert_eq!(graph.state, parent_state);

    let reads = graph
        .compose_read_scopes([&child_reads], &mut work)
        .unwrap();
    let mut child_checkpoint = graph.checkpoint_in_read_scope(&reads, &mut work).unwrap();
    assert!(matches!(
        graph.rollback(&mut parent_checkpoint),
        Err(PreparedEffectInvariant::CheckpointOrder)
    ));
    graph.read().validate_retained_row(&parent.row).unwrap();
    graph.read().validate_retained_row(&child.row).unwrap();
    graph
        .prepare_body(&child.scheme, open(&child.parameters[0]), vec![], &mut work)
        .unwrap();
    assert!(matches!(
        graph.extract_delta(&mut child_checkpoint, &mut Budget(0)),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    graph.read().validate_retained_row(&child.row).unwrap();
    let detached = graph
        .extract_delta(&mut child_checkpoint, &mut work)
        .unwrap();
    assert_eq!(graph.state, parent_state);
    assert_eq!(
        graph.read().validate_retained_row(&child.row),
        Err(PreparedEffectInvariant::StaleReference)
    );
    assert!(matches!(
        graph.read().scheme(&parent.scheme).unwrap().state,
        SchemeState::BodyPrepared { .. }
    ));
    assert!(matches!(
        graph
            .read_extracted(&detached, &mut work)
            .unwrap()
            .scheme(&child.scheme)
            .unwrap()
            .state,
        SchemeState::BodyPrepared { .. }
    ));
    graph.rollback(&mut parent_checkpoint).unwrap();
    assert_base(&graph, base);
}

#[test]
fn empty_projections_publish_without_issuing_an_effect_transition() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let base = graph.state;
    let mut first = graph.checkpoint(&mut work).unwrap();
    let first = graph.extract_delta(&mut first, &mut work).unwrap();
    let mut second = graph.checkpoint(&mut work).unwrap();
    let second = graph.extract_delta(&mut second, &mut work).unwrap();
    let scope = graph.read_scope([&first, &second], &mut work).unwrap();
    graph
        .restore_deltas(vec![first, second], &mut work)
        .unwrap();
    assert_base(&graph, base);
    assert!(graph.state_ancestors.is_empty());

    let mut selected = graph.checkpoint_in_read_scope(&scope, &mut work).unwrap();
    let row = graph
        .union(vec![literal(&["log.write"])], &mut work)
        .unwrap();
    let selected = graph.extract_delta(&mut selected, &mut work).unwrap();
    assert_base(&graph, base);
    graph.restore_delta(selected, &mut work).unwrap();
    assert_eq!(
        graph
            .solve_principal(&mut work)
            .unwrap()
            .resolve_row(&row, &mut work)
            .unwrap(),
        EffectRow::closed(EffectSet::from_labels(["log.write"]).unwrap())
    );
}

#[test]
fn restoring_a_nested_projection_journals_only_authorized_writes() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let base = graph.state;
    let parent = prepare_source(&mut graph);
    let reads = graph.read_scope([&parent.delta], &mut work).unwrap();
    let sibling = prepare_source(&mut graph);
    let sibling_reads = graph.read_scope([&sibling.delta], &mut work).unwrap();
    let mut callback = graph.checkpoint_in_read_scope(&reads, &mut work).unwrap();

    let extra_reads = graph
        .compose_read_scopes([&sibling_reads], &mut work)
        .unwrap();
    let mut nested = graph
        .checkpoint_in_read_scope(&extra_reads, &mut work)
        .unwrap();
    let unused = graph.extract_delta(&mut nested, &mut work).unwrap();
    let (error, _) = graph
        .restore_delta(unused, &mut work)
        .unwrap_err()
        .into_parts();
    assert!(matches!(
        error,
        PreparedEffectFailure::Invariant(PreparedEffectInvariant::ReadScopeDependencies)
    ));
    assert_eq!(graph.state, base);
    assert!(graph.journal.is_empty());

    let mut candidate = graph.checkpoint(&mut work).unwrap();
    graph
        .prepare_body(
            &parent.scheme,
            open(&parent.parameters[0]),
            vec![],
            &mut work,
        )
        .unwrap();
    graph
        .add_scoped_obligation(
            vec![],
            PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            &mut work,
        )
        .unwrap();
    let selected = graph.extract_delta(&mut candidate, &mut work).unwrap();
    graph.restore_delta(selected, &mut work).unwrap();
    assert!(graph.relation.functions.is_empty());
    assert!(graph.relation.rows.is_empty());
    assert_eq!(graph.relation.obligations.len(), 1);
    assert!(matches!(
        graph.read().scheme(&parent.scheme).unwrap().state,
        SchemeState::BodyPrepared { .. }
    ));

    let callback = graph.extract_delta(&mut callback, &mut work).unwrap();
    assert_base(&graph, base);
    let (error, callback) = graph
        .restore_delta(callback, &mut work)
        .unwrap_err()
        .into_parts();
    assert!(matches!(
        error,
        PreparedEffectFailure::Invariant(PreparedEffectInvariant::ReadScopeDependencies)
    ));
    graph
        .restore_deltas(vec![parent.delta, callback], &mut work)
        .unwrap();
    assert_eq!(graph.relation.obligations.len(), 2);
    graph.read().validate_retained_row(&parent.row).unwrap();
    assert_eq!(
        graph.read().validate_retained_row(&sibling.row),
        Err(PreparedEffectInvariant::StaleReference)
    );
}

#[test]
fn selected_sources_at_different_live_bases_compose_without_reading_siblings() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let first = prepare_source(&mut graph);
    let first_reads = graph.read_scope([&first.delta], &mut work).unwrap();
    graph.choice(&mut work).unwrap();
    let second = prepare_source(&mut graph);
    let second_reads = graph.read_scope([&second.delta], &mut work).unwrap();
    let sibling = prepare_source(&mut graph);
    let base = graph.state;

    let reads = graph
        .compose_read_scopes([&second_reads, &first_reads, &first_reads], &mut work)
        .unwrap();
    let view = graph.read_scoped(&reads, &mut work).unwrap();
    view.validate_query_roots(
        &[&first.row, &second.row],
        &[&first.predicate, &second.predicate],
        &mut work,
    )
    .unwrap();
    assert_eq!(
        view.validate_retained_row(&sibling.row),
        Err(PreparedEffectInvariant::StaleReference)
    );
    assert_eq!(reads.layer_count(), 2);
    let recomposed = graph
        .compose_read_scopes([&first_reads, &reads, &second_reads], &mut work)
        .unwrap();
    assert!(recomposed.same_selection(&reads));
    let contained = graph
        .compose_read_scopes([&reads, &first_reads, &second_reads], &mut Budget(7))
        .unwrap();
    assert!(contained.same_selection(&reads));

    let mut abandoned = graph.checkpoint(&mut work).unwrap();
    graph.changed(&mut work).unwrap();
    let abandoned_reads = graph.read_scope([], &mut work).unwrap();
    graph.rollback(&mut abandoned).unwrap();
    assert!(matches!(
        graph.compose_read_scopes([&reads, &abandoned_reads], &mut work),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::DeltaBase
        ))
    ));
    let (derived, derived_reads) = graph
        .compute_in_read_scope(&reads, &mut work, |graph, work| {
            graph.union(vec![first.row.clone(), second.row.clone()], work)
        })
        .unwrap();
    graph
        .read_scoped(&derived_reads, &mut work)
        .unwrap()
        .validate_query_roots(&[&derived], &[], &mut work)
        .unwrap();
    assert_eq!(
        graph.read().validate_retained_row(&derived),
        Err(PreparedEffectInvariant::StaleReference)
    );
    assert_eq!(graph.state, base);
    assert!(matches!(
        graph
            .restore_delta(first.delta, &mut work)
            .unwrap_err()
            .into_parts()
            .0,
        PreparedEffectFailure::Invariant(PreparedEffectInvariant::DeltaBase)
    ));
    assert_eq!(
        graph.read().validate_retained_row(&second.row),
        Err(PreparedEffectInvariant::StaleReference)
    );
}

#[test]
fn a_nested_base_requires_its_actual_enclosing_layer() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let base = graph.state;
    let mut parent = graph.checkpoint(&mut work).unwrap();
    let scheme = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let mut child = graph.checkpoint(&mut work).unwrap();
    graph
        .prepare_body(&scheme, literal(&["child.body"]), vec![], &mut work)
        .unwrap();
    let row = graph.application(&scheme, vec![], &mut work).unwrap();
    let child_delta = graph.extract_delta(&mut child, &mut work).unwrap();
    let child_reads = graph.read_scope([&child_delta], &mut work).unwrap();
    // Keep the child's base in the parent's ancestry behind unrelated
    // revisions. Composition only needs the base that the child references.
    for _ in 0..128 {
        graph.changed(&mut work).unwrap();
    }
    let parent_delta = graph.extract_delta(&mut parent, &mut work).unwrap();
    let parent_reads = graph.read_scope([&parent_delta], &mut work).unwrap();

    assert!(matches!(
        graph.compose_read_scopes([&child_reads], &mut work),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::DeltaBase
        ))
    ));
    let joined = graph
        .compose_read_scopes([&child_reads, &parent_reads], &mut Budget(64))
        .unwrap();
    let view = graph.read_scoped(&joined, &mut work).unwrap();
    view.validate_query_roots(&[&row], &[], &mut work).unwrap();
    assert!(matches!(
        view.scheme(&scheme).unwrap().state,
        SchemeState::BodyPrepared { .. }
    ));
    assert_base(&graph, base);
}

#[test]
fn an_ancestral_read_scope_cannot_override_a_conflicting_live_body() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let scheme = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let mut checkpoint = graph.checkpoint(&mut work).unwrap();
    graph
        .prepare_body(&scheme, literal(&["detached.body"]), vec![], &mut work)
        .unwrap();
    let delta = graph.extract_delta(&mut checkpoint, &mut work).unwrap();
    let reads = graph.read_scope([&delta], &mut work).unwrap();
    graph
        .prepare_body(&scheme, literal(&["live.body"]), vec![], &mut work)
        .unwrap();
    let base = graph.state;
    assert!(matches!(
        graph.compose_read_scopes([&reads], &mut work),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ReadScopeConflict
        ))
    ));
    assert_eq!(graph.state, base);
    assert_eq!(
        graph
            .solve_principal(&mut work)
            .unwrap()
            .scheme(&scheme)
            .unwrap(),
        &EffectRow::closed(EffectSet::from_labels(["live.body"]).unwrap())
    );
}

#[test]
fn a_closed_read_scope_keeps_source_terms_after_the_permit_is_dropped() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let source = prepare_source(&mut graph);
    let base = graph.state;
    let scope = graph.read_scope([&source.delta], &mut work).unwrap();
    drop(source.delta);

    let read = graph.read_scoped(&scope, &mut work).unwrap();
    read.validate_query_roots(&[&source.row], &[&source.predicate], &mut work)
        .unwrap();
    assert_eq!(
        graph.read().validate_retained_row(&source.row),
        Err(PreparedEffectInvariant::StaleReference)
    );
    assert_base(&graph, base);

    let foreign = PreparedEffectGraph::default();
    assert!(matches!(
        foreign.read_scoped(&scope, &mut work),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ForeignIssuer
        ))
    ));
    let mut checkpoint = graph.checkpoint(&mut work).unwrap();
    graph.choice(&mut work).unwrap();
    assert!(matches!(
        graph.read_scoped(&scope, &mut work),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::DeltaBase
        ))
    ));
    graph.rollback(&mut checkpoint).unwrap();
    assert!(graph.read_scoped(&scope, &mut work).is_ok());
    assert_base(&graph, base);
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
fn unchanged_computation_reuses_reads_with_only_a_closing_visit() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let source = prepare_source(&mut graph);
    let input = graph.read_scope([&source.delta], &mut work).unwrap();
    let next_delta = graph.next_delta;
    let read = |_: &mut PreparedEffectGraph,
                _: &mut Budget|
     -> Result<_, PreparedEffectFailure<Exhausted>> { Ok(source.row.clone()) };

    let base = graph.state;
    graph
        .validate_in_read_scope(&input, &mut work, |graph, _| {
            assert!(matches!(
                graph.compute_in_read_scope(&input, &mut Budget(3), read),
                Err(PreparedEffectFailure::Control(Exhausted))
            ));
            let (row, reads) = graph.compute_in_read_scope(&input, &mut Budget(4), read)?;
            assert!(reads.same_selection(&input));
            graph.read().validate_retained_row(&row)?;
            Ok::<_, PreparedEffectFailure<Exhausted>>(())
        })
        .unwrap();

    assert_eq!(graph.next_delta, next_delta);
    assert_base(&graph, base);
}

#[test]
fn live_reads_keep_the_exact_issuer_and_base_without_a_detached_layer() {
    let mut graph = PreparedEffectGraph::default();
    let foreign = PreparedEffectGraph::default();
    let mut work = budget();
    let mut checkpoint = graph.checkpoint(&mut work).unwrap();
    let empty = graph.extract_delta(&mut checkpoint, &mut work).unwrap();
    assert!(matches!(
        graph.read_scope([&empty], &mut Budget(1)),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    let reads = graph.read_scope([&empty], &mut Budget(2)).unwrap();
    assert!(reads.detached().is_none());
    assert!(matches!(
        foreign.read_scoped(&reads, &mut Budget(1)),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ForeignIssuer
        ))
    ));

    let mut edit = graph.edit_scope(&reads, &mut work).unwrap();
    let choice = edit.choice(&mut work).unwrap();
    let delta = edit.finish(&mut work).unwrap();
    assert!(delta.publishes_without_read_scope());
    assert_eq!(
        graph.read().validate_choice(&choice),
        Err(PreparedEffectInvariant::StaleReference)
    );

    let mut restore = graph.checkpoint(&mut work).unwrap();
    graph.restore_delta(delta, &mut work).unwrap();
    graph.read().validate_choice(&choice).unwrap();
    assert!(matches!(
        graph.read_scoped(&reads, &mut Budget(1)),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::DeltaBase
        ))
    ));
    graph.rollback(&mut restore).unwrap();
    graph.read_scoped(&reads, &mut Budget(1)).unwrap();
}

#[test]
fn live_only_scope_publication_does_not_journal_a_definition_already_in_view() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let scheme = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let id = graph.validate(&scheme.0).unwrap();
    let reads = graph.read_scope([], &mut Budget(1)).unwrap();
    let edit = graph.edit_scope(&reads, &mut work).unwrap();
    let mut redundant = edit.graph.checkpoint(&mut work).unwrap();
    let definition = Arc::clone(edit.read().function_definition(id).unwrap());
    edit.graph.publish_function(id, Arc::clone(&definition));
    let delta = edit.graph.extract_delta(&mut redundant, &mut work).unwrap();
    assert!(delta.has_writes());
    let frontier = edit.graph.journal.len();
    edit.graph.restore_delta(delta, &mut work).unwrap();
    assert_eq!(edit.graph.journal.len(), frontier);
    assert!(Arc::ptr_eq(
        edit.read().function_definition(id).unwrap(),
        &definition
    ));
}

#[test]
fn derived_terms_share_unmodified_indexes_and_keep_the_exact_layer() {
    let mut graph = PreparedEffectGraph::default();
    let base = graph.state;
    let mut work = budget();
    let source = prepare_source(&mut graph);
    let input = graph.read_scope([&source.delta], &mut work).unwrap();
    let edit = graph.edit_scope(&input, &mut work).unwrap();
    let empty = edit.finish(&mut work).unwrap();
    let same = graph.read_scope([&empty], &mut work).unwrap();
    assert!(same.same_selection(&input));

    let mut edit = graph.edit_scope(&input, &mut work).unwrap();
    let predicate = edit
        .relate(
            PreparedEffectRelation::Equal,
            source.row.clone(),
            literal(&["fs.read"]),
            &mut work,
        )
        .unwrap();
    let added = edit.finish(&mut work).unwrap();

    let mut rebuilding = budget();
    let rebuilt = graph
        .delta_scope_data([&added], &mut rebuilding, |delta, control| {
            graph.validate_delta_base(delta, control)
        })
        .map(|data| graph.bind_read_scope(data))
        .unwrap();
    let mut extending = budget();
    let available = extending.0;
    let extended = graph.read_scope([&added], &mut extending).unwrap();
    assert!(extending.0 > rebuilding.0);
    assert!(extended.same_origin(&rebuilt, &mut work).unwrap());
    let extended_data = extended.detached().unwrap();
    let input_data = input.detached().unwrap();
    assert_eq!(extended_data.ids, rebuilt.detached().unwrap().ids);
    assert!(Arc::ptr_eq(&extended_data.functions, &input_data.functions));
    assert!(Arc::ptr_eq(&extended_data.rows, &input_data.rows));
    assert!(Arc::ptr_eq(&extended_data.choices, &input_data.choices));
    let view = graph.read_scoped(&extended, &mut work).unwrap();
    view.validate_query_roots(&[&source.row], &[&source.predicate, &predicate], &mut work)
        .unwrap();
    assert_eq!(
        graph
            .read_scoped(&input, &mut work)
            .unwrap()
            .validate_retained_predicate(&predicate),
        Err(PreparedEffectInvariant::StaleReference)
    );
    for remaining in 0..available - extending.0 {
        assert!(matches!(
            graph.read_scope([&added], &mut Budget(remaining)),
            Err(PreparedEffectFailure::Control(Exhausted))
        ));
        assert_base(&graph, base);
    }
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
        graph.read_scope([], &mut control),
        Err(PreparedEffectFailure::Control(Cancelled))
    ));
    assert!(matches!(
        graph.compose_read_scopes([], &mut control),
        Err(PreparedEffectFailure::Control(Cancelled))
    ));
    assert!(matches!(
        graph.read_scope([&source.delta], &mut control),
        Err(PreparedEffectFailure::Control(Cancelled))
    ));
    cancelled.set(false);
    let scope = graph.read_scope([&source.delta], &mut control).unwrap();
    assert!(matches!(
        graph.compute_in_read_scope(&scope, &mut control, |graph, _| {
            graph.read().validate_retained_row(&source.row)?;
            cancelled.set(true);
            Ok::<_, PreparedEffectFailure<Cancelled>>(())
        }),
        Err(PreparedEffectFailure::Control(Cancelled))
    ));
    assert_base(&graph, base);
    cancelled.set(false);
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
