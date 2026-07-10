use super::AwbcProductStepExecutor;
use crate::awbc::product_step::mapping::MappedEffect;
use crate::awbc::schema::{
    AwbcAudioArg, AwbcAudioCommand, AwbcAudioCommandId, AwbcAudioValueRef, AwbcBlock, AwbcBlockId,
    AwbcConstant, AwbcContentUnit, AwbcEffectKind, AwbcEffectPlan, AwbcEffectPlanId,
    AwbcEffectSetId, AwbcEntryId, AwbcFrameLayout, AwbcFrameLayoutId, AwbcFrameSlot,
    AwbcFrameSlotRole, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId, AwbcFunctionKind,
    AwbcHostCall, AwbcHostCallId, AwbcHostCallMode, AwbcInstruction, AwbcProgram, AwbcRegisterId,
    AwbcResumePoint, AwbcResumePointId, AwbcSafePointKind, AwbcSignature, AwbcSignatureId,
    AwbcStringId, AwbcTableRange, AwbcTerminator, AwbcTrapCode, AwbcTypeId,
};
use crate::engine::{FlowExit, FlowFiberStatus};
use crate::step::{
    RuntimeDiagnosticCategory, RuntimeHostCallId, RuntimeHostCallMode, RuntimeHostCallResult,
    RuntimeStepInput, RuntimeStepOptions, RuntimeStepStopReason,
};
use crate::value::{RuntimePayload, RuntimeValue};

#[test]
fn minimal_return_program_finishes_without_diagnostics() {
    let mut executor = AwbcProductStepExecutor::for_entry(
        return_program(),
        crate::awbc::schema::AwbcEntryId(0),
        64,
    )
    .expect("minimal product AWBC executor starts");

    let result = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert_eq!(result.stop_reason, RuntimeStepStopReason::Done);
    assert!(result.output.diagnostics.is_empty());
    assert!(matches!(result.fiber_status, FlowFiberStatus::Done(_)));
}

fn return_program() -> AwbcProgram {
    let mut program = trap_program(AwbcTrapCode::InternalInvariant, "unused");
    program.blocks[0].terminator = AwbcTerminator::Return { value: None };
    program
}

#[test]
fn host_call_request_and_result_resume_at_runtime_step_boundary() {
    let mut executor = AwbcProductStepExecutor::for_entry(host_call_program(), AwbcEntryId(0), 64)
        .expect("host-call product executor starts");

    let first = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert_eq!(first.output.requests.host_calls.len(), 1);
    let request = &first.output.requests.host_calls[0];
    assert_eq!(request.id, RuntimeHostCallId("host.probe".to_owned()));
    assert_eq!(request.public_id, "host.probe");
    assert_eq!(request.capability, "probe");
    assert_eq!(request.operation, "read");
    assert_eq!(request.args, Vec::<RuntimePayload>::new());
    assert_eq!(request.mode, RuntimeHostCallMode::Suspend);
    assert!(request.deterministic);
    assert_eq!(first.stop_reason, RuntimeStepStopReason::Output);

    let second = executor.step(
        RuntimeStepInput {
            host_call_results: vec![RuntimeHostCallResult {
                id: request.id.clone(),
                outcome: Ok(RuntimePayload(RuntimeValue::String("host-ok".to_owned()))),
            }],
            ..RuntimeStepInput::default()
        },
        RuntimeStepOptions::default(),
    );

    assert!(second.output.requests.host_calls.is_empty());
    assert_eq!(second.fiber_status, FlowFiberStatus::Running);

    let third = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
    assert!(
        third
            .output
            .flow_events
            .contains(&crate::plan::FlowEvent::Return {
                value: "host-ok".to_owned()
            })
    );
    assert_eq!(
        third.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return("host-ok".to_owned()))
    );
}

#[test]
fn ensure_content_instruction_projects_typed_content_request() {
    let mut executor =
        AwbcProductStepExecutor::for_entry(content_ensure_program(), AwbcEntryId(0), 64)
            .expect("content product executor starts");

    let first = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert_eq!(first.output.requests.ensure_content.len(), 1);
    assert_eq!(
        first.output.requests.ensure_content[0].content,
        "line.content"
    );
    assert!(first.output.requests.ensure_content[0].resources.is_empty());
    assert_eq!(first.stats.executed_ops, 1);

    let second = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(second.output.requests.ensure_content.is_empty());
    assert_eq!(second.stop_reason, RuntimeStepStopReason::Done);
}

