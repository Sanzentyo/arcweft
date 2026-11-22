use super::*;
use crate::{
    effect_row::{
        EffectPredicate, EffectRow,
        prepared::{PreparedEffectRelation, PreparedEffectUnknownEligibility},
    },
    effects::EffectSet,
    types::{
        constraints::{
            NoConstraintClient, TypeConstraintAbort,
            context::{LocalConstraintAccounting, TypeConstraintLimits},
        },
        effect_domain::PreparedTypeEffectRoots,
    },
};
use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicBool, Ordering},
};

type Context<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;

fn context(cancellation: &AtomicBool) -> Context<'_> {
    TypeConstraintContext::with_accounting(LocalConstraintAccounting::new(
        TypeConstraintLimits::new(131_072, 65_536, 1024, 1024),
        cancellation,
    ))
}

#[test]
fn live_transport_preserves_named_and_future_functions_under_real_frame_permutation() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let effects = ConstraintLexicalScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let types = ConstraintLexicalScope::default().with_binder(GenericBinder::new(1, 0, 0));
    let mut source = effects.clone();
    source.merge(&types, &mut context).unwrap();
    let mut target = types.clone();
    target.merge(&effects, &mut context).unwrap();
    let formal_scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let formal = formal_scope.bound_effect(0, 0).unwrap();
    let named = graph
        .declare(
            vec![formal.clone()],
            EffectPredicate::unconstrained(),
            &mut context,
        )
        .unwrap();
    let future = graph
        .declare_unknown(
            vec![formal.clone()],
            PreparedEffectUnknownEligibility::Future,
            &mut context,
        )
        .unwrap();
    let local = GenericBinder::new(1, 1, 1);
    let source_nested = source.shape().with_binder(local);
    let outer = PreparedEffectRow::Canonical(EffectRow::open(
        EffectSet::new(),
        source_nested.bound_effect(2, 0).unwrap(),
    ));
    let own = PreparedEffectRow::Canonical(EffectRow::open(
        EffectSet::new(),
        source_nested.bound_effect(0, 0).unwrap(),
    ));
    let named_row = graph
        .application(&named, vec![outer], &mut context)
        .unwrap();
    let future_row = graph
        .unknown_application(&future, vec![own], &mut context)
        .unwrap();
    let row = graph
        .union(vec![named_row.clone(), future_row.clone()], &mut context)
        .unwrap();
    let predicate = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            named_row,
            future_row,
            &mut context,
        )
        .unwrap();
    let input: TypeKind<PreparedEffects> = TypeKind::function_with_contract(
        local,
        predicate,
        [TypeKind::Array {
            item: Box::new(TypeKind::GenericParam(
                source_nested.bound_type(0, 0).unwrap(),
            )),
            len: ArrayLength::Generic(source_nested.bound_const(0, 0).unwrap()),
        }],
        TypeKind::GenericParam(source_nested.bound_type(1, 0).unwrap()),
        row,
    );
    let before = input.clone();
    let frames = context.lexical_frames().clone();
    let retained_terms = graph.retained_terms();
    let same_scope = source
        .transport_type(&source, &input, &mut graph, &mut context)
        .unwrap();
    assert_eq!(same_scope, input);
    assert_eq!(graph.retained_terms(), retained_terms);

    let output = target
        .transport_type(&source, &input, &mut graph, &mut context)
        .unwrap();
    assert_eq!(input, before);
    assert_eq!(context.lexical_frames(), &frames);
    let TypeKind::Function {
        binder,
        params,
        return_type,
        effects: row,
        predicate,
    } = &output
    else {
        panic!("prepared function")
    };
    let target_nested = target.shape().with_binder(*binder);
    assert_eq!(
        return_type.as_ref(),
        &TypeKind::GenericParam(target_nested.bound_type(2, 0).unwrap())
    );
    assert_eq!(
        params[0],
        TypeKind::Array {
            item: Box::new(TypeKind::GenericParam(
                target_nested.bound_type(0, 0).unwrap()
            )),
            len: ArrayLength::Generic(target_nested.bound_const(0, 0).unwrap())
        }
    );
    let expected = BTreeSet::from([
        target_nested.bound_effect(1, 0).unwrap(),
        target_nested.bound_effect(0, 0).unwrap(),
    ]);
    let mut references = BTreeSet::new();
    graph
        .read()
        .visit_row_parameters(row, &mut context, &mut |reference, _| {
            references.insert(reference.clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(references, expected);
    references.clear();
    graph
        .read()
        .visit_predicate_parameters(predicate, &mut context, &mut |reference, _| {
            references.insert(reference.clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(references, expected);
    // Neither an unprepared Named body nor an actual Future row was needed
    // for transport. They are supplied only to the original final closure.
    graph
        .prepare_body(
            &named,
            PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), formal)),
            vec![],
            &mut context,
        )
        .unwrap();
    let roots =
        PreparedTypeEffectRoots::collect([&output], std::iter::empty(), &mut context).unwrap();
    let original = graph
        .read()
        .freeze_scoped(roots.rows(), roots.predicates(), &[], &mut context)
        .unwrap();
    let actual = EffectRow::closed(EffectSet::from_labels(["future.actual"]).unwrap());
    let canonical = output
        .close_original_effects(
            target.shape(),
            &original,
            graph.read(),
            &[(&future, &actual)],
            &mut context,
        )
        .unwrap();
    let TypeKind::Function {
        effects: result, ..
    } = canonical.value()
    else {
        panic!("canonical function")
    };
    let expected = actual
        .union(
            &EffectRow::open(EffectSet::new(), target_nested.bound_effect(1, 0).unwrap()),
            &mut context,
        )
        .unwrap();
    assert_eq!(result, &expected);
}

#[test]
fn a_late_scope_failure_and_cancellation_roll_back_all_transport_terms() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let target = ConstraintLexicalScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let types = ConstraintLexicalScope::default().with_binder(GenericBinder::new(1, 0, 0));
    let mut source = target.clone();
    source.merge(&types, &mut context).unwrap();
    let formal = GenericScope::default()
        .with_binder(GenericBinder::new(0, 0, 1))
        .bound_effect(0, 0)
        .unwrap();
    let named = graph
        .declare(vec![formal], EffectPredicate::unconstrained(), &mut context)
        .unwrap();
    let row = graph
        .application(
            &named,
            vec![PreparedEffectRow::Canonical(EffectRow::open(
                EffectSet::new(),
                source.shape().bound_effect(1, 0).unwrap(),
            ))],
            &mut context,
        )
        .unwrap();
    let function: TypeKind<PreparedEffects> = TypeKind::function_with_contract(
        GenericBinder::EMPTY,
        PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        [],
        TypeKind::Unit,
        row,
    );
    let retained = graph.retained_terms();
    let mut checkpoint = graph.checkpoint(&mut context).unwrap();
    target
        .transport_type(&source, &function, &mut graph, &mut context)
        .unwrap();
    assert_ne!(graph.retained_terms(), retained);
    graph.rollback(&mut checkpoint).unwrap();
    assert_eq!(graph.retained_terms(), retained);
    let input = TypeKind::Tuple(vec![
        function.clone(),
        TypeKind::GenericParam(source.shape().bound_type(0, 0).unwrap()),
    ]);
    let before = input.clone();
    let frames = context.lexical_frames().clone();
    assert!(matches!(
        target.transport_type(&source, &input, &mut graph, &mut context),
        Err(TypeProjectionError::Instantiation(
            TypeInstantiationError::SpecializationScopeMismatch
        ))
    ));
    assert_eq!(input, before);
    assert_eq!(graph.retained_terms(), retained);
    assert_eq!(context.lexical_frames(), &frames);
    cancellation.store(true, Ordering::Relaxed);
    assert!(matches!(
        target.transport_type(&source, &function, &mut graph, &mut context),
        Err(TypeProjectionError::Control(TypeConstraintError::Abort(
            TypeConstraintAbort::Cancelled
        )))
    ));
    assert_eq!(graph.retained_terms(), retained);
    assert_eq!(context.lexical_frames(), &frames);
    cancellation.store(false, Ordering::Relaxed);
    // Equal shapes from different actual openings are also rejected.
    let foreign = ConstraintLexicalScope::default().with_binder(GenericBinder::new(0, 0, 1));
    assert!(
        foreign
            .ambient_depth(&target, 0, 0, 0, &mut context)
            .is_err()
    );
}
