use std::{collections::BTreeMap, sync::atomic::AtomicBool};

use crate::{
    callable::VectorDimensions,
    types::{
        DetachedGenericOwnerId, FixedVectorType, GenericParameterOwnerId, GenericTypeParameterId,
        TypeKind,
        constraints::{
            NoConstraintClient, TypeConstraintError, TypeConstraintParameterEligibility,
            TypeConstraintParameterScope, TypeConstraintRejection, TypeConstraintSolution,
            context::{LocalConstraintAccounting, TypeConstraintContext, TypeConstraintLimits},
        },
    },
};

fn parameter(ordinal: u16) -> GenericTypeParameterId {
    GenericTypeParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(716)),
        ordinal,
    )
}

fn vector(dimensions: VectorDimensions, component: TypeKind) -> TypeKind {
    TypeKind::FixedVector(FixedVectorType::new(dimensions, component))
}

#[test]
fn vector_components_open_and_complete_through_the_lower_constraint_authority() {
    for dimensions in [
        VectorDimensions::Two,
        VectorDimensions::Three,
        VectorDimensions::Four,
    ] {
        let scope = TypeConstraintParameterScope::new([
            (parameter(0), TypeConstraintParameterEligibility::Bindable),
            (parameter(1), TypeConstraintParameterEligibility::Bindable),
        ])
        .expect("template parameters");
        let cancellation = AtomicBool::new(false);
        let mut context =
            TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
                TypeConstraintLimits::new(256, 128, 32, 16),
                &cancellation,
                scope,
            );
        let opened_vector = context
            .open_template_type(&vector(
                dimensions,
                TypeKind::generic_parameter(parameter(1)),
            ))
            .expect("component opens in the same template scope");
        let value = context
            .parameter_scope
            .type_reference(&(parameter(0)).clone().into())
            .expect("value slot");
        let component = context
            .parameter_scope
            .type_reference(&(parameter(1)).clone().into())
            .expect("component slot");
        assert_eq!(
            opened_vector,
            vector(dimensions, TypeKind::GenericParam(component.clone()))
        );
        let solution = TypeConstraintSolution::complete_path(
            BTreeMap::from([(value, opened_vector), (component, TypeKind::F32)]),
            BTreeMap::new(),
            BTreeMap::new(),
            &mut context,
        )
        .expect("component alias is normalized before completion");
        let (_, value) = solution
            .bindings()
            .find(|(key, _)| key.value() == &parameter(0).into())
            .expect("completed vector binding");
        assert_eq!(value.value(), &vector(dimensions, TypeKind::F32));
        assert!(value.semantic_identity_digest().is_ok());
    }
}

#[test]
fn cyclic_vector_component_is_rejected_before_a_solution_can_publish() {
    let scope = TypeConstraintParameterScope::new([(
        parameter(0),
        TypeConstraintParameterEligibility::Bindable,
    )])
    .expect("template parameter");
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(256, 128, 32, 16),
            &cancellation,
            scope,
        );
    let reference = context
        .parameter_scope
        .type_reference(&(parameter(0)).clone().into())
        .expect("type slot");
    let error = TypeConstraintSolution::complete_path(
        BTreeMap::from([(
            reference.clone(),
            vector(VectorDimensions::Three, TypeKind::GenericParam(reference)),
        )]),
        BTreeMap::new(),
        BTreeMap::new(),
        &mut context,
    )
    .expect_err("a component cycle is not an opaque leaf");
    assert!(matches!(
        error,
        TypeConstraintError::Rejected(TypeConstraintRejection::CyclicInstantiation { .. })
    ));
}

