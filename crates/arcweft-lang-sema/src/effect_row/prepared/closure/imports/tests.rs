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
    dependencies: Vec<GenericEffectReference>,
) -> PreparedEffectUnknownRef {
    graph
        .declare_unknown(
            dependencies,
            if future {
                PreparedEffectUnknownEligibility::Future
            } else {
                PreparedEffectUnknownEligibility::Bindable
            },
            &mut budget(),
        )
        .unwrap()
}

fn equation(
    graph: &mut PreparedEffectGraph,
    left: PreparedEffectRow,
    right: PreparedEffectRow,
    parameters: Vec<GenericEffectReference>,
) {
    let mut work = budget();
    let relation = graph
        .relate(PreparedEffectRelation::Equal, left, right, &mut work)
        .unwrap();
    graph
        .add_scoped_obligation(parameters, truth(), relation, &mut work)
        .unwrap();
}

fn call(graph: &mut PreparedEffectGraph, function: &PreparedEffectUnknownRef) -> PreparedEffectRow {
    graph
        .unknown_application(function, vec![], &mut budget())
        .unwrap()
}

fn id(reference: &PreparedEffectImportedRef) -> u64 {
    FunctionRef::Imported(reference.clone()).reference().ordinal
}

fn child_equality() -> (
    Arc<ScopedEffectCompletion>,
    PreparedEffectUnknownRef,
    PreparedEffectUnknownRef,
) {
    let mut child = PreparedEffectGraph::default();
    let x = unknown(&mut child, false, vec![]);
    let f = unknown(&mut child, true, vec![]);
    let left = call(&mut child, &x);
    let right = call(&mut child, &f);
    equation(&mut child, left, right, vec![]);
    (child.complete_scoped(&mut budget()).unwrap(), x, f)
}

#[test]
fn imported_future_functions_are_symbolic_parent_coefficients_not_closed_rows() {
    let (child, x, f) = child_equality();
    let guarded = child.unknown(&x).unwrap();
    let mut parent = PreparedEffectGraph::default();
    let p = unknown(&mut parent, false, vec![]);
    let result = unknown(&mut parent, false, vec![]);
    let imported = parent
        .import_guarded_function(&guarded, &[(&f, &p)], &mut budget())
        .unwrap();
    let value = parent
        .imported_application(&imported, vec![], &mut budget())
        .unwrap();
    let output = call(&mut parent, &result);
    equation(&mut parent, output, value.clone(), vec![]);
    // Supply the parent's constraint after importing the still-symbolic child.
    let actual = call(&mut parent, &p);
    equation(&mut parent, actual, literal(&["log.write"]), vec![]);
    let closed = parent.solve_principal(&mut budget()).unwrap();
    let expected = EffectRow::closed(EffectSet::from_labels(["log.write"]).unwrap());
    assert_eq!(closed.unknown(&result).unwrap(), &expected);
    assert_eq!(closed.resolve_row(&value, &mut budget()).unwrap(), expected);
    assert!(
        !closed
            .layout
            .cells
            .iter()
            .any(|cell| matches!(cell, Atom::Cell { scheme, .. } if *scheme == id(&imported)))
    );
}

#[test]
fn parent_constraints_cannot_rescue_a_nonprincipal_imported_child() {
    let mut child = PreparedEffectGraph::default();
    let x = unknown(&mut child, false, vec![]);
    let y = unknown(&mut child, false, vec![]);
    let f = unknown(&mut child, true, vec![]);
    let x_row = call(&mut child, &x);
    let y_row = call(&mut child, &y);
    let f_row = call(&mut child, &f);
    let union = child.union(vec![x_row, y_row], &mut budget()).unwrap();
    equation(&mut child, union, f_row, vec![]);
    let child = child.complete_scoped(&mut budget()).unwrap();
    for nonempty in [false, true] {
        let mut parent = PreparedEffectGraph::default();
        let p = unknown(&mut parent, false, vec![]);
        let ix = parent
            .import_guarded_function(&child.unknown(&x).unwrap(), &[(&f, &p)], &mut budget())
            .unwrap();
        let iy = parent
            .import_guarded_function(&child.unknown(&y).unwrap(), &[(&f, &p)], &mut budget())
            .unwrap();
        let x_row = parent
            .imported_application(&ix, vec![], &mut budget())
            .unwrap();
        let y_row = parent
            .imported_application(&iy, vec![], &mut budget())
            .unwrap();
        let expected = if nonempty {
            literal(&["log.write"])
        } else {
            literal(&[])
        };
        let p_row = call(&mut parent, &p);
        equation(&mut parent, p_row, expected.clone(), vec![]);
        equation(&mut parent, x_row, expected, vec![]);
        equation(&mut parent, y_row, literal(&[]), vec![]);
        if nonempty {
            assert!(matches!(
                parent.solve_principal(&mut budget()),
                Err(PreparedEffectFailure::Unsatisfiable)
            ));
        } else {
            let closed = parent.solve_principal(&mut budget()).unwrap();
            assert!(closed.unknown(&p).unwrap().is_empty());
            assert!(Arc::ptr_eq(
                &closed.layout.imports[&id(&ix)].child,
                &closed.layout.imports[&id(&iy)].child
            ));
        }
    }
}

