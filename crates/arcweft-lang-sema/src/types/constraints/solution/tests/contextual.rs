use super::*;
use crate::{effects::EffectSet, types::GenericTypeReference};
use std::sync::Arc;

fn ambient_value(scope: &GenericScope, ambient_depth: u32, own_function: bool) -> TypeKind {
    let array = |scope: &GenericScope, depth| TypeKind::Array {
        item: Box::new(TypeKind::GenericParam(scope.bound_type(depth, 0).unwrap())),
        len: ArrayLength::Generic(scope.bound_const(depth, 0).unwrap()),
    };
    if own_function {
        let binder = GenericBinder::new(1, 1, 0);
        let nested = scope.with_binder(binder);
        let own = TypeKind::GenericParam(nested.bound_type(0, 0).unwrap());
        TypeKind::function_with_binder(
            binder,
            [own.clone(), array(&nested, ambient_depth + 1)],
            own,
            EffectRow::closed(EffectSet::new()),
        )
    } else {
        array(scope, ambient_depth)
    }
}

fn complete_ambient(
    future: bool,
    own_function: bool,
) -> (TypeConstraintSolution, GenericScope, TypeKind) {
    let cancellation = AtomicBool::new(false);
    let (mut work, parameters) = context(scope(false, future), &cancellation).into_parts();
    let ambient = GenericBinder::new(1, 1, 0);
    let enclosing = work.enter_binder_scope(ambient);
    let application = work.fixture_application_scope((), parameters).unwrap();
    let id = application.id();
    let key = application
        .parameters()
        .type_reference(&parameter(0).into())
        .unwrap();
    let length = application
        .parameters()
        .const_reference(&constant().into())
        .unwrap();
    let mut path = work.start_path(application).unwrap();
    let value = ambient_value(work.lexical_scope(), 0, own_function);
    path = work
        .fixture_add_binding(path, key, &value)
        .unwrap()
        .unwrap();
    if !future {
        let value = ArrayLength::Generic(work.lexical_scope().bound_const(0, 0).unwrap());
        path = work
            .fixture_add_const_binding(path, length, &value)
            .unwrap()
            .unwrap();
    }
    let completed = TypeConstraintSolution::complete_application(&path, id, &mut work).unwrap();
    let residual = if future {
        GenericBinder::new(1, 1, 0)
    } else {
        GenericBinder::EMPTY
    };
    let expected_scope = work.lexical_scope().with_binder(residual);
    let expected = ambient_value(&expected_scope, u32::from(future), own_function);
    work.restore_binder_scope(enclosing);
    assert!(work.lexical_scope().binders().is_empty());
    (completed, expected_scope, expected)
}

#[test]
fn completed_values_retain_their_ambient_type_and_const_context_without_residuals() {
    let (solution, expected_scope, expected) = complete_ambient(false, false);
    let (_, value) = solution.bindings().next().unwrap();
    assert_eq!(value.value(), &expected);
    assert_eq!(value.scope(), &expected_scope);
    let (_, length) = solution.const_bindings().next().unwrap();
    assert_eq!(length.scope(), &expected_scope);
    assert_eq!(
        length.value(),
        &ArrayLength::Generic(expected_scope.bound_const(0, 0).unwrap())
    );
}

#[test]
fn residual_slots_do_not_capture_same_position_ambient_type_and_const_parameters() {
    let (solution, expected_scope, expected) = complete_ambient(true, false);
    let (_, value) = solution.bindings().next().unwrap();
    assert_eq!(value.value(), &expected);
    assert_eq!(value.scope(), &expected_scope);
    assert!(solution.has_residual_type(&parameter(1).into()));
    assert!(solution.has_residual_const(&constant().into()));
}

#[test]
fn own_function_binders_survive_completed_ambient_and_residual_transport() {
    let (solution, expected_scope, expected) = complete_ambient(true, true);
    let (_, value) = solution.bindings().next().unwrap();
    assert_eq!(value.value(), &expected);
    assert_eq!(value.scope(), &expected_scope);
    let projected = solution
        .apply_template(&TypeKind::GenericParam(GenericTypeReference::Free(
            parameter(0),
        )))
        .unwrap();
    assert_eq!(projected.view().scope(), &expected_scope);
    assert_eq!(projected.view().value(), &expected);
}

#[test]
fn result_quantification_generalizes_only_residuals_and_retains_ambient_frames() {
    let (solution, _, _) = complete_ambient(true, true);
    let template = TypeKind::GenericParam(parameter(0).into());
    let projected = solution
        .apply_contextual_result_template_with_control(
            &template,
            &mut crate::types::projection_control::UnmeteredTypeProjection,
        )
        .unwrap();
    let ambient = GenericScope::default().with_binder(GenericBinder::new(1, 1, 0));
    assert_eq!(projected.view().scope(), &ambient);
    let TypeKind::Function {
        binder,
        params,
        return_type,
        ..
    } = projected.view().value()
    else {
        panic!("function")
    };
    assert_eq!(*binder, GenericBinder::new(2, 2, 0));
    let inside = ambient.with_binder(*binder);
    assert_eq!(
        params[0],
        TypeKind::GenericParam(inside.bound_type(0, 1).unwrap())
    );
    assert_eq!(params[1], ambient_value(&inside, 1, false));
    assert_eq!(return_type.as_ref(), &params[0]);
    assert!(
        solution.apply_result_template(&template).is_err(),
        "live ambient captures require enclosing arguments"
    );
}

