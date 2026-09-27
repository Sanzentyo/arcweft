use super::*;
use crate::pattern::RuntimeBuiltinVariantCaseIdentity;

struct ContextFixture {
    program: std::sync::Arc<AwbcProgram>,
    proof: crate::value::RuntimeDialoguePlainTextContextTemplateProof,
}

#[derive(Clone, Copy)]
enum ContextReceiver {
    Result,
    Option,
}

fn context_fixture(receiver: ContextReceiver, defaulted: bool) -> ContextFixture {
    let mut program = minimal_program();
    let content_owner = crate::value::RuntimeDialogueOpaqueRole::Content.exact_owner();
    let error_owner = crate::pattern::runtime_standard_opaque_type(&["ArcError"])
        .and_then(|spec| spec.monomorphic_owner())
        .unwrap();
    program.strings.extend([
        "Ok".to_owned(),
        "Err".to_owned(),
        "Some".to_owned(),
        "None".to_owned(),
        "lazy context".to_owned(),
        content_owner.producer().as_str().to_owned(),
        error_owner.producer().as_str().to_owned(),
    ]);
    program.canonicalize_string_table();
    let string = |value: &str| {
        AwbcStringId(
            u32::try_from(program.strings.binary_search(&value.to_owned()).unwrap()).unwrap(),
        )
    };
    let string_type = AwbcTypeId(0);
    let content_type = AwbcTypeId(1);
    let error_type = AwbcTypeId(2);
    let callback_type = AwbcTypeId(3);
    let tuple_string = AwbcTypeId(4);
    let tuple_error = AwbcTypeId(5);
    let source_result = AwbcTypeId(6);
    let target_result = AwbcTypeId(7);
    program.runtime_types = vec![
        AwbcRuntimeType::new(
            RuntimeCheckedType::String.semantic_identity_digest(),
            AwbcRuntimeTypeShape::String,
        ),
        AwbcRuntimeType::new(
            content_owner.semantic_identity(),
            AwbcRuntimeTypeShape::Opaque {
                producer: string(content_owner.producer().as_str()),
                admission: content_owner.admission(),
                value_class: content_owner.value_class(),
                persistence: content_owner.persistence(),
                arguments: Vec::new(),
            },
        ),
        AwbcRuntimeType::new(
            error_owner.semantic_identity(),
            AwbcRuntimeTypeShape::Opaque {
                producer: string(error_owner.producer().as_str()),
                admission: error_owner.admission(),
                value_class: error_owner.value_class(),
                persistence: error_owner.persistence(),
                arguments: Vec::new(),
            },
        ),
        runtime_type(
            0x31,
            AwbcRuntimeTypeShape::Function {
                contract: RuntimeFunctionTypeContract::default(),
                parameters: Vec::new(),
                result: string_type,
            },
        ),
        runtime_type(0x32, AwbcRuntimeTypeShape::Tuple(vec![string_type])),
        runtime_type(0x33, AwbcRuntimeTypeShape::Tuple(vec![error_type])),
        runtime_type(
            0x34,
            match receiver {
                ContextReceiver::Result => AwbcRuntimeTypeShape::Variant {
                    owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Result),
                    arguments: Vec::new(),
                    cases: vec![
                        AwbcVariantCase {
                            name: string("Ok"),
                            payload: Some(tuple_string),
                        },
                        AwbcVariantCase {
                            name: string("Err"),
                            payload: Some(tuple_string),
                        },
                    ],
                },
                ContextReceiver::Option => AwbcRuntimeTypeShape::Variant {
                    owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Option),
                    arguments: Vec::new(),
                    cases: vec![
                        AwbcVariantCase {
                            name: string("Some"),
                            payload: Some(tuple_string),
                        },
                        AwbcVariantCase {
                            name: string("None"),
                            payload: None,
                        },
                    ],
                },
            },
        ),
        runtime_type(
            0x35,
            AwbcRuntimeTypeShape::Variant {
                owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Result),
                arguments: Vec::new(),
                cases: vec![
                    AwbcVariantCase {
                        name: string("Ok"),
                        payload: Some(tuple_string),
                    },
                    AwbcVariantCase {
                        name: string("Err"),
                        payload: Some(tuple_error),
                    },
                ],
            },
        ),
    ];
    program.signatures[0] = AwbcSignature {
        params: vec![source_result, callback_type],
        result: Some(target_result),
        effects: AwbcEffectSetId(0),
    };
    program.frame_layouts[0].slots = vec![
        AwbcFrameSlot {
            name: None,
            ty: source_result,
            role: AwbcFrameSlotRole::Parameter,
            scope_depth: 0,
        },
        AwbcFrameSlot {
            name: None,
            ty: callback_type,
            role: AwbcFrameSlotRole::Parameter,
            scope_depth: 0,
        },
        AwbcFrameSlot {
            name: None,
            ty: target_result,
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        },
    ];
    program.signatures.push(AwbcSignature {
        params: if defaulted {
            vec![string_type]
        } else {
            Vec::new()
        },
        result: Some(string_type),
        effects: AwbcEffectSetId(0),
    });
    program.frame_layouts.push(AwbcFrameLayout {
        slots: vec![AwbcFrameSlot {
            name: None,
            ty: string_type,
            role: if defaulted {
                AwbcFrameSlotRole::Parameter
            } else {
                AwbcFrameSlotRole::Temporary
            },
            scope_depth: 0,
        }],
        scopes: Vec::new(),
        max_scope_depth: 0,
    });
    program.constants = vec![AwbcConstant::String(string("lazy context"))];
    program.instructions = vec![
        AwbcInstruction::CallIntrinsic {
            dst: Some(AwbcRegisterId(2)),
            intrinsic: AwbcIntrinsicId(0),
            args: vec![AwbcRegisterId(0), AwbcRegisterId(1)],
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(0),
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 1);
    program.blocks[0].terminator = AwbcTerminator::Return {
        value: Some(AwbcRegisterId(2)),
    };
    program.functions[0].flags = AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic);
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(1),
        instructions: AwbcTableRange::new(1, u32::from(!defaulted)),
        terminator: AwbcTerminator::Return {
            value: Some(AwbcRegisterId(0)),
        },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program.functions.push(AwbcFunction {
        public_id: None,
        kind: AwbcFunctionKind::Ordinary,
        signature: AwbcSignatureId(1),
        frame_layout: AwbcFrameLayoutId(1),
        blocks: AwbcTableRange::new(1, 1),
        entry_block: AwbcBlockId(1),
        flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
    });
    if defaulted {
        program.signatures.push(AwbcSignature {
            params: Vec::new(),
            result: Some(string_type),
            effects: AwbcEffectSetId(0),
        });
        program.frame_layouts.push(AwbcFrameLayout {
            slots: vec![AwbcFrameSlot {
                name: None,
                ty: string_type,
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            }],
            scopes: Vec::new(),
            max_scope_depth: 0,
        });
        program.blocks.push(AwbcBlock {
            owner: AwbcFunctionId(2),
            instructions: AwbcTableRange::new(1, 1),
            terminator: AwbcTerminator::Return {
                value: Some(AwbcRegisterId(0)),
            },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        });
        program.functions.push(AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Ordinary,
            signature: AwbcSignatureId(2),
            frame_layout: AwbcFrameLayoutId(2),
            blocks: AwbcTableRange::new(2, 1),
            entry_block: AwbcBlockId(2),
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        });
    }
    program.intrinsics.push(AwbcIntrinsic {
        identity: crate::value::RuntimeCallTarget::intrinsic(match receiver {
            ContextReceiver::Result => crate::value::RuntimeIntrinsic::StdResultWithContext,
            ContextReceiver::Option => crate::value::RuntimeIntrinsic::StdOptionWithContext,
        }),
        signature: AwbcSignatureId(0),
        revision: 1,
    });
    program
        .callable_states
        .push(RuntimeCallableStateDefinition {
            function_type: callback_type,
            origin: RuntimeCallableStateId::from_zero_based(0).unwrap(),
            position: RuntimeCallablePosition::Unapplied,
            retained: Box::new([]),
            parameters: Box::new([]),
            result: string_type,
            attached: if defaulted {
                RuntimeCallableAttachedContract::Defaulted {
                    ty: string_type,
                    default: RuntimeCallableDefault::Body {
                        function: AwbcFunctionId(2),
                        captures: Box::new([]),
                    },
                }
            } else {
                RuntimeCallableAttachedContract::None
            },
            transition: RuntimeCallableTransition::Invoke {
                function: AwbcFunctionId(1),
                captures: Box::new([]),
                arguments: if defaulted {
                    Box::new([RuntimeCallableInputSource::Attached])
                } else {
                    Box::new([])
                },
            },
            partials: Box::new([]),
        });
    let template_id =
        crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0).unwrap();
    let digest = crate::entry::RuntimeDialogueContentTemplateDigest::from_bytes([0x55; 32]);
    let template_ref =
        crate::value::RuntimeDialoguePlainTextContextTemplateRef::from_encoded_identity(
            template_id,
            digest,
        );
    let proof = crate::value::RuntimeDialoguePlainTextContextTemplateProof::try_from_validated_ref(
        template_ref,
        digest,
    )
    .unwrap();
    program.content_templates.push(AwbcDialogueContentTemplate {
        id: template_id,
        digest,
        slots: vec![AwbcDialogueContentSlot {
            slot: crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0).unwrap(),
            role: AwbcDialogueValueRole::Formatted,
            semantic_type: content_type,
        }],
        effects: Vec::new(),
    });
    program.plain_text_context_template = Some(template_ref);
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    ContextFixture {
        program: std::sync::Arc::new(program),
        proof,
    }
}

