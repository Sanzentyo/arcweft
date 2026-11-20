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
fn joint_equalities_keep_shared_coefficients_and_constants() {
    use EffectDecisionReplacement::{Constant, Decision, Variable};

    let coefficient = atom(1).or(&atom(2), &mut Meter::default()).unwrap();
    let cases = || {
        [
            Ok([Variable(0), Decision(&coefficient)]),
            Ok([Variable(1), Constant(false)]),
            Ok([Variable(2), Variable(0)]),
        ]
    };
    let mut measured = Meter::default();
    let decision = EffectDecision::all_equalities(cases(), &mut measured).unwrap();
    assert_eq!(
        truth_table(&decision, 3),
        [true, false, false, false, false, true, false, false]
    );
    let mut short = Meter {
        used: 0,
        limit: Some(measured.used - 1),
    };
    assert_eq!(
        EffectDecision::all_equalities(cases(), &mut short),
        Err((measured.used, measured.used - 1))
    );
    let guarded = EffectDecision::all_equalities_under(
        [Ok([Variable(1), Constant(false)])],
        cases(),
        &mut Meter::default(),
    )
    .unwrap();
    assert_eq!(
        truth_table(&guarded, 3),
        [true, false, true, true, false, true, true, true]
    );
}

#[test]
fn finalization_emits_only_reachable_branches_in_low_before_high_order() {
    let mut work = Meter::default();
    let mut builder = DecisionBuilder::new(&mut work);
    let shared = builder.branch(3, Root::False, Root::True).unwrap();
    let high = builder.branch(2, Root::True, shared).unwrap();
    builder.branch(4, Root::False, Root::True).unwrap();
    let low = builder.branch(1, shared, Root::False).unwrap();
    let root = builder.branch(0, low, high).unwrap();
    let expected = EffectDecision {
        nodes: Box::new([
            Node {
                variable: 3,
                low: Root::False,
                high: Root::True,
            },
            Node {
                variable: 1,
                low: Root::Branch(0),
                high: Root::False,
            },
            Node {
                variable: 2,
                low: Root::True,
                high: Root::Branch(0),
            },
            Node {
                variable: 0,
                low: Root::Branch(1),
                high: Root::Branch(2),
            },
        ]),
        root: Root::Branch(3),
    };
    let required = 2 * expected.node_count();
    for limit in 0..required {
        *builder.control = Meter {
            used: 0,
            limit: Some(limit),
        };
        assert_eq!(builder.finish(root), Err((limit + 1, limit)));
    }
    *builder.control = Meter {
        used: 0,
        limit: Some(required),
    };
    assert_eq!(builder.finish(root).unwrap(), expected);
    assert_eq!(
        builder.finish(Root::False).unwrap(),
        EffectDecision::constant(false)
    );
    assert_eq!(builder.control.used, required);
}

#[test]
fn reference_mapping_builds_ordered_branches_directly_and_normalizes_other_mappings() {
    let mut setup = Meter::default();
    let source = atom(0).conditional(&atom(1), &atom(2), &mut setup).unwrap();
    let expected = atom(3).conditional(&atom(4), &atom(5), &mut setup).unwrap();
    let required = 3 * source.node_count();
    let mut short = Meter {
        used: 0,
        limit: Some(required - 1),
    };
    let mut exact = Meter {
        used: 0,
        limit: Some(required),
    };
    assert_eq!(
        source.map_references(&mut short, &mut |variable, _| Ok(variable + 3)),
        Err((required, required - 1)),
    );
    assert_eq!(
        source.map_references(&mut exact, &mut |variable, _| Ok(variable + 3)),
        Ok(expected),
    );
    assert_eq!(exact.used, required);
    assert_eq!(
        source.map_references(&mut setup, &mut |variable, _| Ok(5 - variable)),
        atom(5).conditional(&atom(4), &atom(3), &mut Meter::default()),
    );
    assert_eq!(
        source.map_references(&mut setup, &mut |variable, _| Ok((*variable == 2) as u8)),
        atom(0).or(&atom(1), &mut Meter::default()),
    );
}

#[test]
fn rebuilding_a_decision_rechecks_order_when_existing_indices_gain_different_edges() {
    let mut work = Meter::default();
    let source = atom(0).conditional(&atom(1), &atom(2), &mut work).unwrap();
    let expected = atom(0).conditional(&atom(2), &atom(1), &mut work).unwrap();
    let rebuilt = source
        .rebuild_nodes(&mut work, |&variable, low, high, builder| {
            if variable == 0 {
                builder.branch(variable, high, low)
            } else {
                builder.branch(variable, low, high)
            }
        })
        .unwrap();
    assert_eq!(rebuilt, expected);
}

