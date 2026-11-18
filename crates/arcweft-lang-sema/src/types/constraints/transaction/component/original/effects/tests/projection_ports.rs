use super::*;
use crate::{
    effect_row::prepared::{
        PreparedEffectInvariant, PreparedEffectPredicate, PreparedEffectUnknownEligibility,
    },
    types::{
        GenericBinder,
        constraints::{ConstraintClosurePolicy, normalization::project_closed_operand},
    },
};

#[test]
fn a_result_use_prepares_and_relates_using_its_own_detached_effect_terms() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let declared = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut context)
        .unwrap();
    let body = EffectRow::closed(EffectSet::from_labels(["result.use"]).unwrap());
    graph
        .prepare_body(
            &declared,
            PreparedEffectRow::Canonical(body.clone()),
            vec![],
            &mut context,
        )
        .unwrap();
    let mut checkpoint = graph.checkpoint(&mut context).unwrap();
    let row = graph.application(&declared, vec![], &mut context).unwrap();
    let delta = graph.extract_delta(&mut checkpoint, &mut context).unwrap();
    let reads = graph.read_scope([&delta], &mut context).unwrap();
    drop(delta);
    assert!(graph.read().validate_retained_row(&row).is_err());

    let original = context
        .original_producer_context(&Arc::new(Source {
            identity: Arc::new(()),
            inputs: [input()],
            domain: EffectPredicate::unconstrained(),
        }))
        .unwrap();
    let owner = context
        .application_scope(
            (),
            TypeConstraintParameterScope::empty(),
            original.clone(),
            &mut graph,
        )
        .unwrap();
    let mut path = context
        .start_path_with_imported(owner, None, original, graph.read())
        .unwrap();
    path.lineage.effect_reads = Some(reads);
    let application = path.root_id();
    let mut transaction =
        TypeConstraintTransaction::from_path(&mut context, &mut graph, application, path, None)
            .unwrap();
    let arrow = |row| {
        TypeKind::function_with_contract(GenericBinder::EMPTY, truth(), [], TypeKind::I64, row)
    };
    let live_terms = graph.retained_terms();
    transaction.request_projection(
        &mut context,
        &mut graph,
        (),
        &arrow(row.clone()),
        TypeConstraintProjectionClosure::Closed,
    );
    let mut prepared = false;
    transaction
        .constrain_result_use(
            &mut context,
            &mut graph,
            (),
            &arrow(PreparedEffectRow::Canonical(body)),
            |source, reads, _| {
                let TypeKind::Function { effects, .. } = source.source_type() else {
                    panic!("the selected result is a function");
                };
                reads.validate_retained_row(&row).unwrap();
                reads.validate_retained_row(effects).unwrap();
                prepared = true;
                Ok(None)
            },
        )
        .unwrap();
    assert!(prepared);
    assert!(transaction.first_failure.is_none());
    let selected = transaction.test_single_path();
    assert_eq!(selected.equations.len(), 1);
    let reads = selected.lineage.effect_reads.as_ref().unwrap();
    graph
        .read_scoped(reads, &mut context)
        .unwrap()
        .validate_retained_row(&row)
        .unwrap();
    assert!(graph.read().validate_retained_row(&row).is_err());
    assert_eq!(graph.retained_terms(), live_terms);
}

