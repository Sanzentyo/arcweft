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
    eligibility: PreparedEffectUnknownEligibility,
) -> PreparedEffectUnknownRef {
    graph
        .declare_unknown(vec![], eligibility, &mut budget())
        .unwrap()
}

/// A retained label is part of this component's coefficient basis. This helper
/// does not claim that arbitrary unresolved external functions have no other
/// support; the unbounded residual/import boundary is a separate operation.
fn retain_label_basis(graph: &mut PreparedEffectGraph) {
    let mut work = budget();
    let equal = graph
        .relate(
            PreparedEffectRelation::Equal,
            literal(&["log.write"]),
            literal(&["log.write"]),
            &mut work,
        )
        .unwrap();
    graph
        .add_scoped_obligation(vec![], truth(), equal, &mut work)
        .unwrap();
}

fn cell(reference: &PreparedEffectUnknownRef, class: usize) -> Atom {
    Atom::Cell {
        scheme: FunctionRef::Unknown(reference.clone()).reference().ordinal,
        class,
        mask: 0,
    }
}

#[test]
fn conditional_completion_keeps_future_cells_free_in_the_least_function() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    retain_label_basis(&mut graph);
    let x = unknown(&mut graph, PreparedEffectUnknownEligibility::Bindable);
    let f = unknown(&mut graph, PreparedEffectUnknownEligibility::Future);
    let x_row = graph.unknown_application(&x, vec![], &mut work).unwrap();
    let f_row = graph.unknown_application(&f, vec![], &mut work).unwrap();
    let equal = graph
        .relate(PreparedEffectRelation::Equal, x_row, f_row, &mut work)
        .unwrap();
    graph
        .add_scoped_obligation(vec![], truth(), equal, &mut work)
        .unwrap();
    let result = CompiledComponent::new(&graph, &BTreeMap::new(), &mut Work(&mut work))
        .unwrap()
        .complete(&mut Work(&mut work))
        .unwrap();
    let future = Decision::variable(cell(&f, 1), &mut Work(&mut work)).unwrap();
    let finite = Decision::variable(cell(&f, 0), &mut Work(&mut work))
        .unwrap()
        .not(&mut Work(&mut work))
        .unwrap();
    assert_eq!(result.completion.principal(), &finite);
    assert_eq!(result.completion.satisfiable(), &finite);
    assert_eq!(
        result.coefficients[&cell(&x, 1)],
        finite.and(&future, &mut Work(&mut work)).unwrap()
    );
    assert!(!result.coefficients.contains_key(&cell(&f, 1)));
    assert_eq!(result.witnesses, finite);
}

#[test]
fn a_parent_restriction_cannot_rescue_a_nonprincipal_child_completion() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    retain_label_basis(&mut graph);
    let x = unknown(&mut graph, PreparedEffectUnknownEligibility::Bindable);
    let y = unknown(&mut graph, PreparedEffectUnknownEligibility::Bindable);
    let f = unknown(&mut graph, PreparedEffectUnknownEligibility::Future);
    let x_row = graph.unknown_application(&x, vec![], &mut work).unwrap();
    let y_row = graph.unknown_application(&y, vec![], &mut work).unwrap();
    let f_row = graph.unknown_application(&f, vec![], &mut work).unwrap();
    let union = graph.union(vec![x_row, y_row], &mut work).unwrap();
    let equal = graph
        .relate(PreparedEffectRelation::Equal, union, f_row, &mut work)
        .unwrap();
    graph
        .add_scoped_obligation(vec![], truth(), equal, &mut work)
        .unwrap();
    let component = CompiledComponent::new(&graph, &BTreeMap::new(), &mut Work(&mut work)).unwrap();
    let parent = BTreeMap::from([
        (cell(&f, 0), Decision::constant(false)),
        (cell(&f, 1), Decision::constant(true)),
        (cell(&x, 0), Decision::constant(false)),
        (cell(&x, 1), Decision::constant(true)),
        (cell(&y, 0), Decision::constant(false)),
        (cell(&y, 1), Decision::constant(false)),
    ]);
    // Re-solving the union together with a parent's choice of x would accept.
    assert!(
        component
            .relation
            .substitute(&parent, &mut Work(&mut work))
            .unwrap()
            .is_constant(true)
    );
    let result = component.complete(&mut Work(&mut work)).unwrap();
    assert!(
        result
            .completion
            .principal()
            .substitute(&parent, &mut Work(&mut work))
            .unwrap()
            .is_constant(false)
    );
    let empty = BTreeMap::from([
        (cell(&f, 0), Decision::constant(false)),
        (cell(&f, 1), Decision::constant(false)),
    ]);
    assert!(
        result
            .completion
            .principal()
            .substitute(&empty, &mut Work(&mut work))
            .unwrap()
            .is_constant(true)
    );
    assert!(result.coefficients[&cell(&x, 1)].is_constant(false));
    assert!(result.coefficients[&cell(&y, 1)].is_constant(false));
}

