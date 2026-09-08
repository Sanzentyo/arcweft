use std::{collections::BTreeSet, sync::Arc};

use serde::Deserialize;

use super::{
    Angle, FX_GOLDEN_ANGLE_RAD, FiniteF32, FxApplication, FxApplicationDraft, FxAuthoredSeed,
    FxCapabilitySet, FxDefinition, FxDefinitionArgumentValue, FxDefinitionError,
    FxDefinitionParameter, FxDefinitionParameterType, FxDiagnosticCode, FxDiagnosticContext,
    FxEvaluationBinding, FxEvaluationBudget, FxEvaluationError, FxFontFamilyName, FxGraph,
    FxGraphChildPath, FxGraphEvaluator, FxId, FxInstanceActivation, FxInstanceId,
    FxInstanceOwnerKey, FxInstanceSnapshot, FxLogicalTime, FxNode, FxNodeKind,
    FxParameterStorageSlot, FxPhase, FxProperty, FxPropertyId, FxProviderError, FxProviderLimits,
    FxProviderOutput, FxProviderStateRecord, FxResourceId, FxRuntimeOperationOpcode, FxRuntimeType,
    FxRuntimeValue, FxSampleContext, FxSamplerProgram, FxSelectorDomain, FxSelectorId,
    FxSemanticHash, FxShaderUniform, FxStaticDefinitionArgumentValue, FxStaticType, FxStaticValue,
    FxTarget, FxUniformRecord, Length, ResolvedColorOperation, ResolvedFilterOperation,
    ResolvedFxOperation, ResolvedFxPlan, ResolvedMaskOperation, ResolvedOffscreenPassOperation,
    ResolvedPostProcessOperation, ResolvedShaderUniformOperation, ResolvedTextStyleOperation,
    ResolvedTransform2D, ResolvedTransformOperation, ResolvedTransitionOperation, Seconds,
    Transform2D, ValueInstruction, ValueProgramInputs, ValueProgramSchema,
    ValueProgramValidationError, derive_deterministic_seed,
};

fn runtime_parameter(
    index: usize,
    name: &str,
    ty: FxRuntimeType,
    default: Option<FxRuntimeValue>,
) -> FxDefinitionParameter {
    FxDefinitionParameter::try_new(
        index,
        name,
        FxDefinitionParameterType::Runtime(ty),
        default.map(FxDefinitionArgumentValue::Runtime),
    )
    .unwrap()
}

fn bind_empty(definition: &FxDefinition, ordinal: u32) -> FxApplication {
    let draft = FxApplicationDraft::try_new(
        definition.id().clone(),
        vec![None; definition.parameters().len()],
        ordinal,
        None,
    )
    .unwrap();
    FxApplication::bind(definition, draft).unwrap()
}

#[test]
fn source_constructor_inventory_is_closed_and_independent_from_runtime_nodes() {
    use super::FxSourceConstructor as Source;
    assert_eq!(Source::ALL.len(), 9);
    for (tag, constructor) in Source::ALL.into_iter().enumerate() {
        assert_eq!(usize::from(constructor.semantic_tag()), tag);
        assert_eq!(
            Source::from_source_name(constructor.source_name()),
            Some(constructor)
        );
    }
    assert_eq!(Source::from_source_name("offscreen_pass"), None);
    assert_eq!(Source::from_source_name("post_process"), None);
    assert_eq!(
        Source::Transform.property_from_source_name("sample"),
        Some(FxPropertyId::Sampler)
    );
    assert_eq!(FxPropertyId::from_source_name("sample"), None);
    assert_eq!(Source::Transition.node_kind(), FxNodeKind::Transition);
    assert_eq!(FxNodeKind::ALL.len(), 12);
    assert_eq!(
        FxNodeKind::ALL
            .iter()
            .map(|kind| kind.semantic_tag())
            .collect::<BTreeSet<_>>()
            .len(),
        12
    );
    assert_eq!(FxNodeKind::OffscreenPass.source_constructor(), None);
    assert_eq!(FxNodeKind::PostProcess.source_constructor(), None);
    assert_eq!(FxNodeKind::Shader.source_constructor(), None);
    assert!(
        FxNodeKind::ALL
            .into_iter()
            .filter_map(FxNodeKind::source_constructor)
            .eq(Source::ALL)
    );
}

#[test]
fn selector_ids_are_domain_typed_and_canonical() {
    let kind = FxSelectorId::try_new(FxSelectorDomain::TransitionKind, "dissolve")
        .expect("canonical selector");
    let easing = FxSelectorId::try_new(FxSelectorDomain::TransitionEasing, "dissolve")
        .expect("same name in another domain");
    assert_ne!(kind, easing);
    assert_eq!(kind.name().as_str(), "dissolve");
    assert!(FxSelectorId::try_new(FxSelectorDomain::TransitionKind, "not-canonical").is_err());
    assert_eq!(
        super::FxShaderStage::from_source_name("offscreen_pass"),
        Some(super::FxShaderStage::OffscreenPass)
    );
}

#[test]
fn typed_property_identity_drives_semantic_and_abi_hashes() {
    let size = FxGraph::try_new(vec![FxNode::Text {
        properties: vec![FxProperty::new(
            FxPropertyId::Size,
            FxRuntimeValue::Length(length(1.0)).into(),
        )],
    }])
    .expect("size graph");
    let spacing = FxGraph::try_new(vec![FxNode::Text {
        properties: vec![FxProperty::new(
            FxPropertyId::Spacing,
            FxRuntimeValue::Length(length(1.0)).into(),
        )],
    }])
    .expect("spacing graph");
    assert_ne!(
        FxSemanticHash::for_graph(&size),
        FxSemanticHash::for_graph(&spacing)
    );
    let size = FxDefinition::new(FxId::try_new("test", "size").unwrap(), vec![], size).unwrap();
    let spacing =
        FxDefinition::new(FxId::try_new("test", "spacing").unwrap(), vec![], spacing).unwrap();
    assert_ne!(size.abi_hash(), spacing.abi_hash());
}

#[test]
fn repeated_property_requirements_do_not_change_the_definition_abi() {
    let property = |value| {
        FxProperty::new(
            FxPropertyId::Size,
            FxRuntimeValue::Length(length(value)).into(),
        )
    };
    let single = FxDefinition::new(
        FxId::try_new("game", "single_size").unwrap(),
        Vec::new(),
        FxGraph::try_new(vec![FxNode::Text {
            properties: vec![property(1.0)],
        }])
        .unwrap(),
    )
    .unwrap();
    let repeated = FxDefinition::new(
        FxId::try_new("game", "repeated_size").unwrap(),
        Vec::new(),
        FxGraph::try_new(vec![
            FxNode::Text {
                properties: vec![property(1.0)],
            },
            FxNode::Text {
                properties: vec![property(2.0)],
            },
        ])
        .unwrap(),
    )
    .unwrap();

    assert_eq!(single.abi_hash(), repeated.abi_hash());
    assert_ne!(single.semantic_hash(), repeated.semantic_hash());
}

