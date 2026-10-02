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

fn partial_record_program() -> AwbcProgram {
    let mut program = replacement_program();
    let nominal = AwbcStringId(program.strings.len() as u32);
    program
        .strings
        .extend(["record.Pair".into(), "first".into(), "second".into()]);
    let field =
        |ordinal| crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap();
    program.runtime_types.push(runtime_type(
        92,
        AwbcRuntimeTypeShape::NominalRecord {
            public_id: nominal,
            layout: [9; 32],
            arguments: Vec::new(),
            shape: crate::entry::RuntimeNominalRecordShape::Record,
            fields: (0..2)
                .map(|ordinal| AwbcRecordField {
                    field: field(ordinal),
                    name: Some(AwbcStringId(nominal.0 + 1 + ordinal as u32)),
                    ty: AwbcTypeId(0),
                })
                .collect(),
        },
    ));
    program.signatures[0].result = Some(AwbcTypeId(1));
    program.frame_layouts[0].slots = (0..5)
        .map(|index| AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(if index == 0 { 1 } else { 0 }),
            role: if index == 0 {
                AwbcFrameSlotRole::Local
            } else {
                AwbcFrameSlotRole::Temporary
            },
            scope_depth: 0,
        })
        .collect();
    program.instructions = vec![
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(2),
            constant: AwbcConstantId(1),
        },
        AwbcInstruction::MakeRecord {
            dst: AwbcRegisterId(0),
            ty: AwbcTypeId(1),
            fields: vec![AwbcRegisterId(1), AwbcRegisterId(2)],
        },
        AwbcInstruction::ReadPlace {
            dst: AwbcRegisterId(3),
            root: AwbcRegisterId(0),
            fields: vec![field(0)],
            mode: AwbcPlaceReadMode::Move,
        },
        AwbcInstruction::ReadPlace {
            dst: AwbcRegisterId(4),
            root: AwbcRegisterId(0),
            fields: vec![field(1)],
            mode: AwbcPlaceReadMode::Copy,
        },
        AwbcInstruction::Assign {
            place: AwbcMutablePlace::NominalField {
                base: AwbcRegisterId(0),
                field: 0,
            },
            value: AwbcRegisterId(3),
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 6);
    program.canonicalize_string_table();
    program
}

#[test]
fn partial_record_move_survives_codec_snapshot_and_restore_then_reinitializes() {
    let program = partial_record_program();
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
    assert!(fiber.frames[0].registers[0].as_ref().is_none());
    assert!(!fiber.frames[0].registers[0].is_vacant());
    assert_eq!(fiber.frames[0].registers[0].values().count(), 1);
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
                max_instructions: 8,
            },
        )
        .unwrap();
        let super::super::vm::VmExit::Returned(Some(RuntimeValue::NominalRecord(record))) =
            result.exit
        else {
            panic!("complete record returns after field initialization");
        };
        assert_eq!(
            record.fields(),
            &[RuntimeValue::Bool(true), RuntimeValue::Bool(false)]
        );
        assert_eq!(
            restored.frames[0].registers[4].as_ref(),
            Some(&RuntimeValue::Bool(false))
        );
        restored.restore(checkpoint.clone(), &owner).unwrap();
        assert!(restored.frames[0].registers[0].as_ref().is_none());
        restored.validate_for_program(&program).unwrap();
    }
}

#[test]
fn partial_parameter_storage_is_borrowed_and_transferred_without_losing_its_children() {
    let mut program = partial_record_program();
    program.frame_layouts[0].slots[0].role = AwbcFrameSlotRole::Parameter;
    program.signatures[0].params = vec![AwbcTypeId(1)];
    program.functions[0].input_ownership = vec![AwbcFunctionInputOwnership::default()];
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).unwrap();
    super::super::vm::step(
        &program,
        &mut fiber,
        super::super::vm::VmStepOptions {
            max_instructions: 4,
        },
    )
    .unwrap();
    let borrowed = fiber.function_argument_storage(&program).unwrap();
    assert_eq!(borrowed.len(), 1);
    assert!(borrowed[0].as_ref().is_none());
    assert_eq!(
        borrowed[0].values().collect::<Vec<_>>(),
        vec![&RuntimeValue::Bool(false)]
    );
    let original = borrowed[0].clone();
    let transferred = fiber.take_function_argument_storage(&program).unwrap();
    assert_eq!(transferred, vec![original]);
    assert!(fiber.frames[0].registers[0].is_vacant());
    assert_eq!(
        transferred[0].values().collect::<Vec<_>>(),
        vec![&RuntimeValue::Bool(false)]
    );
}

