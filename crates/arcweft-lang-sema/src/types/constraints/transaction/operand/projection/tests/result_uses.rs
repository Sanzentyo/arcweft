use super::*;
use crate::types::constraints::transaction::{
    TypeConstraintTransaction, tests::ApplicationSourceDomain,
};
use crate::types::constraints::{TypeConstraintParameterEligibility, TypeConstraintParameterScope};
use crate::types::{DetachedGenericOwnerId, GenericParameterOwnerId, GenericTypeParameterId};

fn scheme_parameters() -> TypeConstraintParameterScope {
    let binder = GenericBinder::new(1, 0, 0);
    let scope = crate::types::GenericScope::default().with_binder(binder);
    TypeConstraintParameterScope::seal_call_scope(
        binder,
        [
            crate::types::constraints::context::TypeConstraintTypeParameterScopeRow::new(
                scope.bound_type(0, 0).unwrap(),
                TypeConstraintParameterEligibility::Bindable,
            ),
        ],
        [],
        crate::types::constraints::TypeConstraintEffectScope::seal_call_scope([], []).unwrap(),
        [],
        [],
    )
    .unwrap()
}

#[test]
fn a_result_use_retains_the_selected_request_and_the_binding_that_normalized_its_input() {
    let cancellation = AtomicBool::new(false);
    let key = GenericTypeParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(792)),
        0,
    );
    let parameters = TypeConstraintParameterScope::new([(
        key.clone(),
        TypeConstraintParameterEligibility::Bindable,
    )])
    .unwrap();
    let reference = parameters.type_reference(&key.into()).unwrap();
    let (mut context, parameters) =
        ConstraintTestSetup::<LocalConstraintAccounting<'_>, ApplicationSourceDomain>::with_scope(
            TypeConstraintLimits::new(4096, 2048, 128, 128),
            &cancellation,
            parameters,
        )
        .into_parts();
    let mut transaction =
        TypeConstraintTransaction::initialize_fixture(&mut context, 0, parameters, None).unwrap();
    let application = transaction.application;
    let replacement = context.fixture_operand(TypeKind::I64);
    let retained_replacement = Arc::downgrade(&replacement.value);
    transaction.frontier[0]
        .bindings
        .insert(reference.clone(), replacement.clone());
    let binder = GenericBinder::new(1, 0, 0);
    let local = crate::types::GenericScope::default()
        .with_binder(binder)
        .bound_type(0, 0)
        .unwrap();
    transaction.request_projection(
        &mut context,
        (),
        &TypeKind::function_with_binder(
            binder,
            [TypeKind::GenericParam(local.clone())],
            TypeKind::Tuple(vec![TypeKind::GenericParam(reference)]),
            crate::effect_row::EffectRow::closed(crate::effects::EffectSet::new()),
        ),
        crate::types::constraints::TypeConstraintProjectionClosure::Closed,
    );
    let request = Arc::clone(&transaction.frontier[0].lineage.projections[0]);
    transaction
        .constrain_result_use(
            &mut context,
            (),
            &TypeKind::function_with_effects(
                [TypeKind::String],
                TypeKind::Tuple(vec![TypeKind::I64]),
                crate::effect_row::EffectRow::closed(crate::effects::EffectSet::new()),
            ),
            |source, context| {
                let TypeKind::Function {
                    params,
                    return_type,
                    effects,
                    ..
                } = source.source_type()
                else {
                    panic!("scheme source")
                };
                let template = TypeKind::function_with_effects(
                    params.clone(),
                    return_type.as_ref().clone(),
                    effects.clone(),
                );
                Ok(Some(source.prepare_function_scheme(
                    1,
                    scheme_parameters(),
                    template,
                    context,
                )?))
            },
        )
        .unwrap();
    assert_eq!(transaction.frontier.len(), 1);
    let input = Arc::clone(
        transaction.frontier[0]
            .lineage
            .projections
            .last()
            .unwrap()
            .input
            .as_ref()
            .unwrap(),
    );
    assert_eq!(
        input.value(),
        &TypeKind::function_with_binder(
            binder,
            [TypeKind::GenericParam(local)],
            TypeKind::Tuple(vec![TypeKind::I64]),
            crate::effect_row::EffectRow::closed(crate::effects::EffectSet::new())
        )
    );
    let ConstraintOperandOrigin::FunctionSchemeInstantiation {
        instance,
        source: actual_source,
    } = transaction.frontier[0]
        .equations
        .last()
        .unwrap()
        .pattern
        .origin
        .as_ref()
    else {
        panic!("the specialization retains its registered instance")
    };
    assert!(Arc::ptr_eq(actual_source, &input));
    assert!(Arc::ptr_eq(
        &instance.request,
        transaction.frontier[0].lineage.projections.last().unwrap()
    ));
    assert!(Arc::ptr_eq(
        instance.request.input.as_ref().unwrap(),
        &input
    ));
    assert_eq!(input.result_origin().unwrap().application(), application);
    let ConstraintOperandOrigin::Rebuilt {
        source, children, ..
    } = input.origin.as_ref()
    else {
        panic!("the retained use input is the corresponding normalized result")
    };
    let ConstraintOperandOrigin::Projection { producer, .. } = source.as_ref() else {
        panic!("the normalized result retains its original occurrence")
    };
    let ConstraintOperandOrigin::Result(result) = producer.as_ref() else {
        panic!("the original result is resolved from the selected request")
    };
    assert!(Arc::ptr_eq(&result.request, &request));
    let ConstraintOperandOrigin::Rebuilt { children, .. } = children[1].origin.as_ref() else {
        panic!("the function return retains its tuple projection")
    };
    let ConstraintOperandOrigin::TypeAlias { steps, .. } = children[0].origin.as_ref() else {
        panic!("the input retains the actual binding substitution")
    };
    assert_eq!(steps.len(), 1);
    assert!(Arc::ptr_eq(&steps[0].replacement.value, &replacement.value));
    drop(request);
    drop(replacement);
    drop(transaction);
    assert!(retained_replacement.upgrade().is_some());
    drop(input);
    assert!(retained_replacement.upgrade().is_none());
}