#[test]
fn source_parameter_schemas_are_closed_and_ordered() {
    use super::FxSourceConstructor as Source;
    for constructor in Source::ALL {
        let schema = constructor.parameter_schema();
        let names = schema
            .iter()
            .map(|row| row.source_name())
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), schema.len());
    }
    assert_eq!(Source::from_source_name("shader"), None);
    let transform = Source::Transform.parameter_schema();
    assert!(transform.iter().any(|row| row.source_name() == "sample"));
    assert!(!transform.iter().any(|row| row.source_name() == "sampler"));
    assert_eq!(
        Source::Conditional
            .parameter_schema()
            .iter()
            .map(|row| row.source_name())
            .collect::<Vec<_>>(),
        ["condition", "then", "else"]
    );
    assert_eq!(Source::Stack.parameter_schema()[0].source_name(), "graphs");
}

#[test]
fn property_ids_are_unique_and_open_record_keys_remain_distinct() {
    let names = FxPropertyId::ALL
        .iter()
        .map(|property| property.source_name())
        .collect::<BTreeSet<_>>();
    let tags = FxPropertyId::ALL
        .iter()
        .map(|property| property.semantic_tag())
        .collect::<BTreeSet<_>>();
    assert_eq!(names.len(), 26);
    assert_eq!(tags.len(), 26);
    let value = FxStaticValue::UniformRecord(
        super::FxUniformRecord::try_new(vec![
            super::FxUniformField::try_new(
                "application_uniform",
                super::FxUniformValue::constant(FxRuntimeValue::F32(finite(0.5))).unwrap(),
            )
            .unwrap(),
        ])
        .unwrap(),
    );
    let bytes = serde_json::to_vec(&value).expect("record serializes");
    assert!(
        String::from_utf8(bytes.clone())
            .expect("JSON")
            .contains("application_uniform")
    );
    assert_eq!(
        serde_json::from_slice::<FxStaticValue>(&bytes).expect("record decodes"),
        value
    );
}

// Keep local constructors terse so tests emphasize the typed contracts.
fn finite(value: f32) -> FiniteF32 {
    FiniteF32::try_new(value).expect("finite fixture")
}

fn length(value: f32) -> Length {
    Length::try_pixels(value).expect("finite length fixture")
}

fn angle(value: f32) -> Angle {
    Angle::try_radians(value).expect("finite angle fixture")
}

fn seconds(value: f32) -> Seconds {
    Seconds::try_seconds(value).expect("finite seconds fixture")
}

fn owner(bytes: &[u8]) -> FxInstanceOwnerKey {
    FxInstanceOwnerKey::from_dialogue_canonical_bytes(bytes)
}

fn runtime_operation_fixtures() -> ([ResolvedFxOperation; 9], [FxRuntimeOperationOpcode; 9]) {
    let transform = Transform2D::default()
        .resolve()
        .expect("default transform resolves");
    let resource = super::FxResourceId::try_new("shader.test").expect("resource");
    let selector =
        FxSelectorId::try_new(FxSelectorDomain::TransitionKind, "dissolve").expect("selector");
    let operations = [
        ResolvedFxOperation::TextStyle(ResolvedTextStyleOperation {
            phase: FxPhase::BeforeLayout,
            target: FxTarget::Content,
            opacity: Some(finite(0.75)),
            weight: Some(700),
            slant: None,
            font_family: Some(FxFontFamilyName::try_new("sans").expect("valid font family")),
            size: Some(length(16.0)),
            spacing: None,
            color: None,
        }),
        ResolvedFxOperation::Color(ResolvedColorOperation {
            phase: FxPhase::GlyphColor,
            target: FxTarget::Glyph,
            tint: Some(super::FxColor::WHITE),
            multiply: None,
            opacity: Some(finite(0.5)),
        }),
        ResolvedFxOperation::Transform(ResolvedTransformOperation::new(
            FxPhase::GlyphTransform,
            FxTarget::Glyph,
            transform,
            false,
        )),
        ResolvedFxOperation::Mask(ResolvedMaskOperation {
            phase: FxPhase::GlyphMask,
            target: FxTarget::Glyph,
            resource: None,
            coverage: Some(finite(0.5)),
            invert: Some(false),
        }),
        ResolvedFxOperation::Filter(ResolvedFilterOperation {
            phase: FxPhase::OffscreenPass,
            target: FxTarget::Content,
            blur_radius: Some(length(2.0)),
            brightness: None,
            contrast: None,
            saturation: None,
        }),
        ResolvedFxOperation::ShaderUniform(ResolvedShaderUniformOperation {
            phase: FxPhase::GlyphColor,
            target: FxTarget::Glyph,
            resource: Some(resource.clone()),
            stage: Some(FxPhase::GlyphColor),
            uniforms: vec![FxShaderUniform::runtime(
                super::FxUniformName::try_new("amount").unwrap(),
                FxRuntimeValue::F32(finite(0.5)),
            )],
        }),
        ResolvedFxOperation::OffscreenPass(ResolvedOffscreenPassOperation {
            phase: FxPhase::OffscreenPass,
            target: FxTarget::Content,
            resource: Some(resource.clone()),
        }),
        ResolvedFxOperation::PostProcess(ResolvedPostProcessOperation {
            phase: FxPhase::PostProcess,
            target: FxTarget::Viewport,
            resource: Some(resource),
        }),
        ResolvedFxOperation::Transition(ResolvedTransitionOperation {
            phase: FxPhase::Transition,
            target: FxTarget::Viewport,
            kind: Some(selector),
            easing: None,
            duration: Some(seconds(0.5)),
            progress: Some(finite(0.25)),
        }),
    ];
    let expected = [
        FxRuntimeOperationOpcode::TextStyle,
        FxRuntimeOperationOpcode::Color,
        FxRuntimeOperationOpcode::Transform,
        FxRuntimeOperationOpcode::Mask,
        FxRuntimeOperationOpcode::Filter,
        FxRuntimeOperationOpcode::ShaderUniform,
        FxRuntimeOperationOpcode::OffscreenPass,
        FxRuntimeOperationOpcode::PostProcess,
        FxRuntimeOperationOpcode::Transition,
    ];
    (operations, expected)
}