fn callback(fixture: &ContextFixture) -> RuntimeValue {
    RuntimeValue::Callable(
        crate::value::RuntimeCallableValue::try_new(
            RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&fixture.program)),
            RuntimeCallableStateId::from_zero_based(0).unwrap(),
            [],
        )
        .unwrap(),
    )
}

fn context(fixture: &ContextFixture, with_proof: bool) -> crate::awbc::vm::VmExecutionContext {
    let artifact = crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0x41; 32]).unwrap();
    if with_proof {
        crate::awbc::vm::VmExecutionContext::for_program_with_plain_text_context_proof(
            artifact,
            std::sync::Arc::clone(&fixture.program),
            fixture.proof,
        )
    } else {
        crate::awbc::vm::VmExecutionContext::for_program(
            artifact,
            std::sync::Arc::clone(&fixture.program),
        )
    }
}

fn context_fiber(fixture: &ContextFixture, receiver: RuntimeValue) -> FiberState {
    let mut fiber =
        FiberState::for_function(&fixture.program, AwbcEntryId(0), AwbcFunctionId(0), 1, 64)
            .unwrap();
    fiber
        .active_frame_mut()
        .unwrap()
        .bind_positional_arguments(&fixture.program, &[receiver, callback(fixture)])
        .unwrap();
    fiber
}

