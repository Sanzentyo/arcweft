use super::codec::{AwbcCodecError, AwbcDecodeBudget};
use super::fiber::{
    FiberAwaitTarget, FiberResumeTarget, FiberScope, FiberScopeCleanup, FiberState, FiberStatus,
    FiberSuspension, FiberSuspensionReason, FiberTrap,
};
use super::schema::*;
use super::verify::{AwbcVerifyBudget, AwbcVerifyContext, AwbcVerifyError};
use crate::effect::RuntimeAssertionGuardId;
use crate::entry::{FlowContractHash, RuntimeFlowExecutable};
use crate::plan::{FlowRuntimeId, RuntimeFlowTargetError};
use crate::value::{RuntimeFunctionValue, RuntimeValue};

fn test_flow_binding(label: &str, function: u32) -> AwbcFlowBinding {
    AwbcFlowBinding {
        flow: FlowRuntimeId::canonical(label).expect("test Flow ID is valid"),
        function: AwbcFunctionId(function),
    }
}

fn minimal_program() -> AwbcProgram {
    AwbcProgram {
        strings: vec!["main".to_owned()],
        signatures: vec![AwbcSignature {
            params: Vec::new(),
            result: None,
            effects: AwbcEffectSetId(0),
        }],
        frame_layouts: vec![AwbcFrameLayout {
            slots: Vec::new(),
            max_scope_depth: 0,
        }],
        functions: vec![AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 1),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
        }],
        blocks: vec![AwbcBlock {
            owner: AwbcFunctionId(0),
            instructions: AwbcTableRange::new(0, 0),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::FlowEntry,
            source_map: None,
        }],
        flow_bindings: vec![test_flow_binding("main", 0)],
        entries: vec![AwbcEntry {
            runtime_id: crate::plan::EntryRuntimeId::canonical("main")
                .expect("test entry runtime ID is valid"),
            binding: crate::entry::EntryBindingIdentity::from_bytes([1; 32]),
            public_id: AwbcStringId(0),
            kind: AwbcEntryKind::Cli,
            signature: AwbcSignatureId(0),
            target: AwbcEntryTarget::Function(AwbcFunctionId(0)),
            roles: crate::entry::RuntimeEntryRoles::None,
        }],
        ..AwbcProgram::default()
    }
}

#[test]
fn fiber_snapshot_rejects_stale_functions_in_cleanup_arguments() {
    let mut program = minimal_program();
    program
        .strings
        .extend(["zz.debug".to_owned(), "zz.message".to_owned()]);
    program.constants.extend([
        AwbcConstant::String(AwbcStringId(1)),
        AwbcConstant::String(AwbcStringId(2)),
    ]);
    let dynamic_type = AwbcTypeId(
        u32::try_from(program.runtime_types.len()).expect("test type table fits AWBC index"),
    );
    program.runtime_types.push(AwbcRuntimeType::Dynamic);
    program.signatures.push(AwbcSignature {
        params: vec![dynamic_type],
        result: None,
        effects: AwbcEffectSetId(0),
    });
    program.effect_plans.push(AwbcEffectPlan {
        kind: AwbcEffectKind::Log,
        signature: AwbcSignatureId(1),
        capability: None,
        audio: None,
        static_args: vec![AwbcConstantId(0), AwbcConstantId(1)],
        resources: Vec::new(),
    });
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64)
        .expect("create snapshot validation fiber");
    fiber
        .active_frame_mut()
        .expect("active frame")
        .root_cleanups
        .push(FiberScopeCleanup {
            key: "cleanup.stale-function".to_owned(),
            effect: AwbcEffectPlanId(0),
            args: vec![RuntimeValue::Function(RuntimeFunctionValue::new_awbc(
                Vec::new(),
                AwbcFunctionId(u32::MAX),
                Vec::new(),
            ))],
        });

    let error = fiber
        .validate_for_program(&program)
        .expect_err("stale cleanup function must reject");
    assert!(
        matches!(
        &error,
        super::fiber::FiberStateError::InvalidRuntimeValue { path, reason }
            if path == "frames[0].root_cleanups[0].args[0]"
                && reason.contains("does not exist")
        ),
        "{error:?}"
    );
}

fn expression_apply_frame_layouts() -> Vec<AwbcFrameLayout> {
    vec![
        AwbcFrameLayout {
            slots: vec![
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(1),
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(0),
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
            ],
            max_scope_depth: 0,
        },
        AwbcFrameLayout {
            slots: vec![AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            }],
            max_scope_depth: 0,
        },
    ]
}

fn expression_apply_functions(synthetic_len: u32) -> Vec<AwbcFunction> {
    vec![
        AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 1),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
        },
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Synthetic,
            signature: AwbcSignatureId(1),
            frame_layout: AwbcFrameLayoutId(1),
            blocks: AwbcTableRange::new(1, synthetic_len),
            entry_block: AwbcBlockId(1),
            flags: AwbcFunctionFlags(
                AwbcFunctionFlags::DETERMINISTIC | AwbcFunctionFlags::MAY_SUSPEND,
            ),
        },
    ]
}

