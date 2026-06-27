use super::codec::{AwbcCodecError, AwbcDecodeBudget};
use super::fiber::{FiberState, FiberStatus, FiberSuspension, FiberSuspensionReason};
use super::schema::*;
use super::verify::{AwbcVerifyBudget, AwbcVerifyContext, AwbcVerifyError};
use crate::value::RuntimeValue;

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
        entries: vec![AwbcEntry {
            public_id: AwbcStringId(0),
            kind: AwbcEntryKind::Game,
            signature: AwbcSignatureId(0),
            target: AwbcEntryTarget::Function(AwbcFunctionId(0)),
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
            resume: AwbcResumePointId(0),
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
