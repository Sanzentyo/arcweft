use super::*;
use crate::{
    effect_row::prepared::{
        BoundScopedEffectCompletion, PreparedEffectFailure, PreparedEffectGraph,
        PreparedEffectInvariant, PreparedEffectPredicate, PreparedEffectRow,
        PreparedEffectUnknownEligibility, PreparedEffectUnknownRef,
    },
    types::{
        DetachedGenericOwnerId, GenericBinder, GenericConstParameterId, GenericEffectParameterId,
        GenericParameterOwnerId, GenericTypeParameterId, PreparedEffects,
        constraints::{
            NoConstraintClient, TypeConstraintParameterScope,
            context::{LocalConstraintAccounting, TypeConstraintLimits},
            normalization::close_type_operand,
            test_support::ConstraintTestSetup,
        },
        effect_domain::PreparedTypeEffectRoots,
    },
};
use std::sync::atomic::AtomicBool;

type Context<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;
type Path = ConstraintPath<NoConstraintClient>;

fn fixture<'a>(
    cancellation: &'a AtomicBool,
    parameters: TypeConstraintParameterScope,
    graph: &mut PreparedEffectGraph,
) -> (Context<'a>, Path) {
    ConstraintTestSetup::with_scope(
        TypeConstraintLimits::new(131_072, 65_536, 1024, 1024),
        cancellation,
        parameters,
    )
    .into_path(graph)
}

fn original(
    path: &Path,
    value: &TypeKind<PreparedEffects>,
    graph: &mut PreparedEffectGraph,
    context: &mut Context<'_>,
) -> BoundScopedEffectCompletion {
    let receipt = path.prepare_effect_constraint(graph, context).unwrap();
    let roots = PreparedTypeEffectRoots::collect([value], std::iter::empty(), context).unwrap();
    let frozen = graph
        .read()
        .freeze_scoped(roots.rows(), roots.predicates(), &[&receipt], context)
        .unwrap();
    graph
        .bind_original_scoped(&frozen, context)
        .unwrap()
        .freeze(graph.read(), context)
        .unwrap()
}

fn arrow(row: PreparedEffectRow) -> TypeKind<PreparedEffects> {
    TypeKind::function_with_contract(
        GenericBinder::EMPTY,
        PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        [],
        TypeKind::I64,
        row,
    )
}

#[test]
fn completed_reification_uses_the_original_named_closure_and_actual_operand() {
    let cancellation = AtomicBool::new(false);
    let mut graph = PreparedEffectGraph::default();
    let (mut context, path) = fixture(
        &cancellation,
        TypeConstraintParameterScope::empty(),
        &mut graph,
    );
    let named = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut context)
        .unwrap();
    let untouched = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut context)
        .unwrap();
    let actual = EffectRow::closed(EffectSet::from_labels(["body.actual"]).unwrap());
    graph
        .prepare_body(
            &named,
            PreparedEffectRow::Canonical(actual.clone()),
            vec![],
            &mut context,
        )
        .unwrap();
    let value = arrow(graph.application(&named, vec![], &mut context).unwrap());
    let original = original(&path, &value, &mut graph, &mut context);
    let effects =
        ConstraintClosedEffects::for_path(&path, &original, &[], &mut graph, &mut context).unwrap();
    let operand = context.fixture_operand(value);
    let closed =
        close_type_operand(operand.view(), &path, &effects, graph.read(), &mut context).unwrap();
    let application = path.root_scope();
    let generic =
        CompletedGenericContext::for_application(&path, application, &mut context).unwrap();
    let receiving = context.enter_binder_scope(GenericBinder::new(1, 1, 1));
    let frames = context.capture_lexical_scope().unwrap();
    let completed = generic
        .reify_type(
            &closed,
            &path,
            application,
            None,
            &effects,
            graph.read(),
            &mut context,
        )
        .unwrap();
    let TypeKind::Function { effects: row, .. } = completed.value() else {
        panic!("canonical function")
    };
    assert_eq!(row, &actual);
    assert!(completed.captures().parameters().is_empty());
    assert_eq!(context.capture_lexical_scope().unwrap(), frames);
    context.restore_binder_scope(receiving);
    graph
        .prepare_body(
            &untouched,
            PreparedEffectRow::Canonical(EffectRow::closed(EffectSet::new())),
            vec![],
            &mut context,
        )
        .unwrap();
}

