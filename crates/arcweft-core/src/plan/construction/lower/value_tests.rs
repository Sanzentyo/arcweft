use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::pattern::{
    RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId,
};
use crate::plan::{
    RuntimeDialogueContentSlotSeed, RuntimeDialogueContentTemplateManifestSeed,
    RuntimeDialogueValueRole, RuntimeFormatContentOperandSeed, RuntimePlan,
    RuntimePlanTypeProjection as Type, RuntimePlanTypeSeed,
};
use crate::runtime_id::{RuntimeDialogueContentTemplateId, RuntimeDialogueValueSlotId};
use crate::value::{
    RuntimeDialogueOpaqueRole, RuntimeFmtParameterId, RuntimeIterator, RuntimeOpaquePersistence,
    RuntimeOpaqueValueClass, RuntimeRange,
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

#[test]
fn need_handle_is_live_input_and_never_a_plan_constant() {
    let types = || [seed(1, Type::String), seed(2, Type::Need(semantic(1)))];
    let value = RuntimeValue::Need(crate::task::NeedId("need.input".to_owned()));
    let mut builder = RuntimePlanBuilder::new();
    builder.admit_type_batch(types(), []).unwrap();
    assert!(matches!(
        builder.lower_expression(RuntimeExprSeed::new(
            semantic(2),
            RuntimeExprSeedKind::Value(value.clone()),
        )),
        Err(RuntimePlanBuildError::InvalidValueType { .. })
    ));

    let sealed = plan(types());
    let ty = sealed.type_table().id_for_semantic(semantic(2)).unwrap();
    assert!(
        sealed
            .validate_live_value(
                ty,
                &value,
                crate::entry::RuntimeSchemaLimits::engine_default()
            )
            .is_ok()
    );
    assert!(admitted(&sealed, 2, &value).is_err());
}

#[test]
fn format_content_lowering_keeps_operand_order_and_rejects_unwitnessed_values() {
    let content_owner = RuntimeDialogueOpaqueRole::Content.exact_owner();
    let content = content_owner.semantic_identity();
    let string = semantic(12);
    let integer = semantic(13);
    let tuple = semantic(14);
    let template = RuntimeDialogueContentTemplateId::from_zero_based(0)
        .expect("first format template identity");
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(
                    content,
                    Type::Opaque {
                        producer: content_owner.producer().clone(),
                        admission: content_owner.admission(),
                        value_class: content_owner.value_class(),
                        persistence: content_owner.persistence(),
                        arguments: Box::new([]),
                    },
                ),
                seed(12, Type::String),
                seed(13, Type::Signed(RuntimeSignedIntWidth::I64)),
                seed(14, Type::Tuple(Box::new([string]))),
            ],
            [],
        )
        .expect("format types admit");
    builder
        .register_dialogue_content_template_seed(RuntimeDialogueContentTemplateManifestSeed {
            id: template,
            digest: crate::entry::RuntimeDialogueContentTemplateDigest::from_bytes([0x36; 32]),
            slots: vec![RuntimeDialogueContentSlotSeed {
                slot: RuntimeDialogueValueSlotId::from_zero_based(0)
                    .expect("first format template slot"),
                role: RuntimeDialogueValueRole::Formatted,
                semantic_type: content,
            }]
            .into_boxed_slice(),
            effects: Box::new([]),
        })
        .expect("exact format template registers");

    let invalid = RuntimeExprSeed::format_content(
        content,
        template,
        [RuntimeFormatContentOperandSeed::new(
            RuntimeFmtParameterId::Value,
            RuntimeExprSeed::new(
                tuple,
                RuntimeExprSeedKind::Value(RuntimeValue::Tuple(vec![RuntimeValue::String(
                    "not display-witnessed".to_owned(),
                )])),
            ),
        )],
    );
    assert!(matches!(
        builder.lower_expression(invalid),
        Err(RuntimePlanBuildError::InvalidFormatParameterType {
            parameter: RuntimeFmtParameterId::Value,
            ty: _
        })
    ));

    let valid = RuntimeExprSeed::format_content(
        content,
        template,
        [
            RuntimeFormatContentOperandSeed::new(
                RuntimeFmtParameterId::Style,
                RuntimeExprSeed::new(
                    string,
                    RuntimeExprSeedKind::Value(RuntimeValue::String("number".to_owned())),
                ),
            ),
            RuntimeFormatContentOperandSeed::new(
                RuntimeFmtParameterId::Value,
                RuntimeExprSeed::new(integer, RuntimeExprSeedKind::Value(RuntimeValue::i64(42))),
            ),
        ],
    );
    let lowered = builder
        .lower_expression(valid)
        .expect("witnessed fmt value lowers");
    let RuntimeExprKind::FormatContent {
        template: actual_template,
        operands,
    } = lowered.kind()
    else {
        panic!("format Content remains a dedicated expression");
    };
    assert_eq!(*actual_template, template);
    assert_eq!(
        operands
            .iter()
            .map(crate::value::RuntimeFormatContentOperand::parameter)
            .collect::<Vec<_>>(),
        [RuntimeFmtParameterId::Style, RuntimeFmtParameterId::Value]
    );
}
