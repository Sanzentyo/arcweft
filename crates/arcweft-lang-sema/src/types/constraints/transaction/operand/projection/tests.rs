use super::*;
use crate::types::constraints::{
    NoConstraintClient,
    context::{LocalConstraintAccounting, TypeConstraintLimits},
    test_support::ConstraintTestSetup,
};
use crate::types::{GenericBinder, GenericTypeReference};
use std::sync::atomic::AtomicBool;

mod authority;
mod bindings;
mod closed_effects;
mod normalization;
mod result_uses;
mod source_expectations;

type Context<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;

fn context(cancellation: &AtomicBool) -> Context<'_> {
    ConstraintTestSetup::new(
        TypeConstraintLimits::new(4096, 2048, 128, 128),
        cancellation,
    )
    .into_parts()
    .0
}

fn steps<T>(
    value: &ConstraintOperand<NoConstraintClient, T>,
) -> Vec<ConstraintOperandProjectionStep> {
    source_steps(&value.origin)
}

fn source_steps(
    origin: &ConstraintOperandOrigin<NoConstraintClient>,
) -> Vec<ConstraintOperandProjectionStep> {
    match origin {
        ConstraintOperandOrigin::Projection {
            address: Some(address),
            ..
        } => address.steps(),
        ConstraintOperandOrigin::Projection {
            address: None,
            producer,
            ..
        }
        | ConstraintOperandOrigin::Rebuilt {
            source: producer, ..
        } => source_steps(producer),
        ConstraintOperandOrigin::ConstOccurrence(value) => source_steps(&value.origin),
        _ => panic!("retained projection"),
    }
}

#[test]
fn tuple_children_borrow_the_root_and_retain_distinct_exact_addresses() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let scope = Arc::new(context.capture_lexical_scope().unwrap());
    let original = context.fixture_original_context(None).unwrap();
    let root = ConstraintOperand::<NoConstraintClient>::from_original_in(
        &TypeKind::Tuple(vec![TypeKind::I64, TypeKind::String]),
        Arc::clone(&scope),
        original,
        &mut context,
    )
    .unwrap();
    let TypeKind::Tuple(values) = root.value() else {
        panic!("tuple")
    };
    let view = root.view();
    let mut children = view.children(&mut context).unwrap();
    let first = children.next().unwrap().unwrap();
    let second = children.next().unwrap().unwrap();
    assert!(children.next().is_none());
    assert!(std::ptr::eq(first.value(), &values[0]));
    assert!(std::ptr::eq(second.value(), &values[1]));
    let first = first.transport(&scope, &mut context).unwrap();
    let second = second.transport(&scope, &mut context).unwrap();
    assert_eq!(
        steps(&first),
        [ConstraintOperandProjectionStep::TypeChild(0)]
    );
    assert_eq!(
        steps(&second),
        [ConstraintOperandProjectionStep::TypeChild(1)]
    );
    drop(root);
    assert_eq!(first.value(), &TypeKind::I64);
    assert_eq!(second.value(), &TypeKind::String);
    let ConstraintOperandOrigin::Projection {
        root: first_root, ..
    } = first.origin.as_ref()
    else {
        unreachable!()
    };
    let ConstraintOperandOrigin::Projection {
        root: second_root, ..
    } = second.origin.as_ref()
    else {
        unreachable!()
    };
    assert!(Arc::ptr_eq(first_root, second_root));
}

#[test]
fn an_array_item_and_its_length_have_distinct_type_and_const_projections() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let outer = context.enter_binder_scope(GenericBinder::new(1, 1, 0));
    let scope = Arc::new(context.capture_lexical_scope().unwrap());
    let original = context.fixture_original_context(None).unwrap();
    let ty = context.lexical_scope().bound_type(0, 0).unwrap();
    let length = ArrayLength::Generic(context.lexical_scope().bound_const(0, 0).unwrap());
    let root = ConstraintOperand::<NoConstraintClient>::from_original_in(
        &TypeKind::Array {
            item: Box::new(TypeKind::GenericParam(ty.clone())),
            len: length.clone(),
        },
        Arc::clone(&scope),
        original,
        &mut context,
    )
    .unwrap();
    let view = root.view();
    let item = view
        .children(&mut context)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .transport(&scope, &mut context)
        .unwrap();
    let len = view
        .array_length()
        .unwrap()
        .unwrap()
        .transport(&scope, &mut context)
        .unwrap();
    assert_eq!(item.value(), &TypeKind::GenericParam(ty));
    assert_eq!(len.value(), &length);
    assert_eq!(
        steps(&item),
        [ConstraintOperandProjectionStep::TypeChild(0)]
    );
    assert_eq!(steps(&len), [ConstraintOperandProjectionStep::ArrayLength]);
    context.restore_binder_scope(outer);
}