#[test]
fn conditional_completion_preserves_one_choice_for_the_whole_future_row() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let x = unknown(&mut graph, PreparedEffectUnknownEligibility::Bindable);
    let f = unknown(&mut graph, PreparedEffectUnknownEligibility::Future);
    let choice = graph.choice(&mut work).unwrap();
    let selected = graph
        .select_row(
            &choice,
            literal(&["log.write"]),
            literal(&["random.read"]),
            &mut work,
        )
        .unwrap();
    let f_row = graph.unknown_application(&f, vec![], &mut work).unwrap();
    let actual = graph
        .relate(
            PreparedEffectRelation::Equal,
            f_row.clone(),
            selected,
            &mut work,
        )
        .unwrap();
    let x_row = graph.unknown_application(&x, vec![], &mut work).unwrap();
    let least = graph
        .relate(PreparedEffectRelation::Equal, x_row, f_row, &mut work)
        .unwrap();
    for predicate in [actual, least] {
        graph
            .add_scoped_obligation(vec![], truth(), predicate, &mut work)
            .unwrap();
    }
    let result = CompiledComponent::new(&graph, &BTreeMap::new(), &mut Work(&mut work))
        .unwrap()
        .complete(&mut Work(&mut work))
        .unwrap();
    assert_eq!(result.layout.labels.len(), 2);
    assert!(
        result
            .witnesses
            .variables()
            .any(|atom| matches!(atom, Atom::Choice(_)))
    );
    for (left, right, admitted) in [
        (true, false, true),
        (false, true, true),
        (true, true, false),
        (false, false, false),
    ] {
        let actual = BTreeMap::from([
            (cell(&f, 0), Decision::constant(false)),
            (cell(&f, 1), Decision::constant(left)),
            (cell(&f, 2), Decision::constant(right)),
        ]);
        assert!(
            result
                .completion
                .principal()
                .substitute(&actual, &mut Work(&mut work))
                .unwrap()
                .is_constant(admitted)
        );
        assert!(
            result
                .witnesses
                .substitute(&actual, &mut Work(&mut work))
                .unwrap()
                .exists(&result.choices, &mut Work(&mut work))
                .unwrap()
                .is_constant(admitted)
        );
    }
}