#[test]
fn the_selected_projection_closes_over_detached_reads_and_returns_a_canonical_result() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let named = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut context)
        .unwrap();
    let mut checkpoint = graph.checkpoint(&mut context).unwrap();
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
    let call_row = graph.application(&named, vec![], &mut context).unwrap();
    let row = graph
        .union(vec![future_row, call_row], &mut context)
        .unwrap();
    let delta = graph.extract_delta(&mut checkpoint, &mut context).unwrap();
    let reads = graph.read_scope([&delta], &mut context).unwrap();
    drop(delta);
    let template = TypeKind::function_with_contract(
        GenericBinder::EMPTY,
        truth(),
        [],
        TypeKind::I64,
        row.clone(),
    );

    let source = Arc::new(Source {
        identity: Arc::new(()),
        inputs: [input()],
        domain: EffectPredicate::unconstrained(),
    });
    let original = context.original_producer_context(&source).unwrap();
    let owner = context
        .application_scope(
            (),
            TypeConstraintParameterScope::empty(),
            original.clone(),
            &mut graph,
        )
        .unwrap();
    let mut path = context
        .start_path_with_imported(owner, None, original, graph.read())
        .unwrap();
    path.lineage.effect_reads = Some(reads);
    let application = path.root_id();
    let mut transaction =
        TypeConstraintTransaction::from_path(&mut context, &mut graph, application, path, None)
            .unwrap();
    transaction.request_projection_with_input(
        &mut context,
        &mut graph,
        (),
        &template,
        TypeConstraintProjectionClosure::Closed,
        Some(&template),
    );
    let frontier = transaction.suspend_result(&mut context, ()).unwrap();
    let mut component = Component::new(&mut context, &graph).unwrap();
    let producer = component
        .admit_fixture_roots(&mut context, &mut graph, vec![frontier], ())
        .unwrap();
    let live_terms = graph.retained_terms();
    let forest = component
        .infer_original_products(&mut context, &mut graph)
        .unwrap();
    assert_eq!(graph.retained_terms(), live_terms);

    let body = EffectRow::closed(EffectSet::from_labels(["named.body"]).unwrap());
    graph
        .prepare_body(
            &named,
            PreparedEffectRow::Canonical(body),
            vec![],
            &mut context,
        )
        .unwrap();
    let actual = EffectRow::closed(EffectSet::from_labels(["future.actual"]).unwrap());
    let solved = forest
        .close(&[(&future, &actual)], &mut graph, &mut context)
        .unwrap()
        .into_candidate_materialization(&producer, &mut context)
        .unwrap()
        .finish(&mut context, |_, _, _| Ok(true))
        .unwrap()
        .into_unique()
        .unwrap();
    let [projection] = solved.component.selected().projections() else {
        panic!("expected the selected result projection");
    };
    let TypeKind::Function { effects, .. } = projection.to_quantified_type().unwrap() else {
        panic!("expected a canonical function type");
    };
    assert_eq!(
        effects,
        EffectRow::closed(EffectSet::from_labels(["named.body", "future.actual"]).unwrap())
    );
    assert!(projection.has_exact_input_match());
    assert_eq!(
        graph.read().validate_unknown(&future),
        Err(PreparedEffectInvariant::StaleReference)
    );
    assert_eq!(
        graph.read().validate_retained_row(&row),
        Err(PreparedEffectInvariant::StaleReference)
    );
    assert_eq!(graph.retained_terms(), live_terms);
}

