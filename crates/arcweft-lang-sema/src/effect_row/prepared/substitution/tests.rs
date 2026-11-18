use super::*;
use crate::{
    effect_row::prepared::test_support::{Budget, Exhausted, budget, literal, open},
    types::{GenericBinder, GenericScope},
};

fn parameters() -> [GenericEffectReference; 2] {
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    [
        scope.bound_effect(0, 0).unwrap(),
        scope.bound_effect(0, 1).unwrap(),
    ]
}

#[test]
fn substituting_a_bare_parameter_reuses_the_existing_row() {
    let [input, _] = parameters();
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let actual = graph
        .union(vec![literal(&["log.write"])], &mut control)
        .unwrap();
    let arguments = BTreeMap::from([(input.clone(), actual.clone())]);
    let base = graph.state;

    assert_eq!(
        graph
            .substitute(&open(&input), &arguments, &mut control)
            .unwrap(),
        actual
    );
    assert_eq!(graph.state, base);

    let labeled = PreparedEffectRow::Canonical(EffectRow::open(
        crate::effects::EffectSet::from_labels(["fs.read"]).unwrap(),
        input,
    ));
    let result = graph
        .substitute(&labeled, &arguments, &mut control)
        .unwrap();
    assert_ne!(result, actual);
    let closed = graph.solve_principal(&mut control).unwrap();
    assert_eq!(
        closed.resolve_row(&result, &mut control).unwrap(),
        EffectRow::closed(
            crate::effects::EffectSet::from_labels(["fs.read", "log.write"]).unwrap()
        )
    );
}

#[test]
fn actual_replacements_are_prepared_once_and_are_not_projected_again() {
    let [input, _] = parameters();
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let function = graph
        .declare_unknown(
            vec![input.clone()],
            PreparedEffectUnknownEligibility::Bindable,
            &mut control,
        )
        .unwrap();
    let source = graph
        .union(vec![open(&input), open(&input)], &mut control)
        .unwrap();
    let mut calls = 0;
    let mut edit = graph.edit_terms();
    let row = edit
        .map_row_parameters_with_edit(&source, &mut control, &mut |parameter, edit, control| {
            assert_eq!(parameter, &input);
            calls += 1;
            edit.unknown_application(&function, vec![open(&input)], control)
                .map(Some)
        })
        .unwrap();
    edit.commit().unwrap();
    assert_eq!(calls, 1);
    let relation = graph
        .relate(
            PreparedEffectRelation::Equal,
            row,
            open(&input),
            &mut control,
        )
        .unwrap();
    graph
        .add_scoped_obligation(
            vec![input.clone()],
            PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            relation,
            &mut control,
        )
        .unwrap();
    let principal = graph.solve_principal(&mut control).unwrap();
    assert_eq!(
        principal.unknown(&function).unwrap(),
        &EffectRow::open(crate::effects::EffectSet::new(), input)
    );
}

#[test]
fn a_later_callback_failure_undoes_earlier_committed_child_edits() {
    let [first, second] = parameters();
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let function = graph
        .declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Bindable,
            &mut control,
        )
        .unwrap();
    let source = graph
        .relate(
            PreparedEffectRelation::Equal,
            open(&first),
            open(&second),
            &mut control,
        )
        .unwrap();
    let retained = graph.retained_terms();
    let mut issued = None;
    let mut calls = 0;
    let mut edit = graph.edit_terms();
    let result =
        edit.map_predicate_parameters_with_edit(&source, &mut control, &mut |_, edit, control| {
            calls += 1;
            if calls == 2 {
                return Err(PreparedEffectFailure::Control(Exhausted));
            }
            let row = edit.unknown_application(&function, vec![], control)?;
            issued = Some(row.clone());
            Ok(Some(row))
        });
    assert!(matches!(
        result,
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    assert!(issued.is_some());
    assert_eq!(edit.commit(), Err(PreparedEffectInvariant::TermEditState));
    assert_eq!(graph.retained_terms(), retained);
    assert_eq!(
        graph.read().validate_retained_row(issued.as_ref().unwrap()),
        Err(PreparedEffectInvariant::StaleReference)
    );
    graph.read().validate_retained_predicate(&source).unwrap();
    graph.read().validate_unknown(&function).unwrap();
}

#[test]
fn a_foreign_replacement_rolls_back_child_allocations_before_publication() {
    let [input, _] = parameters();
    let mut graph = PreparedEffectGraph::default();
    let mut foreign = PreparedEffectGraph::default();
    let mut control = budget();
    let foreign_row = foreign.union(vec![literal(&[])], &mut control).unwrap();
    let function = graph
        .declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Bindable,
            &mut control,
        )
        .unwrap();
    let retained = graph.retained_terms();
    let mut issued = None;
    let mut edit = graph.edit_terms();
    let result =
        edit.map_row_parameters_with_edit(&open(&input), &mut control, &mut |_, edit, control| {
            issued = Some(edit.unknown_application(&function, vec![], control)?);
            Ok(Some(foreign_row.clone()))
        });
    assert!(matches!(
        result,
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ForeignIssuer
        ))
    ));
    assert_eq!(edit.commit(), Err(PreparedEffectInvariant::TermEditState));
    assert_eq!(graph.retained_terms(), retained);
    assert_eq!(
        graph.read().validate_retained_row(issued.as_ref().unwrap()),
        Err(PreparedEffectInvariant::StaleReference)
    );
}

#[test]
fn exhausted_projection_never_enters_the_replacement_callback() {
    let [input, _] = parameters();
    let mut graph = PreparedEffectGraph::default();
    let mut edit = graph.edit_terms();
    let result =
        edit.map_row_parameters_with_edit(&open(&input), &mut Budget(0), &mut |_, _, _| {
            panic!("replacement preparation must be admitted first")
        });
    assert!(matches!(
        result,
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    assert_eq!(edit.commit(), Err(PreparedEffectInvariant::TermEditState));
    assert_eq!(graph.retained_terms(), (0, 0));
}
