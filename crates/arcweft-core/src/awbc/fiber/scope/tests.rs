use super::*;
use crate::awbc::schema::*;
use crate::entry::schema::value_encoding::ValueAdmissionOrigin;
use crate::entry::{RuntimeSchemaError, RuntimeSchemaLimits};
use crate::pattern::RuntimeCheckedType;

fn program() -> AwbcProgram {
    let site = RuntimeDeferSiteId::from_zero_based(0).unwrap();
    AwbcProgram {
        strings: vec!["key".to_owned()],
        runtime_types: vec![
            AwbcRuntimeType::new(
                RuntimeCheckedType::String.semantic_identity_digest(),
                AwbcRuntimeTypeShape::String,
            ),
            AwbcRuntimeType::new(
                RuntimeCheckedType::Unit.semantic_identity_digest(),
                AwbcRuntimeTypeShape::Unit,
            ),
        ],
        signatures: vec![AwbcSignature {
            params: vec![AwbcTypeId(0)],
            result: Some(AwbcTypeId(1)),
            effects: AwbcEffectSetId(0),
        }],
        functions: vec![AwbcFunction {
            definition: crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                *blake3::hash(b"arcweft.scope-input-test.definition.v1\0").as_bytes(),
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
            slots: vec![AwbcFrameSlot {
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Local,
                name: None,
                scope_depth: 0,
            }],
            scopes: vec![AwbcScopeDefinition {
                kind: crate::scope::RuntimeScopeFrameKind::EmittedLexical,
                parent: None,
                identity: crate::scope::RuntimeScopeIdentity::Anonymous,
            }],
            max_scope_depth: 1,
        }],
        blocks: vec![AwbcBlock {
            owner: AwbcFunctionId(0),
            instructions: AwbcTableRange { start: 0, len: 2 },
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        }],
        instructions: vec![
            AwbcInstruction::RegisterDefer {
                site,
                outcome: crate::line_task::RuntimeDeferOutcomeFilter::Always,
                owner: AwbcDeferOwner::CurrentScope,
                captures: vec![AwbcRegisterId(0)],
            },
            AwbcInstruction::RegisterCleanup {
                key: AwbcStringId(0),
                effect: AwbcEffectPlanId(0),
                args: vec![AwbcRegisterId(0)],
            },
        ],
        defer_sites: vec![AwbcFunctionId(0)],
        effect_plans: vec![AwbcEffectPlan {
            signature: AwbcSignatureId(0),
            capability: None,
            payload: crate::awbc::schema::AwbcEffectPayload::Log(vec![]),
            resources: vec![],
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
        scope_count: 1,
    }
}

fn cleanup<V>(value: V) -> FiberScopeCleanup<V> {
    FiberScopeCleanup {
        source: FiberCursor {
            function: AwbcFunctionId(0),
            block: AwbcBlockId(0),
            instruction_offset: 1,
        },
        args: vec![value],
    }
}

fn deferred<V>(value: V) -> FiberDeferredRegistration<V> {
    FiberDeferredRegistration {
        id: RuntimeDeferRegistrationId::from_allocated(std::num::NonZeroU64::new(1).unwrap()),
        site: RuntimeDeferSiteId::from_zero_based(0).unwrap(),
        source: FiberCursor {
            function: AwbcFunctionId(0),
            block: AwbcBlockId(0),
            instruction_offset: 0,
        },
        outcome: crate::line_task::RuntimeDeferOutcomeFilter::Always,
        capture_registers: vec![AwbcRegisterId(0)],
        captures: vec![value],
    }
}

fn scope<V, G>() -> FiberScope<V, G> {
    FiberScope {
        id: AwbcScopeId(0),
        depth: 0,
        presentation_scope: None,
        cleanups: vec![],
        defers: vec![],
        defer_releasing: vec![],
        defer_exit: None,
        defer_inflight: None,
        defer_failure: None,
    }
}

#[test]
fn cleanup_live_and_inert_use_one_exact_value_budget_and_original_signature() {
    let mut program = program();
    let live = cleanup(RuntimeValue::String("cause".into()));
    let inert = cleanup(AwbcRuntimeValueSnapshot::String("cause".into()));
    let exact = RuntimeSchemaLimits {
        max_nodes: 1,
        max_encoded_bytes: 7,
        max_string_bytes: 5,
        max_validation_work: 12,
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
                ValueAdmissionOrigin::InertSnapshot => inert.validate_inputs(
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
                max_validation_work: 11,
                ..exact
            },
        ] {
            assert!(validate(short).is_err(), "one-over policy {short:?}");
        }
        let mut resources = ValueResourceAdmission::snapshot(exact, origin);
        inert
            .validate_inputs(
                &context(&program),
                &mut resources,
                AwbcRuntimeValueSnapshot::view,
            )
            .unwrap();
        assert!(
            matches!(inert.validate_inputs(&context(&program), &mut resources, AwbcRuntimeValueSnapshot::view),
            Err(FiberStateError::Snapshot(crate::value::AwbcRuntimeValueSnapshotError::Resource(error)))
            if matches!(error, RuntimeSchemaError::ValidationWork { limit: 12, consumed: 13, .. }))
        );
    }
    // A forged row with the same semantic identity still uses the original
    // signature's concrete type ID, rather than a semantic-ID resolver.
    program.runtime_types.push(AwbcRuntimeType::new(
        program.runtime_types[0].semantic_identity(),
        AwbcRuntimeTypeShape::Bool,
    ));
    for slot in [false, true] {
        program.frame_layouts[0].slots[0].ty = AwbcTypeId(if slot { 2 } else { 0 });
        program.signatures[0].params[0] = AwbcTypeId(if slot { 0 } else { 2 });
        assert!(matches!(
            inert.validate_inputs(
                &context(&program),
                &mut ValueResourceAdmission::snapshot(
                    RuntimeSchemaLimits::engine_default(),
                    ValueAdmissionOrigin::InertSnapshot
                ),
                AwbcRuntimeValueSnapshot::view
            ),
            Err(FiberStateError::Snapshot(
                crate::value::AwbcRuntimeValueSnapshotError::ProgramType(_)
            ))
        ));
        assert!(matches!(
            live.validate_inputs(
                &context(&program),
                &mut ValueResourceAdmission::live(RuntimeSchemaLimits::engine_default()),
                RuntimeValue::view
            ),
            Err(FiberStateError::Snapshot(
                crate::value::AwbcRuntimeValueSnapshotError::ProgramType(_)
            ))
        ));
    }
}

#[test]
fn defer_correlates_reached_instruction_capture_rows_and_cumulative_resources() {
    let mut program = program();
    let live = deferred(RuntimeValue::String("cause".into()));
    let inert = deferred(AwbcRuntimeValueSnapshot::String("cause".into()));
    let exact = RuntimeSchemaLimits {
        max_nodes: 1,
        max_encoded_bytes: 7,
        max_string_bytes: 5,
        max_validation_work: 8,
        ..RuntimeSchemaLimits::engine_default()
    };
    let mut resources = ValueResourceAdmission::snapshot(exact, ValueAdmissionOrigin::Live);
    live.validate_inputs(&context(&program), &mut resources, RuntimeValue::view)
        .unwrap();
    let mut resources =
        ValueResourceAdmission::snapshot(exact, ValueAdmissionOrigin::InertSnapshot);
    inert
        .validate_inputs(
            &context(&program),
            &mut resources,
            AwbcRuntimeValueSnapshot::view,
        )
        .unwrap();
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
            max_validation_work: 7,
            ..exact
        },
    ] {
        assert!(
            inert
                .validate_inputs(
                    &context(&program),
                    &mut ValueResourceAdmission::snapshot(
                        short,
                        ValueAdmissionOrigin::InertSnapshot
                    ),
                    AwbcRuntimeValueSnapshot::view
                )
                .is_err(),
            "one-over policy {short:?}"
        );
    }
    assert!(
        inert
            .validate_inputs(
                &context(&program),
                &mut resources,
                AwbcRuntimeValueSnapshot::view
            )
            .is_err()
    );
    for change in 0..3 {
        let mut forged = inert.clone();
        match change {
            0 => forged.source.instruction_offset = 1,
            1 => forged.capture_registers[0] = AwbcRegisterId(1),
            _ => forged.outcome = crate::line_task::RuntimeDeferOutcomeFilter::Cancelled,
        }
        assert!(
            forged
                .validate_inputs(
                    &context(&program),
                    &mut ValueResourceAdmission::snapshot(
                        RuntimeSchemaLimits::engine_default(),
                        ValueAdmissionOrigin::InertSnapshot
                    ),
                    AwbcRuntimeValueSnapshot::view
                )
                .is_err()
        );
    }
    program.runtime_types.push(AwbcRuntimeType::new(
        program.runtime_types[0].semantic_identity(),
        AwbcRuntimeTypeShape::Bool,
    ));
    program.frame_layouts[0].slots[0].ty = AwbcTypeId(2);
    assert!(
        inert
            .validate_inputs(
                &context(&program),
                &mut ValueResourceAdmission::snapshot(
                    RuntimeSchemaLimits::engine_default(),
                    ValueAdmissionOrigin::InertSnapshot
                ),
                AwbcRuntimeValueSnapshot::view
            )
            .is_err()
    );
}

