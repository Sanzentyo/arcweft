use super::super::functions::RuntimeFunctionSourceLimits;
use super::super::pattern::tests::{declarations, frames, meter, resources};
use super::*;

#[test]
fn original_expression_sources_restore_inline_formatter_and_dialogue_effect_captures_with_original_states()
 {
    use super::super::callable_states::{RuntimeCallableSourceLimits, tests as graph};
    use super::super::expression::{
        RuntimeExpressionSourceHandles,
        tests::{add_function, decode},
    };
    let mut original_builder = fixture_builder();
    let unit = RuntimeSemanticTypeId::from_bytes([190; 32]);
    let callback_type = RuntimeSemanticTypeId::from_bytes([191; 32]);
    original_builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(
                    callback_type,
                    RuntimePlanTypeProjection::Function {
                        contract: RuntimeFunctionTypeContract::new(
                            RuntimeTypeBinder::new(0, 0, 0),
                            crate::effect_row::EffectPredicate::unconstrained(),
                            crate::effect_row::EffectFormula::empty(),
                        ),
                        parameters: Box::new([]),
                        result: unit,
                    },
                ),
            ],
            [],
        )
        .unwrap();
    let callback = graph::reserve_executable_function(&mut original_builder, 192, &[BOOL], 1, unit);
    original_builder
        .define_function_site_seed(
            &callback,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                    unit,
                    RuntimeExprSeedKind::Value(RuntimeValue::Unit),
                ))]),
            }),
        )
        .unwrap();
    let state = original_builder
        .function_callable_state_seed(callback_type, &callback)
        .unwrap();
    original_builder
        .register_dialogue_content_template_seed(RuntimeDialogueContentTemplateManifestSeed {
            id: template(2),
            digest: RuntimeDialogueContentTemplateDigest::from_bytes([193; 32]),
            slots: Box::new([RuntimeDialogueContentSlotSeed {
                slot: RuntimeDialogueValueSlotId::from_zero_based(0).unwrap(),
                role: RuntimeDialogueValueRole::Interpolation,
                semantic_type: BOOL,
            }]),
            effects: [
                RuntimeDialogueContentEffectTrigger::Content,
                RuntimeDialogueContentEffectTrigger::Delay {
                    duration: LogicalDuration::from_nanos(7),
                },
            ]
            .into_iter()
            .enumerate()
            .map(|(index, trigger)| RuntimeDialogueContentEffectSlotSeed {
                site: RuntimeDialogueEffectSiteId::from_zero_based(index).unwrap(),
                trigger,
                capture_types: Box::new([BOOL]),
            })
            .collect(),
        })
        .unwrap();
    let boolean =
        |value| RuntimeExprSeed::new(BOOL, RuntimeExprSeedKind::Value(RuntimeValue::Bool(value)));
    add_function(
        &mut original_builder,
        194,
        RuntimeExprSeed::format_content(
            RuntimeDialogueOpaqueRole::Content.semantic_identity(),
            template(1),
            None,
            None,
            false,
            [RuntimeFormatContentOperandSeed::new(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([195; 32]),
                Param::Value,
                boolean(true),
            )],
        ),
    );
    add_function(
        &mut original_builder,
        196,
        RuntimeExprSeed::dialogue_content_with_effects(
            RuntimeDialogueOpaqueRole::Content.semantic_identity(),
            template(2),
            [boolean(true)],
            (0..2).map(|index| RuntimeDialogueEffectSiteSeed {
                site: RuntimeDialogueEffectSiteId::from_zero_based(index).unwrap(),
                state: state.clone(),
                captures: Box::new([boolean(index == 0)]),
            }),
        ),
    );
    let seal = RuntimeTaskPlanSealLimits::default();
    let plan = original_builder.finish().unwrap();
    let (mut restored_source, functions) = source(&plan, meter());
    let content_bytes = encode(&plan);
    let attempts = PreparedRuntimeContentDeclarationSources::preflight(
        &content_bytes,
        limits(&plan),
        &mut restored_source.meter,
    )
    .unwrap()
    .reserve(&mut restored_source)
    .unwrap();
    let state_bytes = graph::encode(&plan);
    let state_limits = RuntimeCallableSourceLimits {
        rows: 100,
        elements: 100_000,
        encoded_bytes: 1 << 22,
        decoded_bytes: 1 << 24,
        type_count: plan.type_table().len(),
        function_count: functions.len(),
    };
    let states = RuntimeCallableStateTable::preflight_sources(
        &state_bytes,
        state_limits,
        &mut restored_source.meter,
    )
    .unwrap()
    .materialize(&mut restored_source, &functions)
    .unwrap();
    let handles = RuntimeExpressionSourceHandles {
        states: &states,
        specializations: &[],
        helpers: &[],
        methods: &[],
        attempts: &attempts,
    };
    restored_source
        .builder
        .define_function_site_seed(
            &functions[0],
            RuntimeFunctionSiteBodySeed::Executable(body(&attempts[2])),
        )
        .unwrap();
    for (function, handle) in plan.function_sites().iter().zip(&functions).skip(1) {
        let body = match function.body() {
            RuntimeFunctionSiteBody::Expression(expression) => {
                RuntimeFunctionSiteBodySeed::Expression(decode(
                    expression,
                    &plan,
                    &mut restored_source,
                    &handles,
                ))
            }
            RuntimeFunctionSiteBody::Executable(body) => {
                let [FlowOp::ReturnExpr(expression)] = body.ops() else {
                    unreachable!("original callback has one typed return");
                };
                RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: Box::new([RuntimeFlowOpSeed::ReturnExpr(decode(
                        expression,
                        &plan,
                        &mut restored_source,
                        &handles,
                    ))]),
                })
            }
        };
        restored_source
            .builder
            .define_function_site_seed(handle, body)
            .unwrap();
    }
    let restored = restored_source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan();
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(restored.callable_states(), plan.callable_states());
    assert_eq!(
        restored.dialogue_content_templates(),
        plan.dialogue_content_templates()
    );
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
}