fn expression_apply_program(
    synthetic_entry_instructions: Vec<AwbcInstruction>,
    synthetic_blocks: Vec<(AwbcTerminator, AwbcSafePointKind)>,
    resume_points: Vec<AwbcResumePoint>,
) -> AwbcProgram {
    let synthetic_len =
        u32::try_from(synthetic_blocks.len()).expect("test block count fits in AWBC range");
    let synthetic_instruction_len = u32::try_from(synthetic_entry_instructions.len())
        .expect("test instruction count fits in AWBC range");
    let mut blocks = Vec::with_capacity(synthetic_blocks.len() + 1);
    blocks.push(AwbcBlock {
        owner: AwbcFunctionId(0),
        instructions: AwbcTableRange::new(0, 2),
        terminator: AwbcTerminator::Return {
            value: Some(AwbcRegisterId(1)),
        },
        safe_point: AwbcSafePointKind::FlowEntry,
        source_map: None,
    });
    blocks.extend(synthetic_blocks.into_iter().enumerate().map(
        |(index, (terminator, safe_point))| AwbcBlock {
            owner: AwbcFunctionId(1),
            instructions: AwbcTableRange::new(
                2,
                if index == 0 {
                    synthetic_instruction_len
                } else {
                    0
                },
            ),
            terminator,
            safe_point,
            source_map: None,
        },
    ));

    AwbcProgram {
        strings: vec!["main".to_owned()],
        constants: vec![AwbcConstant::Unit],
        signatures: vec![
            AwbcSignature {
                params: Vec::new(),
                result: Some(AwbcTypeId(0)),
                effects: AwbcEffectSetId(0),
            },
            AwbcSignature {
                params: Vec::new(),
                result: None,
                effects: AwbcEffectSetId(0),
            },
        ],
        frame_layouts: expression_apply_frame_layouts(),
        functions: expression_apply_functions(synthetic_len),
        blocks,
        instructions: {
            let mut instructions = vec![
                AwbcInstruction::MakeFunction {
                    dst: AwbcRegisterId(0),
                    function: AwbcFunctionId(1),
                    params: Vec::new(),
                    capture_names: Vec::new(),
                    captures: Vec::new(),
                },
                AwbcInstruction::ApplyFunction {
                    dst: AwbcRegisterId(1),
                    callee: AwbcRegisterId(0),
                    args: Vec::new(),
                },
            ];
            instructions.extend(synthetic_entry_instructions);
            instructions
        },
        resume_points,
        flow_bindings: vec![test_flow_binding("main", 0)],
        entries: vec![AwbcEntry {
            runtime_id: crate::plan::EntryRuntimeId::canonical("main")
                .expect("test entry runtime ID is valid"),
            binding: crate::entry::EntryBindingIdentity::from_bytes([1; 32]),
            public_id: AwbcStringId(0),
            kind: AwbcEntryKind::Cli,
            signature: AwbcSignatureId(0),
            target: AwbcEntryTarget::Function(AwbcFunctionId(0)),
            roles: crate::entry::RuntimeEntryRoles::None,
        }],
        ..AwbcProgram::default()
    }
}

#[test]
fn canonical_codec_is_deterministic_and_round_trips() {
    let program = minimal_program();
    let first = program.encode_canonical().expect("encode AWBC");
    let second = program.encode_canonical().expect("encode AWBC again");
    assert_eq!(first, second);
    let decoded = AwbcProgram::decode_canonical(&first, AwbcDecodeBudget::default())
        .expect("decode canonical AWBC");
    assert_eq!(decoded, program);
    decoded
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify decoded AWBC");
}

#[test]
fn canonical_codec_round_trips_checked_flow_identity_and_public_label() {
    let mut program = minimal_program();
    let flow = FlowRuntimeId::from_checked_declaration_digest([0xa5; 32], "flow.opening")
        .expect("accepted Flow public label");
    program.flow_bindings[0].flow = flow.clone();
    program.flow_executables.push(AwbcFlowExecutable {
        metadata: RuntimeFlowExecutable {
            flow: flow.clone(),
            contract: FlowContractHash::from_bytes([0x5a; 32]),
            parameters: Vec::new(),
            controller: None,
        },
        function: AwbcFunctionId(0),
    });

    let encoded = program.encode_canonical().expect("encode checked Flow ID");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode checked Flow ID");

    assert_eq!(decoded.flow_executables[0].metadata.flow, flow);
    assert_eq!(
        decoded.flow_executables[0]
            .metadata
            .flow
            .public_label()
            .as_str(),
        "flow.opening"
    );
    assert_eq!(decoded, program);
}

#[test]
fn canonical_flow_bindings_preserve_same_label_declarations_and_reject_ambiguous_targets() {
    let mut program = minimal_program();
    let first = FlowRuntimeId::from_checked_declaration_digest([0x11; 32], "flow.opening")
        .expect("first checked Flow identity");
    let second = FlowRuntimeId::from_checked_declaration_digest([0x22; 32], "flow.opening")
        .expect("second checked Flow identity");
    program.flow_bindings[0].flow = first.clone();
    let mut second_function = program.functions[0].clone();
    second_function.blocks = AwbcTableRange::new(1, 1);
    second_function.entry_block = AwbcBlockId(1);
    program.functions.push(second_function);
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(1),
        instructions: AwbcTableRange::new(0, 0),
        terminator: AwbcTerminator::Return { value: None },
        safe_point: AwbcSafePointKind::FlowEntry,
        source_map: None,
    });
    program.flow_bindings.push(AwbcFlowBinding {
        flow: second.clone(),
        function: AwbcFunctionId(1),
    });

    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("same public label remains a valid typed Flow inventory");
    let encoded = program
        .encode_canonical()
        .expect("encode typed Flow bindings");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode typed Flow bindings");
    assert_eq!(decoded.flow_function(&first), Some(AwbcFunctionId(0)));
    assert_eq!(decoded.flow_function(&second), Some(AwbcFunctionId(1)));
    assert_eq!(decoded.flow_identity(AwbcFunctionId(0)), Some(&first));
    assert_eq!(decoded.flow_identity(AwbcFunctionId(1)), Some(&second));
    assert!(matches!(
        decoded.resolve_flow_target_value("flow.opening"),
        Err(RuntimeFlowTargetError::Ambiguous { matches: 2, .. })
    ));
    assert!(matches!(
        decoded.resolve_flow_target_value("flow.missing"),
        Err(RuntimeFlowTargetError::Missing { .. })
    ));
    assert!(matches!(
        decoded.resolve_flow_target_value(&first.canonical_label()),
        Err(RuntimeFlowTargetError::Invalid(_))
    ));

    program.flow_bindings.pop();
    program.functions.pop();
    program.blocks.pop();
    assert_eq!(
        program
            .resolve_flow_target_value("flow.opening")
            .map(|(flow, function)| (flow.clone(), function)),
        Ok((first, AwbcFunctionId(0)))
    );
}