fn context_step(
    fixture: &ContextFixture,
    fiber: &mut FiberState,
    with_proof: bool,
) -> crate::awbc::vm::VmStepOutput {
    crate::awbc::vm::step_with_host_context(
        &fixture.program,
        fiber,
        crate::awbc::vm::VmStepOptions {
            max_instructions: 1,
        },
        &context(fixture, with_proof),
        &mut crate::awbc::vm::RejectingVmHost,
    )
    .unwrap()
}

fn context_result(
    fixture: &ContextFixture,
    fiber: &mut FiberState,
    with_proof: bool,
) -> RuntimeValue {
    for _ in 0..16 {
        let output = context_step(fixture, fiber, with_proof);
        match output.exit {
            crate::awbc::vm::VmExit::Running => {}
            crate::awbc::vm::VmExit::Returned(Some(value)) => return value,
            exit => panic!("context callback must return: {exit:?}"),
        }
    }
    panic!("context callback did not return within the instruction bound")
}

fn context_error(value: &RuntimeValue) -> crate::value::RuntimeArcError {
    let (case, Some(error)) = value.clone().try_into_builtin_variant_case().unwrap() else {
        panic!("context returns a Result error")
    };
    assert_eq!(case, RuntimeBuiltinVariantCaseIdentity::ResultErr);
    crate::value::RuntimeArcError::try_from_runtime_value(&error).unwrap()
}

