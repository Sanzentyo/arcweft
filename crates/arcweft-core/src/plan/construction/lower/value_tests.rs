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
fn inert_literal_sources_are_admitted_under_the_actual_building_type_domain_authority() {
    use crate::entry::schema::value_encoding::{ValueAdmissionOrigin, ValueResourceAdmission};
    use crate::plan::value_admission::PlanValueAuthority;
    use crate::value::AwbcRuntimeValueSnapshot;
    let fixture = || {
        crate::plan::type_table::source_codec::domains::tests::builder(
            crate::entry::RuntimeNominalRecordShape::Record,
            2,
            true,
        )
    };
    let original = fixture()
        .prepare_inventory(crate::plan::RuntimeTaskPlanSealLimits::default(), 0)
        .unwrap();
    let semantic = RuntimeSemanticTypeId::from_bytes([1; 32]);
    let value = crate::program_types::RuntimeProgramTypes::Plan(&original)
        .try_record_value(
            semantic,
            vec![RuntimeValue::Bool(true), RuntimeValue::Bool(false)],
            crate::entry::RuntimeSchemaLimits::engine_default(),
        )
        .unwrap();
    let mut source = AwbcRuntimeValueSnapshot::from_admitted_storage(&value, None).unwrap();
    let builder = fixture();
    let ty = builder
        .resolve_seed_type("literal source", semantic)
        .unwrap();
    let admit = |source: &AwbcRuntimeValueSnapshot| {
        PlanValueAuthority::Building {
            types: &builder.types,
            records: &builder.nominal_record_domains,
            variants: &builder.variant_domains,
        }
        .admit_literal_source_value(
            ty,
            source,
            &mut ValueResourceAdmission::snapshot(
                crate::entry::RuntimeSchemaLimits::engine_default(),
                ValueAdmissionOrigin::InertSnapshot,
            ),
        )
    };
    admit(&source).unwrap();
    let AwbcRuntimeValueSnapshot::NominalRecord(record) = &mut source else {
        unreachable!()
    };
    record.fields[0].value = AwbcRuntimeValueSnapshot::String("wrong".into());
    assert!(admit(&source).is_err());
}

#[test]
fn materialized_building_literals_reach_the_actual_function_and_common_executable_seal() {
    building_literals_reach_the_common_executable_seal(LiteralSourceInput::Snapshot);
}

#[test]
fn binary_building_literals_reach_the_actual_function_and_common_executable_seal() {
    building_literals_reach_the_common_executable_seal(LiteralSourceInput::BinarySnapshot);
}

#[test]
fn borrowed_live_literal_source_reaches_the_actual_function_and_common_executable_seal() {
    building_literals_reach_the_common_executable_seal(LiteralSourceInput::BinaryLive);
}

#[derive(Clone, Copy)]
enum LiteralSourceInput {
    Snapshot,
    BinarySnapshot,
    BinaryLive,
}

