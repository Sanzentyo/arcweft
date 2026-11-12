use super::*;
use crate::types::{
    DetachedGenericOwnerId, GenericConstParameterId, GenericParameterOwnerId,
    GenericTypeParameterId,
    constraints::{
        ConstraintAcceptance, ConstraintAliasResolution, TypeConstraintConstEligibility,
        TypeConstraintParameterEligibility, TypeConstraintParameterScope, relate_selected_call,
        resolve_const_alias, resolve_type_alias,
    },
};

type BindingSetup<'a> = (
    Context<'a>,
    ConstraintPath<NoConstraintClient>,
    Vec<GenericTypeReference>,
    Vec<GenericConstReference>,
);

pub(super) fn setup(cancellation: &AtomicBool, types: u16, lengths: u16) -> BindingSetup<'_> {
    setup_with_limits(
        cancellation,
        types,
        lengths,
        TypeConstraintLimits::new(4096, 2048, 128, 128),
    )
}

pub(super) fn setup_with_limits(
    cancellation: &AtomicBool,
    types: u16,
    lengths: u16,
    limits: TypeConstraintLimits,
) -> BindingSetup<'_> {
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(791));
    let types = (0..types)
        .map(|slot| GenericTypeParameterId::new(owner.clone(), slot))
        .collect::<Vec<_>>();
    let lengths = (0..lengths)
        .map(|slot| GenericConstParameterId::new(owner.clone(), slot))
        .collect::<Vec<_>>();
    let parameters = TypeConstraintParameterScope::new_with_constants(
        types
            .iter()
            .cloned()
            .map(|key| (key, TypeConstraintParameterEligibility::Bindable)),
        lengths
            .iter()
            .cloned()
            .map(|key| (key, TypeConstraintConstEligibility::Bindable)),
    )
    .unwrap();
    let types = types
        .into_iter()
        .map(|key| parameters.type_reference(&key.into()).unwrap())
        .collect();
    let lengths = lengths
        .into_iter()
        .map(|key| parameters.const_reference(&key.into()).unwrap())
        .collect();
    let (context, path) =
        ConstraintTestSetup::with_scope(limits, cancellation, parameters).into_path();
    (context, path, types, lengths)
}

#[test]
fn a_selected_relation_keeps_each_binding_on_its_actual_tuple_child_through_seal_and_fork() {
    let cancellation = AtomicBool::new(false);
    let (mut context, path, parameters, _) = setup(&cancellation, 2, 0);
    let pattern = context.fixture_operand(TypeKind::Tuple(
        parameters
            .iter()
            .cloned()
            .map(TypeKind::GenericParam)
            .collect(),
    ));
    let actual = context.fixture_operand(TypeKind::Tuple(vec![TypeKind::I64, TypeKind::String]));
    let retained_root = Arc::downgrade(&actual.value);
    let mut paths = relate_selected_call(
        &pattern.view(),
        &actual.view(),
        path,
        &mut context,
        ConstraintAcceptance::PatternAcceptsActual,
    )
    .unwrap();
    assert_eq!(paths.len(), 1);
    let path = paths.pop().unwrap();
    let first = &path.bindings[&parameters[0]];
    let second = &path.bindings[&parameters[1]];
    assert_eq!(first.value(), &TypeKind::I64);
    assert_eq!(second.value(), &TypeKind::String);
    assert_eq!(
        steps(first),
        [ConstraintOperandProjectionStep::TypeChild(0)]
    );
    assert_eq!(
        steps(second),
        [ConstraintOperandProjectionStep::TypeChild(1)]
    );
    let fork = context.fork_path(&path).unwrap();
    assert!(Arc::ptr_eq(
        &first.value,
        &fork.bindings[&parameters[0]].value
    ));
    assert!(Arc::ptr_eq(
        &second.origin,
        &fork.bindings[&parameters[1]].origin
    ));
    drop(pattern);
    drop(actual);
    assert!(retained_root.upgrade().is_some());
    drop(path);
    assert!(retained_root.upgrade().is_some());
    drop(fork);
    assert!(retained_root.upgrade().is_none());
}