#[test]
fn flat_instance_closure_cannot_substitute_an_unrelated_same_shaped_caller() {
    let (solution, _, _) = complete_ambient(false, false);
    let binder = GenericBinder::new(1, 1, 0);
    let template = GenericScope::default().with_binder(binder);
    let ty = template.bound_type(0, 0).unwrap();
    let length = template.bound_const(0, 0).unwrap();
    let parameters = TypeConstraintParameterScope::seal_call_scope(
        binder,
        [TypeRow::new(ty.clone(), TypeRole::Bindable)],
        [ConstRow::new(length.clone(), ConstRole::Bindable)],
        crate::types::constraints::TypeConstraintEffectScope::seal_call_scope([], []).unwrap(),
        [],
        [],
    )
    .unwrap();
    let cancellation = AtomicBool::new(false);
    let (mut work, mut path) = context(parameters, &cancellation).into_path();
    let type_key = path.root_scope().parameters().type_reference(&ty).unwrap();
    let const_key = path
        .root_scope()
        .parameters()
        .const_reference(&length)
        .unwrap();
    path.bindings
        .insert(type_key, work.fixture_operand(TypeKind::I64));
    path.const_bindings
        .insert(const_key, work.fixture_operand(ArrayLength::Const(7)));
    let caller = TypeConstraintSolution::complete_application(&path, path.root_id(), &mut work)
        .unwrap()
        .close_instantiation(None)
        .unwrap();
    assert!(matches!(
        solution.close_instantiation(Some(&caller)),
        Err(TypeInstantiationError::Scope(_))
    ));
}

fn continuation_parameters() -> TypeConstraintParameterScope {
    TypeConstraintParameterScope::seal_call_scope(
        GenericBinder::EMPTY,
        [
            TypeRow::new(parameter(0), TypeRole::Bindable),
            TypeRow::new(parameter(1), TypeRole::FutureEligible),
        ],
        [ConstRow::new(constant(), ConstRole::FutureEligible)],
        crate::types::constraints::TypeConstraintEffectScope::seal_call_scope([], []).unwrap(),
        [parameter(0).into()],
        [],
    )
    .unwrap()
}

#[test]
fn reopening_preserves_actual_ambient_frames_and_opens_residuals_freshly() {
    let cancellation = AtomicBool::new(false);
    let (mut work, parameters) = context(scope(false, true), &cancellation).into_parts();
    let outer = work.enter_binder_scope(GenericBinder::new(1, 1, 0));
    let application = work.fixture_application_scope((), parameters).unwrap();
    let key = application
        .parameters()
        .type_reference(&parameter(0).into())
        .unwrap();
    let mut path = work.start_path(application).unwrap();
    let value = ambient_value(work.lexical_scope(), 0, true);
    path = work
        .fixture_add_binding(path, key, &value)
        .unwrap()
        .unwrap();
    let solution =
        TypeConstraintSolution::complete_application(&path, path.root_id(), &mut work).unwrap();
    let solution = Arc::new(solution);
    let inner = work.enter_binder_scope(GenericBinder::new(1, 1, 0));
    let mut previous = None;
    for _ in 0..2 {
        let application = work
            .fixture_application_scope((), continuation_parameters())
            .unwrap();
        let future = application
            .parameters()
            .type_reference(&parameter(1).into())
            .unwrap();
        if let Some(previous) = previous.replace(future.clone()) {
            assert_ne!(previous, future);
        }
        let path = work.start_path(application).unwrap();
        let path = solution
            .restore_inherited_path(path.root_id(), path, &mut work)
            .unwrap();
        let completed =
            TypeConstraintSolution::complete_application(&path, path.root_id(), &mut work).unwrap();
        let (_, value) = completed.bindings().next().unwrap();
        assert_eq!(value.value(), &ambient_value(value.scope(), 2, true));
        assert_eq!(value.scope().binders().len(), 3);
    }
    work.restore_binder_scope(inner);
    work.restore_binder_scope(outer);
}