#[test]
fn trap_terminators_project_typed_runtime_diagnostics() {
    let cases = [
        (AwbcTrapCode::TypeMismatch, RuntimeDiagnosticCategory::Type),
        (
            AwbcTrapCode::PatternMismatch,
            RuntimeDiagnosticCategory::Pattern,
        ),
        (
            AwbcTrapCode::HostAbiMismatch,
            RuntimeDiagnosticCategory::Host,
        ),
        (
            AwbcTrapCode::CapabilityDenied,
            RuntimeDiagnosticCategory::Capability,
        ),
        (
            AwbcTrapCode::DivisionByZero,
            RuntimeDiagnosticCategory::Runtime,
        ),
        (
            AwbcTrapCode::InvalidIndex,
            RuntimeDiagnosticCategory::Runtime,
        ),
        (
            AwbcTrapCode::MissingDynamicTarget,
            RuntimeDiagnosticCategory::Runtime,
        ),
        (
            AwbcTrapCode::ExplicitPanic,
            RuntimeDiagnosticCategory::Runtime,
        ),
        (
            AwbcTrapCode::UninitializedRegister,
            RuntimeDiagnosticCategory::Runtime,
        ),
        (
            AwbcTrapCode::InternalInvariant,
            RuntimeDiagnosticCategory::Internal,
        ),
    ];

    for (code, category) in cases {
        let message = format!("typed trap {code:?}");
        let mut executor =
            AwbcProductStepExecutor::for_entry(trap_program(code, &message), AwbcEntryId(0), 64)
                .expect("trap product executor starts");

        let result = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

        assert_eq!(result.stop_reason, RuntimeStepStopReason::Failed);
        assert_eq!(result.stats.diagnostics, 1);
        assert_eq!(result.output.diagnostics.len(), 1);
        assert_eq!(result.output.diagnostics[0].category, category);
        assert_eq!(result.output.diagnostics[0].message, message);
        assert_eq!(
            result.fiber_status,
            FlowFiberStatus::Failed(message.clone())
        );
    }
}

#[test]
fn effect_mapping_table_covers_every_awbc_effect_kind() {
    let mut program = AwbcProgram::default();
    let cases = [
        (AwbcEffectKind::RegisterHandle, true),
        (AwbcEffectKind::DropHandle, true),
        (AwbcEffectKind::Wait, true),
        (AwbcEffectKind::Audio, false),
        (AwbcEffectKind::Call, true),
        (AwbcEffectKind::Log, true),
        (AwbcEffectKind::SignalWrite, true),
        (AwbcEffectKind::MetricWrite, true),
        (AwbcEffectKind::EmitEvent, true),
        (AwbcEffectKind::Out, true),
        (AwbcEffectKind::Return, true),
        (AwbcEffectKind::Goto, true),
        (AwbcEffectKind::Panic, true),
        (AwbcEffectKind::Fail, true),
        (AwbcEffectKind::Bail, true),
        (AwbcEffectKind::Ensure, true),
        (AwbcEffectKind::Assert, true),
        (AwbcEffectKind::Close, true),
        (AwbcEffectKind::Select, true),
        (AwbcEffectKind::Break, true),
        (AwbcEffectKind::Continue, true),
    ];

    for (kind, should_map_to_line_effect) in cases {
        let effect = push_effect_plan(&mut program, kind);
        let mapped = kind.map_product_effect(&program, effect, &[]);
        assert_eq!(
            matches!(mapped, MappedEffect::Line(_)),
            should_map_to_line_effect
        );
        assert_eq!(
            matches!(mapped, MappedEffect::Unsupported(_)),
            !should_map_to_line_effect
        );
    }
}

#[test]
fn audio_effect_mapping_without_typed_payload_is_typed_internal_diagnostic() {
    let mut program = AwbcProgram::default();
    let effect = push_effect_plan(&mut program, AwbcEffectKind::Audio);

    let mapped = AwbcEffectKind::Audio.map_product_effect(&program, effect, &[]);

    let MappedEffect::Unsupported(diagnostic) = mapped else {
        panic!("malformed audio payload must not map to a line or audio request");
    };
    assert_eq!(diagnostic.category, RuntimeDiagnosticCategory::Internal);
}