#[test]
fn runtime_operation_opcodes_are_fixed_and_all_payloads_round_trip() {
    let (operations, expected) = runtime_operation_fixtures();
    assert_eq!(FxRuntimeOperationOpcode::ALL, expected);
    let tags = expected
        .iter()
        .map(|opcode| opcode.encoded())
        .collect::<BTreeSet<_>>();
    assert_eq!(tags.len(), 9);
    assert!(FxRuntimeOperationOpcode::from_encoded(9).is_none());
    for (operation, opcode) in operations.iter().zip(expected) {
        assert_eq!(operation.opcode(), opcode);
        assert_eq!(
            opcode.encoded(),
            u8::try_from(expected.iter().position(|item| *item == opcode).unwrap())
                .expect("the fixed opcode table fits in u8")
        );
        let encoded = serde_json::to_vec(operation).expect("operation serializes");
        let decoded: ResolvedFxOperation =
            serde_json::from_slice(&encoded).expect("operation deserializes");
        assert_eq!(&decoded, operation);
    }
}

fn heterogeneous_definition() -> (FxDefinition, FxResourceId, FxUniformRecord) {
    let resource = FxResourceId::try_new("shader.wave").expect("resource identity");
    let uniforms = FxUniformRecord::try_new(Vec::new()).expect("uniform record");
    let definition = FxDefinition::new(
        FxId::try_new("game", "ui.effects.wave").expect("Fx ID"),
        vec![
            FxDefinitionParameter::try_new(
                0,
                "resource",
                FxDefinitionParameterType::Resource,
                Some(FxDefinitionArgumentValue::Resource(resource.clone())),
            )
            .expect("resource parameter"),
            runtime_parameter(1, "amplitude", FxRuntimeType::F32, None),
            FxDefinitionParameter::try_new(
                2,
                "uniforms",
                FxDefinitionParameterType::UniformRecord,
                Some(FxDefinitionArgumentValue::UniformRecord(uniforms.clone())),
            )
            .expect("uniform parameter"),
            runtime_parameter(3, "seed", FxRuntimeType::U32, None),
        ],
        FxGraph::default(),
    )
    .expect("heterogeneous definition");
    (definition, resource, uniforms)
}

fn assert_heterogeneous_layout(definition: &FxDefinition) {
    let layout = definition.parameter_layout();
    assert_eq!(layout.abi_rows().len(), 4);
    assert!(matches!(
        layout.abi_rows()[0].storage(),
        FxParameterStorageSlot::Static(slot) if slot.get() == 0
    ));
    assert!(matches!(
        layout.abi_rows()[1].storage(),
        FxParameterStorageSlot::Runtime(slot) if slot.get() == 0
    ));
    assert!(matches!(
        layout.abi_rows()[2].storage(),
        FxParameterStorageSlot::Static(slot) if slot.get() == 1
    ));
    assert!(matches!(
        layout.abi_rows()[3].storage(),
        FxParameterStorageSlot::Runtime(slot) if slot.get() == 1
    ));
    assert_eq!(
        layout
            .runtime_rows()
            .iter()
            .map(|row| (row.parameter().index().get(), row.reference().slot().get()))
            .collect::<Vec<_>>(),
        vec![(1, 0), (3, 1)]
    );
    assert_eq!(
        layout
            .static_rows()
            .iter()
            .map(|row| (row.parameter().index().get(), row.slot().get()))
            .collect::<Vec<_>>(),
        vec![(0, 0), (2, 1)]
    );
}

fn bind_heterogeneous_application(definition: &FxDefinition) -> FxApplication {
    FxApplication::bind(
        definition,
        FxApplicationDraft::try_new(
            definition.id().clone(),
            vec![
                None,
                Some(FxDefinitionArgumentValue::Runtime(FxRuntimeValue::F32(
                    finite(0.75),
                ))),
                None,
                Some(FxDefinitionArgumentValue::Runtime(FxRuntimeValue::U32(
                    u32::MAX,
                ))),
            ],
            4,
            None,
        )
        .expect("application draft"),
    )
    .expect("exact binding")
}

fn assert_heterogeneous_application(
    definition: &FxDefinition,
    application: &FxApplication,
    resource: FxResourceId,
    uniforms: FxUniformRecord,
) {
    assert_eq!(
        application.template().initial_runtime(),
        [
            FxRuntimeValue::F32(finite(0.75)),
            FxRuntimeValue::U32(u32::MAX)
        ]
    );
    assert_eq!(
        application.template().static_arguments(),
        [
            FxStaticDefinitionArgumentValue::Resource(resource),
            FxStaticDefinitionArgumentValue::UniformRecord(uniforms)
        ]
    );
    application
        .validate_for_definition(definition)
        .expect("application validates against its definition");
    let encoded = serde_json::to_vec(application).expect("application encodes");
    let decoded = serde_json::from_slice::<FxApplication>(&encoded).expect("application decodes");
    assert_eq!(&decoded, application);
    decoded
        .validate_for_definition(definition)
        .expect("decoded application rejoins its definition");
}

fn assert_heterogeneous_snapshot(definition: &FxDefinition, application: &FxApplication) {
    let snapshot = FxInstanceSnapshot::try_new(
        application.instance_identity(owner(b"mixed-layout")),
        definition,
        FxInstanceActivation::new(
            FxLogicalTime::zero(),
            Some(FxAuthoredSeed::new(7)),
            FxGraphChildPath::default(),
        ),
        application.template().clone(),
        application
            .template()
            .initial_runtime()
            .to_vec()
            .into_boxed_slice(),
        Vec::new(),
    )
    .expect("snapshot binds exact definition");
    assert!(Arc::ptr_eq(application.template(), snapshot.template()));
    snapshot
        .validate_for_definition(definition)
        .expect("snapshot validates against its definition");

    let foreign_definition =
        FxDefinition::new(definition.id().clone(), Vec::new(), FxGraph::default())
            .expect("foreign layout");
    let foreign_application = bind_empty(&foreign_definition, 4);
    assert!(matches!(
        FxInstanceSnapshot::try_new(
            snapshot.identity().clone(),
            definition,
            FxInstanceActivation::new(
                snapshot.activation_logical_time(),
                snapshot.authored_seed(),
                snapshot.child_path().clone(),
            ),
            foreign_application.template().clone(),
            snapshot.parameters().to_vec().into_boxed_slice(),
            snapshot.provider_state().to_vec(),
        ),
        Err(super::FxInstanceSnapshotError::LayoutMismatch { .. })
    ));
}

#[test]
fn runtime_operation_opcode_rejects_unknown_and_tampered_wire_values() {
    assert!(serde_json::from_str::<FxRuntimeOperationOpcode>("255").is_err());
    assert!(serde_json::from_str::<ResolvedFxOperation>("[1,255,{}]").is_err());
    assert!(serde_json::from_str::<ResolvedFxOperation>("[2,0,{}]").is_err());
    assert!(serde_json::from_str::<ResolvedFxOperation>("[1,0,{} , 1]").is_err());
    assert!(
        serde_json::from_value::<ResolvedFxOperation>(serde_json::json!([
            1,
            1,
            {"phase": "before_layout", "target": "content", "weight": 700}
        ]))
        .is_err()
    );
}

