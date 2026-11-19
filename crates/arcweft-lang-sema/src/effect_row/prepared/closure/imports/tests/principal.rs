use super::*;

fn require(graph: &mut PreparedEffectGraph, predicate: PreparedEffectPredicate) {
    graph
        .add_scoped_obligation(vec![], truth(), predicate, &mut budget())
        .unwrap();
}

#[test]
fn principal_import_does_not_require_or_invent_an_output_function() {
    for admitted in [false, true] {
        let mut source = PreparedEffectGraph::default();
        let predicate = if admitted {
            truth()
        } else {
            source.not(truth(), &mut budget()).unwrap()
        };
        require(&mut source, predicate);
        let source = source.complete_scoped(&mut budget()).unwrap();
        let mut parent = PreparedEffectGraph::default();
        let relation = parent
            .import_scoped_relation(&source, &[], &mut budget())
            .unwrap();
        let principal = parent.imported_principal(&relation, &mut budget()).unwrap();
        assert!(parent.relation.functions.is_empty());
        require(&mut parent, principal);
        match parent.solve_principal(&mut budget()) {
            Ok(closed) => {
                assert!(admitted);
                assert!(closed.functions.is_empty());
            }
            Err(PreparedEffectFailure::Unsatisfiable) => assert!(!admitted),
            Err(error) => panic!("unexpected import result: {error:?}"),
        }
    }
}

#[test]
fn a_satisfiable_child_without_a_joint_least_tuple_has_no_principal_region() {
    let mut source = PreparedEffectGraph::default();
    let x = unknown(&mut source, false, vec![]);
    let y = unknown(&mut source, false, vec![]);
    let x_value = call(&mut source, &x);
    let y_value = call(&mut source, &y);
    let mut choices = Vec::new();
    for first in [false, true] {
        let left = source
            .relate(
                PreparedEffectRelation::Equal,
                x_value.clone(),
                literal(if first { &["log.write"] } else { &[] }),
                &mut budget(),
            )
            .unwrap();
        let right = source
            .relate(
                PreparedEffectRelation::Equal,
                y_value.clone(),
                literal(if first { &[] } else { &["log.write"] }),
                &mut budget(),
            )
            .unwrap();
        choices.push(source.and(vec![left, right], &mut budget()).unwrap());
    }
    let predicate = source.or(choices, &mut budget()).unwrap();
    require(&mut source, predicate);
    let source = source.complete_scoped(&mut budget()).unwrap();
    assert!(matches!(
        source.graph.specialize_principal(&[], &mut budget()),
        Err(PreparedEffectFailure::NonPrincipal)
    ));
    let mut parent = PreparedEffectGraph::default();
    let relation = parent
        .import_scoped_relation(&source, &[], &mut budget())
        .unwrap();
    let principal = parent.imported_principal(&relation, &mut budget()).unwrap();
    require(&mut parent, principal);
    assert!(matches!(
        parent.solve_principal(&mut budget()),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
}

#[test]
fn a_late_future_binding_extends_support_before_testing_the_principal_condition() {
    let mut source = PreparedEffectGraph::default();
    let x = unknown(&mut source, false, vec![]);
    let future = unknown(&mut source, true, vec![]);
    let x_value = call(&mut source, &x);
    let future_value = call(&mut source, &future);
    let lower = source
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            future_value,
            x_value.clone(),
            &mut budget(),
        )
        .unwrap();
    let upper = source
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            x_value,
            literal(&["fs.read"]),
            &mut budget(),
        )
        .unwrap();
    let predicate = source.and(vec![lower, upper], &mut budget()).unwrap();
    require(&mut source, predicate);
    let source = source.complete_scoped(&mut budget()).unwrap();
    for allowed in [false, true] {
        let mut parent = PreparedEffectGraph::default();
        let actual = unknown(&mut parent, true, vec![]);
        let relation = parent
            .import_scoped_relation(&source, &[(&future, &actual)], &mut budget())
            .unwrap();
        let principal = parent.imported_principal(&relation, &mut budget()).unwrap();
        require(&mut parent, principal);
        let row = EffectRow::closed(
            EffectSet::from_labels(if allowed {
                ["fs.read"]
            } else {
                ["outside.label"]
            })
            .unwrap(),
        );
        match parent.specialize_principal(&[(&actual, &row)], &mut budget()) {
            Ok(closed) => {
                assert!(allowed);
                assert_eq!(closed.unknown(&actual).unwrap(), &row);
            }
            Err(PreparedEffectFailure::Unsatisfiable) => assert!(!allowed),
            Err(error) => panic!("unexpected late-input result: {error:?}"),
        }
    }
}

