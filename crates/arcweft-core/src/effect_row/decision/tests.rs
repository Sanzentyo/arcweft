use super::*;

#[derive(Default)]
struct Meter {
    used: usize,
    limit: Option<usize>,
}

impl DecisionControl for Meter {
    type Error = (usize, usize);

    fn charge(&mut self, _work: DecisionWork) -> Result<(), Self::Error> {
        self.used += 1;
        if let Some(limit) = self.limit
            && self.used > limit
        {
            return Err((self.used, limit));
        }
        Ok(())
    }
}

fn atom(variable: u8) -> EffectDecision<u8> {
    EffectDecision::variable(variable, &mut Meter::default()).unwrap()
}

fn truth_table(decision: &EffectDecision<u8>, variables: u8) -> Vec<bool> {
    (0..1u64 << variables)
        .map(|bits| decision.evaluate(|variable| bits & (1 << variable) != 0))
        .collect()
}

#[test]
fn boolean_construction_is_extensional_and_canonical() {
    let (a, b, c) = (atom(0), atom(1), atom(2));
    let mut meter = Meter::default();
    let left = a.and(&b.or(&c, &mut meter).unwrap(), &mut meter).unwrap();
    let right = a
        .and(&b, &mut meter)
        .unwrap()
        .or(&c.and(&a, &mut meter).unwrap(), &mut meter)
        .unwrap();
    assert_eq!(left, right);
    assert_eq!(
        truth_table(&left, 3),
        (0..8)
            .map(|bits| bits & 1 != 0 && bits & 6 != 0)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        left.variables()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    let negated = left.not(&mut meter).unwrap();
    assert_eq!(
        truth_table(&negated, 3),
        truth_table(&left, 3)
            .into_iter()
            .map(|value| !value)
            .collect::<Vec<_>>()
    );
    assert!(left.or(&negated, &mut meter).unwrap().is_constant(true));
    assert!(left.and(&negated, &mut meter).unwrap().is_constant(false));
}

#[test]
fn existential_projection_preserves_its_full_admissibility_domain() {
    let mut meter = Meter::default();
    let a = atom(0);
    let b = atom(1);
    let equal = a
        .conditional(&b, &b.not(&mut meter).unwrap(), &mut meter)
        .unwrap();
    assert!(
        equal
            .exists(&BTreeSet::from([1]), &mut meter)
            .unwrap()
            .is_constant(true)
    );
    let constrained = equal.and(&a, &mut meter).unwrap();
    assert_eq!(
        constrained
            .exists(&BTreeSet::from([1]), &mut meter)
            .unwrap(),
        a
    );
}

#[test]
fn substitutions_are_simultaneous_and_reorder_decisions() {
    let mut meter = Meter::default();
    let first = atom(0);
    let second = atom(1);
    let relation = first
        .and(&second.not(&mut meter).unwrap(), &mut meter)
        .unwrap();
    let replacements = BTreeMap::from([(0, second.clone()), (1, first.clone())]);
    let swapped = relation.substitute(&replacements, &mut meter).unwrap();
    assert_eq!(
        swapped,
        second
            .and(&first.not(&mut meter).unwrap(), &mut meter)
            .unwrap()
    );
    assert_eq!(truth_table(&swapped, 2), [false, false, true, false]);
    assert_eq!(first.substitute(&replacements, &mut meter).unwrap(), second);
}

#[test]
fn least_completion_can_require_difference_of_rigid_rows() {
    let mut meter = Meter::default();
    let (p, q, e) = (atom(0), atom(1), atom(2));
    let relation = p
        .not(&mut meter)
        .unwrap()
        .or(&q.or(&e, &mut meter).unwrap(), &mut meter)
        .unwrap();
    let completed = relation.complete(&BTreeSet::from([2]), &mut meter).unwrap();
    assert!(completed.admissibility.is_constant(true));
    let expected = p.and(&q.not(&mut meter).unwrap(), &mut meter).unwrap();
    assert_eq!(completed.least.unwrap(), BTreeMap::from([(2, expected)]));
}

#[test]
fn independent_witnesses_are_not_chosen_by_iteration_order() {
    let mut meter = Meter::default();
    let (p, a, b) = (atom(0), atom(1), atom(2));
    let relation = p
        .not(&mut meter)
        .unwrap()
        .or(&a.or(&b, &mut meter).unwrap(), &mut meter)
        .unwrap();
    let completed = relation
        .complete(&BTreeSet::from([1, 2]), &mut meter)
        .unwrap();
    assert!(completed.admissibility.is_constant(true));
    assert!(
        completed.least.is_none(),
        "either witness alone satisfies p, but their intersection does not"
    );
    let required_first = relation
        .and(
            &p.conditional(&a, &EffectDecision::constant(true), &mut meter)
                .unwrap(),
            &mut meter,
        )
        .unwrap();
    let completed = required_first
        .complete(&BTreeSet::from([1, 2]), &mut meter)
        .unwrap();
    assert_eq!(
        completed.least.unwrap(),
        BTreeMap::from([(1, p), (2, EffectDecision::constant(false))])
    );
}

#[test]
fn recursive_equations_have_the_least_fixed_point() {
    let mut meter = Meter::default();
    let (p, a, b) = (atom(0), atom(1), atom(2));
    let p_into_a = p
        .conditional(&a, &EffectDecision::constant(true), &mut meter)
        .unwrap();
    let a_into_b = a
        .conditional(&b, &EffectDecision::constant(true), &mut meter)
        .unwrap();
    let b_into_a = b
        .conditional(&a, &EffectDecision::constant(true), &mut meter)
        .unwrap();
    let relation = p_into_a
        .and(&a_into_b, &mut meter)
        .unwrap()
        .and(&b_into_a, &mut meter)
        .unwrap();
    let completed = relation
        .complete(&BTreeSet::from([1, 2]), &mut meter)
        .unwrap();
    assert!(completed.admissibility.is_constant(true));
    assert_eq!(
        completed.least.unwrap(),
        BTreeMap::from([(1, p.clone()), (2, p)])
    );
}

#[test]
fn completion_matches_exhaustive_models_of_every_three_variable_relation() {
    let mut meter = Meter::default();
    let atoms = [atom(0), atom(1), atom(2)];
    for table in 0u16..256 {
        let mut relation = EffectDecision::constant(false);
        for valuation in 0..8 {
            if table & (1 << valuation) == 0 {
                continue;
            }
            let mut term = EffectDecision::constant(true);
            for (variable, atom) in atoms.iter().enumerate() {
                let literal = if valuation & (1 << variable) == 0 {
                    atom.not(&mut meter).unwrap()
                } else {
                    atom.clone()
                };
                term = term.and(&literal, &mut meter).unwrap();
            }
            relation = relation.or(&term, &mut meter).unwrap();
        }
        for quantified_mask in 0..8u8 {
            let quantified = (0..3u8)
                .filter(|variable| quantified_mask & (1 << variable) != 0)
                .collect::<BTreeSet<_>>();
            let completed = relation.complete(&quantified, &mut meter).unwrap();
            assert!(
                completed
                    .admissibility
                    .variables()
                    .all(|variable| !quantified.contains(variable))
            );
            let mut least_models = [0u8; 8];
            let mut has_least = true;
            for rigid in (0..8u8).filter(|rigid| rigid & quantified_mask == 0) {
                let models = (0..8u8)
                    .filter(|witness| {
                        witness & !quantified_mask == 0 && table & (1 << (rigid | witness)) != 0
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    completed
                        .admissibility
                        .evaluate(|variable| rigid & (1 << variable) != 0),
                    !models.is_empty(),
                    "table {table}, mask {quantified_mask}, rigid {rigid}"
                );
                if let Some((&first, rest)) = models.split_first() {
                    let least = rest.iter().fold(first, |meet, next| meet & next);
                    least_models[usize::from(rigid)] = least;
                    has_least &= models.contains(&least);
                }
            }
            assert_eq!(
                completed.least.is_some(),
                has_least,
                "table {table}, mask {quantified_mask}"
            );
            if let Some(least) = completed.least {
                assert_eq!(least.keys().copied().collect::<BTreeSet<_>>(), quantified);
                for rigid in (0..8u8).filter(|rigid| rigid & quantified_mask == 0) {
                    for variable in &quantified {
                        assert!(
                            least[variable]
                                .variables()
                                .all(|variable| !quantified.contains(variable))
                        );
                        assert_eq!(
                            least[variable].evaluate(|variable| rigid & (1 << variable) != 0),
                            least_models[usize::from(rigid)] & (1 << variable) != 0,
                            "table {table}, mask {quantified_mask}, rigid {rigid}, variable {variable}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn every_budget_boundary_aborts_without_altering_inputs() {
    let mut meter = Meter::default();
    let relation = atom(0).or(&atom(1), &mut meter).unwrap();
    let before = relation.clone();
    let quantified = BTreeSet::from([1]);
    let mut full = Meter::default();
    let expected = relation.complete(&quantified, &mut full).unwrap();
    for limit in 0..full.used {
        let mut limited = Meter {
            used: 0,
            limit: Some(limit),
        };
        assert_eq!(
            relation.complete(&quantified, &mut limited).unwrap_err(),
            (limit + 1, limit)
        );
        assert_eq!(relation, before);
    }
    let mut exact = Meter {
        used: 0,
        limit: Some(full.used),
    };
    assert_eq!(
        relation.complete(&quantified, &mut exact).unwrap(),
        expected
    );
}
