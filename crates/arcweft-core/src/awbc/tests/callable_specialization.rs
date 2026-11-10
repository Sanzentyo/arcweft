use super::*;
use crate::effect::RuntimeArtifactFingerprint;
use crate::effect_row::{EffectFormula, EffectPredicate};
use crate::plan::{
    RuntimeArrayLength, RuntimeCallableParameterCoordinate, RuntimeCallableParameterInput,
    RuntimeCallableSpecializationDefinition, RuntimeCallableSpecializationState,
    RuntimeFunctionSpecializationArguments, RuntimeFunctionTypeContract, RuntimeTypeBinder,
    RuntimeTypeScope,
};
use crate::runtime_id::RuntimeCallableSpecializationId;
use crate::value::RuntimeValue;
use std::sync::Arc;

fn specialization_id(index: usize) -> RuntimeCallableSpecializationId {
    RuntimeCallableSpecializationId::from_zero_based(index)
        .expect("test callable-specialization index is valid")
}

fn specialization_program() -> AwbcProgram {
    let mut program = minimal_program();
    let binder = RuntimeTypeBinder::new(1, 0, 0);
    let generic_scope = RuntimeTypeScope::root()
        .enter(binder)
        .expect("one type binder is within the scope limit");
    let generic_reference = crate::plan::RuntimeBoundTypeReference::from_coordinates(0, 0);
    let generic_function = RuntimeFunctionTypeContract::new(
        binder,
        EffectPredicate::unconstrained(),
        EffectFormula::empty(),
    );
    let concrete_function = RuntimeFunctionTypeContract::default();

    program.runtime_types = vec![
        AwbcRuntimeType::unit(),
        AwbcRuntimeType::dynamic(),
        AwbcRuntimeType::new(
            RuntimeSemanticTypeId::from_bytes([0x52; 32]),
            AwbcRuntimeTypeShape::BoundType(generic_reference),
        )
        .with_scope(generic_scope),
        runtime_type(0x53, AwbcRuntimeTypeShape::UInt(AwbcUnsignedIntKind::U32)),
        runtime_type(
            0x54,
            AwbcRuntimeTypeShape::Function {
                contract: generic_function,
                parameters: vec![AwbcTypeId(2)],
                result: AwbcTypeId(2),
            },
        ),
        runtime_type(
            0x55,
            AwbcRuntimeTypeShape::Function {
                contract: concrete_function,
                parameters: vec![AwbcTypeId(3)],
                result: AwbcTypeId(3),
            },
        ),
    ];
    program.signatures = vec![
        AwbcSignature {
            params: Vec::new(),
            result: None,
            effects: AwbcEffectSetId(0),
        },
        AwbcSignature {
            params: vec![AwbcTypeId(3)],
            result: Some(AwbcTypeId(3)),
            effects: AwbcEffectSetId(0),
        },
    ];
    program.frame_layouts = vec![
        AwbcFrameLayout {
            slots: vec![
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(4),
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(5),
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
                ty: AwbcTypeId(3),
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            }],
            scopes: Vec::new(),
            max_scope_depth: 0,
        },
    ];
    let deterministic = AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic);
    program.functions = vec![
        AwbcFunction {
            definition: crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(*blake3::hash(b"crates.arcweft-core.src.awbc.tests.callable_specialization.specialization_program.definition.0.v1\0").as_bytes()),
            semantic_role: crate::plan::RuntimeFunctionSemanticRole::Ordinary,
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: AwbcSignatureId(0),
            type_context: None,
            input_ownership: Vec::new(),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 1),
            entry_block: AwbcBlockId(0),
            flags: deterministic,
        },
        AwbcFunction {
            definition: crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(*blake3::hash(b"crates.arcweft-core.src.awbc.tests.callable_specialization.specialization_program.definition.1.v1\0").as_bytes()),
            semantic_role: crate::plan::RuntimeFunctionSemanticRole::Ordinary,
            public_id: None,
            kind: AwbcFunctionKind::Ordinary,
            signature: AwbcSignatureId(1),
            type_context: None,
            input_ownership: vec![AwbcFunctionInputOwnership::parameter(
                manual_awbc_formal_identity(
                    "arcweft-core.src.awbc.tests.callable_specialization.specialization_program.input-a",
                ),
                0,
                crate::plan::RuntimeFunctionParameterPassing::Value,
            )],
            frame_layout: AwbcFrameLayoutId(1),
            blocks: AwbcTableRange::new(1, 1),
            entry_block: AwbcBlockId(1),
            flags: deterministic,
        },
    ];
    program.blocks = vec![
        AwbcBlock {
            owner: AwbcFunctionId(0),
            instructions: AwbcTableRange::new(0, 2),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::FlowEntry,
            source_map: None,
        },
        AwbcBlock {
            owner: AwbcFunctionId(1),
            instructions: AwbcTableRange::new(2, 0),
            terminator: AwbcTerminator::Return {
                value: Some(AwbcRegisterId(0)),
            },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
    ];
    let source_state = callable_state_id(0);
    let target_state = callable_state_id(1);
    let coordinate = RuntimeCallableParameterCoordinate {
        group: 0,
        parameter: 0,
    };
    program.callable_states = vec![
        RuntimeCallableStateDefinition {
            function_type: AwbcTypeId(4),
            origin: source_state,
            position: RuntimeCallablePosition::Unapplied,
            retained: Box::new([]),
            parameters: Box::new([RuntimeCallableParameterInput {
                coordinate,
                kind: RuntimeCallableParameterKind::Fixed,
                abi_ty: AwbcTypeId(2),
                binding_ty: AwbcTypeId(2),
            }]),
            result: AwbcTypeId(2),
            attached: RuntimeCallableAttachedContract::None,
            transition: RuntimeCallableTransition::RequiresSpecialization,
            partials: Box::new([]),
        },
        RuntimeCallableStateDefinition {
            function_type: AwbcTypeId(5),
            origin: source_state,
            position: RuntimeCallablePosition::Unapplied,
            retained: Box::new([]),
            parameters: Box::new([RuntimeCallableParameterInput {
                coordinate,
                kind: RuntimeCallableParameterKind::Fixed,
                abi_ty: AwbcTypeId(3),
                binding_ty: AwbcTypeId(3),
            }]),
            result: AwbcTypeId(3),
            attached: RuntimeCallableAttachedContract::None,
            transition: RuntimeCallableTransition::Invoke {
                function: AwbcFunctionId(1),
                captures: Box::new([]),
                arguments: Box::new([RuntimeCallableInputSource::Argument { position: 0 }]),
            },
            partials: Box::new([]),
        },
    ];
    program.callable_specializations = vec![RuntimeCallableSpecializationDefinition {
        source_type: AwbcTypeId(4),
        target_type: AwbcTypeId(5),
        arguments: RuntimeFunctionSpecializationArguments {
            types: Box::new([AwbcTypeId(3)]),
            const_lengths: Box::<[RuntimeArrayLength]>::default(),
            effects: Box::new([]),
        },
        states: Box::new([RuntimeCallableSpecializationState {
            source: source_state,
            target: target_state,
        }]),
    }];
    program.instructions = vec![
        AwbcInstruction::MakeCallable {
            dst: AwbcRegisterId(0),
            state: source_state,
            captures: Vec::new(),
        },
        AwbcInstruction::SpecializeCallable {
            dst: AwbcRegisterId(1),
            src: AwbcRegisterId(0),
            specialization: specialization_id(0),
        },
    ];
    program.canonicalize_string_table();
    program
}