#[test]
fn instance_identity_separates_owner_domains_and_authored_occurrences() {
    let definition = FxId::try_new("game", "ui.effects.wave").expect("Fx id");
    let dialogue_owner = FxInstanceOwnerKey::from_dialogue_canonical_bytes(b"same-fields");
    let view_owner = FxInstanceOwnerKey::from_view_canonical_bytes(b"same-fields");

    assert_ne!(
        FxInstanceId::derive(&definition, dialogue_owner, 0),
        FxInstanceId::derive(&definition, view_owner, 0)
    );
    assert_ne!(
        FxInstanceId::derive(&definition, dialogue_owner, 0),
        FxInstanceId::derive(&definition, dialogue_owner, 1)
    );
}

fn sampler(return_type: FxRuntimeType, instructions: Vec<ValueInstruction>) -> FxSamplerProgram {
    FxSamplerProgram::validate(
        ValueProgramSchema::new(Vec::new(), Vec::new(), return_type),
        instructions,
    )
    .expect("valid sampler fixture")
}

fn context(time: f32, ordinal: u32) -> FxSampleContext {
    FxSampleContext::from_elapsed(seconds(time), ordinal, 0x55aa, false)
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 0.000_01,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn finite_values_reject_nonfinite_and_canonicalize_negative_zero() {
    let negative_zero = FiniteF32::try_new(-0.0).expect("negative zero is finite");
    assert_eq!(negative_zero, FiniteF32::ZERO);
    assert_eq!(negative_zero.to_bits(), 0);
    assert!(FiniteF32::try_new(f32::INFINITY).is_err());
    assert!(FiniteF32::try_new(f32::NEG_INFINITY).is_err());
    assert!(FiniteF32::try_new(f32::NAN).is_err());

    let deserializer = serde::de::value::F32Deserializer::<serde::de::value::Error>::new(f32::NAN);
    assert!(FiniteF32::deserialize(deserializer).is_err());
    assert!(serde_json::from_str::<FiniteF32>("1e100").is_err());
    assert!(serde_json::from_str::<FiniteF32>("1e-100").is_err());
}

#[test]
fn units_convert_once_to_canonical_runtime_values() {
    assert_close(
        Angle::try_degrees(180.0).expect("degrees").radians(),
        std::f32::consts::PI,
    );
    assert_close(
        Angle::try_turns(0.5).expect("turns").radians(),
        std::f32::consts::PI,
    );
    assert_close(
        Seconds::try_milliseconds(125.0)
            .expect("milliseconds")
            .seconds(),
        0.125,
    );
}

#[test]
fn u32_runtime_tags_are_appended_without_changing_existing_tags() {
    let tags = [
        FxRuntimeType::Bool,
        FxRuntimeType::I32,
        FxRuntimeType::F32,
        FxRuntimeType::Length,
        FxRuntimeType::Angle,
        FxRuntimeType::Seconds,
        FxRuntimeType::Color,
        FxRuntimeType::Vec2,
        FxRuntimeType::Transform2D,
        FxRuntimeType::U32,
    ]
    .map(|value| value as u8);
    assert_eq!(tags, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);

    for value in [0, u32::MAX] {
        let value = FxRuntimeValue::U32(value);
        let encoded = serde_json::to_vec(&value).expect("U32 serializes");
        assert_eq!(
            serde_json::from_slice::<FxRuntimeValue>(&encoded).unwrap(),
            value
        );
        assert_eq!(value.value_type(), FxRuntimeType::U32);
    }

    let id = FxId::try_new("game", "u32_effect").expect("Fx ID");
    let definition = FxDefinition::new(
        id.clone(),
        vec![runtime_parameter(
            0,
            "seed",
            FxRuntimeType::U32,
            Some(FxRuntimeValue::U32(0)),
        )],
        FxGraph::default(),
    )
    .expect("U32 definition");
    let draft = FxApplicationDraft::try_new(
        id,
        vec![Some(FxDefinitionArgumentValue::Runtime(
            FxRuntimeValue::U32(u32::MAX),
        ))],
        0,
        None,
    )
    .unwrap();
    let application = FxApplication::bind(&definition, draft).expect("U32 application");
    let encoded = serde_json::to_vec(&application).expect("U32 application serializes");
    assert_eq!(
        serde_json::from_slice::<FxApplication>(&encoded).unwrap(),
        application
    );

    let encoded = serde_json::to_vec(&definition).expect("U32 definition serializes");
    assert_eq!(
        serde_json::from_slice::<FxDefinition>(&encoded).unwrap(),
        definition
    );
}

#[test]
fn u32_sampler_constants_and_bitcast_use_new_canonical_tags() {
    let mut expected_prefix = b"arcweft.fx-sampler-program".to_vec();
    expected_prefix.extend_from_slice(&[0, 1, 0, 0, 1, 3]);

    for (raw, expected, canonical) in [
        (0_u32, 0_i32, &[0][..]),
        (127_u32, 127_i32, &[0x7f][..]),
        (128_u32, 128_i32, &[0x80, 0x01][..]),
        (
            i32::MAX as u32,
            i32::MAX,
            &[0xff, 0xff, 0xff, 0xff, 0x07][..],
        ),
        (
            0x8000_0000_u32,
            i32::MIN,
            &[0x80, 0x80, 0x80, 0x80, 0x08][..],
        ),
        (u32::MAX, -1_i32, &[0xff, 0xff, 0xff, 0xff, 0x0f][..]),
    ] {
        let program = sampler(
            FxRuntimeType::I32,
            vec![
                ValueInstruction::Constant {
                    value: FxRuntimeValue::U32(raw),
                },
                ValueInstruction::BitcastU32ToI32,
                ValueInstruction::Return,
            ],
        );
        let value = program
            .evaluate(
                ValueProgramInputs {
                    parameters: &[],
                    state: &[],
                },
                context(0.0, 0),
                &mut FxEvaluationBudget::default(),
            )
            .expect("bitcast evaluates");
        assert_eq!(value, FxRuntimeValue::I32(expected));

        let mut expected_bytes = expected_prefix.clone();
        expected_bytes.extend_from_slice(&[0, 9]);
        expected_bytes.extend_from_slice(canonical);
        expected_bytes.extend_from_slice(&[32, 29]);
        assert_eq!(program.canonical_v1_bytes().unwrap(), expected_bytes);
        assert_eq!(program.canonical_v1_len().unwrap(), expected_bytes.len());
        assert_eq!(
            program.canonical_v1_len_u64(),
            u64::try_from(expected_bytes.len()).unwrap()
        );

        let encoded = serde_json::to_vec(&program).expect("bitcast sampler serializes");
        assert_eq!(
            serde_json::from_slice::<FxSamplerProgram>(&encoded).unwrap(),
            program
        );
    }
}

#[test]
fn u32_equality_is_typed_but_arithmetic_and_invalid_bitcasts_are_rejected() {
    let equality = sampler(
        FxRuntimeType::Bool,
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::U32(7),
            },
            ValueInstruction::Constant {
                value: FxRuntimeValue::U32(7),
            },
            ValueInstruction::Equal,
            ValueInstruction::Return,
        ],
    );
    assert_eq!(
        equality
            .evaluate(
                ValueProgramInputs {
                    parameters: &[],
                    state: &[],
                },
                context(0.0, 0),
                &mut FxEvaluationBudget::default(),
            )
            .unwrap(),
        FxRuntimeValue::Bool(true)
    );

    let arithmetic = FxSamplerProgram::validate(
        ValueProgramSchema::new(Vec::new(), Vec::new(), FxRuntimeType::U32),
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::U32(1),
            },
            ValueInstruction::Constant {
                value: FxRuntimeValue::U32(2),
            },
            ValueInstruction::Add,
            ValueInstruction::Return,
        ],
    )
    .expect_err("U32 arithmetic is not part of the Fx numeric algebra");
    assert!(matches!(
        arithmetic,
        ValueProgramValidationError::InvalidOperands {
            operation: "add",
            ..
        }
    ));

    let invalid_bitcast = FxSamplerProgram::validate(
        ValueProgramSchema::new(Vec::new(), Vec::new(), FxRuntimeType::I32),
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::I32(1),
            },
            ValueInstruction::BitcastU32ToI32,
            ValueInstruction::Return,
        ],
    )
    .expect_err("bitcast rejects non-U32 operands");
    assert!(matches!(
        invalid_bitcast,
        ValueProgramValidationError::InvalidOperands {
            operation: "bitcast_u32_to_i32",
            ..
        }
    ));
}

