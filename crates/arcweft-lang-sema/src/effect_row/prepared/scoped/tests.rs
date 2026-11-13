use super::*;
use crate::effect_row::prepared::test_support::*;
use crate::types::{GenericBinder, GenericScope};

fn truth() -> PreparedEffectPredicate {
    PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained())
}

#[test]
fn an_outer_unknown_has_the_uniform_least_row_required_by_all_inner_valuations() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let f = parameter(&scope, 0);
    let x = graph.declare_unknown(vec![], &mut control).unwrap();
    let application = graph.unknown_application(&x, vec![], &mut control).unwrap();
    let allowed = crate::effects::EffectSet::from_labels(["log.write"]).unwrap();
    let required = EffectFormula::literal(allowed.clone(), None)
        .intersection(
            &EffectFormula::literal(crate::effects::EffectSet::new(), Some(f.clone())),
            &mut control,
        )
        .unwrap();
    let predicate = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            PreparedEffectRow::Canonical(EffectRow {
                formula: Some(required),
            }),
            application,
            &mut control,
        )
        .unwrap();
    graph
        .add_scoped_obligation(vec![f], truth(), predicate, &mut control)
        .unwrap();
    let closed = graph.solve_principal(&mut control).unwrap();
    assert_eq!(closed.unknown(&x).unwrap(), &EffectRow::closed(allowed));
}

#[test]
fn declared_recursion_and_unknown_functions_share_one_joint_closure() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let f = parameter(&scope, 0);
    let unknown = graph
        .declare_unknown(vec![f.clone()], &mut control)
        .unwrap();
    let scheme = graph
        .declare(
            vec![f.clone()],
            EffectPredicate::unconstrained(),
            &mut control,
        )
        .unwrap();
    let recursive = graph
        .application(&scheme, vec![open(&f)], &mut control)
        .unwrap();
    let inferred = graph
        .unknown_application(&unknown, vec![open(&f)], &mut control)
        .unwrap();
    let equality = graph
        .relate(
            PreparedEffectRelation::Equal,
            inferred.clone(),
            open(&f),
            &mut control,
        )
        .unwrap();
    graph
        .add_scoped_obligation(vec![f.clone()], truth(), equality, &mut control)
        .unwrap();
    let body = graph
        .union(vec![recursive, inferred], &mut control)
        .unwrap();
    graph
        .prepare_body(&scheme, body, vec![], &mut control)
        .unwrap();
    let call = graph
        .application(&scheme, vec![literal(&["log.write"])], &mut control)
        .unwrap();
    let closed = graph.solve_principal(&mut control).unwrap();
    assert_eq!(
        closed.unknown(&unknown).unwrap(),
        closed.scheme(&scheme).unwrap()
    );
    assert_eq!(
        closed.scheme(&scheme).unwrap(),
        &EffectRow::open(crate::effects::EffectSet::new(), f)
    );
    assert_eq!(
        closed.resolve_row(&call, &mut control).unwrap(),
        EffectRow::closed(crate::effects::EffectSet::from_labels(["log.write"]).unwrap())
    );
}

#[test]
fn failed_unknown_and_obligation_admission_preserves_the_graph() {
    let mut graph = PreparedEffectGraph::default();
    let mut exhausted = Budget(0);
    assert!(matches!(
        graph.declare_unknown(vec![], &mut exhausted),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    assert!(graph.functions.is_empty());
    assert!(matches!(
        graph.add_scoped_obligation(vec![], truth(), truth(), &mut exhausted),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    assert!(graph.obligations.is_empty());
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let f = parameter(&scope, 0);
    assert!(matches!(
        graph.declare_unknown(vec![f.clone(), f.clone()], &mut control),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::DuplicateParameter
        ))
    ));
    let unknown = graph.declare_unknown(vec![f], &mut control).unwrap();
    assert!(matches!(
        graph.unknown_application(&unknown, vec![], &mut control),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ApplicationArity
        ))
    ));
    assert_eq!(graph.retained_terms(), (0, 0));
}