#[test]
fn equally_shaped_unrelated_ambient_frames_do_not_authorize_reopening() {
    let (solution, _, _) = complete_ambient(true, false);
    let cancellation = AtomicBool::new(false);
    let (mut work, parameters) = context(continuation_parameters(), &cancellation).into_parts();
    let outer = work.enter_binder_scope(GenericBinder::new(1, 1, 0));
    let application = work.fixture_application_scope((), parameters).unwrap();
    let path = work.start_path(application).unwrap();
    assert!(matches!(
        Arc::new(solution).restore_inherited_path(path.root_id(), path, &mut work),
        Err(crate::types::constraints::TypeConstraintError::Rejected(
            crate::types::constraints::TypeConstraintRejection::EscapingInstantiation { .. }
        ))
    ));
    assert_eq!(work.lexical_scope().binders().len(), 1);
    work.restore_binder_scope(outer);
}

#[test]
fn ambient_residual_and_function_owned_effects_keep_distinct_owners() {
    use crate::effect_row::{EffectConstraintEligibility, EffectConstraintVariable, EffectFormula};
    use crate::types::constraints::{ConstraintClosurePolicy, TypeConstraintEffectScope};
    use crate::types::{GenericEffectParameterId, GenericEffectReference};
    let cancellation = AtomicBool::new(false);
    let effect = GenericEffectReference::Free(GenericEffectParameterId::new(
        parameter(0).owner().clone(),
        0,
    ));
    let parameters = TypeConstraintParameterScope::seal_call_scope(
        GenericBinder::EMPTY,
        [TypeRow::new(parameter(0), TypeRole::Bindable)],
        [],
        TypeConstraintEffectScope::seal_call_scope(
            [EffectConstraintVariable::new(
                effect.clone(),
                EffectConstraintEligibility::FutureEligible,
            )],
            [],
        )
        .unwrap(),
        [],
        [],
    )
    .unwrap();
    let (mut work, parameters) = context(parameters, &cancellation).into_parts();
    let outer = work.enter_binder_scope(GenericBinder::new(0, 0, 1));
    let application = work.fixture_application_scope((), parameters).unwrap();
    let key = application
        .parameters()
        .type_reference(&parameter(0).into())
        .unwrap();
    let future = application.parameters().effect_reference(&effect).unwrap();
    let mut path = work.start_path(application).unwrap();
    let own = GenericBinder::new(0, 0, 1);
    let nested = work.lexical_scope().with_binder(own);
    let ambient = nested.bound_effect(1, 0).unwrap();
    let local = nested.bound_effect(0, 0).unwrap();
    let predicate = EffectFormula::literal(EffectSet::new(), Some(local.clone()))
        .subset(
            &EffectFormula::literal(EffectSet::new(), Some(ambient.clone())),
            &mut work,
        )
        .unwrap();
    let effects = EffectRow::open(EffectSet::new(), ambient)
        .union(&EffectRow::open(EffectSet::new(), local), &mut work)
        .unwrap()
        .union(&EffectRow::open(EffectSet::new(), future), &mut work)
        .unwrap();
    let value = TypeKind::function_with_contract(own, predicate, [], TypeKind::Unit, effects);
    path = work
        .fixture_add_binding(path, key, &value)
        .unwrap()
        .unwrap();
    let solution =
        TypeConstraintSolution::complete_application(&path, path.root_id(), &mut work).unwrap();
    let (_, completed) = solution.bindings().next().unwrap();
    let TypeKind::Function {
        effects, predicate, ..
    } = completed.value()
    else {
        panic!("function")
    };
    let completed_scope = completed.scope().with_binder(own);
    let mut expected = vec![
        completed_scope.bound_effect(0, 0).unwrap(),
        completed_scope.bound_effect(1, 0).unwrap(),
        completed_scope.bound_effect(2, 0).unwrap(),
    ];
    expected.sort();
    let mut actual = effects.variables().unwrap().cloned().collect::<Vec<_>>();
    actual.sort();
    assert_eq!(actual, expected);
    let expected_predicate = EffectFormula::literal(
        EffectSet::new(),
        Some(completed_scope.bound_effect(0, 0).unwrap()),
    )
    .subset(
        &EffectFormula::literal(
            EffectSet::new(),
            Some(completed_scope.bound_effect(2, 0).unwrap()),
        ),
        &mut work,
    )
    .unwrap();
    assert_eq!(predicate, &expected_predicate);
    let quantified = solution
        .apply_contextual_result_template_with_control(
            &TypeKind::GenericParam(parameter(0).into()),
            &mut crate::types::projection_control::UnmeteredTypeProjection,
        )
        .unwrap();
    let TypeKind::Function {
        binder, effects, ..
    } = quantified.view().value()
    else {
        panic!("function")
    };
    assert_eq!(*binder, GenericBinder::new(0, 0, 2));
    assert_eq!(effects.variables().unwrap().count(), 3);
    let projection = solution
        .reify_projection(
            std::sync::Arc::new(()),
            &value,
            &path,
            path.root_id(),
            ConstraintClosurePolicy::ProjectionFuture,
            &mut work,
        )
        .unwrap();
    assert!(
        projection.to_quantified_type().is_err(),
        "the ambient effect must not be generalized"
    );
    work.restore_binder_scope(outer);
}
