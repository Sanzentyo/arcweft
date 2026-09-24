use super::*;

#[test]
fn specialization_preserves_a_nested_known_scheme_and_its_predicate() {
    let outer = GenericBinder::new(1, 0, 0);
    let inner = GenericBinder::new(1, 0, 1);
    let outer_scope = GenericScope::default().with_binder(outer);
    let nested_scope = outer_scope.with_binder(inner);
    let nested = |scope: &GenericScope, result| {
        let effect = scope.bound_effect(0, 0).unwrap();
        TypeKind::function_with_contract(
            inner,
            EffectFormula::literal(EffectSet::new(), Some(effect.clone()))
                .subset(
                    &EffectFormula::literal(EffectSet::from_labels(["fs.read"]).unwrap(), None),
                    &mut Decisions,
                )
                .unwrap(),
            [TypeKind::GenericParam(scope.bound_type(0, 0).unwrap())],
            result,
            EffectRow::open(EffectSet::new(), effect),
        )
    };
    let source = TypeKind::function_with_contract(
        outer,
        EffectPredicate::unconstrained(),
        [TypeKind::GenericParam(
            outer_scope.bound_type(0, 0).unwrap(),
        )],
        nested(
            &nested_scope,
            TypeKind::GenericParam(nested_scope.bound_type(1, 0).unwrap()),
        ),
        EffectRow::closed(EffectSet::new()),
    );
    let expected = TypeKind::function_with_effects(
        [TypeKind::I64],
        nested(&GenericScope::default().with_binder(inner), TypeKind::I64),
        EffectRow::closed(EffectSet::new()),
    );
    let original = source.semantic_identity_digest().unwrap();
    let sources = [Source::Scheme(source.clone())];
    let graph = PreparedCallGraph::<()>::new();
    let observations = Arc::new(Mutex::new(Vec::new()));
    let cancellation = AtomicBool::new(false);
    let solved = drive(
        &graph,
        Client {
            graph: &graph,
            application_owners: Vec::new(),
            sources: &sources,
            observations: Arc::clone(&observations),
            cancellation: &cancellation,
            cancel_during_probe: false,
            check_foreign_graph: false,
        },
        TypeConstraintParameterScope::empty(),
        &[expected.clone()],
        PRODUCTION_CALLABLE_LIMITS,
    )
    .unwrap();
    assert_eq!(solved.component.applications().len(), 2);
    assert_eq!(observations.lock().unwrap()[0].1, expected);
    assert_eq!(source.semantic_identity_digest().unwrap(), original);
}