#[test]
fn canonical_awbc_assertion_payload_round_trips_as_typed_identity() {
    let mut program = minimal_program();
    let guard =
        RuntimeAssertionGuardId::try_from_bytes([0xa7; 16]).expect("non-zero assertion guard");
    program.strings = vec![
        "always".to_owned(),
        "inventory >= 0".to_owned(),
        "inventory must stay non-negative".to_owned(),
        "main".to_owned(),
    ];
    program.entries[0].public_id = AwbcStringId(3);
    program.constants = vec![
        AwbcConstant::Bytes(guard.as_bytes().to_vec()),
        AwbcConstant::String(AwbcStringId(1)),
        AwbcConstant::String(AwbcStringId(2)),
        AwbcConstant::String(AwbcStringId(0)),
    ];
    program.effect_plans.push(AwbcEffectPlan {
        kind: AwbcEffectKind::Assert,
        signature: AwbcSignatureId(0),
        capability: None,
        audio: None,
        static_args: vec![
            AwbcConstantId(0),
            AwbcConstantId(1),
            AwbcConstantId(2),
            AwbcConstantId(3),
        ],
        resources: Vec::new(),
    });

    let encoded = program.encode_canonical().expect("encode assertion AWBC");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode assertion AWBC");
    decoded
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify decoded assertion AWBC");
    assert_eq!(decoded, program);

    let decoded_guard = match &decoded.constants[0] {
        AwbcConstant::Bytes(bytes) => RuntimeAssertionGuardId::try_from_bytes(
            bytes.as_slice().try_into().expect("fixed 16-byte guard"),
        )
        .expect("decoded guard remains non-zero"),
        other => panic!("assertion guard changed constant kind: {other:?}"),
    };
    assert_eq!(decoded_guard, guard);
    assert_eq!(decoded.effect_plans[0].kind, AwbcEffectKind::Assert);
}

#[test]
fn canonical_codec_round_trips_typed_audio_payload_table() {
    let mut program = minimal_program();
    program.audio_commands.push(AwbcAudioCommand::StopAll {
        fade_out_millis: AwbcAudioValueRef::Arg(AwbcAudioArg::new(0)),
    });

    let encoded = program
        .encode_canonical()
        .expect("encode AWBC audio payload");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode AWBC audio payload");

    assert_eq!(decoded, program);
}

#[test]
fn canonical_codec_round_trips_choice_and_nominal_runtime_types() {
    let mut program = minimal_program();
    let nominal_name = AwbcStringId(
        u32::try_from(program.strings.len()).expect("test string table fits AWBC index"),
    );
    program.strings.push("state.game.State".to_owned());
    let string_type = AwbcTypeId(
        u32::try_from(program.runtime_types.len()).expect("test type table fits AWBC index"),
    );
    program.runtime_types.push(AwbcRuntimeType::String);
    let nominal_type = AwbcTypeId(
        u32::try_from(program.runtime_types.len()).expect("test type table fits AWBC index"),
    );
    program.runtime_types.push(AwbcRuntimeType::Nominal {
        public_id: nominal_name,
        semantic_identity: [17; 32],
        layout: [18; 32],
    });
    program
        .runtime_types
        .push(AwbcRuntimeType::Choice(vec![string_type, nominal_type]));

    let encoded = program
        .encode_canonical()
        .expect("encode typed runtime type table");
    assert_eq!(
        u16::from_le_bytes([encoded[8], encoded[9]]),
        AWBC_CODEC_VERSION
    );
    assert_eq!(AWBC_CODEC_VERSION, 1);
    let mut unsupported_version = encoded.clone();
    unsupported_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        AwbcProgram::decode_canonical(&unsupported_version, AwbcDecodeBudget::default())
            .expect_err("codec rejects an unsupported version"),
        AwbcCodecError::UnsupportedCodecVersion {
            actual: 2,
            expected: AWBC_CODEC_VERSION,
        }
    );
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode typed runtime type table");
    decoded
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify typed runtime type table");

    assert_eq!(decoded, program);
}

#[test]
fn verifier_rejects_non_canonical_builtin_variant_schema() {
    let mut program = minimal_program();
    program
        .strings
        .extend(["None".to_owned(), "Some".to_owned()]);
    program.runtime_types.push(AwbcRuntimeType::Unit);
    program.runtime_types.push(AwbcRuntimeType::Variant {
        owner: AwbcVariantIdentity::Option,
        cases: vec![
            AwbcVariantCase {
                name: AwbcStringId(1),
                payload: None,
            },
            AwbcVariantCase {
                name: AwbcStringId(2),
                payload: Some(AwbcTypeId(0)),
            },
        ],
    });
    program.canonicalize_string_table();

    let error = program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("non-canonical Option schema must reject");
    assert!(
        matches!(
            &error,
            AwbcVerifyError::InvalidInvariant { message, .. }
                if message == "builtin variant owner has a non-canonical case schema"
        ),
        "{error:?}"
    );
}

#[test]
fn verifier_rejects_variant_constant_with_obsolete_nominal_type() {
    let mut program = minimal_program();
    program
        .strings
        .extend(["state.Widget".to_owned(), "Ready".to_owned()]);
    program.runtime_types.push(AwbcRuntimeType::Nominal {
        public_id: AwbcStringId(1),
        semantic_identity: [23; 32],
        layout: [24; 32],
    });
    program.constants.push(AwbcConstant::Variant {
        ty: AwbcTypeId(0),
        case: 0,
        case_name: AwbcStringId(2),
        payload: None,
    });
    program.canonicalize_string_table();

    let error = program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("variant constant cannot reference a nominal type");
    assert!(
        matches!(
            &error,
            AwbcVerifyError::InvalidInvariant { message, .. }
                if message == "variant constant references a non-variant type"
        ),
        "{error:?}"
    );
}