#[test]
fn extra_parent_labels_do_not_unconditionally_expand_the_childs_exception_support() {
    let mut child = PreparedEffectGraph::default();
    let x = unknown(&mut child, false, vec![]);
    let f = unknown(&mut child, true, vec![]);
    let x_row = call(&mut child, &x);
    let required = child
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            literal(&["fs.read(save)"]),
            x_row,
            &mut budget(),
        )
        .unwrap();
    child
        .add_scoped_obligation(vec![], truth(), required, &mut budget())
        .unwrap();
    let child = child.complete_scoped(&mut budget()).unwrap();
    for future_contains_extra_label in [false, true] {
        let mut parent = PreparedEffectGraph::default();
        let p = unknown(&mut parent, false, vec![]);
        let imported = parent
            .import_guarded_function(&child.unknown(&x).unwrap(), &[(&f, &p)], &mut budget())
            .unwrap();
        let value = parent
            .imported_application(&imported, vec![], &mut budget())
            .unwrap();
        // Retain an additional grant only in the receiving component's basis.
        equation(
            &mut parent,
            literal(&["fs.read"]),
            literal(&["fs.read"]),
            vec![],
        );
        let actual = call(&mut parent, &p);
        equation(
            &mut parent,
            actual,
            if future_contains_extra_label {
                literal(&["fs.read"])
            } else {
                literal(&[])
            },
            vec![],
        );
        equation(&mut parent, value, literal(&["fs.read(save)"]), vec![]);
        if future_contains_extra_label {
            // The Future adds a genuine second admissible exception. The two
            // incomparable covering rows have no pointwise least witness.
            assert!(matches!(
                parent.solve_principal(&mut budget()),
                Err(PreparedEffectFailure::Unsatisfiable)
            ));
        } else {
            parent.solve_principal(&mut budget()).unwrap();
        }
    }
}

#[test]
fn imported_function_dependencies_keep_their_order_and_local_universal_scope() {
    let mut work = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let e = parameter(&scope, 0);
    let receiving = parameter(&scope, 1);
    let mut child = PreparedEffectGraph::default();
    let x = unknown(&mut child, false, vec![e.clone()]);
    let f = unknown(&mut child, true, vec![e.clone()]);
    let left = child
        .unknown_application(&x, vec![open(&e)], &mut work)
        .unwrap();
    let right = child
        .unknown_application(&f, vec![open(&e)], &mut work)
        .unwrap();
    equation(&mut child, left, right, vec![e]);
    let child = child.complete_scoped(&mut work).unwrap();
    let mut parent = PreparedEffectGraph::default();
    let p = unknown(&mut parent, false, vec![receiving.clone()]);
    let output = unknown(&mut parent, false, vec![receiving.clone()]);
    let imported = parent
        .import_guarded_function(&child.unknown(&x).unwrap(), &[(&f, &p)], &mut work)
        .unwrap();
    let value = parent
        .imported_application(&imported, vec![open(&receiving)], &mut work)
        .unwrap();
    let output_row = parent
        .unknown_application(&output, vec![open(&receiving)], &mut work)
        .unwrap();
    equation(&mut parent, output_row, value, vec![receiving.clone()]);
    let actual = parent
        .unknown_application(&p, vec![open(&receiving)], &mut work)
        .unwrap();
    equation(
        &mut parent,
        actual,
        open(&receiving),
        vec![receiving.clone()],
    );
    let closed = parent.solve_principal(&mut work).unwrap();
    assert_eq!(
        closed.unknown(&output).unwrap(),
        &EffectRow::open(EffectSet::new(), receiving)
    );
}