#[test]
fn original_expression_sources_restore_formatter_attempt_reference_in_its_actual_executable_body() {
    use super::super::expression::{RuntimeExpressionSourceHandles, tests::decode};
    let plan = fixture();
    let bytes = encode(&plan);
    let (mut source, functions) = source(&plan, meter());
    let attempts = PreparedRuntimeContentDeclarationSources::preflight(
        &bytes,
        limits(&plan),
        &mut source.meter,
    )
    .unwrap()
    .reserve(&mut source)
    .unwrap();
    let handles = RuntimeExpressionSourceHandles {
        states: &[],
        specializations: &[],
        helpers: &[],
        methods: &[],
        attempts: &attempts,
    };
    let RuntimeFunctionSiteBody::Executable(original) =
        plan.function_sites().iter().next().unwrap().body()
    else {
        unreachable!()
    };
    let FlowOp::ReturnExpr(expression) = original.ops().last().unwrap() else {
        unreachable!()
    };
    let decoded = decode(expression, &plan, &mut source, &handles);
    // Operation source transport is separate work. Here the existing genuine
    // staging operations establish the formatter's actual execution context.
    let mut restored_body = body(&attempts[2]);
    *restored_body.ops.last_mut().unwrap() = RuntimeFlowOpSeed::ReturnExpr(decoded);
    source
        .builder
        .define_function_site_seed(
            &functions[0],
            RuntimeFunctionSiteBodySeed::Executable(restored_body),
        )
        .unwrap();
    let seal = RuntimeTaskPlanSealLimits::default();
    let restored = source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan();
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(restored.format_attempts(), plan.format_attempts());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
}
use crate::entry::RuntimeDialogueContentTemplateDigest;
use crate::pattern::{RuntimeOpaqueTypeAdmission, RuntimeSemanticTypeId};
use crate::plan::*;
use crate::runtime_id::*;
use crate::time::LogicalDuration;
use crate::value::{
    RuntimeDialogueOpaqueRole, RuntimeFmtParameterId as Param, RuntimeOpaquePersistence,
    RuntimeOpaqueValueClass, RuntimeValue,
};

