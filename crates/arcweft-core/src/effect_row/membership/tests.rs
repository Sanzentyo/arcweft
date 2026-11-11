use super::*;

#[derive(Default)]
struct Meter(usize);

impl DecisionControl for Meter {
    type Error = std::convert::Infallible;
    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        self.0 += 1;
        Ok(())
    }
}

fn row(labels: &[&str]) -> EffectFormula<u8> {
    EffectFormula::from_set(
        &EffectSet::from_labels(labels.iter().copied()).unwrap(),
        &mut Meter::default(),
    )
    .unwrap()
}

fn variable(index: u8) -> EffectFormula<u8> {
    EffectFormula::variable(index, &mut Meter::default()).unwrap()
}

#[derive(Debug, PartialEq, thiserror::Error)]
enum AssemblyError {
    #[error(transparent)]
    Declaration(#[from] EffectDeclarationError),
    #[error("assembly stopped after {used} visits with limit {limit}")]
    Limit { used: usize, limit: usize },
}

#[derive(Default)]
struct AssemblyControl {
    used: usize,
    limit: Option<usize>,
}

impl DecisionControl for AssemblyControl {
    type Error = AssemblyError;

    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        self.used += 1;
        if let Some(limit) = self.limit
            && self.used > limit
        {
            return Err(AssemblyError::Limit {
                used: self.used,
                limit,
            });
        }
        Ok(())
    }
}

#[test]
fn membership_view_preserves_conditional_support_and_the_predicate_role() {
    let mut control = Meter::default();
    let original = row(&["fs.read"])
        .intersection(&variable(0), &mut control)
        .unwrap();
    assert!(original.constant_effects().is_empty());
    let view = original.membership();
    assert!(view.default_decision().is_constant(false));
    assert_eq!(view.overrides().len(), 1);
    assert_eq!(
        view.at(&EffectId::parse("fs.read").unwrap())
            .single_reference(),
        Some(&0)
    );
    assert!(
        view.at(&EffectId::parse("fs.write").unwrap())
            .is_constant(false)
    );
    let reconstructed = EffectFormula::try_from_decisions(
        view.default_decision().clone(),
        view.overrides()
            .map(|(label, decision)| (label.clone(), decision.clone()))
            .collect(),
        &mut AssemblyControl::default(),
    )
    .unwrap();
    assert_eq!(reconstructed, original);

    let predicate = original.subset(&row(&["fs.write"]), &mut control).unwrap();
    let relation = predicate.membership();
    assert!(relation.default_decision().is_constant(true));
    assert_eq!(
        EffectFormula::try_from_decisions(
            relation.default_decision().clone(),
            relation
                .overrides()
                .map(|(label, decision)| (label.clone(), decision.clone()))
                .collect(),
            &mut AssemblyControl::default(),
        )
        .unwrap_err(),
        AssemblyError::Declaration(EffectDeclarationError::InfiniteRow)
    );
}

#[test]
fn decision_assembly_normalizes_overrides_and_aborts_before_unadmitted_growth() {
    let default = variable(0).membership().default_decision().clone();
    let overrides = BTreeMap::from([
        (
            EffectId::parse("fs.read").unwrap(),
            EffectDecision::constant(true),
        ),
        (EffectId::parse("fs.write").unwrap(), default.clone()),
    ]);
    let before_default = default.clone();
    let before_overrides = overrides.clone();
    let mut full = AssemblyControl::default();
    let expected =
        EffectFormula::try_from_decisions(default.clone(), overrides.clone(), &mut full).unwrap();
    assert_eq!(expected.membership().overrides().len(), 1);
    assert_eq!(
        expected,
        variable(0)
            .union(&row(&["fs.read"]), &mut Meter::default())
            .unwrap()
    );
    for limit in 0..full.used {
        let mut limited = AssemblyControl {
            used: 0,
            limit: Some(limit),
        };
        assert_eq!(
            EffectFormula::try_from_decisions(default.clone(), overrides.clone(), &mut limited)
                .unwrap_err(),
            AssemblyError::Limit {
                used: limit + 1,
                limit
            }
        );
        assert_eq!(default, before_default);
        assert_eq!(overrides, before_overrides);
    }
    let mut exact = AssemblyControl {
        used: 0,
        limit: Some(full.used),
    };
    assert_eq!(
        EffectFormula::try_from_decisions(default, overrides, &mut exact).unwrap(),
        expected
    );
}

#[test]
fn local_universal_rows_are_eliminated_before_input_existentials() {
    let mut meter = Meter::default();
    let equality = variable(0)
        .subset(&variable(1), &mut meter)
        .unwrap()
        .and(
            &variable(1).subset(&variable(0), &mut meter).unwrap(),
            &mut meter,
        )
        .unwrap();
    assert!(
        equality
            .project(&BTreeSet::from([0]), &mut meter)
            .unwrap()
            .universally_quantified(&BTreeSet::from([1]), &mut meter)
            .unwrap()
            .is_unconstrained()
    );
    assert!(
        equality
            .universally_quantified(&BTreeSet::from([1]), &mut meter)
            .unwrap()
            .project(&BTreeSet::from([0]), &mut meter)
            .unwrap()
            .is_impossible()
    );
}

#[test]
fn finite_set_operations_preserve_distinct_labels_and_canonical_identity() {
    let mut meter = Meter::default();
    let read = row(&["fs.read"]);
    let write = row(&["fs.write"]);
    let both = read.union(&write, &mut meter).unwrap();
    assert_eq!(both, row(&["fs.write", "fs.read"]));
    assert_eq!(both.intersection(&read, &mut meter).unwrap(), read);
    assert_eq!(both.difference(&read, &mut meter).unwrap(), write);
    assert_eq!(
        both.closed(&mut meter).unwrap().unwrap().to_labels(),
        ["fs.read", "fs.write"]
    );
    assert!(read.subset(&write, &mut meter).unwrap().is_impossible());
    assert!(read.subset(&both, &mut meter).unwrap().is_unconstrained());
}

