use super::*;
use crate::awbc::schema::*;
use crate::entry::schema::value_encoding::ValueAdmissionOrigin;
use crate::entry::{RuntimeSchemaError, RuntimeSchemaLimits};
use crate::pattern::RuntimeCheckedType;

fn program() -> AwbcProgram {
    AwbcProgram {
        runtime_types: vec![AwbcRuntimeType::new(
            RuntimeCheckedType::String.semantic_identity_digest(),
            AwbcRuntimeTypeShape::String,
        )],
        signatures: vec![AwbcSignature {
            params: vec![],
            result: Some(AwbcTypeId(0)),
            effects: AwbcEffectSetId(0),
        }],
        functions: vec![AwbcFunction {
            definition: crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                *blake3::hash(b"arcweft.format-storage-test.definition.v1\0").as_bytes(),
            ),
            semantic_role: crate::plan::RuntimeFunctionSemanticRole::Ordinary,
            public_id: None,
            kind: AwbcFunctionKind::Ordinary,
            signature: AwbcSignatureId(0),
            type_context: None,
            input_ownership: vec![],
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange { start: 0, len: 1 },
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags::default(),
        }],
        frame_layouts: vec![AwbcFrameLayout {
            slots: vec![],
            scopes: vec![],
            max_scope_depth: 0,
        }],
        blocks: vec![AwbcBlock {
            owner: AwbcFunctionId(0),
            instructions: AwbcTableRange { start: 0, len: 1 },
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        }],
        instructions: vec![AwbcInstruction::FormatContent {
            destination: AwbcRegisterId(0),
            template: crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
                .unwrap(),
            attempt: None,
            attempt_operands: vec![],
            project_method: None,
            project_option: false,
            project_result: None,
            operands: vec![AwbcFormatOperand {
                parameter: RuntimeFmtParameterId::Value,
                function: AwbcFunctionId(0),
                captures: vec![],
            }],
        }],
        ..AwbcProgram::default()
    }
}

fn context<V>(program: &AwbcProgram) -> FrameValueContext<'_, V> {
    FrameValueContext {
        program,
        function: AwbcFunctionId(0),
        layout: &program.frame_layouts[0],
        binding: None,
        registers: &[],
        scope_count: 0,
    }
}

fn staging<V>(value: Option<V>) -> FormatStaging<V> {
    FormatStaging {
        site: FiberCursor {
            function: AwbcFunctionId(0),
            block: AwbcBlockId(0),
            instruction_offset: 0,
        },
        format_context: RuntimeFormatContext::new(arcweft_id::LocaleTag::try_new("en").unwrap()),
        next_operand: usize::from(value.is_some()),
        values: vec![value],
        first_recoverable: None,
    }
}