#[test]
fn structural_relations_keep_the_direct_child_and_slice_branch_budgets() {
    let cancellation = AtomicBool::new(false);
    for (case, branches, bindings) in [(0, 0, 0), (1, 0, 1), (2, 1, 2), (3, 2, 2)] {
        let (mut context, path, parameters, _) = setup_with_limits(
            &cancellation,
            2,
            0,
            TypeConstraintLimits::new(4096, 2048, branches, 128),
        );
        let left = TypeKind::GenericParam(parameters[0].clone());
        let right = TypeKind::GenericParam(parameters[1].clone());
        let (pattern, actual) = match case {
            0 => (TypeKind::Tuple(vec![]), TypeKind::Tuple(vec![])),
            1 => (
                TypeKind::Vec(Box::new(left)),
                TypeKind::Vec(Box::new(TypeKind::I64)),
            ),
            2 => (
                TypeKind::Result {
                    ok: Box::new(left),
                    error: Box::new(right),
                },
                TypeKind::Result {
                    ok: Box::new(TypeKind::I64),
                    error: Box::new(TypeKind::String),
                },
            ),
            3 => (
                TypeKind::Tuple(vec![left, right]),
                TypeKind::Tuple(vec![TypeKind::I64, TypeKind::String]),
            ),
            _ => unreachable!(),
        };
        let pattern = context.fixture_operand(pattern);
        let actual = context.fixture_operand(actual);
        let paths = relate_selected_call(
            &pattern.view(),
            &actual.view(),
            path,
            &mut context,
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .unwrap();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].bindings.len(), bindings);
    }
    let (mut context, path, parameters, _) = setup_with_limits(
        &cancellation,
        2,
        0,
        TypeConstraintLimits::new(4096, 2048, 1, 128),
    );
    let pattern = context.fixture_operand(TypeKind::Tuple(
        parameters.into_iter().map(TypeKind::GenericParam).collect(),
    ));
    let actual = context.fixture_operand(TypeKind::Tuple(vec![TypeKind::I64, TypeKind::String]));
    assert!(matches!(
        relate_selected_call(
            &pattern.view(),
            &actual.view(),
            path,
            &mut context,
            ConstraintAcceptance::PatternAcceptsActual
        ),
        Err(TypeConstraintError::Abort(
            TypeConstraintAbort::BranchLimit {
                actual: 2,
                limit: 1
            }
        ))
    ));
}

#[test]
fn a_transitive_type_alias_retains_every_occurrence_and_the_exact_replacement_root() {
    let cancellation = AtomicBool::new(false);
    let (mut context, path, parameters, _) = setup(&cancellation, 3, 0);
    let root = context.fixture_operand(TypeKind::Tuple(vec![
        TypeKind::GenericParam(parameters[1].clone()),
        TypeKind::String,
    ]));
    let retained_root = Arc::downgrade(&root.value);
    let child = root
        .view()
        .children(&mut context)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    let path = context
        .add_binding(path, parameters[0].clone(), child)
        .unwrap()
        .unwrap();
    let next = context.fixture_operand(TypeKind::GenericParam(parameters[2].clone()));
    let path = context
        .add_binding(path, parameters[1].clone(), next.view())
        .unwrap()
        .unwrap();
    let terminal = context.fixture_operand(TypeKind::I64);
    let path = context
        .add_binding(path, parameters[2].clone(), terminal.view())
        .unwrap()
        .unwrap();
    let input = context.fixture_operand(TypeKind::GenericParam(parameters[0].clone()));
    let ConstraintAliasResolution::Value(resolved) =
        resolve_type_alias(input.view(), &path, &mut context).unwrap()
    else {
        panic!("a bound alias resolves to its retained term")
    };
    assert_eq!(resolved.value(), &TypeKind::I64);
    let ConstraintOperandOrigin::TypeAlias {
        steps: derivation,
        terminal: proof,
    } = resolved.origin.as_ref()
    else {
        panic!("a direct alias retains its complete flat derivation")
    };
    assert_eq!(derivation.len(), parameters.len());
    for (index, (step, parameter)) in derivation.iter().zip(&parameters).enumerate() {
        assert_eq!(
            step.occurrence.value(),
            &TypeKind::GenericParam(parameter.clone())
        );
        assert!(Arc::ptr_eq(
            &step.replacement.value,
            &path.bindings[parameter].value
        ));
        assert!(Arc::ptr_eq(
            &step.replacement.origin,
            &path.bindings[parameter].origin
        ));
        if index == 1 {
            assert_eq!(
                steps(&step.occurrence),
                [ConstraintOperandProjectionStep::TypeChild(0)]
            );
        }
    }
    assert_eq!(
        derivation.last().unwrap().replacement.value(),
        &TypeKind::I64
    );
    assert_eq!(proof.scope, terminal.scope);
    assert!(Arc::ptr_eq(&proof.origin, &terminal.origin));
    drop(input);
    drop(path);
    drop(root);
    assert!(retained_root.upgrade().is_some());
    drop(resolved);
    assert!(retained_root.upgrade().is_none());
}