fn assert_plain_text_message(error: &crate::value::RuntimeArcError, expected: &str) {
    let slot = crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0).unwrap();
    let Some(crate::value::RuntimeDialogueContentBinding::Formatted { value, .. }) =
        error.message().binding(slot)
    else {
        panic!("ArcError has a plain-text Content message")
    };
    assert_eq!(
        value.outcome(),
        &crate::value::RuntimeDialogueFormattedOutcome::Success {
            value: crate::value::RuntimeDialogueFormattedSuccess::Text(expected.to_owned()),
            color: None,
        }
    );
}

#[test]
fn verified_lazy_context_success_never_invokes_callback_or_requires_content_proof() {
    for receiver in [ContextReceiver::Result, ContextReceiver::Option] {
        let fixture = context_fixture(receiver, true);
        let value = RuntimeValue::String("success".to_owned());
        let input = match receiver {
            ContextReceiver::Result => RuntimeValue::result_ok(value.clone()),
            ContextReceiver::Option => RuntimeValue::option_some(value.clone()),
        };
        let mut fiber = context_fiber(&fixture, input);
        assert_eq!(
            context_result(&fixture, &mut fiber, false),
            RuntimeValue::result_ok(value)
        );
        assert_eq!(fiber.frames.len(), 1);
    }
}

#[test]
fn verified_option_none_uses_same_fiber_callback_and_has_no_typed_cause() {
    let fixture = context_fixture(ContextReceiver::Option, false);
    let mut fiber = context_fiber(&fixture, RuntimeValue::option_none());
    let error = context_error(&context_result(&fixture, &mut fiber, true));
    assert_plain_text_message(&error, "lazy context");
    assert!(error.source().is_none());
}

#[test]
fn awbc_lazy_context_matches_the_native_arc_error_contract() {
    for receiver in [ContextReceiver::Result, ContextReceiver::Option] {
        for defaulted in [false, true] {
            let fixture = context_fixture(receiver, defaulted);
            let input = match receiver {
                ContextReceiver::Result => {
                    RuntimeValue::result_err(RuntimeValue::String("cause".to_owned()))
                }
                ContextReceiver::Option => RuntimeValue::option_none(),
            };
            let mut fiber = context_fiber(&fixture, input.clone());
            let awbc = context_result(&fixture, &mut fiber, true);
            let message = context_error(&awbc).message().clone();
            let mut calls = 0;
            let mut make_message = || {
                calls += 1;
                Ok(message.clone())
            };
            let native = match receiver {
                ContextReceiver::Result => {
                    crate::value::RuntimeArcError::context_result_value_with(
                        input,
                        &mut make_message,
                        crate::value::RuntimeArcErrorFrame::empty(),
                        crate::entry::RuntimeSchemaLimits::engine_default(),
                    )
                }
                ContextReceiver::Option => {
                    crate::value::RuntimeArcError::context_option_value_with(
                        input,
                        &mut make_message,
                        crate::value::RuntimeArcErrorFrame::empty(),
                        crate::entry::RuntimeSchemaLimits::engine_default(),
                    )
                }
            }
            .unwrap();
            assert_eq!(calls, 1);
            assert_eq!(awbc, native);
        }
    }
}

fn round_trip_context_fiber(fixture: &ContextFixture, fiber: &FiberState) -> FiberState {
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&fixture.program));
    let snapshot = AwbcFiberStateSnapshot::from_live(fiber).unwrap();
    let encoded = serde_json::to_vec(&snapshot).unwrap();
    let snapshot: AwbcFiberStateSnapshot = serde_json::from_slice(&encoded).unwrap();
    let mut restored = snapshot.into_live_for_program(&owner).unwrap();
    restored.validate_for_program(&fixture.program).unwrap();
    restored.resume_budget_yield(&fixture.program).unwrap();
    restored.replenish_budget();
    restored
}