#[test]
fn conditional_completion_cancellation_does_not_change_the_prepared_graph() {
    let mut graph = PreparedEffectGraph::default();
    let f = unknown(&mut graph, PreparedEffectUnknownEligibility::Future);
    let mut work = budget();
    let component = CompiledComponent::new(&graph, &BTreeMap::new(), &mut Work(&mut work)).unwrap();
    assert!(matches!(
        component.complete(&mut Work(&mut Budget(0))),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    let empty = EffectRow::closed(EffectSet::new());
    let result = graph
        .specialize_principal(&[(&f, &empty)], &mut work)
        .unwrap();
    assert!(result.unknown(&f).unwrap().is_empty());
}

#[test]
fn future_rows_keep_their_actual_new_labels_and_can_be_specialized_independently() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let x = unknown(&mut graph, PreparedEffectUnknownEligibility::Bindable);
    let f = unknown(&mut graph, PreparedEffectUnknownEligibility::Future);
    let x_row = graph.unknown_application(&x, vec![], &mut work).unwrap();
    let f_row = graph.unknown_application(&f, vec![], &mut work).unwrap();
    let equal = graph
        .relate(PreparedEffectRelation::Equal, x_row, f_row, &mut work)
        .unwrap();
    graph
        .add_scoped_obligation(vec![], truth(), equal, &mut work)
        .unwrap();
    assert!(matches!(
        graph.solve_principal(&mut work),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::UnresolvedFuture
        ))
    ));
    let a = EffectRow::closed(EffectSet::from_labels(["log.write"]).unwrap());
    let b = EffectRow::closed(EffectSet::from_labels(["random.read"]).unwrap());
    let first = graph.specialize_principal(&[(&f, &a)], &mut work).unwrap();
    let second = graph.specialize_principal(&[(&f, &b)], &mut work).unwrap();
    assert_eq!(first.unknown(&x).unwrap(), &a);
    assert_eq!(first.unknown(&f).unwrap(), &a);
    assert_eq!(second.unknown(&x).unwrap(), &b);
    assert_eq!(second.unknown(&f).unwrap(), &b);
    assert!(matches!(
        graph.solve_principal(&mut work),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::UnresolvedFuture
        ))
    ));
}

#[test]
fn a_future_function_retains_its_parameter_dependencies() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let e = parameter(&scope, 0);
    let alien = parameter(&scope, 1);
    let x = graph
        .declare_unknown(
            vec![e.clone()],
            PreparedEffectUnknownEligibility::Bindable,
            &mut work,
        )
        .unwrap();
    let f = graph
        .declare_unknown(
            vec![e.clone()],
            PreparedEffectUnknownEligibility::Future,
            &mut work,
        )
        .unwrap();
    let x_row = graph
        .unknown_application(&x, vec![open(&e)], &mut work)
        .unwrap();
    let f_row = graph
        .unknown_application(&f, vec![open(&e)], &mut work)
        .unwrap();
    let equal = graph
        .relate(PreparedEffectRelation::Equal, x_row, f_row, &mut work)
        .unwrap();
    graph
        .add_scoped_obligation(vec![e.clone()], truth(), equal, &mut work)
        .unwrap();
    let identity = EffectRow::open(EffectSet::new(), e);
    let closed = graph
        .specialize_principal(&[(&f, &identity)], &mut work)
        .unwrap();
    assert_eq!(closed.unknown(&x).unwrap(), &identity);
    let foreign = EffectRow::open(EffectSet::new(), alien.clone());
    assert!(
        matches!(graph.specialize_principal(&[(&f, &foreign)], &mut work),
        Err(PreparedEffectFailure::Invariant(PreparedEffectInvariant::UndeclaredParameter(reference))) if reference == alien)
    );
}

#[test]
fn a_future_value_can_make_a_joint_solution_nonprincipal() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let x = unknown(&mut graph, PreparedEffectUnknownEligibility::Bindable);
    let y = unknown(&mut graph, PreparedEffectUnknownEligibility::Bindable);
    let f = unknown(&mut graph, PreparedEffectUnknownEligibility::Future);
    let x_row = graph.unknown_application(&x, vec![], &mut work).unwrap();
    let y_row = graph.unknown_application(&y, vec![], &mut work).unwrap();
    let f_row = graph.unknown_application(&f, vec![], &mut work).unwrap();
    let union = graph.union(vec![x_row, y_row], &mut work).unwrap();
    let equal = graph
        .relate(PreparedEffectRelation::Equal, union, f_row, &mut work)
        .unwrap();
    graph
        .add_scoped_obligation(vec![], truth(), equal, &mut work)
        .unwrap();
    let empty = EffectRow::closed(EffectSet::new());
    let closed = graph
        .specialize_principal(&[(&f, &empty)], &mut work)
        .unwrap();
    assert!(closed.unknown(&x).unwrap().is_empty());
    assert!(closed.unknown(&y).unwrap().is_empty());
    let nonempty = EffectRow::closed(EffectSet::from_labels(["log.write"]).unwrap());
    assert!(matches!(
        graph.specialize_principal(&[(&f, &nonempty)], &mut work),
        Err(PreparedEffectFailure::NonPrincipal)
    ));
}