const BOOL: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([171; 32]);
const STRING: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([172; 32]);
const COLOR: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([173; 32]);
const SCHEMA: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([174; 32]);
fn template(index: usize) -> RuntimeDialogueContentTemplateId {
    RuntimeDialogueContentTemplateId::from_zero_based(index).unwrap()
}
fn manifest(index: usize, formatted: bool) -> RuntimeDialogueContentTemplateManifestSeed {
    RuntimeDialogueContentTemplateManifestSeed {
        id: template(index),
        digest: RuntimeDialogueContentTemplateDigest::from_bytes([175 + index as u8; 32]),
        slots: if formatted {
            Box::new([RuntimeDialogueContentSlotSeed {
                slot: RuntimeDialogueValueSlotId::from_zero_based(0).unwrap(),
                role: RuntimeDialogueValueRole::Formatted,
                semantic_type: RuntimeDialogueOpaqueRole::Content.semantic_identity(),
            }])
        } else {
            RuntimeDialogueValueRole::ALL
                .into_iter()
                .enumerate()
                .map(|(i, role)| RuntimeDialogueContentSlotSeed {
                    slot: RuntimeDialogueValueSlotId::from_zero_based(i).unwrap(),
                    role,
                    semantic_type: if role == RuntimeDialogueValueRole::Interpolation {
                        SCHEMA
                    } else {
                        RuntimeDialogueOpaqueRole::Content.semantic_identity()
                    },
                })
                .collect()
        },
        effects: if formatted {
            Box::new([])
        } else {
            Box::new([
                RuntimeDialogueContentEffectSlotSeed {
                    site: RuntimeDialogueEffectSiteId::from_zero_based(0).unwrap(),
                    trigger: RuntimeDialogueContentEffectTrigger::Content,
                    capture_types: Box::new([BOOL, STRING]),
                },
                RuntimeDialogueContentEffectSlotSeed {
                    site: RuntimeDialogueEffectSiteId::from_zero_based(1).unwrap(),
                    trigger: RuntimeDialogueContentEffectTrigger::Delay {
                        duration: LogicalDuration::from_nanos(u64::MAX),
                    },
                    capture_types: Box::new([COLOR]),
                },
            ])
        },
    }
}
fn formatter_specs() -> [Box<[RuntimeFormatAttemptOperandSeed]>; 3] {
    let operand = |parameter, ty| RuntimeFormatAttemptOperandSeed { parameter, ty };
    [
        Box::new([
            operand(Param::Color, COLOR),
            operand(Param::Value, BOOL),
            operand(Param::Style, STRING),
            operand(Param::Locale, STRING),
            operand(Param::Currency, STRING),
            operand(Param::NoneValue, STRING),
            operand(Param::OnError, BOOL),
        ]),
        Box::new([
            operand(Param::Value, BOOL),
            operand(Param::Fallback, STRING),
        ]),
        Box::new([
            operand(Param::DiscardError, BOOL),
            operand(Param::Value, BOOL),
        ]),
    ]
}
fn body(attempt: &RuntimeFormatAttemptSeedId) -> RuntimeExecutableBodySeed {
    RuntimeExecutableBodySeed {
        effects: RuntimeEffectSet::empty(),
        ops: Box::new([
            RuntimeFlowOpSeed::FormatOperandAttempt {
                attempt: attempt.clone(),
                parameter: Param::DiscardError,
                body: vec![],
                value: RuntimeExprSeed::new(
                    BOOL,
                    RuntimeExprSeedKind::Value(RuntimeValue::Bool(false)),
                ),
            },
            RuntimeFlowOpSeed::FormatOperandAttempt {
                attempt: attempt.clone(),
                parameter: Param::Value,
                body: vec![],
                value: RuntimeExprSeed::new(
                    BOOL,
                    RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
                ),
            },
            RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                RuntimeDialogueOpaqueRole::Content.semantic_identity(),
                RuntimeExprSeedKind::FormatContent {
                    template: template(1),
                    attempt: Some(attempt.clone()),
                    operands: Box::new([]),
                    project_method: None,
                    project_option: false,
                },
            )),
        ]),
    }
}
fn fixture() -> RuntimePlan {
    fixture_builder().finish().unwrap()
}
fn fixture_builder() -> RuntimePlanBuilder {
    let mut builder = RuntimePlanBuilder::new();
    let content = RuntimeDialogueOpaqueRole::Content;
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(BOOL, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(STRING, RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(COLOR, RuntimePlanTypeProjection::Color),
                RuntimePlanTypeSeed::new(
                    content.semantic_identity(),
                    RuntimePlanTypeProjection::Opaque {
                        producer: content.producer(),
                        admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                        value_class: RuntimeOpaqueValueClass::Plain,
                        persistence: RuntimeOpaquePersistence::SnapshotOnly,
                        arguments: Box::new([]),
                    },
                ),
            ],
            [],
        )
        .unwrap();
    builder
        .register_dialogue_content_template_seed(manifest(0, false))
        .unwrap();
    builder
        .register_plain_text_context_template_seed(manifest(1, true))
        .unwrap();
    let attempts = formatter_specs()
        .into_iter()
        .map(|operands| {
            builder
                .reserve_format_attempt_seed(RuntimeFormatAttemptDeclarationSeed {
                    template: template(1),
                    operands,
                })
                .unwrap()
        })
        .collect::<Vec<_>>();
    let function = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([179; 32]),
            role: RuntimeFunctionSemanticRole::Ordinary,
            function_type: None,
            inputs: Box::new([]),
            result: content.semantic_identity(),
            body_kind: RuntimeFunctionSiteBodyKind::Executable,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap();
    builder
        .define_function_site_seed(
            &function,
            RuntimeFunctionSiteBodySeed::Executable(body(&attempts[2])),
        )
        .unwrap();
    builder
}
fn limits(plan: &RuntimePlan) -> RuntimeContentDeclarationSourceLimits {
    RuntimeContentDeclarationSourceLimits {
        templates: 100,
        attempts: 100,
        elements: 100_000,
        encoded_bytes: 1 << 22,
        decoded_bytes: 1 << 24,
        type_count: plan.type_table().len(),
    }
}
fn encode(plan: &RuntimePlan) -> Vec<u8> {
    let prepared = plan
        .prepare_content_declaration_sources_encoding(limits(plan), &mut meter())
        .unwrap();
    let mut bytes = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut bytes).unwrap();
    bytes
}
fn source(
    plan: &RuntimePlan,
    mut bank: TaskSemanticMeter,
) -> (
    RuntimePlanSourceConstruction,
    Box<[RuntimeFunctionSiteSeedId]>,
) {
    let frames = frames(plan);
    let declarations = declarations(&frames, &mut bank);
    let limits = RuntimeFunctionSourceLimits {
        rows: 100,
        elements: 100_000,
        pattern_nodes: 10_000,
        encoded_bytes: 1 << 22,
        decoded_bytes: 1 << 24,
        type_count: plan.type_table().len(),
        local_count: plan.local_declarations().len(),
    };
    let encoded = plan
        .function_sites()
        .prepare_declaration_sources_encoding(limits, &resources(), &mut bank)
        .unwrap();
    let mut bytes = Vec::with_capacity(encoded.encoded_bytes());
    encoded.append_to(&mut bytes).unwrap();
    let functions = RuntimeFunctionSiteTable::preflight_declaration_sources(
        &bytes,
        limits,
        &resources(),
        &mut bank,
    )
    .unwrap();
    let mut source = declarations.into_construction(bank).unwrap();
    let functions = functions.reserve(&mut source, &mut resources()).unwrap();
    (source, functions)
}

