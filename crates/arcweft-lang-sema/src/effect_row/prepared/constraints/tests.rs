use super::*;

mod predicate_equality;
use crate::{
    effect_row::EffectConstraintEligibility,
    effects::EffectSet,
    types::{
        DetachedGenericOwnerId, GenericBinder, GenericEffectParameterId, GenericParameterOwnerId,
        GenericScope,
    },
};

#[derive(Debug, PartialEq, thiserror::Error)]
enum Failure {
    #[error("original work exhausted")]
    Exhausted,
    #[error(transparent)]
    Scope(#[from] EffectConstraintEnvironmentError),
}

struct Control(usize);
impl DecisionControl for Control {
    type Error = Failure;
    fn charge(&mut self, _: DecisionWork) -> Result<(), Failure> {
        self.0 = self.0.checked_sub(1).ok_or(Failure::Exhausted)?;
        Ok(())
    }
}

fn row(labels: &[&str]) -> PreparedEffectRow {
    PreparedEffectRow::Canonical(EffectRow::closed(
        EffectSet::from_labels(labels.iter().copied()).unwrap(),
    ))
}

fn environment(control: &mut Control) -> EffectConstraintEnvironment<PreparedEffects> {
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(91_817));
    let variables = [0, 1].map(|slot| {
        EffectConstraintVariable::new(
            GenericEffectParameterId::new(owner.clone(), slot).into(),
            EffectConstraintEligibility::Bindable,
        )
    });
    EffectConstraintEnvironment::new_prepared(
        &variables,
        &EffectPredicate::unconstrained(),
        control,
    )
    .unwrap()
}

#[test]
fn original_relation_is_retained_before_the_body_and_rejects_a_later_conflict() {
    for conflict in [false, true] {
        let mut control = Control(1_000_000);
        let mut graph = PreparedEffectGraph::default();
        let mut environment = environment(&mut control);
        let formal = GenericScope::default()
            .with_binder(GenericBinder::new(0, 0, 1))
            .bound_effect(0, 0)
            .unwrap();
        let scheme = graph
            .declare(vec![formal], EffectPredicate::unconstrained(), &mut control)
            .unwrap();
        let argument = environment.original_variables().next().unwrap().clone();
        let call = graph
            .application(
                &scheme,
                vec![PreparedEffectRow::Canonical(EffectRow::open(
                    EffectSet::new(),
                    argument,
                ))],
                &mut control,
            )
            .unwrap();
        environment
            .constrain_relation(
                &mut graph,
                PreparedEffectRelation::Equal,
                &call,
                &row(if conflict { &[] } else { &["fs.read"] }),
                &mut control,
            )
            .unwrap();
        assert!(matches!(
            graph.scheme(&scheme).unwrap().state,
            SchemeState::Declared
        ));
        graph
            .prepare_body(&scheme, row(&["fs.read"]), vec![], &mut control)
            .unwrap();
        graph
            .add_scoped_obligation(
                environment.original_variables().cloned().collect(),
                PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
                environment.original_predicate().clone(),
                &mut control,
            )
            .unwrap();
        let completed = graph.solve_principal(&mut control);
        if conflict {
            assert!(matches!(
                completed,
                Err(PreparedEffectFailure::Unsatisfiable)
            ));
        } else {
            assert_eq!(
                completed.unwrap().resolve_row(&call, &mut control).unwrap(),
                EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap())
            );
        }
    }
}

#[test]
fn every_failed_relation_edit_preserves_the_original_r_and_graph_but_spends_work() {
    let mut control = Control(1_000_000);
    let mut graph = PreparedEffectGraph::default();
    let seed = environment(&mut control);
    let reference = seed.original_variables().next().unwrap().clone();
    let actual = PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), reference));
    let mut checkpoint = graph.checkpoint(&mut control).unwrap();
    let mut probe = seed.clone();
    let mut required = Control(4096);
    probe
        .constrain_relation(
            &mut graph,
            PreparedEffectRelation::AdmittedSubset,
            &actual,
            &row(&["fs.read"]),
            &mut required,
        )
        .unwrap();
    let required = 4096 - required.0;
    graph.rollback(&mut checkpoint).unwrap();
    let state = graph.state;
    let terms = graph.retained_terms();
    for available in 0..=required {
        let mut checkpoint = graph.checkpoint(&mut control).unwrap();
        let mut branch = seed.clone();
        let mut work = Control(available);
        let result = branch.constrain_relation(
            &mut graph,
            PreparedEffectRelation::AdmittedSubset,
            &actual,
            &row(&["fs.read"]),
            &mut work,
        );
        if available < required {
            assert!(matches!(
                result,
                Err(PreparedEffectFailure::Control(Failure::Exhausted))
            ));
            assert_eq!(branch, seed);
            assert_eq!(graph.state, state);
            assert_eq!(graph.retained_terms(), terms);
            assert_eq!(work.0, 0);
        } else {
            result.unwrap();
            assert_ne!(branch, seed);
        }
        graph.rollback(&mut checkpoint).unwrap();
        assert_eq!(graph.state, state);
        assert_eq!(graph.retained_terms(), terms);
    }
}

#[test]
fn inherited_rows_and_restored_predicates_keep_the_original_inventory_and_exact_equalities() {
    let mut control = Control(1_000_000);
    let mut graph = PreparedEffectGraph::default();
    let mut environment = environment(&mut control);
    let reference = environment.original_variables().next().unwrap().clone();
    environment
        .restore_completed_inherited(
            reference.clone(),
            &row(&["fs.read"]),
            &mut graph,
            &mut control,
        )
        .unwrap();
    let before = environment.clone();
    let state = graph.state;
    let terms = graph.retained_terms();
    assert!(matches!(
        environment.restore_completed_inherited(reference, &row(&[]), &mut graph, &mut control),
        Err(PreparedEffectFailure::Control(Failure::Scope(
            EffectConstraintEnvironmentError::NonCanonicalScope
        )))
    ));
    assert_eq!(environment, before);
    assert_eq!(graph.state, state);
    assert_eq!(graph.retained_terms(), terms);
    let foreign_owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(91_818));
    let foreign = GenericEffectParameterId::new(foreign_owner, 0).into();
    let predicate = graph
        .relate(
            PreparedEffectRelation::Equal,
            PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), foreign)),
            row(&[]),
            &mut control,
        )
        .unwrap();
    let state = graph.state;
    assert!(matches!(
        environment.restore_predicate(&predicate, &mut graph, &mut control),
        Err(PreparedEffectFailure::Control(Failure::Scope(
            EffectConstraintEnvironmentError::ForeignVariable { .. }
        )))
    ));
    assert_eq!(environment, before);
    assert_eq!(graph.state, state);
    let impossible = EffectFormula::literal(EffectSet::from_labels(["fs.read"]).unwrap(), None)
        .subset(
            &EffectFormula::literal(EffectSet::new(), None),
            &mut control,
        )
        .unwrap();
    environment
        .restore_predicate(
            &PreparedEffectPredicate::Canonical(impossible),
            &mut graph,
            &mut control,
        )
        .unwrap();
    graph
        .add_scoped_obligation(
            environment.original_variables().cloned().collect(),
            PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            environment.original_predicate().clone(),
            &mut control,
        )
        .unwrap();
    assert!(matches!(
        graph.solve_principal(&mut control),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
}