#[test]
fn fiber_checkpoint_and_serde_preserve_cleanup_stacks() {
    let program = minimal_program();
    let mut fiber =
        FiberState::for_entry(&program, AwbcEntryId(0), 7, 64).expect("fiber initializes");
    fiber
        .active_frame_mut()
        .expect("active frame")
        .root_cleanups
        .push(FiberScopeCleanup {
            key: "handle.root".to_owned(),
            effect: AwbcEffectPlanId(0),
            args: vec![RuntimeValue::String("root".to_owned())],
        });
    fiber
        .active_frame_mut()
        .expect("active frame")
        .scopes
        .push(FiberScope {
            id: AwbcScopeId(0),
            depth: 1,
            cleanups: vec![FiberScopeCleanup {
                key: "handle.scope".to_owned(),
                effect: AwbcEffectPlanId(0),
                args: vec![RuntimeValue::String("scope".to_owned())],
            }],
        });

    let checkpoint = fiber.checkpoint();
    let encoded_checkpoint =
        serde_json::to_string(&checkpoint).expect("fiber checkpoint serializes");
    let decoded_checkpoint = serde_json::from_str(&encoded_checkpoint)
        .expect("fiber checkpoint deserializes without session identity");
    let encoded = serde_json::to_string(&fiber).expect("fiber state serializes");
    let decoded: FiberState = serde_json::from_str(&encoded).expect("fiber state deserializes");
    assert_eq!(decoded, fiber);

    fiber
        .active_frame_mut()
        .expect("active frame")
        .root_cleanups
        .clear();
    fiber
        .active_frame_mut()
        .expect("active frame")
        .scopes
        .clear();
    fiber.restore(decoded_checkpoint);

    let frame = fiber.active_frame().expect("active frame restored");
    assert_eq!(frame.root_cleanups[0].key, "handle.root");
    assert_eq!(frame.scopes[0].cleanups[0].key, "handle.scope");
}

#[test]
fn verifier_rejects_audio_effect_without_typed_payload() {
    let mut program = minimal_program();
    program.effect_plans.push(AwbcEffectPlan {
        kind: AwbcEffectKind::Audio,
        signature: AwbcSignatureId(0),
        capability: None,
        audio: None,
        static_args: Vec::new(),
        resources: Vec::new(),
    });

    assert!(matches!(
        program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::MalformedAudioPayload { effect: 0, .. })
    ));
}

#[test]
fn verifier_rejects_audio_payload_arg_outside_effect_signature() {
    let mut program = minimal_program();
    program.audio_commands.push(AwbcAudioCommand::StopAll {
        fade_out_millis: AwbcAudioValueRef::Arg(AwbcAudioArg::new(1)),
    });
    program.effect_plans.push(AwbcEffectPlan {
        kind: AwbcEffectKind::Audio,
        signature: AwbcSignatureId(0),
        capability: None,
        audio: Some(AwbcAudioCommandId(0)),
        static_args: Vec::new(),
        resources: Vec::new(),
    });

    assert!(matches!(
        program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::MalformedAudioPayload { effect: 0, .. })
    ));
}

#[test]
fn verifier_rejects_non_audio_effect_with_typed_audio_payload() {
    let mut program = minimal_program();
    program.audio_commands.push(AwbcAudioCommand::StopAll {
        fade_out_millis: AwbcAudioValueRef::Const(AwbcConstantId(0)),
    });
    program.effect_plans.push(AwbcEffectPlan {
        kind: AwbcEffectKind::Log,
        signature: AwbcSignatureId(0),
        capability: None,
        audio: Some(AwbcAudioCommandId(0)),
        static_args: Vec::new(),
        resources: Vec::new(),
    });

    assert!(matches!(
        program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::MalformedAudioPayload { effect: 0, .. })
    ));
}

#[test]
fn verifier_rejects_effect_static_argument_shape_that_product_mapping_cannot_read() {
    let mut program = minimal_program();
    program.effect_plans.push(AwbcEffectPlan {
        kind: AwbcEffectKind::Log,
        signature: AwbcSignatureId(0),
        capability: None,
        audio: None,
        static_args: Vec::new(),
        resources: Vec::new(),
    });

    let error = program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("zero assert guard must be rejected");
    assert!(
        matches!(
            error,
            AwbcVerifyError::MalformedEffectPayload { effect: 0, .. }
        ),
        "{error:?}"
    );
}

#[test]
fn verifier_rejects_evaluated_effect_signature_with_wrong_arity() {
    let mut program = minimal_program();
    program
        .constants
        .push(AwbcConstant::String(AwbcStringId(0)));
    program.signatures.push(AwbcSignature {
        params: vec![AwbcTypeId(0)],
        result: None,
        effects: AwbcEffectSetId(0),
    });
    program.effect_plans.push(AwbcEffectPlan {
        kind: AwbcEffectKind::SignalWrite,
        signature: AwbcSignatureId(1),
        capability: None,
        audio: None,
        static_args: vec![AwbcConstantId(0), AwbcConstantId(0)],
        resources: Vec::new(),
    });

    assert!(matches!(
        program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::MalformedEffectPayload { effect: 0, .. })
    ));
}

#[test]
fn verifier_rejects_unknown_assert_profile_instead_of_defaulting_it() {
    let mut program = minimal_program();
    program.strings.extend([
        "profile.condition".to_owned(),
        "profile.message".to_owned(),
        "sometimes".to_owned(),
    ]);
    program.constants = vec![
        AwbcConstant::Bytes(vec![7; 16]),
        AwbcConstant::String(AwbcStringId(1)),
        AwbcConstant::String(AwbcStringId(2)),
        AwbcConstant::String(AwbcStringId(3)),
    ];
    program.effect_plans.push(AwbcEffectPlan {
        kind: AwbcEffectKind::Assert,
        signature: AwbcSignatureId(0),
        capability: None,
        audio: None,
        static_args: vec![
            AwbcConstantId(0),
            AwbcConstantId(1),
            AwbcConstantId(2),
            AwbcConstantId(3),
        ],
        resources: Vec::new(),
    });

    let error = program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("unknown assert profile must be rejected");
    assert!(
        matches!(
            error,
            AwbcVerifyError::MalformedEffectPayload { effect: 0, .. }
        ),
        "{error:?}"
    );
}

