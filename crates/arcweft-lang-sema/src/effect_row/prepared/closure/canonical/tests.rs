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
        principal.functions.insert(
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
fn confirmation_rejects_a_different_row_for_a_fixed_input() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let scope = crate::types::GenericScope::default()
        .with_binder(crate::types::GenericBinder::new(0, 0, 1));
    let reference = parameter(&scope, 0);
    let input = graph
        .declare_unknown(
            vec![reference.clone()],
            PreparedEffectUnknownEligibility::Future,
            &mut work,
        )
        .unwrap();
    let effects = EffectSet::from_labels(["fs.read"]).unwrap();
    let row = EffectRow::open(effects.clone(), reference);
    let mut closed = graph
        .specialize_principal(&[(&input, &row)], &mut work)
        .unwrap();
    assert_eq!(closed.unknown(&input).unwrap(), &row);

    let id = FunctionRef::Unknown(input).reference().ordinal;
    closed.functions.insert(id, EffectRow::closed(effects));
    assert!(matches!(
        closed.confirm(&mut Work(&mut work)),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::PrincipalConfirmation
        ))
    ));
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

#[test]
fn a_new_scoped_fact_must_hold_for_every_retained_choice_witness() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let truth = PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained());
    let falsity = PreparedEffectPredicate::Canonical(EffectPredicate::impossible());
    let choice = graph.choice(&mut control).unwrap();
    let conditional = graph
        .select_predicate(&choice, truth.clone(), falsity, &mut control)
        .unwrap();
    let fact = graph
        .read()
        .scoped_constraint(&[], &truth, &conditional, &mut control)
        .unwrap();
    let principal = graph.solve_principal(&mut control).unwrap();
    assert!(matches!(
        principal.confirm_scoped_constraint(&fact, &mut control),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::PrincipalConfirmation
        ))
    ));
}

#[test]
fn scoped_confirmation_proves_the_whole_implication_with_unused_inputs() {
    let graph = PreparedEffectGraph::default();
    let mut control = budget();
    let gamma = crate::types::GenericScope::default()
        .with_binder(crate::types::GenericBinder::new(0, 0, 2));
    let parameters = [parameter(&gamma, 0), parameter(&gamma, 1)];
    let truth = PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained());
    let falsity = PreparedEffectPredicate::Canonical(EffectPredicate::impossible());
    let principal = graph.solve_principal(&mut control).unwrap();
    for (domain, accepted) in [(&truth, false), (&falsity, true)] {
        let fact = graph
            .read()
            .scoped_constraint(&parameters, domain, &falsity, &mut control)
            .unwrap();
        let result = principal.confirm_scoped_constraint(&fact, &mut control);
        if accepted {
            assert_eq!(result.unwrap(), EffectPredicate::unconstrained());
        } else {
            assert!(matches!(
                result,
                Err(PreparedEffectFailure::Invariant(
                    PreparedEffectInvariant::PrincipalConfirmation
                ))
            ));
        }
    }
}

#[test]
fn query_function_uses_follow_actual_arguments_and_validate_owner_identity() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let mut declare = |graph: &mut PreparedEffectGraph| {
        graph
            .declare_unknown(
                vec![],
                PreparedEffectUnknownEligibility::Bindable,
                &mut control,
            )
            .unwrap()
    };
    let used = declare(&mut graph);
    let unused = declare(&mut graph);
    let inner = graph
        .unknown_application(&used, vec![], &mut control)
        .unwrap();
    let gamma = crate::types::GenericScope::default()
        .with_binder(crate::types::GenericBinder::new(0, 0, 1));
    let outer = graph
        .declare_unknown(
            vec![parameter(&gamma, 0)],
            PreparedEffectUnknownEligibility::Bindable,
            &mut control,
        )
        .unwrap();
    let row = graph
        .unknown_application(&outer, vec![inner], &mut control)
        .unwrap();
    let principal = graph.solve_principal(&mut control).unwrap();
    let uses = PreparedEffectTerms::Closed(&principal)
        .row_function_uses(&row, &mut control)
        .unwrap();
    assert!(uses.contains_unknown(&used).unwrap());
    assert!(uses.contains_unknown(&outer).unwrap());
    assert!(!uses.contains_unknown(&unused).unwrap());
    let mut foreign = PreparedEffectGraph::default();
    let foreign = foreign
        .declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Bindable,
            &mut control,
        )
        .unwrap();
    assert!(uses.contains_unknown(&foreign).is_err());
}
