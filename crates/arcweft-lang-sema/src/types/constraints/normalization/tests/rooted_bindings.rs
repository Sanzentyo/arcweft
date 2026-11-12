use super::*;
use crate::types::constraints::{
    ConstraintGenericParameterId,
    normalization::{ConstraintClosurePolicy, project_type, seal_path},
};

fn escaping(parameter: impl Into<ConstraintGenericParameterId>) -> TypeConstraintError {
    TypeConstraintRejection::EscapingInstantiation {
        parameter: parameter.into(),
    }
    .into()
}

#[test]
fn provenance_projection_releases_ten_thousand_aliases_with_the_exact_shared_node_budget() {
    let cancellation = AtomicBool::new(false);
    let (scope, parameters) = type_inventory(10_000);
    let (mut context, mut path) = context(scope, &cancellation, 10_001).into_path();
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
    let projected = super::super::projection::seal_type_operand(
        input.view(),
        path.projection_view(),
        &mut BTreeSet::new(),
        &mut context,
    )
    .unwrap();
    assert_eq!(projected.value(), &TypeKind::I64);
    assert_eq!(
        context.enter_node(),
        Err(TypeConstraintError::Abort(TypeConstraintAbort::NodeLimit {
            actual: 10_002,
            limit: 10_001,
        }))
    );
    drop(path);
    drop(input);
    drop(projected);
}

#[test]
fn outer_type_and_const_bindings_reject_new_inner_formals() {
    let cancellation = AtomicBool::new(false);
    let (scope, types) = type_inventory(1);
    let (mut context, path) = context(scope, &cancellation, 4096).into_path();
    context
        .with_binder(GenericBinder::new(1, 0, 0), |context| {
            let value = TypeKind::GenericParam(context.lexical_scope().bound_type(0, 0).unwrap());
            let result = context.fixture_add_binding(path, types[0].clone(), &value);
            assert!(matches!(result, Err(error) if error == escaping(types[0].clone())));
            Ok(())
        })
        .unwrap();
    assert!(context.lexical_scope().binders().is_empty());

    let (scope, lengths) = const_inventory(1);
    let (mut context, path) = super::context(scope, &cancellation, 4096).into_path();
    context
        .with_binder(GenericBinder::new(0, 1, 0), |context| {
            let value = ArrayLength::Generic(context.lexical_scope().bound_const(0, 0).unwrap());
            let result = context.fixture_add_const_binding(path, lengths[0].clone(), &value);
            assert!(matches!(result, Err(error) if error == escaping(lengths[0].clone())));
            Ok(())
        })
        .unwrap();
    assert!(context.lexical_scope().binders().is_empty());
}

#[test]
fn outer_binding_cannot_capture_an_inner_effect_through_a_function_type() {
    let cancellation = AtomicBool::new(false);
    let (scope, types) = type_inventory(1);
    let (mut context, path) = context(scope, &cancellation, 4096).into_path();
    context
        .with_binder(GenericBinder::new(0, 0, 1), |context| {
            let value = TypeKind::function_with_binder(
                GenericBinder::EMPTY,
                [],
                TypeKind::Bool,
                EffectRow::open(
                    EffectSet::new(),
                    context.lexical_scope().bound_effect(0, 0).unwrap(),
                ),
            );
            let result = context.fixture_add_binding(path, types[0].clone(), &value);
            assert!(matches!(result, Err(error) if error == escaping(types[0].clone())));
            Ok(())
        })
        .unwrap();
}