#[test]
fn verifier_rejects_zero_assert_guard() {
    let mut program = minimal_program();
    program.strings.extend([
        "profile.condition".to_owned(),
        "profile.message".to_owned(),
        "sometimes".to_owned(),
    ]);
    program.constants = vec![
        AwbcConstant::Bytes(vec![0; 16]),
        AwbcConstant::String(AwbcStringId(1)),
        AwbcConstant::String(AwbcStringId(2)),
        AwbcConstant::String(AwbcStringId(3)),
    ];
    program.effect_plans.push(AwbcEffectPlan {
        kind: AwbcEffectKind::Assert,
        signature: AwbcSignatureId(0),
        capability: None,
        audio: None,
        static_args: vec![
            AwbcConstantId(0),
            AwbcConstantId(1),
            AwbcConstantId(2),
            AwbcConstantId(3),
        ],
        resources: Vec::new(),
    });

    let error = program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("zero assert guard must be rejected");
    assert!(
        matches!(
            error,
            AwbcVerifyError::MalformedEffectPayload { effect: 0, .. }
        ),
        "{error:?}"
    );
}

#[test]
fn decode_rejects_encoded_byte_budget() {
    let bytes = minimal_program().encode_canonical().expect("encode AWBC");
    let budget = AwbcDecodeBudget {
        encoded_bytes: bytes.len() - 1,
        ..AwbcDecodeBudget::default()
    };
    assert!(matches!(
        AwbcProgram::decode_canonical(&bytes, budget),
        Err(AwbcCodecError::BudgetExceeded {
            budget: "encoded_bytes",
            ..
        })
    ));
}

#[test]
fn verifier_reports_uninitialized_register() {
    let mut program = minimal_program();
    program.runtime_types.push(AwbcRuntimeType::Bool);
    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(2),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    program.instructions.push(AwbcInstruction::Move {
        dst: AwbcRegisterId(0),
        src: AwbcRegisterId(0),
    });
    program.blocks[0].instructions = AwbcTableRange::new(0, 1);
    assert!(matches!(
        program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::UninitializedRegister { register: 0, .. })
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "The AWBC closure fixture is intentionally inline so opcode, verifier, codec, and VM expectations stay in one place."
)]
fn closure_instructions_capture_and_apply_awbc_function_value() {
    let program = AwbcProgram {
        strings: vec!["captured".to_owned(), "main".to_owned(), "x".to_owned()],
        constants: vec![AwbcConstant::String(AwbcStringId(0))],
        signatures: vec![
            AwbcSignature {
                params: Vec::new(),
                result: Some(AwbcTypeId(1)),
                effects: AwbcEffectSetId(0),
            },
            AwbcSignature {
                params: vec![AwbcTypeId(1)],
                result: Some(AwbcTypeId(1)),
                effects: AwbcEffectSetId(0),
            },
        ],
        frame_layouts: vec![
            AwbcFrameLayout {
                slots: vec![
                    AwbcFrameSlot {
                        name: Some(AwbcStringId(2)),
                        ty: AwbcTypeId(1),
                        role: AwbcFrameSlotRole::Local,
                        scope_depth: 0,
                    },
                    AwbcFrameSlot {
                        name: None,
                        ty: AwbcTypeId(1),
                        role: AwbcFrameSlotRole::Temporary,
                        scope_depth: 0,
                    },
                    AwbcFrameSlot {
                        name: None,
                        ty: AwbcTypeId(1),
                        role: AwbcFrameSlotRole::Temporary,
                        scope_depth: 0,
                    },
                ],
                max_scope_depth: 0,
            },
            AwbcFrameLayout {
                slots: vec![AwbcFrameSlot {
                    name: Some(AwbcStringId(2)),
                    ty: AwbcTypeId(1),
                    role: AwbcFrameSlotRole::Parameter,
                    scope_depth: 0,
                }],
                max_scope_depth: 0,
            },
        ],
        functions: vec![
            AwbcFunction {
                public_id: Some(AwbcStringId(1)),
                kind: AwbcFunctionKind::Flow,
                signature: AwbcSignatureId(0),
                frame_layout: AwbcFrameLayoutId(0),
                blocks: AwbcTableRange::new(0, 1),
                entry_block: AwbcBlockId(0),
                flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
            },
            AwbcFunction {
                public_id: None,
                kind: AwbcFunctionKind::Synthetic,
                signature: AwbcSignatureId(1),
                frame_layout: AwbcFrameLayoutId(1),
                blocks: AwbcTableRange::new(1, 1),
                entry_block: AwbcBlockId(1),
                flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
            },
        ],
        blocks: vec![
            AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::new(0, 3),
                terminator: AwbcTerminator::Return {
                    value: Some(AwbcRegisterId(2)),
                },
                safe_point: AwbcSafePointKind::FlowEntry,
                source_map: None,
            },
            AwbcBlock {
                owner: AwbcFunctionId(1),
                instructions: AwbcTableRange::new(3, 0),
                terminator: AwbcTerminator::Return {
                    value: Some(AwbcRegisterId(0)),
                },
                safe_point: AwbcSafePointKind::CallableBoundary,
                source_map: None,
            },
        ],
        instructions: vec![
            AwbcInstruction::LoadConst {
                dst: AwbcRegisterId(0),
                constant: AwbcConstantId(0),
            },
            AwbcInstruction::MakeFunction {
                dst: AwbcRegisterId(1),
                function: AwbcFunctionId(1),
                params: Vec::new(),
                capture_names: vec![AwbcStringId(2)],
                captures: vec![AwbcRegisterId(0)],
            },
            AwbcInstruction::ApplyFunction {
                dst: AwbcRegisterId(2),
                callee: AwbcRegisterId(1),
                args: Vec::new(),
            },
        ],
        flow_bindings: vec![test_flow_binding("main", 0)],
        entries: vec![AwbcEntry {
            runtime_id: crate::plan::EntryRuntimeId::canonical("main")
                .expect("test entry runtime ID is valid"),
            binding: crate::entry::EntryBindingIdentity::from_bytes([1; 32]),
            public_id: AwbcStringId(1),
            kind: AwbcEntryKind::Cli,
            signature: AwbcSignatureId(0),
            target: AwbcEntryTarget::Function(AwbcFunctionId(0)),
            roles: crate::entry::RuntimeEntryRoles::None,
        }],
        ..AwbcProgram::default()
    };
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("closure program verifies");

    let encoded = program.encode_canonical().expect("encode closure program");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode closure program");
    assert_eq!(decoded, program);

    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64).expect("create fiber");
    let output = super::vm::step(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("step closure program");
    assert_eq!(
        output.exit,
        super::vm::VmExit::Returned(Some(RuntimeValue::String("captured".to_owned())))
    );
}

