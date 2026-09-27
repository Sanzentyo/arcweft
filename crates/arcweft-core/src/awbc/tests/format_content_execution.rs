use super::*;

#[derive(Clone, Copy)]
enum ValueThunk {
    Succeeds,
    RecoversFromDivisionByZero,
    Traps,
}

fn format_program(value_thunk: ValueThunk) -> AwbcProgram {
    let mut program = super::minimal_program();
    let content_owner = crate::value::RuntimeDialogueOpaqueRole::Content.exact_owner();
    let producer = content_owner.producer().as_str().to_owned();
    program
        .strings
        .extend([producer.clone(), "number".to_owned()]);
    program.canonicalize_string_table();
    let producer = AwbcStringId(
        u32::try_from(
            program
                .strings
                .binary_search(&producer)
                .expect("Content producer string"),
        )
        .expect("Content producer string index"),
    );
    let style = AwbcStringId(
        u32::try_from(
            program
                .strings
                .binary_search(&"number".to_owned())
                .expect("Style string"),
        )
        .expect("Style string index"),
    );

    let content_type = AwbcTypeId(0);
    let int_type = AwbcTypeId(1);
    let string_type = AwbcTypeId(2);
    program.runtime_types = vec![
        AwbcRuntimeType::new(
            content_owner.semantic_identity(),
            AwbcRuntimeTypeShape::Opaque {
                producer,
                admission: content_owner.admission(),
                value_class: content_owner.value_class(),
                persistence: content_owner.persistence(),
                arguments: Vec::new(),
            },
        ),
        AwbcRuntimeType::new(
            RuntimeCheckedType::Signed(crate::value::RuntimeSignedIntWidth::I64)
                .semantic_identity_digest(),
            AwbcRuntimeTypeShape::Int(AwbcSignedIntKind::I64),
        ),
        AwbcRuntimeType::new(
            RuntimeCheckedType::String.semantic_identity_digest(),
            AwbcRuntimeTypeShape::String,
        ),
    ];

    program.signatures[0].result = Some(content_type);
    program.frame_layouts[0].slots = vec![
        AwbcFrameSlot {
            name: None,
            ty: content_type,
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
    program.content_templates = vec![AwbcDialogueContentTemplate {
        id: crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
            .expect("template ID"),
        digest: crate::entry::RuntimeDialogueContentTemplateDigest::from_bytes([0x19; 32]),
        slots: vec![AwbcDialogueContentSlot {
            slot: crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0)
                .expect("slot ID"),
            role: AwbcDialogueValueRole::Formatted,
            semantic_type: content_type,
        }],
        effects: Vec::new(),
    }];

    let int_constant = |value: i64| {
        let mut bits = [0; 16];
        bits[..8].copy_from_slice(&value.to_le_bytes());
        AwbcConstant::Int {
            kind: AwbcSignedIntKind::I64,
            bits,
        }
    };
    let denominator = match value_thunk {
        ValueThunk::RecoversFromDivisionByZero => 0,
        ValueThunk::Succeeds | ValueThunk::Traps => 1,
    };
    program.constants = vec![
        int_constant(17),
        int_constant(denominator),
        AwbcConstant::String(style),
    ];
    program.instructions = vec![
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::FormatContent {
            destination: AwbcRegisterId(0),
            template: crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
                .expect("template ID"),
            operands: vec![
                AwbcFormatOperand {
                    parameter: crate::value::RuntimeFmtParameterId::Value,
                    function: AwbcFunctionId(1),
                    captures: vec![AwbcRegisterId(1)],
                },
                AwbcFormatOperand {
                    parameter: crate::value::RuntimeFmtParameterId::Style,
                    function: AwbcFunctionId(2),
                    captures: vec![AwbcRegisterId(1)],
                },
            ],
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(1),
        },
        AwbcInstruction::Binary {
            dst: AwbcRegisterId(2),
            op: AwbcBinaryOp::Div,
            lhs: AwbcRegisterId(0),
            rhs: AwbcRegisterId(1),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(2),
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 2);
    program.blocks[0].terminator = AwbcTerminator::Return {
        value: Some(AwbcRegisterId(0)),
    };
    program.signatures.extend([
        AwbcSignature {
            params: vec![int_type],
            result: Some(int_type),
            effects: AwbcEffectSetId(0),
        },
        AwbcSignature {
            params: vec![int_type],
            result: Some(string_type),
            effects: AwbcEffectSetId(0),
        },
    ]);
    program.frame_layouts.extend([
        AwbcFrameLayout {
            slots: vec![
                AwbcFrameSlot {
                    name: None,
                    ty: int_type,
                    role: AwbcFrameSlotRole::Parameter,
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
            ],
            scopes: Vec::new(),
            max_scope_depth: 0,
        },
        AwbcFrameLayout {
            slots: vec![
                AwbcFrameSlot {
                    name: None,
                    ty: int_type,
                    role: AwbcFrameSlotRole::Parameter,
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
    ]);
    program.functions.extend([
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Synthetic,
            signature: AwbcSignatureId(1),
            frame_layout: AwbcFrameLayoutId(1),
            blocks: AwbcTableRange::new(1, 1),
            entry_block: AwbcBlockId(1),
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        },
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Synthetic,
            signature: AwbcSignatureId(2),
            frame_layout: AwbcFrameLayoutId(2),
            blocks: AwbcTableRange::new(2, 1),
            entry_block: AwbcBlockId(2),
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        },
    ]);
    program.blocks.extend([
        AwbcBlock {
            owner: AwbcFunctionId(1),
            instructions: AwbcTableRange::new(2, 2),
            terminator: match value_thunk {
                ValueThunk::Traps => AwbcTerminator::Trap {
                    code: AwbcTrapCode::ExplicitPanic,
                    message: None,
                },
                ValueThunk::Succeeds | ValueThunk::RecoversFromDivisionByZero => {
                    AwbcTerminator::Return {
                        value: Some(AwbcRegisterId(2)),
                    }
                }
            },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
        AwbcBlock {
            owner: AwbcFunctionId(2),
            instructions: AwbcTableRange::new(4, 1),
            terminator: AwbcTerminator::Return {
                value: Some(AwbcRegisterId(1)),
            },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
    ]);
    program
}

fn verify(program: AwbcProgram) -> std::sync::Arc<AwbcProgram> {
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("FormatContent execution fixture verifies");
    std::sync::Arc::new(program)
}

fn context(program: &std::sync::Arc<AwbcProgram>) -> crate::awbc::vm::VmExecutionContext {
    let artifact = crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0x5a; 32])
        .expect("test artifact fingerprint is valid");
    crate::awbc::vm::VmExecutionContext::for_program(artifact, std::sync::Arc::clone(program))
}

fn step(
    program: &std::sync::Arc<AwbcProgram>,
    fiber: &mut FiberState,
    max_instructions: u64,
) -> crate::awbc::vm::VmStepOutput {
    crate::awbc::vm::step_with_host_context(
        program,
        fiber,
        crate::awbc::vm::VmStepOptions { max_instructions },
        &context(program),
        &mut crate::awbc::vm::RejectingVmHost,
    )
    .expect("verified formatter program executes")
}

fn fiber(program: &AwbcProgram) -> FiberState {
    FiberState::for_entry(program, AwbcEntryId(0), 1, 64).expect("formatter fiber initializes")
}

fn returned_content(
    output: crate::awbc::vm::VmStepOutput,
) -> crate::value::RuntimeDialogueContentValue {
    let crate::awbc::vm::VmExit::Returned(Some(value)) = output.exit else {
        panic!("formatter returns a Content value: {:?}", output.exit);
    };
    crate::value::RuntimeDialogueContentValue::try_from_runtime_value(&value)
        .expect("formatter result has the exact Content owner")
}

fn run_to_completion(
    program: &std::sync::Arc<AwbcProgram>,
    fiber: &mut FiberState,
) -> crate::awbc::vm::VmStepOutput {
    for _ in 0..16 {
        let output = step(program, fiber, 64);
        if !matches!(output.exit, crate::awbc::vm::VmExit::Running) {
            return output;
        }
    }
    panic!("verified formatter did not finish within the instruction bound");
}

#[test]
fn format_content_vm_executes_value_then_style_and_returns_formatted_content() {
    let program = verify(format_program(ValueThunk::Succeeds));
    let mut fiber = fiber(&program);
    let content = returned_content(run_to_completion(&program, &mut fiber));

    let slot = crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0).expect("slot ID");
    let Some(crate::value::RuntimeDialogueContentBinding::Formatted { value, .. }) =
        content.binding(slot)
    else {
        panic!("Content has the one Formatted slot");
    };
    assert_eq!(
        value.outcome(),
        &crate::value::RuntimeDialogueFormattedOutcome::Success {
            value: crate::value::RuntimeDialogueFormattedSuccess::Text("17".to_owned()),
            color: None,
        }
    );
}

#[test]
fn format_content_vm_evaluates_style_after_recoverable_value_failure_and_resumes_snapshot() {
    let program = verify(format_program(ValueThunk::RecoversFromDivisionByZero));
    let mut fiber = fiber(&program);
    for _ in 0..8 {
        if fiber.frames[0]
            .format
            .as_ref()
            .is_some_and(|state| state.next_operand() == 1)
        {
            break;
        }
        let output = step(&program, &mut fiber, 1);
        assert!(matches!(output.exit, crate::awbc::vm::VmExit::Running));
    }

    let state = fiber.frames[0]
        .format
        .as_ref()
        .expect("formatter state remains at its instruction site");
    assert_eq!(state.next_operand(), 1);
    assert_eq!(state.values(), &[None, None]);
    assert!(
        state
            .first_recoverable()
            .is_some_and(|reason| reason.contains("division by zero"))
    );

    // Suspend at the exact formatter site after the failed Value thunk, then
    // restore its staged state before the later Style thunk runs.
    fiber.budget.remaining = 0;
    let output = step(&program, &mut fiber, 1);
    assert!(matches!(
        output.exit,
        crate::awbc::vm::VmExit::BudgetYield(_)
    ));
    let snapshot =
        AwbcFiberStateSnapshot::from_live(&fiber).expect("recoverable formatter state snapshots");
    let encoded = serde_json::to_vec(&snapshot).expect("snapshot serializes");
    let decoded: AwbcFiberStateSnapshot =
        serde_json::from_slice(&encoded).expect("snapshot deserializes");
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&program));
    let mut restored = decoded
        .into_live_for_program(&owner)
        .expect("formatter snapshot restores for the exact program");
    restored
        .resume_budget_yield(&program)
        .expect("formatter resumes at its exact instruction");
    restored.replenish_budget();

    let style_start = step(&program, &mut restored, 1);
    assert!(matches!(style_start.exit, crate::awbc::vm::VmExit::Running));
    assert_eq!(restored.cursor.function, AwbcFunctionId(2));
    let _ = step(&program, &mut restored, 1);
    let _ = step(&program, &mut restored, 1);
    let resumed_state = restored.frames[0]
        .format
        .as_ref()
        .expect("formatter state waits for final construction");
    assert_eq!(resumed_state.next_operand(), 2);
    assert_eq!(
        resumed_state.values()[1],
        Some(crate::value::RuntimeValue::String("number".to_owned()))
    );

    let content = returned_content(run_to_completion(&program, &mut restored));
    let slot = crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0).expect("slot ID");
    let Some(crate::value::RuntimeDialogueContentBinding::Formatted { value, .. }) =
        content.binding(slot)
    else {
        panic!("Content has the one Formatted slot");
    };
    let crate::value::RuntimeDialogueFormattedOutcome::Failure { reason, .. } = value.outcome()
    else {
        panic!("recoverable Value evaluation produces formatted failure data");
    };
    assert!(reason.contains("division by zero"));
}

#[test]
fn format_content_vm_propagates_a_fatal_operand_trap_without_running_later_style() {
    let program = verify(format_program(ValueThunk::Traps));
    let mut fiber = fiber(&program);
    let output = run_to_completion(&program, &mut fiber);
    let crate::awbc::vm::VmExit::Trapped(trap) = output.exit else {
        panic!("fatal Value operand trap propagates from FormatContent");
    };
    assert_eq!(trap.code, AwbcTrapCode::ExplicitPanic);
    assert!(!output.observations.iter().any(|observation| matches!(
        observation,
        crate::awbc::vm::VmObservation::Instruction {
            function: AwbcFunctionId(2),
            ..
        }
    )));
}
