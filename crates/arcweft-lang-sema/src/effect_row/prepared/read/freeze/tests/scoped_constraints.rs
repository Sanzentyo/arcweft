use super::*;
use crate::{
    effect_row::prepared::test_support::open,
    types::{GenericBinder, GenericScope},
};

#[test]
fn a_frozen_branch_conjoins_its_r_with_original_c_without_mutating_other_branches() {
    for conflict in [false, true] {
        let mut graph = PreparedEffectGraph::default();
        let mut control = budget();
        let output = graph
            .declare_unknown(
                vec![],
                PreparedEffectUnknownEligibility::Bindable,
                &mut control,
            )
            .unwrap();
        let value = graph
            .unknown_application(&output, vec![], &mut control)
            .unwrap();
        let original = graph
            .relate(
                PreparedEffectRelation::Equal,
                value.clone(),
                literal(&["fs.read"]),
                &mut control,
            )
            .unwrap();
        graph
            .add_scoped_obligation(vec![], truth(), original, &mut control)
            .unwrap();
        let branch_r = graph
            .relate(
                PreparedEffectRelation::Equal,
                value.clone(),
                literal(if conflict {
                    &["log.write"]
                } else {
                    &["fs.read"]
                }),
                &mut control,
            )
            .unwrap();
        let branch = graph
            .read()
            .scoped_constraint(&[], &truth(), &branch_r, &mut control)
            .unwrap();
        let state = graph.state;
        let terms = graph.retained_terms();
        let frozen = graph
            .read()
            .freeze_scoped(&[&value], &[], &[&branch], &mut control)
            .unwrap();
        assert_eq!(
            frozen.graph.obligations.len(),
            graph.relation.obligations.len() + 1
        );
        assert_eq!(
            frozen
                .reduce(&mut control)
                .unwrap()
                .satisfiable()
                .truth_value(),
            Some(!conflict)
        );
        let repeated = graph
            .read()
            .freeze_scoped(&[&value], &[], &[&branch], &mut control)
            .unwrap();
        assert!(
            frozen
                .graph
                .same_original(&repeated.graph, &mut control)
                .unwrap()
        );
        assert_eq!(graph.state, state);
        assert_eq!(graph.retained_terms(), terms);
        // The other branch still has only the original C.
        assert_eq!(
            graph
                .solve_principal(&mut control)
                .unwrap()
                .unknown(&output)
                .unwrap(),
            &EffectRow::closed(crate::effects::EffectSet::from_labels(["fs.read"]).unwrap())
        );
    }
}

#[test]
fn whole_row_implication_and_unused_gamma_survive_the_frozen_scope() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let gamma = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let parameters = [
        gamma.bound_effect(0, 0).unwrap(),
        gamma.bound_effect(0, 1).unwrap(),
    ];
    // D is false for the whole finite row; this is a vacuous obligation even
    // though R is false too. The unused second input remains in the real Γ.
    let impossible = graph.not(truth(), &mut control).unwrap();
    let branch = graph
        .read()
        .scoped_constraint(&parameters, &impossible, &impossible, &mut control)
        .unwrap();
    let frozen = graph
        .read()
        .freeze_scoped(&[], &[], &[&branch], &mut control)
        .unwrap();
    let retained = frozen.graph.obligations.last().unwrap();
    assert_eq!(retained.parameters.as_ref(), &parameters);
    assert_eq!(retained.domain, impossible);
    assert_eq!(retained.predicate, impossible);
    assert_eq!(
        frozen
            .reduce(&mut control)
            .unwrap()
            .satisfiable()
            .truth_value(),
        Some(true)
    );
    assert!(graph.relation.obligations.is_empty());
}

#[test]
fn incomplete_gamma_foreign_receipts_and_stale_predicates_are_rejected() {
    let mut graph = PreparedEffectGraph::default();
    let foreign = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let parameter = scope.bound_effect(0, 0).unwrap();
    let predicate = graph
        .relate(
            PreparedEffectRelation::Equal,
            open(&parameter),
            literal(&[]),
            &mut control,
        )
        .unwrap();
    assert!(matches!(
        graph
            .read()
            .scoped_constraint(&[], &truth(), &predicate, &mut control),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ParameterContract
        ))
    ));
    assert!(matches!(
        graph.read().scoped_constraint(
            &[parameter.clone(), parameter],
            &truth(),
            &truth(),
            &mut control
        ),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::DuplicateParameter
        ))
    ));
    let foreign_receipt = foreign
        .read()
        .scoped_constraint(&[], &truth(), &truth(), &mut control)
        .unwrap();
    assert!(matches!(
        graph
            .read()
            .freeze_scoped(&[], &[], &[&foreign_receipt], &mut control),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ForeignIssuer
        ))
    ));
    let mut checkpoint = graph.checkpoint(&mut control).unwrap();
    let issued = graph.and(vec![], &mut control).unwrap();
    let stale = graph
        .read()
        .scoped_constraint(&[], &truth(), &issued, &mut control)
        .unwrap();
    graph.rollback(&mut checkpoint).unwrap();
    let state = graph.state;
    let terms = graph.retained_terms();
    assert!(matches!(
        graph
            .read()
            .freeze_scoped(&[], &[], &[&stale], &mut control),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::StaleReference
        ))
    ));
    assert_eq!(graph.state, state);
    assert_eq!(graph.retained_terms(), terms);
}

#[test]
fn every_scope_and_freeze_failure_preserves_original_state_and_spends_work() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let parameters = [scope.bound_effect(0, 0).unwrap()];
    let predicate = graph
        .relate(
            PreparedEffectRelation::Equal,
            open(&parameters[0]),
            literal(&[]),
            &mut control,
        )
        .unwrap();
    let mut checkpoint = graph.checkpoint(&mut control).unwrap();
    let state = graph.state;
    let terms = graph.retained_terms();
    let mut measured = Budget(10_000);
    let receipt = graph
        .read()
        .scoped_constraint(&parameters, &truth(), &predicate, &mut measured)
        .unwrap();
    let required = 10_000 - measured.0;
    for available in 0..required {
        let mut work = Budget(available);
        assert!(matches!(
            graph
                .read()
                .scoped_constraint(&parameters, &truth(), &predicate, &mut work),
            Err(PreparedEffectFailure::Control(Exhausted))
        ));
        assert_eq!(work.0, 0);
        assert_eq!(graph.state, state);
        assert_eq!(graph.retained_terms(), terms);
    }
    let mut measured = Budget(10_000);
    graph
        .read()
        .freeze_scoped(&[], &[], &[&receipt], &mut measured)
        .unwrap();
    let required = 10_000 - measured.0;
    for available in 0..required {
        let mut work = Budget(available);
        assert!(matches!(
            graph.read().freeze_scoped(&[], &[], &[&receipt], &mut work),
            Err(PreparedEffectFailure::Control(Exhausted))
        ));
        assert_eq!(work.0, 0);
        assert_eq!(graph.state, state);
        assert_eq!(graph.retained_terms(), terms);
    }
    graph.rollback(&mut checkpoint).unwrap();
}
