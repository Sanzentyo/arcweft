use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::pattern::{
    RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId,
};
use crate::plan::{RuntimePlan, RuntimePlanTypeProjection as Type, RuntimePlanTypeSeed};
use crate::value::{
    RuntimeIterator, RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeRange,
};

#[test]
fn exact_opaque_branches_keep_their_identities_under_a_producer_wide_result() {
    let producer = RuntimeOpaqueTypeProducerId::try_new("test.dialogue")
        .expect("test opaque producer identity");
    let other =
        RuntimeOpaqueTypeProducerId::try_new("test.other").expect("other opaque producer identity");
    let opaque = |producer: RuntimeOpaqueTypeProducerId, admission| Type::Opaque {
        producer,
        admission,
        value_class: RuntimeOpaqueValueClass::Plain,
        persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
        arguments: Vec::new().into_boxed_slice(),
    };
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                seed(
                    1,
                    opaque(producer.clone(), RuntimeOpaqueTypeAdmission::ProducerWide),
                ),
                seed(
                    2,
                    opaque(producer.clone(), RuntimeOpaqueTypeAdmission::ExactIdentity),
                ),
                seed(
                    3,
                    opaque(producer.clone(), RuntimeOpaqueTypeAdmission::ExactIdentity),
                ),
                seed(4, Type::Bool),
                seed(
                    5,
                    opaque(other.clone(), RuntimeOpaqueTypeAdmission::ExactIdentity),
                ),
            ],
            [],
        )
        .expect("checked opaque type graph");
    let value = |tag: u8, producer: RuntimeOpaqueTypeProducerId| {
        RuntimeExprSeed::new(
            semantic(tag),
            RuntimeExprSeedKind::Value(
                RuntimeOpaqueTypeOwner::exact(producer, semantic(tag))
                    .try_wrap(RuntimeValue::Unit)
                    .expect("valid exact opaque value"),
            ),
        )
    };
    let conditional = |right: RuntimeExprSeed| {
        RuntimeExprSeed::new(
            semantic(1),
            RuntimeExprSeedKind::If {
                condition: Box::new(RuntimeExprSeed::new(
                    semantic(4),
                    RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
                )),
                then_expr: Box::new(value(2, producer.clone())),
                else_expr: Box::new(right),
            },
        )
    };
    let accepted = builder
        .lower_expression(conditional(value(3, producer.clone())))
        .expect("exact values of one producer widen to its top type");
    let RuntimeExprKind::If {
        then_expr,
        else_expr,
        ..
    } = accepted.kind()
    else {
        panic!("admitted conditional remains structural");
    };
    assert_ne!(then_expr.ty(), accepted.ty());
    assert_ne!(else_expr.ty(), accepted.ty());
    assert!(matches!(
        builder.lower_expression(conditional(value(5, other))),
        Err(RuntimePlanBuildError::TypeMismatch {
            context: "if else branch",
            ..
        })
    ));
}

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}
fn seed(tag: u8, projection: Type<RuntimeSemanticTypeId>) -> RuntimePlanTypeSeed {
    RuntimePlanTypeSeed::new(semantic(tag), projection)
}
fn plan(types: impl IntoIterator<Item = RuntimePlanTypeSeed>) -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    builder.admit_type_batch(types, []).unwrap();
    builder.finish().unwrap()
}
fn admitted(
    plan: &RuntimePlan,
    tag: u8,
    value: &RuntimeValue,
) -> Result<crate::entry::RuntimeValueDigest, crate::plan::RuntimePlanValueAdmissionError> {
    plan.accepts_value(
        plan.type_table().id_for_semantic(semantic(tag)).unwrap(),
        value,
        crate::entry::RuntimeSchemaLimits::engine_default(),
    )
}
#[test]
fn literal_builder_uses_the_same_choice_rule_and_preserves_runtime_only_values() {
    let types = || {
        [
            seed(1, Type::Choice(Box::new([semantic(2), semantic(3)]))),
            seed(2, Type::Bool),
            seed(3, Type::AgentValue),
            seed(4, Type::Iterator(semantic(5))),
            seed(5, Type::Signed(RuntimeSignedIntWidth::I16)),
            seed(6, Type::Range(semantic(5))),
        ]
    };
    let mut builder = RuntimePlanBuilder::new();
    builder.admit_type_batch(types(), []).unwrap();
    assert!(matches!(
        builder.lower_expression(RuntimeExprSeed::new(
            semantic(1),
            RuntimeExprSeedKind::Value(RuntimeValue::Bool(true))
        )),
        Err(RuntimePlanBuildError::InvalidValueType { .. })
    ));
    let mut builder = RuntimePlanBuilder::new();
    builder.admit_type_batch(types(), []).unwrap();
    let iterator = RuntimeValue::Iterator(RuntimeIterator::Values {
        items: vec![RuntimeValue::i16(1)],
        index: 0,
    });
    let range = RuntimeValue::Range(RuntimeRange::Int {
        start: Some(crate::value::RuntimeInt::I16(1)),
        end: Some(crate::value::RuntimeInt::I16(3)),
        inclusive: false,
    });
    for (tag, value) in [(4, iterator), (6, range)] {
        builder
            .lower_expression(RuntimeExprSeed::new(
                semantic(tag),
                RuntimeExprSeedKind::Value(value.clone()),
            ))
            .unwrap();
        let sealed = plan(types());
        assert!(admitted(&sealed, tag, &value).is_err());
    }
}
