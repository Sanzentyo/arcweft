use super::*;
use crate::{
    entry::RuntimeSchemaLimits,
    pattern::RuntimeSemanticTypeId,
    presentation::{
        RuntimePresentationFactoryCall,
        factory_test_support::{ALIAS, BOOL, HANDLE, RESOURCE, fixture},
    },
    value::RuntimeCallTarget,
};
use arcweft_interaction_model::presentation::{
    PresentationArgumentKey, PresentationArgumentRole, PresentationCallableId,
};
use std::sync::Arc;
#[test]
fn factory_builder_rejects_wrong_input_and_result_types_before_expression_publication() {
    for (input, result, accepted) in [
        (BOOL, HANDLE, true),
        (ALIAS, HANDLE, false),
        (BOOL, BOOL, false),
    ] {
        let (builder, call) = fixture();
        let expression = builder.lower_expression(RuntimeExprSeed::new(
            result,
            RuntimeExprSeedKind::Call {
                callee: RuntimeCallTarget::PresentationFactory(call),
                args: Box::new([RuntimeCallArgumentSeed::new(
                    RuntimeExprSeed::new(
                        input,
                        RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
                    ),
                    RuntimeCallArgumentMode::Value,
                    0,
                )]),
            },
        ));
        assert_eq!(
            expression.is_ok(),
            accepted,
            "input {input:?}, result {result:?}: {expression:?}"
        );
    }
}

#[test]
fn factory_builder_preserves_source_order_and_expands_typed_tuple_and_array_spreads() {
    for mode in ["reordered", "tuple", "array"] {
        let (mut builder, _) = fixture();
        let fields = if mode == "array" {
            [BOOL, BOOL]
        } else {
            [BOOL, ALIAS]
        };
        let keys: Box<[_]> = ["first", "second"]
            .map(|key| {
                PresentationArgumentKey::try_new(
                    PresentationArgumentRole::Parameter,
                    Some(key.into()),
                )
                .unwrap()
            })
            .into();
        let body = RuntimePresentationFactoryCall::retained_body_identity(
            PresentationCallableId::Image,
            &keys,
            RESOURCE,
            &crate::plan::RuntimeTypeScope::root(),
            2,
            |index| fields.get(index).copied(),
        )
        .unwrap();
        let container = RuntimeSemanticTypeId::from_bytes([96; 32]);
        let projection = if mode == "array" {
            RuntimePlanTypeProjection::Array {
                item: BOOL,
                length: 2.into(),
            }
        } else {
            RuntimePlanTypeProjection::Tuple(Box::new(fields))
        };
        builder
            .admit_type_batch(
                [
                    crate::plan::RuntimePlanTypeSeed::new(
                        body,
                        RuntimePlanTypeProjection::Tuple(Box::new(fields)),
                    ),
                    crate::plan::RuntimePlanTypeSeed::new(container, projection),
                ],
                [],
            )
            .unwrap();
        let call = Arc::new(
            RuntimePresentationFactoryCall::try_new(
                PresentationCallableId::Image,
                keys,
                RESOURCE,
                body,
                RuntimeSchemaLimits::engine_default(),
            )
            .unwrap(),
        );
        let literal = |ty, value| {
            RuntimeExprSeed::new(ty, RuntimeExprSeedKind::Value(RuntimeValue::Bool(value)))
        };
        let args = match mode {
            "reordered" => vec![
                RuntimeCallArgumentSeed::new(
                    literal(ALIAS, false),
                    RuntimeCallArgumentMode::Value,
                    1,
                ),
                RuntimeCallArgumentSeed::new(
                    literal(BOOL, true),
                    RuntimeCallArgumentMode::Value,
                    0,
                ),
            ],
            "tuple" => vec![RuntimeCallArgumentSeed::new(
                RuntimeExprSeed::new(
                    container,
                    RuntimeExprSeedKind::Tuple(Box::new([
                        literal(BOOL, true),
                        literal(ALIAS, false),
                    ])),
                ),
                RuntimeCallArgumentMode::Spread,
                0,
            )],
            "array" => vec![RuntimeCallArgumentSeed::new(
                RuntimeExprSeed::new(
                    container,
                    RuntimeExprSeedKind::Value(RuntimeValue::Seq(
                        crate::value::RuntimeSeq::values(vec![
                            RuntimeValue::Bool(true),
                            RuntimeValue::Bool(false),
                        ]),
                    )),
                ),
                RuntimeCallArgumentMode::Spread,
                0,
            )],
            _ => unreachable!(),
        };
        let expression = builder
            .lower_expression(RuntimeExprSeed::new(
                HANDLE,
                RuntimeExprSeedKind::Call {
                    callee: RuntimeCallTarget::PresentationFactory(call),
                    args: args.into_boxed_slice(),
                },
            ))
            .unwrap_or_else(|error| panic!("{mode}: {error}"));
        let RuntimeExprKind::Call { args, .. } = expression.kind() else {
            unreachable!()
        };
        let positions = args
            .iter()
            .map(RuntimeCallArgument::abi_position)
            .collect::<Vec<_>>();
        assert_eq!(
            positions,
            if mode == "reordered" {
                vec![1, 0]
            } else {
                vec![0]
            }
        );
    }
}