#[test]
fn imported_scheme_domains_still_guard_application_admission() {
    let mut work = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let e = parameter(&scope, 0);
    let formula = EffectFormula::variable(e.clone(), &mut work).unwrap();
    let allowed =
        EffectFormula::from_set(&EffectSet::from_labels(["fs.read"]).unwrap(), &mut work).unwrap();
    let mut child = PreparedEffectGraph::default();
    let scheme = child
        .declare(
            vec![e.clone()],
            formula.subset(&allowed, &mut work).unwrap(),
            &mut work,
        )
        .unwrap();
    child
        .prepare_body(&scheme, open(&e), vec![], &mut work)
        .unwrap();
    let child = child.complete_scoped(&mut work).unwrap();
    let mut parent = PreparedEffectGraph::default();
    let imported = parent
        .import_guarded_function(&child.scheme(&scheme).unwrap(), &[], &mut work)
        .unwrap();
    let yes = parent
        .imported_application(&imported, vec![literal(&["fs.read"])], &mut work)
        .unwrap();
    let no = parent
        .imported_application(&imported, vec![literal(&["log.write"])], &mut work)
        .unwrap();
    let closed = parent.solve_principal(&mut work).unwrap();
    assert_eq!(
        closed.resolve_row(&yes, &mut work).unwrap(),
        EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap())
    );
    assert!(matches!(
        closed.resolve_row(&no, &mut work),
        Err(PreparedEffectFailure::ObservationNotTotal)
    ));
}

#[test]
fn guarded_import_preflights_identity_dependencies_and_budget_without_mutation() {
    let (child, x, f) = child_equality();
    let guarded = child.unknown(&x).unwrap();
    let mut parent = PreparedEffectGraph::default();
    let p = unknown(&mut parent, false, vec![]);
    let mut foreign = PreparedEffectGraph::default();
    let alien = unknown(&mut foreign, false, vec![]);
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let wrong_dependencies = unknown(&mut parent, false, vec![parameter(&scope, 0)]);
    let count = parent.functions.len();
    let ordinal = parent.next_function;
    assert!(matches!(
        child.unknown(&alien),
        Err(PreparedEffectInvariant::ForeignIssuer)
    ));
    assert!(matches!(
        parent.import_guarded_function(&guarded, &[(&f, &wrong_dependencies)], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ApplicationArity
        ))
    ));
    assert!(matches!(
        parent.import_guarded_function(&guarded, &[], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::UnresolvedFuture
        ))
    ));
    assert!(matches!(
        parent.import_guarded_function(&guarded, &[(&f, &alien)], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ForeignIssuer
        ))
    ));
    assert!(matches!(
        parent.import_guarded_function(&guarded, &[(&f, &p), (&f, &p)], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::DuplicateFutureBinding
        ))
    ));
    assert!(matches!(
        parent.import_guarded_function(&guarded, &[(&x, &p)], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::FutureBinding
        ))
    ));
    assert!(matches!(
        parent.import_guarded_function(&guarded, &[(&f, &p)], &mut Budget(0)),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    assert_eq!(parent.functions.len(), count);
    assert_eq!(parent.next_function, ordinal);
    let mut checkpoint = parent.checkpoint(&mut budget()).unwrap();
    let stale = parent
        .import_guarded_function(&guarded, &[(&f, &p)], &mut budget())
        .unwrap();
    parent.rollback(&mut checkpoint).unwrap();
    assert!(matches!(
        parent.imported_application(&stale, vec![], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::StaleReference
        ))
    ));
    let actual = parent
        .import_guarded_function(&guarded, &[(&f, &p)], &mut budget())
        .unwrap();
    assert_ne!(id(&stale), id(&actual));
}

#[test]
fn a_derived_import_is_not_an_extra_coordinate_in_the_parents_minimization() {
    let mut work = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let e = parameter(&scope, 0);
    let known = EffectFormula::from_set(&EffectSet::from_labels(["log.write"]).unwrap(), &mut work)
        .unwrap();
    let variable = EffectFormula::variable(e.clone(), &mut work).unwrap();
    let complement = known.difference(&variable, &mut work).unwrap();
    let mut child = PreparedEffectGraph::default();
    let x = unknown(&mut child, false, vec![]);
    let f = unknown(&mut child, true, vec![]);
    let f_row = call(&mut child, &f);
    let opposite = child
        .apply_formula(complement, BTreeMap::from([(e, f_row)]), &mut work)
        .unwrap();
    let result = call(&mut child, &x);
    equation(&mut child, result, opposite, vec![]);
    let child = child.complete_scoped(&mut work).unwrap();
    let mut parent = PreparedEffectGraph::default();
    let p = unknown(&mut parent, false, vec![]);
    let imported = parent
        .import_guarded_function(&child.unknown(&x).unwrap(), &[(&f, &p)], &mut work)
        .unwrap();
    let value = parent
        .imported_application(&imported, vec![], &mut work)
        .unwrap();
    // Minimizing (p, imported) jointly would make the two opposite tuples
    // incomparable. Only p belongs to this parent's minimization boundary.
    let closed = parent.solve_principal(&mut work).unwrap();
    assert!(closed.unknown(&p).unwrap().is_empty());
    assert_eq!(
        closed.resolve_row(&value, &mut work).unwrap(),
        EffectRow::closed(EffectSet::from_labels(["log.write"]).unwrap())
    );
}