#[test]
fn projecting_an_outer_binding_under_a_function_lifts_its_type_and_const_references() {
    let cancellation = AtomicBool::new(false);
    let (scope, types) = type_inventory(1);
    let (mut context, parameters) = context(scope, &cancellation, 4096).into_parts();
    let enclosing = context.enter_binder_scope(GenericBinder::new(1, 1, 0));
    let application = context.fixture_application_scope((), parameters).unwrap();
    let path = context.start_path(application).unwrap();
    let value = TypeKind::Array {
        item: Box::new(TypeKind::GenericParam(
            context.lexical_scope().bound_type(0, 0).unwrap(),
        )),
        len: ArrayLength::Generic(context.lexical_scope().bound_const(0, 0).unwrap()),
    };
    let path = context
        .fixture_add_binding(path, types[0].clone(), &value)
        .unwrap()
        .unwrap();
    let binder = GenericBinder::new(1, 1, 0);
    let input = TypeKind::function_with_binder(
        binder,
        [TypeKind::GenericParam(types[0].clone())],
        TypeKind::GenericParam(types[0].clone()),
        EffectRow::closed(EffectSet::new()),
    );
    let nested = context.lexical_scope().with_binder(binder);
    let lifted = TypeKind::Array {
        item: Box::new(TypeKind::GenericParam(nested.bound_type(1, 0).unwrap())),
        len: ArrayLength::Generic(nested.bound_const(1, 0).unwrap()),
    };
    let expected = TypeKind::function_with_binder(
        binder,
        [lifted.clone()],
        lifted,
        EffectRow::closed(EffectSet::new()),
    );
    let projected = project_type(
        &input,
        path.projection_view(),
        ConstraintClosurePolicy::ProjectionClosed,
        &mut context,
    )
    .unwrap();
    assert_eq!(projected.value, expected);
    assert!(projected.remaining.is_empty());
    assert_eq!(
        path.bindings.get(&types[0]).map(|value| value.value()),
        Some(&value)
    );
    context.restore_binder_scope(enclosing);
}

#[test]
fn a_binding_may_own_its_own_polymorphic_function_binder() {
    let cancellation = AtomicBool::new(false);
    let (scope, types) = type_inventory(1);
    let (mut context, path) = context(scope, &cancellation, 4096).into_path();
    let enclosing = context.enter_binder_scope(GenericBinder::new(1, 1, 1));
    let binder = GenericBinder::new(1, 1, 1);
    let own = context.lexical_scope().with_binder(binder);
    let item = TypeKind::Array {
        item: Box::new(TypeKind::GenericParam(own.bound_type(0, 0).unwrap())),
        len: ArrayLength::Generic(own.bound_const(0, 0).unwrap()),
    };
    let value = TypeKind::function_with_binder(
        binder,
        [item.clone()],
        item,
        EffectRow::open(EffectSet::new(), own.bound_effect(0, 0).unwrap()),
    );
    let path = context
        .fixture_add_binding(path, types[0].clone(), &value)
        .unwrap()
        .unwrap();
    context.restore_binder_scope(enclosing);
    let path = seal_path(path, &mut context).unwrap();
    let projected = project_type(
        &TypeKind::GenericParam(types[0].clone()),
        path.projection_view(),
        ConstraintClosurePolicy::ProjectionClosed,
        &mut context,
    )
    .unwrap();
    assert_eq!(projected.value, value);
    assert!(context.lexical_scope().binders().is_empty());
}

