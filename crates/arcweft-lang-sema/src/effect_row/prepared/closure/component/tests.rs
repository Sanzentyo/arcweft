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