fn building_literals_reach_the_common_executable_seal(input: LiteralSourceInput) {
    use crate::entry::schema::value_encoding::{ValueAdmissionOrigin, ValueResourceAdmission};
    use crate::plan::value_admission::PlanValueAuthority;
    use crate::value::AwbcRuntimeValueSnapshot;
    let fixture = || {
        crate::plan::type_table::source_codec::domains::tests::builder(
            crate::entry::RuntimeNominalRecordShape::Record,
            2,
            true,
        )
    };
    let original = fixture()
        .prepare_inventory(crate::plan::RuntimeTaskPlanSealLimits::default(), 0)
        .unwrap();
    let semantic = RuntimeSemanticTypeId::from_bytes([1; 32]);
    let value = crate::program_types::RuntimeProgramTypes::Plan(&original)
        .try_record_value(
            semantic,
            vec![RuntimeValue::Bool(true), RuntimeValue::Bool(false)],
            crate::entry::RuntimeSchemaLimits::engine_default(),
        )
        .unwrap();
    let source = match input {
        LiteralSourceInput::Snapshot | LiteralSourceInput::BinarySnapshot => {
            Some(AwbcRuntimeValueSnapshot::from_admitted_storage(&value, None).unwrap())
        }
        LiteralSourceInput::BinaryLive => None,
    };
    let mut builder = fixture();
    let ty = builder
        .resolve_seed_type("literal source", semantic)
        .unwrap();
    let authority = PlanValueAuthority::Building {
        types: &builder.types,
        records: &builder.nominal_record_domains,
        variants: &builder.variant_domains,
    };
    let mut resources = ValueResourceAdmission::snapshot(
        crate::entry::RuntimeSchemaLimits::engine_default(),
        ValueAdmissionOrigin::InertSnapshot,
    );
    let restored = if matches!(
        input,
        LiteralSourceInput::BinarySnapshot | LiteralSourceInput::BinaryLive
    ) {
        let limits = crate::value::RuntimeLiteralSourceLimits {
            encoded_bytes: 1 << 20,
            decoded_bytes: 1 << 20,
            elements: 10_000,
        };
        let mut meter = crate::task::semantic::TaskSemanticMeter::new(1 << 24, 1 << 24);
        let prepared = match input {
            LiteralSourceInput::BinaryLive => {
                value.prepare_literal_source_encoding(limits, &resources, &mut meter)
            }
            LiteralSourceInput::BinarySnapshot => source
                .as_ref()
                .unwrap()
                .prepare_literal_source_encoding(limits, &resources, &mut meter),
            LiteralSourceInput::Snapshot => unreachable!("inert source uses direct admission"),
        }
        .unwrap();
        let mut bytes = Vec::with_capacity(prepared.encoded_bytes());
        prepared.append_to(&mut bytes).unwrap();
        authority
            .decode_literal_source_value(ty, &bytes, limits, &mut resources, &mut meter)
            .unwrap()
    } else {
        authority
            .materialize_literal_source_value(ty, source.unwrap(), &mut resources)
            .unwrap()
    };
    let mut expected = fixture();
    for (builder, value) in [(&mut builder, restored), (&mut expected, value)] {
        builder
            .push_function_site_seed(
                crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([141; 32]),
                crate::plan::RuntimeFunctionSemanticRole::Ordinary,
                [],
                RuntimeExprSeed::new(semantic, RuntimeExprSeedKind::Value(value)),
            )
            .unwrap();
    }
    assert_eq!(
        builder.finish().unwrap().executable_semantic_digest(),
        expected.finish().unwrap().executable_semantic_digest()
    );
}

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

#[test]
fn dense_values_admit_only_their_exact_container_and_width_in_building_and_sealed_plans() {
    use crate::math::{DenseMatrix, DenseTensor, RuntimeDenseNumericType};
    use crate::pattern::RuntimeCheckedType;

    let values = [
        RuntimeValue::MatrixF32(DenseMatrix::new(1, 1, vec![1.0]).unwrap()),
        RuntimeValue::MatrixF64(DenseMatrix::new(1, 1, vec![1.0]).unwrap()),
        RuntimeValue::TensorF32(DenseTensor::new(vec![1], vec![1.0]).unwrap()),
        RuntimeValue::TensorF64(DenseTensor::new(vec![1], vec![1.0]).unwrap()),
    ];
    let types = RuntimeDenseNumericType::ALL.map(|ty| {
        RuntimePlanTypeSeed::new(
            RuntimeCheckedType::DenseNumeric(ty).semantic_identity_digest(),
            Type::DenseNumeric(ty),
        )
    });
    let mut builder = RuntimePlanBuilder::new();
    builder.admit_type_batch(types.clone(), []).unwrap();
    let sealed = plan(types);
    for (index, dense) in RuntimeDenseNumericType::ALL.into_iter().enumerate() {
        let semantic = RuntimeCheckedType::DenseNumeric(dense).semantic_identity_digest();
        let ty = sealed.type_table().id_for_semantic(semantic).unwrap();
        for (value_index, value) in values.iter().enumerate() {
            assert_eq!(
                builder
                    .lower_expression(RuntimeExprSeed::new(
                        semantic,
                        RuntimeExprSeedKind::Value(value.clone()),
                    ))
                    .is_ok(),
                index == value_index,
                "building {dense:?} with {value:?}"
            );
            assert_eq!(
                sealed
                    .accepts_value(
                        ty,
                        value,
                        crate::entry::RuntimeSchemaLimits::engine_default()
                    )
                    .is_ok(),
                index == value_index,
                "sealed {dense:?} with {value:?}"
            );
        }
    }
}