#[test]
fn audio_effect_mapping_missing_arg_is_typed_internal_diagnostic() {
    let mut program = AwbcProgram::default();
    program.audio_commands.push(AwbcAudioCommand::StopAll {
        fade_out_millis: AwbcAudioValueRef::Arg(AwbcAudioArg::new(0)),
    });
    let effect = push_effect_plan(&mut program, AwbcEffectKind::Audio);
    program.effect_plans[effect.index()].audio = Some(AwbcAudioCommandId(0));
    program.effect_plans[effect.index()].static_args.clear();

    let mapped = AwbcEffectKind::Audio.map_product_effect(&program, effect, &[]);

    let MappedEffect::Unsupported(diagnostic) = mapped else {
        panic!("missing audio dynamic arg must not map to a request");
    };
    assert_eq!(diagnostic.category, RuntimeDiagnosticCategory::Internal);
}

#[test]
fn audio_effect_mapping_invalid_identifier_is_typed_type_diagnostic() {
    let mut program = AwbcProgram::default();
    let invalid_voice = constant_string(&mut program, "   ");
    program.audio_commands.push(AwbcAudioCommand::Stop {
        voice: AwbcAudioValueRef::Const(invalid_voice),
        fade_out_millis: AwbcAudioValueRef::Arg(AwbcAudioArg::new(0)),
    });
    let effect = push_effect_plan(&mut program, AwbcEffectKind::Audio);
    program.effect_plans[effect.index()].audio = Some(AwbcAudioCommandId(0));
    program.effect_plans[effect.index()].static_args.clear();

    let mapped =
        AwbcEffectKind::Audio.map_product_effect(&program, effect, &[RuntimeValue::u32(25)]);

    let MappedEffect::Unsupported(diagnostic) = mapped else {
        panic!("invalid audio identifier must not map to a request");
    };
    assert_eq!(diagnostic.category, RuntimeDiagnosticCategory::Type);
}

fn trap_program(code: AwbcTrapCode, message: &str) -> AwbcProgram {
    let strings = vec!["entry.main".to_owned(), message.to_owned()];
    let signature = AwbcSignature {
        params: Vec::new(),
        result: None,
        effects: AwbcEffectSetId(0),
    };
    AwbcProgram {
        strings,
        signatures: vec![signature],
        frame_layouts: vec![AwbcFrameLayout {
            slots: Vec::new(),
            max_scope_depth: 0,
        }],
        blocks: vec![AwbcBlock {
            owner: AwbcFunctionId(0),
            instructions: AwbcTableRange::new(0, 0),
            terminator: AwbcTerminator::Trap {
                code,
                message: Some(AwbcStringId(1)),
            },
            safe_point: AwbcSafePointKind::FlowEntry,
            source_map: None,
        }],
        functions: vec![AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 1),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags::default(),
        }],
        entries: vec![crate::awbc::schema::AwbcEntry {
            public_id: AwbcStringId(0),
            kind: crate::awbc::schema::AwbcEntryKind::Game,
            signature: AwbcSignatureId(0),
            target: crate::awbc::schema::AwbcEntryTarget::Function(AwbcFunctionId(0)),
        }],
        ..AwbcProgram::default()
    }
}

fn content_ensure_program() -> AwbcProgram {
    let strings = vec!["entry.main".to_owned(), "line.content".to_owned()];
    let signature = AwbcSignature {
        params: Vec::new(),
        result: None,
        effects: AwbcEffectSetId(0),
    };
    AwbcProgram {
        strings,
        signatures: vec![signature],
        frame_layouts: vec![AwbcFrameLayout {
            slots: Vec::new(),
            max_scope_depth: 0,
        }],
        content_units: vec![AwbcContentUnit {
            public_id: AwbcStringId(1),
            line_task_group: None,
            display: None,
            source: None,
            resources: Vec::new(),
        }],
        instructions: vec![AwbcInstruction::EnsureContent {
            content: crate::awbc::schema::AwbcContentUnitId(0),
        }],
        blocks: vec![AwbcBlock {
            owner: AwbcFunctionId(0),
            instructions: AwbcTableRange::new(0, 1),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::FlowEntry,
            source_map: None,
        }],
        functions: vec![AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 1),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags::default(),
        }],
        entries: vec![crate::awbc::schema::AwbcEntry {
            public_id: AwbcStringId(0),
            kind: crate::awbc::schema::AwbcEntryKind::Game,
            signature: AwbcSignatureId(0),
            target: crate::awbc::schema::AwbcEntryTarget::Function(AwbcFunctionId(0)),
        }],
        ..AwbcProgram::default()
    }
}