#[test]
fn transform_uses_documented_origin_scale_rotation_translation_order() {
    let transform = Transform2D {
        translate_x: length(5.0),
        translate_y: length(7.0),
        scale_x: finite(2.0),
        scale_y: finite(3.0),
        skew_x: Angle::ZERO,
        skew_y: Angle::ZERO,
        rotation: angle(std::f32::consts::FRAC_PI_2),
        origin_x: length(1.0),
        origin_y: length(1.0),
        opacity: FiniteF32::ONE,
    };
    let point = transform
        .resolve()
        .expect("transform resolves")
        .apply_point(length(2.0), length(1.0))
        .expect("point remains finite");
    assert_close(point[0].pixels(), 6.0);
    assert_close(point[1].pixels(), 10.0);
}

#[test]
fn authored_transform_stack_applies_each_next_transform_after_previous() {
    let translate = Transform2D {
        translate_x: length(10.0),
        ..Transform2D::default()
    };
    let scale = Transform2D {
        scale_x: finite(2.0),
        scale_y: finite(2.0),
        ..Transform2D::default()
    };
    let resolved = ResolvedTransform2D::compose_authored([translate, scale])
        .expect("authored transforms compose");
    let point = resolved
        .apply_point(length(1.0), Length::ZERO)
        .expect("point remains finite");
    assert_close(point[0].pixels(), 22.0);
}

#[test]
fn transform_deserialization_rejects_invalid_opacity() {
    let json = r#"{
        "translate_x": 0.0, "translate_y": 0.0,
        "scale_x": 1.0, "scale_y": 1.0,
        "skew_x": 0.0, "skew_y": 0.0, "rotation": 0.0,
        "origin_x": 0.0, "origin_y": 0.0, "opacity": 1.01
    }"#;
    assert!(serde_json::from_str::<Transform2D>(json).is_err());
}

#[test]
fn program_validation_rejects_mixed_units_before_execution() {
    let error = FxSamplerProgram::validate(
        ValueProgramSchema::new(Vec::new(), Vec::new(), FxRuntimeType::Length),
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::Length(length(1.0)),
            },
            ValueInstruction::Constant {
                value: FxRuntimeValue::Angle(angle(1.0)),
            },
            ValueInstruction::Add,
            ValueInstruction::Return,
        ],
    )
    .expect_err("mixed units are invalid");
    assert!(matches!(
        error,
        ValueProgramValidationError::InvalidOperands {
            operation: "add",
            ..
        }
    ));
}

#[test]
fn sampler_deserialization_revalidates_stack_and_return_contract() {
    let json = r#"{
        "program": {
            "schema": {"parameter_types": [], "state_types": [], "return_type": "f32"},
            "instructions": [{"op": "return"}]
        }
    }"#;
    assert!(serde_json::from_str::<FxSamplerProgram>(json).is_err());
}

#[test]
fn validated_sampler_round_trips_through_serde() {
    let program = sampler(
        FxRuntimeType::F32,
        vec![
            ValueInstruction::LoadContext {
                slot: super::FxContextSlot::Time,
            },
            ValueInstruction::Return,
        ],
    );
    let bytes = serde_json::to_vec(&program).expect("sampler serializes");
    assert_eq!(
        serde_json::from_slice::<FxSamplerProgram>(&bytes).expect("sampler revalidates"),
        program
    );
}

#[test]
fn sampler_canonical_length_matches_checked_v1_encoding() {
    let program = sampler(
        FxRuntimeType::F32,
        vec![
            ValueInstruction::LoadContext {
                slot: super::FxContextSlot::Time,
            },
            ValueInstruction::Abs,
            ValueInstruction::Return,
        ],
    );
    let measured = program
        .canonical_v1_len()
        .expect("validated sampler has a representable canonical length");
    let encoded = program
        .canonical_v1_bytes()
        .expect("validated sampler canonical encoding succeeds");
    assert_eq!(measured, encoded.len());
}

#[test]
fn definition_round_trip_revalidates_typed_hashes() {
    let graph = FxGraph::try_new(vec![FxNode::Style {
        properties: vec![super::FxProperty::new(
            FxPropertyId::Opacity,
            FxRuntimeValue::F32(finite(0.75)).into(),
        )],
    }])
    .expect("valid graph");
    let definition = FxDefinition::new(
        FxId::try_new("game", "ui.effects.fade").expect("Fx ID"),
        vec![runtime_parameter(
            0,
            "strength",
            FxRuntimeType::F32,
            Some(FxRuntimeValue::F32(finite(0.75))),
        )],
        graph,
    )
    .expect("definition");
    let bytes = serde_json::to_vec(&definition).expect("definition serializes");
    assert_eq!(
        serde_json::from_slice::<FxDefinition>(&bytes).expect("hashes revalidate"),
        definition
    );
}