#[test]
fn completed_reification_requires_the_actual_future_row_and_can_retry() {
    let cancellation = AtomicBool::new(false);
    let mut graph = PreparedEffectGraph::default();
    let (mut context, path) = fixture(
        &cancellation,
        TypeConstraintParameterScope::empty(),
        &mut graph,
    );
    let future = graph
        .declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Future,
            &mut context,
        )
        .unwrap();
    let value = arrow(
        graph
            .unknown_application(&future, vec![], &mut context)
            .unwrap(),
    );
    let original = original(&path, &value, &mut graph, &mut context);
    let before = graph.retained_terms();
    assert!(matches!(
        ConstraintClosedEffects::for_path(&path, &original, &[], &mut graph, &mut context),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::PreparedEffect(PreparedEffectFailure::Invariant(
                PreparedEffectInvariant::UnresolvedFuture
            ))
        ))
    ));
    assert_eq!(graph.retained_terms(), before);
    let actual = EffectRow::closed(EffectSet::from_labels(["future.actual"]).unwrap());
    let inputs: [(&PreparedEffectUnknownRef, &EffectRow); 1] = [(&future, &actual)];
    let effects =
        ConstraintClosedEffects::for_path(&path, &original, &inputs, &mut graph, &mut context)
            .unwrap();
    let operand = context.fixture_operand(value);
    let closed =
        close_type_operand(operand.view(), &path, &effects, graph.read(), &mut context).unwrap();
    let application = path.root_scope();
    let generic =
        CompletedGenericContext::for_application(&path, application, &mut context).unwrap();
    let completed = generic
        .reify_type(
            &closed,
            &path,
            application,
            None,
            &effects,
            graph.read(),
            &mut context,
        )
        .unwrap();
    let TypeKind::Function { effects: row, .. } = completed.value() else {
        panic!("canonical function")
    };
    assert_eq!(row, &actual);
}

#[test]
fn a_foreign_completion_cannot_reify_an_equal_type() {
    let cancellation = AtomicBool::new(false);
    let mut graph = PreparedEffectGraph::default();
    let (mut context, path) = fixture(
        &cancellation,
        TypeConstraintParameterScope::empty(),
        &mut graph,
    );
    let value = TypeKind::<PreparedEffects>::Bool;
    let source = original(&path, &value, &mut graph, &mut context);
    let effects =
        ConstraintClosedEffects::for_path(&path, &source, &[], &mut graph, &mut context).unwrap();
    let (mut other, other_path) = fixture(
        &cancellation,
        TypeConstraintParameterScope::empty(),
        &mut graph,
    );
    let application = other_path.root_scope();
    let generic =
        CompletedGenericContext::for_application(&other_path, application, &mut other).unwrap();
    let operand = other.fixture_operand(value);
    assert!(matches!(
        generic.reify_type(
            &operand,
            &other_path,
            application,
            None,
            &effects,
            graph.read(),
            &mut other
        ),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::OriginalProducer
        ))
    ));
}

#[test]
fn a_receivers_type_declaration_does_not_supply_the_operands_free_authority() {
    let cancellation = AtomicBool::new(false);
    let mut graph = PreparedEffectGraph::default();
    let parameter = GenericTypeParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(98_930)),
        0,
    );
    let parameters = TypeConstraintParameterScope::new([(
        parameter.clone(),
        TypeConstraintParameterEligibility::Rigid,
    )])
    .unwrap();
    let (mut context, path) = fixture(&cancellation, parameters, &mut graph);
    let reference: GenericTypeReference = parameter.into();
    let value = TypeKind::<PreparedEffects>::GenericParam(reference.clone());
    let source = original(&path, &value, &mut graph, &mut context);
    let effects =
        ConstraintClosedEffects::for_path(&path, &source, &[], &mut graph, &mut context).unwrap();
    let application = path.root_scope();
    assert!(
        application
            .parameters()
            .type_reference(&reference)
            .is_some()
    );
    let generic =
        CompletedGenericContext::for_application(&path, application, &mut context).unwrap();
    let operand = context.fixture_operand(value);
    assert_eq!(
        generic.reify_type(
            &operand,
            &path,
            application,
            None,
            &effects,
            graph.read(),
            &mut context
        ),
        Err(references::type_out_of_scope(&reference))
    );
}