#[test]
fn expression_apply_preserves_dynamic_call_frame_across_suspension_and_resume() {
    let program = expression_apply_program(
        Vec::new(),
        vec![
            (
                AwbcTerminator::BudgetYield {
                    resume: AwbcResumePointId(0),
                },
                AwbcSafePointKind::CallableBoundary,
            ),
            (
                AwbcTerminator::Return { value: None },
                AwbcSafePointKind::None,
            ),
        ],
        vec![AwbcResumePoint {
            function: AwbcFunctionId(1),
            block: AwbcBlockId(2),
            frame_layout: AwbcFrameLayoutId(1),
            kind: AwbcSafePointKind::BudgetYield,
        }],
    );
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify expression apply suspension program");

    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64)
        .expect("create expression apply fiber");
    let output = super::vm::step(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("suspending expression apply exits normally");

    assert_eq!(
        output.exit,
        super::vm::VmExit::Suspended(FiberSuspensionReason::BudgetYield)
    );
    assert_eq!(fiber.frames.len(), 2);
    assert_eq!(fiber.cursor.function, AwbcFunctionId(1));
    assert_eq!(
        fiber
            .suspension
            .as_ref()
            .and_then(FiberSuspension::declared_resume),
        Some(AwbcResumePointId(0))
    );
    assert_eq!(
        fiber.frames[1]
            .return_to
            .expect("dynamic call continuation")
            .cursor,
        super::fiber::FiberCursor {
            function: AwbcFunctionId(0),
            block: AwbcBlockId(0),
            instruction_offset: 2,
        }
    );
    fiber
        .validate_for_program(&program)
        .expect("suspended dynamic call snapshot validates");
    let encoded = serde_json::to_string(&fiber).expect("serialize suspended dynamic call");
    let mut fiber: FiberState =
        serde_json::from_str(&encoded).expect("deserialize suspended dynamic call");
    fiber
        .validate_for_program(&program)
        .expect("restored dynamic call snapshot validates");

    fiber
        .resume_at(&program, AwbcResumePointId(0))
        .expect("resume dynamic callee");
    let output = super::vm::step(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("finish resumed dynamic call");
    assert_eq!(
        output.exit,
        super::vm::VmExit::Returned(Some(RuntimeValue::Unit))
    );
    assert_eq!(fiber.frames.len(), 1);
}

#[test]
fn expression_apply_surfaces_await_from_the_dynamic_callee() {
    let mut await_program = expression_apply_program(
        vec![AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(1),
        }],
        vec![
            (
                AwbcTerminator::Await {
                    handle: AwbcRegisterId(0),
                    binding: None,
                    resume: AwbcResumePointId(0),
                },
                AwbcSafePointKind::CallableBoundary,
            ),
            (
                AwbcTerminator::Return { value: None },
                AwbcSafePointKind::None,
            ),
        ],
        vec![AwbcResumePoint {
            function: AwbcFunctionId(1),
            block: AwbcBlockId(2),
            frame_layout: AwbcFrameLayoutId(1),
            kind: AwbcSafePointKind::Await,
        }],
    );
    await_program.strings.push("task.dynamic".to_owned());
    await_program
        .constants
        .push(AwbcConstant::String(AwbcStringId(1)));
    await_program
        .runtime_types
        .push(AwbcRuntimeType::TaskHandle);
    await_program.frame_layouts[1].slots[0].ty = AwbcTypeId(2);

    let mut fiber = FiberState::for_entry(&await_program, AwbcEntryId(0), 0, 64)
        .expect("create await expression apply fiber");
    let output = super::vm::step(
        &await_program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("dynamic callee reaches await");
    assert!(matches!(
        output.exit,
        super::vm::VmExit::Suspended(FiberSuspensionReason::Await {
            target: FiberAwaitTarget::Task(RuntimeValue::String(ref task)),
            binding: None,
        }) if task == "task.dynamic"
    ));
    assert_eq!(
        fiber
            .suspension
            .as_ref()
            .and_then(FiberSuspension::declared_resume),
        Some(AwbcResumePointId(0))
    );
    fiber
        .validate_for_program(&await_program)
        .expect("awaiting dynamic callee snapshot validates");
    fiber
        .resume_at(&await_program, AwbcResumePointId(0))
        .expect("resume await dynamic callee");
    assert_eq!(
        super::vm::step(
            &await_program,
            &mut fiber,
            super::vm::VmStepOptions {
                max_instructions: 16,
            },
        )
        .expect("finish await dynamic callee")
        .exit,
        super::vm::VmExit::Returned(Some(RuntimeValue::Unit))
    );
}

#[test]
fn expression_apply_surfaces_host_call_from_the_dynamic_callee() {
    let mut host_program = expression_apply_program(
        Vec::new(),
        vec![
            (
                AwbcTerminator::HostCall {
                    call: AwbcHostCallId(0),
                    args: Vec::new(),
                    dst: None,
                    resume: AwbcResumePointId(0),
                },
                AwbcSafePointKind::CallableBoundary,
            ),
            (
                AwbcTerminator::Return { value: None },
                AwbcSafePointKind::None,
            ),
        ],
        vec![AwbcResumePoint {
            function: AwbcFunctionId(1),
            block: AwbcBlockId(2),
            frame_layout: AwbcFrameLayoutId(1),
            kind: AwbcSafePointKind::HostCall,
        }],
    );
    host_program.host_calls.push(AwbcHostCall {
        public_id: AwbcStringId(0),
        capability: AwbcStringId(0),
        operation: AwbcStringId(0),
        signature: AwbcSignatureId(1),
        mode: AwbcHostCallMode::Suspend,
        deterministic: true,
    });

    let mut fiber = FiberState::for_entry(&host_program, AwbcEntryId(0), 0, 64)
        .expect("create host-call expression apply fiber");
    let output = super::vm::step(
        &host_program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("dynamic callee reaches host call");
    assert_eq!(
        output.exit,
        super::vm::VmExit::Suspended(FiberSuspensionReason::HostCall {
            call: AwbcHostCallId(0),
            args: Vec::new(),
            destination: None,
        })
    );
    assert_eq!(
        fiber
            .suspension
            .as_ref()
            .and_then(FiberSuspension::declared_resume),
        Some(AwbcResumePointId(0))
    );
    fiber
        .validate_for_program(&host_program)
        .expect("host-call dynamic callee snapshot validates");
    fiber
        .resume_at(&host_program, AwbcResumePointId(0))
        .expect("resume host-call dynamic callee");
    assert_eq!(
        super::vm::step(
            &host_program,
            &mut fiber,
            super::vm::VmStepOptions {
                max_instructions: 16,
            },
        )
        .expect("finish host-call dynamic callee")
        .exit,
        super::vm::VmExit::Returned(Some(RuntimeValue::Unit))
    );
}

#[test]
fn expression_apply_uses_the_callers_budget_without_a_hidden_inner_limit() {
    let synthetic_instructions = vec![
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(0),
        };
        4_097
    ];
    let program = expression_apply_program(
        synthetic_instructions,
        vec![(
            AwbcTerminator::Return { value: None },
            AwbcSafePointKind::CallableBoundary,
        )],
        Vec::new(),
    );
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify expression apply budget program");

    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 8_192)
        .expect("create expression apply fiber");
    let output = super::vm::step(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 8_192,
        },
    )
    .expect("long dynamic call uses caller budget");

    assert_eq!(
        output.exit,
        super::vm::VmExit::Returned(Some(RuntimeValue::Unit))
    );
    assert!(output.executed > 4_096);
}

