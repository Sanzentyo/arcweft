use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::AtomicBool,
};

use super::{occurs_in_type, types_equal};
use crate::{
    effect_row::EffectRow,
    effects::EffectSet,
    types::{
        ArrayLength, DetachedGenericOwnerId, GenericBinder, GenericConstParameterId,
        GenericConstReference, GenericParameterOwnerId, GenericScope, GenericTypeParameterId,
        GenericTypeReference, TypeKind,
        constraints::{
            ConstraintAcceptance, NoConstraintClient, TypeConstraintAbort,
            TypeConstraintConstEligibility, TypeConstraintError, TypeConstraintInvariant,
            TypeConstraintParameterEligibility, TypeConstraintParameterScope,
            TypeConstraintParameterScopeInvariant, TypeConstraintRejection,
            context::{LocalConstraintAccounting, TypeConstraintContext, TypeConstraintLimits},
            relate_selected_call,
        },
    },
};

type TestContext<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;

fn context(
    scope: TypeConstraintParameterScope,
    cancellation: &AtomicBool,
    max_nodes: u64,
) -> TestContext<'_> {
    TestContext::with_scope(
        TypeConstraintLimits::new(max_nodes.saturating_mul(2), max_nodes, 128, 64),
        cancellation,
        scope,
    )
}

fn empty_scope() -> TypeConstraintParameterScope {
    TypeConstraintParameterScope::new([]).expect("empty template inventory")
}

fn nested_function() -> TypeKind {
    let outer_binder = GenericBinder::new(1, 0, 0);
    let inner_binder = GenericBinder::new(0, 1, 0);
    let outer = GenericScope::default().with_binder(outer_binder);
    let inner = outer.with_binder(inner_binder);
    let inner_function = TypeKind::function_with_binder(
        inner_binder,
        [TypeKind::Array {
            item: Box::new(TypeKind::GenericParam(
                inner.bound_type(1, 0).expect("outer type"),
            )),
            len: ArrayLength::Generic(inner.bound_const(0, 0).expect("inner length")),
        }],
        TypeKind::Bool,
        EffectRow::closed(EffectSet::new()),
    );
    TypeKind::function_with_binder(
        outer_binder,
        [TypeKind::GenericParam(
            outer.bound_type(0, 0).expect("outer parameter"),
        )],
        inner_function,
        EffectRow::closed(EffectSet::new()),
    )
}

#[test]
fn equality_enters_function_type_and_constant_binders() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(empty_scope(), &cancellation, 256);
    let left = nested_function();
    let right = nested_function();
    assert!(types_equal(&left, &right, &mut context).expect("bound references are in scope"));
    assert!(context.lexical_scope().binders().is_empty());
}

#[test]
fn equality_and_selected_relation_reject_escaped_type_and_length_references() {
    let source_scope = GenericScope::default().with_binder(GenericBinder::new(1, 1, 0));
    let escaped_type = source_scope.bound_type(0, 0).expect("source type slot");
    let escaped_const = source_scope
        .bound_const(0, 0)
        .expect("source constant slot");
    let cancellation = AtomicBool::new(false);
    for (ty, expected) in [
        (
            TypeKind::GenericParam(escaped_type.clone()),
            TypeConstraintParameterScopeInvariant::TypeParameterOutOfScope {
                parameter: escaped_type,
            },
        ),
        (
            TypeKind::Array {
                item: Box::new(TypeKind::I64),
                len: ArrayLength::Generic(escaped_const.clone()),
            },
            TypeConstraintParameterScopeInvariant::ConstParameterOutOfScope {
                parameter: escaped_const,
            },
        ),
    ] {
        let mut context = context(empty_scope(), &cancellation, 256);
        let error =
            TypeConstraintError::Invariant(TypeConstraintInvariant::ParameterScope(expected));
        assert_eq!(types_equal(&ty, &ty, &mut context), Err(error.clone()));
        let path = context.start_path().expect("empty path");
        assert_eq!(
            relate_selected_call(
                &ty,
                &ty,
                path,
                &mut context,
                ConstraintAcceptance::PatternAcceptsActual,
            )
            .err()
            .expect("reflexivity cannot admit a reference outside its binder"),
            error,
        );
    }
}