#[test]
fn live_and_inert_formatter_values_share_exact_resources_and_original_rows() {
    let mut program = program();
    let live = FiberFormatState {
        storage: staging(Some(RuntimeValue::String("cause".into()))),
    };
    let saved = FiberFormatState {
        storage: staging(Some(AwbcRuntimeValueSnapshot::String("cause".into()))),
    };
    let exact = RuntimeSchemaLimits {
        max_nodes: 1,
        max_encoded_bytes: 7,
        max_string_bytes: 5,
        max_validation_work: 5,
        ..RuntimeSchemaLimits::engine_default()
    };
    for origin in [
        ValueAdmissionOrigin::Live,
        ValueAdmissionOrigin::InertSnapshot,
    ] {
        let validate = |limits| {
            let mut resources = ValueResourceAdmission::snapshot(limits, origin);
            match origin {
                ValueAdmissionOrigin::Live => {
                    live.validate_inputs(&context(&program), &mut resources, RuntimeValue::view)
                }
                ValueAdmissionOrigin::InertSnapshot => saved.validate_inputs(
                    &context(&program),
                    &mut resources,
                    AwbcRuntimeValueSnapshot::view,
                ),
            }
        };
        validate(exact).unwrap();
        for short in [
            RuntimeSchemaLimits {
                max_nodes: 0,
                ..exact
            },
            RuntimeSchemaLimits {
                max_encoded_bytes: 6,
                ..exact
            },
            RuntimeSchemaLimits {
                max_string_bytes: 4,
                ..exact
            },
            RuntimeSchemaLimits {
                max_validation_work: 4,
                ..exact
            },
        ] {
            assert!(validate(short).is_err(), "one-over policy {short:?}");
        }
        let mut resources = ValueResourceAdmission::snapshot(exact, origin);
        saved
            .validate_inputs(
                &context(&program),
                &mut resources,
                AwbcRuntimeValueSnapshot::view,
            )
            .unwrap();
        let error = saved
            .validate_inputs(
                &context(&program),
                &mut resources,
                AwbcRuntimeValueSnapshot::view,
            )
            .unwrap_err();
        assert!(
            matches!(error, FiberStateError::Snapshot(crate::value::AwbcRuntimeValueSnapshotError::Resource(error))
            if matches!(error, RuntimeSchemaError::ValidationWork { limit: 5, consumed: 6, .. }))
        );
    }
    let duplicate = AwbcTypeId(1);
    program.runtime_types.push(AwbcRuntimeType::new(
        program.runtime_types[0].semantic_identity(),
        AwbcRuntimeTypeShape::Bool,
    ));
    program.signatures[0].result = Some(duplicate);
    let error = saved
        .validate_inputs(
            &context(&program),
            &mut ValueResourceAdmission::snapshot(
                RuntimeSchemaLimits::engine_default(),
                ValueAdmissionOrigin::InertSnapshot,
            ),
            AwbcRuntimeValueSnapshot::view,
        )
        .unwrap_err();
    assert!(
        matches!(error, FiberStateError::Snapshot(crate::value::AwbcRuntimeValueSnapshotError::ProgramType(error))
        if matches!(&*error, crate::program_types::RuntimeProgramTypeError::AwbcValue(error)
            if matches!(&**error, crate::awbc::value_admission::AwbcValueAdmissionError::Value { ty, .. } if *ty == duplicate)))
    );
}

#[test]
fn unreached_formatter_slots_charge_work_without_synthetic_values() {
    let program = program();
    let saved: FiberFormatState<AwbcRuntimeValueSnapshot> = FiberFormatState {
        storage: staging(None),
    };
    let exact = RuntimeSchemaLimits {
        max_nodes: 0,
        max_encoded_bytes: 0,
        max_validation_work: 3,
        ..RuntimeSchemaLimits::engine_default()
    };
    saved
        .validate_inputs(
            &context(&program),
            &mut ValueResourceAdmission::snapshot(exact, ValueAdmissionOrigin::InertSnapshot),
            AwbcRuntimeValueSnapshot::view,
        )
        .unwrap();
    assert!(
        saved
            .validate_inputs(
                &context(&program),
                &mut ValueResourceAdmission::snapshot(
                    RuntimeSchemaLimits {
                        max_validation_work: 2,
                        ..exact
                    },
                    ValueAdmissionOrigin::InertSnapshot
                ),
                AwbcRuntimeValueSnapshot::view
            )
            .is_err()
    );
    let mut completed_failure = saved;
    completed_failure.storage.next_operand = 1;
    completed_failure.storage.first_recoverable = Some("failure".into());
    let exact_failure = RuntimeSchemaLimits {
        max_string_bytes: 7,
        max_validation_work: 4,
        ..exact
    };
    completed_failure
        .validate_inputs(
            &context(&program),
            &mut ValueResourceAdmission::snapshot(
                exact_failure,
                ValueAdmissionOrigin::InertSnapshot,
            ),
            AwbcRuntimeValueSnapshot::view,
        )
        .unwrap();
    assert!(
        completed_failure
            .validate_inputs(
                &context(&program),
                &mut ValueResourceAdmission::snapshot(
                    RuntimeSchemaLimits {
                        max_string_bytes: 6,
                        ..exact_failure
                    },
                    ValueAdmissionOrigin::InertSnapshot
                ),
                AwbcRuntimeValueSnapshot::view
            )
            .is_err()
    );
}

