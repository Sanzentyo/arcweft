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
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 1),
            entry_block: AwbcBlockId(0),
            flags: deterministic,
        },
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Ordinary,
            signature: AwbcSignatureId(1),
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
        leased_program.clone(),
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