#[test]
fn independent_future_environments_share_only_the_uninstantiated_child_completion() {
    let (child, x, f) = child_equality();
    let mut parent = PreparedEffectGraph::default();
    let mut imported = Vec::new();
    for label in ["fs.read", "log.write"] {
        let actual = unknown(&mut parent, false, vec![]);
        let function = parent
            .import_guarded_function(&child.unknown(&x).unwrap(), &[(&f, &actual)], &mut budget())
            .unwrap();
        let value = parent
            .imported_application(&function, vec![], &mut budget())
            .unwrap();
        let actual_row = call(&mut parent, &actual);
        equation(&mut parent, actual_row, literal(&[label]), vec![]);
        imported.push((function, value, label));
    }
    let closed = parent.solve_principal(&mut budget()).unwrap();
    for (_, value, label) in &imported {
        assert_eq!(
            closed.resolve_row(value, &mut budget()).unwrap(),
            EffectRow::closed(EffectSet::from_labels([*label]).unwrap())
        );
    }
    assert!(Arc::ptr_eq(
        &closed.layout.imports[&id(&imported[0].0)].child,
        &closed.layout.imports[&id(&imported[1].0)].child
    ));
}

#[test]
fn nested_frozen_components_lower_and_replay_without_recursive_graph_walks() {
    let (mut child, mut x, mut f) = child_equality();
    for _ in 0..32 {
        let mut next = PreparedEffectGraph::default();
        let future = unknown(&mut next, true, vec![]);
        let output = unknown(&mut next, false, vec![]);
        let imported = next
            .import_guarded_function(&child.unknown(&x).unwrap(), &[(&f, &future)], &mut budget())
            .unwrap();
        let value = next
            .imported_application(&imported, vec![], &mut budget())
            .unwrap();
        let result = call(&mut next, &output);
        equation(&mut next, result, value, vec![]);
        child = next.complete_scoped(&mut budget()).unwrap();
        x = output;
        f = future;
    }
    let mut parent = PreparedEffectGraph::default();
    let p = unknown(&mut parent, false, vec![]);
    let imported = parent
        .import_guarded_function(&child.unknown(&x).unwrap(), &[(&f, &p)], &mut budget())
        .unwrap();
    let value = parent
        .imported_application(&imported, vec![], &mut budget())
        .unwrap();
    let actual = call(&mut parent, &p);
    equation(&mut parent, actual, literal(&["fresh.label"]), vec![]);
    let closed = parent.solve_principal(&mut budget()).unwrap();
    assert_eq!(
        closed.resolve_row(&value, &mut budget()).unwrap(),
        EffectRow::closed(EffectSet::from_labels(["fresh.label"]).unwrap())
    );
}

#[test]
fn independent_replay_rejects_a_corrupted_child_lowering_even_if_the_parent_agrees() {
    let (child, x, f) = child_equality();
    let mut parent = PreparedEffectGraph::default();
    let p = unknown(&mut parent, false, vec![]);
    let imported = parent
        .import_guarded_function(&child.unknown(&x).unwrap(), &[(&f, &p)], &mut budget())
        .unwrap();
    let actual = call(&mut parent, &p);
    equation(&mut parent, actual, literal(&[]), vec![]);
    equation(
        &mut parent,
        literal(&["log.write"]),
        literal(&["log.write"]),
        vec![],
    );
    let mut closed = parent.solve_principal(&mut budget()).unwrap();
    let table = Arc::get_mut(&mut closed.layout)
        .unwrap()
        .imports
        .get_mut(&id(&imported))
        .unwrap();
    let cached = Arc::get_mut(&mut table.child).unwrap();
    let x_id = FunctionRef::Unknown(x).reference().ordinal;
    cached.coefficients.insert(
        Atom::Cell {
            scheme: x_id,
            class: 1,
            mask: 0,
        },
        Decision::constant(true),
    );
    table.membership[1][0] = Decision::constant(true);
    closed.functions.insert(
        id(&imported),
        EffectRow::closed(EffectSet::from_labels(["log.write"]).unwrap()),
    );
    assert!(matches!(
        closed.confirm(&mut Work(&mut budget())),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::PrincipalConfirmation
        ))
    ));
}
