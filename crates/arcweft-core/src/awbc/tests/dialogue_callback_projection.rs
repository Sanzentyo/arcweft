use super::*;
use std::sync::Arc;

fn fixture() -> Arc<AwbcProgram> {
    let mut program = minimal_program();
    let unit = AwbcTypeId(0);
    let boolean = AwbcTypeId(1);
    let callback = AwbcTypeId(2);
    let first = AwbcTypeId(3);
    program.runtime_types = vec![
        runtime_type(241, AwbcRuntimeTypeShape::Unit),
        runtime_type(242, AwbcRuntimeTypeShape::Bool),
        runtime_type(
            243,
            AwbcRuntimeTypeShape::Function {
                contract: Default::default(),
                parameters: vec![],
                result: unit,
            },
        ),
        runtime_type(
            244,
            AwbcRuntimeTypeShape::Function {
                contract: Default::default(),
                parameters: vec![boolean],
                result: callback,
            },
        ),
    ];
    program.signatures.push(AwbcSignature {
        params: vec![boolean],
        result: Some(unit),
        effects: AwbcEffectSetId(0),
    });
    program.frame_layouts.push(AwbcFrameLayout {
        scopes: vec![],
        slots: vec![
            AwbcFrameSlot {
                name: None,
                ty: boolean,
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: unit,
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
        ],
        max_scope_depth: 0,
    });
    program.functions.push(AwbcFunction {
        definition: crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
            [245; 32],
        ),
        semantic_role: crate::plan::RuntimeFunctionSemanticRole::Ordinary,
        public_id: None,
        kind: AwbcFunctionKind::Ordinary,
        signature: AwbcSignatureId(1),
        type_context: None,
        input_ownership: vec![AwbcFunctionInputOwnership::parameter(
            manual_awbc_formal_identity("arcweft-core.awbc.dialogue_callback_projection.input.v1"),
            0,
            crate::plan::RuntimeFunctionParameterPassing::Value,
        )],
        frame_layout: AwbcFrameLayoutId(1),
        blocks: AwbcTableRange::new(1, 1),
        entry_block: AwbcBlockId(1),
        flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
    });
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(1),
        instructions: AwbcTableRange::new(0, 1),
        terminator: AwbcTerminator::Return {
            value: Some(AwbcRegisterId(1)),
        },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program.constants.push(AwbcConstant::Unit);
    program.instructions.push(AwbcInstruction::LoadConst {
        dst: AwbcRegisterId(1),
        constant: AwbcConstantId(0),
    });
    let coordinate = RuntimeCallableParameterCoordinate {
        group: 0,
        parameter: 0,
    };
    program.callable_states = vec![
        RuntimeCallableStateDefinition {
            function_type: first,
            origin: callable_state_id(0),
            position: RuntimeCallablePosition::Unapplied,
            retained: Box::new([]),
            parameters: Box::new([RuntimeCallableParameterInput {
                coordinate,
                kind: RuntimeCallableParameterKind::Fixed,
                abi_ty: boolean,
                binding_ty: boolean,
            }]),
            result: callback,
            attached: RuntimeCallableAttachedContract::None,
            transition: RuntimeCallableTransition::Retain {
                state: callable_state_id(1),
                values: Box::new([RuntimeCallableInputSource::Argument { position: 0 }]),
            },
            partials: Box::new([]),
        },
        RuntimeCallableStateDefinition {
            function_type: callback,
            origin: callable_state_id(0),
            position: RuntimeCallablePosition::AfterGroup { completed: 0 },
            retained: Box::new([RuntimeCallableRetainedInput {
                role: RuntimeCallableRetainedRole::Parameter(coordinate),
                ty: boolean,
            }]),
            parameters: Box::new([]),
            result: unit,
            attached: RuntimeCallableAttachedContract::None,
            transition: RuntimeCallableTransition::Invoke {
                function: AwbcFunctionId(1),
                captures: Box::new([]),
                arguments: Box::new([RuntimeCallableInputSource::Retained { position: 0 }]),
            },
            partials: Box::new([]),
        },
    ];
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    let bytes = program.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
    assert_eq!(decoded, program);
    Arc::new(decoded)
}