#[test]
fn a_registered_projection_keeps_named_and_future_queries_before_final_closure() {
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
    let application = graph
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
    let unrelated_future = graph
        .declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Future,
            &mut context,
        )
        .unwrap();
    let row = graph
        .union(vec![application, future_row], &mut context)
        .unwrap();
    let template = TypeKind::function_with_contract(
        GenericBinder::EMPTY,
        PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        [],
        TypeKind::I64,
        row,
    );
    let mut component = Component::new(&mut context, &graph).unwrap();
    let member = root_with_projection(
        &mut component,
        &mut context,
        &mut graph,
        EffectPredicate::unconstrained(),
        false,
        &template,
        None,
    );
    let source = component.original_path(member).unwrap();
    let request = Arc::clone(&source.lineage.projections[0]);
    let retained = graph.retained_terms();
    for _ in 0..2 {
        let (value, resolved) =
            crate::types::constraints::transaction::ResolvedConstraintResult::resolve(
                crate::types::constraints::transaction::ConstraintResultProjection {
                    application: source.root_id(),
                    key: Arc::clone(&request.key),
                },
                source,
                &mut graph,
                &mut context,
            )
            .unwrap();
        assert_eq!(&value, request.value.value());
        let projection = resolved.projection();
        assert_eq!(projection.application, source.root_id());
        assert!(Arc::ptr_eq(&projection.key, &request.key));
        assert_eq!(graph.retained_terms(), retained);
    }
    // The original forest is prepared while the actual Named body is absent.
    // The final reader uses that same query after the body becomes available.
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
    let failed = match forest.close(&[], &mut graph, &mut context) {
        Ok(_) => panic!("the selected product still requires its Future input"),
        Err(failure) => failure,
    };
    let (error, forest) = failed.into_parts();
    assert!(matches!(
        error,
        PreparedValueFailure::Constraint(TypeConstraintError::Invariant(
            TypeConstraintInvariant::PreparedEffect(PreparedEffectFailure::Invariant(
                crate::effect_row::prepared::PreparedEffectInvariant::UnresolvedFuture
            ))
        ))
    ));
    let closed = forest
        .close(&[(&future, &actual_future)], &mut graph, &mut context)
        .unwrap();
    graph.read().validate_unknown(&unrelated_future).unwrap();
    let joint = &closed.products[0].result.as_ref().unwrap()[0];
    let retained = graph.retained_terms();
    let value = project_closed_operand(
        request.value.view(),
        &joint.branch,
        ConstraintClosurePolicy::ProjectionClosed,
        &joint.effects,
        graph.read(),
        &mut context,
    )
    .unwrap();
    let canonical = joint
        .effects
        .canonicalize(value.value(), value.scope().shape(), &mut context)
        .unwrap();
    let TypeKind::Function { effects: row, .. } = canonical else {
        panic!("canonical function")
    };
    let expected = EffectRow::open(
        EffectSet::from_labels(["named.body", "future.actual"]).unwrap(),
        input(),
    );
    assert_eq!(row, expected);
    assert_eq!(graph.retained_terms(), retained);
}

#[test]
fn a_late_projection_opening_failure_publishes_no_request_or_graph_terms() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let source = Arc::new(Source {
        identity: Arc::new(()),
        inputs: [input()],
        domain: EffectPredicate::unconstrained(),
    });
    let original = context.original_producer_context(&source).unwrap();
    let owner = context
        .application_scope(
            (),
            TypeConstraintParameterScope::empty(),
            original.clone(),
            &mut graph,
        )
        .unwrap();
    let path = context
        .start_path_with_imported(owner, None, original, graph.read())
        .unwrap();
    let application = path.root_id();
    let mut transaction =
        TypeConstraintTransaction::from_path(&mut context, &mut graph, application, path, None)
            .unwrap();
    let named = graph
        .declare(
            vec![input()],
            EffectPredicate::unconstrained(),
            &mut context,
        )
        .unwrap();
    let row = graph
        .application(
            &named,
            vec![PreparedEffectRow::Canonical(EffectRow::open(
                EffectSet::new(),
                input(),
            ))],
            &mut context,
        )
        .unwrap();
    let function = TypeKind::function_with_contract(
        GenericBinder::EMPTY,
        PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        [],
        TypeKind::I64,
        row,
    );
    let foreign = crate::types::GenericScope::default()
        .with_binder(GenericBinder::new(1, 0, 0))
        .bound_type(0, 0)
        .unwrap();
    let template = TypeKind::Tuple(vec![function.clone(), TypeKind::GenericParam(foreign)]);
    let retained = graph.retained_terms();
    transaction.request_projection_with_input(
        &mut context,
        &mut graph,
        (),
        &template,
        TypeConstraintProjectionClosure::Closed,
        Some(&function),
    );
    assert!(transaction.first_failure.is_some());
    assert_eq!(transaction.frontier.len(), 1);
    assert!(transaction.frontier[0].lineage.projections.is_empty());
    assert_eq!(graph.retained_terms(), retained);
}