#[test]
fn nonfinite_execution_values_admit_literals_live_values_and_snapshots_but_not_canonical_inputs() {
    let types = [seed(1, Type::F32), seed(2, Type::F64)];
    let mut builder = RuntimePlanBuilder::new();
    builder.admit_type_batch(types.clone(), []).unwrap();
    let sealed = plan(types);
    for (tag, value) in [
        (1, RuntimeValue::F32(f32::from_bits(0x7fc0_1234))),
        (1, RuntimeValue::F32(f32::INFINITY)),
        (2, RuntimeValue::F64(f64::from_bits(0x7ff8_0000_0000_1234))),
        (2, RuntimeValue::F64(f64::NEG_INFINITY)),
    ] {
        assert!(
            builder
                .lower_expression(RuntimeExprSeed::new(
                    semantic(tag),
                    RuntimeExprSeedKind::Value(value.clone()),
                ))
                .is_ok()
        );
        let ty = sealed.type_table().id_for_semantic(semantic(tag)).unwrap();
        let limits = crate::entry::RuntimeSchemaLimits::engine_default();
        assert!(sealed.validate_live_value(ty, &value, limits).is_ok());
        assert!(sealed.validate_snapshot_value(ty, &value, limits).is_ok());
        assert!(sealed.accepts_value(ty, &value, limits).is_err());
    }
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
    let iterator = RuntimeValue::Iterator(RuntimeIterator::values(vec![RuntimeValue::i16(1)]));
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
    let value = RuntimeValue::NeedHandle(crate::tests::reusable_need_with_outcome(
        "need.input",
        crate::task::TaskOutcomeContract::program(semantic(1)),
    ));
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
    assert_eq!(
        admitted(&sealed, 2, &value).unwrap(),
        value.try_digest(4096).unwrap()
    );
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
        None,
        None,
        false,
        [RuntimeFormatContentOperandSeed::new(
crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(*blake3::hash(b"crates.arcweft-core.src.plan.construction.lower.value_tests.format_content_lowering_keeps_operand_order_and_rejects_unwitnessed_values.formatter.v1\0").as_bytes()).generated_child(crate::plan::RuntimeGeneratedFunctionRole::FormatOperand { parameter: RuntimeFmtParameterId::Value }),
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
        None,
        None,
        false,
        [
            RuntimeFormatContentOperandSeed::new(
                crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(*blake3::hash(b"crates.arcweft-core.src.plan.construction.lower.value_tests.format_content_lowering_keeps_operand_order_and_rejects_unwitnessed_values.formatter.v1\0").as_bytes()).generated_child(crate::plan::RuntimeGeneratedFunctionRole::FormatOperand { parameter: RuntimeFmtParameterId::Style }),
                RuntimeFmtParameterId::Style,
                RuntimeExprSeed::new(
                    string,
                    RuntimeExprSeedKind::Value(RuntimeValue::String("number".to_owned())),
                ),
            ),
            RuntimeFormatContentOperandSeed::new(
                crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(*blake3::hash(b"crates.arcweft-core.src.plan.construction.lower.value_tests.format_content_lowering_keeps_operand_order_and_rejects_unwitnessed_values.formatter.v1\0").as_bytes()).generated_child(crate::plan::RuntimeGeneratedFunctionRole::FormatOperand { parameter: RuntimeFmtParameterId::Value }),
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
        attempt,
        operands,
        project_method,
        project_option,
    } = lowered.kind()
    else {
        panic!("format Content remains a dedicated expression");
    };
    assert_eq!(*actual_template, template);
    assert_eq!(*attempt, None);
    assert_eq!(*project_method, None);
    assert!(!project_option);
    assert_eq!(
        operands
            .iter()
            .map(crate::value::RuntimeFormatContentOperand::parameter)
            .collect::<Vec<_>>(),
        [RuntimeFmtParameterId::Style, RuntimeFmtParameterId::Value]
    );
}

fn sum_expression_seed(result: u8, value: RuntimeValue) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        semantic(result),
        RuntimeExprSeedKind::Sum {
            source: Box::new(RuntimeExprSeed::new(
                semantic(2),
                RuntimeExprSeedKind::BracketSeq(Box::new([RuntimeExprSeed::new(
                    semantic(1),
                    RuntimeExprSeedKind::Value(value),
                )])),
            )),
        },
    )
}

#[test]
fn sum_construction_accepts_all_integer_widths_with_an_i64_result() {
    for (item, value) in [
        (Type::Signed(RuntimeSignedIntWidth::I8), RuntimeValue::i8(7)),
        (
            Type::Signed(RuntimeSignedIntWidth::I16),
            RuntimeValue::i16(7),
        ),
        (
            Type::Signed(RuntimeSignedIntWidth::I32),
            RuntimeValue::i32(7),
        ),
        (
            Type::Signed(RuntimeSignedIntWidth::I64),
            RuntimeValue::i64(7),
        ),
        (
            Type::Signed(RuntimeSignedIntWidth::I128),
            RuntimeValue::i128(7),
        ),
        (
            Type::Signed(RuntimeSignedIntWidth::ISize),
            RuntimeValue::isize(7),
        ),
        (
            Type::Unsigned(RuntimeUnsignedIntWidth::U8),
            RuntimeValue::u8(7),
        ),
        (
            Type::Unsigned(RuntimeUnsignedIntWidth::U16),
            RuntimeValue::u16(7),
        ),
        (
            Type::Unsigned(RuntimeUnsignedIntWidth::U32),
            RuntimeValue::u32(7),
        ),
        (
            Type::Unsigned(RuntimeUnsignedIntWidth::U64),
            RuntimeValue::u64(7),
        ),
        (
            Type::Unsigned(RuntimeUnsignedIntWidth::U128),
            RuntimeValue::u128(7),
        ),
        (
            Type::Unsigned(RuntimeUnsignedIntWidth::USize),
            RuntimeValue::usize(7),
        ),
    ] {
        for sequence in [
            Type::Sequence {
                kind: RuntimePlanSequenceKind::Vec,
                item: semantic(1),
            },
            Type::Array {
                item: semantic(1),
                length: 1.into(),
            },
        ] {
            let mut builder = RuntimePlanBuilder::new();
            builder
                .admit_type_batch(
                    [
                        seed(1, item.clone()),
                        seed(2, sequence),
                        seed(3, Type::Signed(RuntimeSignedIntWidth::I64)),
                    ],
                    [],
                )
                .expect("exact integer and collection types admit");
            let lowered = builder
                .lower_expression(sum_expression_seed(3, value.clone()))
                .expect("each integer width sums into the maintained i64 result");
            assert_eq!(
                builder.projection(lowered.ty()).unwrap(),
                &Type::Signed(RuntimeSignedIntWidth::I64)
            );
            let RuntimeExprKind::Sum { source } = lowered.kind() else {
                panic!("the admitted sum retains its source expression");
            };
            let (source_item, _) = builder
                .root_body_context()
                .sequence_projection(source.ty(), "test sum source")
                .unwrap();
            assert_eq!(
                source_item,
                builder
                    .resolve_seed_type("test original sum element", semantic(1))
                    .unwrap()
            );
        }
    }
}

#[test]
fn sum_construction_rejects_noninteger_elements_at_their_exact_type() {
    for (item, value) in [
        (Type::F32, RuntimeValue::f32(7.0)),
        (Type::F64, RuntimeValue::f64(7.0)),
        (Type::Bool, RuntimeValue::Bool(true)),
        (Type::String, RuntimeValue::String("seven".to_owned())),
    ] {
        for result in [1, 3] {
            let mut builder = RuntimePlanBuilder::new();
            builder
                .admit_type_batch(
                    [
                        seed(1, item.clone()),
                        seed(
                            2,
                            Type::Sequence {
                                kind: RuntimePlanSequenceKind::Vec,
                                item: semantic(1),
                            },
                        ),
                        seed(3, Type::Signed(RuntimeSignedIntWidth::I64)),
                    ],
                    [],
                )
                .unwrap();
            let item_type = builder
                .resolve_seed_type("test sum element", semantic(1))
                .unwrap();
            assert!(matches!(
                builder.lower_expression(sum_expression_seed(result, value.clone())),
                Err(RuntimePlanBuildError::InvalidTypeProjection {
                    context: "sum element", ty
                }) if ty == item_type
            ));
        }
    }
}

#[test]
fn sum_construction_rejects_a_forged_result_matching_the_source_width() {
    for (item, value) in [
        (
            Type::Signed(RuntimeSignedIntWidth::I32),
            RuntimeValue::i32(7),
        ),
        (
            Type::Unsigned(RuntimeUnsignedIntWidth::U64),
            RuntimeValue::u64(7),
        ),
    ] {
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [
                    seed(1, item),
                    seed(
                        2,
                        Type::Sequence {
                            kind: RuntimePlanSequenceKind::Vec,
                            item: semantic(1),
                        },
                    ),
                ],
                [],
            )
            .unwrap();
        let result_type = builder
            .resolve_seed_type("test sum result", semantic(1))
            .unwrap();
        assert!(matches!(
            builder.lower_expression(sum_expression_seed(1, value)),
            Err(RuntimePlanBuildError::InvalidTypeProjection {
                context: "sum result", ty
            }) if ty == result_type
        ));
    }
}