#[test]
fn an_inner_unknown_can_depend_on_its_introduction_parameter() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let f = parameter(&scope, 0);
    let e = graph
        .declare_unknown(vec![f.clone()], &mut control)
        .unwrap();
    let application = graph
        .unknown_application(&e, vec![open(&f)], &mut control)
        .unwrap();
    let equal = graph
        .relate(
            PreparedEffectRelation::Equal,
            application,
            open(&f),
            &mut control,
        )
        .unwrap();
    graph
        .add_scoped_obligation(vec![f.clone()], truth(), equal, &mut control)
        .unwrap();
    let closed = graph.solve_principal(&mut control).unwrap();
    assert_eq!(
        closed.unknown(&e).unwrap(),
        &EffectRow::open(crate::effects::EffectSet::new(), f)
    );
}

#[test]
fn an_outer_unknown_cannot_acquire_a_later_universal_parameter() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let f = parameter(&scope, 0);
    let x = graph.declare_unknown(vec![], &mut control).unwrap();
    let application = graph.unknown_application(&x, vec![], &mut control).unwrap();
    let equal = graph
        .relate(
            PreparedEffectRelation::Equal,
            application,
            open(&f),
            &mut control,
        )
        .unwrap();
    graph
        .add_scoped_obligation(vec![f], truth(), equal, &mut control)
        .unwrap();
    assert!(matches!(
        graph.solve_principal(&mut control),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
}

#[test]
fn jointly_minimized_outer_and_inner_unknowns_reject_incomparable_solutions() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let f = parameter(&scope, 0);
    let x = graph.declare_unknown(vec![], &mut control).unwrap();
    let y = graph
        .declare_unknown(vec![f.clone()], &mut control)
        .unwrap();
    let x = graph.unknown_application(&x, vec![], &mut control).unwrap();
    let y = graph
        .unknown_application(&y, vec![open(&f)], &mut control)
        .unwrap();
    let union = graph.union(vec![x, y], &mut control).unwrap();
    let equal = graph
        .relate(
            PreparedEffectRelation::Equal,
            union,
            literal(&["log.write"]),
            &mut control,
        )
        .unwrap();
    graph
        .add_scoped_obligation(vec![f], truth(), equal, &mut control)
        .unwrap();
    assert!(matches!(
        graph.solve_principal(&mut control),
        Err(PreparedEffectFailure::NonPrincipal)
    ));
}

#[test]
fn a_false_whole_row_domain_vacuously_admits_its_obligation() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    // D is false at random.read; C is false at log.write. Pointwise D=>C
    // would reject log.write, whereas the whole-row domain is simply false.
    let domain = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            literal(&["log.write", "random.read"]),
            literal(&["log.write"]),
            &mut control,
        )
        .unwrap();
    let predicate = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            literal(&["log.write", "random.read"]),
            literal(&["random.read"]),
            &mut control,
        )
        .unwrap();
    graph
        .add_scoped_obligation(vec![], domain, predicate, &mut control)
        .unwrap();
    graph.solve_principal(&mut control).unwrap();
}

#[test]
fn a_live_whole_row_domain_does_not_discard_a_false_obligation() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let predicate = graph
        .relate(
            PreparedEffectRelation::Equal,
            literal(&["log.write"]),
            literal(&[]),
            &mut control,
        )
        .unwrap();
    graph
        .add_scoped_obligation(vec![], truth(), predicate, &mut control)
        .unwrap();
    assert!(matches!(
        graph.solve_principal(&mut control),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
}

#[test]
fn rollback_removes_scoped_obligations_and_never_reuses_unknown_identity() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let mut checkpoint = graph.checkpoint(&mut control).unwrap();
    let stale = graph.declare_unknown(vec![], &mut control).unwrap();
    let impossible = graph
        .relate(
            PreparedEffectRelation::Equal,
            literal(&["log.write"]),
            literal(&[]),
            &mut control,
        )
        .unwrap();
    graph
        .add_scoped_obligation(vec![], truth(), impossible, &mut control)
        .unwrap();
    graph.rollback(&mut checkpoint).unwrap();
    let fresh = graph.declare_unknown(vec![], &mut control).unwrap();
    assert_ne!(stale, fresh);
    assert!(matches!(
        graph.unknown_application(&stale, vec![], &mut control),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::StaleReference
        ))
    ));
    let mut other = PreparedEffectGraph::default();
    assert!(matches!(
        other.unknown_application(&fresh, vec![], &mut control),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ForeignIssuer
        ))
    ));
    let closed = graph.solve_principal(&mut control).unwrap();
    assert!(closed.unknown(&fresh).unwrap().is_empty());
}