#[test]
fn content_declaration_sources_restore_all_schema_roles_effect_triggers_and_real_formatter_body_handles()
 {
    let plan = fixture();
    let bytes = encode(&plan);
    let encoded = plan
        .prepare_content_declaration_sources_encoding(limits(&plan), &mut meter())
        .unwrap();
    let (mut source, functions) = source(&plan, meter());
    let prepared = PreparedRuntimeContentDeclarationSources::preflight(
        &bytes,
        limits(&plan),
        &mut source.meter,
    )
    .unwrap();
    assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
    assert_eq!(prepared.elements(), encoded.elements());
    let attempts = prepared.reserve(&mut source).unwrap();
    source
        .builder
        .define_function_site_seed(
            &functions[0],
            RuntimeFunctionSiteBodySeed::Executable(body(&attempts[2])),
        )
        .unwrap();
    let seal = RuntimeTaskPlanSealLimits::default();
    let restored = source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan();
    assert_eq!(
        restored.dialogue_content_templates(),
        plan.dialogue_content_templates()
    );
    assert_eq!(restored.format_attempts(), plan.format_attempts());
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(
        restored.dialogue_content().plain_text_context_template(),
        plan.dialogue_content().plain_text_context_template()
    );
    assert!(
        restored
            .dialogue_content()
            .plain_text_context_template_proof()
            .is_none()
    );
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    assert_eq!(encode(&restored), bytes);
}