fn host_call_program() -> AwbcProgram {
    let strings = vec![
        "entry.main".to_owned(),
        "host.probe".to_owned(),
        "probe".to_owned(),
        "read".to_owned(),
    ];
    let signature = AwbcSignature {
        params: Vec::new(),
        result: Some(AwbcTypeId(1)),
        effects: AwbcEffectSetId(0),
    };
    let frame_layout = AwbcFrameLayout {
        slots: vec![AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(1),
            role: AwbcFrameSlotRole::ReturnValue,
            scope_depth: 0,
        }],
        max_scope_depth: 0,
    };
    AwbcProgram {
        strings,
        signatures: vec![signature],
        frame_layouts: vec![frame_layout],
        host_calls: vec![AwbcHostCall {
            public_id: AwbcStringId(1),
            capability: AwbcStringId(2),
            operation: AwbcStringId(3),
            signature: AwbcSignatureId(0),
            mode: AwbcHostCallMode::Suspend,
            deterministic: true,
        }],
        resume_points: vec![AwbcResumePoint {
            function: AwbcFunctionId(0),
            block: AwbcBlockId(1),
            frame_layout: AwbcFrameLayoutId(0),
            kind: AwbcSafePointKind::HostCall,
        }],
        blocks: vec![
            AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::HostCall {
                    call: AwbcHostCallId(0),
                    args: Vec::new(),
                    dst: Some(AwbcRegisterId(0)),
                    resume: AwbcResumePointId(0),
                },
                safe_point: AwbcSafePointKind::FlowEntry,
                source_map: None,
            },
            AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::Return {
                    value: Some(AwbcRegisterId(0)),
                },
                safe_point: AwbcSafePointKind::Return,
                source_map: None,
            },
        ],
        functions: vec![AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 2),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags(AwbcFunctionFlags::MAY_SUSPEND),
        }],
        entries: vec![crate::awbc::schema::AwbcEntry {
            public_id: AwbcStringId(0),
            kind: crate::awbc::schema::AwbcEntryKind::Game,
            signature: AwbcSignatureId(0),
            target: crate::awbc::schema::AwbcEntryTarget::Function(AwbcFunctionId(0)),
        }],
        ..AwbcProgram::default()
    }
}

fn push_effect_plan(program: &mut AwbcProgram, kind: AwbcEffectKind) -> AwbcEffectPlanId {
    let static_arg_count = match kind {
        AwbcEffectKind::Audio => 0,
        AwbcEffectKind::Call
        | AwbcEffectKind::RegisterHandle
        | AwbcEffectKind::SignalWrite
        | AwbcEffectKind::MetricWrite
        | AwbcEffectKind::Out
        | AwbcEffectKind::Ensure
        | AwbcEffectKind::Break => 2,
        AwbcEffectKind::Log => 4,
        AwbcEffectKind::EmitEvent | AwbcEffectKind::Assert => 3,
        AwbcEffectKind::DropHandle
        | AwbcEffectKind::Wait
        | AwbcEffectKind::Return
        | AwbcEffectKind::Goto
        | AwbcEffectKind::Panic
        | AwbcEffectKind::Fail
        | AwbcEffectKind::Bail
        | AwbcEffectKind::Close
        | AwbcEffectKind::Select
        | AwbcEffectKind::Continue => 1,
    };
    let static_args = (0..static_arg_count)
        .map(|index| {
            let value = if kind == AwbcEffectKind::Assert && index == 2 {
                "always".to_owned()
            } else {
                format!("arg{index}")
            };
            constant_string(program, &value)
        })
        .collect();
    let effect = AwbcEffectPlanId(
        u32::try_from(program.effect_plans.len()).expect("test effect table index fits u32"),
    );
    program.effect_plans.push(AwbcEffectPlan {
        kind,
        signature: AwbcSignatureId(0),
        capability: None,
        audio: None,
        static_args,
        resources: Vec::new(),
    });
    effect
}

fn constant_string(program: &mut AwbcProgram, value: &str) -> crate::awbc::schema::AwbcConstantId {
    let string =
        AwbcStringId(u32::try_from(program.strings.len()).expect("test string index fits u32"));
    program.strings.push(value.to_owned());
    let constant = crate::awbc::schema::AwbcConstantId(
        u32::try_from(program.constants.len()).expect("test constant index fits u32"),
    );
    program.constants.push(AwbcConstant::String(string));
    constant
}