#[test]
fn invoked_rows_join_without_widening_the_escaping_callback() {
    let mut meter = Meter::default();
    let (first, second) = (variable(0), variable(1));
    let invocation = first.union(&second, &mut meter).unwrap();
    let replacements = BTreeMap::from([(0, row(&["fs.read"])), (1, row(&["fs.write"]))]);
    assert_eq!(
        invocation.substitute(&replacements, &mut meter).unwrap(),
        row(&["fs.read", "fs.write"])
    );
    assert_eq!(
        first.substitute(&replacements, &mut meter).unwrap(),
        row(&["fs.read"])
    );
    assert_eq!(first, variable(0));
}

#[test]
fn sibling_uses_of_a_residual_row_are_independent() {
    let mut meter = Meter::default();
    let residual = variable(0);
    assert_eq!(
        residual
            .substitute(&BTreeMap::from([(0, row(&[]))]), &mut meter)
            .unwrap(),
        row(&[])
    );
    assert_eq!(
        residual
            .substitute(&BTreeMap::from([(0, row(&["fs.read"]))]), &mut meter)
            .unwrap(),
        row(&["fs.read"])
    );
    assert!(residual.closed(&mut meter).unwrap().is_none());
}

#[test]
fn completion_retains_a_bound_instead_of_assuming_all_future_rows_are_valid() {
    let mut meter = Meter::default();
    let relation = variable(0)
        .subset(&variable(1), &mut meter)
        .unwrap()
        .and(
            &variable(1).subset(&row(&["fs.read"]), &mut meter).unwrap(),
            &mut meter,
        )
        .unwrap();
    let completed = relation.complete(&BTreeSet::from([1]), &mut meter).unwrap();
    assert_eq!(
        completed.admissibility,
        variable(0).subset(&row(&["fs.read"]), &mut meter).unwrap()
    );
    assert!(!completed.admissibility.is_unconstrained());
    let least = completed.least.unwrap();
    assert!(least[&1].closed(&mut meter).unwrap().is_none());
    for effects in [row(&[]), row(&["fs.read"])] {
        let replacement = BTreeMap::from([(0, effects.clone())]);
        assert!(
            completed
                .admissibility
                .substitute(&replacement, &mut meter)
                .unwrap()
                .is_unconstrained()
        );
        assert_eq!(
            least[&1].substitute(&replacement, &mut meter).unwrap(),
            effects
        );
    }
    assert!(
        completed
            .admissibility
            .substitute(&BTreeMap::from([(0, row(&["fs.write"]))]), &mut meter)
            .unwrap()
            .is_impossible()
    );
}

#[test]
fn explicit_effects_do_not_choose_between_independent_residual_witnesses() {
    let mut meter = Meter::default();
    let relation = row(&["fs.read"])
        .subset(
            &variable(0).union(&variable(1), &mut meter).unwrap(),
            &mut meter,
        )
        .unwrap();
    let completed = relation
        .complete(&BTreeSet::from([0, 1]), &mut meter)
        .unwrap();
    assert!(completed.admissibility.is_unconstrained());
    assert!(completed.least.is_none());
    let retained = relation.complete(&BTreeSet::new(), &mut meter).unwrap();
    assert_eq!(retained.admissibility, relation);
    assert!(retained.least.unwrap().is_empty());
}

#[test]
fn impossible_default_is_not_mistaken_for_an_infinite_effect_witness() {
    let mut meter = Meter::default();
    let raw = Membership {
        default: EffectDecision::variable(0u8, &mut meter).unwrap(),
        overrides: BTreeMap::new(),
    };
    let impossible = EffectPredicate::normalized(raw);
    assert_eq!(impossible, EffectPredicate::impossible());
    let completed = impossible
        .complete(&BTreeSet::from([0]), &mut meter)
        .unwrap();
    assert!(completed.admissibility.is_impossible());
    assert_eq!(completed.least.unwrap(), BTreeMap::from([(0, row(&[]))]));
    assert!(EffectPredicate::<u8>::unconstrained().is_unconstrained());
}

#[test]
fn substitutions_include_labels_introduced_only_by_replacements() {
    let mut meter = Meter::default();
    let formula = variable(0).difference(&variable(1), &mut meter).unwrap();
    let replacement = BTreeMap::from([
        (0, row(&["fs.read", "log.write"])),
        (1, row(&["log.write"])),
    ]);
    assert_eq!(
        formula.substitute(&replacement, &mut meter).unwrap(),
        row(&["fs.read"])
    );
    let replacement = BTreeMap::from([(0, variable(1)), (1, row(&["fs.read"]))]);
    assert_eq!(
        variable(0).substitute(&replacement, &mut meter).unwrap(),
        variable(1)
    );
}

#[test]
fn semantic_child_counts_use_owner_lists_instead_of_encoded_reference_ordinals() {
    let formula = variable(0)
        .union(
            &row(&["fs.read", "fs.write", "io.read"]),
            &mut Meter::default(),
        )
        .unwrap();
    let mut widths = Vec::new();
    formula
        .try_visit_semantic_child_counts(&mut |count| {
            widths.push(count);
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(widths, [1, 3, 0, 0, 0]);
    let mut visited = Vec::new();
    assert_eq!(
        formula.try_visit_semantic_child_counts(&mut |count| {
            visited.push(count);
            if count > 2 { Err(count) } else { Ok(()) }
        }),
        Err(3)
    );
    assert_eq!(visited, [1, 3]);
}
