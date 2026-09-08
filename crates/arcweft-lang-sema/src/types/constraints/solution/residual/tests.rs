use std::{collections::BTreeMap, sync::atomic::AtomicBool};

use crate::{
    effect_row::EffectRow,
    types::{
        ArrayLength, DetachedGenericOwnerId, GenericBinder, GenericConstParameterId,
        GenericConstReference, GenericParameterOwnerId, GenericScope, GenericScopeError,
        GenericTypeParameterId, GenericTypeReference, TypeKind,
        constraints::{
            NoConstraintClient, TypeConstraintConstEligibility, TypeConstraintParameterEligibility,
            TypeConstraintParameterScope, TypeConstraintSolution,
            context::{
                LocalConstraintAccounting, TypeConstraintConstParameterScopeRow,
                TypeConstraintContext, TypeConstraintLimits, TypeConstraintTypeParameterScopeRow,
            },
        },
    },
};

type TestContext<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;

fn parameter(slot: u16) -> GenericTypeParameterId {
    GenericTypeParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(715)),
        slot,
    )
}

fn constant() -> GenericConstParameterId {
    GenericConstParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(715)),
        0,
    )
}

fn context(scope: TypeConstraintParameterScope, cancellation: &AtomicBool) -> TestContext<'_> {
    TestContext::with_scope(
        TypeConstraintLimits::new(65_536, 32_768, 1024, 1024),
        cancellation,
        scope,
    )
}

fn recursive_scope() -> TypeConstraintParameterScope {
    TypeConstraintParameterScope::seal_call_scope(
        crate::types::GenericBinder::EMPTY,
        [
            TypeConstraintTypeParameterScopeRow::new(
                parameter(0),
                TypeConstraintParameterEligibility::Rigid,
            ),
            TypeConstraintTypeParameterScopeRow::new(
                parameter(0),
                TypeConstraintParameterEligibility::Bindable,
            ),
        ],
        [],
        [],
        [],
    )
    .expect("caller and callee occupy distinct namespaces")
}

#[test]
fn recursive_binding_to_the_same_declaration_is_free_and_issuer_independent() {
    let cancellation = AtomicBool::new(false);
    let complete = || {
        let mut context = context(recursive_scope(), &cancellation);
        let callee = context
            .parameter_scope
            .type_reference(&(parameter(0)).clone().into())
            .expect("template slot");
        let caller = GenericTypeReference::Free(parameter(0));
        assert_ne!(callee, caller);
        TypeConstraintSolution::complete_path(
            BTreeMap::from([(callee, TypeKind::GenericParam(caller))]),
            BTreeMap::new(),
            BTreeMap::new(),
            &mut context,
        )
        .expect("recursive binding is not an occurs cycle")
    };
    let first = complete();
    let second = complete();
    assert_eq!(
        first, second,
        "a completed solution retains no active issuer"
    );
    let (_, value) = first.bindings().next().expect("one binding");
    let value = value.value();
    assert_eq!(value, &TypeKind::generic_parameter(parameter(0)));
    assert!(value.semantic_identity_digest().is_ok());
}

fn continuation_scope(next: bool) -> TypeConstraintParameterScope {
    TypeConstraintParameterScope::seal_call_scope(
        crate::types::GenericBinder::EMPTY,
        [
            TypeConstraintTypeParameterScopeRow::new(
                parameter(1),
                TypeConstraintParameterEligibility::Rigid,
            ),
            TypeConstraintTypeParameterScopeRow::new(
                parameter(0),
                TypeConstraintParameterEligibility::Bindable,
            ),
            TypeConstraintTypeParameterScopeRow::new(
                parameter(1),
                if next {
                    TypeConstraintParameterEligibility::Bindable
                } else {
                    TypeConstraintParameterEligibility::FutureEligible
                },
            ),
        ],
        [TypeConstraintConstParameterScopeRow::new(
            constant(),
            if next {
                TypeConstraintConstEligibility::Bindable
            } else {
                TypeConstraintConstEligibility::FutureEligible
            },
        )],
        (if next { vec![parameter(0)] } else { vec![] })
            .into_iter()
            .map(Into::into),
        [],
    )
    .expect("canonical continuation scope")
}

