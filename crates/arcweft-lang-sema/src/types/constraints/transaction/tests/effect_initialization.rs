use super::*;
use crate::effect_row::prepared::{
    PreparedEffectGraph, PreparedEffectInvariant, PreparedEffectRelation, PreparedEffectRow,
};

#[test]
fn initialization_publishes_the_new_r_only_with_its_transaction() {
    let cancellation = AtomicBool::new(false);
    for fail_after_restore in [false, true] {
        let (mut context, mut path): (_, ConstraintPath<NoConstraintClient>) =
            ConstraintTestSetup::new(
                TypeConstraintLimits::new(65_536, 32_768, 1024, 1024),
                &cancellation,
            )
            .into_path();
        let mut graph = PreparedEffectGraph::default();
        let pure = PreparedEffectRow::Canonical(EffectRow::closed(EffectSet::new()));
        let original = graph.union(vec![pure.clone()], &mut context).unwrap();
        let terms = graph.retained_terms();
        let mut issued = None;
        let result = context.with_effect_transaction(&mut graph, |context, graph| {
            let predicate = graph.relate(
                PreparedEffectRelation::Equal,
                original.clone(),
                pure,
                context,
            )?;
            path.effects.restore_predicate(&predicate, graph, context)?;
            issued = Some(path.effects.original_predicate().clone());
            let application = path.root_id();
            let transaction =
                TypeConstraintTransaction::from_path(context, graph, application, path, None)?;
            if fail_after_restore {
                return Err(TypeConstraintAbort::Cancelled.into());
            }
            Ok(transaction)
        });
        graph.read().validate_retained_row(&original).unwrap();
        if fail_after_restore {
            assert!(matches!(
                result,
                Err(TypeConstraintError::Abort(TypeConstraintAbort::Cancelled))
            ));
            assert_eq!(graph.retained_terms(), terms);
            assert_eq!(
                graph
                    .read()
                    .validate_retained_predicate(issued.as_ref().unwrap()),
                Err(PreparedEffectInvariant::StaleReference)
            );
        } else {
            let transaction = result.unwrap();
            graph
                .read()
                .validate_retained_predicate(transaction.frontier[0].effects.original_predicate())
                .unwrap();
            assert!(graph.retained_terms().1 > terms.1);
        }
    }
}

#[test]
fn cancelled_initialization_never_starts_its_graph_operation() {
    let cancellation = AtomicBool::new(true);
    let (mut context, _) = ConstraintTestSetup::<_, NoConstraintClient>::new(
        TypeConstraintLimits::new(65_536, 32_768, 1024, 1024),
        &cancellation,
    )
    .into_parts();
    let mut graph = PreparedEffectGraph::default();
    let result: Result<(), TypeConstraintError> = context
        .with_effect_transaction(&mut graph, |_, _| {
            panic!("cancelled initialization entered its operation")
        });
    assert!(matches!(
        result,
        Err(TypeConstraintError::Abort(TypeConstraintAbort::Cancelled))
    ));
    assert_eq!(graph.retained_terms(), (0, 0));
}