#[test]
fn occurs_check_visits_nested_binders_and_restores_the_enclosing_scope() {
    let parameter = GenericTypeParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(718)),
        0,
    );
    let scope = TypeConstraintParameterScope::new([(
        parameter.clone(),
        TypeConstraintParameterEligibility::Bindable,
    )])
    .expect("template parameter");
    let cancellation = AtomicBool::new(false);
    let mut context = context(scope, &cancellation, 256);
    let target = context
        .parameter_scope
        .type_reference(&(parameter).clone().into())
        .expect("active type");
    let nested = nested_function();
    assert!(
        !occurs_in_type(&nested, &target, &BTreeMap::new(), &mut context)
            .expect("function-local variables are rigid")
    );
    assert!(context.lexical_scope().binders().is_empty());

    let cycle = TypeKind::Tuple(vec![nested, TypeKind::GenericParam(target.clone())]);
    assert!(
        occurs_in_type(&cycle, &target, &BTreeMap::new(), &mut context)
            .expect("the active type occurs after the nested function")
    );
    assert!(context.lexical_scope().binders().is_empty());
}

#[test]
fn function_comparison_restores_scope_after_budget_failure() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(empty_scope(), &cancellation, 1);
    let function = nested_function();
    assert_eq!(
        types_equal(&function, &function, &mut context),
        Err(TypeConstraintError::Abort(TypeConstraintAbort::NodeLimit {
            actual: 2,
            limit: 1,
        })),
    );
    assert!(context.lexical_scope().binders().is_empty());
}

#[test]
fn equal_unresolved_array_lengths_do_not_establish_type_equality() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(empty_scope(), &cancellation, 256);
    let unresolved = TypeKind::Array {
        item: Box::new(TypeKind::Bool),
        len: ArrayLength::Inferred,
    };
    assert_eq!(
        types_equal(&unresolved, &unresolved, &mut context),
        Err(TypeConstraintError::Rejected(
            TypeConstraintRejection::UnresolvedType
        )),
    );
}

fn type_inventory(count: u16) -> (TypeConstraintParameterScope, Vec<GenericTypeReference>) {
    let declarations = (0..count)
        .map(|ordinal| {
            GenericTypeParameterId::new(
                GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(719)),
                ordinal,
            )
        })
        .collect::<Vec<_>>();
    let scope = TypeConstraintParameterScope::new(
        declarations
            .iter()
            .cloned()
            .map(|parameter| (parameter, TypeConstraintParameterEligibility::Bindable)),
    )
    .expect("distinct type declarations");
    let references = declarations
        .into_iter()
        .map(|declaration| {
            scope
                .type_reference(&declaration.into())
                .expect("opened type")
        })
        .collect();
    (scope, references)
}

fn const_inventory(count: u16) -> (TypeConstraintParameterScope, Vec<GenericConstReference>) {
    let declarations = (0..count)
        .map(|ordinal| {
            GenericConstParameterId::new(
                GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(720)),
                ordinal,
            )
        })
        .collect::<Vec<_>>();
    let scope = TypeConstraintParameterScope::new_with_constants(
        [],
        declarations
            .iter()
            .cloned()
            .map(|parameter| (parameter, TypeConstraintConstEligibility::Bindable)),
    )
    .expect("distinct constant declarations");
    let references = declarations
        .into_iter()
        .map(|declaration| {
            scope
                .const_reference(&declaration.into())
                .expect("opened constant")
        })
        .collect();
    (scope, references)
}

#[test]
fn projection_preserves_nested_function_binders() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(empty_scope(), &cancellation, 256);
    let function = nested_function();
    let projected = super::project_type(
        &function,
        &BTreeMap::new(),
        &BTreeMap::new(),
        super::ConstraintClosurePolicy::ProjectionClosed,
        &mut context,
    )
    .expect("function-local variables remain rigid under their binders");
    assert_eq!(projected.value, function);
    assert!(projected.remaining.is_empty());
    assert!(context.lexical_scope().binders().is_empty());
}