#[test]
fn mutable_record_field_remains_usable_after_moving_its_sibling() {
    let mut program = partial_record_program();
    program.runtime_types.push(runtime_type(
        93,
        AwbcRuntimeTypeShape::Sequence {
            kind: crate::plan::RuntimePlanSequenceKind::Vec,
            item: AwbcTypeId(0),
        },
    ));
    let mut shape = program.runtime_types[1].shape().clone();
    let AwbcRuntimeTypeShape::NominalRecord { fields, .. } = &mut shape else {
        panic!("record");
    };
    fields[0].ty = AwbcTypeId(2);
    program.runtime_types[1] = runtime_type(92, shape);
    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(2),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    program.instructions = vec![
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(2),
            constant: AwbcConstantId(1),
        },
        AwbcInstruction::MakeSequence {
            dst: AwbcRegisterId(5),
            items: vec![AwbcRegisterId(1)],
        },
        AwbcInstruction::MakeRecord {
            dst: AwbcRegisterId(0),
            ty: AwbcTypeId(1),
            fields: vec![AwbcRegisterId(5), AwbcRegisterId(2)],
        },
        AwbcInstruction::ReadPlace {
            dst: AwbcRegisterId(3),
            root: AwbcRegisterId(0),
            fields: vec![
                crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(1).unwrap(),
            ],
            mode: AwbcPlaceReadMode::Move,
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(2),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::VecPush {
            place: AwbcMutablePlace::NominalField {
                base: AwbcRegisterId(0),
                field: 0,
            },
            value: AwbcRegisterId(2),
        },
        AwbcInstruction::Assign {
            place: AwbcMutablePlace::NominalField {
                base: AwbcRegisterId(0),
                field: 1,
            },
            value: AwbcRegisterId(3),
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, program.instructions.len() as u32);
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    let bytes = program.encode_canonical().unwrap();
    let program = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).unwrap();
    let result = super::super::vm::step(
        &program,
        &mut fiber,
        super::super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .unwrap();
    let super::super::vm::VmExit::Returned(Some(RuntimeValue::NominalRecord(record))) = result.exit
    else {
        panic!("record");
    };
    let RuntimeValue::Seq(sequence) = &record.fields()[0] else {
        panic!("Vec field");
    };
    assert_eq!(sequence.len(), 2);
    assert_eq!(record.fields()[1], RuntimeValue::Bool(false));
}

#[test]
fn verifier_rejects_whole_and_moved_child_reads_after_partial_move() {
    let program = partial_record_program();
    for instruction in [
        AwbcInstruction::Move {
            dst: AwbcRegisterId(3),
            src: AwbcRegisterId(0),
        },
        AwbcInstruction::ReadPlace {
            dst: AwbcRegisterId(4),
            root: AwbcRegisterId(0),
            fields: vec![
                crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
            ],
            mode: AwbcPlaceReadMode::Move,
        },
    ] {
        let mut invalid = program.clone();
        invalid.instructions[4] = instruction;
        assert!(
            invalid
                .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
                .is_err()
        );
    }
}

#[test]
fn corrupt_partial_header_or_child_is_rejected_without_replacing_the_live_fiber() {
    let program = partial_record_program();
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(program.clone()));
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).unwrap();
    super::super::vm::step(
        &program,
        &mut fiber,
        super::super::vm::VmStepOptions {
            max_instructions: 4,
        },
    )
    .unwrap();
    let before = fiber.checkpoint().unwrap();
    let encoded = serde_json::to_value(AwbcFiberStateSnapshot::from_live(&fiber).unwrap()).unwrap();
    let mut wrong_header = encoded.clone();
    wrong_header["frames"][0]["registers"][0]["Record"]["header"]["Nominal"]["layout"] =
        serde_json::to_value(crate::entry::TypeLayoutHash::from_bytes([8; 32])).unwrap();
    let mut wrong_child = encoded;
    wrong_child["frames"][0]["registers"][0]["Record"]["fields"][1] =
        serde_json::json!({ "Initialized": { "String": "wrong field type" } });
    for malformed in [wrong_header, wrong_child] {
        let snapshot: AwbcFiberStateSnapshot = serde_json::from_value(malformed).unwrap();
        assert!(fiber.replace_from_snapshot(snapshot, &owner).is_err());
        assert_eq!(fiber.checkpoint().unwrap(), before);
        fiber.validate_for_program(&program).unwrap();
    }
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
    assert_eq!(
        fiber.frames[0].registers[0].as_ref().cloned(),
        Some(RuntimeValue::Bool(true))
    );
    assert_eq!(
        fiber.frames[0].registers[1].as_ref().cloned(),
        Some(RuntimeValue::Bool(true))
    );
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
        assert_eq!(restored.frames[0].registers[2].as_ref().cloned(), None);
        restored.restore(checkpoint.clone(), &owner).unwrap();
        assert_eq!(
            restored.frames[0].registers[0].as_ref().cloned(),
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
    assert!(fiber.frames[0].registers[0].is_vacant());
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
        assert!(restored.frames[0].registers[0].is_vacant());
        assert_eq!(restored.frames[0].registers[2].as_ref().cloned(), None);
        assert_eq!(
            restored.frames[0].registers[1].as_ref().cloned(),
            Some(RuntimeValue::Bool(true))
        );
        restored.restore(checkpoint.clone(), &owner).unwrap();
        restored.validate_for_program(&program).unwrap();
    }
}
