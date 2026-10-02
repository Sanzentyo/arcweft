use super::*;

fn replacement_program() -> AwbcProgram {
    let mut program = minimal_program();
    program.runtime_types = vec![runtime_type(91, AwbcRuntimeTypeShape::Bool)];
    program.constants = vec![AwbcConstant::Bool(true), AwbcConstant::Bool(false)];
    program.signatures[0].result = Some(AwbcTypeId(0));
    program.frame_layouts[0] = AwbcFrameLayout {
        scopes: vec![AwbcScopeDefinition {
            parent: None,
            identity: crate::scope::RuntimeScopeIdentity::Anonymous,
        }],
        slots: [
            (AwbcFrameSlotRole::Local, 0),
            (AwbcFrameSlotRole::Temporary, 0),
            (AwbcFrameSlotRole::Temporary, 1),
        ]
        .into_iter()
        .map(|(role, scope_depth)| AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(0),
            role,
            scope_depth,
        })
        .collect(),
        max_scope_depth: 1,
    };
    program.instructions = vec![
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::CopyValue {
            dst: AwbcRegisterId(1),
            src: AwbcRegisterId(0),
        },
        AwbcInstruction::EnterScope {
            scope: AwbcScopeId(0),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(2),
            constant: AwbcConstantId(1),
        },
        AwbcInstruction::Assign {
            place: AwbcMutablePlace::Local(AwbcRegisterId(0)),
            value: AwbcRegisterId(2),
        },
        AwbcInstruction::ExitScope {
            scope: AwbcScopeId(0),
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 6);
    program.blocks[0].terminator = AwbcTerminator::Return {
        value: Some(AwbcRegisterId(0)),
    };
    program
}

#[test]
fn live_outer_local_survives_codec_snapshot_and_checkpoint_then_replaces() {
    let program = replacement_program();
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    let bytes = program.encode_canonical().unwrap();
    let program = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
    assert_eq!(program.encode_canonical().unwrap(), bytes);
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).unwrap();
    super::super::vm::step(
        &program,
        &mut fiber,
        super::super::vm::VmStepOptions {
            max_instructions: 3,
        },
    )
    .unwrap();
    assert_eq!(fiber.frames[0].registers[0], Some(RuntimeValue::Bool(true)));
    assert_eq!(fiber.frames[0].registers[1], Some(RuntimeValue::Bool(true)));
    let snapshot = AwbcFiberStateSnapshot::from_live(&fiber).unwrap();
    let bytes = serde_json::to_vec(&snapshot).unwrap();
    let snapshot: AwbcFiberStateSnapshot = serde_json::from_slice(&bytes).unwrap();
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(program.clone()));
    let mut restored = snapshot.into_live_for_program(&owner).unwrap();
    restored.validate_for_program(&program).unwrap();
    let checkpoint = restored.checkpoint().unwrap();
    for _ in 0..2 {
        let result = super::super::vm::step(
            &program,
            &mut restored,
            super::super::vm::VmStepOptions {
                max_instructions: 4,
            },
        )
        .unwrap();
        assert_eq!(
            result.exit,
            super::super::vm::VmExit::Returned(Some(RuntimeValue::Bool(false)))
        );
        assert_eq!(restored.frames[0].registers[2], None);
        restored.restore(checkpoint.clone(), &owner).unwrap();
        assert_eq!(
            restored.frames[0].registers[0],
            Some(RuntimeValue::Bool(true))
        );
        restored.validate_for_program(&program).unwrap();
    }
}

#[test]
fn local_assignment_rejects_source_alias_and_incompatible_type() {
    let program = replacement_program();
    let mut aliased = program.clone();
    aliased.instructions[4] = AwbcInstruction::Assign {
        place: AwbcMutablePlace::Local(AwbcRegisterId(2)),
        value: AwbcRegisterId(2),
    };
    assert!(
        aliased
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .is_err()
    );
    let mut wrong_type = program;
    wrong_type
        .runtime_types
        .push(runtime_type(92, AwbcRuntimeTypeShape::Unit));
    wrong_type.frame_layouts[0].slots[0].ty = AwbcTypeId(1);
    wrong_type.constants[0] = AwbcConstant::Unit;
    wrong_type.frame_layouts[0].slots[1].ty = AwbcTypeId(1);
    wrong_type.signatures[0].result = Some(AwbcTypeId(1));
    assert!(
        wrong_type
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .is_err()
    );
}

#[test]
fn assignment_after_move_reinitializes_after_codec_restore_and_checkpoint() {
    let mut program = replacement_program();
    program.instructions[1] = AwbcInstruction::Move {
        dst: AwbcRegisterId(1),
        src: AwbcRegisterId(0),
    };
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    let bytes = program.encode_canonical().unwrap();
    let program = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
    assert_eq!(program.encode_canonical().unwrap(), bytes);
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).unwrap();
    super::super::vm::step(
        &program,
        &mut fiber,
        super::super::vm::VmStepOptions {
            max_instructions: 4,
        },
    )
    .unwrap();
    assert!(fiber.frames[0].registers[0].is_none());
    for _ in 0..4 {
        if fiber.cursor.instruction_offset == 4 {
            break;
        }
        super::super::vm::step(
            &program,
            &mut fiber,
            super::super::vm::VmStepOptions {
                max_instructions: 1,
            },
        )
        .unwrap();
    }
    assert_eq!(fiber.cursor.instruction_offset, 4);
    let snapshot = AwbcFiberStateSnapshot::from_live(&fiber).unwrap();
    let bytes = serde_json::to_vec(&snapshot).unwrap();
    let snapshot: AwbcFiberStateSnapshot = serde_json::from_slice(&bytes).unwrap();
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(program.clone()));
    let mut restored = snapshot.into_live_for_program(&owner).unwrap();
    restored.validate_for_program(&program).unwrap();
    let checkpoint = restored.checkpoint().unwrap();
    for _ in 0..2 {
        let result = super::super::vm::step(
            &program,
            &mut restored,
            super::super::vm::VmStepOptions {
                max_instructions: 4,
            },
        )
        .unwrap();
        assert_eq!(
            result.exit,
            super::super::vm::VmExit::Returned(Some(RuntimeValue::Bool(false)))
        );
        assert!(restored.frames[0].registers[0].is_none());
        assert_eq!(restored.frames[0].registers[2], None);
        assert_eq!(
            restored.frames[0].registers[1],
            Some(RuntimeValue::Bool(true))
        );
        restored.restore(checkpoint.clone(), &owner).unwrap();
        restored.validate_for_program(&program).unwrap();
    }
}