#[test]
fn projection_follows_long_type_and_constant_aliases_with_exact_node_admission() {
    let cancellation = AtomicBool::new(false);
    let (scope, parameters) = type_inventory(10_000);
    let bindings = parameters
        .iter()
        .cloned()
        .zip(
            parameters
                .iter()
                .skip(1)
                .cloned()
                .map(TypeKind::GenericParam)
                .chain([TypeKind::I64]),
        )
        .collect::<BTreeMap<_, _>>();
    let mut type_context = context(scope, &cancellation, 10_001);
    let projected = super::project_type(
        &TypeKind::GenericParam(parameters[0].clone()),
        &bindings,
        &BTreeMap::new(),
        super::ConstraintClosurePolicy::ProjectionClosed,
        &mut type_context,
    )
    .expect("transitive type aliases do not consume the Rust call stack");
    assert_eq!(projected.value, TypeKind::I64);
    assert!(projected.remaining.is_empty());
    let exhausted = Err(TypeConstraintError::Abort(TypeConstraintAbort::NodeLimit {
        actual: 10_002,
        limit: 10_001,
    }));
    assert_eq!(type_context.enter_node(), exhausted);

    let (scope, parameters) = const_inventory(10_000);
    let bindings = parameters
        .iter()
        .cloned()
        .zip(
            parameters
                .iter()
                .skip(1)
                .cloned()
                .map(ArrayLength::Generic)
                .chain([ArrayLength::Const(37)]),
        )
        .collect::<BTreeMap<_, _>>();
    let mut const_context = context(scope, &cancellation, 10_001);
    let projected = super::project_const_argument(
        &ArrayLength::Generic(parameters[0].clone()),
        &bindings,
        super::ConstraintClosurePolicy::ProjectionClosed,
        &mut const_context,
    )
    .expect("transitive constant aliases do not consume the Rust call stack");
    assert_eq!(projected, ArrayLength::Const(37));
    assert_eq!(const_context.enter_node(), exhausted);
}

#[test]
fn projection_rebuilds_deep_types_and_rejects_exhausted_work_before_the_leaf() {
    let cancellation = AtomicBool::new(false);
    let mut ty = (0..10_000).fold(TypeKind::I64, |ty, _| TypeKind::Vec(Box::new(ty)));
    let mut complete_context = context(empty_scope(), &cancellation, 10_001);
    let complete = super::project_type(
        &ty,
        &BTreeMap::new(),
        &BTreeMap::new(),
        super::ConstraintClosurePolicy::ProjectionClosed,
        &mut complete_context,
    );
    let mut limited_context = context(empty_scope(), &cancellation, 32);
    let limited = super::project_type(
        &ty,
        &BTreeMap::new(),
        &BTreeMap::new(),
        super::ConstraintClosurePolicy::ProjectionClosed,
        &mut limited_context,
    );
    // Deliberately extreme test values are also consumed iteratively: their
    // derived Drop behavior is independent of the projection traversal.
    while let TypeKind::Vec(inner) = ty {
        ty = *inner;
    }
    let projected = complete.expect("deep structural reconstruction");
    let mut value = projected.value;
    let mut depth = 0;
    while let TypeKind::Vec(inner) = value {
        value = *inner;
        depth += 1;
    }
    assert_eq!(depth, 10_000);
    assert_eq!(value, TypeKind::I64);
    assert!(projected.remaining.is_empty());
    assert_eq!(
        limited,
        Err(TypeConstraintError::Abort(TypeConstraintAbort::NodeLimit {
            actual: 33,
            limit: 32,
        })),
    );
}

#[test]
fn projection_restores_enclosing_scope_and_seeded_guards_after_nested_abort() {
    let cancellation = AtomicBool::new(false);
    let (scope, parameters) = type_inventory(2);
    let bindings = BTreeMap::from([(parameters[0].clone(), nested_function())]);
    let seed = BTreeSet::from([parameters[1].clone()]);
    let mut visiting = seed.clone();
    let mut remaining = BTreeSet::new();
    let mut context = context(scope, &cancellation, 2);
    let outer = GenericBinder::new(1, 1, 0);
    context
        .with_binder(outer, |context| {
            let projected = super::project_type_inner(
                &TypeKind::GenericParam(parameters[0].clone()),
                &bindings,
                &BTreeMap::new(),
                super::ConstraintClosurePolicy::ProjectionClosed,
                context,
                &mut visiting,
                &mut remaining,
            );
            assert_eq!(
                projected,
                Err(TypeConstraintError::Abort(TypeConstraintAbort::NodeLimit {
                    actual: 3,
                    limit: 2,
                })),
            );
            assert_eq!(context.lexical_scope().binders(), &[outer]);
            assert_eq!(visiting, seed);
            Ok(())
        })
        .expect("assertions run within the enclosing binder");
    assert!(context.lexical_scope().binders().is_empty());
}

