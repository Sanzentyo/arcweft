use super::*;
use crate::effect_row::prepared::test_support::*;

#[test]
fn confirmation_rejects_a_larger_fixed_point_than_the_retained_least_tuple() {
    for named_support in [false, true] {
        let mut graph = PreparedEffectGraph::default();
        let mut control = budget();
        let scheme = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut control)
            .unwrap();
        let recursive = graph.application(&scheme, vec![], &mut control).unwrap();
        graph
            .prepare_body(&scheme, recursive, vec![], &mut control)
            .unwrap();
        if named_support {
            graph
                .union(vec![literal(&["fs.read"])], &mut control)
                .unwrap();
        }
        let mut principal = graph.solve_principal(&mut control).unwrap();
        assert_eq!(
            principal.scheme(&scheme).unwrap(),
            &EffectRow::closed(EffectSet::new())
        );
        principal.schemes.insert(
            scheme.0.ordinal,
            EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap()),
        );
        assert!(matches!(
            principal.confirm(&mut Work(&mut control)),
            Err(PreparedEffectFailure::Invariant(
                PreparedEffectInvariant::PrincipalConfirmation
            ))
        ));
    }
}

#[test]
fn a_uniform_choice_is_not_a_finite_effect_row_variable() {
    let graph = PreparedEffectGraph::default();
    let mut control = budget();
    let principal = graph.solve_principal(&mut control).unwrap();
    let scope = FiniteRowValuation {
        classes: principal.layout.view().label_classes(),
        owner: FormalOwner::Observation,
        parameters: &[],
    };
    let replay = CanonicalReplay::new(&principal, scope);
    // Core row variables are finite sets. A raw Choice variable passes its
    // all-variables-empty check but denotes infinitely many labels when true.
    let infinite = Formula::variable(Variable::Choice(0), &mut control).unwrap();
    assert!(matches!(
        replay.finite(&infinite, &mut Work(&mut control)),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::PrincipalConfirmation
        ))
    ));
    let finite = Formula::try_from_decisions(
        EffectDecision::constant(false),
        BTreeMap::from([(
            EffectId::parse("fs.read").unwrap(),
            EffectDecision::variable(Variable::Choice(0), &mut control).unwrap(),
        )]),
        &mut Work(&mut control),
    )
    .unwrap();
    replay.finite(&finite, &mut Work(&mut control)).unwrap();
}