fn collection_intrinsic_call_seed(
    intrinsic: crate::value::RuntimeIntrinsic,
    result: u8,
    arguments: Vec<RuntimeExprSeed>,
) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        semantic(result),
        RuntimeExprSeedKind::Call {
            callee: crate::value::RuntimeCallTarget::intrinsic(intrinsic),
            args: arguments
                .into_iter()
                .enumerate()
                .map(|(position, value)| {
                    RuntimeCallArgumentSeed::new(
                        value,
                        RuntimeCallArgumentMode::Value,
                        u32::try_from(position).unwrap(),
                    )
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        },
    )
}

fn collection_receiver_seed(value: RuntimeValue) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        semantic(2),
        RuntimeExprSeedKind::BracketSeq(Box::new([RuntimeExprSeed::new(
            semantic(1),
            RuntimeExprSeedKind::Value(value),
        )])),
    )
}

#[test]
fn collection_intrinsic_construction_admits_exact_sequence_and_array_results() {
    use crate::value::RuntimeIntrinsic;
    for (item, value) in [
        (Type::Signed(RuntimeSignedIntWidth::I8), RuntimeValue::i8(7)),
        (
            Type::Signed(RuntimeSignedIntWidth::I16),
            RuntimeValue::i16(7),
        ),
        (
            Type::Signed(RuntimeSignedIntWidth::I32),
            RuntimeValue::i32(7),
        ),
        (
            Type::Signed(RuntimeSignedIntWidth::I64),
            RuntimeValue::i64(7),
        ),
        (
            Type::Signed(RuntimeSignedIntWidth::I128),
            RuntimeValue::i128(7),
        ),
        (
            Type::Signed(RuntimeSignedIntWidth::ISize),
            RuntimeValue::isize(7),
        ),
        (
            Type::Unsigned(RuntimeUnsignedIntWidth::U8),
            RuntimeValue::u8(7),
        ),
        (
            Type::Unsigned(RuntimeUnsignedIntWidth::U16),
            RuntimeValue::u16(7),
        ),
        (
            Type::Unsigned(RuntimeUnsignedIntWidth::U32),
            RuntimeValue::u32(7),
        ),
        (
            Type::Unsigned(RuntimeUnsignedIntWidth::U64),
            RuntimeValue::u64(7),
        ),
        (
            Type::Unsigned(RuntimeUnsignedIntWidth::U128),
            RuntimeValue::u128(7),
        ),
        (
            Type::Unsigned(RuntimeUnsignedIntWidth::USize),
            RuntimeValue::usize(7),
        ),
    ] {
        for collection in [
            Type::Sequence {
                kind: RuntimePlanSequenceKind::Vec,
                item: semantic(1),
            },
            Type::Sequence {
                kind: RuntimePlanSequenceKind::Seq,
                item: semantic(1),
            },
            Type::Sequence {
                kind: RuntimePlanSequenceKind::Slice,
                item: semantic(1),
            },
            Type::Array {
                item: semantic(1),
                length: 1.into(),
            },
        ] {
            let mut builder = RuntimePlanBuilder::new();
            builder
                .admit_type_batch(
                    [
                        seed(1, item.clone()),
                        seed(2, collection),
                        seed(3, Type::Signed(RuntimeSignedIntWidth::I64)),
                        seed(4, Type::Unsigned(RuntimeUnsignedIntWidth::USize)),
                    ],
                    [],
                )
                .unwrap();
            for (intrinsic, result, expected) in [
                (
                    RuntimeIntrinsic::CoreSeqSum,
                    3,
                    Type::Signed(RuntimeSignedIntWidth::I64),
                ),
                (
                    RuntimeIntrinsic::CoreSeqLen,
                    4,
                    Type::Unsigned(RuntimeUnsignedIntWidth::USize),
                ),
            ] {
                let lowered = builder
                    .lower_expression(collection_intrinsic_call_seed(
                        intrinsic,
                        result,
                        vec![collection_receiver_seed(value.clone())],
                    ))
                    .unwrap();
                assert_eq!(builder.projection(lowered.ty()).unwrap(), &expected);
                let RuntimeExprKind::Call { callee, args } = lowered.kind() else {
                    panic!("the selected intrinsic remains a typed call");
                };
                assert_eq!(callee.as_intrinsic(), Some(intrinsic));
                assert_eq!(args.len(), 1);
            }
        }
    }
}