#[test]
fn completed_future_ports_use_their_own_scope_and_capture() {
    let cancellation = AtomicBool::new(false);
    let mut graph = PreparedEffectGraph::default();
    let (mut context, path) = fixture(
        &cancellation,
        TypeConstraintParameterScope::empty(),
        &mut graph,
    );
    let generic =
        CompletedGenericContext::for_application(&path, path.root_scope(), &mut context).unwrap();
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(98_931));
    let ty: GenericTypeReference = GenericTypeParameterId::new(owner.clone(), 0).into();
    let length: GenericConstReference = GenericConstParameterId::new(owner.clone(), 0).into();
    let effect: GenericEffectReference = GenericEffectParameterId::new(owner, 0).into();
    for (value, parameter) in [
        (TypeKind::GenericParam(ty.clone()), ty.into()),
        (
            TypeKind::Array {
                item: Box::new(TypeKind::Bool),
                len: ArrayLength::Generic(length.clone()),
            },
            length.into(),
        ),
        (
            TypeKind::function_with_effects(
                [],
                TypeKind::Unit,
                EffectRow::open(EffectSet::new(), effect.clone()),
            ),
            effect.into(),
        ),
    ] {
        let mut completed = CompletedValue {
            value,
            captures: RetainedRigidGenericScope::from_admitted(BTreeSet::new()),
        };
        assert!(
            generic
                .validate_reified_future_type(&completed, &mut context)
                .is_err()
        );
        completed.captures = RetainedRigidGenericScope::from_admitted(BTreeSet::from([parameter]));
        generic
            .validate_reified_future_type(&completed, &mut context)
            .unwrap();
    }
    let binder = GenericBinder::new(1, 1, 1);
    let local = generic.value_scope().with_binder(binder);
    let completed = CompletedValue {
        value: TypeKind::function_with_contract(
            binder,
            EffectPredicate::unconstrained(),
            [TypeKind::Array {
                item: Box::new(TypeKind::GenericParam(local.bound_type(0, 0).unwrap())),
                len: ArrayLength::Generic(local.bound_const(0, 0).unwrap()),
            }],
            TypeKind::Unit,
            EffectRow::open(EffectSet::new(), local.bound_effect(0, 0).unwrap()),
        ),
        captures: RetainedRigidGenericScope::from_admitted(BTreeSet::new()),
    };
    generic
        .validate_reified_future_type(&completed, &mut context)
        .unwrap();
    // Future-result annotations retain the existing omitted-row policy.
    let omitted = CompletedValue {
        value: TypeKind::function_with_effects([], TypeKind::Unit, EffectRow::unknown()),
        captures: RetainedRigidGenericScope::from_admitted(BTreeSet::new()),
    };
    generic
        .validate_reified_future_type(&omitted, &mut context)
        .unwrap();
    for value in [
        TypeKind::Named("_".into()),
        TypeKind::Error(crate::types::TypePoisonId::from_index(98_932)),
        TypeKind::Array {
            item: Box::new(TypeKind::Bool),
            len: ArrayLength::Inferred,
        },
        TypeKind::GenericParam(local.bound_type(0, 0).unwrap()),
    ] {
        let invalid = CompletedValue {
            value,
            captures: RetainedRigidGenericScope::from_admitted(BTreeSet::new()),
        };
        assert!(
            generic
                .validate_reified_future_type(&invalid, &mut context)
                .is_err()
        );
    }
}
