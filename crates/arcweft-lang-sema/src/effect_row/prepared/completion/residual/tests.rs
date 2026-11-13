use super::*;
use crate::effect_row::prepared::test_support::*;
use crate::{
    effects::EffectSet,
    types::{GenericBinder, GenericScope},
};

fn truth() -> PreparedEffectPredicate {
    PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained())
}
fn unknown(
    graph: &mut PreparedEffectGraph,
    future: bool,
    parameters: Vec<GenericEffectReference>,
) -> PreparedEffectUnknownRef {
    graph
        .declare_unknown(
            parameters,
            if future {
                PreparedEffectUnknownEligibility::Future
            } else {
                PreparedEffectUnknownEligibility::Bindable
            },
            &mut budget(),
        )
        .unwrap()
}
fn call(graph: &mut PreparedEffectGraph, function: &PreparedEffectUnknownRef) -> PreparedEffectRow {
    graph
        .unknown_application(function, vec![], &mut budget())
        .unwrap()
}
fn equation(
    graph: &mut PreparedEffectGraph,
    left: PreparedEffectRow,
    right: PreparedEffectRow,
    parameters: Vec<GenericEffectReference>,
) {
    let predicate = graph
        .relate(PreparedEffectRelation::Equal, left, right, &mut budget())
        .unwrap();
    graph
        .add_scoped_obligation(parameters, truth(), predicate, &mut budget())
        .unwrap();
}
fn row(labels: &[&str]) -> EffectRow {
    EffectRow::closed(EffectSet::from_labels(labels.iter().copied()).unwrap())
}