#[test]
fn constructor_property_expectations_are_the_validator_source_of_truth() {
    assert_eq!(FxPropertyId::Target.value_type(), FxStaticType::Target);
    assert_eq!(FxPropertyId::Phase.value_type(), FxStaticType::Phase);
    assert_eq!(
        FxPropertyId::Sampler.value_type(),
        FxStaticType::Runtime(FxRuntimeType::Transform2D)
    );
    assert!(!FxNodeKind::Transform.accepts_property(FxPropertyId::Coverage));
    let parameter = runtime_parameter(0, "value", FxRuntimeType::F32, None);
    assert!(
        FxStaticType::Runtime(FxRuntimeType::F32)
            .accepts(&FxStaticValue::Parameter(parameter.parameter_ref()))
    );
}

#[test]
fn definition_validation_rejects_out_of_bounds_parameter_slots() {
    let foreign = runtime_parameter(1, "foreign", FxRuntimeType::F32, None);
    let graph = FxGraph::try_new(vec![FxNode::Style {
        properties: vec![super::FxProperty::new(
            FxPropertyId::Opacity,
            FxStaticValue::Parameter(foreign.parameter_ref()),
        )],
    }])
    .expect("property is locally typed");
    let error = FxDefinition::new(
        FxId::try_new("game", "ui.effects.fade").expect("Fx ID"),
        vec![runtime_parameter(0, "strength", FxRuntimeType::F32, None)],
        graph,
    )
    .expect_err("definition slot inventory is authoritative");
    assert_eq!(
        error,
        FxDefinitionError::ParameterReferenceOutOfBounds {
            index: 1,
            available: 1
        }
    );
}

#[test]
fn heterogeneous_parameter_layout_binds_one_dense_runtime_and_static_template() {
    let (definition, resource, uniforms) = heterogeneous_definition();
    assert_heterogeneous_layout(&definition);
    let application = bind_heterogeneous_application(&definition);
    assert_heterogeneous_application(&definition, &application, resource, uniforms);
    assert_heterogeneous_snapshot(&definition, &application);
}

#[test]
fn instance_snapshot_round_trip_preserves_logical_state() {
    let definition = FxDefinition::new(
        FxId::try_new("game", "ui.effects.fade").expect("Fx ID"),
        vec![runtime_parameter(0, "amount", FxRuntimeType::F32, None)],
        FxGraph::default(),
    )
    .unwrap();
    let application = FxApplication::bind(
        &definition,
        FxApplicationDraft::try_new(
            definition.id().clone(),
            vec![Some(FxDefinitionArgumentValue::Runtime(
                FxRuntimeValue::F32(finite(0.75)),
            ))],
            0,
            None,
        )
        .unwrap(),
    )
    .unwrap();
    let snapshot = FxInstanceSnapshot::try_new(
        application.instance_identity(owner(b"snapshot")),
        &definition,
        FxInstanceActivation::new(
            FxLogicalTime::try_new(seconds(4.5)).expect("logical time"),
            Some(FxAuthoredSeed::new(42)),
            FxGraphChildPath::try_new(vec![2, 1]).expect("child path"),
        ),
        application.template().clone(),
        vec![FxRuntimeValue::F32(finite(0.75))].into_boxed_slice(),
        Vec::new(),
    )
    .expect("snapshot bounds");
    let bytes = serde_json::to_vec(&snapshot).expect("snapshot serializes");
    assert_eq!(
        serde_json::from_slice::<FxInstanceSnapshot>(&bytes).expect("snapshot revalidates"),
        snapshot
    );
}

#[test]
fn provider_state_is_canonicalized_by_identity_and_accepts_only_version_one() {
    let definition = FxDefinition::new(
        FxId::try_new("game", "provider_state").unwrap(),
        Vec::new(),
        FxGraph::default(),
    )
    .unwrap();
    let application = bind_empty(&definition, 0);
    let zeta =
        FxProviderStateRecord::try_new(FxId::try_new("provider", "zeta").unwrap(), Vec::new())
            .unwrap();
    let alpha =
        FxProviderStateRecord::try_new(FxId::try_new("provider", "alpha").unwrap(), Vec::new())
            .unwrap();
    let snapshot = FxInstanceSnapshot::try_new(
        application.instance_identity(owner(b"provider-state")),
        &definition,
        FxInstanceActivation::new(FxLogicalTime::zero(), None, FxGraphChildPath::default()),
        application.template().clone(),
        Box::default(),
        vec![zeta, alpha],
    )
    .unwrap();
    assert_eq!(snapshot.provider_state()[0].provider().function(), "alpha");
    assert_eq!(snapshot.provider_state()[1].provider().function(), "zeta");
    assert!(
        snapshot
            .provider_state()
            .iter()
            .all(|row| row.version() == 1)
    );

    let encoded = serde_json::to_string(&snapshot.provider_state()[0]).unwrap();
    let tampered = encoded.replace("\"version\":1", "\"version\":2");
    assert!(serde_json::from_str::<FxProviderStateRecord>(&tampered).is_err());
}

#[test]
fn division_by_zero_is_a_structured_evaluation_error() {
    let program = sampler(
        FxRuntimeType::F32,
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(finite(1.0)),
            },
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(FiniteF32::ZERO),
            },
            ValueInstruction::Div,
            ValueInstruction::Return,
        ],
    );
    let error = program
        .evaluate(
            ValueProgramInputs {
                parameters: &[],
                state: &[],
            },
            context(0.0, 0),
            &mut FxEvaluationBudget::default(),
        )
        .expect_err("zero division fails");
    assert_eq!(error, FxEvaluationError::DivisionByZero { instruction: 2 });
}

#[test]
fn evaluator_rejects_nonfinite_arithmetic_without_clamping() {
    let program = sampler(
        FxRuntimeType::F32,
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(finite(f32::MAX)),
            },
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(finite(f32::MAX)),
            },
            ValueInstruction::Add,
            ValueInstruction::Return,
        ],
    );
    assert!(matches!(
        program.evaluate(
            ValueProgramInputs {
                parameters: &[],
                state: &[],
            },
            context(0.0, 0),
            &mut FxEvaluationBudget::default(),
        ),
        Err(FxEvaluationError::NonFiniteResult {
            instruction: 2,
            operation: "add"
        })
    ));
}

#[test]
fn floor_to_i32_is_checked_before_hash_noise() {
    let program = sampler(
        FxRuntimeType::F32,
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(finite(3.75)),
            },
            ValueInstruction::FloorToI32,
            ValueInstruction::HashNoise,
            ValueInstruction::Return,
        ],
    );
    let value = program
        .evaluate(
            ValueProgramInputs {
                parameters: &[],
                state: &[],
            },
            context(0.0, 2),
            &mut FxEvaluationBudget::default(),
        )
        .expect("finite bucket evaluates");
    assert_eq!(
        value,
        FxRuntimeValue::F32(
            context(0.0, 2)
                .deterministic_noise(3)
                .expect("same typed noise context")
        )
    );

    let overflow = sampler(
        FxRuntimeType::I32,
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(finite(f32::MAX)),
            },
            ValueInstruction::FloorToI32,
            ValueInstruction::Return,
        ],
    );
    assert!(matches!(
        overflow.evaluate(
            ValueProgramInputs {
                parameters: &[],
                state: &[],
            },
            context(0.0, 0),
            &mut FxEvaluationBudget::default(),
        ),
        Err(FxEvaluationError::IntegerConversion {
            instruction: 1,
            operation: "floor_to_i32"
        })
    ));
}