#[test]
fn dialogue_callback_projects_retained_parameter_into_body_argument_and_executes_after_codec_round_trip()
 {
    let program = fixture();
    let callback = crate::awbc::vm::dialogue_effect_callable(
        &program,
        RuntimeProgramOwner::Awbc(Arc::clone(&program)),
        callable_state_id(1),
        &[AwbcTypeId(1)],
        vec![RuntimeValue::Bool(true)],
    )
    .unwrap();
    let prepared =
        crate::awbc::fiber::validate_runtime_callable_activation(&program, &callback).unwrap();
    let instance = crate::runtime_id::RuntimeFiberInstanceId::from_allocated(
        std::num::NonZeroU64::new(1).unwrap(),
    );
    let mut fiber = FiberState::for_callable_callback_prepared(
        &program,
        callback,
        prepared,
        crate::awbc::fiber::FiberCreationContext::new(
            instance,
            1,
            1,
            crate::runtime_id::RuntimePresentationBankId::Root,
        ),
    );
    assert_eq!(
        fiber.frames[0].registers[0].as_ref(),
        Some(&RuntimeValue::Bool(true))
    );
    let output = crate::awbc::vm::step(
        &program,
        &mut fiber,
        crate::awbc::vm::VmStepOptions {
            max_instructions: 1,
        },
    )
    .unwrap();
    assert_eq!(output.executed, 1);
    assert_eq!(output.exit, crate::awbc::vm::VmExit::Running);
    assert_eq!(fiber.status, FiberStatus::Running);
    let yielded = crate::awbc::vm::step(
        &program,
        &mut fiber,
        crate::awbc::vm::VmStepOptions {
            max_instructions: 1,
        },
    )
    .unwrap();
    assert_eq!(yielded.executed, 0);
    assert!(matches!(
        yielded.exit,
        crate::awbc::vm::VmExit::BudgetYield(_)
    ));
    assert_eq!(fiber.status, FiberStatus::Suspended);
    let owner = RuntimeProgramOwner::Awbc(Arc::clone(&program));
    let snapshot = AwbcFiberStateSnapshot::from_live(
        &fiber,
        &mut crate::value::RuntimeRetainedValueAdmission::for_program(
            (owner).clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        ),
    )
    .unwrap();
    let bytes = serde_json::to_vec(&snapshot).unwrap();
    let restored: AwbcFiberStateSnapshot = serde_json::from_slice(&bytes).unwrap();
    let mut fiber = restored.into_live_for_program(&owner).unwrap();
    assert_eq!(
        fiber.frames[0].registers[0].as_ref(),
        Some(&RuntimeValue::Bool(true))
    );
    fiber.replenish_budget();
    fiber.resume_budget_yield(&program).unwrap();
    let output = crate::awbc::vm::step(
        &program,
        &mut fiber,
        crate::awbc::vm::VmStepOptions {
            max_instructions: 1,
        },
    )
    .unwrap();
    assert_eq!(output.executed, 1);
    assert_eq!(
        output.exit,
        crate::awbc::vm::VmExit::Returned(Some(RuntimeValue::Unit))
    );
    assert_eq!(fiber.status, FiberStatus::Returned);
}

#[test]
fn dialogue_callback_rejects_foreign_owner_wrong_manifest_and_wrong_retained_value() {
    let program = fixture();
    let foreign = Arc::new((*program).clone());
    assert!(
        crate::awbc::vm::dialogue_effect_callable(
            &program,
            RuntimeProgramOwner::Awbc(foreign),
            callable_state_id(1),
            &[AwbcTypeId(1)],
            vec![RuntimeValue::Bool(true)]
        )
        .is_err()
    );
    assert!(
        crate::awbc::vm::dialogue_effect_callable(
            &program,
            RuntimeProgramOwner::Awbc(Arc::clone(&program)),
            callable_state_id(1),
            &[AwbcTypeId(0)],
            vec![RuntimeValue::Bool(true)]
        )
        .is_err()
    );
    assert!(
        crate::awbc::vm::dialogue_effect_callable(
            &program,
            RuntimeProgramOwner::Awbc(Arc::clone(&program)),
            callable_state_id(1),
            &[AwbcTypeId(1)],
            vec![RuntimeValue::Unit]
        )
        .is_err()
    );
}