#[test]
fn quantified_residual_exposes_original_scopes_dependencies_and_uniform_choices() {
    let mut work = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let e = parameter(&scope, 0);
    let mut graph = PreparedEffectGraph::default();
    let x = unknown(&mut graph, false, vec![]);
    let f = unknown(&mut graph, true, vec![e.clone()]);
    let scheme = graph
        .declare(vec![e.clone()], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let choice = graph.choice(&mut work).unwrap();
    let future = graph
        .unknown_application(&f, vec![open(&e)], &mut work)
        .unwrap();
    let selected = graph
        .select_row(&choice, future.clone(), literal(&[]), &mut work)
        .unwrap();
    graph
        .prepare_body(&scheme, selected, vec![], &mut work)
        .unwrap();
    let actual = call(&mut graph, &x);
    equation(&mut graph, actual, future, vec![e.clone()]);
    let frozen = graph.complete_scoped(&mut work).unwrap();
    let EffectResidualPredicateView::ExistsLocalFunctions {
        functions,
        support,
        body,
    } = frozen.satisfiable().view().unwrap()
    else {
        panic!("local functions must be bound outside all lexical scopes");
    };
    let functions: Vec<_> = functions.collect();
    assert_eq!(functions.len(), 2);
    assert!(
        functions
            .iter()
            .any(|function| function.is_unknown(&x).unwrap())
    );
    let futures: Vec<_> = support.future_exceptions.collect();
    assert_eq!(futures.len(), 1);
    assert!(futures[0].is_unknown(&f).unwrap());
    assert_eq!(futures[0].parameters(), &[e.clone()]);
    assert!(matches!(
        futures[0].role().unwrap(),
        ResidualFunctionRole::Future { actual: None }
    ));
    let EffectResidualPredicateView::ExistsUniformChoices { choices, body } = body.view().unwrap()
    else {
        panic!("one uniform choice quantifier must surround the relation");
    };
    assert_eq!(choices.collect::<Vec<_>>(), vec![choice]);
    let EffectResidualPredicateView::And(clauses) = body.view().unwrap() else {
        panic!("the original relation is a conjunction");
    };
    let mut lexical_scopes = 0;
    let mut saw_outer_equation = false;
    for clause in clauses {
        let EffectResidualPredicateView::ForAllFiniteRows {
            scope,
            parameters,
            body,
        } = clause.view().unwrap()
        else {
            continue;
        };
        lexical_scopes += 1;
        assert_eq!(parameters, &[e.clone()]);
        let EffectResidualPredicateView::Implies {
            domain,
            consequence,
        } = body.view().unwrap()
        else {
            panic!("whole domain and consequence must stay distinct");
        };
        assert!(matches!(
            domain.view().unwrap(),
            EffectResidualPredicateView::SourcePredicate(_)
        ));
        if matches!(scope, ResidualRowScope::Obligation(_)) {
            let EffectResidualPredicateView::SourcePredicate(source) = consequence.view().unwrap()
            else {
                panic!("the original obligation must be inspectable");
            };
            let ResidualPreparedPredicateView::Relation {
                relation,
                actual,
                permitted,
            } = source.view().unwrap()
            else {
                panic!("the obligation must retain its original operands");
            };
            assert_eq!(relation, PreparedEffectRelation::Equal);
            let ResidualPreparedRowView::Application {
                function,
                arguments,
            } = actual.view().unwrap()
            else {
                panic!("the left operand must retain its actual function");
            };
            assert!(function.is_unknown(&x).unwrap());
            assert_eq!(arguments.count(), 0);
            let ResidualPreparedRowView::Application {
                function,
                mut arguments,
            } = permitted.view().unwrap()
            else {
                panic!("the right operand must retain its actual function");
            };
            assert!(function.is_unknown(&f).unwrap());
            let argument = arguments.next().unwrap();
            assert_eq!(argument.parameters(), &[e.clone()]);
            assert!(
                matches!(argument.view().unwrap(), ResidualPreparedRowView::Canonical(value) if value.known().unwrap().single_reference() == Some(&e))
            );
            assert!(arguments.next().is_none());
            saw_outer_equation = true;
        }
    }
    assert_eq!(lexical_scopes, 2);
    assert!(saw_outer_equation);
    let EffectResidualPredicateView::And(mut principal) = frozen.principal().view().unwrap() else {
        panic!("principal must include satisfiability and its lower witness");
    };
    assert!(matches!(
        principal.next().unwrap().view().unwrap(),
        EffectResidualPredicateView::ExistsLocalFunctions { .. }
    ));
    let EffectResidualPredicateView::ExistsUniformChoices { body, .. } =
        principal.next().unwrap().view().unwrap()
    else {
        panic!("the lower tuple must retain a shared witness");
    };
    let EffectResidualPredicateView::SubstituteLower { bindings, .. } = body.view().unwrap() else {
        panic!("principal must replay the original relation at the lower tuple");
    };
    assert_eq!(bindings.count(), 2);
}

#[test]
fn lower_comprehensions_bind_a_logical_label_and_keep_the_principal_guard() {
    let mut graph = PreparedEffectGraph::default();
    let x = unknown(&mut graph, false, vec![]);
    let f = unknown(&mut graph, true, vec![]);
    let left = call(&mut graph, &x);
    let right = call(&mut graph, &f);
    equation(&mut graph, left, right, vec![]);
    let frozen = graph.complete_scoped(&mut budget()).unwrap();
    let guarded = frozen.unknown(&x).unwrap();
    let value = guarded.residual();
    assert!(value.function.is_unknown(&x).unwrap());
    assert!(matches!(
        value.principal.view().unwrap(),
        EffectResidualPredicateView::And(_)
    ));
    let EffectResidualPredicateView::And(mut lower) = value.lower.unwrap().view().unwrap() else {
        panic!("the lower comprehension must include global satisfiability");
    };
    assert!(matches!(
        lower.next().unwrap().view().unwrap(),
        EffectResidualPredicateView::ExistsLocalFunctions { .. }
    ));
    let EffectResidualPredicateView::Not(exists) = lower.next().unwrap().view().unwrap() else {
        panic!("membership is necessary in all satisfying tuples");
    };
    let EffectResidualPredicateView::ExistsLocalFunctions { body, .. } = exists.view().unwrap()
    else {
        panic!("the counterexample must quantify local functions");
    };
    let EffectResidualPredicateView::ExistsUniformChoices { body, .. } = body.view().unwrap()
    else {
        panic!("the counterexample must quantify its uniform witness");
    };
    let EffectResidualPredicateView::And(mut counterexample) = body.view().unwrap() else {
        panic!("counterexample is R and a missing member");
    };
    counterexample.next().unwrap();
    let EffectResidualPredicateView::Not(member) = counterexample.next().unwrap().view().unwrap()
    else {
        panic!("counterexample must negate membership");
    };
    let EffectResidualPredicateView::OutputMembership {
        function,
        scope,
        parameters,
        probe,
    } = member.view().unwrap()
    else {
        panic!("membership must retain a bound label probe");
    };
    assert!(function.is_unknown(&x).unwrap());
    assert!(probe.output.is_unknown(&x).unwrap());
    assert!(matches!(scope, ResidualRowScope::Output(_)));
    assert!(parameters.is_empty());
}

#[test]
fn partial_future_bindings_share_the_original_program_and_keep_remaining_inputs_free() {
    let mut child = PreparedEffectGraph::default();
    let x = unknown(&mut child, false, vec![]);
    let f = unknown(&mut child, true, vec![]);
    let g = unknown(&mut child, true, vec![]);
    let f_row = call(&mut child, &f);
    let g_row = call(&mut child, &g);
    let union = child.union(vec![f_row, g_row], &mut budget()).unwrap();
    let x_row = call(&mut child, &x);
    equation(&mut child, x_row, union, vec![]);
    let child = child.complete_scoped(&mut budget()).unwrap();
    let a = row(&["fresh.a"]);
    let partial = child
        .specialize_partial(&[(&f, &a)], &mut budget())
        .unwrap();
    assert!(Arc::ptr_eq(&child.graph, &partial.graph));
    assert!(Arc::ptr_eq(&child.program, &partial.program));
    assert!(child.fixed.is_empty());
    let EffectResidualPredicateView::ExistsLocalFunctions { support, .. } =
        partial.satisfiable().view().unwrap()
    else {
        unreachable!()
    };
    let mut fixed = 0;
    let mut free = 0;
    for function in support.future_exceptions {
        match function.role().unwrap() {
            ResidualFunctionRole::Future {
                actual: Some(value),
            } => {
                assert_eq!(value, &a);
                fixed += 1;
            }
            ResidualFunctionRole::Future { actual: None } => {
                assert!(function.is_unknown(&g).unwrap());
                free += 1;
            }
            _ => unreachable!(),
        }
    }
    assert_eq!((fixed, free), (1, 1));
    let mut parent = PreparedEffectGraph::default();
    let p = unknown(&mut parent, false, vec![]);
    let imported = parent
        .import_guarded_function(&partial.unknown(&x).unwrap(), &[(&g, &p)], &mut budget())
        .unwrap();
    let value = parent
        .imported_application(&imported, vec![], &mut budget())
        .unwrap();
    let actual = call(&mut parent, &p);
    equation(&mut parent, actual, literal(&["fresh.b"]), vec![]);
    let closed = parent.solve_principal(&mut budget()).unwrap();
    assert_eq!(
        closed.resolve_row(&value, &mut budget()).unwrap(),
        row(&["fresh.a", "fresh.b"])
    );
    let all = partial
        .specialize_partial(&[(&g, &row(&[]))], &mut budget())
        .unwrap();
    let mut outer = PreparedEffectGraph::default();
    let imported = outer
        .import_guarded_function(&all.unknown(&x).unwrap(), &[], &mut budget())
        .unwrap();
    let value = outer
        .imported_application(&imported, vec![], &mut budget())
        .unwrap();
    let closed = outer.solve_principal(&mut budget()).unwrap();
    assert_eq!(closed.resolve_row(&value, &mut budget()).unwrap(), a);
}

#[test]
fn partial_binding_admission_preserves_identity_dependencies_and_failure_atomicity() {
    let mut child = PreparedEffectGraph::default();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let e = parameter(&scope, 0);
    let alien_parameter = parameter(&scope, 1);
    let x = unknown(&mut child, false, vec![e.clone()]);
    let f = unknown(&mut child, true, vec![e.clone()]);
    let left = child
        .unknown_application(&x, vec![open(&e)], &mut budget())
        .unwrap();
    let right = child
        .unknown_application(&f, vec![open(&e)], &mut budget())
        .unwrap();
    equation(&mut child, left, right, vec![e.clone()]);
    let child = child.complete_scoped(&mut budget()).unwrap();
    let identity = EffectRow::open(EffectSet::new(), e.clone());
    let bad = EffectRow::open(EffectSet::new(), alien_parameter);
    assert!(matches!(
        child.specialize_partial(&[(&f, &bad)], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::UndeclaredParameter(_)
        ))
    ));
    assert!(matches!(
        child.specialize_partial(&[(&x, &identity)], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::FutureBinding
        ))
    ));
    assert!(matches!(
        child.specialize_partial(&[(&f, &identity), (&f, &identity)], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::DuplicateFutureBinding
        ))
    ));
    assert!(matches!(
        child.specialize_partial(&[(&f, &identity)], &mut Budget(0)),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    assert!(child.fixed.is_empty());
    let fixed = child
        .specialize_partial(&[(&f, &identity)], &mut budget())
        .unwrap();
    assert!(matches!(
        fixed.specialize_partial(&[(&f, &identity)], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::DuplicateFutureBinding
        ))
    ));
    let mut parent = PreparedEffectGraph::default();
    let p = unknown(&mut parent, false, vec![e]);
    assert!(matches!(
        parent.import_guarded_function(&fixed.unknown(&x).unwrap(), &[(&f, &p)], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::DuplicateFutureBinding
        ))
    ));
    let imported = parent
        .import_guarded_function(&fixed.unknown(&x).unwrap(), &[], &mut budget())
        .unwrap();
    let value = parent
        .imported_application(&imported, vec![literal(&["fresh.identity"])], &mut budget())
        .unwrap();
    let closed = parent.solve_principal(&mut budget()).unwrap();
    assert_eq!(
        closed.resolve_row(&value, &mut budget()).unwrap(),
        row(&["fresh.identity"])
    );
}