#[test]
fn verified_defaulted_context_callback_restores_both_stages_and_writes_result_once() {
    let fixture = context_fixture(ContextReceiver::Result, true);
    let mut fiber = context_fiber(
        &fixture,
        RuntimeValue::result_err(RuntimeValue::String("cause".to_owned())),
    );
    assert!(matches!(
        context_step(&fixture, &mut fiber, true).exit,
        crate::awbc::vm::VmExit::Running
    ));
    assert_eq!(fiber.frames.len(), 2);
    assert_eq!(fiber.cursor.function, AwbcFunctionId(2));
    assert!(matches!(
        fiber.frames[1].return_to.as_ref().unwrap().continuation,
        FiberReturnContinuation::ContextCallbackDefault { .. }
    ));
    assert!(fiber.frames[0].registers[2].is_none());

    fiber.budget.remaining = 0;
    assert!(matches!(
        context_step(&fixture, &mut fiber, true).exit,
        crate::awbc::vm::VmExit::BudgetYield(_)
    ));
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&fixture.program));
    let snapshot = AwbcFiberStateSnapshot::from_live(&fiber).unwrap();
    let mut forged = snapshot.clone();
    let crate::awbc::fiber::AwbcFiberReturnContinuationSnapshot::ContextCallbackDefault {
        pending,
        ..
    } = &mut forged.frames[1].return_to.as_mut().unwrap().continuation
    else {
        panic!("default stage has a typed continuation")
    };
    *pending = crate::awbc::fiber::AwbcFiberContextPendingSnapshot::OptionNone;
    assert!(
        forged
            .into_live_for_program(&owner)
            .unwrap()
            .validate_for_program(&fixture.program)
            .is_err()
    );

    let mut restored = round_trip_context_fiber(&fixture, &fiber);
    assert!(matches!(
        context_step(&fixture, &mut restored, true).exit,
        crate::awbc::vm::VmExit::Running
    ));
    assert_eq!(restored.cursor.function, AwbcFunctionId(2));
    assert!(matches!(
        context_step(&fixture, &mut restored, true).exit,
        crate::awbc::vm::VmExit::Running
    ));
    assert_eq!(restored.cursor.function, AwbcFunctionId(1));
    assert_eq!(restored.frames.len(), 2);
    assert!(restored.frames[0].registers[2].is_none());
    assert!(matches!(
        restored.frames[1].return_to.as_ref().unwrap().continuation,
        FiberReturnContinuation::ContextCallbackInvoke {
            attached_default: Some(RuntimeValue::String(ref message)),
            ..
        } if message == "lazy context"
    ));

    restored.budget.remaining = 0;
    assert!(matches!(
        context_step(&fixture, &mut restored, true).exit,
        crate::awbc::vm::VmExit::BudgetYield(_)
    ));
    let mut restored = round_trip_context_fiber(&fixture, &restored);
    let error = context_error(&context_result(&fixture, &mut restored, true));
    assert_plain_text_message(&error, "lazy context");
    assert!(matches!(
        error.source(),
        Some(crate::value::RuntimeArcErrorSource::TypedValue(RuntimeValue::String(value)))
            if value == "cause"
    ));
    assert_eq!(restored.frames.len(), 1);
}

fn assert_context_abi_rejected(program: &AwbcProgram) {
    let error = program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("context callback ABI must reject this program");
    assert!(
        error.to_string().contains("context"),
        "unexpected verifier error: {error}"
    );
}

#[test]
fn verifier_rejects_context_callback_effects_and_suspension_in_both_bodies() {
    let fixture = context_fixture(ContextReceiver::Result, true);

    let mut callback_contract = (*fixture.program).clone();
    callback_contract.runtime_types[3] = runtime_type(
        0x31,
        AwbcRuntimeTypeShape::Function {
            contract: RuntimeFunctionTypeContract::monomorphic(
                crate::effect_row::EffectSet::from_labels(["fs.read"]).unwrap(),
            ),
            parameters: Vec::new(),
            result: AwbcTypeId(0),
        },
    );
    assert_context_abi_rejected(&callback_contract);

    for signature in [0, 1, 2] {
        let mut effectful = (*fixture.program).clone();
        let effects = add_effect_set(&mut effectful, &["fs.read"]);
        effectful.signatures[signature].effects = effects;
        effectful.canonicalize_string_table();
        assert_context_abi_rejected(&effectful);
    }

    for function in [1, 2] {
        let mut suspending = (*fixture.program).clone();
        suspending.functions[function].flags = suspending.functions[function]
            .flags
            .with(AwbcFunctionFlag::MaySuspend);
        assert_context_abi_rejected(&suspending);
    }
}

