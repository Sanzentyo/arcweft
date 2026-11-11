use std::collections::{BTreeMap, BTreeSet};

use arcweft_core::effect_row::{
    DecisionControl, DecisionWork, EffectDecision, EffectDeclarationError, EffectFormula, EffectSet,
};
use arcweft_id::EffectId;

struct Control;

impl DecisionControl for Control {
    type Error = EffectDeclarationError;

    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn identity_scheme_keeps_true_boolean_cells_without_issuing_an_infinite_row() {
    let mut control = Control;
    let input = EffectDecision::variable(0u8, &mut control).unwrap();
    let output = input
        .conditional(
            &EffectDecision::constant(true),
            &EffectDecision::constant(false),
            &mut control,
        )
        .unwrap();
    let identity =
        EffectFormula::try_from_decisions(output, BTreeMap::new(), &mut control).unwrap();
    assert_eq!(identity, EffectFormula::variable(0, &mut control).unwrap());
    assert_eq!(
        EffectFormula::<u8>::try_from_decisions(
            EffectDecision::constant(true),
            BTreeMap::new(),
            &mut control,
        )
        .unwrap_err(),
        EffectDeclarationError::InfiniteRow
    );
}

#[test]
fn one_discrete_choice_cannot_supply_different_winners_for_different_labels() {
    let mut control = Control;
    let choice = EffectDecision::variable(0u8, &mut control).unwrap();
    let read = EffectId::parse("fs.read").unwrap();
    let write = EffectId::parse("fs.write").unwrap();
    let alternatives = EffectFormula::try_from_decisions(
        EffectDecision::constant(false),
        BTreeMap::from([
            (read.clone(), choice.clone()),
            (write.clone(), choice.not(&mut control).unwrap()),
        ]),
        &mut control,
    )
    .unwrap();
    let view = alternatives.membership();
    assert!(alternatives.constant_effects().is_empty());
    assert_eq!(view.overrides().len(), 2);
    let joint = view.at(&read).and(view.at(&write), &mut control).unwrap();
    let completion = joint
        .complete_conditionally(&BTreeSet::from([0]), &mut control)
        .unwrap();
    assert!(completion.satisfiable().is_constant(false));
    assert!(completion.principal().is_constant(false));

    // Ordinary row-reference inference remains independently quantified at each
    // label. A program-level choice must instead use the joint decision above.
    let required = EffectFormula::from_set(
        &EffectSet::from_labels(["fs.read", "fs.write"]).unwrap(),
        &mut control,
    )
    .unwrap();
    let labelwise = required.subset(&alternatives, &mut control).unwrap();
    assert!(
        labelwise
            .complete(&BTreeSet::from([0]), &mut control)
            .unwrap()
            .least
            .is_some()
    );
}
