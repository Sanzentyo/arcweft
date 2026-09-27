use super::*;

fn int_constant(value: i64) -> AwbcConstant {
    let mut bits = [0; 16];
    bits[..8].copy_from_slice(&value.to_le_bytes());
    AwbcConstant::Int {
        kind: AwbcSignedIntKind::I64,
        bits,
    }
}

fn mut_trait_program(receiver_out: AwbcRegisterId) -> std::sync::Arc<AwbcProgram> {
    let mut program = minimal_program();
    program.strings.push("mut_method".to_owned());
    program.canonicalize_string_table();
    let method_name = AwbcStringId(
        u32::try_from(
            program
                .strings
                .binary_search(&"mut_method".to_owned())
                .unwrap(),
        )
        .unwrap(),
    );
    let int_type = AwbcTypeId(0);
    program.runtime_types = vec![runtime_type(
        0x31,
        AwbcRuntimeTypeShape::Int(AwbcSignedIntKind::I64),
    )];
    program.signatures[0].result = Some(int_type);
    program.signatures.push(AwbcSignature {
        params: vec![int_type],
        result: Some(int_type),
        effects: AwbcEffectSetId(0),
    });
    let temp = |role| AwbcFrameSlot {
        name: None,
        ty: int_type,
        role,
        scope_depth: 0,
    };
    program.frame_layouts[0].slots = vec![
        temp(AwbcFrameSlotRole::Temporary),
        temp(AwbcFrameSlotRole::Temporary),
    ];
    program.frame_layouts.push(AwbcFrameLayout {
        slots: vec![
            temp(AwbcFrameSlotRole::Parameter),
            temp(AwbcFrameSlotRole::Temporary),
        ],
        scopes: Vec::new(),
        max_scope_depth: 0,
    });
    program.constants = vec![int_constant(7), int_constant(10), int_constant(99)];
    program.instructions = vec![
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::CallTraitMethod {
            dst: AwbcRegisterId(1),
            method: AwbcTraitMethodId(0),
            receiver: AwbcRegisterId(0),
            args: Vec::new(),
            receiver_out: Some(receiver_out),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(1),
        },
        AwbcInstruction::Binary {
            dst: AwbcRegisterId(0),
            op: AwbcBinaryOp::Add,
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
        value: Some(AwbcRegisterId(1)),
    };
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(1),
        instructions: AwbcTableRange::new(2, 3),
        terminator: AwbcTerminator::Return {
            value: Some(AwbcRegisterId(1)),
        },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program.functions.push(AwbcFunction {
        public_id: None,
        kind: AwbcFunctionKind::TraitMethod,
        signature: AwbcSignatureId(1),
        frame_layout: AwbcFrameLayoutId(1),
        blocks: AwbcTableRange::new(1, 1),
        entry_block: AwbcBlockId(1),
        flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
    });
    program.trait_methods.push(AwbcTraitMethod {
        public_id: method_name,
        signature: AwbcSignatureId(1),
        function: AwbcFunctionId(1),
        receiver: AwbcTraitReceiverMode::MutRef,
        receiver_state_slot: Some(AwbcRegisterId(0)),
    });
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    std::sync::Arc::new(program)
}

#[test]
fn mut_trait_return_updates_receiver_on_the_same_saved_fiber() {
    for (receiver_out, expected_result) in [(AwbcRegisterId(0), 99), (AwbcRegisterId(1), 17)] {
        let program = mut_trait_program(receiver_out);
        let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).unwrap();
        for _ in 0..8 {
            if fiber.cursor.function == AwbcFunctionId(1) && fiber.cursor.instruction_offset == 2 {
                break;
            }
            let output = super::super::vm::step(
                &program,
                &mut fiber,
                super::super::vm::VmStepOptions {
                    max_instructions: 1,
                },
            )
            .unwrap();
            assert!(matches!(output.exit, super::super::vm::VmExit::Running));
        }
        assert_eq!(fiber.cursor.function, AwbcFunctionId(1));
        assert_eq!(fiber.cursor.instruction_offset, 2);
        assert_eq!(
            fiber
                .active_frame()
                .unwrap()
                .register(AwbcRegisterId(0))
                .unwrap()
                .try_i64(),
            Some(17)
        );
        fiber.budget.remaining = 0;
        assert!(matches!(
            super::super::vm::step(
                &program,
                &mut fiber,
                super::super::vm::VmStepOptions {
                    max_instructions: 1
                }
            )
            .unwrap()
            .exit,
            super::super::vm::VmExit::BudgetYield(_)
        ));
        let snapshot = AwbcFiberStateSnapshot::from_live(&fiber).unwrap();
        let encoded = serde_json::to_vec(&snapshot).unwrap();
        let decoded: AwbcFiberStateSnapshot = serde_json::from_slice(&encoded).unwrap();
        let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&program));
        let mut forged = decoded.clone();
        forged.frames[1].return_to.as_mut().unwrap().destination = Some(AwbcRegisterId(0));
        assert!(
            forged
                .into_live_for_program(&owner)
                .unwrap()
                .validate_for_program(&program)
                .is_err()
        );
        let mut restored = decoded.into_live_for_program(&owner).unwrap();
        restored.validate_for_program(&program).unwrap();
        restored.resume_budget_yield(&program).unwrap();
        restored.replenish_budget();
        let mut cancelled = restored.clone();
        assert!(matches!(
            super::super::vm::cancel_fiber(&mut cancelled).exit,
            super::super::vm::VmExit::Cancelled
        ));
        let output = loop {
            let output = super::super::vm::step(
                &program,
                &mut restored,
                super::super::vm::VmStepOptions {
                    max_instructions: 1,
                },
            )
            .unwrap();
            if !matches!(output.exit, super::super::vm::VmExit::Running) {
                break output;
            }
        };
        assert_eq!(
            output.exit,
            super::super::vm::VmExit::Returned(Some(RuntimeValue::Int(
                crate::value::RuntimeInt::I64(expected_result)
            )))
        );
        let expected_receiver = if receiver_out == AwbcRegisterId(0) {
            17
        } else {
            7
        };
        assert_eq!(
            restored.frames[0]
                .register(AwbcRegisterId(0))
                .unwrap()
                .try_i64(),
            Some(expected_receiver)
        );
        assert_eq!(
            restored.frames[0]
                .register(AwbcRegisterId(1))
                .unwrap()
                .try_i64(),
            Some(expected_result)
        );
    }
}