fn formatter_context_fixture() -> ContextFixture {
    let ContextFixture { program, proof } = context_fixture(ContextReceiver::Result, false);
    let mut program = (*program).clone();
    program.strings.push("style".to_owned());
    program.canonicalize_string_table();
    let style = AwbcStringId(
        u32::try_from(program.strings.binary_search(&"style".to_owned()).unwrap()).unwrap(),
    );
    let content_type = AwbcTypeId(1);
    let source_result = AwbcTypeId(6);
    let target_result = AwbcTypeId(7);
    let callback_type = AwbcTypeId(3);
    let string_type = AwbcTypeId(0);
    let int_type = AwbcTypeId(8);
    program.runtime_types.push(AwbcRuntimeType::new(
        RuntimeCheckedType::Signed(crate::value::RuntimeSignedIntWidth::I64)
            .semantic_identity_digest(),
        AwbcRuntimeTypeShape::Int(AwbcSignedIntKind::I64),
    ));
    program.signatures[0].result = Some(content_type);
    program.signatures.extend([
        AwbcSignature {
            params: vec![source_result, callback_type],
            result: Some(string_type),
            effects: AwbcEffectSetId(0),
        },
        AwbcSignature {
            params: Vec::new(),
            result: Some(string_type),
            effects: AwbcEffectSetId(0),
        },
        AwbcSignature {
            params: vec![source_result, callback_type],
            result: Some(target_result),
            effects: AwbcEffectSetId(0),
        },
    ]);
    program.intrinsics[0].signature = AwbcSignatureId(4);
    program.frame_layouts[0].slots = vec![
        AwbcFrameSlot {
            name: None,
            ty: source_result,
            role: AwbcFrameSlotRole::Parameter,
            scope_depth: 0,
        },
        AwbcFrameSlot {
            name: None,
            ty: callback_type,
            role: AwbcFrameSlotRole::Parameter,
            scope_depth: 0,
        },
        AwbcFrameSlot {
            name: None,
            ty: content_type,
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        },
    ];
    program.frame_layouts[1].slots = vec![
        AwbcFrameSlot {
            name: None,
            ty: string_type,
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        },
        AwbcFrameSlot {
            name: None,
            ty: int_type,
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        },
        AwbcFrameSlot {
            name: None,
            ty: int_type,
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        },
        AwbcFrameSlot {
            name: None,
            ty: int_type,
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        },
    ];
    program.frame_layouts.extend([
        AwbcFrameLayout {
            slots: vec![
                AwbcFrameSlot {
                    name: None,
                    ty: source_result,
                    role: AwbcFrameSlotRole::Parameter,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: callback_type,
                    role: AwbcFrameSlotRole::Parameter,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: target_result,
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: string_type,
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
            ],
            scopes: Vec::new(),
            max_scope_depth: 0,
        },
        AwbcFrameLayout {
            slots: vec![AwbcFrameSlot {
                name: None,
                ty: string_type,
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            }],
            scopes: Vec::new(),
            max_scope_depth: 0,
        },
    ]);
    let int_constant = |value: i64| {
        let mut bits = [0; 16];
        bits[..8].copy_from_slice(&value.to_le_bytes());
        AwbcConstant::Int {
            kind: AwbcSignedIntKind::I64,
            bits,
        }
    };
    program.constants.extend([
        int_constant(1),
        int_constant(0),
        AwbcConstant::String(style),
    ]);
    program.instructions = vec![
        AwbcInstruction::FormatContent {
            destination: AwbcRegisterId(2),
            template: crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
                .unwrap(),
            operands: vec![
                AwbcFormatOperand {
                    parameter: crate::value::RuntimeFmtParameterId::Value,
                    function: AwbcFunctionId(2),
                    captures: vec![AwbcRegisterId(0), AwbcRegisterId(1)],
                },
                AwbcFormatOperand {
                    parameter: crate::value::RuntimeFmtParameterId::Style,
                    function: AwbcFunctionId(3),
                    captures: Vec::new(),
                },
            ],
        },
        AwbcInstruction::CallIntrinsic {
            dst: Some(AwbcRegisterId(2)),
            intrinsic: AwbcIntrinsicId(0),
            args: vec![AwbcRegisterId(0), AwbcRegisterId(1)],
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(3),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(1),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(2),
            constant: AwbcConstantId(2),
        },
        AwbcInstruction::Binary {
            dst: AwbcRegisterId(3),
            op: AwbcBinaryOp::Div,
            lhs: AwbcRegisterId(1),
            rhs: AwbcRegisterId(2),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(3),
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 1);
    program.blocks[0].terminator = AwbcTerminator::Return {
        value: Some(AwbcRegisterId(2)),
    };
    program.blocks[1].instructions = AwbcTableRange::new(3, 4);
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(2),
        instructions: AwbcTableRange::new(1, 2),
        terminator: AwbcTerminator::Return {
            value: Some(AwbcRegisterId(3)),
        },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(3),
        instructions: AwbcTableRange::new(7, 1),
        terminator: AwbcTerminator::Return {
            value: Some(AwbcRegisterId(0)),
        },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program.functions.extend([
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Synthetic,
            signature: AwbcSignatureId(2),
            frame_layout: AwbcFrameLayoutId(2),
            blocks: AwbcTableRange::new(2, 1),
            entry_block: AwbcBlockId(2),
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        },
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Synthetic,
            signature: AwbcSignatureId(3),
            frame_layout: AwbcFrameLayoutId(3),
            blocks: AwbcTableRange::new(3, 1),
            entry_block: AwbcBlockId(3),
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        },
    ]);
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    ContextFixture {
        program: std::sync::Arc::new(program),
        proof,
    }
}