#[test]
fn callable_specialization_table_round_trips_and_vm_uses_the_admitted_state_relation() {
    let program = specialization_program();
    program
        .verify(Default::default(), Default::default())
        .unwrap();

    let encoded = program.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&encoded, Default::default()).unwrap();
    assert_eq!(
        decoded.callable_specializations,
        program.callable_specializations
    );
    assert_eq!(decoded.encode_canonical().unwrap(), encoded);
    decoded
        .verify(Default::default(), Default::default())
        .unwrap();

    let leased_program = Arc::new(decoded);
    let execution = super::super::vm::VmExecutionContext::for_program(
        RuntimeArtifactFingerprint::try_from_bytes([0x91; 32]).unwrap(),
        super::admitted_program(&leased_program),
    );
    let mut fiber = FiberState::for_entry(&leased_program, AwbcEntryId(0), 1, 8).unwrap();
    let mut host = super::super::vm::RejectingVmHost;
    let output = super::super::vm::step_with_host_context(
        &leased_program,
        &mut fiber,
        super::super::vm::VmStepOptions {
            max_instructions: 2,
        },
        &execution,
        &mut host,
    )
    .unwrap();
    assert_eq!(output.executed, 2);
    let RuntimeValue::Callable(original) = fiber.frames[0].registers[0].as_ref().unwrap() else {
        panic!("source register retains its generic callable");
    };
    let RuntimeValue::Callable(specialized) = fiber.frames[0].registers[1].as_ref().unwrap() else {
        panic!("destination register contains the specialized callable");
    };
    assert_eq!(original.state(), callable_state_id(0));
    assert_eq!(specialized.state(), callable_state_id(1));
    specialized
        .validate_for_owner(&RuntimeProgramOwner::Awbc(leased_program))
        .unwrap();
}