#[test]
fn aliases_cannot_lend_a_later_owners_inner_type_or_const_dependencies() {
    let cancellation = AtomicBool::new(false);
    let (parent_parameters, outer_types) = type_inventory(1);
    let (mut context, parent) = context(parent_parameters, &cancellation, 4096).into_path();
    let lease = parent
        .applications
        .import_parameters(&[outer_types[0].clone().into()])
        .unwrap();
    let (child_parameters, inner_types) = type_inventory(1);
    let enclosing = context.enter_binder_scope(GenericBinder::new(1, 0, 0));
    let child_application = context
        .fixture_application_scope((), child_parameters)
        .unwrap();
    let child = context
        .start_fixture_path_with_imported(child_application, Some(lease))
        .unwrap();
    let alias = TypeKind::GenericParam(inner_types[0].clone());
    let child = context
        .fixture_add_binding(child, outer_types[0].clone(), &alias)
        .unwrap()
        .unwrap();
    let inner = TypeKind::GenericParam(context.lexical_scope().bound_type(0, 0).unwrap());
    let child = context
        .fixture_add_binding(child, inner_types[0].clone(), &inner)
        .unwrap()
        .unwrap();
    assert_eq!(
        crate::types::constraints::normalization::resolve_type_alias(
            context
                .fixture_operand(TypeKind::GenericParam(outer_types[0].clone()))
                .view(),
            &child,
            &mut context
        )
        .err(),
        Some(escaping(inner_types[0].clone()))
    );
    assert!(
        matches!(seal_path(child, &mut context), Err(error) if error == escaping(inner_types[0].clone()))
    );
    context.restore_binder_scope(enclosing);
    assert!(parent.bindings.is_empty());

    let (parent_parameters, outer_lengths) = const_inventory(1);
    let (mut context, parent) = super::context(parent_parameters, &cancellation, 4096).into_path();
    let lease = parent
        .applications
        .import_parameters(&[outer_lengths[0].clone().into()])
        .unwrap();
    let (child_parameters, inner_lengths) = const_inventory(1);
    let enclosing = context.enter_binder_scope(GenericBinder::new(0, 1, 0));
    let application = context
        .fixture_application_scope((), child_parameters)
        .unwrap();
    let child = context
        .start_fixture_path_with_imported(application, Some(lease))
        .unwrap();
    let child = context
        .fixture_add_const_binding(
            child,
            outer_lengths[0].clone(),
            &ArrayLength::Generic(inner_lengths[0].clone()),
        )
        .unwrap()
        .unwrap();
    let inner = ArrayLength::Generic(context.lexical_scope().bound_const(0, 0).unwrap());
    let child = context
        .fixture_add_const_binding(child, inner_lengths[0].clone(), &inner)
        .unwrap()
        .unwrap();
    assert_eq!(
        crate::types::constraints::normalization::resolve_const_alias(
            context
                .fixture_operand(ArrayLength::Generic(outer_lengths[0].clone()))
                .view(),
            &child,
            &mut context
        )
        .err(),
        Some(escaping(inner_lengths[0].clone()))
    );
    assert!(
        matches!(seal_path(child, &mut context), Err(error) if error == escaping(inner_lengths[0].clone()))
    );
    context.restore_binder_scope(enclosing);
    assert!(parent.const_bindings.is_empty());
}

#[test]
fn an_imported_owner_retains_its_frame_but_does_not_authorize_an_unrelated_export() {
    let cancellation = AtomicBool::new(false);
    let (parameters, types) = type_inventory(1);
    let (mut context, parameters) = context(parameters, &cancellation, 4096).into_parts();
    let enclosing = context.enter_binder_scope(GenericBinder::new(1, 0, 0));
    let application = context.fixture_application_scope((), parameters).unwrap();
    let parent = context.start_path(application).unwrap();
    let owner = parent.lineage.scope.clone();
    let lease = parent
        .applications
        .import_parameters(&[types[0].clone().into()])
        .unwrap();
    let value = TypeKind::GenericParam(context.lexical_scope().bound_type(0, 0).unwrap());
    context.restore_binder_scope(enclosing);
    let application = context
        .fixture_application_scope((), empty_scope())
        .unwrap();
    let child = context
        .start_fixture_path_with_imported(application, Some(lease))
        .unwrap();
    let child = context
        .with_original_scope(owner.clone(), |context| {
            context.fixture_add_binding(child, types[0].clone(), &value)
        })
        .unwrap()
        .unwrap();
    let child = seal_path(child, &mut context).unwrap();
    let input = TypeKind::GenericParam(types[0].clone());
    assert_eq!(
        project_type(
            &input,
            child.projection_view(),
            ConstraintClosurePolicy::ProjectionClosed,
            &mut context
        ),
        Err(escaping(types[0].clone()))
    );
    let projected = context
        .with_original_scope(owner, |context| {
            project_type(
                &input,
                child.projection_view(),
                ConstraintClosurePolicy::ProjectionClosed,
                context,
            )
        })
        .unwrap();
    assert_eq!(projected.value, value);
    assert!(parent.bindings.is_empty());
    assert!(context.lexical_scope().binders().is_empty());
}