#[test]
fn a_constant_alias_keeps_the_array_length_port_without_a_synthetic_type() {
    let cancellation = AtomicBool::new(false);
    let (mut context, path, _, parameters) = setup(&cancellation, 0, 2);
    let array = context.fixture_operand(TypeKind::Array {
        item: Box::new(TypeKind::I64),
        len: ArrayLength::Generic(parameters[1].clone()),
    });
    let retained_array = Arc::downgrade(&array.value);
    let path = context
        .add_const_binding(
            path,
            parameters[0].clone(),
            array.view().array_length().unwrap().unwrap(),
        )
        .unwrap()
        .unwrap();
    let terminal = context.fixture_operand(ArrayLength::Const(7));
    let path = context
        .add_const_binding(path, parameters[1].clone(), terminal.view())
        .unwrap()
        .unwrap();
    let input = context.fixture_operand(ArrayLength::Generic(parameters[0].clone()));
    let ConstraintAliasResolution::Value(resolved) =
        resolve_const_alias(input.view(), &path, &mut context).unwrap()
    else {
        panic!("a bound length resolves to its retained term")
    };
    assert_eq!(resolved.value(), &ArrayLength::Const(7));
    let ConstraintOperandOrigin::ConstAlias {
        steps: derivation,
        terminal: proof,
    } = resolved.origin.as_ref()
    else {
        panic!("a length alias retains its complete flat derivation")
    };
    assert_eq!(derivation.len(), 2);
    assert_eq!(
        derivation[1].occurrence.value(),
        &ArrayLength::Generic(parameters[1].clone())
    );
    assert_eq!(
        steps(&derivation[1].occurrence),
        [ConstraintOperandProjectionStep::ArrayLength]
    );
    assert!(Arc::ptr_eq(&proof.origin, &terminal.origin));
    drop(path);
    drop(array);
    assert!(retained_array.upgrade().is_some());
    drop(resolved);
    assert!(retained_array.upgrade().is_none());
}

#[test]
fn a_binding_lookup_rejects_a_value_whose_actual_scope_disagrees_with_its_owner() {
    let cancellation = AtomicBool::new(false);
    let (mut context, mut path, parameters, _) = setup(&cancellation, 1, 0);
    let inner = context.enter_binder_scope(GenericBinder::new(1, 0, 0));
    let foreign = context.fixture_operand(TypeKind::I64);
    context.restore_binder_scope(inner);
    // Deliberately bypass admission to exercise the projection reader's boundary.
    path.bindings.insert(parameters[0].clone(), foreign);
    assert!(matches!(
        path.projection_view().binding(&parameters[0]),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::OriginalProducer
        ))
    ));
}

#[test]
fn ten_thousand_direct_aliases_retain_and_release_a_flat_proof_with_exact_node_admission() {
    let cancellation = AtomicBool::new(false);
    let (mut context, mut path, parameters, _) = setup_with_limits(
        &cancellation,
        10_000,
        0,
        TypeConstraintLimits::new(20_002, 10_001, 128, 10_001),
    );
    path.bindings = context.fixture_bindings(
        parameters.iter().cloned().zip(
            parameters
                .iter()
                .skip(1)
                .cloned()
                .map(TypeKind::GenericParam)
                .chain([TypeKind::I64]),
        ),
    );
    let input = context.fixture_operand(TypeKind::GenericParam(parameters[0].clone()));
    let ConstraintAliasResolution::Value(resolved) =
        resolve_type_alias(input.view(), &path, &mut context).unwrap()
    else {
        panic!("the entire direct alias chain closes")
    };
    assert_eq!(resolved.value(), &TypeKind::I64);
    let ConstraintOperandOrigin::TypeAlias { steps, terminal } = resolved.origin.as_ref() else {
        panic!("one flat proof owns the chain")
    };
    assert_eq!(steps.len(), 10_000);
    assert_eq!(steps.last().unwrap().replacement.value(), &TypeKind::I64);
    assert!(Arc::ptr_eq(
        &terminal.origin,
        &steps.last().unwrap().replacement.origin
    ));
    context.enter_node().unwrap();
    assert_eq!(
        context.enter_node(),
        Err(TypeConstraintError::Abort(TypeConstraintAbort::NodeLimit {
            actual: 10_002,
            limit: 10_001,
        }))
    );
    drop(path);
    drop(resolved);
}
