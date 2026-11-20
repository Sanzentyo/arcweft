use super::*;

fn truth() -> PreparedEffectPredicate {
    PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained())
}

#[test]
fn agreement_waits_for_the_actual_body_and_keeps_the_existing_relation() {
    for conflict in [false, true] {
        let mut control = Control(1_000_000);
        let mut graph = PreparedEffectGraph::default();
        let mut environment = environment(graph.read(), &mut control);
        let declaration = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut control)
            .unwrap();
        let application = graph
            .application(&declaration, vec![], &mut control)
            .unwrap();
        let required = graph
            .relate(
                PreparedEffectRelation::Equal,
                application,
                row(&["fs.read"]),
                &mut control,
            )
            .unwrap();
        environment
            .constrain_predicate_equality(&mut graph, &required, &truth(), &mut control)
            .unwrap();
        let previous = environment.original_predicate().clone();
        let previous_terms = graph.retained_terms();
        let previous_state = graph.state;
        environment
            .constrain_predicate_equality(&mut graph, &truth(), &truth(), &mut control)
            .unwrap();
        assert_eq!(environment.original_predicate(), &previous);
        assert_eq!(graph.retained_terms(), previous_terms);
        assert_eq!(graph.state, previous_state);
        assert!(matches!(
            environment.constrain_predicate_equality(
                &mut PreparedEffectGraph::default(),
                &truth(),
                &truth(),
                &mut control,
            ),
            Err(PreparedEffectFailure::Invariant(
                PreparedEffectInvariant::ForeignIssuer
            ))
        ));
        graph.read().validate_retained_predicate(&previous).unwrap();
        assert!(matches!(
            graph.scheme(&declaration).unwrap().state,
            SchemeState::Declared
        ));
        graph
            .prepare_body(
                &declaration,
                row(if conflict {
                    &["log.write"]
                } else {
                    &["fs.read"]
                }),
                vec![],
                &mut control,
            )
            .unwrap();
        graph
            .add_scoped_obligation(
                vec![],
                truth(),
                environment.original_predicate().clone(),
                &mut control,
            )
            .unwrap();
        match graph.solve_principal(&mut control) {
            Ok(_) => assert!(!conflict),
            Err(PreparedEffectFailure::Unsatisfiable) => assert!(conflict),
            Err(error) => panic!("unexpected completion: {error:?}"),
        }
    }
}

#[test]
fn agreement_compares_whole_facts_including_different_nonempty_rows() {
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let first = scope.bound_effect(0, 0).unwrap();
    let second = scope.bound_effect(0, 1).unwrap();
    let values = [
        vec![],
        vec!["fs.read"],
        vec!["log.write"],
        vec!["outside.label"],
    ];
    for left in &values {
        for right in &values {
            let mut control = Control(1_000_000);
            let mut graph = PreparedEffectGraph::default();
            let mut environment = environment(graph.read(), &mut control);
            let mut facts = Vec::new();
            for variable in [&first, &second] {
                facts.push(
                    graph
                        .relate(
                            PreparedEffectRelation::Equal,
                            PreparedEffectRow::Canonical(EffectRow::open(
                                EffectSet::new(),
                                variable.clone(),
                            )),
                            row(&[]),
                            &mut control,
                        )
                        .unwrap(),
                );
            }
            environment
                .constrain_predicate_equality(&mut graph, &facts[0], &facts[1], &mut control)
                .unwrap();
            let assigned = graph
                .substitute_predicate(
                    environment.original_predicate(),
                    &BTreeMap::from([(first.clone(), row(left)), (second.clone(), row(right))]),
                    &mut control,
                )
                .unwrap();
            graph
                .add_scoped_obligation(vec![], truth(), assigned, &mut control)
                .unwrap();
            match graph.solve_principal(&mut control) {
                Ok(_) => assert_eq!(left.is_empty(), right.is_empty()),
                Err(PreparedEffectFailure::Unsatisfiable) => {
                    assert_ne!(left.is_empty(), right.is_empty())
                }
                Err(error) => panic!("unexpected completion: {error:?}"),
            }
        }
    }
}