#[test]
fn expression_apply_keeps_partial_application_as_a_value_operation() {
    let mut program = expression_apply_program(
        Vec::new(),
        vec![(
            AwbcTerminator::Return { value: None },
            AwbcSafePointKind::CallableBoundary,
        )],
        Vec::new(),
    );
    program.signatures[0].result = Some(AwbcTypeId(1));
    program.signatures[1].params.push(AwbcTypeId(1));
    program.frame_layouts[0].slots[1].ty = AwbcTypeId(1);
    program.frame_layouts[1].slots[0].ty = AwbcTypeId(1);
    program.frame_layouts[1].slots[0].role = AwbcFrameSlotRole::Parameter;
    program.frame_layouts[1].slots[0].name = Some(AwbcStringId(0));
    let AwbcInstruction::MakeFunction { params, .. } = &mut program.instructions[0] else {
        panic!("expression apply fixture starts with MakeFunction");
    };
    params.push(AwbcStringId(0));
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify partial expression apply program");

    let mut fiber =
        FiberState::for_entry(&program, AwbcEntryId(0), 0, 16).expect("create partial fiber");
    let output = super::vm::step(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("partially apply function");
    let super::vm::VmExit::Returned(Some(RuntimeValue::Function(function))) = output.exit else {
        panic!("partial application must return a function value");
    };
    assert_eq!(function.params, vec!["main".to_owned()]);
    assert!(function.captures.is_empty());
    assert_eq!(fiber.frames.len(), 1);
}

#[test]
fn verifier_rejects_make_function_binding_name_mismatch() {
    let mut program = expression_apply_program(
        Vec::new(),
        vec![(
            AwbcTerminator::Return { value: None },
            AwbcSafePointKind::CallableBoundary,
        )],
        Vec::new(),
    );
    program.signatures[1].params.push(AwbcTypeId(1));
    program.frame_layouts[1].slots[0].ty = AwbcTypeId(1);
    program.frame_layouts[1].slots[0].role = AwbcFrameSlotRole::Parameter;
    program.frame_layouts[1].slots[0].name = Some(AwbcStringId(0));
    let AwbcInstruction::MakeFunction { params, .. } = &mut program.instructions[0] else {
        panic!("expression apply fixture starts with MakeFunction");
    };
    params.push(AwbcStringId(0));
    program.frame_layouts[1].slots[0].name = None;

    assert!(matches!(
        program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::InvalidInvariant { message, .. })
            if message.contains("name matching its closure binding")
    ));
}