#[test]
fn a_future_value_can_make_scoped_obligations_unsatisfiable() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let f = unknown(&mut graph, PreparedEffectUnknownEligibility::Future);
    let f_row = graph.unknown_application(&f, vec![], &mut work).unwrap();
    let equal = graph
        .relate(
            PreparedEffectRelation::Equal,
            f_row,
            literal(&[]),
            &mut work,
        )
        .unwrap();
    graph
        .add_scoped_obligation(vec![], truth(), equal, &mut work)
        .unwrap();
    graph
        .specialize_principal(&[(&f, &EffectRow::closed(EffectSet::new()))], &mut work)
        .unwrap();
    let nonempty = EffectRow::closed(EffectSet::from_labels(["log.write"]).unwrap());
    assert!(matches!(
        graph.specialize_principal(&[(&f, &nonempty)], &mut work),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
}

#[test]
fn an_outer_unknown_cannot_capture_a_future_functions_local_parameter() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let e = parameter(&scope, 0);
    let x = unknown(&mut graph, PreparedEffectUnknownEligibility::Bindable);
    let f = graph
        .declare_unknown(
            vec![e.clone()],
            PreparedEffectUnknownEligibility::Future,
            &mut work,
        )
        .unwrap();
    let x_row = graph.unknown_application(&x, vec![], &mut work).unwrap();
    let f_row = graph
        .unknown_application(&f, vec![open(&e)], &mut work)
        .unwrap();
    let equal = graph
        .relate(PreparedEffectRelation::Equal, x_row, f_row, &mut work)
        .unwrap();
    graph
        .add_scoped_obligation(vec![e.clone()], truth(), equal, &mut work)
        .unwrap();
    let identity = EffectRow::open(EffectSet::new(), e);
    assert!(matches!(
        graph.specialize_principal(&[(&f, &identity)], &mut work),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
    let constant = EffectRow::closed(EffectSet::from_labels(["log.write"]).unwrap());
    assert_eq!(
        graph
            .specialize_principal(&[(&f, &constant)], &mut work)
            .unwrap()
            .unknown(&x)
            .unwrap(),
        &constant
    );
}

#[test]
fn future_bindings_reject_missing_duplicates_foreign_and_bindable_references() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let f = unknown(&mut graph, PreparedEffectUnknownEligibility::Future);
    let x = unknown(&mut graph, PreparedEffectUnknownEligibility::Bindable);
    let empty = EffectRow::closed(EffectSet::new());
    let terms = graph.retained_terms();
    assert!(matches!(
        graph.specialize_principal(&[(&f, &empty)], &mut Budget(0)),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    assert_eq!(graph.retained_terms(), terms);
    assert!(matches!(
        graph.specialize_principal(&[(&f, &EffectRow::unknown())], &mut work),
        Err(PreparedEffectFailure::Row(EffectRowError::UnknownRow))
    ));
    assert!(matches!(
        graph.specialize_principal(&[], &mut work),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::UnresolvedFuture
        ))
    ));
    assert!(matches!(
        graph.specialize_principal(&[(&f, &empty), (&f, &empty)], &mut work),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::DuplicateFutureBinding
        ))
    ));
    assert!(matches!(
        graph.specialize_principal(&[(&x, &empty)], &mut work),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::FutureBinding
        ))
    ));
    let other = PreparedEffectGraph::default();
    assert!(matches!(
        other.specialize_principal(&[(&f, &empty)], &mut work),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ForeignIssuer
        ))
    ));
    graph
        .specialize_principal(&[(&f, &empty)], &mut work)
        .unwrap();
}
