//! Ordinary omission is a canonical binding, separate from attached Content.

use super::*;

fn program(present: bool) -> AwbcProgram {
    let mut program = project_call_fixed_program();
    let some = AwbcStringId(u32::try_from(program.strings.len()).unwrap());
    program.strings.push("Some".to_owned());
    let none = AwbcStringId(u32::try_from(program.strings.len()).unwrap());
    program.strings.push("None".to_owned());
    program.runtime_types.extend([
        runtime_type(3, AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(0)])),
        runtime_type(
            4,
            AwbcRuntimeTypeShape::Variant {
                owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Option),
                arguments: Vec::new(),
                cases: vec![
                    AwbcVariantCase {
                        name: some,
                        payload: Some(AwbcTypeId(2)),
                    },
                    AwbcVariantCase {
                        name: none,
                        payload: None,
                    },
                ],
            },
        ),
    ]);
    program
        .constants
        .push(AwbcConstant::Tuple(vec![AwbcConstantId(0)]));
    let identity = if present {
        crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionSome
    } else {
        crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionNone
    };
    let (case, _) = identity.owner().resolve_case(identity).unwrap();
    program.constants.push(AwbcConstant::Variant {
        ty: AwbcTypeId(3),
        case,
        payload: present.then_some(AwbcConstantId(1)),
    });
    let input = &mut program.callable_states[0].parameters[0];
    input.kind = RuntimeCallableParameterKind::Optional;
    input.binding_ty = AwbcTypeId(3);
    program.signatures[1].params[0] = AwbcTypeId(3);
    program.functions[1].input_ownership = vec![AwbcFunctionInputOwnership::parameter(
        manual_awbc_formal_identity("arcweft-core.fixture.optional-callable-binding.input"),
        0,
        crate::plan::RuntimeFunctionParameterPassing::Value,
    )];
    program.frame_layouts[0].slots[1].ty = AwbcTypeId(3);
    program.frame_layouts[1].slots[0].ty = AwbcTypeId(3);
    program.frame_layouts[1].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(0),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    program.instructions[1] = AwbcInstruction::LoadConst {
        dst: AwbcRegisterId(1),
        constant: AwbcConstantId(2),
    };
    program.instructions[2] = AwbcInstruction::LoadConst {
        dst: AwbcRegisterId(1),
        constant: AwbcConstantId(0),
    };
    program.blocks[2].terminator = AwbcTerminator::Return {
        value: Some(AwbcRegisterId(1)),
    };
    program.canonicalize_string_table();
    program
}

#[test]
fn optional_callable_binding_survives_canonical_wire_and_executes_present_and_omitted() {
    for present in [false, true] {
        let original = program(present);
        original
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .unwrap();
        let bytes = original.encode_canonical().unwrap();
        let decoded = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
        assert_eq!(decoded, original);
        let program = std::sync::Arc::new(decoded);
        let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).unwrap();
        let output = step_with_callable_context(
            &program,
            &mut fiber,
            super::super::vm::VmStepOptions {
                max_instructions: 3,
            },
        )
        .unwrap();
        assert_eq!(output.exit, super::super::vm::VmExit::Running);
        assert_eq!(fiber.cursor.function, AwbcFunctionId(1));
        let expected = if present {
            RuntimeValue::option_some(RuntimeValue::Unit)
        } else {
            RuntimeValue::option_none()
        };
        assert_eq!(
            fiber.frames.last().unwrap().registers[0].as_ref(),
            Some(&expected)
        );
        let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&program));
        let saved = AwbcFiberStateSnapshot::from_live(
            &fiber,
            &mut crate::value::RuntimeRetainedValueAdmission::for_program(
                owner.clone(),
                crate::entry::RuntimeSchemaLimits::engine_default(),
            ),
        )
        .unwrap();
        let bytes = serde_json::to_vec(&saved).unwrap();
        let saved: AwbcFiberStateSnapshot = serde_json::from_slice(&bytes).unwrap();
        fiber = saved.into_live_for_program(&owner).unwrap();
        fiber.validate_for_program(&program).unwrap();
        let output = step_with_callable_context(
            &program,
            &mut fiber,
            super::super::vm::VmStepOptions {
                max_instructions: 16,
            },
        )
        .unwrap();
        assert_eq!(output.exit, super::super::vm::VmExit::Returned(None));
    }
}

#[test]
fn optional_callable_arrow_wraps_present_value_before_entering_the_body() {
    let mut program = program(false);
    program.frame_layouts[0].slots[1].ty = AwbcTypeId(0);
    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(0),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    program.instructions[1] = AwbcInstruction::LoadConst {
        dst: AwbcRegisterId(1),
        constant: AwbcConstantId(0),
    };
    program.instructions.insert(
        2,
        AwbcInstruction::ApplyGroup {
            dst: AwbcRegisterId(2),
            callee: AwbcRegisterId(0),
            args: vec![AwbcRegisterId(1)],
        },
    );
    program.blocks[0].instructions = AwbcTableRange::new(0, 3);
    program.blocks[0].terminator = AwbcTerminator::Jump {
        target: AwbcBlockId(1),
    };
    program.blocks[2].instructions = AwbcTableRange::new(3, 1);
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    let program = std::sync::Arc::new(program);
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).unwrap();
    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::super::vm::VmStepOptions {
            max_instructions: 3,
        },
    )
    .unwrap();
    assert_eq!(output.exit, super::super::vm::VmExit::Running);
    assert_eq!(fiber.cursor.function, AwbcFunctionId(1));
    assert_eq!(
        fiber.frames.last().unwrap().registers[0].as_ref(),
        Some(&RuntimeValue::option_some(RuntimeValue::Unit))
    );
    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .unwrap();
    assert_eq!(output.exit, super::super::vm::VmExit::Returned(None));
}

#[test]
fn optional_callable_binding_rejects_scalar_binding_and_unwrapped_group_operand() {
    let mut scalar_binding = program(false);
    scalar_binding.callable_states[0].parameters[0].binding_ty = AwbcTypeId(0);
    let error = scalar_binding
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("callable parameter ABI/binding shape is invalid"),
        "{error}"
    );
    let mut raw_operand = program(false);
    raw_operand.frame_layouts[0].slots[1].ty = AwbcTypeId(0);
    raw_operand.instructions[1] = AwbcInstruction::LoadConst {
        dst: AwbcRegisterId(1),
        constant: AwbcConstantId(0),
    };
    assert!(
        raw_operand
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .is_err()
    );
}