#[test]
fn expression_apply_rejects_over_application_before_entering_the_callee() {
    let mut program = expression_apply_program(
        Vec::new(),
        vec![(
            AwbcTerminator::Return { value: None },
            AwbcSafePointKind::CallableBoundary,
        )],
        Vec::new(),
    );
    let AwbcInstruction::ApplyFunction { args, .. } = &mut program.instructions[1] else {
        panic!("expression apply fixture ends with ApplyFunction");
    };
    args.push(AwbcRegisterId(0));
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify over-application program");

    let mut fiber =
        FiberState::for_entry(&program, AwbcEntryId(0), 0, 16).expect("create over-apply fiber");
    let output = super::vm::step(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("over-application becomes a typed trap");
    assert!(matches!(
        output.exit,
        super::vm::VmExit::Trapped(FiberTrap {
            code: AwbcTrapCode::TypeMismatch,
            message: Some(ref message),
            ..
        }) if message == "function application expected 0 arguments, received 1"
    ));
    assert_eq!(fiber.frames.len(), 1);
}

#[test]
fn budget_preemption_inside_dynamic_callee_resumes_at_the_exact_cursor() {
    let program = expression_apply_program(
        Vec::new(),
        vec![(
            AwbcTerminator::Return { value: None },
            AwbcSafePointKind::CallableBoundary,
        )],
        Vec::new(),
    );
    let mut fiber =
        FiberState::for_entry(&program, AwbcEntryId(0), 0, 2).expect("create budgeted fiber");
    let output = super::vm::step(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("preempt dynamic callee");

    let exact_callee_entry = super::fiber::FiberCursor {
        function: AwbcFunctionId(1),
        block: AwbcBlockId(1),
        instruction_offset: 0,
    };
    assert!(matches!(
        output.exit,
        super::vm::VmExit::BudgetYield(point) if point.cursor == exact_callee_entry
    ));
    assert_eq!(
        fiber
            .suspension
            .as_ref()
            .map(|suspension| suspension.resume),
        Some(FiberResumeTarget::Exact(exact_callee_entry))
    );
    fiber
        .validate_for_program(&program)
        .expect("preempted dynamic call snapshot validates");
    let encoded = serde_json::to_string(&fiber).expect("serialize preempted dynamic call");
    let mut fiber: FiberState =
        serde_json::from_str(&encoded).expect("deserialize preempted dynamic call");
    fiber
        .resume_budget_yield(&program)
        .expect("resume exact budget target");
    assert_eq!(fiber.cursor, exact_callee_entry);
    fiber.replenish_budget();

    let output = super::vm::step(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("finish exact-resumed dynamic call");
    assert_eq!(
        output.exit,
        super::vm::VmExit::Returned(Some(RuntimeValue::Unit))
    );
}

#[test]
fn verifier_rejects_branch_outside_function() {
    let mut program = minimal_program();
    program.blocks[0].terminator = AwbcTerminator::Jump {
        target: AwbcBlockId(1),
    };
    assert!(matches!(
        program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::ControlFlowEscapesFunction { target: 1, .. })
    ));
}

#[test]
fn budget_safe_point_suspends_and_resumes() {
    let mut program = minimal_program();
    program.functions[0].blocks = AwbcTableRange::new(0, 2);
    program.blocks[0].terminator = AwbcTerminator::BudgetYield {
        resume: AwbcResumePointId(0),
    };
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(0),
        instructions: AwbcTableRange::new(0, 0),
        terminator: AwbcTerminator::Return { value: None },
        safe_point: AwbcSafePointKind::None,
        source_map: None,
    });
    program.resume_points.push(AwbcResumePoint {
        function: AwbcFunctionId(0),
        block: AwbcBlockId(1),
        frame_layout: AwbcFrameLayoutId(0),
        kind: AwbcSafePointKind::BudgetYield,
    });
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify budget-yield program");

    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 7, 100).expect("create fiber");
    fiber
        .suspend(FiberSuspension {
            resume: FiberResumeTarget::Declared(AwbcResumePointId(0)),
            reason: FiberSuspensionReason::BudgetYield,
        })
        .expect("suspend fiber");
    assert_eq!(fiber.status, FiberStatus::Suspended);
    fiber
        .resume_at(&program, AwbcResumePointId(0))
        .expect("resume fiber");
    assert_eq!(fiber.status, FiberStatus::Running);
    assert_eq!(fiber.cursor.block, AwbcBlockId(1));
}

#[test]
fn nested_return_restores_caller_resume_and_destination() {
    let mut program = minimal_program();
    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(1),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    program.frame_layouts.push(AwbcFrameLayout {
        slots: Vec::new(),
        max_scope_depth: 0,
    });
    program.signatures.push(AwbcSignature {
        params: Vec::new(),
        result: Some(AwbcTypeId(1)),
        effects: AwbcEffectSetId(0),
    });
    program.functions[0].blocks = AwbcTableRange::new(0, 2);
    program.functions.push(AwbcFunction {
        public_id: None,
        kind: AwbcFunctionKind::Synthetic,
        signature: AwbcSignatureId(1),
        frame_layout: AwbcFrameLayoutId(1),
        blocks: AwbcTableRange::new(2, 1),
        entry_block: AwbcBlockId(2),
        flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
    });
    program.blocks[0].terminator = AwbcTerminator::CallFunction {
        function: AwbcFunctionId(1),
        args: Vec::new(),
        dst: Some(AwbcRegisterId(0)),
        resume: AwbcResumePointId(0),
    };
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(0),
        instructions: AwbcTableRange::new(0, 0),
        terminator: AwbcTerminator::Return {
            value: Some(AwbcRegisterId(0)),
        },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(1),
        instructions: AwbcTableRange::new(0, 0),
        terminator: AwbcTerminator::Return { value: None },
        safe_point: AwbcSafePointKind::FlowEntry,
        source_map: None,
    });
    program.resume_points.push(AwbcResumePoint {
        function: AwbcFunctionId(0),
        block: AwbcBlockId(1),
        frame_layout: AwbcFrameLayoutId(0),
        kind: AwbcSafePointKind::CallableBoundary,
    });

    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 3, 100).expect("fiber");
    fiber
        .push_call_frame(
            &program,
            AwbcFunctionId(1),
            AwbcResumePointId(0),
            Some(AwbcRegisterId(0)),
        )
        .expect("push callee");
    assert_eq!(fiber.frames.len(), 2);
    assert_eq!(fiber.cursor.function, AwbcFunctionId(1));

    let returned = RuntimeValue::String("ok".to_owned());
    assert!(
        !fiber
            .finish_return(&program, Some(returned.clone()))
            .expect("return to caller")
    );
    assert_eq!(fiber.frames.len(), 1);
    assert_eq!(fiber.cursor.function, AwbcFunctionId(0));
    assert_eq!(fiber.cursor.block, AwbcBlockId(1));
    assert_eq!(
        fiber
            .active_frame()
            .expect("caller frame")
            .register(AwbcRegisterId(0))
            .expect("return register"),
        &returned
    );
}