#[test]
fn make_color_validates_each_channel_without_clamping() {
    let program = sampler(
        FxRuntimeType::Color,
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(finite(0.25)),
            },
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(finite(0.5)),
            },
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(finite(0.75)),
            },
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(FiniteF32::ONE),
            },
            ValueInstruction::MakeColor,
            ValueInstruction::Return,
        ],
    );
    let value = program
        .evaluate(
            ValueProgramInputs {
                parameters: &[],
                state: &[],
            },
            context(0.0, 0),
            &mut FxEvaluationBudget::default(),
        )
        .expect("closed color evaluates");
    let FxRuntimeValue::Color(color) = value else {
        panic!("declared Color result");
    };
    assert_eq!(color.red().value(), finite(0.25));
    assert_eq!(color.green().value(), finite(0.5));
    assert_eq!(color.blue().value(), finite(0.75));
    assert_eq!(color.alpha().value(), FiniteF32::ONE);

    let invalid = sampler(
        FxRuntimeType::Color,
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(finite(1.5)),
            },
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(FiniteF32::ZERO),
            },
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(FiniteF32::ZERO),
            },
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(FiniteF32::ONE),
            },
            ValueInstruction::MakeColor,
            ValueInstruction::Return,
        ],
    );
    assert_eq!(
        invalid
            .evaluate(
                ValueProgramInputs {
                    parameters: &[],
                    state: &[],
                },
                context(0.0, 0),
                &mut FxEvaluationBudget::default(),
            )
            .expect_err("out-of-range channel fails"),
        FxEvaluationError::InvalidOpacity { instruction: 4 }
    );
}

#[test]
fn sin_sampler_uses_logical_time_and_golden_ordinal_phase() {
    let program = sampler(
        FxRuntimeType::F32,
        vec![
            ValueInstruction::LoadContext {
                slot: super::FxContextSlot::Time,
            },
            ValueInstruction::LoadContext {
                slot: super::FxContextSlot::OrdinalPhase,
            },
            ValueInstruction::Add,
            ValueInstruction::Sin,
            ValueInstruction::Return,
        ],
    );
    let value = program
        .evaluate(
            ValueProgramInputs {
                parameters: &[],
                state: &[],
            },
            context(0.5, 1),
            &mut FxEvaluationBudget::default(),
        )
        .expect("sampler evaluates");
    let FxRuntimeValue::F32(value) = value else {
        panic!("declared F32 result");
    };
    assert_close(value.get(), (0.5 + FX_GOLDEN_ANGLE_RAD).sin());
}

#[test]
fn ordinal_phase_uses_fixed_golden_angle_bits() {
    assert_eq!(FX_GOLDEN_ANGLE_RAD.to_bits(), 0x4019_98ff);
    assert_eq!(
        context(9.0, 1)
            .ordinal_phase()
            .expect("phase remains finite")
            .to_bits(),
        0x4019_98ff
    );
    let expected = (7.0 * FX_GOLDEN_ANGLE_RAD).rem_euclid(std::f32::consts::TAU);
    assert_eq!(
        context(0.0, 7)
            .ordinal_phase()
            .expect("phase remains finite"),
        finite(expected)
    );
}

#[test]
fn reduce_motion_freezes_sampler_time_without_changing_ordinal() {
    let context = FxSampleContext::from_elapsed(seconds(12.0), 7, 123, true);
    assert_eq!(context.time(), FiniteF32::ZERO);
    assert_eq!(context.ordinal(), 7);
}

#[test]
fn evaluation_budget_is_shared_and_fails_before_partial_return() {
    let program = sampler(
        FxRuntimeType::F32,
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(finite(2.0)),
            },
            ValueInstruction::Return,
        ],
    );
    let error = program
        .evaluate(
            ValueProgramInputs {
                parameters: &[],
                state: &[],
            },
            context(0.0, 0),
            &mut FxEvaluationBudget::new(1),
        )
        .expect_err("return requires a second operation");
    assert_eq!(
        error,
        FxEvaluationError::BudgetExceeded {
            instruction: 1,
            limit: 1
        }
    );
}

#[test]
fn seed_derivation_is_stable_and_includes_nested_child_path() {
    let id = FxId::try_new("game", "ui.effects.wave").expect("Fx id");
    let instance = FxInstanceId::derive(&id, owner(b"nested-seed"), 0);
    let semantic = FxSemanticHash::from_bytes([0x5a; 32]);
    let first_path = FxGraphChildPath::try_new(vec![1, 2]).expect("path");
    let second_path = FxGraphChildPath::try_new(vec![1, 3]).expect("path");
    let first = derive_deterministic_seed(
        instance,
        semantic,
        Some(FxAuthoredSeed::new(17)),
        &first_path,
    );
    assert_eq!(
        first,
        derive_deterministic_seed(
            instance,
            semantic,
            Some(FxAuthoredSeed::new(17)),
            &first_path,
        )
    );
    assert_ne!(
        first,
        derive_deterministic_seed(
            instance,
            semantic,
            Some(FxAuthoredSeed::new(17)),
            &second_path,
        )
    );
    assert_ne!(
        first,
        derive_deterministic_seed(instance, semantic, None, &first_path)
    );
}

#[test]
fn unsupported_target_interface_fails_transactionally_with_typed_context() {
    let definition = FxId::try_new("game", "ui.effects.blur").expect("Fx id");
    let instance = FxInstanceId::derive(&definition, owner(b"unsupported-target"), 0);
    let context = FxDiagnosticContext {
        definition: Some(definition),
        instance: Some(instance),
        ..FxDiagnosticContext::default()
    };
    let operation = ResolvedFxOperation::Filter(ResolvedFilterOperation {
        phase: FxPhase::GlyphColor,
        target: FxTarget::Glyph,
        blur_radius: None,
        brightness: None,
        contrast: None,
        saturation: None,
    });
    let plan = ResolvedFxPlan::resolve_application(
        &context,
        &FxCapabilitySet::canonical(),
        vec![operation],
    );
    assert!(plan.glyph().is_empty());
    assert_eq!(plan.diagnostics().len(), 1);
    assert_eq!(
        plan.diagnostics()[0].code,
        FxDiagnosticCode::UnsupportedCapability
    );
    assert_eq!(plan.diagnostics()[0].context.instance, Some(instance));
}