#[test]
fn output_and_principal_share_one_attachment_and_one_lowered_table() {
    let (source, x, future) = child_equality();
    let mut parent = PreparedEffectGraph::default();
    let actual = unknown(&mut parent, false, vec![]);
    let relation = parent
        .import_scoped_relation(&source, &[(&future, &actual)], &mut budget())
        .unwrap();
    let output = parent
        .import_relation_function(&relation, &source.unknown(&x).unwrap(), &mut budget())
        .unwrap();
    let principal = parent.imported_principal(&relation, &mut budget()).unwrap();
    require(&mut parent, principal);
    let actual_value = call(&mut parent, &actual);
    equation(&mut parent, actual_value, literal(&["fs.read"]), vec![]);
    let output_value = parent
        .imported_application(&output, vec![], &mut budget())
        .unwrap();
    let closed = parent.solve_principal(&mut budget()).unwrap();
    assert_eq!(
        closed.resolve_row(&output_value, &mut budget()).unwrap(),
        EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap())
    );
    let functions = &closed.layout.imports.functions[&id(&output)];
    let scalar = closed
        .layout
        .imports
        .relation(&relation.relation, &mut Work(&mut budget()))
        .unwrap();
    assert!(std::ptr::eq(functions.relation.as_ref(), scalar));
    assert_eq!(closed.layout.imports.relations.len(), 1);
}

#[test]
fn original_cell_coordinates_share_the_guard_and_local_coefficients() {
    let mut source = PreparedEffectGraph::default();
    let future = unknown(&mut source, true, vec![]);
    let local = unknown(&mut source, false, vec![]);
    let actual_row = call(&mut source, &future);
    let local_row = call(&mut source, &local);
    equation(&mut source, local_row, actual_row.clone(), vec![]);
    let permitted = literal(&["fs.read"]);
    let bound = source
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            actual_row,
            permitted.clone(),
            &mut budget(),
        )
        .unwrap();
    require(&mut source, bound);
    let source = source.complete_scoped(&mut budget()).unwrap();

    for displaced in [false, true] {
        let mut parent = PreparedEffectGraph::default();
        if displaced {
            unknown(&mut parent, false, vec![]);
        }
        let actual = unknown(&mut parent, false, vec![]);
        let relation = parent
            .import_scoped_relation(&source, &[(&future, &actual)], &mut budget())
            .unwrap();
        let principal = parent.imported_principal(&relation, &mut budget()).unwrap();
        require(&mut parent, principal);
        let output = parent
            .import_relation_function(&relation, &source.unknown(&local).unwrap(), &mut budget())
            .unwrap();
        let output_row = parent
            .imported_application(&output, vec![], &mut budget())
            .unwrap();
        let actual_row = call(&mut parent, &actual);
        equation(&mut parent, actual_row, permitted.clone(), vec![]);
        let closed = parent.solve_principal(&mut budget()).unwrap();
        assert_eq!(
            closed.resolve_row(&output_row, &mut budget()).unwrap(),
            EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap()),
        );
        let table = &closed.layout.imports.functions[&id(&output)];
        assert!(table.relation.principal().node_count() > 0);
        assert_eq!(
            std::ptr::eq(table.relation.principal(), &table.relation.child.principal),
            !displaced,
        );
        let classes = table.relation.child.layout.view().classes();
        let scheme = FunctionRef::Unknown(local.clone()).reference().ordinal;
        assert!(table.coefficient(1, 0).unwrap().node_count() > 0);
        for class in 0..classes {
            let original = table
                .relation
                .child
                .coefficients
                .get(&Atom::Cell {
                    scheme,
                    class,
                    mask: 0,
                })
                .unwrap();
            assert_eq!(
                std::ptr::eq(table.coefficient(class, 0).unwrap(), original),
                !displaced
            );
        }
        assert_eq!(
            table.coefficient(classes, 0),
            Err(PreparedEffectInvariant::UnclosedCell)
        );
    }
}

#[test]
fn foreign_stale_and_differently_fixed_attachments_cannot_issue_results() {
    let (source, x, future) = child_equality();
    let mut parent = PreparedEffectGraph::default();
    let mut checkpoint = parent.checkpoint(&mut budget()).unwrap();
    let actual = unknown(&mut parent, false, vec![]);
    let relation = parent
        .import_scoped_relation(&source, &[(&future, &actual)], &mut budget())
        .unwrap();
    let fixed = source
        .specialize_partial(
            &[(
                &future,
                &EffectRow::closed(EffectSet::from_labels(["log.write"]).unwrap()),
            )],
            &mut budget(),
        )
        .unwrap();
    assert!(matches!(
        parent.import_relation_function(&relation, &fixed.unknown(&x).unwrap(), &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ImportedRelationOrigin
        ))
    ));
    let mut other = PreparedEffectGraph::default();
    assert!(matches!(
        other.imported_principal(&relation, &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ForeignIssuer
        ))
    ));
    parent.rollback(&mut checkpoint).unwrap();
    let terms = parent.retained_terms();
    assert!(matches!(
        parent.imported_principal(&relation, &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::StaleReference
        ))
    ));
    assert!(matches!(
        parent.import_relation_function(&relation, &source.unknown(&x).unwrap(), &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::StaleReference
        ))
    ));
    assert_eq!(parent.retained_terms(), terms);
}