#[test]
fn a_stored_constant_has_its_own_borrowed_view_without_a_synthetic_type() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let binder = GenericBinder::new(0, 1, 0);
    let outer = context.enter_binder_scope(binder);
    let scope = Arc::new(context.capture_lexical_scope().unwrap());
    let original = context.fixture_original_context(None).unwrap();
    let root = ConstraintOperand::<NoConstraintClient, ArrayLength>::from_original_in(
        &ArrayLength::Generic(context.lexical_scope().bound_const(0, 0).unwrap()),
        Arc::clone(&scope),
        original,
        &mut context,
    )
    .unwrap();
    let view = root.view();
    assert!(std::ptr::eq(view.value(), root.value()));
    let retained = view.transport(&scope, &mut context).unwrap();
    assert_eq!(retained.value(), root.value());
    assert!(matches!(
        retained.origin.as_ref(),
        ConstraintOperandOrigin::Original(_)
    ));
    context.restore_binder_scope(outer);
    let foreign = context.enter_binder_scope(binder);
    let target = Arc::new(context.capture_lexical_scope().unwrap());
    assert_eq!(scope.shape(), target.shape());
    assert!(view.transport(&target, &mut context).is_err());
    context.restore_binder_scope(foreign);
}

#[test]
fn function_children_retain_the_actual_local_binder_and_reject_equal_shaped_foreign_frames() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let ambient = GenericBinder::new(1, 0, 0);
    let outer = context.enter_binder_scope(ambient);
    let scope = Arc::new(context.capture_lexical_scope().unwrap());
    let original = context.fixture_original_context(None).unwrap();
    let nested = scope.shape().with_binder(ambient);
    let root = ConstraintOperand::<NoConstraintClient>::from_original_in(
        &TypeKind::function_with_binder(
            ambient,
            [TypeKind::GenericParam(nested.bound_type(0, 0).unwrap())],
            TypeKind::GenericParam(nested.bound_type(1, 0).unwrap()),
            crate::effect_row::EffectRow::closed(crate::effects::EffectSet::new()),
        ),
        Arc::clone(&scope),
        original,
        &mut context,
    )
    .unwrap();
    let view = root.view();
    assert!(view.children(&mut context).is_err());
    let local = context.enter_binder_scope(ambient);
    let local_scope = Arc::new(context.capture_lexical_scope().unwrap());
    let mut children = view.children(&mut context).unwrap();
    let parameter = children.next().unwrap().unwrap();
    let result = children.next().unwrap().unwrap();
    assert!(parameter.transport(&scope, &mut context).is_err());
    let parameter = parameter.transport(&local_scope, &mut context).unwrap();
    let result = result.transport(&scope, &mut context).unwrap();
    assert_eq!(
        result.value(),
        &TypeKind::GenericParam(scope.shape().bound_type(0, 0).unwrap())
    );
    assert!(matches!(
        parameter.value(),
        TypeKind::GenericParam(GenericTypeReference::Bound(_))
    ));
    context.restore_binder_scope(local);
    let foreign_local = context.enter_binder_scope(ambient);
    let foreign_scope = Arc::new(context.capture_lexical_scope().unwrap());
    assert_eq!(foreign_scope.shape(), local_scope.shape());
    assert!(parameter.transport(&foreign_scope, &mut context).is_err());
    context.restore_binder_scope(foreign_local);
    context.restore_binder_scope(outer);
    let unrelated = context.enter_binder_scope(ambient);
    let unrelated_local = context.enter_binder_scope(ambient);
    assert!(view.children(&mut context).is_err());
    context.restore_binder_scope(unrelated_local);
    context.restore_binder_scope(unrelated);
}
