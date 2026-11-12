use super::*;
use crate::{
    effect_row::prepared::test_support::*,
    effects::EffectSet,
    types::{GenericBinder, GenericScope},
};

pub(super) fn truth(value: bool) -> PreparedEffectPredicate {
    PreparedEffectPredicate::Canonical(if value {
        EffectPredicate::unconstrained()
    } else {
        EffectPredicate::impossible()
    })
}

fn assert_only_region(observation: &EffectRowObservation<'_>, expected: EffectRowRegion) {
    for region in [
        EffectRowRegion::Vacuous,
        EffectRowRegion::Rejected,
        EffectRowRegion::MixedAdmission,
        EffectRowRegion::Ambiguous,
        EffectRowRegion::Invariant,
    ] {
        assert_eq!(observation.region_is_universal(region), region == expected);
        assert_eq!(observation.region_is_empty(region), region != expected);
    }
}

#[test]
fn vacuity_rejection_mixed_admission_and_row_ambiguity_are_distinct() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let never = graph
        .declare(vec![], EffectPredicate::impossible(), &mut control)
        .unwrap();
    graph
        .prepare_body(&never, literal(&[]), vec![], &mut control)
        .unwrap();
    let rejected = graph.application(&never, vec![], &mut control).unwrap();
    let choice = graph.choice(&mut control).unwrap();
    let mixed = graph
        .select_row(&choice, literal(&[]), rejected.clone(), &mut control)
        .unwrap();
    let ambiguous = graph
        .select_row(&choice, literal(&["fs.read"]), literal(&[]), &mut control)
        .unwrap();
    let principal = graph.solve_principal(&mut control).unwrap();
    for (context, row, expected) in [
        (truth(false), literal(&[]), EffectRowRegion::Vacuous),
        (truth(true), rejected, EffectRowRegion::Rejected),
        (truth(true), mixed, EffectRowRegion::MixedAdmission),
        (truth(true), ambiguous, EffectRowRegion::Ambiguous),
        (truth(true), literal(&[]), EffectRowRegion::Invariant),
    ] {
        let observation = principal.observe_row(&context, &row, &mut control).unwrap();
        assert_only_region(&observation, expected);
        assert!(matches!(
            observation.canonical_extension(&mut Budget(0)),
            Err(PreparedEffectFailure::Control(Exhausted))
        ));
        let canonical = observation.canonical_extension(&mut control).unwrap();
        assert_eq!(canonical.is_some(), expected == EffectRowRegion::Invariant);
        assert_eq!(
            principal.resolve_row(&row, &mut control).is_ok(),
            expected == EffectRowRegion::Invariant || expected == EffectRowRegion::Vacuous
        );
    }
}

#[test]
fn later_use_observes_the_least_effects_without_promoting_another_witness() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scheme = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    let recursive = graph.application(&scheme, vec![], &mut control).unwrap();
    let choice = graph.choice(&mut control).unwrap();
    let body = graph
        .select_row(&choice, literal(&["fs.read"]), recursive, &mut control)
        .unwrap();
    graph
        .prepare_body(&scheme, body, vec![], &mut control)
        .unwrap();
    let needs_other_witness = graph
        .select_predicate(&choice, truth(true), truth(false), &mut control)
        .unwrap();
    let principal = graph.solve_principal(&mut control).unwrap();
    assert_eq!(
        principal.scheme(&scheme).unwrap(),
        &EffectRow::closed(EffectSet::new())
    );
    assert_eq!(
        principal
            .observe_predicate(&truth(true), &needs_other_witness, &mut control)
            .unwrap()
            .total_truth(),
        Some(false)
    );
    let scoped = principal
        .observe_row(&needs_other_witness, &literal(&[]), &mut control)
        .unwrap();
    assert_only_region(&scoped, EffectRowRegion::Vacuous);
    assert!(scoped.canonical_extension(&mut control).unwrap().is_none());
    assert_eq!(
        principal.scheme(&scheme).unwrap(),
        &EffectRow::closed(EffectSet::new())
    );
}

