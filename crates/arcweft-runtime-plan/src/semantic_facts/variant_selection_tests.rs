use super::{
    RuntimeAgentTypeShape, RuntimeBuiltinVariantIdentity, RuntimeNormalizedType,
    RuntimeNormalizedVariantSelectionError, RuntimeSemanticTypeId, RuntimeTypeShape,
};

fn ty(marker: u8, shape: RuntimeTypeShape) -> RuntimeNormalizedType {
    RuntimeNormalizedType::new(RuntimeSemanticTypeId::from_bytes([marker; 32]), shape)
}

fn tuple(marker: u8, item: RuntimeNormalizedType) -> RuntimeNormalizedType {
    ty(marker, RuntimeTypeShape::Tuple(Box::new([item])))
}

fn result(value: RuntimeNormalizedType) -> RuntimeNormalizedType {
    let error = ty(2, RuntimeTypeShape::Unit);
    ty(
        3,
        RuntimeTypeShape::Result {
            value_payload: Box::new(tuple(4, value.clone())),
            error_payload: Box::new(tuple(5, error.clone())),
            value: Box::new(value),
            error: Box::new(error),
        },
    )
}

#[test]
fn variant_selection_preserves_agent_range_and_function_payloads() {
    let scalar = ty(
        10,
        RuntimeTypeShape::Signed(arcweft_core::value::RuntimeSignedIntWidth::I64),
    );
    for value in [
        ty(11, RuntimeTypeShape::Agent(RuntimeAgentTypeShape::Resource)),
        ty(12, RuntimeTypeShape::Range(Box::new(scalar.clone()))),
        ty(
            13,
            RuntimeTypeShape::Function {
                parameters: Box::new([scalar.clone()]),
                result: Box::new(scalar),
            },
        ),
    ] {
        let owner = result(value.clone());
        for (ordinal, expected) in [
            (0, value.identity()),
            (1, RuntimeSemanticTypeId::from_bytes([2; 32])),
        ] {
            let selected = owner
                .variant_selection(ordinal)
                .expect("complete normalized case");
            assert_eq!(selected.owner().identity(), owner.identity());
            assert_eq!(selected.ordinal(), ordinal);
            assert_eq!(
                selected
                    .single_payload_item()
                    .expect("unary payload")
                    .expect("payload")
                    .identity(),
                expected
            );
        }
        let owner = ty(
            6,
            RuntimeTypeShape::Option {
                some_payload: Box::new(tuple(7, value.clone())),
                item: Box::new(value.clone()),
            },
        );
        assert_eq!(
            owner
                .variant_selection(0)
                .expect("Some")
                .single_payload_item()
                .expect("tuple"),
            Some(&value)
        );
        assert!(
            owner
                .variant_selection(1)
                .expect("None")
                .payload()
                .is_none()
        );
    }
}

#[test]
fn variant_selection_rejects_same_shape_with_a_different_payload_identity() {
    let value = ty(20, RuntimeTypeShape::Bool);
    let alias = ty(21, RuntimeTypeShape::Bool);
    let error = ty(22, RuntimeTypeShape::Unit);
    for owner in [
        ty(
            23,
            RuntimeTypeShape::Result {
                value: Box::new(value.clone()),
                error: Box::new(error.clone()),
                value_payload: Box::new(tuple(24, alias.clone())),
                error_payload: Box::new(tuple(25, error)),
            },
        ),
        ty(
            26,
            RuntimeTypeShape::Option {
                item: Box::new(value),
                some_payload: Box::new(tuple(27, alias)),
            },
        ),
    ] {
        for ordinal in [0, 1] {
            assert!(matches!(
                owner.variant_selection(ordinal),
                Err(RuntimeNormalizedVariantSelectionError::PayloadMismatch { ordinal: 0, .. })
            ));
        }
    }
}

#[test]
fn variant_selection_validates_the_unselected_result_payload() {
    let value = ty(30, RuntimeTypeShape::String);
    let error = ty(31, RuntimeTypeShape::Unit);
    for payload in [
        ty(32, RuntimeTypeShape::Tuple(Box::new([]))),
        ty(
            33,
            RuntimeTypeShape::Tuple(Box::new([error.clone(), error.clone()])),
        ),
        error.clone(),
    ] {
        let owner = ty(
            34,
            RuntimeTypeShape::Result {
                value_payload: Box::new(tuple(35, value.clone())),
                error_payload: Box::new(payload),
                value: Box::new(value.clone()),
                error: Box::new(error.clone()),
            },
        );
        assert!(owner.variant_selection(0).is_err());
    }
}

#[test]
fn variant_selection_uses_the_builtin_case_count_and_payload_schema() {
    let payload = tuple(40, ty(41, RuntimeTypeShape::Unit));
    for cases in [
        vec![Some(payload.clone())],
        vec![Some(payload.clone()), None, None],
        vec![None, None],
        vec![Some(payload.clone()), Some(payload.clone())],
    ] {
        let owner = ty(
            42,
            RuntimeTypeShape::BuiltinVariant {
                owner: RuntimeBuiltinVariantIdentity::Option,
                cases: cases.into_boxed_slice(),
            },
        );
        assert!(owner.variant_selection(0).is_err());
    }
    let owner = ty(
        43,
        RuntimeTypeShape::BuiltinVariant {
            owner: RuntimeBuiltinVariantIdentity::Option,
            cases: Box::new([Some(payload.clone()), None]),
        },
    );
    assert_eq!(
        owner.variant_selection(0).expect("Some").payload(),
        Some(&payload)
    );
    assert!(
        owner
            .variant_selection(1)
            .expect("None")
            .payload()
            .is_none()
    );
    assert!(matches!(
        owner.variant_selection(2),
        Err(RuntimeNormalizedVariantSelectionError::CaseOrdinal { .. })
    ));
    assert!(matches!(
        payload.variant_selection(0),
        Err(RuntimeNormalizedVariantSelectionError::InvalidOwner { .. })
    ));
}
