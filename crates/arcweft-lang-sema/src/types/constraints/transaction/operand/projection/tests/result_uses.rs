use super::*;
use crate::types::constraints::transaction::{
    TypeConstraintTransaction, tests::ApplicationSourceDomain,
};
use crate::types::constraints::{TypeConstraintParameterEligibility, TypeConstraintParameterScope};
use crate::types::{DetachedGenericOwnerId, GenericParameterOwnerId, GenericTypeParameterId};

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
    transaction.request_projection(
        &mut context,
        (),
        &TypeKind::Tuple(vec![TypeKind::GenericParam(reference)]),
        crate::types::constraints::TypeConstraintProjectionClosure::Closed,
    );
    let request = Arc::clone(&transaction.frontier[0].lineage.projections[0]);
    transaction
        .constrain_result_use(
            &mut context,
            (),
            &TypeKind::Tuple(vec![TypeKind::I64]),
            |source, _| {
                Ok(Some((
                    1,
                    TypeConstraintParameterScope::empty(),
                    source.clone(),
                )))
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
    assert_eq!(input.value(), &TypeKind::Tuple(vec![TypeKind::I64]));
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