#[test]
fn a_witness_invariant_row_can_still_depend_on_another_labels_membership() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let e = parameter(&scope, 0);
    let choice = graph.choice(&mut control).unwrap();
    let bit = graph
        .select_predicate(&choice, truth(true), truth(false), &mut control)
        .unwrap();
    let contains_log = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            literal(&["log.write"]),
            open(&e),
            &mut control,
        )
        .unwrap();
    let yes = graph
        .and(vec![bit.clone(), contains_log.clone()], &mut control)
        .unwrap();
    let not_bit = graph.not(bit, &mut control).unwrap();
    let not_log = graph.not(contains_log, &mut control).unwrap();
    let no = graph.and(vec![not_bit, not_log], &mut control).unwrap();
    let context = graph.or(vec![yes, no], &mut control).unwrap();
    let row = graph
        .select_row(&choice, literal(&["fs.read"]), literal(&[]), &mut control)
        .unwrap();
    let principal = graph.solve_principal(&mut control).unwrap();
    let observation = principal.observe_row(&context, &row, &mut control).unwrap();
    assert_only_region(&observation, EffectRowRegion::Invariant);
    assert!(matches!(
        observation.canonical_extension(&mut control),
        Err(PreparedEffectFailure::NonPointwiseObservation)
    ));
}

/// Independent finite-set interpretation. Presence is computed from actual
/// distinct labels, without calling the solver's folding or substitution code.
fn whole_value(decision: &Decision, labels: &[EffectId], rows: &[BTreeSet<EffectId>]) -> bool {
    decision
        .evaluate_with_control(
            |atom| match atom {
                Atom::Formal {
                    owner: FormalOwner::Observation,
                    class,
                    parameter,
                } => {
                    assert_ne!(
                        *class, 0,
                        "a whole-row guard retained a distinguished probe"
                    );
                    rows[*parameter].contains(&labels[*class - 1])
                }
                Atom::Presence {
                    owner: FormalOwner::Observation,
                    mask,
                } => rows
                    .iter()
                    .flat_map(|row| row.iter())
                    .filter(|label| !labels.contains(label))
                    .any(|label| {
                        rows.iter().enumerate().fold(0, |bits, (slot, row)| {
                            bits | (usize::from(row.contains(label)) << slot)
                        }) == *mask
                    }),
                other => panic!("unclosed whole-row guard: {other:?}"),
            },
            &mut budget(),
        )
        .unwrap()
}