#[test]
fn inert_formatter_codec_uses_one_grammar_and_rejects_invalid_progress() {
    let saved: FiberFormatState<AwbcRuntimeValueSnapshot> = FiberFormatState {
        storage: staging(None),
    };
    let bytes = serde_json::to_vec(&saved).unwrap();
    assert_eq!(
        serde_json::from_slice::<FiberFormatState<AwbcRuntimeValueSnapshot>>(&bytes).unwrap(),
        saved
    );
    for progress in [
        FormatOperandProgress::Idle,
        FormatOperandProgress::Evaluating {
            parameter: RuntimeFmtParameterId::Value,
            scope_depth: 0,
            failure: None,
        },
    ] {
        let attempt = FiberFormatAttemptState {
            storage: FormatAttempt {
                attempt: RuntimeFormatAttemptId::from_zero_based(0).unwrap(),
                staging: staging(None::<AwbcRuntimeValueSnapshot>),
                progress,
            },
        };
        let mut json = serde_json::to_value(&attempt).unwrap();
        assert_eq!(
            serde_json::from_value::<FiberFormatAttemptState<AwbcRuntimeValueSnapshot>>(
                json.clone()
            )
            .unwrap(),
            attempt
        );
        json["progress"] =
            serde_json::json!({"Evaluating": {"parameter":255,"scope_depth":0,"failure":null}});
        assert!(
            serde_json::from_value::<FiberFormatAttemptState<AwbcRuntimeValueSnapshot>>(json)
                .unwrap_err()
                .to_string()
                .contains("unknown fmt parameter 255")
        );
        let mut json = serde_json::to_value(&attempt).unwrap();
        json["active_failure"] = serde_json::json!("orphaned failure");
        assert!(
            serde_json::from_value::<FiberFormatAttemptState<AwbcRuntimeValueSnapshot>>(json)
                .is_err()
        );
    }
}

#[test]
fn active_attempt_admission_meters_its_original_completion_walk_and_scope() {
    let mut program = program();
    let attempt = RuntimeFormatAttemptId::from_zero_based(0).unwrap();
    let AwbcInstruction::FormatContent {
        attempt: selected,
        attempt_operands,
        operands,
        ..
    } = &mut program.instructions[0]
    else {
        unreachable!()
    };
    *selected = Some(attempt);
    operands.clear();
    attempt_operands.push(AwbcFormatAttemptOperand {
        parameter: RuntimeFmtParameterId::Value,
        ty: AwbcTypeId(0),
    });
    program
        .instructions
        .push(AwbcInstruction::CompleteFormatOperand {
            attempt,
            parameter: RuntimeFmtParameterId::Value,
            value: AwbcRegisterId(0),
        });
    program.blocks[0].instructions.len = 2;
    let mut saved = FiberFormatAttemptState {
        storage: FormatAttempt {
            attempt,
            staging: staging(None::<AwbcRuntimeValueSnapshot>),
            progress: FormatOperandProgress::Evaluating {
                parameter: RuntimeFmtParameterId::Value,
                scope_depth: 0,
                failure: None,
            },
        },
    };
    let exact = RuntimeSchemaLimits {
        max_nodes: 0,
        max_encoded_bytes: 0,
        max_validation_work: 9,
        ..RuntimeSchemaLimits::engine_default()
    };
    saved
        .validate_inputs(
            &context(&program),
            &mut ValueResourceAdmission::snapshot(exact, ValueAdmissionOrigin::InertSnapshot),
            AwbcRuntimeValueSnapshot::view,
        )
        .unwrap();
    assert!(
        saved
            .validate_inputs(
                &context(&program),
                &mut ValueResourceAdmission::snapshot(
                    RuntimeSchemaLimits {
                        max_validation_work: 8,
                        ..exact
                    },
                    ValueAdmissionOrigin::InertSnapshot
                ),
                AwbcRuntimeValueSnapshot::view
            )
            .is_err()
    );
    saved.storage.progress = FormatOperandProgress::Evaluating {
        parameter: RuntimeFmtParameterId::Value,
        scope_depth: 1,
        failure: None,
    };
    assert!(matches!(
        saved.validate_inputs(
            &context(&program),
            &mut ValueResourceAdmission::snapshot(exact, ValueAdmissionOrigin::InertSnapshot),
            AwbcRuntimeValueSnapshot::view
        ),
        Err(FiberStateError::InvalidFrame)
    ));
}