#[test]
fn every_exhausted_attachment_or_predicate_creation_preserves_the_parent() {
    let (source, _, future) = child_equality();
    let mut parent = PreparedEffectGraph::default();
    let actual = unknown(&mut parent, false, vec![]);
    let terms = parent.retained_terms();
    let functions = parent.relation.functions.len();
    let state = parent.state;
    let mut measured = Budget(4096);
    let relation = parent
        .import_scoped_relation(&source, &[(&future, &actual)], &mut measured)
        .unwrap();
    let required = 4096 - measured.0;
    for available in 0..required {
        let mut work = Budget(available);
        assert!(matches!(
            parent.import_scoped_relation(&source, &[(&future, &actual)], &mut work),
            Err(PreparedEffectFailure::Control(Exhausted))
        ));
        assert_eq!(work.0, 0);
        assert_eq!(parent.retained_terms(), terms);
        assert_eq!(parent.relation.functions.len(), functions);
        assert_eq!(parent.state, state);
    }
    let mut checkpoint = parent.checkpoint(&mut budget()).unwrap();
    let mut measured = Budget(4096);
    parent.imported_principal(&relation, &mut measured).unwrap();
    let required = 4096 - measured.0;
    parent.rollback(&mut checkpoint).unwrap();
    for available in 0..required {
        let mut work = Budget(available);
        assert!(matches!(
            parent.imported_principal(&relation, &mut work),
            Err(PreparedEffectFailure::Control(Exhausted))
        ));
        assert_eq!(work.0, 0);
        assert_eq!(parent.retained_terms(), terms);
        assert_eq!(parent.relation.functions.len(), functions);
        assert_eq!(parent.state, state);
    }
}

#[test]
fn an_imported_global_condition_does_not_expose_function_dependencies_as_row_ports() {
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let parameter = scope.bound_effect(0, 0).unwrap();
    let mut source = PreparedEffectGraph::default();
    let x = unknown(&mut source, false, vec![parameter.clone()]);
    let future = unknown(&mut source, true, vec![parameter.clone()]);
    let argument =
        PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), parameter.clone()));
    let left = source
        .unknown_application(&x, vec![argument.clone()], &mut budget())
        .unwrap();
    let right = source
        .unknown_application(&future, vec![argument], &mut budget())
        .unwrap();
    equation(&mut source, left, right, vec![parameter]);
    let source = source.complete_scoped(&mut budget()).unwrap();
    let mut parent = PreparedEffectGraph::default();
    let actual_scope = scope.with_binder(GenericBinder::new(0, 0, 1));
    let actual = unknown(
        &mut parent,
        false,
        vec![actual_scope.bound_effect(1, 0).unwrap()],
    );
    let relation = parent
        .import_scoped_relation(&source, &[(&future, &actual)], &mut budget())
        .unwrap();
    let principal = parent.imported_principal(&relation, &mut budget()).unwrap();
    parent
        .read()
        .visit_predicate_parameters(&principal, &mut budget(), &mut |_, _| {
            panic!("whole function inputs are not ambient row occurrences")
        })
        .unwrap();
    let mut edit = parent.edit_terms();
    let mapped = edit
        .map_predicate_parameters(&principal, &mut budget(), &mut |_, _| {
            panic!("closed original quantifiers must not be projected again")
        })
        .unwrap();
    edit.commit().unwrap();
    assert_eq!(mapped, principal);
}

#[test]
fn named_inputs_keep_the_original_issuer_and_wait_for_the_actual_body() {
    for allowed in [false, true] {
        let mut graph = PreparedEffectGraph::default();
        let named = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut budget())
            .unwrap();
        let value = graph.application(&named, vec![], &mut budget()).unwrap();
        let expected = graph
            .relate(
                PreparedEffectRelation::Equal,
                value.clone(),
                literal(&["fs.read"]),
                &mut budget(),
            )
            .unwrap();
        let original = graph
            .read()
            .scoped_constraint(&[], &truth(), &expected, &mut budget())
            .unwrap();
        let source = graph
            .read()
            .freeze_scoped(&[&value], &[], &[&original], &mut budget())
            .unwrap();
        let mut foreign = PreparedEffectGraph::default();
        assert!(matches!(
            foreign.import_scoped_relation(&source, &[], &mut budget()),
            Err(PreparedEffectFailure::Invariant(
                PreparedEffectInvariant::ForeignIssuer
            ))
        ));
        let imported = graph
            .import_scoped_relation(&source, &[], &mut budget())
            .unwrap();
        let principal = graph.imported_principal(&imported, &mut budget()).unwrap();
        require(&mut graph, principal);
        graph
            .prepare_body(
                &named,
                literal(if allowed {
                    &["fs.read"]
                } else {
                    &["outside.label"]
                }),
                vec![],
                &mut budget(),
            )
            .unwrap();
        match graph.solve_principal(&mut budget()) {
            Ok(_) => assert!(allowed),
            Err(PreparedEffectFailure::Unsatisfiable) => assert!(!allowed),
            Err(error) => panic!("unexpected named-input result: {error:?}"),
        }
    }
}