#[test]
fn global_boolean_guards_match_exhaustive_finite_sets_with_distinct_unnamed_labels() {
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
        .or(vec![subset.clone(), superset.clone()], &mut control)
        .unwrap();
    let incomparable = graph.not(comparable.clone(), &mut control).unwrap();
    let equal = graph.and(vec![subset, superset], &mut control).unwrap();
    let principal = graph.solve_principal(&mut control).unwrap();
    let observed = [&comparable, &incomparable, &equal].map(|predicate| {
        principal
            .observe_predicate(&truth(true), predicate, &mut control)
            .unwrap()
    });
    for observation in &observed[..2] {
        assert!(matches!(
            observation.canonical_extension(&mut control),
            Err(PreparedEffectFailure::NonPointwiseObservation)
        ));
    }
    let extension = observed[2]
        .canonical_extension(&mut control)
        .unwrap()
        .unwrap();
    assert!(extension.is_total());
    let universe =
        ["fresh.first", "fresh.second", "fresh.third"].map(|label| EffectId::parse(label).unwrap());
    let sets: Vec<BTreeSet<EffectId>> = (0..8)
        .map(|bits| {
            universe
                .iter()
                .enumerate()
                .filter(|(slot, _)| bits & (1usize << slot) != 0)
                .map(|(_, label)| label.clone())
                .collect()
        })
        .collect();
    let mut checked = 0;
    for left in &sets {
        for right in &sets {
            let comparable = left.is_subset(right) || right.is_subset(left);
            for (observation, expected) in
                observed
                    .iter()
                    .zip([comparable, !comparable, left == right])
            {
                let rows = [left.clone(), right.clone()];
                assert_eq!(whole_value(&observation.yes, &[], &rows), expected);
                assert_eq!(whole_value(&observation.no, &[], &rows), !expected);
                assert!(observation.region_is_empty(EffectPredicateRegion::Vacuous));
                assert!(observation.region_is_empty(EffectPredicateRegion::Ambiguous));
            }
            assert_eq!(
                super::predicate_tests::finite_predicate(
                    extension.predicate(),
                    &BTreeMap::from([(e.clone(), left.clone()), (f.clone(), right.clone())])
                ),
                left == right
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 64);
    // Two disjoint singleton rows are the smallest counterexample to folding
    // the outer disjunction at one probe rather than over complete finite rows.
    assert!(!whole_value(
        &observed[0].yes,
        &[],
        &[sets[1].clone(), sets[2].clone()]
    ));
    drop(principal);
    let scheme = graph
        .declare(vec![e, f], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    graph
        .prepare_body(&scheme, literal(&[]), vec![comparable], &mut control)
        .unwrap();
    assert!(matches!(
        graph.solve_principal(&mut control),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
}

#[test]
fn default_class_ambiguity_is_a_whole_row_property_even_at_an_absent_probe() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let e = parameter(&scope, 0);
    let choice = graph.choice(&mut control).unwrap();
    let row = graph
        .select_row(&choice, open(&e), literal(&[]), &mut control)
        .unwrap();
    let principal = graph.solve_principal(&mut control).unwrap();
    let observation = principal
        .observe_row(&truth(true), &row, &mut control)
        .unwrap();
    assert!(observation.layout.labels.is_empty());
    for rows in [
        BTreeSet::new(),
        BTreeSet::from([EffectId::parse("fresh.first").unwrap()]),
    ] {
        assert_eq!(
            whole_value(&observation.regions.ambiguous, &[], &[rows.clone()]),
            !rows.is_empty()
        );
        assert_eq!(
            whole_value(&observation.regions.invariant, &[], &[rows.clone()]),
            rows.is_empty()
        );
    }
    let extension = observation
        .canonical_extension(&mut control)
        .unwrap()
        .unwrap();
    assert_eq!(extension.row(), &EffectRow::closed(EffectSet::new()));
    assert!(!extension.is_total());
    assert!(extension.into_total().is_none());
}

#[test]
fn application_domains_use_the_callers_presence_profile_after_substitution() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let e = parameter(&scope, 0);
    let f = parameter(&scope, 1);
    let ef = EffectFormula::variable(e.clone(), &mut control).unwrap();
    let ff = EffectFormula::variable(f.clone(), &mut control).unwrap();
    let domain = ef
        .subset(&ff, &mut control)
        .unwrap()
        .and(&ff.subset(&ef, &mut control).unwrap(), &mut control)
        .unwrap();
    let callee = graph
        .declare(vec![e.clone(), f], domain, &mut control)
        .unwrap();
    graph
        .prepare_body(&callee, literal(&[]), vec![], &mut control)
        .unwrap();
    let caller = graph
        .declare(
            vec![e.clone()],
            EffectPredicate::unconstrained(),
            &mut control,
        )
        .unwrap();
    let body = graph
        .application(&callee, vec![open(&e), open(&e)], &mut control)
        .unwrap();
    graph
        .prepare_body(&caller, body.clone(), vec![], &mut control)
        .unwrap();
    let principal = graph.solve_principal(&mut control).unwrap();
    assert_eq!(
        principal.scheme(&caller).unwrap(),
        &EffectRow::closed(EffectSet::new())
    );
    assert_eq!(
        principal.resolve_row(&body, &mut control).unwrap(),
        EffectRow::closed(EffectSet::new())
    );
}

#[test]
fn observation_can_extend_support_without_changing_the_least_scheme_tables() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let e = parameter(&scope, 0);
    let scheme = graph
        .declare(
            vec![e.clone()],
            EffectPredicate::unconstrained(),
            &mut control,
        )
        .unwrap();
    graph
        .prepare_body(&scheme, open(&e), vec![], &mut control)
        .unwrap();
    let call = graph
        .application(&scheme, vec![open(&e)], &mut control)
        .unwrap();
    let principal = graph.solve_principal(&mut control).unwrap();
    assert!(principal.layout.labels.is_empty());
    let query_label = EffectSet::from_labels(["fs.read"]).unwrap();
    let contains = EffectFormula::from_set(&query_label, &mut control)
        .unwrap()
        .subset(
            &EffectFormula::variable(e.clone(), &mut control).unwrap(),
            &mut control,
        )
        .unwrap();
    let observation = principal
        .observe_row(
            &PreparedEffectPredicate::Canonical(contains),
            &call,
            &mut control,
        )
        .unwrap();
    assert_eq!(
        observation.layout.labels.as_ref(),
        query_label.iter().cloned().collect::<Vec<_>>()
    );
    let extension = observation
        .canonical_extension(&mut control)
        .unwrap()
        .unwrap();
    let expected = EffectRow::open(EffectSet::new(), e);
    assert_eq!(extension.row(), &expected);
    assert!(!extension.is_total());
    assert!(principal.layout.labels.is_empty());
    assert_eq!(principal.scheme(&scheme).unwrap(), &expected);
}

#[test]
fn failed_observation_preserves_the_graph_and_has_an_exact_inclusive_work_boundary() {
    let graph = PreparedEffectGraph::default();
    let principal = graph.solve_principal(&mut budget()).unwrap();
    let context = truth(true);
    let row = literal(&["fs.read"]);
    let mut measured = budget();
    let initial = measured.0;
    let observation = principal
        .observe_row(&context, &row, &mut measured)
        .unwrap();
    let cost = initial - measured.0;
    assert!(cost > 0);
    assert!(matches!(
        principal.observe_row(&context, &row, &mut Budget(cost - 1)),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    assert_only_region(
        &principal
            .observe_row(&context, &row, &mut Budget(cost))
            .unwrap(),
        EffectRowRegion::Invariant,
    );
    let mut measured = budget();
    let initial = measured.0;
    observation
        .canonical_extension(&mut measured)
        .unwrap()
        .unwrap();
    let cost = initial - measured.0;
    assert!(cost > 0);
    assert!(matches!(
        observation.canonical_extension(&mut Budget(cost - 1)),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    assert_eq!(
        observation
            .canonical_extension(&mut Budget(cost))
            .unwrap()
            .unwrap()
            .row(),
        &EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap())
    );
    assert_eq!(graph.retained_terms(), (0, 0));
    assert!(graph.schemes.is_empty());
    assert!(graph.choices.is_empty());
}

#[test]
fn observation_rejects_foreign_and_rolled_back_terms_without_a_fallback_value() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let mut checkpoint = graph.checkpoint(&mut control).unwrap();
    let stale = graph.union(vec![literal(&[])], &mut control).unwrap();
    graph.rollback(&mut checkpoint).unwrap();
    let mut other = PreparedEffectGraph::default();
    let foreign = other.union(vec![literal(&[])], &mut control).unwrap();
    let principal = graph.solve_principal(&mut control).unwrap();
    for (row, expected) in [
        (stale, PreparedEffectInvariant::StaleReference),
        (foreign, PreparedEffectInvariant::ForeignIssuer),
    ] {
        match principal.observe_row(&truth(true), &row, &mut control) {
            Err(PreparedEffectFailure::Invariant(actual)) => assert_eq!(actual, expected),
            _ => panic!("invalid authority must fail without an observation"),
        }
    }
    assert_eq!(graph.retained_terms(), (0, 0));
}

#[test]
fn predicate_negation_is_total_even_when_an_atomic_relation_is_inapplicable() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let never = graph
        .declare(vec![], EffectPredicate::impossible(), &mut control)
        .unwrap();
    graph
        .prepare_body(&never, literal(&[]), vec![], &mut control)
        .unwrap();
    let invalid = graph.application(&never, vec![], &mut control).unwrap();
    let relation = graph
        .relate(
            PreparedEffectRelation::Equal,
            invalid,
            literal(&[]),
            &mut control,
        )
        .unwrap();
    let negated = graph.not(relation.clone(), &mut control).unwrap();
    let principal = graph.solve_principal(&mut control).unwrap();
    for (predicate, expected) in [(relation, false), (negated, true)] {
        assert_eq!(
            principal
                .observe_predicate(&truth(true), &predicate, &mut control)
                .unwrap()
                .total_truth(),
            Some(expected)
        );
    }
}