#[test]
fn content_declaration_sources_admit_exact_combined_limits_and_reject_one_less_before_allocation() {
    let plan = fixture();
    let bytes = encode(&plan);
    let prepared =
        PreparedRuntimeContentDeclarationSources::preflight(&bytes, limits(&plan), &mut meter())
            .unwrap();
    let exact = RuntimeContentDeclarationSourceLimits {
        templates: 2,
        attempts: 3,
        elements: prepared.elements() as u32,
        encoded_bytes: bytes.len(),
        decoded_bytes: prepared.storage_bytes(),
        ..limits(&plan)
    };
    PreparedRuntimeContentDeclarationSources::preflight(&bytes, exact, &mut meter()).unwrap();
    for low in [
        RuntimeContentDeclarationSourceLimits {
            templates: 1,
            ..exact
        },
        RuntimeContentDeclarationSourceLimits {
            attempts: 2,
            ..exact
        },
        RuntimeContentDeclarationSourceLimits {
            elements: exact.elements - 1,
            ..exact
        },
        RuntimeContentDeclarationSourceLimits {
            encoded_bytes: exact.encoded_bytes - 1,
            ..exact
        },
        RuntimeContentDeclarationSourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
    ] {
        let mut bank = meter();
        assert!(
            PreparedRuntimeContentDeclarationSources::preflight(&bytes, low, &mut bank).is_err()
        );
        assert!(bank.status().is_err());
    }
}

#[test]
fn content_declaration_sources_reject_every_truncation_and_invalid_scalar_metadata() {
    let plan = fixture();
    let bytes = encode(&plan);
    for end in 0..bytes.len() {
        assert!(
            PreparedRuntimeContentDeclarationSources::preflight(
                &bytes[..end],
                limits(&plan),
                &mut meter()
            )
            .is_err(),
            "prefix {end}"
        );
    }
    let first = DOMAIN.len() + 1 + 4 + 4;
    let (effects, last_end) = {
        let mut cursor = Cursor::new(&bytes, limits(&plan));
        cursor.header().unwrap();
        let first = cursor.template(0).unwrap();
        let second = cursor.template(1).unwrap();
        (
            first.effects.as_ptr() as usize - bytes.as_ptr() as usize,
            second.effects.as_ptr() as usize + second.effects.len() - bytes.as_ptr() as usize,
        )
    };
    for (offset, value) in [
        (0, b'?'),
        (DOMAIN.len(), 2),
        (first, 0),
        (first + 4 + 32 + 4, 0),
        (first + 4 + 32 + 4 + 4, 255),
        (effects + 4, 255),
        (effects + 4 + 1 + 4, 0),
        (last_end + 4, 0),
        (last_end + 4 + 4, 0),
        (last_end + 4 + 12, 255),
    ] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        assert!(
            PreparedRuntimeContentDeclarationSources::preflight(&bad, limits(&plan), &mut meter())
                .is_err(),
            "offset {offset}"
        );
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert!(
        PreparedRuntimeContentDeclarationSources::preflight(&trailing, limits(&plan), &mut meter())
            .is_err()
    );
}