#[test]
fn empty_scope_consumes_metadata_work_without_synthetic_values() {
    let program = program();
    let packet =
        scope::<AwbcRuntimeValueSnapshot, crate::presentation::RuntimePresentationScopeBinding>();
    let exact = RuntimeSchemaLimits {
        max_nodes: 0,
        max_encoded_bytes: 0,
        max_validation_work: 1,
        ..RuntimeSchemaLimits::engine_default()
    };
    packet
        .validate_inputs(
            &context(&program),
            0,
            None,
            &mut BTreeSet::new(),
            &mut ValueResourceAdmission::snapshot(exact, ValueAdmissionOrigin::InertSnapshot),
            AwbcRuntimeValueSnapshot::view,
        )
        .unwrap();
    assert!(
        packet
            .validate_inputs(
                &context(&program),
                0,
                None,
                &mut BTreeSet::new(),
                &mut ValueResourceAdmission::snapshot(
                    RuntimeSchemaLimits {
                        max_validation_work: 0,
                        ..exact
                    },
                    ValueAdmissionOrigin::InertSnapshot
                ),
                AwbcRuntimeValueSnapshot::view
            )
            .is_err()
    );
    let mut forged = packet;
    forged.defer_failure = Some(FiberTrap {
        code: AwbcTrapCode::InternalInvariant,
        message: Some("failure".into()),
        source_map: None,
    });
    assert!(
        forged
            .validate_inputs(
                &context(&program),
                0,
                None,
                &mut BTreeSet::new(),
                &mut ValueResourceAdmission::snapshot(
                    RuntimeSchemaLimits::engine_default(),
                    ValueAdmissionOrigin::InertSnapshot
                ),
                AwbcRuntimeValueSnapshot::view
            )
            .is_err()
    );
}

