use super::*;
use crate::{
    effect_row::{
        DecisionControl, DecisionWork, EffectPredicate, EffectRow,
        prepared::{PreparedEffectGraph, PreparedEffectRelation, PreparedEffectRow},
    },
    effects::EffectSet,
    types::constraints::{
        NoConstraintClient,
        context::{LocalConstraintAccounting, TypeConstraintLimits},
        test_support::ConstraintTestSetup,
    },
};
use std::sync::atomic::AtomicBool;

struct Preparation;
impl DecisionControl for Preparation {
    type Error = std::convert::Infallible;
    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn original_r_transport_keeps_actual_frames_and_the_unprepared_scheme() {
    let cancellation = AtomicBool::new(false);
    let mut context: TypeConstraintContext<'_, LocalConstraintAccounting<'_>, NoConstraintClient> =
        ConstraintTestSetup::new(
            TypeConstraintLimits::new(65_536, 32_768, 1024, 1024),
            &cancellation,
        )
        .into_parts()
        .0;
    let binder = GenericBinder::new(0, 0, 1);
    let original = ConstraintLexicalScope::default().with_binder(binder);
    let unrelated = ConstraintLexicalScope::default().with_binder(binder);
    let mut joint = original.clone();
    joint.merge(&unrelated, &mut context).unwrap();
    let mut graph = PreparedEffectGraph::default();
    let formal = original.shape().bound_effect(0, 0).unwrap();
    let scheme = graph
        .declare(
            vec![formal.clone()],
            EffectPredicate::unconstrained(),
            &mut Preparation,
        )
        .unwrap();
    let input = PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), formal));
    let call = graph
        .application(&scheme, vec![input.clone()], &mut Preparation)
        .unwrap();
    let predicate = graph
        .relate(
            PreparedEffectRelation::Equal,
            call.clone(),
            input,
            &mut Preparation,
        )
        .unwrap();
    let projected = joint
        .transport_prepared_predicate(&original, &predicate, &mut graph, &mut context)
        .unwrap();
    let mut references = std::collections::BTreeSet::new();
    graph
        .read()
        .visit_predicate_parameters(&projected, &mut Preparation, &mut |reference, _| {
            references.insert(reference.clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(
        references,
        std::collections::BTreeSet::from([joint.shape().bound_effect(1, 0).unwrap()])
    );
    let terms = graph.retained_terms();
    assert!(
        unrelated
            .transport_prepared_predicate(&original, &predicate, &mut graph, &mut context)
            .is_err()
    );
    assert_eq!(graph.retained_terms(), terms);
    graph
        .read()
        .validate_retained_predicate(&predicate)
        .unwrap();
    // The actual declaration still accepts its single original body after
    // both transports; neither transport prepares or replaces it.
    graph
        .prepare_body(
            &scheme,
            PreparedEffectRow::Canonical(EffectRow::closed(EffectSet::new())),
            vec![],
            &mut Preparation,
        )
        .unwrap();
    assert_eq!(
        graph
            .solve_principal(&mut Preparation)
            .unwrap()
            .resolve_row(&call, &mut Preparation)
            .unwrap(),
        EffectRow::closed(EffectSet::new())
    );
}