fn error_payload(ok: TypeKind) -> TypeKind {
    use crate::types::{
        AcceptedVariantCaseSemanticId, VariantPayloadOwnerFamily, VariantPayloadShape,
        VariantPayloadType,
    };
    let owner = TypeKind::Result {
        ok: Box::new(ok),
        error: Box::new(TypeKind::String),
    };
    let identity = owner
        .semantic_identity_digest()
        .expect("declaration owner type");
    let shape = VariantPayloadShape::try_tuple(
        VariantPayloadOwnerFamily::Result,
        identity,
        1,
        [TypeKind::String],
    )
    .expect("Err payload schema");
    let case = AcceptedVariantCaseSemanticId::issue(
        VariantPayloadOwnerFamily::Result,
        identity,
        1,
        &shape,
    );
    TypeKind::VariantPayload(Box::new(
        VariantPayloadType::try_new(VariantPayloadOwnerFamily::Result, owner, 1, case, shape)
            .expect("payload keeps its typed owner"),
    ))
}

#[test]
fn identical_payload_fields_do_not_widen_their_dependent_owner() {
    let exact = error_payload(TypeKind::I64);
    let wider_owner = error_payload(TypeKind::AgentValue);
    let mut control = crate::types::NoopTypeCompatibilityControl;
    let accepts = |expected: &TypeKind,
                   actual: &TypeKind,
                   control: &mut crate::types::NoopTypeCompatibilityControl| {
        expected
            .accepts_with(
                actual,
                crate::types::TypeCompatibilityPolicy::SelectedCall,
                control,
            )
            .expect("closed payload types")
    };
    assert!(accepts(&exact, &exact, &mut control));
    assert!(!accepts(&wider_owner, &exact, &mut control));
    assert!(!accepts(&exact, &wider_owner, &mut control));
}

#[test]
fn payload_relation_infers_parameters_used_only_by_the_owner() {
    use crate::types::constraints::{ConstraintAcceptance, relate_selected_call};

    let scope = TypeConstraintParameterScope::new([(
        parameter(0),
        TypeConstraintParameterEligibility::Bindable,
    )])
    .expect("callee type parameter");
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(4096, 2048, 128, 64),
            &cancellation,
            scope,
        );
    let expected = context
        .open_template_type(&error_payload(TypeKind::generic_parameter(parameter(0))))
        .expect("open payload owner");
    let actual = error_payload(TypeKind::I64);
    let path = context.start_path().expect("candidate path");
    let mut paths = relate_selected_call(
        &expected,
        &actual,
        path,
        &mut context,
        ConstraintAcceptance::PatternAcceptsActual,
    )
    .expect("payload relation");
    assert_eq!(paths.len(), 1);
    let path = paths.pop().expect("selected path");
    let solution = TypeConstraintSolution::complete_path(
        path.bindings,
        path.const_bindings,
        BTreeMap::new(),
        &mut context,
    )
    .expect("owner parameter is closed by the relation");
    let (bound, value) = solution.bindings().next().expect("inferred owner argument");
    assert_eq!(bound.value(), &parameter(0).into());
    assert_eq!(value.value(), &TypeKind::I64);
    assert!(value.scope().binders().is_empty());
}

#[test]
fn payload_owner_arguments_open_normalize_and_reseal_with_the_fields() {
    let scope = TypeConstraintParameterScope::new([
        (parameter(0), TypeConstraintParameterEligibility::Bindable),
        (parameter(1), TypeConstraintParameterEligibility::Bindable),
    ])
    .expect("template parameters");
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(1024, 512, 32, 16),
            &cancellation,
            scope,
        );
    let declared = error_payload(TypeKind::generic_parameter(parameter(1)));
    let TypeKind::VariantPayload(before) = &declared else {
        panic!("payload fixture")
    };
    let before = before.try_seal().expect("declaration payload");
    let opened = context
        .open_template_type(&declared)
        .expect("logical owner can contain inference");
    assert!(matches!(
        opened.semantic_identity_digest(),
        Err(crate::types::GenericScopeError::EscapedInference { .. })
    ));
    let root = context
        .parameter_scope
        .type_reference(&(parameter(0)).clone().into())
        .expect("result slot");
    let argument = context
        .parameter_scope
        .type_reference(&(parameter(1)).clone().into())
        .expect("owner argument slot");
    let solution = TypeConstraintSolution::complete_path(
        BTreeMap::from([(root, opened), (argument, TypeKind::I64)]),
        BTreeMap::new(),
        BTreeMap::new(),
        &mut context,
    )
    .expect("the owner and fields close together");
    let (_, value) = solution
        .bindings()
        .find(|(key, _)| key.value() == &parameter(0).into())
        .expect("payload binding");
    let TypeKind::VariantPayload(projected) = value.value() else {
        panic!("payload result")
    };
    let checked = projected.try_seal().expect("closed payload rows");
    assert_eq!(
        projected.owner_type(),
        &TypeKind::Result {
            ok: Box::new(TypeKind::I64),
            error: Box::new(TypeKind::String)
        }
    );
    assert_eq!(
        checked.semantic_type(),
        value.semantic_identity_digest().expect("closed identity")
    );
    assert_ne!(checked.owner_semantic_type(), before.owner_semantic_type());
    assert_ne!(checked.case(), before.case());
    let before_field = &before.shape().tuple_fields().expect("tuple payload")[0];
    let after_field = &checked.shape().tuple_fields().expect("tuple payload")[0];
    assert_eq!(before_field.ty(), after_field.ty());
    assert_ne!(before_field.semantic_id(), after_field.semantic_id());
}