#[test]
fn inert_scope_codec_has_one_grammar_and_requires_defer_source() {
    let mut packet = scope::<_, crate::presentation::RuntimePresentationScopeSnapshot>();
    packet
        .cleanups
        .push(cleanup(AwbcRuntimeValueSnapshot::String("cleanup".into())));
    packet
        .defers
        .push(deferred(AwbcRuntimeValueSnapshot::String("capture".into())));
    let bytes = serde_json::to_vec(&packet).unwrap();
    assert_eq!(
        serde_json::from_slice::<crate::awbc::fiber::AwbcFiberScopeSnapshot>(&bytes).unwrap(),
        packet
    );
    let mut json = serde_json::to_value(&packet).unwrap();
    json["defers"][0].as_object_mut().unwrap().remove("source");
    assert!(serde_json::from_value::<crate::awbc::fiber::AwbcFiberScopeSnapshot>(json).is_err());
    for field in ["key", "effect"] {
        let mut json = serde_json::to_value(&packet).unwrap();
        json["cleanups"][0][field] = serde_json::json!(0);
        assert!(
            serde_json::from_value::<crate::awbc::fiber::AwbcFiberScopeSnapshot>(json).is_err()
        );
    }
    let mut json = serde_json::to_value(&packet).unwrap();
    json["cleanups"][0]
        .as_object_mut()
        .unwrap()
        .remove("source");
    assert!(serde_json::from_value::<crate::awbc::fiber::AwbcFiberScopeSnapshot>(json).is_err());
    let mut json = serde_json::to_value(&packet).unwrap();
    json["old_live_values"] = serde_json::json!([]);
    assert!(serde_json::from_value::<crate::awbc::fiber::AwbcFiberScopeSnapshot>(json).is_err());
}

