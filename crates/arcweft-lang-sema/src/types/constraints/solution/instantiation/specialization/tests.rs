use super::*;
use crate::types::constraints::solution::tests::{
    ProjectionRecorder, ProjectionStop, complete, constant, context, parameter, scope,
};
use crate::{
    effect_row::{DecisionControl, DecisionWork, EffectFormula, EffectPredicate},
    effects::EffectSet,
    types::{GenericEffectReference, TypeProjectionError, UnmeteredTypeProjection},
};
use std::sync::atomic::AtomicBool;

struct Decisions;
impl DecisionControl for Decisions {
    type Error = std::convert::Infallible;
    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn subset(reference: GenericEffectReference, labels: &[&str]) -> EffectPredicate {
    EffectFormula::literal(EffectSet::new(), Some(reference))
        .subset(
            &EffectFormula::literal(
                EffectSet::from_labels(labels.iter().copied()).unwrap(),
                None,
            ),
            &mut Decisions,
        )
        .unwrap()
}

fn arguments(binder: GenericBinder, effects: &[&str]) -> ClosedTypeInstantiation {
    let scope = GenericScope::default().with_binder(binder);
    ClosedTypeInstantiation {
        bindings: if binder.types() == 0 {
            Box::new([])
        } else {
            Box::new([CheckedTypeArgumentBinding::new(
                scope.bound_type(0, 0).unwrap(),
                TypeKind::I64,
            )])
        },
        const_bindings: if binder.const_lengths() == 0 {
            Box::new([])
        } else {
            Box::new([CheckedConstArgumentBinding::new(
                scope.bound_const(0, 0).unwrap(),
                ArrayLength::Const(7),
            )])
        },
        effect_bindings: if binder.effects() == 0 {
            Box::new([])
        } else {
            Box::new([CheckedEffectArgumentBinding::new(
                scope.bound_effect(0, 0).unwrap(),
                EffectRow::closed(EffectSet::from_labels(effects.iter().copied()).unwrap()),
            )])
        },
        template_scope: scope,
    }
}

#[test]
fn project_callable_source_declaration_arguments_round_trip_all_namespaces() {
    let template = GenericScope::default().with_binder(GenericBinder::new(2, 1, 1));
    // The declaration key and value-scheme ordinal are deliberately different.
    let declaration = crate::types::GenericDeclarationBinder::new(
        template.clone(),
        Box::new([template.bound_type(0, 1).unwrap()]),
        Box::new([template.bound_const(0, 0).unwrap()]),
        Box::new([template.bound_effect(0, 0).unwrap()]),
    )
    .unwrap();
    let supplied = arguments(GenericBinder::new(1, 1, 1), &["fs.read"]);
    let body = supplied
        .for_declaration_with_control(&declaration, &mut UnmeteredTypeProjection)
        .unwrap();
    assert_eq!(
        body.instantiate_type(&TypeKind::GenericParam(template.bound_type(0, 1).unwrap()))
            .unwrap(),
        TypeKind::I64
    );
    assert_eq!(
        body.instantiate_array_length(&ArrayLength::Generic(template.bound_const(0, 0).unwrap()))
            .unwrap(),
        ArrayLength::Const(7)
    );
    assert_eq!(
        body.instantiate_effect_row(&EffectRow::open(
            EffectSet::new(),
            template.bound_effect(0, 0).unwrap()
        ))
        .unwrap(),
        EffectSet::from_labels(["fs.read"]).unwrap()
    );
    assert_eq!(
        body.declaration_arguments_with_control(&declaration, &mut UnmeteredTypeProjection)
            .unwrap(),
        supplied
    );
    assert!(matches!(
        supplied.for_declaration_with_control(&declaration, &mut ProjectionRecorder::new(0)),
        Err(TypeProjectionError::Control(ProjectionStop::Node { .. }))
    ));
    assert!(matches!(
        ClosedTypeInstantiation::default()
            .for_declaration_with_control(&declaration, &mut UnmeteredTypeProjection),
        Err(TypeProjectionError::Instantiation(
            TypeInstantiationError::SpecializationScopeMismatch
        ))
    ));
}

#[test]
fn project_specialization_preserves_nested_binders_and_enforces_the_source_predicate() {
    let binder = GenericBinder::new(1, 1, 1);
    let outer = GenericScope::default().with_binder(binder);
    let nested = outer.with_binder(binder);
    let nested_predicate = subset(nested.bound_effect(0, 0).unwrap(), &["fs.write"]);
    let local_parameter = TypeKind::GenericParam(nested.bound_type(0, 0).unwrap());
    let source = TypeKind::function_with_contract(
        binder,
        subset(outer.bound_effect(0, 0).unwrap(), &["fs.read"]),
        [TypeKind::GenericParam(outer.bound_type(0, 0).unwrap())],
        TypeKind::function_with_contract(
            binder,
            nested_predicate.clone(),
            [local_parameter.clone()],
            TypeKind::Array {
                item: Box::new(TypeKind::GenericParam(nested.bound_type(1, 0).unwrap())),
                len: ArrayLength::Generic(nested.bound_const(1, 0).unwrap()),
            },
            EffectRow::open(EffectSet::new(), nested.bound_effect(1, 0).unwrap()),
        ),
        EffectRow::closed(EffectSet::new()),
    );
    let closed = arguments(binder, &["fs.read"]);
    let projected = closed
        .specialize_function_with_control(&source, None, &mut UnmeteredTypeProjection)
        .unwrap();
    let expected = TypeKind::function_with_effects(
        [TypeKind::I64],
        TypeKind::function_with_contract(
            binder,
            nested_predicate,
            [local_parameter],
            TypeKind::Array {
                item: Box::new(TypeKind::I64),
                len: ArrayLength::Const(7),
            },
            EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap()),
        ),
        EffectRow::closed(EffectSet::new()),
    );
    assert_eq!(projected, expected);
    assert!(projected.semantic_identity_digest().is_ok());
    assert!(source.semantic_identity_digest().is_ok());
    assert!(matches!(
        arguments(binder, &["fs.write"]).specialize_function_with_control(
            &source,
            None,
            &mut UnmeteredTypeProjection,
        ),
        Err(TypeProjectionError::Instantiation(
            TypeInstantiationError::UnsatisfiedEffectConstraint
        ))
    ));
    assert!(matches!(
        closed.specialize_function_with_control(&source, None, &mut ProjectionRecorder::new(0)),
        Err(TypeProjectionError::Control(ProjectionStop::Node { .. }))
    ));
}