type ResultContext<'a> =
    TypeConstraintContext<'a, LocalConstraintAccounting<'a>, ApplicationSourceDomain>;

fn function_result(
    cancellation: &AtomicBool,
) -> (
    ResultContext<'_>,
    TypeConstraintTransaction<ApplicationSourceDomain>,
    TypeKind,
) {
    let (mut context, parameters) =
        ConstraintTestSetup::<LocalConstraintAccounting<'_>, ApplicationSourceDomain>::new(
            TypeConstraintLimits::new(4096, 2048, 128, 128),
            cancellation,
        )
        .into_parts();
    let binder = GenericBinder::new(1, 0, 0);
    let local = crate::types::GenericScope::default()
        .with_binder(binder)
        .bound_type(0, 0)
        .unwrap();
    let scheme = TypeKind::function_with_binder(
        binder,
        [TypeKind::GenericParam(local)],
        TypeKind::I64,
        crate::effect_row::EffectRow::closed(crate::effects::EffectSet::new()),
    );
    let mut transaction =
        TypeConstraintTransaction::initialize_fixture(&mut context, 0, parameters, None).unwrap();
    transaction.request_projection(
        &mut context,
        (),
        &scheme,
        crate::types::constraints::TypeConstraintProjectionClosure::Closed,
    );
    (context, transaction, scheme)
}

#[test]
fn a_scheme_opening_rejects_changed_value_ports_headers_and_parameter_roles() {
    for case in 0..5 {
        let cancellation = AtomicBool::new(false);
        let (mut context, mut transaction, scheme) = function_result(&cancellation);
        let result =
            transaction.constrain_result_use(&mut context, (), &scheme, |source, context| {
                let TypeKind::Function {
                    binder,
                    params,
                    return_type,
                    effects,
                    ..
                } = source.source_type()
                else {
                    panic!("scheme")
                };
                let template = match case {
                    0 => TypeKind::String,
                    1 => TypeKind::function_with_effects(
                        [TypeKind::Bool],
                        return_type.as_ref().clone(),
                        effects.clone(),
                    ),
                    2 => TypeKind::function_with_effects(
                        params.clone(),
                        TypeKind::Bool,
                        effects.clone(),
                    ),
                    3 => TypeKind::function_with_binder(
                        *binder,
                        params.clone(),
                        return_type.as_ref().clone(),
                        effects.clone(),
                    ),
                    _ => TypeKind::function_with_effects(
                        params.clone(),
                        return_type.as_ref().clone(),
                        effects.clone(),
                    ),
                };
                let parameters = if case == 4 {
                    TypeConstraintParameterScope::empty()
                } else {
                    scheme_parameters()
                };
                Ok(Some(source.prepare_function_scheme(
                    1, parameters, template, context,
                )?))
            });
        assert!(
            matches!(
                result,
                Err(crate::types::constraints::TypeConstraintFailure::Invariant(
                    crate::types::constraints::TypeConstraintFailureInvariant::Constraint(
                        crate::types::constraints::TypeConstraintInvariant::OriginalProducer
                    )
                ))
            ),
            "case {case}"
        );
    }
}