#[test]
fn finite_lowering_consumes_the_residual_relation_and_original_replay_remains_independent() {
    let mut graph = PreparedEffectGraph::default();
    let x = unknown(&mut graph, false, vec![]);
    let f = unknown(&mut graph, true, vec![]);
    let x_row = call(&mut graph, &x);
    let f_row = call(&mut graph, &f);
    equation(&mut graph, x_row, f_row, vec![]);
    let mut child = graph.complete_scoped(&mut budget()).unwrap();
    let program = Arc::get_mut(&mut Arc::get_mut(&mut child).unwrap().program).unwrap();
    let consequence = program
        .nodes
        .iter_mut()
        .find(|node| {
            matches!(
                node,
                ResidualNode::SourcePredicate(ResidualPredicateSource::ObligationConsequence(_))
            )
        })
        .unwrap();
    *consequence = ResidualNode::True;
    let mut parent = PreparedEffectGraph::default();
    let p = unknown(&mut parent, false, vec![]);
    let imported = parent
        .import_guarded_function(&child.unknown(&x).unwrap(), &[(&f, &p)], &mut budget())
        .unwrap();
    let value = parent
        .imported_application(&imported, vec![], &mut budget())
        .unwrap();
    let actual = call(&mut parent, &p);
    equation(&mut parent, actual, literal(&["log.write"]), vec![]);
    // If lowering rebuilt R from the original graph, it would ignore the
    // corrupted residual program and silently succeed instead of rejecting it.
    assert!(matches!(
        parent.solve_principal(&mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::PrincipalConfirmation
        ))
    ));
    assert!(matches!(value, PreparedEffectRow::Term(_)));
}