#[test]
fn agreement_quantifies_the_two_operands_at_the_same_valuation() {
    let mut control = Control(1_000_000);
    let mut graph = PreparedEffectGraph::default();
    let mut environment = environment(graph.read(), &mut control);
    let parameter = GenericScope::default()
        .with_binder(GenericBinder::new(0, 0, 1))
        .bound_effect(0, 0)
        .unwrap();
    let input = PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), parameter.clone()));
    let [left, right] = ["fs.read", "log.write"].map(|label| {
        graph
            .relate(
                PreparedEffectRelation::AdmittedSubset,
                input.clone(),
                row(&[label]),
                &mut control,
            )
            .unwrap()
    });
    environment
        .constrain_predicate_equality(&mut graph, &left, &right, &mut control)
        .unwrap();
    graph
        .add_scoped_obligation(
            vec![parameter],
            truth(),
            environment.original_predicate().clone(),
            &mut control,
        )
        .unwrap();
    // Neither operand holds for every finite row. They disagree when the
    // input contains just one of the two permitted effects.
    assert!(matches!(
        graph.solve_principal(&mut control),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
}

#[test]
fn every_failed_agreement_edit_rolls_back_without_refunding_work() {
    let mut control = Control(1_000_000);
    let mut graph = PreparedEffectGraph::default();
    let seed = environment(graph.read(), &mut control);
    let variable = seed.original_variables().next().unwrap().clone();
    let input = PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), variable));
    let [left, right] = ["fs.read", "log.write"].map(|label| {
        graph
            .relate(
                PreparedEffectRelation::Equal,
                input.clone(),
                row(&[label]),
                &mut control,
            )
            .unwrap()
    });
    let mut checkpoint = graph.checkpoint(&mut control).unwrap();
    let mut probe = seed.clone();
    let mut measured = Control(4096);
    probe
        .constrain_predicate_equality(&mut graph, &left, &right, &mut measured)
        .unwrap();
    let required = 4096 - measured.0;
    graph.rollback(&mut checkpoint).unwrap();
    let state = graph.state;
    let terms = graph.retained_terms();
    for available in 0..=required {
        let mut checkpoint = graph.checkpoint(&mut control).unwrap();
        let mut branch = seed.clone();
        let mut work = Control(available);
        let result = branch.constrain_predicate_equality(&mut graph, &left, &right, &mut work);
        if available < required {
            assert!(matches!(
                result,
                Err(PreparedEffectFailure::Control(Failure::Exhausted))
            ));
            assert_eq!(branch, seed);
            assert_eq!(graph.state, state);
            assert_eq!(graph.retained_terms(), terms);
            assert_eq!(work.0, 0);
        } else {
            result.unwrap();
            assert_ne!(branch, seed);
        }
        graph.rollback(&mut checkpoint).unwrap();
    }
}

#[test]
fn foreign_stale_and_unadmitted_inputs_cannot_publish_agreement() {
    let mut control = Control(1_000_000);
    let mut graph = PreparedEffectGraph::default();
    let mut foreign_graph = PreparedEffectGraph::default();
    let foreign = foreign_graph
        .relate(
            PreparedEffectRelation::Equal,
            row(&[]),
            row(&[]),
            &mut control,
        )
        .unwrap();
    let mut checkpoint = graph.checkpoint(&mut control).unwrap();
    let stale = graph
        .relate(
            PreparedEffectRelation::Equal,
            row(&[]),
            row(&[]),
            &mut control,
        )
        .unwrap();
    graph.rollback(&mut checkpoint).unwrap();
    let variable: GenericEffectReference = GenericEffectParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(91_818)),
        0,
    )
    .into();
    let unauthorized = graph
        .relate(
            PreparedEffectRelation::Equal,
            PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), variable.clone())),
            row(&[]),
            &mut control,
        )
        .unwrap();
    for (input, invariant) in [
        (&foreign, Some(PreparedEffectInvariant::ForeignIssuer)),
        (&stale, Some(PreparedEffectInvariant::StaleReference)),
        (&unauthorized, None),
    ] {
        let mut environment = environment(graph.read(), &mut control);
        let before = environment.clone();
        let terms = graph.retained_terms();
        let state = graph.state;
        let result =
            environment.constrain_predicate_equality(&mut graph, input, &truth(), &mut control);
        if let Some(expected) = invariant {
            assert!(
                matches!(result, Err(PreparedEffectFailure::Invariant(actual)) if actual == expected)
            );
        } else {
            assert!(
                matches!(result, Err(PreparedEffectFailure::Control(Failure::Scope(
                EffectConstraintEnvironmentError::ForeignVariable { variable: actual },
            ))) if actual == variable)
            );
        }
        assert_eq!(environment, before);
        assert_eq!(graph.retained_terms(), terms);
        assert_eq!(graph.state, state);
    }
}