#[test]
fn formatter_recovers_context_callback_expression_failure_after_snapshot() {
    let fixture = formatter_context_fixture();
    let mut fiber =
        FiberState::for_function(&fixture.program, AwbcEntryId(0), AwbcFunctionId(0), 1, 64)
            .unwrap();
    fiber
        .active_frame_mut()
        .unwrap()
        .bind_positional_arguments(
            &fixture.program,
            &[
                RuntimeValue::result_err(RuntimeValue::String("cause".to_owned())),
                callback(&fixture),
            ],
        )
        .unwrap();
    for _ in 0..8 {
        if fiber.cursor.function == AwbcFunctionId(1) {
            break;
        }
        assert!(matches!(
            context_step(&fixture, &mut fiber, true).exit,
            crate::awbc::vm::VmExit::Running
        ));
    }
    assert_eq!(fiber.cursor.function, AwbcFunctionId(1));
    assert_eq!(fiber.frames.len(), 3);
    fiber.budget.remaining = 0;
    assert!(matches!(
        context_step(&fixture, &mut fiber, true).exit,
        crate::awbc::vm::VmExit::BudgetYield(_)
    ));
    let mut restored = round_trip_context_fiber(&fixture, &fiber);
    let mut style_instructions = 0;
    let content = loop {
        let output = context_step(&fixture, &mut restored, true);
        style_instructions += output
            .observations
            .iter()
            .filter(|observation| {
                matches!(
                    observation,
                    crate::awbc::vm::VmObservation::Instruction {
                        function: AwbcFunctionId(3),
                        ..
                    }
                )
            })
            .count();
        match output.exit {
            crate::awbc::vm::VmExit::Running => {}
            crate::awbc::vm::VmExit::Returned(Some(content)) => {
                break crate::value::RuntimeDialogueContentValue::try_from_runtime_value(&content)
                    .unwrap();
            }
            exit => panic!("formatter must recover callback expression error: {exit:?}"),
        }
    };
    assert_eq!(style_instructions, 1);
    let slot = crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0).unwrap();
    let Some(crate::value::RuntimeDialogueContentBinding::Formatted { value, .. }) =
        content.binding(slot)
    else {
        panic!("formatter returns a Formatted Content slot")
    };
    assert!(matches!(
        value.outcome(),
        crate::value::RuntimeDialogueFormattedOutcome::Failure { reason, .. }
            if reason.contains("division by zero")
    ));
}