fn prefix(cancellation: &AtomicBool) -> TypeConstraintSolution {
    let mut context = context(continuation_scope(false), cancellation);
    let local = GenericScope::default()
        .with_binder(GenericBinder::new(1, 0, 0))
        .bound_type(0, 0)
        .expect("function-local bound parameter");
    let remaining = context
        .parameter_scope
        .type_reference(&(parameter(1)).clone().into())
        .expect("future type");
    let remaining_const = context
        .parameter_scope
        .const_reference(&(constant()).clone().into())
        .expect("future length");
    let value = TypeKind::function_with_binder(
        GenericBinder::new(1, 0, 0),
        [TypeKind::GenericParam(local)],
        TypeKind::Tuple(vec![
            TypeKind::GenericParam(remaining),
            TypeKind::generic_parameter(parameter(1)),
            TypeKind::Array {
                item: Box::new(TypeKind::Bool),
                len: ArrayLength::Generic(remaining_const),
            },
        ]),
        EffectRow::closed(crate::effects::EffectSet::new()),
    );
    TypeConstraintSolution::complete_path(
        BTreeMap::from([(
            context
                .parameter_scope
                .type_reference(&(parameter(0)).clone().into())
                .expect("bound type"),
            value,
        )]),
        BTreeMap::new(),
        BTreeMap::new(),
        &mut context,
    )
    .expect("future references become residual binders")
}

#[test]
fn nested_function_binders_preserve_inner_variables_and_bind_future_types_and_lengths() {
    let cancellation = AtomicBool::new(false);
    let solution = prefix(&cancellation);
    let (_, value) = solution.bindings().next().expect("prefix binding");
    let value = value.value();
    let TypeKind::Function {
        params,
        return_type,
        ..
    } = value
    else {
        panic!("function binding")
    };
    assert!(
        matches!(params.as_slice(), [TypeKind::GenericParam(GenericTypeReference::Bound(parameter))] if parameter.depth() == 0)
    );
    let TypeKind::Tuple(items) = return_type.as_ref() else {
        panic!("tuple result")
    };
    assert!(
        matches!(&items[0], TypeKind::GenericParam(GenericTypeReference::Bound(parameter)) if parameter.depth() == 1 && parameter.slot() == 0)
    );
    assert_eq!(items[1], TypeKind::generic_parameter(parameter(1)));
    assert!(
        matches!(&items[2], TypeKind::Array { len: ArrayLength::Generic(GenericConstReference::Bound(parameter)), .. } if parameter.depth() == 1 && parameter.slot() == 0)
    );
    assert!(matches!(
        value.semantic_identity_digest(),
        Err(GenericScopeError::UnknownDepth { depth: 1 })
    ));
    assert!(
        value
            .semantic_identity_digest_in_scope(solution.residual.scope())
            .is_ok()
    );
}

#[test]
fn one_prefix_can_reopen_twice_without_sharing_inference_variables() {
    let cancellation = AtomicBool::new(false);
    let solution = prefix(&cancellation);
    let mut first = context(continuation_scope(true), &cancellation);
    let mut second = context(continuation_scope(true), &cancellation);
    let mut first_path = solution
        .restore_inherited_path(&mut first)
        .expect("first opening");
    let mut second_path = solution
        .restore_inherited_path(&mut second)
        .expect("second opening");
    let first_key = first
        .parameter_scope
        .type_reference(&(parameter(1)).clone().into())
        .expect("first residual variable");
    let second_key = second
        .parameter_scope
        .type_reference(&(parameter(1)).clone().into())
        .expect("second residual variable");
    assert_ne!(first_key, second_key);
    first_path.bindings.insert(first_key, TypeKind::I32);
    second_path.bindings.insert(second_key, TypeKind::String);
    first_path.const_bindings.insert(
        first
            .parameter_scope
            .const_reference(&(constant()).clone().into())
            .expect("length"),
        ArrayLength::Const(3),
    );
    second_path.const_bindings.insert(
        second
            .parameter_scope
            .const_reference(&(constant()).clone().into())
            .expect("length"),
        ArrayLength::Const(5),
    );
    let first = TypeConstraintSolution::complete_path(
        first_path.bindings,
        first_path.const_bindings,
        BTreeMap::new(),
        &mut first,
    )
    .expect("first application closes");
    let second = TypeConstraintSolution::complete_path(
        second_path.bindings,
        second_path.const_bindings,
        BTreeMap::new(),
        &mut second,
    )
    .expect("second application closes");
    assert_ne!(first, second);
    for (solution, item, length) in [(&first, TypeKind::I32, 3), (&second, TypeKind::String, 5)] {
        let (_, value) = solution.bindings().next().expect("inherited function");
        let value = value.value();
        let TypeKind::Function { return_type, .. } = value else {
            panic!("function")
        };
        assert_eq!(
            return_type.as_ref(),
            &TypeKind::Tuple(vec![
                item,
                TypeKind::generic_parameter(parameter(1)),
                TypeKind::Array {
                    item: Box::new(TypeKind::Bool),
                    len: ArrayLength::Const(length)
                }
            ])
        );
        assert!(value.semantic_identity_digest().is_ok());
    }
}