#[test]
fn projection_cycle_policy_preserves_caller_guards_for_both_parameter_kinds() {
    let cancellation = AtomicBool::new(false);
    let (scope, parameters) = type_inventory(3);
    let bindings = BTreeMap::from([
        (
            parameters[0].clone(),
            TypeKind::GenericParam(parameters[1].clone()),
        ),
        (
            parameters[1].clone(),
            TypeKind::GenericParam(parameters[0].clone()),
        ),
    ]);
    let seed = BTreeSet::from([parameters[2].clone()]);
    for policy in [
        super::ConstraintClosurePolicy::Hint,
        super::ConstraintClosurePolicy::ProjectionClosed,
    ] {
        let mut visiting = seed.clone();
        let mut remaining = BTreeSet::new();
        let mut context = context(scope.clone(), &cancellation, 16);
        let result = super::project_type_inner(
            &TypeKind::GenericParam(parameters[0].clone()),
            &bindings,
            &BTreeMap::new(),
            policy,
            &mut context,
            &mut visiting,
            &mut remaining,
        );
        if policy == super::ConstraintClosurePolicy::Hint {
            assert_eq!(result, Ok(TypeKind::GenericParam(parameters[0].clone())));
            assert_eq!(
                remaining,
                BTreeSet::from([super::RemainingConstraintParameter(
                    parameters[0].clone().into()
                )])
            );
        } else {
            assert_eq!(
                result,
                Err(TypeConstraintError::Rejected(
                    TypeConstraintRejection::CyclicInstantiation {
                        parameter: parameters[0].clone().into()
                    }
                ))
            );
            assert!(remaining.is_empty());
        }
        assert_eq!(visiting, seed);
    }

    let (scope, parameters) = const_inventory(3);
    let bindings = BTreeMap::from([
        (
            parameters[0].clone(),
            ArrayLength::Generic(parameters[1].clone()),
        ),
        (
            parameters[1].clone(),
            ArrayLength::Generic(parameters[0].clone()),
        ),
    ]);
    let seed = BTreeSet::from([parameters[2].clone()]);
    for policy in [
        super::ConstraintClosurePolicy::Hint,
        super::ConstraintClosurePolicy::ProjectionClosed,
    ] {
        let mut visiting = seed.clone();
        let mut remaining = BTreeSet::new();
        let mut context = context(scope.clone(), &cancellation, 16);
        let result = super::project_array_length(
            &ArrayLength::Generic(parameters[0].clone()),
            &bindings,
            policy,
            &mut context,
            &mut visiting,
            &mut remaining,
        );
        if policy == super::ConstraintClosurePolicy::Hint {
            assert_eq!(result, Ok(ArrayLength::Generic(parameters[0].clone())));
            assert_eq!(
                remaining,
                BTreeSet::from([super::RemainingConstraintParameter(
                    parameters[0].clone().into()
                )])
            );
        } else {
            assert_eq!(
                result,
                Err(TypeConstraintError::Rejected(
                    TypeConstraintRejection::CyclicInstantiation {
                        parameter: parameters[0].clone().into()
                    }
                ))
            );
            assert!(remaining.is_empty());
        }
        assert_eq!(visiting, seed);
    }
}

#[test]
fn projection_validates_array_length_before_descending_into_the_item() {
    let cancellation = AtomicBool::new(false);
    let source_scope = GenericScope::default().with_binder(GenericBinder::new(1, 0, 0));
    let ty = TypeKind::Array {
        item: Box::new(TypeKind::GenericParam(
            source_scope.bound_type(0, 0).expect("escaped type"),
        )),
        len: ArrayLength::Inferred,
    };
    let mut context = context(empty_scope(), &cancellation, 2);
    assert_eq!(
        super::project_type(
            &ty,
            &BTreeMap::new(),
            &BTreeMap::new(),
            super::ConstraintClosurePolicy::ProjectionClosed,
            &mut context,
        ),
        Err(TypeConstraintError::Rejected(
            TypeConstraintRejection::UnresolvedType
        )),
    );
}