#[test]
fn collection_intrinsic_construction_rejects_wrong_receiver_and_result_types() {
    use crate::value::RuntimeIntrinsic;
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                seed(1, Type::Signed(RuntimeSignedIntWidth::I32)),
                seed(
                    2,
                    Type::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item: semantic(1),
                    },
                ),
                seed(3, Type::Signed(RuntimeSignedIntWidth::I64)),
                seed(4, Type::Unsigned(RuntimeUnsignedIntWidth::USize)),
            ],
            [],
        )
        .unwrap();
    let scalar = builder
        .resolve_seed_type("test scalar receiver", semantic(1))
        .unwrap();
    assert!(
        matches!(builder.lower_expression(collection_intrinsic_call_seed(
        RuntimeIntrinsic::CoreSeqLen,4,vec![RuntimeExprSeed::new(semantic(1),RuntimeExprSeedKind::Value(RuntimeValue::i32(7)))],
    )),Err(RuntimePlanBuildError::InvalidTypeProjection {context:"collection intrinsic receiver",ty}) if ty==scalar)
    );
    for (intrinsic, result, context) in [
        (RuntimeIntrinsic::CoreSeqSum, 1, "collection sum result"),
        (RuntimeIntrinsic::CoreSeqLen, 3, "collection length result"),
    ] {
        let actual = builder
            .resolve_seed_type("test forged result", semantic(result))
            .unwrap();
        assert!(
            matches!(builder.lower_expression(collection_intrinsic_call_seed(
            intrinsic,result,vec![collection_receiver_seed(RuntimeValue::i32(7))],
        )),Err(RuntimePlanBuildError::InvalidTypeProjection {context:actual_context,ty}) if actual_context==context && ty==actual)
        );
    }
}