#[test]
fn content_source_late_duplicate_formatter_parameter_cannot_publish_earlier_rows() {
    let plan = fixture();
    let mut bytes = encode(&plan);
    let second_operand = {
        let mut cursor = Cursor::new(&bytes, limits(&plan));
        let (n, _) = cursor.header().unwrap();
        for index in 0..n {
            cursor.template(index).unwrap();
        }
        cursor.attempt_count().unwrap();
        cursor.attempt(0, n).unwrap();
        let row = cursor.attempt(1, n).unwrap();
        row.operands.as_ptr() as usize + 5 - bytes.as_ptr() as usize
    };
    bytes[second_operand] = Param::Value.index() as u8;
    let (mut source, _) = source(&plan, meter());
    let prepared = PreparedRuntimeContentDeclarationSources::preflight(
        &bytes,
        limits(&plan),
        &mut source.meter,
    )
    .unwrap();
    assert!(matches!(
        prepared.reserve(&mut source),
        Err(Error::Construction(
            RuntimePlanBuildError::DuplicateFormatParameter {
                parameter: Param::Value
            }
        ))
    ));
    assert_eq!(source.builder.format_attempts.len(), 1);
    assert_eq!(source.builder.dialogue_content.template_count(), 2);
    assert!(source.meter.status().is_err());
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}

#[test]
fn content_source_plain_text_pointer_needs_the_actual_canonical_manifest_and_never_grants_a_proof()
{
    let plan = fixture();
    let mut bytes = encode(&plan);
    let marker = DOMAIN.len() + 1 + 4;
    bytes[marker..marker + 4].copy_from_slice(&1u32.to_le_bytes());
    let (mut source, _) = source(&plan, meter());
    let prepared = PreparedRuntimeContentDeclarationSources::preflight(
        &bytes,
        limits(&plan),
        &mut source.meter,
    )
    .unwrap();
    assert!(prepared.reserve(&mut source).is_err());
    assert_eq!(source.builder.dialogue_content.template_count(), 0);
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
    bytes[marker..marker + 4].copy_from_slice(&3u32.to_le_bytes());
    assert!(matches!(
        PreparedRuntimeContentDeclarationSources::preflight(&bytes, limits(&plan), &mut meter()),
        Err(Error::Reference {
            kind: "plain-text template",
            ..
        })
    ));
}

#[test]
fn content_source_rejects_existing_namespaces_before_reserving_formatters() {
    let plan = fixture();
    let bytes = encode(&plan);
    let (mut source, _) = source(&plan, meter());
    source
        .builder
        .register_dialogue_content_template_seed(manifest(0, false))
        .unwrap();
    let prepared = PreparedRuntimeContentDeclarationSources::preflight(
        &bytes,
        limits(&plan),
        &mut source.meter,
    )
    .unwrap();
    assert!(matches!(
        prepared.reserve(&mut source),
        Err(Error::Namespace {
            kind: "existing template",
            ..
        })
    ));
    assert!(source.builder.format_attempts.is_empty());
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}

#[test]
fn content_source_preflight_keeps_the_finite_original_bank_after_declaration_and_function_admission()
 {
    let plan = fixture();
    let bytes = encode(&plan);
    let (measured, _) = source(&plan, meter());
    let prelude = measured.meter.totals().0;
    let (mut source, _) = source(
        &plan,
        TaskSemanticMeter::new(prelude + bytes.len() as u64 * 3 - 1, 100_000_000),
    );
    assert_eq!(source.meter.totals().0, prelude);
    assert!(matches!(
        PreparedRuntimeContentDeclarationSources::preflight(
            &bytes,
            limits(&plan),
            &mut source.meter
        ),
        Err(Error::Resource(TaskSemanticEncodingError::SemanticWork))
    ));
    assert!(source.builder.format_attempts.is_empty());
    assert_eq!(source.builder.dialogue_content.template_count(), 0);
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}

#[test]
fn content_source_empty_namespaces_have_no_retained_row_storage_and_seal_normally() {
    let plan = RuntimePlanBuilder::new().finish().unwrap();
    let bytes = encode(&plan);
    let limits = RuntimeContentDeclarationSourceLimits {
        templates: 0,
        attempts: 0,
        elements: 0,
        encoded_bytes: bytes.len(),
        decoded_bytes: 0,
        type_count: 0,
    };
    let (mut source, _) = source(&plan, meter());
    let prepared =
        PreparedRuntimeContentDeclarationSources::preflight(&bytes, limits, &mut source.meter)
            .unwrap();
    assert_eq!(prepared.storage_bytes(), 0);
    assert_eq!(prepared.elements(), 0);
    assert!(prepared.reserve(&mut source).unwrap().is_empty());
    let seal = RuntimeTaskPlanSealLimits::default();
    let restored = source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan();
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
}

