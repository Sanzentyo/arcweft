use super::*;
use crate::{
    effect_row::prepared::{PreparedEffectPredicate, PreparedEffectUnknownEligibility},
    types::{
        GenericBinder,
        constraints::{ConstraintClosurePolicy, normalization::project_closed_operand},
    },
};

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