#[test]
fn collection_intrinsic_construction_rejects_noninteger_sum_and_wrong_arity() {
    use crate::value::RuntimeIntrinsic;
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                seed(1, Type::Bool),
                seed(
                    2,
                    Type::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item: semantic(1),
                    },
                ),
                seed(3, Type::Signed(RuntimeSignedIntWidth::I64)),
                seed(4, Type::Unsigned(RuntimeUnsignedIntWidth::USize)),
            ],
            [],
        )
        .unwrap();
    let item = builder
        .resolve_seed_type("test bool element", semantic(1))
        .unwrap();
    assert!(
        matches!(builder.lower_expression(collection_intrinsic_call_seed(
        RuntimeIntrinsic::CoreSeqSum,3,vec![collection_receiver_seed(RuntimeValue::Bool(true))],
    )),Err(RuntimePlanBuildError::InvalidTypeProjection {context:"collection sum element",ty}) if ty==item)
    );
    for count in [0, 2] {
        assert!(
            matches!(builder.lower_expression(collection_intrinsic_call_seed(
            RuntimeIntrinsic::CoreSeqLen,4,(0..count).map(|_|collection_receiver_seed(RuntimeValue::Bool(true))).collect(),
        )),Err(RuntimePlanBuildError::CallableAbiArity {context:"collection intrinsic",expected:1,actual}) if actual==count)
        );
    }
}