#[test]
fn content_schema_semantic_identity_does_not_manufacture_a_runtime_type_declaration() {
    let mut builder = RuntimePlanBuilder::new();
    let mut schema = manifest(0, true);
    schema.slots = Box::new([RuntimeDialogueContentSlotSeed {
        slot: RuntimeDialogueValueSlotId::from_zero_based(0).unwrap(),
        role: RuntimeDialogueValueRole::Interpolation,
        semantic_type: SCHEMA,
    }]);
    builder
        .register_dialogue_content_template_seed(schema)
        .unwrap();
    builder
        .register_plain_text_context_template_seed(manifest(1, true))
        .unwrap();
    let plan = builder.finish().unwrap();
    assert!(plan.type_table().is_empty());
    let bytes = encode(&plan);
    let (mut source, _) = source(&plan, meter());
    let prepared = PreparedRuntimeContentDeclarationSources::preflight(
        &bytes,
        limits(&plan),
        &mut source.meter,
    )
    .unwrap();
    assert!(prepared.reserve(&mut source).unwrap().is_empty());
    let seal = RuntimeTaskPlanSealLimits::default();
    let restored = source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan();
    assert!(restored.type_table().is_empty());
    assert_eq!(
        restored.dialogue_content_templates(),
        plan.dialogue_content_templates()
    );
    assert_eq!(encode(&restored), bytes);
}

#[test]
fn content_source_encoding_checks_capacity_before_changing_the_destination_prefix() {
    let plan = fixture();
    let prepared = plan
        .prepare_content_declaration_sources_encoding(limits(&plan), &mut meter())
        .unwrap();
    let mut bytes = Vec::with_capacity(prepared.encoded_bytes());
    bytes.push(27);
    let before = bytes.clone();
    assert!(matches!(
        prepared.append_to(&mut bytes),
        Err(Error::OutputCapacity { .. })
    ));
    assert_eq!(bytes, before);
}

#[test]
fn dense_template_lookup_preserves_interning_conflicts_and_unknown_identity_rejection() {
    let plan = fixture();
    assert!(plan.dialogue_content_templates().get(template(0)).is_some());
    assert!(plan.dialogue_content_templates().get(template(1)).is_some());
    assert!(plan.dialogue_content_templates().get(template(2)).is_none());
    let mut builder = RuntimePlanBuilder::new();
    let schema = manifest(0, true);
    assert_eq!(
        builder
            .register_dialogue_content_template_seed(schema.clone())
            .unwrap(),
        template(0)
    );
    assert_eq!(
        builder
            .register_dialogue_content_template_seed(schema.clone())
            .unwrap(),
        template(0)
    );
    assert_eq!(builder.dialogue_content.template_count(), 1);
    let mut conflict = schema;
    conflict.digest = RuntimeDialogueContentTemplateDigest::from_bytes([181; 32]);
    assert!(
        builder
            .register_dialogue_content_template_seed(conflict)
            .is_err()
    );
    assert_eq!(builder.dialogue_content.template_count(), 1);
    assert!(builder.finish().is_err());
}

#[test]
fn content_source_formatter_handles_keep_the_actual_construction_issuer() {
    let plan = fixture();
    let bytes = encode(&plan);
    let (mut own, functions) = source(&plan, meter());
    let (mut foreign, _) = source(&plan, meter());
    let own_handles =
        PreparedRuntimeContentDeclarationSources::preflight(&bytes, limits(&plan), &mut own.meter)
            .unwrap()
            .reserve(&mut own)
            .unwrap();
    let foreign_handles = PreparedRuntimeContentDeclarationSources::preflight(
        &bytes,
        limits(&plan),
        &mut foreign.meter,
    )
    .unwrap()
    .reserve(&mut foreign)
    .unwrap();
    assert!(own_handles[2].resolve(&own.builder.issuer).is_some());
    assert!(foreign_handles[2].resolve(&own.builder.issuer).is_none());
    assert!(matches!(
        own.builder.define_function_site_seed(
            &functions[0],
            RuntimeFunctionSiteBodySeed::Executable(body(&foreign_handles[2]))
        ),
        Err(RuntimePlanBuildError::ForeignFormatAttemptSeed)
    ));
    assert!(
        own.prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}