#[test]
fn interactive_noninvertible_transform_is_not_committed() {
    let transform = Transform2D {
        scale_x: FiniteF32::ZERO,
        ..Transform2D::default()
    }
    .resolve()
    .expect("zero scale is a finite visual transform");
    let operation = ResolvedFxOperation::Transform(super::ResolvedTransformOperation::new(
        FxPhase::LayoutTransform,
        FxTarget::Node,
        transform,
        true,
    ));
    let plan = ResolvedFxPlan::resolve_application(
        &FxDiagnosticContext::default(),
        &FxCapabilitySet::canonical(),
        vec![operation],
    );
    assert!(plan.layout().is_empty());
    assert_eq!(
        plan.diagnostics()[0].code,
        FxDiagnosticCode::NonInvertibleTransform
    );
}

#[test]
fn provider_output_enforces_typed_operation_budget() {
    let mut output = FxProviderOutput::new(FxProviderLimits {
        max_operations: 1,
        max_values_per_operation: 2,
        max_state_values: 0,
    });
    let operation = ResolvedFxOperation::Color(ResolvedColorOperation {
        phase: FxPhase::GlyphColor,
        target: FxTarget::Glyph,
        tint: None,
        multiply: None,
        opacity: None,
    });
    output
        .try_push(operation.clone())
        .expect("first output fits");
    assert_eq!(
        output.try_push(operation),
        Err(FxProviderError::OutputBudgetExceeded { limit: 1 })
    );
}

#[test]
fn graph_evaluator_resolves_typed_values_and_transform_sampler_in_authored_order() {
    let id = FxId::try_new("game", "dialogue.wave").expect("Fx id");
    let transform = Transform2D {
        translate_y: length(4.0),
        ..Transform2D::default()
    };
    let graph = FxGraph::try_new(vec![
        FxNode::Text {
            properties: vec![FxProperty::new(
                FxPropertyId::Weight,
                FxRuntimeValue::I32(700).into(),
            )],
        },
        FxNode::Transform {
            fx: id.clone(),
            properties: vec![
                FxProperty::new(FxPropertyId::Target, FxStaticValue::Target(FxTarget::Glyph)),
                FxProperty::new(
                    FxPropertyId::Sampler,
                    FxStaticValue::Sampler(sampler(
                        FxRuntimeType::Transform2D,
                        vec![
                            ValueInstruction::Constant {
                                value: FxRuntimeValue::Transform2D(transform),
                            },
                            ValueInstruction::Return,
                        ],
                    )),
                ),
            ],
        },
    ])
    .expect("typed graph");
    let definition = FxDefinition::new(id.clone(), Vec::new(), graph).expect("definition");
    let application = bind_empty(&definition, 2);
    let instance = FxInstanceSnapshot::try_new(
        application.instance_identity(owner(b"dialogue-transform")),
        &definition,
        FxInstanceActivation::new(
            FxLogicalTime::zero(),
            Some(FxAuthoredSeed::new(9)),
            FxGraphChildPath::default(),
        ),
        application.template().clone(),
        Box::default(),
        Vec::new(),
    )
    .expect("snapshot");
    let mut budget = FxEvaluationBudget::new(32);
    let plan = FxGraphEvaluator::evaluate(
        &application,
        FxEvaluationBinding {
            definition: &definition,
            instance: &instance,
            runtime_time: FxLogicalTime::try_new(seconds(1.0)).expect("runtime time"),
        },
        7,
        false,
        false,
        &FxCapabilitySet::canonical(),
        &mut budget,
    );

    assert!(plan.is_conformant(), "{:?}", plan.diagnostics());
    let [ResolvedFxOperation::TextStyle(text)] = plan.layout() else {
        panic!("text operation is retained before layout");
    };
    assert_eq!(text.weight, Some(700));
    let [ResolvedFxOperation::Transform(transform)] = plan.glyph() else {
        panic!("transform sampler resolves at glyph phase");
    };
    assert_eq!(transform.transform.translation()[1], length(4.0));
}

#[test]
fn graph_evaluator_budget_failure_commits_no_partial_operations() {
    let id = FxId::try_new("game", "too.expensive").expect("Fx id");
    let graph = FxGraph::try_new(vec![
        FxNode::Text {
            properties: vec![FxProperty::new(
                FxPropertyId::Weight,
                FxRuntimeValue::I32(700).into(),
            )],
        },
        FxNode::Color {
            properties: vec![FxProperty::new(
                FxPropertyId::Opacity,
                FxRuntimeValue::F32(finite(0.5)).into(),
            )],
        },
    ])
    .expect("typed graph");
    let definition = FxDefinition::new(id.clone(), Vec::new(), graph).expect("definition");
    let application = bind_empty(&definition, 0);
    let instance = FxInstanceSnapshot::try_new(
        application.instance_identity(owner(b"budget")),
        &definition,
        FxInstanceActivation::new(FxLogicalTime::zero(), None, FxGraphChildPath::default()),
        application.template().clone(),
        Box::default(),
        Vec::new(),
    )
    .expect("snapshot");
    let mut budget = FxEvaluationBudget::new(1);
    let plan = FxGraphEvaluator::evaluate(
        &application,
        FxEvaluationBinding {
            definition: &definition,
            instance: &instance,
            runtime_time: FxLogicalTime::zero(),
        },
        0,
        false,
        false,
        &FxCapabilitySet::canonical(),
        &mut budget,
    );

    assert!(plan.layout().is_empty());
    assert!(plan.glyph().is_empty());
    assert_eq!(
        plan.diagnostics()[0].code,
        FxDiagnosticCode::EvaluationBudgetExceeded
    );
}

#[test]
fn semantic_hash_canonicalizes_named_property_order() {
    let first = FxGraph::try_new(vec![FxNode::Style {
        properties: vec![
            super::FxProperty::new(
                FxPropertyId::Opacity,
                FxStaticValue::Runtime(FxRuntimeValue::F32(finite(0.5))),
            ),
            super::FxProperty::new(
                FxPropertyId::Size,
                FxRuntimeValue::Length(length(18.0)).into(),
            ),
        ],
    }])
    .expect("valid style graph");
    let reordered = FxGraph::try_new(vec![FxNode::Style {
        properties: vec![
            super::FxProperty::new(
                FxPropertyId::Size,
                FxRuntimeValue::Length(length(18.0)).into(),
            ),
            super::FxProperty::new(
                FxPropertyId::Opacity,
                FxStaticValue::Runtime(FxRuntimeValue::F32(finite(0.5))),
            ),
        ],
    }])
    .expect("valid style graph");
    assert_eq!(
        FxSemanticHash::for_graph(&first),
        FxSemanticHash::for_graph(&reordered)
    );
}

#[test]
fn provider_kind_inventory_is_typed() {
    let kinds = BTreeSet::from([super::FxProviderKind::Builtin, super::FxProviderKind::Wasm]);
    assert!(kinds.contains(&super::FxProviderKind::Builtin));
    assert!(!kinds.contains(&super::FxProviderKind::Rust));
}