#[test]
fn formatter_does_not_recover_context_message_proof_failure() {
    let mut fixture = formatter_context_fixture();
    let mut program = (*fixture.program).clone();
    program.constants[2] = program.constants[1].clone();
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    fixture.program = std::sync::Arc::new(program);
    let mut fiber =
        FiberState::for_function(&fixture.program, AwbcEntryId(0), AwbcFunctionId(0), 1, 64)
            .unwrap();
    fiber
        .active_frame_mut()
        .unwrap()
        .bind_positional_arguments(
            &fixture.program,
            &[
                RuntimeValue::result_err(RuntimeValue::String("cause".to_owned())),
                callback(&fixture),
            ],
        )
        .unwrap();
    let mut style_instructions = 0;
    let trap = loop {
        let output = context_step(&fixture, &mut fiber, false);
        style_instructions += output
            .observations
            .iter()
            .filter(|observation| {
                matches!(
                    observation,
                    crate::awbc::vm::VmObservation::Instruction {
                        function: AwbcFunctionId(3),
                        ..
                    }
                )
            })
            .count();
        match output.exit {
            crate::awbc::vm::VmExit::Running => {}
            crate::awbc::vm::VmExit::Trapped(trap) => break trap,
            exit => panic!("missing Content proof must be fatal: {exit:?}"),
        }
    };
    assert_eq!(trap.code, AwbcTrapCode::InternalInvariant);
    assert_eq!(style_instructions, 0);
    assert_eq!(fiber.frames[0].format.as_ref().unwrap().next_operand(), 0);
}

#[test]
fn verified_lazy_context_callback_survives_budget_snapshot_without_replay() {
    let fixture = context_fixture(ContextReceiver::Result, false);
    let mut fiber =
        FiberState::for_function(&fixture.program, AwbcEntryId(0), AwbcFunctionId(0), 1, 64)
            .unwrap();
    fiber
        .active_frame_mut()
        .unwrap()
        .bind_positional_arguments(
            &fixture.program,
            &[
                RuntimeValue::result_err(RuntimeValue::String("cause".to_owned())),
                callback(&fixture),
            ],
        )
        .unwrap();
    let context = crate::awbc::vm::VmExecutionContext::for_program_with_plain_text_context_proof(
        crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0x41; 32]).unwrap(),
        std::sync::Arc::clone(&fixture.program),
        fixture.proof,
    );
    let output = crate::awbc::vm::step_with_host_context(
        &fixture.program,
        &mut fiber,
        crate::awbc::vm::VmStepOptions {
            max_instructions: 1,
        },
        &context,
        &mut crate::awbc::vm::RejectingVmHost,
    )
    .unwrap();
    assert!(matches!(output.exit, crate::awbc::vm::VmExit::Running));
    assert_eq!(fiber.frames.len(), 2);
    fiber.budget.remaining = 0;
    assert!(matches!(
        crate::awbc::vm::step_with_host_context(
            &fixture.program,
            &mut fiber,
            crate::awbc::vm::VmStepOptions {
                max_instructions: 1
            },
            &context,
            &mut crate::awbc::vm::RejectingVmHost
        )
        .unwrap()
        .exit,
        crate::awbc::vm::VmExit::BudgetYield(_)
    ));
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&fixture.program));
    let snapshot = AwbcFiberStateSnapshot::from_live(&fiber).unwrap();
    let encoded = serde_json::to_vec(&snapshot).unwrap();
    let snapshot: AwbcFiberStateSnapshot = serde_json::from_slice(&encoded).unwrap();
    let mut restored = snapshot.into_live_for_program(&owner).unwrap();
    restored.validate_for_program(&fixture.program).unwrap();
    restored.resume_budget_yield(&fixture.program).unwrap();
    restored.replenish_budget();
    let output = loop {
        let output = crate::awbc::vm::step_with_host_context(
            &fixture.program,
            &mut restored,
            crate::awbc::vm::VmStepOptions {
                max_instructions: 1,
            },
            &context,
            &mut crate::awbc::vm::RejectingVmHost,
        )
        .unwrap();
        if !matches!(output.exit, crate::awbc::vm::VmExit::Running) {
            break output;
        }
    };
    let crate::awbc::vm::VmExit::Returned(Some(value)) = output.exit else {
        panic!("context returns")
    };
    let (case, Some(error)) = value.try_into_builtin_variant_case().unwrap() else {
        panic!("result error")
    };
    assert_eq!(case, RuntimeBuiltinVariantCaseIdentity::ResultErr);
    let error = crate::value::RuntimeArcError::try_from_runtime_value(&error).unwrap();
    assert!(
        matches!(error.source(), Some(crate::value::RuntimeArcErrorSource::TypedValue(RuntimeValue::String(cause_text))) if cause_text == "cause")
    );
}