#[test]
fn table_selection_preserves_correlated_selectors_and_entry_references() {
    let mut work = Meter::default();
    let bit = atom(0);
    let borrowed = atom(1);
    let result = EffectDecision::select_table(
        2,
        4,
        &mut work,
        |_| EffectDecisionReplacement::Decision(&bit),
        |index, _| {
            Ok(match index {
                0 => EffectDecisionReplacement::Decision(&borrowed),
                3 => EffectDecisionReplacement::Variable(2),
                _ => EffectDecisionReplacement::Constant(true),
            })
        },
    )
    .unwrap()
    .unwrap();
    let expected = bit.conditional(&atom(2), &borrowed, &mut work).unwrap();
    assert_eq!(result, expected);
    let invalid = EffectDecision::<u8>::select_table(
        2,
        3,
        &mut work,
        |_| panic!("invalid table must not resolve selectors"),
        |_, _| panic!("invalid table must not resolve entries"),
    );
    assert_eq!(invalid.unwrap(), None);
}

#[test]
fn truth_table_uses_the_same_work_for_permuted_literal_selectors() {
    let mut ordered = Meter::default();
    let expected = EffectDecision::select_table(
        3,
        8,
        &mut ordered,
        |bit| EffectDecisionReplacement::Variable(bit as u8),
        |mask, _| Ok(EffectDecisionReplacement::Variable(8 + mask as u8)),
    )
    .unwrap()
    .unwrap();

    let permutation = [2, 0, 1];
    let selectors = permutation.map(atom);
    let mut work = Meter::default();
    let selected = EffectDecision::select_table(
        3,
        8,
        &mut work,
        |bit| EffectDecisionReplacement::Decision(&selectors[bit]),
        |mask, _| {
            let value = permutation
                .iter()
                .enumerate()
                .map(|(bit, reference)| ((mask as u8 >> bit) & 1) << reference)
                .sum::<u8>();
            Ok(EffectDecisionReplacement::Variable(8 + value))
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(selected, expected);
    assert_eq!(work.used, ordered.used);
    // Eight coefficient nodes and seven output branches. Selector literals
    // are read as descriptors and create no standalone decision nodes.
    assert_eq!(work.used, 71);
    for mask in 0..8u8 {
        for cell in 0..8u8 {
            let value = selected.evaluate(|&reference| {
                if reference < 3 {
                    mask & (1 << reference) != 0
                } else {
                    reference == 8 + cell
                }
            });
            assert_eq!(value, mask == cell, "mask {mask}, coefficient {cell}");
        }
    }
}

#[test]
fn table_selection_interns_a_repeated_literal_result_once() {
    let mut work = Meter::default();
    let result = EffectDecision::select_table(
        2,
        4,
        &mut work,
        |bit| EffectDecisionReplacement::Variable(bit as u8),
        |mask, _| Ok(EffectDecisionReplacement::Constant(mask & 2 != 0)),
    )
    .unwrap()
    .unwrap();
    assert_eq!(result, atom(1));
    assert_eq!(work.used, 12);
}

#[test]
fn table_selection_handles_negated_literals_with_and_without_shared_references() {
    let mut work = Meter::default();
    let bit = atom(0);
    let negated = bit.not(&mut work).unwrap();
    let coefficient = atom(1);
    for high in [&coefficient, &bit] {
        let selected = EffectDecision::select_table(
            1,
            2,
            &mut work,
            |_| EffectDecisionReplacement::Decision(&negated),
            |mask, _| {
                Ok(if mask == 0 {
                    EffectDecisionReplacement::Constant(true)
                } else {
                    EffectDecisionReplacement::Decision(high)
                })
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(selected, bit.or(high, &mut work).unwrap());
    }
}

#[test]
fn uniform_truth_table_reads_all_entries_without_loading_selectors() {
    let mut seen = Vec::new();
    let selected = EffectDecision::<u8>::select_table(
        3,
        8,
        &mut Meter::default(),
        |_| panic!("uniform table does not use selectors"),
        |mask, _| {
            seen.push(mask);
            Ok(EffectDecisionReplacement::Constant(true))
        },
    )
    .unwrap();
    assert_eq!(seen, (0..8).collect::<Vec<_>>());
    assert_eq!(selected, Some(EffectDecision::constant(true)));
}

#[test]
fn guarded_prefix_cases_share_the_suffix_and_keep_free_guard_order() {
    let mut setup = Meter::default();
    let suffix = atom(24).or(&atom(25), &mut setup).unwrap();
    let entries: Vec<_> = (16..24)
        .map(|cell| suffix.and(&atom(cell), &mut setup).unwrap())
        .collect();
    let point = EffectDecision::select_table(
        3,
        8,
        &mut setup,
        |bit| EffectDecisionReplacement::Variable(bit as u8),
        |mask, _| Ok(EffectDecisionReplacement::Decision(&entries[mask])),
    )
    .unwrap()
    .unwrap();
    let select = |mask: usize, reference: &u8| mask & (1 << reference) != 0;
    // Exercise guards before, equal to, and after the first suffix reference.
    let guard = |mask: usize| (mask != 0).then_some(15 + mask as u8 + (mask % 3) as u8);
    let mut repeated = Meter::default();
    let mut expected = EffectDecision::constant(true);
    for mask in 0..8 {
        let value = point
            .restrict_references(
                |reference| (*reference <= 2).then(|| select(mask, reference)),
                &mut repeated,
            )
            .unwrap();
        let requirement = match guard(mask) {
            Some(variable) => atom(variable)
                .conditional(&value, &EffectDecision::constant(true), &mut repeated)
                .unwrap(),
            None => value,
        };
        expected = expected.and(&requirement, &mut repeated).unwrap();
    }
    let mut work = Meter::default();
    let actual = EffectDecisionRequirement::Predicate(&point)
        .all_guarded_prefixes(&2, 0..8, &mut work, |mask, v| Ok(select(mask, v)), guard)
        .unwrap();
    assert_eq!(actual, expected);
    assert!(
        work.used < repeated.used,
        "{} < {}",
        work.used,
        repeated.used
    );
}

#[test]
fn prefix_table_equations_match_the_universal_relation() {
    let mut setup = Meter::default();
    let body = atom(0)
        .conditional(
            &atom(9),
            &atom(1).or(&atom(10), &mut setup).unwrap(),
            &mut setup,
        )
        .unwrap();
    let mut reference = Meter::default();
    let table = EffectDecision::select_table(
        2,
        4,
        &mut reference,
        |bit| EffectDecisionReplacement::Variable(bit as u8),
        |mask, _| Ok(EffectDecisionReplacement::Variable(4 + mask as u8)),
    )
    .unwrap()
    .unwrap();
    let expected = body
        .equivalent(&table, &mut reference)
        .unwrap()
        .forall(&[0, 1].into_iter().collect(), &mut reference)
        .unwrap();
    let mut work = Meter::default();
    let actual = body
        .matches_prefix_table(
            &1,
            0..4,
            &mut work,
            |mask, reference| Ok(mask & (1 << reference) != 0),
            |mask| EffectDecisionReplacement::Variable(4 + mask as u8),
        )
        .unwrap();
    assert_eq!(actual, expected);
    assert!(
        work.used < reference.used,
        "{} < {}",
        work.used,
        reference.used
    );
}

#[test]
fn universal_prefix_shares_reachable_pairs_and_preserves_suffix_references() {
    let mut setup = Meter::default();
    let premise = atom(0)
        .conditional(&atom(8), &atom(9), &mut setup)
        .unwrap()
        .and(&atom(2).or(&atom(4), &mut setup).unwrap(), &mut setup)
        .unwrap();
    let consequence = atom(2)
        .conditional(&atom(10), &atom(11), &mut setup)
        .unwrap()
        .or(&atom(4).and(&atom(8), &mut setup).unwrap(), &mut setup)
        .unwrap();
    for requirement in [
        EffectDecisionRequirement::Predicate(&consequence),
        EffectDecisionRequirement::Implication {
            premise: &premise,
            consequence: &consequence,
        },
    ] {
        let mut explicit = Meter::default();
        let reference = requirement
            .all_guarded_prefixes(
                &5,
                0..64,
                &mut explicit,
                |mask, variable| Ok(mask & (1 << variable) != 0),
                |_| None,
            )
            .unwrap();
        let mut work = Meter::default();
        let actual = requirement.forall_prefix(&5, &mut work).unwrap();
        assert_eq!(actual, reference);
        assert!(actual.node_count() > 0);
        assert!(actual.variables().all(|variable| *variable > 5));
        assert!(
            work.used < explicit.used,
            "{} < {}",
            work.used,
            explicit.used
        );

        let mut limited = Meter {
            used: 0,
            limit: Some(work.used - 1),
        };
        assert_eq!(
            requirement.forall_prefix(&5, &mut limited),
            Err((work.used, work.used - 1)),
        );
    }
}

#[test]
fn guarded_conjunction_shares_points_required_by_many_guards() {
    let mut setup = Meter::default();
    let point = atom(24).or(&atom(25), &mut setup).unwrap();
    let (expected, sequential_work) = {
        let mut work = Meter::default();
        let mut builder = DecisionBuilder::new(&mut work);
        let point = builder.import(&point).unwrap();
        let mut result = Root::True;
        for guard in 0..16 {
            let requirement = builder
                .branch_on_variable(guard, Root::True, point)
                .unwrap();
            result = builder
                .conditional(Conditional::new(result, requirement, Root::False))
                .unwrap();
        }
        (builder.finish(result).unwrap(), work.used)
    };
    let mut work = Meter::default();
    let mut builder = DecisionBuilder::new(&mut work);
    let point = builder.import(&point).unwrap();
    let clauses: Vec<_> = (0..16)
        .map(|guard| GuardedPoint {
            point,
            guard: Some(guard),
        })
        .collect();
    let result = builder.conjoin_guarded(&clauses).unwrap();
    assert_eq!(builder.finish(result).unwrap(), expected);
    assert!(
        work.used < sequential_work,
        "{} < {sequential_work}",
        work.used
    );
}

#[test]
fn guarded_prefix_only_imports_reachable_suffixes() {
    let mut setup = Meter::default();
    let suffix = atom(1).and(&atom(2), &mut setup).unwrap();
    let point = atom(0)
        .conditional(&suffix, &EffectDecision::constant(true), &mut setup)
        .unwrap();
    let mut work = Meter {
        used: 0,
        limit: Some(2),
    };

    assert_eq!(
        EffectDecisionRequirement::Predicate(&point).all_guarded_prefixes(
            &0,
            [0],
            &mut work,
            |_, _| Ok(false),
            |_| panic!("a true point needs no guard"),
        ),
        Ok(EffectDecision::constant(true))
    );
    assert_eq!(work.used, 2);
}

#[test]
fn guarded_prefix_implication_requires_only_the_present_patterns() {
    let mut work = Meter::default();
    let premise = atom(0).conditional(&atom(8), &atom(9), &mut work).unwrap();
    let consequence = atom(1)
        .conditional(&atom(10), &atom(11), &mut work)
        .unwrap();
    let requirement = EffectDecisionRequirement::Implication {
        premise: &premise,
        consequence: &consequence,
    };
    let folded = requirement
        .all_guarded_prefixes(
            &1,
            0..4,
            &mut work,
            |mask, reference| Ok(mask & (1 << reference) != 0),
            |mask| (mask != 0).then_some(4 + mask as u8),
        )
        .unwrap();
    for assignment in 0..1u16 << 12 {
        let value = |reference: u8| assignment & (1 << reference) != 0;
        let expected = (0..4u8).all(|mask| {
            let present = mask == 0 || value(4 + mask);
            let premise = value(if mask & 1 != 0 { 8 } else { 9 });
            let consequence = value(if mask & 2 != 0 { 10 } else { 11 });
            !present || !premise || consequence
        });
        assert_eq!(folded.evaluate(|&reference| value(reference)), expected);
    }
}

#[test]
fn shared_conjunction_has_the_same_result_as_pairwise_conjunction() {
    let mut work = Meter::default();
    let left = atom(0).or(&atom(1), &mut work).unwrap();
    let right = atom(1).or(&atom(2), &mut work).unwrap();
    let last = atom(0).not(&mut work).unwrap();
    let expected = left
        .and(&right, &mut work)
        .unwrap()
        .and(&last, &mut work)
        .unwrap();
    assert_eq!(
        EffectDecision::all([&left, &right, &last], &mut work).unwrap(),
        expected
    );
    let not_right = right.not(&mut work).unwrap();
    assert_eq!(
        left.equivalent(&right, &mut work).unwrap(),
        left.conditional(&right, &not_right, &mut work).unwrap()
    );
}

#[test]
fn equivalence_to_true_admits_only_a_copy_of_the_other_operand() {
    let mut setup = Meter::default();
    let effect = atom(0).and(&atom(1), &mut setup).unwrap();
    let mut copy = Meter::default();
    effect.copy_with_control(&mut copy).unwrap();
    let truth = EffectDecision::constant(true);
    for (left, right) in [(&truth, &effect), (&effect, &truth)] {
        let mut work = Meter::default();
        assert_eq!(left.equivalent(right, &mut work).unwrap(), effect);
        assert_eq!(work.used, copy.used);
    }
}

#[test]
fn fallible_evaluation_loads_only_admitted_references_on_the_selected_path() {
    let decision = atom(0).and(&atom(1), &mut Meter::default()).unwrap();
    let mut lookups = Vec::new();
    let mut lookup = |&reference: &u8| {
        lookups.push(reference);
        if reference == 0 {
            Ok(false)
        } else {
            Err((0, 0))
        }
    };
    let mut exhausted = Meter {
        limit: Some(0),
        ..Meter::default()
    };
    assert_eq!(
        decision.try_evaluate_with_control(&mut lookup, &mut exhausted),
        Err((1, 0))
    );
    assert!(
        !decision
            .try_evaluate_with_control(lookup, &mut Meter::default())
            .unwrap()
    );
    assert_eq!(lookups, [0]);
    assert_eq!(
        decision.try_evaluate_with_control(
            |&reference| if reference == 0 {
                Ok(true)
            } else {
                Err((0, 0))
            },
            &mut Meter::default(),
        ),
        Err((0, 0))
    );
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
fn terminal_condition_copies_only_the_selected_graph() {
    let mut meter = Meter::default();
    let selected = atom(0);
    let discarded = (2..12).fold(atom(1), |row, variable| {
        row.or(&atom(variable), &mut meter).unwrap()
    });
    let yes = EffectDecision::constant(true);
    let no = EffectDecision::constant(false);
    let copy_work = selected.node_count() + 1;
    for (condition, when_true, when_false) in [
        (&yes, &selected, &discarded),
        (&no, &discarded, &selected),
        (&selected, &yes, &no),
    ] {
        let mut exact = Meter {
            used: 0,
            limit: Some(copy_work),
        };
        assert_eq!(
            condition
                .conditional(when_true, when_false, &mut exact)
                .unwrap(),
            selected,
        );
        assert_eq!(exact.used, copy_work);
        let mut insufficient = Meter {
            used: 0,
            limit: Some(copy_work - 1),
        };
        assert_eq!(
            condition.conditional(when_true, when_false, &mut insufficient),
            Err((copy_work, copy_work - 1)),
        );
    }
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
fn quantifying_terminal_decisions_preserves_zero_work() {
    for quantified in [BTreeSet::new(), BTreeSet::from([0u8])] {
        for value in [false, true] {
            let relation = EffectDecision::constant(value);
            let mut meter = Meter {
                used: 0,
                limit: Some(0),
            };
            assert_eq!(relation.exists(&quantified, &mut meter).unwrap(), relation);
            assert_eq!(relation.forall(&quantified, &mut meter).unwrap(), relation);
            assert_eq!(meter.used, 0);
        }
    }
}

#[test]
fn universal_projection_combines_branches_without_rebuilding_complements() {
    let (a, b, c) = (atom(0), atom(1), atom(2));
    let mut setup = Meter::default();
    let relation = a
        .or(&b, &mut setup)
        .unwrap()
        .and(&c.or(&b, &mut setup).unwrap(), &mut setup)
        .unwrap();
    let quantified = BTreeSet::from([0, 2]);
    let mut dual = Meter::default();
    let expected = relation
        .not(&mut dual)
        .unwrap()
        .exists(&quantified, &mut dual)
        .unwrap()
        .not(&mut dual)
        .unwrap();
    let mut direct = Meter {
        used: 0,
        limit: Some(dual.used - 1),
    };
    let actual = relation.forall(&quantified, &mut direct).unwrap();
    assert_eq!(actual, b);
    assert_eq!(actual, expected);
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
fn restriction_retains_free_branches_without_visiting_excluded_subgraphs() {
    let mut setup = Meter::default();
    let discarded = (3..64).fold(EffectDecision::constant(false), |tail, variable| {
        atom(variable).or(&tail, &mut setup).unwrap()
    });
    let left = atom(1)
        .conditional(&discarded, &atom(2), &mut setup)
        .unwrap();
    let right = atom(1)
        .conditional(&discarded, &atom(3), &mut setup)
        .unwrap();
    let decision = atom(0).conditional(&left, &right, &mut setup).unwrap();
    let expected = atom(0).conditional(&atom(2), &atom(3), &mut setup).unwrap();
    let mut meter = Meter {
        limit: Some(discarded.nodes.len()),
        ..Meter::default()
    };
    let assignment = BTreeMap::from([(1, false)]);
    assert_eq!(
        decision.restrict(&assignment, &mut meter).unwrap(),
        expected
    );
    assert_eq!(
        decision
            .restrict_references(|variable| (*variable == 1).then_some(true), &mut setup)
            .unwrap(),
        discarded
    );
}

#[test]
fn least_witnesses_for_disjoint_cofactors_fit_the_work_limit() {
    let mut setup = Meter::default();
    let selectors = 5;
    let candidates = 1u8 << selectors;
    let witnesses = selectors..selectors + candidates;
    let mut frontier: Vec<_> = witnesses.clone().map(atom).collect();
    for selector in 0..selectors {
        let bit = atom(selector);
        frontier = frontier
            .chunks_exact(2)
            .map(|pair| bit.conditional(&pair[1], &pair[0], &mut setup).unwrap())
            .collect();
    }
    let relation = frontier.pop().unwrap();
    let mut work = Meter {
        limit: Some(3000),
        ..Meter::default()
    };
    let completed = relation
        .complete_conditionally(witnesses.clone().collect::<BTreeSet<_>>(), &mut work)
        .unwrap();
    assert!(completed.satisfiable().is_constant(true));
    assert!(completed.principal().is_constant(true));
    for variable in witnesses {
        let decision = completed.lower_envelope().get(&variable).unwrap();
        let expected: Vec<_> = (0..candidates)
            .map(|mask| mask == variable - selectors)
            .collect();
        assert_eq!(truth_table(decision, selectors), expected);
    }
}

#[test]
fn least_closed_tuple_accounts_for_literals_skipped_by_an_accepting_path() {
    let mut setup = Meter::default();
    let dense = (0..64).fold(EffectDecision::constant(true), |decision, variable| {
        decision.and(&atom(variable), &mut setup).unwrap()
    });
    let alternate = dense.or(&atom(64), &mut setup).unwrap();
    let bound: Arc<BTreeSet<_>> = Arc::new((0..=64).collect());
    for (relation, has_least) in [(dense, true), (alternate, false)] {
        let mut work = Meter {
            limit: Some(1024),
            ..Meter::default()
        };
        let completed = relation
            .complete_conditionally(Arc::clone(&bound), &mut work)
            .unwrap();
        assert!(completed.satisfiable().is_constant(true));
        assert!(completed.principal().is_constant(has_least));
        for variable in 0..=64 {
            assert!(
                completed
                    .lower_envelope()
                    .get(&variable)
                    .unwrap()
                    .is_constant(has_least && variable < 64)
            );
        }
    }
}

#[test]
fn substitution_borrows_target_formulas_and_resolves_each_source_reference_once() {
    let mut meter = Meter::default();
    let (a, b) = (atom(0), atom(1));
    let source = a
        .conditional(&b, &b.not(&mut meter).unwrap(), &mut meter)
        .unwrap();
    assert_eq!(
        source
            .variables()
            .filter(|variable| **variable == 1)
            .count(),
        2
    );
    let target = atom(2).or(&a, &mut meter).unwrap();
    let mut seen = BTreeSet::new();
    let result = source
        .substitute_references(&mut meter, |reference, _| {
            assert!(seen.insert(*reference));
            Ok(if *reference == 0 {
                EffectDecisionReplacement::Decision(&target)
            } else {
                EffectDecisionReplacement::Variable(0)
            })
        })
        .unwrap();
    assert_eq!(seen, BTreeSet::from([0, 1]));
    assert_eq!(
        result,
        a.or(&atom(2).not(&mut meter).unwrap(), &mut meter).unwrap()
    );
}

#[test]
fn substitution_of_a_reference_admits_the_canonical_replacement_directly() {
    let mut setup = Meter::default();
    let source = atom(0);
    let operands = (1..=32).map(atom).collect::<Vec<_>>();
    let replacement = EffectDecision::all(&operands, &mut setup).unwrap();
    let limit = replacement.node_count() + 2;
    let mut short = Meter {
        used: 0,
        limit: Some(limit - 1),
    };
    let mut exact = Meter {
        used: 0,
        limit: Some(limit),
    };
    let bind = |_: &u8, _: &mut Meter| Ok(EffectDecisionReplacement::Decision(&replacement));
    assert_eq!(
        source.substitute_references(&mut short, bind),
        Err((limit, limit - 1))
    );
    assert_eq!(
        source.substitute_references(&mut exact, bind).unwrap(),
        replacement
    );
}

#[test]
fn substitution_work_depends_only_on_reachable_replacements() {
    let source = atom(0);
    let replacement = atom(1);
    let mut replacements = BTreeMap::from([(0, replacement.clone())]);
    let mut exact = Meter::default();
    assert_eq!(
        source.substitute(&replacements, &mut exact).unwrap(),
        replacement
    );

    replacements.extend((1..=u8::MAX).map(|variable| (variable, EffectDecision::constant(false))));
    let mut limited = Meter {
        used: 0,
        limit: Some(exact.used),
    };
    assert_eq!(
        source.substitute(&replacements, &mut limited).unwrap(),
        replacement
    );
    assert_eq!(limited.used, exact.used);
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
            let universal = relation.forall(&quantified, &mut meter).unwrap();
            let independent = relation
                .independence_region(&quantified, &mut meter)
                .unwrap();
            assert!(
                universal
                    .variables()
                    .all(|variable| !quantified.contains(variable))
            );
            assert!(
                independent
                    .variables()
                    .all(|variable| !quantified.contains(variable))
            );
            let completed = relation.complete(&quantified, &mut meter).unwrap();
            let conditional = relation
                .complete_conditionally(quantified.clone(), &mut meter)
                .unwrap();
            assert_eq!(conditional.satisfiable(), &completed.admissibility);
            let envelope = conditional.lower_envelope();
            assert_eq!(envelope.domain(), &quantified);
            assert_eq!(
                &envelope.instantiate(&relation, &mut meter).unwrap(),
                conditional.principal()
            );
            for rigid in (0..3u8).filter(|variable| !quantified.contains(variable)) {
                assert!(envelope.get(&rigid).is_none());
            }
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
                    universal.evaluate(|variable| rigid & (1 << variable) != 0),
                    models.len() == (1usize << quantified.len()),
                    "forall: table {table}, mask {quantified_mask}, rigid {rigid}"
                );
                assert_eq!(
                    independent.evaluate(|variable| rigid & (1 << variable) != 0),
                    models.is_empty() || models.len() == (1usize << quantified.len()),
                    "independence: table {table}, mask {quantified_mask}, rigid {rigid}"
                );
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
                let meet = least_models[usize::from(rigid)];
                let assignment = (0..3u8)
                    .filter(|variable| !quantified.contains(variable))
                    .map(|variable| (variable, rigid & (1 << variable) != 0))
                    .collect();
                let specialized = envelope.specialize(&assignment, &mut meter).unwrap();
                assert!(specialized.is_closed(&mut meter).unwrap());
                assert_eq!(specialized.domain(), &quantified);
                assert_eq!(
                    conditional
                        .principal()
                        .evaluate(|variable| rigid & (1 << variable) != 0),
                    models.contains(&meet),
                    "conditional principal: table {table}, mask {quantified_mask}, rigid {rigid}"
                );
                for variable in &quantified {
                    let value = envelope.get(variable).unwrap();
                    assert!(
                        specialized
                            .get(variable)
                            .unwrap()
                            .is_constant(meet & (1 << variable) != 0)
                    );
                    assert!(
                        value
                            .variables()
                            .all(|variable| !quantified.contains(variable))
                    );
                    assert_eq!(
                        value.evaluate(|variable| rigid & (1 << variable) != 0),
                        meet & (1 << variable) != 0,
                        "conditional envelope: table {table}, mask {quantified_mask}, rigid {rigid}, variable {variable}"
                    );
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
fn conditional_completion_retains_exact_principal_and_unsatisfiable_domains() {
    let mut meter = Meter::default();
    let (parameter, first, second) = (atom(0), atom(1), atom(2));
    let exclusive = first
        .conditional(&second.not(&mut meter).unwrap(), &second, &mut meter)
        .unwrap();
    let relation = parameter.or(&exclusive, &mut meter).unwrap();
    let quantified = BTreeSet::from([1, 2]);
    let conditional = relation
        .complete_conditionally(quantified.clone(), &mut meter)
        .unwrap();
    assert!(conditional.satisfiable().is_constant(true));
    assert_eq!(conditional.principal(), &parameter);
    assert!(conditional.lower_envelope().nonzero.is_empty());
    assert!(
        relation
            .complete(&quantified, &mut meter)
            .unwrap()
            .least
            .is_none()
    );

    let incomparable = exclusive
        .complete_conditionally(quantified.clone(), &mut meter)
        .unwrap();
    assert!(incomparable.satisfiable().is_constant(true));
    assert!(incomparable.principal().is_constant(false));
    let impossible = EffectDecision::constant(false)
        .complete_conditionally(quantified.clone(), &mut meter)
        .unwrap();
    assert!(impossible.satisfiable().is_constant(false));
    assert!(impossible.principal().is_constant(false));
}

#[test]
fn conditional_completion_specializes_shared_coefficients_once() {
    let mut setup = Meter::default();
    let required = atom(0).and(&atom(1), &mut setup).unwrap();
    let outputs = atom(2).and(&atom(3), &mut setup).unwrap();
    let relation = required
        .conditional(&outputs, &EffectDecision::constant(true), &mut setup)
        .unwrap();
    let completion = relation
        .complete_conditionally(BTreeSet::from([2, 3, 4]), &mut setup)
        .unwrap();
    assert!(completion.principal().is_constant(true));
    let envelope = completion.lower_envelope();
    let left = envelope.get(&2).unwrap();
    let right = envelope.get(&3).unwrap();
    assert_eq!(left, &required);
    assert!(std::ptr::eq(left, right));
    assert!(envelope.get(&4).unwrap().is_constant(false));
    assert!(envelope.get(&5).is_none());

    let untouched = envelope.specialize(&BTreeMap::new(), &mut setup).unwrap();
    assert!(std::ptr::eq(untouched.get(&2).unwrap(), left));

    let assignment = BTreeMap::from([(0, true)]);
    let mut single_restriction = Meter::default();
    assert_eq!(
        required
            .restrict(&assignment, &mut single_restriction)
            .unwrap(),
        atom(1)
    );
    let mut work = Meter::default();
    let specialized = envelope.specialize(&assignment, &mut work).unwrap();
    assert_eq!(specialized.get(&2), Some(&atom(1)));
    assert!(std::ptr::eq(
        specialized.get(&2).unwrap(),
        specialized.get(&3).unwrap()
    ));
    // Two coordinates, one cached restriction, and one result tuple.
    assert_eq!(work.used, single_restriction.used + 6);

    let zero = envelope
        .specialize(&BTreeMap::from([(0, false)]), &mut setup)
        .unwrap();
    assert!(zero.nonzero.is_empty());
    assert_eq!(zero.domain(), envelope.domain());
}

#[test]
fn conditional_completion_aborts_at_every_budget_boundary_without_mutating_inputs() {
    let mut meter = Meter::default();
    let relation = atom(0).or(&atom(1), &mut meter).unwrap();
    let original = relation.clone();
    let quantified = BTreeSet::from([1]);
    let mut full = Meter::default();
    let expected = relation
        .complete_conditionally(quantified.clone(), &mut full)
        .unwrap();
    for limit in 0..full.used {
        let mut limited = Meter {
            used: 0,
            limit: Some(limit),
        };
        assert_eq!(
            relation
                .complete_conditionally(quantified.clone(), &mut limited)
                .unwrap_err(),
            (limit + 1, limit)
        );
        assert_eq!(relation, original);
    }
    let mut exact = Meter {
        used: 0,
        limit: Some(full.used),
    };
    assert_eq!(
        relation
            .complete_conditionally(quantified.clone(), &mut exact)
            .unwrap(),
        expected
    );
}

#[test]
fn controlled_evaluation_admits_each_visit_before_reading_its_variable() {
    let mut meter = Meter::default();
    let decision = atom(0).and(&atom(1), &mut meter).unwrap();
    let mut full = Meter::default();
    assert!(decision.evaluate_with_control(|_| true, &mut full).unwrap());
    assert_eq!(full.used, 3);
    for limit in 0..full.used {
        let mut calls = 0;
        let mut limited = Meter {
            used: 0,
            limit: Some(limit),
        };
        assert_eq!(
            decision
                .evaluate_with_control(
                    |_| {
                        calls += 1;
                        true
                    },
                    &mut limited
                )
                .unwrap_err(),
            (limit + 1, limit)
        );
        assert_eq!(calls, limit.min(2));
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