#[test]
fn cleanup_requires_its_exact_original_instruction_function_and_argument_inventory() {
    let program = program();
    let packet = cleanup(AwbcRuntimeValueSnapshot::String("cause".into()));
    for change in 0..6 {
        let mut forged = packet.clone();
        match change {
            0 => forged.source.function = AwbcFunctionId(1),
            1 => forged.source.block = AwbcBlockId(1),
            2 => forged.source.instruction_offset = 0,
            3 => forged.source.instruction_offset = 2,
            4 => forged.args.clear(),
            _ => forged
                .args
                .push(AwbcRuntimeValueSnapshot::String("extra".into())),
        }
        assert!(
            forged
                .validate_inputs(
                    &context(&program),
                    &mut ValueResourceAdmission::snapshot(
                        RuntimeSchemaLimits::engine_default(),
                        ValueAdmissionOrigin::InertSnapshot
                    ),
                    AwbcRuntimeValueSnapshot::view
                )
                .is_err(),
            "forgery {change}"
        );
    }
}

#[test]
fn cleanup_key_cancellation_and_terminal_unwind_reject_later_packets_before_mutation() {
    let mut program = program();
    program.frame_layouts[0].scopes.clear();
    let mut fiber = super::super::FiberState::for_function(
        &program,
        super::super::AwbcFiberRoot::Function(AwbcFunctionId(0)),
        AwbcFunctionId(0),
        1,
        64,
    )
    .unwrap();
    let good = cleanup(RuntimeValue::String("first".into()));
    let mut bad = cleanup(RuntimeValue::String("later".into()));
    bad.source.instruction_offset = 0;
    fiber.frames[0].root_cleanups = vec![good.clone(), bad.clone()];
    let cursor = fiber.cursor;
    let status = fiber.status;
    assert!(
        fiber.frames[0]
            .cancel_cleanup_by_key(&program, "key")
            .is_err()
    );
    assert_eq!(
        fiber.frames[0].root_cleanups,
        vec![good.clone(), bad.clone()]
    );
    assert!(crate::awbc::vm::cancel_fiber(&program, &mut fiber).is_err());
    assert_eq!(fiber.frames[0].root_cleanups, vec![good.clone(), bad]);
    assert_eq!(fiber.cursor, cursor);
    assert_eq!(fiber.status, status);
    assert!(fiber.terminal.is_none());
    fiber.frames[0].root_cleanups.pop();
    let cancelled = crate::awbc::vm::cancel_fiber(&program, &mut fiber).unwrap();
    assert_eq!(cancelled.exit, crate::awbc::vm::VmExit::Cancelled);
    assert!(
        matches!(&cancelled.observations[..], [crate::awbc::vm::VmObservation::Effect { effect: AwbcEffectPlanId(0), args }]
        if args == &good.args)
    );
    assert!(fiber.frames[0].root_cleanups.is_empty());
    assert!(
        crate::awbc::vm::cancel_fiber(&program, &mut fiber)
            .unwrap()
            .observations
            .is_empty()
    );
}