#[test]
fn callable_specialization_verification_rejects_wrong_register_types_and_relation_arguments() {
    let mut wrong_destination = specialization_program();
    wrong_destination.frame_layouts[0].slots[1].ty = AwbcTypeId(4);
    assert!(matches!(
        wrong_destination.verify(Default::default(), Default::default()),
        Err(AwbcVerifyError::InvalidInvariant { .. })
    ));

    let mut wrong_argument = specialization_program();
    wrong_argument.callable_specializations[0].arguments.types[0] = AwbcTypeId(0);
    assert!(matches!(
        wrong_argument.verify(Default::default(), Default::default()),
        Err(AwbcVerifyError::InvalidInvariant { .. })
    ));
}

#[test]
fn callable_specialization_retains_the_applied_input_and_the_next_groups_binder() {
    let mut program = specialization_program();
    let source_binder = RuntimeTypeBinder::new(2, 0, 0);
    let next_binder = RuntimeTypeBinder::new(1, 0, 0);
    let source_scope = RuntimeTypeScope::root().enter(source_binder).unwrap();
    let next_scope = RuntimeTypeScope::root().enter(next_binder).unwrap();
    let function = |binder, parameter, result| AwbcRuntimeTypeShape::Function {
        contract: RuntimeFunctionTypeContract::new(
            binder,
            EffectPredicate::unconstrained(),
            EffectFormula::empty(),
        ),
        parameters: vec![parameter],
        result,
    };
    let middle_parameter = AwbcTypeId(2);
    let closed_middle = AwbcTypeId(3);
    let source_late = AwbcTypeId(6);
    let source_result = AwbcTypeId(7);
    let target_result = AwbcTypeId(8);
    let target_late = AwbcTypeId(9);
    program.runtime_types[2] = runtime_type(
        0x52,
        AwbcRuntimeTypeShape::BoundType(source_scope.bound_type(0, 0).unwrap()),
    )
    .with_scope(source_scope.clone());
    program.runtime_types[4] = runtime_type(
        0x54,
        function(source_binder, middle_parameter, source_result),
    );
    program.runtime_types[5] = runtime_type(
        0x55,
        function(RuntimeTypeBinder::EMPTY, closed_middle, target_result),
    );
    program.runtime_types.extend([
        runtime_type(
            0x56,
            AwbcRuntimeTypeShape::BoundType(source_scope.bound_type(0, 1).unwrap()),
        )
        .with_scope(source_scope.clone()),
        runtime_type(
            0x57,
            function(RuntimeTypeBinder::EMPTY, source_late, source_late),
        )
        .with_scope(source_scope),
        runtime_type(0x58, function(next_binder, target_late, target_late)),
        runtime_type(
            0x59,
            AwbcRuntimeTypeShape::BoundType(next_scope.bound_type(0, 0).unwrap()),
        )
        .with_scope(next_scope),
    ]);
    let next = callable_state_id(2);
    program.callable_states[0].result = source_result;
    program.callable_states[1].result = target_result;
    program.callable_states[1].transition = RuntimeCallableTransition::Retain {
        state: next,
        values: Box::new([RuntimeCallableInputSource::Argument { position: 0 }]),
    };
    let retained = program.callable_states[1].parameters[0].coordinate;
    program
        .callable_states
        .push(RuntimeCallableStateDefinition {
            function_type: target_result,
            origin: callable_state_id(0),
            position: RuntimeCallablePosition::AfterGroup { completed: 0 },
            retained: Box::new([crate::plan::RuntimeCallableRetainedInput {
                role: crate::plan::RuntimeCallableRetainedRole::Parameter(retained),
                ty: closed_middle,
            }]),
            parameters: Box::new([RuntimeCallableParameterInput {
                coordinate: RuntimeCallableParameterCoordinate {
                    group: 1,
                    parameter: 0,
                },
                kind: RuntimeCallableParameterKind::Fixed,
                abi_ty: target_late,
                binding_ty: target_late,
            }]),
            result: target_late,
            attached: RuntimeCallableAttachedContract::None,
            transition: RuntimeCallableTransition::RequiresSpecialization,
            partials: Box::new([]),
        });
    program.callable_specializations[0].arguments.types = Box::new([closed_middle, target_late]);
    program.frame_layouts[0]
        .slots
        .extend([closed_middle, target_result].map(|ty| AwbcFrameSlot {
            name: None,
            ty,
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        }));
    program.constants.push(AwbcConstant::UInt {
        kind: AwbcUnsignedIntKind::U32,
        bits: 42u128.to_le_bytes(),
    });
    program.instructions.extend([
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(2),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::ApplyGroup {
            dst: AwbcRegisterId(3),
            callee: AwbcRegisterId(1),
            args: vec![AwbcRegisterId(2)],
        },
    ]);
    program.blocks[0].instructions.len = 4;
    program.blocks[1].instructions.start = 4;
    let bytes = program.encode_canonical().unwrap();
    let decoded = Arc::new(AwbcProgram::decode_canonical(&bytes, Default::default()).unwrap());
    assert_eq!(decoded.encode_canonical().unwrap(), bytes);
    decoded
        .verify(Default::default(), Default::default())
        .unwrap();
    let execution = super::super::vm::VmExecutionContext::for_program(
        RuntimeArtifactFingerprint::try_from_bytes(*blake3::hash(&bytes).as_bytes()).unwrap(),
        super::admitted_program(&decoded),
    );
    let mut fiber = FiberState::for_entry(&decoded, AwbcEntryId(0), 1, 8).unwrap();
    let mut host = super::super::vm::RejectingVmHost;
    for _ in 0..4 {
        let step = super::super::vm::step_with_host_context(
            &decoded,
            &mut fiber,
            super::super::vm::VmStepOptions {
                max_instructions: 1,
            },
            &execution,
            &mut host,
        )
        .unwrap();
        assert_eq!(step.executed, 1);
    }
    let RuntimeValue::Callable(retained) = fiber.frames[0].registers[3].as_ref().unwrap() else {
        panic!("the retaining application produces the next callable");
    };
    assert_eq!(retained.state(), next);
    assert_eq!(retained.retained(), &[RuntimeValue::u32(42)]);
    retained
        .validate_for_owner(&RuntimeProgramOwner::Awbc(decoded))
        .unwrap();
}

