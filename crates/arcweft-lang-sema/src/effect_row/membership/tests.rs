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
fn scoped_reference_mapping_preserves_inner_binders_and_reorders_outer_slots() {
    use crate::types::{GenericBinder, GenericEffectReference, GenericScope, GenericScopeError};

    struct ScopeControl;
    impl DecisionControl for ScopeControl {
        type Error = GenericScopeError;
        fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    let incoming = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let nested = incoming.with_binder(GenericBinder::new(0, 0, 1));
    let outer_first = nested.bound_effect(1, 0).unwrap();
    let outer_second = nested.bound_effect(1, 1).unwrap();
    let inner = nested.bound_effect(0, 0).unwrap();
    let mut control = ScopeControl;
    let first = EffectFormula::variable(outer_first, &mut control).unwrap();
    let second = EffectFormula::variable(outer_second, &mut control).unwrap();
    let inner_formula = EffectFormula::variable(inner.clone(), &mut control).unwrap();
    let effects = EffectSet::from_labels(["fs.read"]).unwrap();
    let read = EffectFormula::from_set(&effects, &mut control).unwrap();
    let source = first
        .union(&inner_formula, &mut control)
        .unwrap()
        .difference(&second, &mut control)
        .unwrap()
        .union(&read, &mut control)
        .unwrap();
    let mut mapping = |reference: &GenericEffectReference, _: &mut ScopeControl| match reference
        .template_key(&incoming, &nested)?
    {
        Some(GenericEffectReference::Bound(parameter)) => {
            nested.bound_effect(1, 1 - parameter.slot())
        }
        Some(_) | None => Ok(reference.clone()),
    };
    let mapped = source.map_references(&mut control, &mut mapping).unwrap();
    let expected = second
        .union(&inner_formula, &mut control)
        .unwrap()
        .difference(&first, &mut control)
        .unwrap()
        .union(&read, &mut control)
        .unwrap();
    assert_eq!(mapped, expected);
    let predicate = source.subset(&read, &mut control).unwrap();
    assert_eq!(
        predicate
            .map_references(&mut control, &mut mapping)
            .unwrap(),
        expected.subset(&read, &mut control).unwrap()
    );

    let difference = first.difference(&second, &mut control).unwrap();
    let collapsed = difference
        .map_references(&mut control, &mut |_, _| Ok(inner.clone()))
        .unwrap();
    assert_eq!(collapsed, EffectFormula::empty());
    let before = source.clone();
    let root = GenericScope::default();
    assert!(matches!(
        source.map_references(&mut control, &mut |reference, _| {
            reference
                .template_key(&root, &root)
                .map(|key| key.expect("root template key"))
        }),
        Err(GenericScopeError::UnknownDepth { .. })
    ));
    assert_eq!(source, before);
}
