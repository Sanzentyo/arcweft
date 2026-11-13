use super::tests::truth;
use super::*;
use crate::{
    effect_row::prepared::test_support::*,
    effects::EffectSet,
    types::{GenericBinder, GenericScope},
};

/// Interpret a core pointwise predicate directly on actual finite sets. The
/// all-zero default also occurs at infinitely many labels outside this union.
pub(super) fn finite_predicate(
    predicate: &EffectPredicate,
    rows: &BTreeMap<GenericEffectReference, BTreeSet<EffectId>>,
) -> bool {
    if !predicate
        .membership()
        .default_decision()
        .evaluate_with_control(|_| false, &mut budget())
        .unwrap()
    {
        return false;
    }
    let labels: BTreeSet<_> = rows
        .values()
        .flat_map(|row| row.iter())
        .chain(predicate.membership().overrides().map(|(label, _)| label))
        .collect();
    labels.into_iter().all(|label| {
        predicate
            .membership()
            .at(label)
            .evaluate_with_control(|reference| rows[reference].contains(label), &mut budget())
            .unwrap()
    })
}

#[test]
fn extension_retains_a_nonconstant_generic_condition_across_unforced_choices() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let e = parameter(&scope, 0);
    let f = parameter(&scope, 1);
    let expected = EffectFormula::variable(e, &mut control)
        .unwrap()
        .subset(
            &EffectFormula::variable(f, &mut control).unwrap(),
            &mut control,
        )
        .unwrap();
    let source = PreparedEffectPredicate::Canonical(expected.clone());
    let choice = graph.choice(&mut control).unwrap();
    let selected = graph
        .select_predicate(&choice, source.clone(), source, &mut control)
        .unwrap();
    let principal = graph.solve_principal(&mut control).unwrap();
    let observed = principal
        .observe_predicate(&truth(true), &selected, &mut control)
        .unwrap();
    assert_eq!(observed.total_truth(), None);
    let extension = observed.canonical_extension(&mut control).unwrap().unwrap();
    assert!(extension.is_total());
    assert_eq!(extension.into_total().unwrap(), expected);
}

#[test]
fn extension_retains_partial_invariance_and_never_certifies_vacuity() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let e = parameter(&scope, 0);
    let empty = graph
        .relate(
            PreparedEffectRelation::Equal,
            open(&e),
            literal(&[]),
            &mut control,
        )
        .unwrap();
    let choice = graph.choice(&mut control).unwrap();
    let selected = graph
        .select_predicate(&choice, empty, truth(false), &mut control)
        .unwrap();
    let ambiguous = graph
        .select_predicate(&choice, truth(true), truth(false), &mut control)
        .unwrap();
    let principal = graph.solve_principal(&mut control).unwrap();
    let observed = principal
        .observe_predicate(&truth(true), &selected, &mut control)
        .unwrap();
    let extension = observed.canonical_extension(&mut control).unwrap().unwrap();
    assert!(extension.predicate().is_impossible());
    assert!(!extension.is_total());
    assert!(extension.into_total().is_none());
    for (context, predicate) in [(truth(false), &selected), (truth(true), &ambiguous)] {
        let observed = principal
            .observe_predicate(&context, predicate, &mut control)
            .unwrap();
        assert!(matches!(
            observed.canonical_extension(&mut Budget(0)),
            Err(PreparedEffectFailure::Control(Exhausted))
        ));
        assert!(
            observed
                .canonical_extension(&mut control)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn extension_preserves_named_and_outside_conditions_against_finite_sets() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let e = parameter(&scope, 0);
    let permitted = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            open(&e),
            literal(&["fs.read", "log.write"]),
            &mut control,
        )
        .unwrap();
    let required = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            literal(&["fs.read"]),
            open(&e),
            &mut control,
        )
        .unwrap();
    let predicate = graph.and(vec![permitted, required], &mut control).unwrap();
    let principal = graph.solve_principal(&mut control).unwrap();
    let observation = principal
        .observe_predicate(&truth(true), &predicate, &mut control)
        .unwrap();
    let extension = observation
        .canonical_extension(&mut control)
        .unwrap()
        .unwrap()
        .into_total()
        .unwrap();
    let universe =
        ["fs.read", "log.write", "fresh.first"].map(|label| EffectId::parse(label).unwrap());
    for mask in 0..8 {
        let set: BTreeSet<_> = universe
            .iter()
            .enumerate()
            .filter(|(slot, _)| mask & (1usize << slot) != 0)
            .map(|(_, label)| label.clone())
            .collect();
        let expected = set.contains(&universe[0]) && !set.contains(&universe[2]);
        assert_eq!(
            finite_predicate(&extension, &BTreeMap::from([(e.clone(), set)])),
            expected
        );
    }
}

#[test]
fn extension_keeps_its_context_guard_when_a_global_condition_becomes_local() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let e = parameter(&scope, 0);
    let f = parameter(&scope, 1);
    let subset = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            open(&e),
            open(&f),
            &mut control,
        )
        .unwrap();
    let superset = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            open(&f),
            open(&e),
            &mut control,
        )
        .unwrap();
    let comparable = graph
        .or(vec![subset.clone(), superset], &mut control)
        .unwrap();
    let principal = graph.solve_principal(&mut control).unwrap();
    let observation = principal
        .observe_predicate(&subset, &comparable, &mut control)
        .unwrap();
    let extension = observation
        .canonical_extension(&mut control)
        .unwrap()
        .unwrap();
    assert!(!extension.is_total());
    assert!(extension.into_total().is_none());
}

#[test]
fn extension_has_an_exact_inclusive_budget_without_mutating_the_graph() {
    let graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let e = parameter(&scope, 0);
    let source = EffectFormula::variable(e, &mut control)
        .unwrap()
        .subset(
            &EffectFormula::from_set(&EffectSet::from_labels(["fs.read"]).unwrap(), &mut control)
                .unwrap(),
            &mut control,
        )
        .unwrap();
    let principal = graph.solve_principal(&mut control).unwrap();
    let observed = principal
        .observe_predicate(
            &truth(true),
            &PreparedEffectPredicate::Canonical(source.clone()),
            &mut control,
        )
        .unwrap();
    let mut measured = budget();
    let before = measured.0;
    observed
        .canonical_extension(&mut measured)
        .unwrap()
        .unwrap();
    let cost = before - measured.0;
    assert!(cost > 0);
    assert!(matches!(
        observed.canonical_extension(&mut Budget(cost - 1)),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    assert_eq!(
        observed
            .canonical_extension(&mut Budget(cost))
            .unwrap()
            .unwrap()
            .into_total()
            .unwrap(),
        source
    );
    assert_eq!(graph.retained_terms(), (0, 0));
    assert!(graph.functions.is_empty());
}