#[test]
fn callable_specialization_verification_enforces_table_and_cumulative_work_budgets() {
    let program = specialization_program();
    assert_eq!(
        program.verify(
            AwbcVerifyBudget {
                callable_specializations: 0,
                ..AwbcVerifyBudget::default()
            },
            Default::default(),
        ),
        Err(AwbcVerifyError::BudgetExceeded {
            budget: "callable_specializations",
        })
    );
    assert_eq!(
        program.verify(
            AwbcVerifyBudget {
                specialization_validation_work: 0,
                ..AwbcVerifyBudget::default()
            },
            Default::default(),
        ),
        Err(AwbcVerifyError::BudgetExceeded {
            budget: "specialization_validation_work",
        })
    );

    let encoded = program.encode_canonical().unwrap();
    let budget = AwbcDecodeBudget {
        callable_specializations: 0,
        ..AwbcDecodeBudget::default()
    };
    assert!(matches!(
        AwbcProgram::decode_canonical(&encoded, budget),
        Err(AwbcCodecError::BudgetExceeded {
            budget: "callable_specializations",
            ..
        })
    ));
}

fn manual_awbc_formal_identity(declaration: &str) -> crate::plan::RuntimeFunctionParameterIdentity {
    let mut hash = blake3::Hasher::new();
    hash.update(b"arcweft.manual-awbc-formal.v1\0");
    hash.update(declaration.as_bytes());
    crate::plan::RuntimeFunctionParameterIdentity::from_accepted_identity(
        *hash.finalize().as_bytes(),
    )
}