#[test]
fn project_specialization_composes_residual_rows_and_recursive_caller_values_simultaneously() {
    let caller = complete(scope(false, false), TypeKind::String, ArrayLength::Const(5))
        .close_instantiation(None)
        .unwrap();
    let cancellation = AtomicBool::new(false);
    let (mut context, mut path) = context(scope(true, true), &cancellation).into_path();
    let opening = path.applications.root_scope().parameters();
    let first = opening.type_reference(&parameter(0).into()).unwrap();
    let future = opening.type_reference(&parameter(1).into()).unwrap();
    let length = opening.const_reference(&constant().into()).unwrap();
    path.bindings.insert(
        first,
        TypeKind::Tuple(vec![
            TypeKind::Vec(Box::new(TypeKind::generic_parameter(parameter(0)))),
            TypeKind::GenericParam(future),
            TypeKind::Array {
                item: Box::new(TypeKind::Bool),
                len: ArrayLength::Generic(length),
            },
        ]),
    );
    let prefix = TypeConstraintSolution::complete_application(
        &path,
        path.applications.root_id(),
        &mut context,
    )
    .unwrap();
    let supplied = arguments(GenericBinder::new(1, 1, 0), &[]);
    let mut completed = prefix
        .close_residual_with_control(&supplied, Some(&caller), &mut UnmeteredTypeProjection)
        .unwrap();
    assert_eq!(
        completed
            .instantiate_type(&TypeKind::generic_parameter(parameter(0)))
            .unwrap(),
        TypeKind::Tuple(vec![
            TypeKind::Vec(Box::new(TypeKind::String)),
            TypeKind::I64,
            TypeKind::Array {
                item: Box::new(TypeKind::Bool),
                len: ArrayLength::Const(7)
            },
        ])
    );
    assert_eq!(
        completed
            .instantiate_type(&TypeKind::generic_parameter(parameter(1)))
            .unwrap(),
        TypeKind::I64
    );
    assert_eq!(
        prefix
            .residual_arguments_with_control(
                &completed,
                Some(&caller),
                &mut UnmeteredTypeProjection
            )
            .unwrap(),
        supplied
    );
    assert!(matches!(
        prefix.close_residual_with_control(
            &ClosedTypeInstantiation::default(),
            Some(&caller),
            &mut UnmeteredTypeProjection
        ),
        Err(TypeProjectionError::Instantiation(
            TypeInstantiationError::SpecializationScopeMismatch
        ))
    ));
    let mut stopped = ProjectionRecorder::new(usize::MAX);
    stopped.reject_binding = true;
    assert!(matches!(
        prefix.close_residual_with_control(&supplied, Some(&caller), &mut stopped),
        Err(TypeProjectionError::Control(ProjectionStop::Binding))
    ));
    completed.bindings[0].value = TypeKind::Bool;
    assert!(matches!(
        prefix.residual_arguments_with_control(
            &completed,
            Some(&caller),
            &mut UnmeteredTypeProjection
        ),
        Err(TypeProjectionError::Instantiation(
            TypeInstantiationError::SpecializationConflict
        ))
    ));
}