#[test]
fn equal_result_types_cannot_exchange_their_prepared_scheme_openings() {
    let cancellation = AtomicBool::new(false);
    let (mut context, mut transaction, scheme) = function_result(&cancellation);
    let second = context.fork_path(&transaction.frontier[0]).unwrap();
    transaction.frontier.push(second);
    let mut retained = None;
    let mut visits = 0;
    let result = transaction.constrain_result_use(&mut context, (), &scheme, |source, context| {
        visits += 1;
        if let Some(prepared) = retained.take() {
            return Ok(Some(prepared));
        }
        let TypeKind::Function {
            params,
            return_type,
            effects,
            ..
        } = source.source_type()
        else {
            panic!("scheme")
        };
        let template = TypeKind::function_with_effects(
            params.clone(),
            return_type.as_ref().clone(),
            effects.clone(),
        );
        retained =
            Some(source.prepare_function_scheme(1, scheme_parameters(), template, context)?);
        Ok(None)
    });
    assert_eq!(visits, 2);
    assert!(matches!(
        result,
        Err(crate::types::constraints::TypeConstraintFailure::Invariant(
            crate::types::constraints::TypeConstraintFailureInvariant::Constraint(
                crate::types::constraints::TypeConstraintInvariant::OriginalProducer
            )
        ))
    ));
}

#[test]
fn a_scheme_opening_requires_its_original_effect_predicate_in_the_parameter_contract() {
    use crate::effect_row::{
        EffectConstraintEligibility, EffectConstraintVariable, EffectFormula, EffectPredicate,
        EffectRow,
    };
    use crate::effects::EffectSet;
    use crate::types::constraints::TypeConstraintEffectScope;

    let cancellation = AtomicBool::new(false);
    let (mut context, parameters) =
        ConstraintTestSetup::<LocalConstraintAccounting<'_>, ApplicationSourceDomain>::new(
            TypeConstraintLimits::new(4096, 2048, 128, 128),
            &cancellation,
        )
        .into_parts();
    let binder = GenericBinder::new(0, 0, 1);
    let variable = crate::types::GenericScope::default()
        .with_binder(binder)
        .bound_effect(0, 0)
        .unwrap();
    let log = EffectSet::from_labels(["log.write"]).unwrap();
    let predicate = EffectFormula::variable(variable.clone(), &mut context)
        .unwrap()
        .subset(&EffectFormula::literal(log, None), &mut context)
        .unwrap();
    let scheme = TypeKind::function_with_contract(
        binder,
        predicate.clone(),
        [],
        TypeKind::I64,
        EffectRow::open(EffectSet::new(), variable.clone()),
    );
    let mut transaction =
        TypeConstraintTransaction::initialize_fixture(&mut context, 0, parameters, None).unwrap();
    transaction.request_projection(
        &mut context,
        (),
        &scheme,
        crate::types::constraints::TypeConstraintProjectionClosure::Closed,
    );
    transaction
        .constrain_result_use(
            &mut context,
            (),
            &TypeKind::function_with_effects(
                [],
                TypeKind::I64,
                EffectRow::closed(EffectSet::new()),
            ),
            |source, context| {
                let parameters = |predicate| {
                    TypeConstraintParameterScope::seal_call_scope(
                        binder,
                        [],
                        [],
                        TypeConstraintEffectScope::seal_call_scope_with_predicate(
                            [EffectConstraintVariable::new(
                                variable.clone(),
                                EffectConstraintEligibility::Bindable,
                            )],
                            [],
                            predicate,
                        )
                        .unwrap(),
                        [],
                        [],
                    )
                    .unwrap()
                };
                let template = TypeKind::function_with_effects(
                    [],
                    TypeKind::I64,
                    EffectRow::open(EffectSet::new(), variable.clone()),
                );
                assert!(matches!(
                    source.prepare_function_scheme(
                        1,
                        parameters(EffectPredicate::unconstrained()),
                        template.clone(),
                        context
                    ),
                    Err(crate::types::constraints::TypeConstraintError::Invariant(
                        crate::types::constraints::TypeConstraintInvariant::OriginalProducer
                    ))
                ));
                Ok(Some(source.prepare_function_scheme(
                    1,
                    parameters(predicate.clone()),
                    template,
                    context,
                )?))
            },
        )
        .unwrap();
    let solved = transaction
        .finish(&mut context, |left, right| Ok(left == right))
        .unwrap();
    let projected = solved
        .component
        .projection(1, &())
        .unwrap()
        .projection()
        .value();
    let TypeKind::Function { effects, .. } = projected.value() else {
        panic!("specialized function")
    };
    assert!(effects.is_empty());
}