#[test]
fn invalid_cleanup_precedes_return_value_transfer() {
    let mut program = program();
    program.frame_layouts[0].scopes.clear();
    program.signatures[0].result = Some(AwbcTypeId(0));
    program.blocks[0].terminator = AwbcTerminator::Return {
        value: Some(AwbcRegisterId(0)),
    };
    let mut fiber = super::super::FiberState::for_function(
        &program,
        super::super::AwbcFiberRoot::Function(AwbcFunctionId(0)),
        AwbcFunctionId(0),
        1,
        64,
    )
    .unwrap();
    fiber.cursor.instruction_offset = program.blocks[0].instructions.len;
    fiber.frames[0]
        .set_register(AwbcRegisterId(0), RuntimeValue::String("return".into()))
        .unwrap();
    let mut forged = cleanup(RuntimeValue::String("cleanup".into()));
    forged.source.instruction_offset = 0;
    fiber.frames[0].root_cleanups.push(forged.clone());
    assert!(
        crate::awbc::vm::step(
            &program,
            &mut fiber,
            crate::awbc::vm::VmStepOptions {
                max_instructions: 1
            }
        )
        .is_err()
    );
    assert_eq!(
        fiber.frames[0].register(AwbcRegisterId(0)).unwrap(),
        &RuntimeValue::String("return".into())
    );
    assert_eq!(fiber.frames[0].root_cleanups, vec![forged]);
    assert!(fiber.terminal.is_none());
}

#[test]
fn register_transfer_prepares_copy_duplicates_and_rejects_bad_later_inputs_atomically() {
    let program = program();
    let mut frame = super::super::FiberFrame::new(
        crate::runtime_id::RuntimeFrameInstanceId::from_allocated(std::num::NonZeroU64::MIN),
        &program,
        AwbcFunctionId(0),
        None,
    )
    .unwrap();
    frame
        .set_register(AwbcRegisterId(0), RuntimeValue::String("capture".into()))
        .unwrap();
    assert!(
        frame
            .take_register_values(&program, &[AwbcRegisterId(0), AwbcRegisterId(1)])
            .is_err()
    );
    assert_eq!(
        frame.register(AwbcRegisterId(0)).unwrap(),
        &RuntimeValue::String("capture".into())
    );
    let mut forged = program.clone();
    forged.runtime_types.push(AwbcRuntimeType::new(
        forged.runtime_types[0].semantic_identity(),
        AwbcRuntimeTypeShape::Bool,
    ));
    forged.frame_layouts[0].slots[0].ty = AwbcTypeId(2);
    assert!(matches!(
        frame.take_register_values(&forged, &[AwbcRegisterId(0)]),
        Err(FiberStateError::Snapshot(
            crate::value::AwbcRuntimeValueSnapshotError::ProgramType(_)
        ))
    ));
    assert_eq!(
        frame.register(AwbcRegisterId(0)).unwrap(),
        &RuntimeValue::String("capture".into())
    );
    let transferred = frame
        .take_register_values(&program, &[AwbcRegisterId(0); 3])
        .unwrap();
    assert_eq!(transferred, vec![RuntimeValue::String("capture".into()); 3]);
    assert!(frame.registers[0].is_vacant());
}

#[test]
fn register_transfer_rejects_affine_duplicates_before_moving_the_source() {
    let mut program = program();
    program.runtime_types.push(AwbcRuntimeType::new(
        crate::pattern::RuntimeSemanticTypeId::from_bytes([0x68; 32]),
        AwbcRuntimeTypeShape::Need(AwbcTypeId(0)),
    ));
    program.frame_layouts[0].slots[0].ty = AwbcTypeId(2);
    let mut frame = super::super::FiberFrame::new(
        crate::runtime_id::RuntimeFrameInstanceId::from_allocated(std::num::NonZeroU64::MIN),
        &program,
        AwbcFunctionId(0),
        None,
    )
    .unwrap();
    frame
        .set_register(
            AwbcRegisterId(0),
            RuntimeValue::NeedHandle(crate::tests::reusable_need("need.capture")),
        )
        .unwrap();
    assert!(matches!(
        frame.take_register_values(&program, &[AwbcRegisterId(0); 2]),
        Err(FiberStateError::Snapshot(
            crate::value::AwbcRuntimeValueSnapshotError::AffineValue
        ))
    ));
    assert!(matches!(
        frame.register(AwbcRegisterId(0)),
        Ok(RuntimeValue::NeedHandle(_))
    ));
    let transferred = frame
        .take_register_values(&program, &[AwbcRegisterId(0)])
        .unwrap();
    assert!(matches!(
        transferred.as_slice(),
        [RuntimeValue::NeedHandle(_)]
    ));
    assert!(frame.registers[0].is_vacant());
}
