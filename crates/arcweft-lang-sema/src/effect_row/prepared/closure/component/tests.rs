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
fn owned_snapshot_closes_without_recopying_the_definitions() {
    use crate::effect_row::prepared::read::SnapshotSource;

    let mut graph = PreparedEffectGraph::default();
    let future = unknown(&mut graph, PreparedEffectUnknownEligibility::Future);
    let row = graph
        .unknown_application(&future, vec![], &mut budget())
        .unwrap();
    let snapshot = graph
        .read()
        .snapshot(
            &[&row],
            &[],
            &[],
            SnapshotSource::Observation(&[]),
            &mut budget(),
        )
        .unwrap();
    let actual = EffectRow::closed(EffectSet::from_labels(["log.write"]).unwrap());
    let inputs = graph
        .read()
        .actual_inputs(&[(&future, &actual)], &mut budget())
        .unwrap();
    let mut measured = budget();
    let borrowed_work = measured.0;
    let borrowed = ClosureDefinitions::Borrowed(&snapshot)
        .solve_with_actual_inputs(&inputs, &mut measured)
        .unwrap();
    let available = borrowed_work - measured.0 - 1;
    assert!(matches!(
        ClosureDefinitions::Borrowed(&snapshot)
            .solve_with_actual_inputs(&inputs, &mut Budget(available)),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    let owned = snapshot
        .solve_with_actual_inputs(&inputs, &mut Budget(available))
        .unwrap();

    drop(graph);
    assert_eq!(borrowed.resolve_row(&row, &mut budget()).unwrap(), actual);
    assert_eq!(owned.resolve_row(&row, &mut budget()).unwrap(), actual);
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
    let result = CompiledComponent::new(&graph.relation, &BTreeMap::new(), &mut Work(&mut work))
        .unwrap()
        .complete(&mut Work(&mut work))
        .unwrap();
    let future = Decision::variable(cell(&f, 1), &mut Work(&mut work)).unwrap();
    let finite = Decision::variable(cell(&f, 0), &mut Work(&mut work))
        .unwrap()
        .not(&mut Work(&mut work))
        .unwrap();
    assert_eq!(&result.principal, &finite);
    assert_eq!(&result.satisfiable, &finite);
    assert_eq!(
        result.coefficients.get(&cell(&x, 1)).unwrap(),
        &finite.and(&future, &mut Work(&mut work)).unwrap()
    );
    assert!(result.coefficients.get(&cell(&f, 1)).is_none());
    assert!(result.choice_witnesses.is_none());
}

#[test]
fn an_extra_label_requires_all_local_memberships_to_agree_when_inputs_agree() {
    let mut graph = PreparedEffectGraph::default();
    let future = unknown(&mut graph, PreparedEffectUnknownEligibility::Future);
    let first = unknown(&mut graph, PreparedEffectUnknownEligibility::Bindable);
    let second = unknown(&mut graph, PreparedEffectUnknownEligibility::Bindable);
    let functions = [&future, &first, &second];
    let cells: Vec<_> = functions
        .into_iter()
        .flat_map(|function| (0..3).map(move |class| cell(function, class)))
        .collect();
    let labels: Arc<[_]> = EffectSet::from_labels(["fs.read", "log.write"])
        .unwrap()
        .iter()
        .cloned()
        .collect();
    let authored = [BTreeSet::new(), BTreeSet::from([labels[0].clone()])];
    let future_id = FunctionRef::Unknown(future).reference().ordinal;
    let mut control = budget();
    let work = &mut Work(&mut control);
    let program = EffectResidualProgram::new(&graph.relation, work).unwrap();
    let partial = EffectSet::from_labels(["log.write"]).unwrap();
    let empty = EffectSet::new();
    for fixed in [None, Some(&empty), Some(&partial)] {
        let rows = fixed
            .map(|effects| (future_id, EffectRow::closed(effects.clone())))
            .into_iter()
            .collect();
        let compile = |support, work: &mut Work<'_, Budget>| {
            let layout =
                Layout::with_shared_labels(&graph.relation, Arc::clone(&labels), work).unwrap();
            let actual = FixedCells::new(&graph.relation, &rows, layout.view(), work).unwrap();
            CompiledComponent::compile(
                &graph.relation,
                &program,
                actual,
                layout,
                CompiledObligations::default(),
                support,
                work,
            )
            .unwrap()
            .relation
        };
        let unrestricted = compile(None, work);
        for authored in &authored {
            let restricted = compile(Some(authored), work);
            for mask in 0..(1 << cells.len()) {
                let membership = |index| (mask & (1 << index)) != 0;
                let assignment =
                    |atom: &Atom| membership(cells.iter().position(|cell| cell == atom).unwrap());
                let originally_admitted = unrestricted
                    .evaluate_with_control(assignment, work)
                    .unwrap();
                let support_admitted = labels.iter().enumerate().all(|(index, label)| {
                    let class = index + 1;
                    let input_differs = fixed.map_or_else(
                        || membership(0) != membership(class),
                        |effects| effects.contains(label),
                    );
                    let outputs_agree = [3, 6]
                        .into_iter()
                        .all(|outside| membership(outside) == membership(outside + class));
                    authored.contains(label) || input_differs || outputs_agree
                });
                assert_eq!(
                    restricted.evaluate_with_control(assignment, work).unwrap(),
                    originally_admitted && support_admitted,
                    "fixed={fixed:?}, authored={authored:?}, cells={mask:09b}",
                );
            }
        }
    }
}

#[test]
fn conditional_completion_rejects_an_invalid_cell_even_with_a_total_principal_domain() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    retain_label_basis(&mut graph);
    let local = unknown(&mut graph, PreparedEffectUnknownEligibility::Bindable);
    let future = unknown(&mut graph, PreparedEffectUnknownEligibility::Future);
    let mut work = Work(&mut work);
    let mut component =
        CompiledComponent::new(&graph.relation, &BTreeMap::new(), &mut work).unwrap();
    let local = Decision::variable(cell(&local, 1), &mut work).unwrap();
    let outside_layout = Decision::variable(cell(&future, 2), &mut work).unwrap();
    // Every valuation has a least witness, but the witness would refer to a
    // class that the declared input does not have.
    component.relation = Arc::new(local.equivalent(&outside_layout, &mut work).unwrap());
    assert!(matches!(
        component.complete(&mut work),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::UnclosedCell
        ))
    ));
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
    let component =
        CompiledComponent::new(&graph.relation, &BTreeMap::new(), &mut Work(&mut work)).unwrap();
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
            .principal
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
            .principal
            .substitute(&empty, &mut Work(&mut work))
            .unwrap()
            .is_constant(true)
    );
    assert!(
        result
            .coefficients
            .get(&cell(&x, 1))
            .unwrap()
            .is_constant(false)
    );
    assert!(
        result
            .coefficients
            .get(&cell(&y, 1))
            .unwrap()
            .is_constant(false)
    );
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
    let result = CompiledComponent::new(&graph.relation, &BTreeMap::new(), &mut Work(&mut work))
        .unwrap()
        .complete(&mut Work(&mut work))
        .unwrap();
    assert_eq!(result.layout.labels.len(), 2);
    let witnesses = result.choice_witnesses.as_ref().unwrap();
    assert!(
        witnesses
            .relation
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
            (cell(&f, 0), false),
            (cell(&f, 1), left),
            (cell(&f, 2), right),
        ]);
        assert!(
            result
                .principal
                .restrict(&actual, &mut Work(&mut work))
                .unwrap()
                .is_constant(admitted)
        );
        let specialized = witnesses.specialize(&actual, &mut Work(&mut work)).unwrap();
        let closed = specialized.close(&mut Work(&mut work));
        if admitted {
            assert!(closed.is_ok());
        } else {
            assert!(matches!(
                closed,
                Err(PreparedEffectFailure::Invariant(
                    PreparedEffectInvariant::PrincipalConfirmation
                ))
            ));
        }
    }
}

#[test]
fn conditional_completion_cancellation_does_not_change_the_prepared_graph() {
    let mut graph = PreparedEffectGraph::default();
    let f = unknown(&mut graph, PreparedEffectUnknownEligibility::Future);
    let mut work = budget();
    let component =
        CompiledComponent::new(&graph.relation, &BTreeMap::new(), &mut Work(&mut work)).unwrap();
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