#[test]
fn occurs_check_includes_parameters_used_only_by_the_payload_owner() {
    let scope = TypeConstraintParameterScope::new([(
        parameter(0),
        TypeConstraintParameterEligibility::Bindable,
    )])
    .expect("template parameter");
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(1024, 512, 32, 16),
            &cancellation,
            scope,
        );
    let opened = context
        .open_template_type(&error_payload(TypeKind::generic_parameter(parameter(0))))
        .expect("logical payload owner");
    let reference = context
        .parameter_scope
        .type_reference(&(parameter(0)).clone().into())
        .expect("type slot");
    let error = TypeConstraintSolution::complete_path(
        BTreeMap::from([(reference, opened)]),
        BTreeMap::new(),
        BTreeMap::new(),
        &mut context,
    )
    .expect_err("a phantom owner parameter still participates in occurs checks");
    assert!(matches!(
        error,
        TypeConstraintError::Rejected(TypeConstraintRejection::CyclicInstantiation { .. })
    ));
}

#[test]
fn residual_payload_owner_requires_its_retained_type_and_const_scope() {
    use crate::types::constraints::TypeConstraintConstEligibility;
    use crate::types::{ArrayLength, GenericConstParameterId};
    let constant = GenericConstParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(716)),
        0,
    );
    let scope = TypeConstraintParameterScope::new_with_constants(
        [
            (parameter(0), TypeConstraintParameterEligibility::Bindable),
            (
                parameter(1),
                TypeConstraintParameterEligibility::FutureEligible,
            ),
        ],
        [(
            constant.clone(),
            TypeConstraintConstEligibility::FutureEligible,
        )],
    )
    .expect("residual owner parameters");
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(1024, 512, 32, 16),
            &cancellation,
            scope,
        );
    let declared = error_payload(TypeKind::Array {
        item: Box::new(TypeKind::generic_parameter(parameter(1))),
        len: ArrayLength::generic_parameter(constant),
    });
    let opened = context
        .open_template_type(&declared)
        .expect("open owner arguments");
    let root = context
        .parameter_scope
        .type_reference(&(parameter(0)).clone().into())
        .expect("result slot");
    let solution = TypeConstraintSolution::complete_path(
        BTreeMap::from([(root, opened)]),
        BTreeMap::new(),
        BTreeMap::new(),
        &mut context,
    )
    .expect("future owner arguments become residual bound references");
    let (_, value) = solution.bindings().next().expect("payload binding");
    assert!(value.semantic_identity_digest().is_ok());
    assert!(matches!(
        value.value().semantic_identity_digest(),
        Err(crate::types::GenericScopeError::UnknownDepth { .. })
    ));
    let TypeKind::VariantPayload(payload) = value.value() else {
        panic!("payload result")
    };
    assert!(matches!(
        payload.try_seal(),
        Err(crate::types::VariantPayloadSealError::InvalidOwnerScope(_))
    ));
}
