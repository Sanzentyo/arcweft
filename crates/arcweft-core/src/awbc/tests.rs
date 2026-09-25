use super::codec::{AwbcCodecError, AwbcDecodeBudget};
mod agent_constructors;
mod agent_projection;
mod array;
mod callable_specialization;
mod record_shapes;
use super::fiber::{
    AwbcFiberStateSnapshot, FiberAwaitTarget, FiberResumeTarget, FiberReturnContinuation,
    FiberScopeCleanup, FiberState, FiberStatus, FiberSuspension, FiberSuspensionReason,
};
use super::schema::*;
use super::verify::{AwbcVerifyBudget, AwbcVerifyContext, AwbcVerifyError};
use crate::effect::RuntimeAssertionGuardId;
use crate::entry::{FlowContractHash, RuntimeFlowExecutable};
use crate::pattern::{
    RuntimeCheckedType, RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner,
    RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId,
};
use crate::plan::{
    FlowRuntimeId, RuntimeAgentOperationalType, RuntimeCallableAttachedContract,
    RuntimeCallableDefault, RuntimeCallableInputSource, RuntimeCallableParameterCoordinate,
    RuntimeCallableParameterInput, RuntimeCallableParameterKind, RuntimeCallablePosition,
    RuntimeCallableRetainedInput, RuntimeCallableRetainedRole, RuntimeCallableStateDefinition,
    RuntimeCallableTransition, RuntimeFlowTargetError, RuntimeFunctionTypeContract,
};
use crate::runtime_id::RuntimeCallableStateId;
use crate::task::RuntimeProgramOwner;
use crate::value::{
    RuntimeHandleKind, RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeValue,
    runtime_sequence_values,
};

fn runtime_type(marker: u8, shape: AwbcRuntimeTypeShape) -> AwbcRuntimeType {
    AwbcRuntimeType::new(RuntimeSemanticTypeId::from_bytes([marker; 32]), shape)
}

fn test_flow_binding(label: &str, function: u32) -> AwbcFlowBinding {
    AwbcFlowBinding {
        flow: FlowRuntimeId::canonical(label).expect("test Flow ID is valid"),
        function: AwbcFunctionId(function),
    }
}

fn test_flow_executable(label: &str, function: u32) -> AwbcFlowExecutable {
    AwbcFlowExecutable {
        metadata: RuntimeFlowExecutable {
            flow: FlowRuntimeId::canonical(label).expect("test Flow ID is valid"),
            contract: FlowContractHash::from_bytes([0x5a; 32]),
            controller: None,
        },
        function: AwbcFunctionId(function),
    }
}

fn minimal_program() -> AwbcProgram {
    AwbcProgram {
        strings: vec!["main".to_owned()],
        signatures: vec![AwbcSignature {
            params: Vec::new(),
            result: None,
            effects: AwbcEffectSetId(0),
        }],
        frame_layouts: vec![AwbcFrameLayout {
            scopes: Vec::new(),
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
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        }],
        blocks: vec![AwbcBlock {
            owner: AwbcFunctionId(0),
            instructions: AwbcTableRange::new(0, 0),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::FlowEntry,
            source_map: None,
        }],
        flow_bindings: vec![test_flow_binding("main", 0)],
        flow_executables: vec![test_flow_executable("main", 0)],
        entries: vec![AwbcEntry {
            runtime_id: crate::plan::EntryRuntimeId::canonical("main")
                .expect("test entry runtime ID is valid"),
            binding: crate::entry::EntryBindingIdentity::from_bytes([1; 32]),
            public_id: AwbcStringId(0),
            kind: AwbcEntryKind::Cli,
            target: AwbcEntryTarget::Function {
                function: AwbcFunctionId(0),
            },
            roles: crate::entry::RuntimeEntryRoles::None,
        }],
        ..AwbcProgram::default()
    }
}

#[test]
fn named_scope_layout_roundtrip_and_snapshot_admission_preserve_static_identity() {
    let mut program = minimal_program();
    let named = |name| {
        crate::scope::RuntimeScopeIdentity::Named(
            arcweft_id::DeclarationName::try_new(name).unwrap(),
        )
    };
    program.frame_layouts[0].scopes = vec![
        AwbcScopeDefinition {
            parent: None,
            identity: named("rain"),
        },
        AwbcScopeDefinition {
            parent: Some(AwbcScopeId(0)),
            identity: named("window"),
        },
        AwbcScopeDefinition {
            parent: Some(AwbcScopeId(0)),
            identity: crate::scope::RuntimeScopeIdentity::Anonymous,
        },
    ];
    program.frame_layouts[0].max_scope_depth = 2;
    program.instructions = vec![
        AwbcInstruction::EnterScope {
            scope: AwbcScopeId(0),
        },
        AwbcInstruction::EnterScope {
            scope: AwbcScopeId(1),
        },
        AwbcInstruction::ExitScope {
            scope: AwbcScopeId(1),
        },
        AwbcInstruction::EnterScope {
            scope: AwbcScopeId(2),
        },
        AwbcInstruction::ExitScope {
            scope: AwbcScopeId(2),
        },
        AwbcInstruction::ExitScope {
            scope: AwbcScopeId(0),
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 6);
    program
        .verify(Default::default(), Default::default())
        .unwrap();
    let encoded = program.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&encoded, Default::default()).unwrap();
    assert_eq!(decoded.frame_layouts, program.frame_layouts);
    assert_eq!(decoded.encode_canonical().unwrap(), encoded);

    let mut fiber = FiberState::for_entry(&decoded, AwbcEntryId(0), 1, 64).unwrap();
    super::vm::step(
        &decoded,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 2,
        },
    )
    .unwrap();
    fiber.validate_for_program(&decoded).unwrap();
    let snapshot = AwbcFiberStateSnapshot::from_live(&fiber).unwrap();
    let serialized = serde_json::to_vec(&snapshot).unwrap();
    let snapshot: AwbcFiberStateSnapshot = serde_json::from_slice(&serialized).unwrap();
    let restored = snapshot
        .into_live_for_program(&crate::task::RuntimeProgramOwner::Awbc(
            std::sync::Arc::new(decoded.clone()),
        ))
        .unwrap();
    restored.validate_for_program(&decoded).unwrap();
    assert_eq!(restored, fiber);

    let mut wrong_sibling = restored.clone();
    wrong_sibling.frames[0].scopes[1].id = AwbcScopeId(2);
    assert!(wrong_sibling.validate_for_program(&decoded).is_err());
    let mut wrong_depth = restored.clone();
    wrong_depth.frames[0].scopes[1].depth = 0;
    assert!(wrong_depth.validate_for_program(&decoded).is_err());
    let mut unknown_scope = restored;
    unknown_scope.frames[0].scopes[1].id = AwbcScopeId(99);
    assert!(unknown_scope.validate_for_program(&decoded).is_err());

    let mut wrong_parent = decoded;
    wrong_parent.frame_layouts[0].scopes[1].parent = Some(AwbcScopeId(2));
    assert!(
        wrong_parent
            .verify(Default::default(), Default::default())
            .is_err()
    );
}

fn add_effect_set(program: &mut AwbcProgram, effects: &[&str]) -> AwbcEffectSetId {
    let id = AwbcEffectSetId(u32::try_from(program.effect_sets.len()).unwrap());
    let effects = effects
        .iter()
        .map(|effect| {
            let string = AwbcStringId(u32::try_from(program.strings.len()).unwrap());
            program.strings.push((*effect).to_owned());
            string
        })
        .collect();
    program.effect_sets.push(AwbcEffectSet { effects });
    id
}

fn callable_state_id(index: usize) -> RuntimeCallableStateId {
    RuntimeCallableStateId::from_zero_based(index).expect("test callable-state index is valid")
}

#[test]
fn awbc_function_scope_contract_bound_array_and_effect_refs_round_trip_with_scope_admission() {
    use crate::effect_row::{EffectFormula, EffectPredicate, EffectSet};
    use crate::plan::{
        RuntimeArrayLength, RuntimeFunctionTypeContract, RuntimeTypeBinder, RuntimeTypeScope,
    };

    let binder = RuntimeTypeBinder::new(1, 1, 1);
    let child_scope = RuntimeTypeScope::root()
        .enter(binder)
        .expect("one binder is within the type depth limit");
    let type_reference = child_scope
        .bound_type(0, 0)
        .expect("the binder has one type parameter");
    let const_reference = child_scope
        .bound_const(0, 0)
        .expect("the binder has one const parameter");
    let effect_reference = child_scope
        .bound_effect(0, 0)
        .expect("the binder has one effect parameter");
    let contract = RuntimeFunctionTypeContract::new(
        binder,
        EffectPredicate::unconstrained(),
        EffectFormula::literal(EffectSet::new(), Some(effect_reference)),
    );

    let mut program = minimal_program();
    program.runtime_types.extend([
        runtime_type(3, AwbcRuntimeTypeShape::BoundType(type_reference))
            .with_scope(child_scope.clone()),
        runtime_type(
            4,
            AwbcRuntimeTypeShape::Array {
                item: AwbcTypeId(2),
                length: RuntimeArrayLength::Bound(const_reference),
            },
        )
        .with_scope(child_scope.clone()),
        runtime_type(
            5,
            AwbcRuntimeTypeShape::Function {
                contract,
                parameters: vec![AwbcTypeId(2), AwbcTypeId(3)],
                result: AwbcTypeId(0),
            },
        ),
    ]);

    let bytes = program
        .encode_canonical()
        .expect("scoped function types have a canonical codec");
    let decoded = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default())
        .expect("scoped function types decode");
    assert_eq!(decoded.runtime_types, program.runtime_types);
    assert_eq!(decoded.encode_canonical().unwrap(), bytes);
    decoded
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("bound references are admitted against their exact owner scopes");

    let wrong_child_scope = RuntimeTypeScope::root()
        .enter(RuntimeTypeBinder::new(2, 1, 1))
        .expect("one binder is within the type depth limit");
    let mut mismatched = decoded.clone();
    mismatched.runtime_types[2] = mismatched.runtime_types[2]
        .clone()
        .with_scope(wrong_child_scope);
    assert!(
        mismatched
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .is_err()
    );

    let mut escaped = decoded;
    escaped.runtime_types[2] = escaped.runtime_types[2]
        .clone()
        .with_scope(RuntimeTypeScope::root());
    assert!(
        escaped
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .is_err()
    );
}

fn step_with_callable_context(
    program: &std::sync::Arc<AwbcProgram>,
    fiber: &mut FiberState,
    options: super::vm::VmStepOptions,
) -> Result<super::vm::VmStepOutput, super::vm::VmError> {
    let artifact = crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0x5a; 32])
        .expect("test artifact fingerprint is valid");
    let context =
        super::vm::VmExecutionContext::for_program(artifact, std::sync::Arc::clone(program));
    super::vm::step_with_host_context(
        program,
        fiber,
        options,
        &context,
        &mut super::vm::RejectingVmHost,
    )
}

fn project_call_invoke_program() -> AwbcProgram {
    let mut program = minimal_program();
    program.constants = vec![AwbcConstant::Unit];
    program.runtime_types = vec![
        runtime_type(1, AwbcRuntimeTypeShape::Unit),
        runtime_type(
            2,
            AwbcRuntimeTypeShape::Function {
                contract: RuntimeFunctionTypeContract::default(),
                parameters: Vec::new(),
                result: AwbcTypeId(0),
            },
        ),
    ];
    program.patterns = vec![AwbcPattern::Discard];
    program.signatures.push(AwbcSignature {
        params: Vec::new(),
        result: Some(AwbcTypeId(0)),
        effects: AwbcEffectSetId(0),
    });
    program.frame_layouts[0] = AwbcFrameLayout {
        scopes: Vec::new(),
        slots: vec![AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(1),
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        }],
        max_scope_depth: 0,
    };
    program.frame_layouts.push(AwbcFrameLayout {
        scopes: Vec::new(),
        slots: vec![AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(0),
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        }],
        max_scope_depth: 0,
    });
    program.functions[0].blocks = AwbcTableRange::new(0, 2);
    program.functions.push(AwbcFunction {
        public_id: None,
        kind: AwbcFunctionKind::Ordinary,
        signature: AwbcSignatureId(1),
        frame_layout: AwbcFrameLayoutId(1),
        blocks: AwbcTableRange::new(2, 1),
        entry_block: AwbcBlockId(2),
        flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
    });
    program.blocks[0].terminator = AwbcTerminator::ProjectCall {
        call: AwbcProjectCall {
            callee: AwbcRegisterId(0),
            state: crate::runtime_id::RuntimeCallableStateId::from_zero_based(0).unwrap(),
            completed_group: 0,
            operands: Vec::new(),
            ordinary: Vec::new(),
            attached: None,
            result_pattern: AwbcPatternId(0),
            resume: AwbcResumePointId(0),
        },
    };
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(0),
        instructions: AwbcTableRange::new(0, 0),
        terminator: AwbcTerminator::Return { value: None },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(1),
        instructions: AwbcTableRange::new(1, 1),
        terminator: AwbcTerminator::Return {
            value: Some(AwbcRegisterId(0)),
        },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program.instructions.push(AwbcInstruction::MakeCallable {
        dst: AwbcRegisterId(0),
        state: callable_state_id(0),
        captures: Vec::new(),
    });
    program.instructions.push(AwbcInstruction::LoadConst {
        dst: AwbcRegisterId(0),
        constant: AwbcConstantId(0),
    });
    program.blocks[0].instructions = AwbcTableRange::new(0, 1);
    program
        .callable_states
        .push(RuntimeCallableStateDefinition {
            function_type: AwbcTypeId(1),
            origin: callable_state_id(0),
            position: RuntimeCallablePosition::Unapplied,
            retained: Box::new([]),
            parameters: Box::new([]),
            result: AwbcTypeId(0),
            attached: RuntimeCallableAttachedContract::None,
            transition: RuntimeCallableTransition::Invoke {
                function: AwbcFunctionId(1),
                captures: Box::new([]),
                arguments: Box::new([]),
            },
            partials: Box::new([]),
        });
    program.resume_points.push(AwbcResumePoint {
        function: AwbcFunctionId(0),
        block: AwbcBlockId(1),
        frame_layout: AwbcFrameLayoutId(0),
        kind: AwbcSafePointKind::CallableBoundary,
    });
    program.canonicalize_string_table();
    program
}

fn ordinary_returning_call_program() -> AwbcProgram {
    let mut program = minimal_program();
    program.signatures.push(AwbcSignature {
        params: Vec::new(),
        result: None,
        effects: AwbcEffectSetId(0),
    });
    program.frame_layouts.push(AwbcFrameLayout {
        scopes: Vec::new(),
        slots: Vec::new(),
        max_scope_depth: 0,
    });
    program.functions[0].blocks = AwbcTableRange::new(0, 2);
    program.functions.push(AwbcFunction {
        public_id: None,
        kind: AwbcFunctionKind::Ordinary,
        signature: AwbcSignatureId(1),
        frame_layout: AwbcFrameLayoutId(1),
        blocks: AwbcTableRange::new(2, 1),
        entry_block: AwbcBlockId(2),
        flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
    });
    program.blocks[0].terminator = AwbcTerminator::CallFunction {
        function: AwbcFunctionId(1),
        args: Vec::new(),
        dst: None,
        resume: AwbcResumePointId(0),
    };
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(0),
        instructions: AwbcTableRange::new(0, 0),
        terminator: AwbcTerminator::Return { value: None },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(1),
        instructions: AwbcTableRange::new(0, 0),
        terminator: AwbcTerminator::Return { value: None },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program.resume_points.push(AwbcResumePoint {
        function: AwbcFunctionId(0),
        block: AwbcBlockId(1),
        frame_layout: AwbcFrameLayoutId(0),
        kind: AwbcSafePointKind::CallableBoundary,
    });
    program
}
fn project_call_fixed_program() -> AwbcProgram {
    let mut program = project_call_invoke_program();
    program.constants = vec![AwbcConstant::Unit];
    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(0),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    program.signatures[1].params = vec![AwbcTypeId(0)];
    program.frame_layouts[1].slots[0].role = AwbcFrameSlotRole::Parameter;
    program.runtime_types[1] = runtime_type(
        2,
        AwbcRuntimeTypeShape::Function {
            contract: RuntimeFunctionTypeContract::default(),
            parameters: vec![AwbcTypeId(0)],
            result: AwbcTypeId(0),
        },
    );
    program.instructions = vec![
        AwbcInstruction::MakeCallable {
            dst: AwbcRegisterId(0),
            state: callable_state_id(0),
            captures: Vec::new(),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(0),
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 2);
    program.blocks[2].instructions = AwbcTableRange::new(2, 1);
    program.callable_states[0].function_type = AwbcTypeId(1);
    program.callable_states[0].parameters = vec![RuntimeCallableParameterInput {
        coordinate: RuntimeCallableParameterCoordinate {
            group: 0,
            parameter: 0,
        },
        kind: crate::plan::RuntimeCallableParameterKind::Fixed,
        abi_ty: AwbcTypeId(0),
        binding_ty: AwbcTypeId(0),
    }]
    .into_boxed_slice();
    program.callable_states[0].transition = RuntimeCallableTransition::Invoke {
        function: AwbcFunctionId(1),
        captures: Box::new([]),
        arguments: vec![RuntimeCallableInputSource::Argument { position: 0 }].into_boxed_slice(),
    };
    program.blocks[0].terminator = AwbcTerminator::ProjectCall {
        call: AwbcProjectCall {
            callee: AwbcRegisterId(0),
            state: callable_state_id(0),
            completed_group: 0,
            operands: vec![AwbcProjectCallOperand {
                value: AwbcRegisterId(1),
                mode: AwbcProjectCallOperandMode::Value,
            }],
            ordinary: vec![AwbcProjectCallOrdinaryMaterialization::Fixed {
                parameter: 0,
                source_index: 0,
            }],
            attached: None,
            result_pattern: AwbcPatternId(0),
            resume: AwbcResumePointId(0),
        },
    };
    program
}

fn project_call_two_fixed_parameters_program() -> AwbcProgram {
    let mut program = project_call_fixed_program();
    program.runtime_types[1] = runtime_type(
        2,
        AwbcRuntimeTypeShape::Function {
            contract: RuntimeFunctionTypeContract::default(),
            parameters: vec![AwbcTypeId(0), AwbcTypeId(0)],
            result: AwbcTypeId(0),
        },
    );
    program.signatures[1].params = vec![AwbcTypeId(0), AwbcTypeId(0)];
    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(0),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    program.frame_layouts[1].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(0),
        role: AwbcFrameSlotRole::Parameter,
        scope_depth: 0,
    });
    let second_coordinate = RuntimeCallableParameterCoordinate {
        group: 0,
        parameter: 1,
    };
    let mut parameters = program.callable_states[0].parameters.to_vec();
    parameters.push(RuntimeCallableParameterInput {
        coordinate: second_coordinate,
        kind: RuntimeCallableParameterKind::Fixed,
        abi_ty: AwbcTypeId(0),
        binding_ty: AwbcTypeId(0),
    });
    program.callable_states[0].parameters = parameters.into_boxed_slice();
    program.callable_states[0].transition = RuntimeCallableTransition::Invoke {
        function: AwbcFunctionId(1),
        captures: Box::new([]),
        arguments: vec![
            RuntimeCallableInputSource::Argument { position: 0 },
            RuntimeCallableInputSource::Argument { position: 1 },
        ]
        .into_boxed_slice(),
    };
    program.instructions.insert(
        2,
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(2),
            constant: AwbcConstantId(0),
        },
    );
    program.blocks[0].instructions = AwbcTableRange::new(0, 3);
    program.blocks[2].instructions = AwbcTableRange::new(3, 1);
    let call = project_call_mut(&mut program);
    call.operands.push(AwbcProjectCallOperand {
        value: AwbcRegisterId(2),
        mode: AwbcProjectCallOperandMode::Value,
    });
    call.ordinary
        .push(AwbcProjectCallOrdinaryMaterialization::Fixed {
            parameter: 1,
            source_index: 1,
        });
    program
}

fn expect_project_call_rejection(program: AwbcProgram, message: &str) {
    let error = program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("malformed ProjectCall must be rejected");
    assert!(
        error.to_string().contains(message),
        "expected ProjectCall rejection containing {message:?}, got {error:?}"
    );
}

fn project_call_mut(program: &mut AwbcProgram) -> &mut AwbcProjectCall {
    match &mut program.blocks[0].terminator {
        AwbcTerminator::ProjectCall { call } => call,
        terminator => panic!("test program does not have a ProjectCall terminator: {terminator:?}"),
    }
}

fn project_call_at_mut(program: &mut AwbcProgram, block: usize) -> &mut AwbcProjectCall {
    match &mut program.blocks[block].terminator {
        AwbcTerminator::ProjectCall { call } => call,
        terminator => panic!("block {block} is not a ProjectCall terminator: {terminator:?}"),
    }
}

fn project_call_retained_program() -> AwbcProgram {
    let mut program = minimal_program();
    program.constants = vec![AwbcConstant::Unit];
    program.runtime_types = vec![
        runtime_type(1, AwbcRuntimeTypeShape::Unit),
        runtime_type(
            2,
            AwbcRuntimeTypeShape::Function {
                contract: RuntimeFunctionTypeContract::default(),
                parameters: vec![AwbcTypeId(0)],
                result: AwbcTypeId(2),
            },
        ),
        runtime_type(
            3,
            AwbcRuntimeTypeShape::Function {
                contract: RuntimeFunctionTypeContract::default(),
                parameters: vec![AwbcTypeId(0)],
                result: AwbcTypeId(0),
            },
        ),
    ];
    program.patterns = vec![
        AwbcPattern::Discard,
        AwbcPattern::Bind {
            target: AwbcRegisterId(1),
            mutable: false,
            expected: None,
        },
    ];
    program.signatures.push(AwbcSignature {
        params: vec![AwbcTypeId(0), AwbcTypeId(0)],
        result: Some(AwbcTypeId(0)),
        effects: AwbcEffectSetId(0),
    });
    program.frame_layouts[0] = AwbcFrameLayout {
        scopes: Vec::new(),
        slots: vec![
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(1),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(2),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
        ],
        max_scope_depth: 0,
    };
    program.frame_layouts.push(AwbcFrameLayout {
        scopes: Vec::new(),
        slots: vec![
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
        ],
        max_scope_depth: 0,
    });
    program.functions[0].blocks = AwbcTableRange::new(0, 4);
    program.functions.push(AwbcFunction {
        public_id: None,
        kind: AwbcFunctionKind::Ordinary,
        signature: AwbcSignatureId(1),
        frame_layout: AwbcFrameLayoutId(1),
        blocks: AwbcTableRange::new(4, 1),
        entry_block: AwbcBlockId(4),
        flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
    });
    program.blocks[0].instructions = AwbcTableRange::new(0, 2);
    program.blocks[0].terminator = AwbcTerminator::ProjectCall {
        call: AwbcProjectCall {
            callee: AwbcRegisterId(0),
            state: callable_state_id(0),
            completed_group: 0,
            operands: vec![AwbcProjectCallOperand {
                value: AwbcRegisterId(2),
                mode: AwbcProjectCallOperandMode::Value,
            }],
            ordinary: vec![AwbcProjectCallOrdinaryMaterialization::Fixed {
                parameter: 0,
                source_index: 0,
            }],
            attached: None,
            result_pattern: AwbcPatternId(1),
            resume: AwbcResumePointId(0),
        },
    };
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(0),
        instructions: AwbcTableRange::new(2, 0),
        terminator: AwbcTerminator::BudgetYield {
            resume: AwbcResumePointId(1),
        },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(0),
        instructions: AwbcTableRange::new(2, 0),
        terminator: AwbcTerminator::ProjectCall {
            call: AwbcProjectCall {
                callee: AwbcRegisterId(1),
                state: callable_state_id(1),
                completed_group: 1,
                operands: vec![AwbcProjectCallOperand {
                    value: AwbcRegisterId(2),
                    mode: AwbcProjectCallOperandMode::Value,
                }],
                ordinary: vec![AwbcProjectCallOrdinaryMaterialization::Fixed {
                    parameter: 0,
                    source_index: 0,
                }],
                attached: None,
                result_pattern: AwbcPatternId(0),
                resume: AwbcResumePointId(2),
            },
        },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(0),
        instructions: AwbcTableRange::new(2, 0),
        terminator: AwbcTerminator::Return { value: None },
        safe_point: AwbcSafePointKind::None,
        source_map: None,
    });
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(1),
        instructions: AwbcTableRange::new(2, 1),
        terminator: AwbcTerminator::Return {
            value: Some(AwbcRegisterId(2)),
        },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program.instructions = vec![
        AwbcInstruction::MakeCallable {
            dst: AwbcRegisterId(0),
            state: callable_state_id(0),
            captures: Vec::new(),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(2),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(2),
            constant: AwbcConstantId(0),
        },
    ];
    let first_coordinate = RuntimeCallableParameterCoordinate {
        group: 0,
        parameter: 0,
    };
    let second_coordinate = RuntimeCallableParameterCoordinate {
        group: 1,
        parameter: 0,
    };
    program.callable_states = vec![
        RuntimeCallableStateDefinition {
            function_type: AwbcTypeId(1),
            origin: callable_state_id(0),
            position: RuntimeCallablePosition::Unapplied,
            retained: Box::new([]),
            parameters: vec![RuntimeCallableParameterInput {
                coordinate: first_coordinate,
                kind: RuntimeCallableParameterKind::Fixed,
                abi_ty: AwbcTypeId(0),
                binding_ty: AwbcTypeId(0),
            }]
            .into_boxed_slice(),
            result: AwbcTypeId(2),
            attached: RuntimeCallableAttachedContract::None,
            transition: RuntimeCallableTransition::Retain {
                state: callable_state_id(1),
                values: vec![RuntimeCallableInputSource::Argument { position: 0 }]
                    .into_boxed_slice(),
            },
            partials: Box::new([]),
        },
        RuntimeCallableStateDefinition {
            function_type: AwbcTypeId(2),
            origin: callable_state_id(0),
            position: RuntimeCallablePosition::AfterGroup { completed: 0 },
            retained: vec![RuntimeCallableRetainedInput {
                role: RuntimeCallableRetainedRole::Parameter(first_coordinate),
                ty: AwbcTypeId(0),
            }]
            .into_boxed_slice(),
            parameters: vec![RuntimeCallableParameterInput {
                coordinate: second_coordinate,
                kind: RuntimeCallableParameterKind::Fixed,
                abi_ty: AwbcTypeId(0),
                binding_ty: AwbcTypeId(0),
            }]
            .into_boxed_slice(),
            result: AwbcTypeId(0),
            attached: RuntimeCallableAttachedContract::None,
            transition: RuntimeCallableTransition::Invoke {
                function: AwbcFunctionId(1),
                captures: Box::new([]),
                arguments: vec![
                    RuntimeCallableInputSource::Retained { position: 0 },
                    RuntimeCallableInputSource::Argument { position: 0 },
                ]
                .into_boxed_slice(),
            },
            partials: Box::new([]),
        },
    ];
    program.resume_points = vec![
        AwbcResumePoint {
            function: AwbcFunctionId(0),
            block: AwbcBlockId(1),
            frame_layout: AwbcFrameLayoutId(0),
            kind: AwbcSafePointKind::CallableBoundary,
        },
        AwbcResumePoint {
            function: AwbcFunctionId(0),
            block: AwbcBlockId(2),
            frame_layout: AwbcFrameLayoutId(0),
            kind: AwbcSafePointKind::BudgetYield,
        },
        AwbcResumePoint {
            function: AwbcFunctionId(0),
            block: AwbcBlockId(3),
            frame_layout: AwbcFrameLayoutId(0),
            kind: AwbcSafePointKind::CallableBoundary,
        },
    ];
    program
}

fn project_call_default_program() -> AwbcProgram {
    let mut program = minimal_program();
    program
        .strings
        .extend(["Some".to_owned(), "None".to_owned()]);
    program.constants = vec![AwbcConstant::Unit];
    program.runtime_types = vec![
        runtime_type(1, AwbcRuntimeTypeShape::Unit),
        runtime_type(2, AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(0)])),
        runtime_type(
            3,
            AwbcRuntimeTypeShape::Variant {
                owner: AwbcVariantIdentity::Builtin(
                    crate::pattern::RuntimeBuiltinVariantIdentity::Option,
                ),
                arguments: Vec::new(),
                cases: vec![
                    AwbcVariantCase {
                        name: AwbcStringId(1),
                        payload: Some(AwbcTypeId(1)),
                    },
                    AwbcVariantCase {
                        name: AwbcStringId(2),
                        payload: None,
                    },
                ],
            },
        ),
        runtime_type(
            4,
            AwbcRuntimeTypeShape::Function {
                contract: RuntimeFunctionTypeContract::default(),
                parameters: Vec::new(),
                result: AwbcTypeId(0),
            },
        ),
    ];
    program.patterns = vec![AwbcPattern::Discard];
    program.signatures.extend([
        AwbcSignature {
            params: Vec::new(),
            result: Some(AwbcTypeId(0)),
            effects: AwbcEffectSetId(0),
        },
        AwbcSignature {
            params: vec![AwbcTypeId(0)],
            result: Some(AwbcTypeId(0)),
            effects: AwbcEffectSetId(0),
        },
    ]);
    program.frame_layouts.extend([
        AwbcFrameLayout {
            scopes: Vec::new(),
            slots: vec![AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            }],
            max_scope_depth: 0,
        },
        AwbcFrameLayout {
            scopes: Vec::new(),
            slots: vec![
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(0),
                    role: AwbcFrameSlotRole::Parameter,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(0),
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
            ],
            max_scope_depth: 0,
        },
    ]);
    program.frame_layouts[0] = AwbcFrameLayout {
        scopes: Vec::new(),
        slots: vec![AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(3),
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        }],
        max_scope_depth: 0,
    };
    program.functions[0].blocks = AwbcTableRange::new(0, 2);
    program.functions.extend([
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Ordinary,
            signature: AwbcSignatureId(1),
            frame_layout: AwbcFrameLayoutId(1),
            blocks: AwbcTableRange::new(2, 1),
            entry_block: AwbcBlockId(2),
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        },
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Ordinary,
            signature: AwbcSignatureId(2),
            frame_layout: AwbcFrameLayoutId(2),
            blocks: AwbcTableRange::new(3, 1),
            entry_block: AwbcBlockId(3),
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        },
    ]);
    program.blocks[0].terminator = AwbcTerminator::ProjectCall {
        call: AwbcProjectCall {
            callee: AwbcRegisterId(0),
            state: callable_state_id(0),
            completed_group: 0,
            operands: Vec::new(),
            ordinary: Vec::new(),
            attached: Some(AwbcProjectCallAttachedMaterialization {
                source_index: None,
                presence: AwbcProjectCallAttachedPresence::DefaultedOmitted,
            }),
            result_pattern: AwbcPatternId(0),
            resume: AwbcResumePointId(0),
        },
    };
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(0),
        instructions: AwbcTableRange::new(0, 0),
        terminator: AwbcTerminator::Return { value: None },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program.blocks.extend([
        AwbcBlock {
            owner: AwbcFunctionId(1),
            instructions: AwbcTableRange::new(0, 1),
            terminator: AwbcTerminator::Return {
                value: Some(AwbcRegisterId(0)),
            },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
        AwbcBlock {
            owner: AwbcFunctionId(2),
            instructions: AwbcTableRange::new(1, 1),
            terminator: AwbcTerminator::Return {
                value: Some(AwbcRegisterId(1)),
            },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
    ]);
    program.instructions.extend([
        AwbcInstruction::MakeCallable {
            dst: AwbcRegisterId(0),
            state: callable_state_id(0),
            captures: Vec::new(),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(0),
        },
    ]);
    program.blocks[0].instructions = AwbcTableRange::new(0, 1);
    program.blocks[2].instructions = AwbcTableRange::new(1, 1);
    program.blocks[3].instructions = AwbcTableRange::new(2, 1);
    program
        .callable_states
        .push(RuntimeCallableStateDefinition {
            function_type: AwbcTypeId(3),
            origin: callable_state_id(0),
            position: RuntimeCallablePosition::Unapplied,
            retained: Box::new([]),
            parameters: Box::new([]),
            result: AwbcTypeId(0),
            attached: RuntimeCallableAttachedContract::Defaulted {
                ty: AwbcTypeId(0),
                default: RuntimeCallableDefault::Body {
                    function: AwbcFunctionId(1),
                    captures: Box::new([]),
                },
            },
            transition: RuntimeCallableTransition::Invoke {
                function: AwbcFunctionId(2),
                captures: Box::new([]),
                arguments: vec![RuntimeCallableInputSource::Attached].into_boxed_slice(),
            },
            partials: Box::new([]),
        });
    program.resume_points.push(AwbcResumePoint {
        function: AwbcFunctionId(0),
        block: AwbcBlockId(1),
        frame_layout: AwbcFrameLayoutId(0),
        kind: AwbcSafePointKind::CallableBoundary,
    });
    program.canonicalize_string_table();
    program
}

fn goto_unwind_program(dynamic: bool) -> AwbcProgram {
    let mut program = minimal_program();
    let inner_flags = if dynamic {
        AwbcFunctionFlags::empty()
            .with(AwbcFunctionFlag::Deterministic)
            .with(AwbcFunctionFlag::HasDynamicTarget)
    } else {
        AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic)
    };
    program.strings.push("target".to_owned());
    program.runtime_types = vec![
        runtime_type(1, AwbcRuntimeTypeShape::Unit),
        runtime_type(2, AwbcRuntimeTypeShape::String),
    ];
    program.constants = vec![AwbcConstant::Unit, AwbcConstant::String(AwbcStringId(1))];
    program.signatures.push(AwbcSignature {
        params: Vec::new(),
        result: None,
        effects: AwbcEffectSetId(0),
    });
    program.effect_plans = vec![AwbcEffectPlan {
        kind: AwbcEffectKind::Wait,
        signature: AwbcSignatureId(1),
        capability: None,
        audio: None,
        static_args: vec![AwbcConstantId(0)],
        resources: Vec::new(),
    }];
    program.frame_layouts = vec![
        AwbcFrameLayout {
            scopes: Vec::new(),
            slots: Vec::new(),
            max_scope_depth: 0,
        },
        AwbcFrameLayout {
            scopes: Vec::new(),
            slots: vec![AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(1),
                role: AwbcFrameSlotRole::Local,
                scope_depth: 0,
            }],
            max_scope_depth: 0,
        },
        AwbcFrameLayout {
            scopes: Vec::new(),
            slots: Vec::new(),
            max_scope_depth: 0,
        },
    ];
    program.functions[0].blocks = AwbcTableRange::new(0, 1);
    program.functions[0].frame_layout = AwbcFrameLayoutId(0);
    program.functions.extend([
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Ordinary,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(1),
            blocks: AwbcTableRange::new(1, 1),
            entry_block: AwbcBlockId(1),
            flags: inner_flags,
        },
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Flow,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(2),
            blocks: AwbcTableRange::new(2, 1),
            entry_block: AwbcBlockId(2),
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        },
    ]);
    program.blocks[0].terminator = AwbcTerminator::Return { value: None };
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(1),
        instructions: AwbcTableRange::new(0, 1),
        terminator: if dynamic {
            AwbcTerminator::GotoDynamic {
                target: AwbcRegisterId(0),
                args: Vec::new(),
            }
        } else {
            AwbcTerminator::GotoStatic {
                function: AwbcFunctionId(2),
                args: Vec::new(),
            }
        },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(2),
        instructions: AwbcTableRange::new(1, 0),
        terminator: AwbcTerminator::Return { value: None },
        safe_point: AwbcSafePointKind::FlowEntry,
        source_map: None,
    });
    program.instructions.push(AwbcInstruction::LoadConst {
        dst: AwbcRegisterId(0),
        constant: AwbcConstantId(1),
    });
    program.flow_bindings.push(test_flow_binding("target", 2));
    program
}

fn project_call_target_goto_program(dynamic: bool) -> AwbcProgram {
    let mut program = project_call_invoke_program();
    program.strings.push("goto-target".to_owned());
    program
        .runtime_types
        .push(runtime_type(3, AwbcRuntimeTypeShape::String));
    program
        .constants
        .push(AwbcConstant::String(AwbcStringId(1)));
    if dynamic {
        program.frame_layouts[1].slots[0].ty = AwbcTypeId(2);
        program.functions[1].flags = AwbcFunctionFlags::empty()
            .with(AwbcFunctionFlag::Deterministic)
            .with(AwbcFunctionFlag::HasDynamicTarget);
        let target_instruction = usize::try_from(program.blocks[2].instructions.start)
            .expect("target instruction offset fits usize");
        program.instructions[target_instruction] = AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(1),
        };
    }
    program.blocks[2].terminator = if dynamic {
        AwbcTerminator::GotoDynamic {
            target: AwbcRegisterId(0),
            args: Vec::new(),
        }
    } else {
        AwbcTerminator::GotoStatic {
            function: AwbcFunctionId(2),
            args: Vec::new(),
        }
    };
    program.functions.push(AwbcFunction {
        public_id: None,
        kind: AwbcFunctionKind::Flow,
        signature: AwbcSignatureId(0),
        frame_layout: AwbcFrameLayoutId(0),
        blocks: AwbcTableRange::new(3, 1),
        entry_block: AwbcBlockId(3),
        flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
    });
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(2),
        instructions: AwbcTableRange::new(1, 0),
        terminator: AwbcTerminator::Return { value: None },
        safe_point: AwbcSafePointKind::FlowEntry,
        source_map: None,
    });
    program
        .flow_bindings
        .push(test_flow_binding("goto-target", 2));
    program.canonicalize_string_table();
    program
}

fn project_call_default_goto_program() -> AwbcProgram {
    let mut program = project_call_default_program();
    program.blocks[2].terminator = AwbcTerminator::GotoStatic {
        function: AwbcFunctionId(3),
        args: Vec::new(),
    };
    program.functions.push(AwbcFunction {
        public_id: None,
        kind: AwbcFunctionKind::Flow,
        signature: AwbcSignatureId(0),
        frame_layout: AwbcFrameLayoutId(0),
        blocks: AwbcTableRange::new(4, 1),
        entry_block: AwbcBlockId(4),
        flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
    });
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(3),
        instructions: AwbcTableRange::new(2, 0),
        terminator: AwbcTerminator::Return { value: None },
        safe_point: AwbcSafePointKind::FlowEntry,
        source_map: None,
    });
    program
        .flow_bindings
        .push(test_flow_binding("default-goto-target", 3));
    program
}

#[test]
fn opcode_owner_exhaustively_seals_every_v1_byte_and_family() {
    use AwbcOpcodeFamily::{CallTask, Ownership, StreamLine, Terminator, Value};

    let expected = [
        (AwbcOpcode::Nop, 0x00, Value),
        (AwbcOpcode::LoadConst, 0x01, Value),
        (AwbcOpcode::MakeTuple, 0x02, Value),
        (AwbcOpcode::MakeSequence, 0x03, Value),
        (AwbcOpcode::RepeatSequence, 0x04, Value),
        (AwbcOpcode::MakeRecord, 0x05, Value),
        (AwbcOpcode::MakeVariant, 0x06, Value),
        (AwbcOpcode::MakeCallable, 0x07, Value),
        (AwbcOpcode::MakeAgent, 0x08, Value),
        (AwbcOpcode::MakeReductionUnchanged, 0x09, Value),
        (AwbcOpcode::SequenceLen, 0x0a, Value),
        (AwbcOpcode::SequenceGet, 0x0b, Value),
        (AwbcOpcode::SequenceSlice, 0x0c, Value),
        (AwbcOpcode::SequencePush, 0x0d, Value),
        (AwbcOpcode::ProjectTuple, 0x0e, Value),
        (AwbcOpcode::ProjectRecord, 0x0f, Value),
        (AwbcOpcode::ProjectField, 0x10, Value),
        (AwbcOpcode::AssignRecordField, 0x11, Value),
        (AwbcOpcode::TestPattern, 0x12, Value),
        (AwbcOpcode::Unary, 0x13, Value),
        (AwbcOpcode::Binary, 0x14, Value),
        (AwbcOpcode::SpecializeCallable, 0x15, Value),
        (AwbcOpcode::CallPureHelper, 0x20, CallTask),
        (AwbcOpcode::CallIntrinsic, 0x21, CallTask),
        (AwbcOpcode::CallTraitMethod, 0x22, CallTask),
        (AwbcOpcode::ApplyGroup, 0x23, CallTask),
        (AwbcOpcode::EnsureContent, 0x24, CallTask),
        (AwbcOpcode::EmitEffect, 0x25, CallTask),
        (AwbcOpcode::StartTask, 0x26, CallTask),
        (AwbcOpcode::SpawnFiber, 0x27, CallTask),
        (AwbcOpcode::MakeDialogueContent, 0x28, CallTask),
        (AwbcOpcode::CharacterDialogue, 0x29, CallTask),
        (AwbcOpcode::StreamYield, 0x32, StreamLine),
        (AwbcOpcode::StreamClose, 0x34, StreamLine),
        (AwbcOpcode::ExecuteLineOperation, 0x35, StreamLine),
        (AwbcOpcode::CommitDialogueResult, 0x36, StreamLine),
        (AwbcOpcode::Move, 0x40, Ownership),
        (AwbcOpcode::CopyValue, 0x41, Ownership),
        (AwbcOpcode::Clear, 0x42, Ownership),
        (AwbcOpcode::Drop, 0x43, Ownership),
        (AwbcOpcode::EnterScope, 0x44, Ownership),
        (AwbcOpcode::ExitScope, 0x45, Ownership),
        (AwbcOpcode::BindPattern, 0x46, Ownership),
        (AwbcOpcode::RegisterCleanup, 0x47, Ownership),
        (AwbcOpcode::CancelCleanup, 0x48, Ownership),
        (AwbcOpcode::RegisterDefer, 0x49, Ownership),
        (AwbcOpcode::Jump, 0x80, Terminator),
        (AwbcOpcode::Branch, 0x81, Terminator),
        (AwbcOpcode::Match, 0x82, Terminator),
        (AwbcOpcode::CallFunction, 0x83, Terminator),
        (AwbcOpcode::GotoStatic, 0x84, Terminator),
        (AwbcOpcode::GotoDynamic, 0x85, Terminator),
        (AwbcOpcode::Return, 0x86, Terminator),
        (AwbcOpcode::ProjectCall, 0x87, Terminator),
        (AwbcOpcode::HostCall, 0x88, Terminator),
        (AwbcOpcode::Await, 0x89, Terminator),
        (AwbcOpcode::AwaitMany, 0x8a, Terminator),
        (AwbcOpcode::BudgetYield, 0x8b, Terminator),
        (AwbcOpcode::Dialogue, 0x98, Terminator),
        (AwbcOpcode::Choice, 0x99, Terminator),
        (AwbcOpcode::Trap, 0xa0, Terminator),
        (AwbcOpcode::Unreachable, 0xa1, Terminator),
    ];
    assert_eq!(AwbcOpcode::ALL, expected.map(|(opcode, _, _)| opcode));

    for (opcode, encoded, family) in expected {
        assert_eq!(opcode.encoded(), encoded);
        assert_eq!(opcode.family(), family);
        assert_eq!(opcode.class(), family.class());
        assert_eq!(AwbcOpcode::from_encoded(encoded), Some(opcode));
        assert_eq!(
            serde_json::to_value(opcode).expect("opcode serializes numerically"),
            serde_json::json!(encoded)
        );
        assert_eq!(
            serde_json::from_value::<AwbcOpcode>(serde_json::json!(encoded))
                .expect("opcode numeric Serde round trip"),
            opcode
        );
    }

    for encoded in u8::MIN..=u8::MAX {
        let expected = AwbcOpcode::ALL
            .iter()
            .copied()
            .find(|opcode| opcode.encoded() == encoded);
        assert_eq!(AwbcOpcode::from_encoded(encoded), expected);
    }
    assert!(serde_json::from_value::<AwbcOpcode>(serde_json::json!(0xff)).is_err());
}

#[test]
fn project_call_callable_state_graph_verifies_and_roundtrips() {
    let program = project_call_retained_program();
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("two-group callable ProjectCall program verifies");
    let encoded = program
        .encode_canonical()
        .expect("encode program-owned callable states");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode program-owned callable states");
    assert_eq!(decoded, program);
}

#[test]
fn project_call_invoke_rejoins_the_verified_site_after_target_return() {
    let program = std::sync::Arc::new(project_call_invoke_program());
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("invoking ProjectCall program verifies");

    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).expect("fiber");
    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("execute invoking ProjectCall");
    assert_eq!(output.exit, super::vm::VmExit::Returned(None));
    assert_eq!(fiber.frames.len(), 1);
}

#[test]
fn project_call_retained_callable_survives_snapshot_and_rejoins_next_group() {
    let program = std::sync::Arc::new(project_call_retained_program());
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).expect("fiber");
    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("first group retains its callable state before the yield");
    assert_eq!(
        output.exit,
        super::vm::VmExit::Suspended(FiberSuspensionReason::BudgetYield)
    );
    let RuntimeValue::Callable(retained) = fiber
        .active_frame()
        .expect("caller frame")
        .register(AwbcRegisterId(1))
        .expect("retained callable register")
    else {
        panic!("the first group must return a callable state");
    };
    assert_eq!(retained.state(), callable_state_id(1));
    assert_eq!(retained.retained(), [RuntimeValue::Unit]);

    let snapshot = AwbcFiberStateSnapshot::from_live(&fiber).expect("snapshot retained callable");
    let serialized = serde_json::to_vec(&snapshot).expect("serialize retained callable snapshot");
    let snapshot: AwbcFiberStateSnapshot =
        serde_json::from_slice(&serialized).expect("decode retained callable snapshot");
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&program));
    let mut restored = snapshot
        .into_live_for_program(&owner)
        .expect("restore retained callable for the same AWBC program");
    restored
        .validate_for_program(&program)
        .expect("restored callable lease and retained layout validate");
    assert_eq!(restored, fiber);

    restored
        .resume_at(&program, AwbcResumePointId(1))
        .expect("resume into the next checked group");
    assert_eq!(
        step_with_callable_context(
            &program,
            &mut restored,
            super::vm::VmStepOptions {
                max_instructions: 16,
            },
        )
        .expect("complete the retained callable's next group")
        .exit,
        super::vm::VmExit::Returned(None)
    );
}

#[test]
fn project_call_defaulted_omitted_runs_default_once_then_target() {
    let program = std::sync::Arc::new(project_call_default_program());
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("defaulted ProjectCall program verifies");
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).expect("fiber");
    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 32,
        },
    )
    .expect("execute defaulted ProjectCall");
    assert_eq!(output.exit, super::vm::VmExit::Returned(None));
    assert_eq!(fiber.frames.len(), 1);
}

#[test]
fn callable_arrow_omission_runs_default_then_target_without_an_extra_argument() {
    let mut program = project_call_default_program();
    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(0),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    program.instructions.insert(
        1,
        AwbcInstruction::ApplyGroup {
            dst: AwbcRegisterId(1),
            callee: AwbcRegisterId(0),
            args: Vec::new(),
        },
    );
    program.blocks[0].instructions = AwbcTableRange::new(0, 2);
    program.blocks[0].terminator = AwbcTerminator::Jump {
        target: AwbcBlockId(1),
    };
    program.blocks[2].instructions = AwbcTableRange::new(2, 1);
    program.blocks[3].instructions = AwbcTableRange::new(3, 1);
    let program = std::sync::Arc::new(program);
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).unwrap();
    for expected in [AwbcFunctionId(1), AwbcFunctionId(2)] {
        let output = step_with_callable_context(
            &program,
            &mut fiber,
            super::vm::VmStepOptions {
                max_instructions: 2,
            },
        )
        .unwrap();
        assert_eq!(output.exit, super::vm::VmExit::Running);
        assert_eq!(fiber.cursor.function, expected);
        assert_eq!(fiber.frames.len(), 2);
    }
    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .unwrap();
    assert_eq!(output.exit, super::vm::VmExit::Returned(None));
    assert_eq!(fiber.frames.len(), 1);
}

#[test]
fn project_call_supplied_attached_enters_target_without_running_default() {
    let mut program = project_call_default_program();
    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(0),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    program.instructions.insert(
        1,
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(0),
        },
    );
    program.blocks[0].instructions = AwbcTableRange::new(0, 2);
    program.blocks[2].instructions = AwbcTableRange::new(2, 1);
    program.blocks[3].instructions = AwbcTableRange::new(3, 1);
    let call = project_call_mut(&mut program);
    call.operands = vec![AwbcProjectCallOperand {
        value: AwbcRegisterId(1),
        mode: AwbcProjectCallOperandMode::Value,
    }];
    call.attached = Some(AwbcProjectCallAttachedMaterialization {
        source_index: Some(0),
        presence: AwbcProjectCallAttachedPresence::DefaultedPresent,
    });
    let program = std::sync::Arc::new(program);
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).unwrap();
    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 3,
        },
    )
    .unwrap();
    assert_eq!(output.exit, super::vm::VmExit::Running);
    assert_eq!(fiber.cursor.function, AwbcFunctionId(2));
    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .unwrap();
    assert_eq!(output.exit, super::vm::VmExit::Returned(None));
}

#[test]
fn callable_verifier_rejects_attached_content_flattened_into_the_function_arrow() {
    let mut program = project_call_default_program();
    program.runtime_types[3] = runtime_type(
        4,
        AwbcRuntimeTypeShape::Function {
            contract: RuntimeFunctionTypeContract::default(),
            parameters: vec![AwbcTypeId(2)],
            result: AwbcTypeId(0),
        },
    );
    expect_project_call_rejection(
        program,
        "callable function arrow disagrees with its state layout",
    );
}

#[test]
fn project_call_rejects_flat_option_payload_in_type_inventory() {
    let mut program = project_call_default_program();
    let AwbcRuntimeTypeShape::Variant {
        owner,
        arguments,
        mut cases,
    } = program.runtime_types[2].shape().clone()
    else {
        unreachable!()
    };
    cases[0].payload = Some(AwbcTypeId(0));
    program.runtime_types[2] = runtime_type(
        3,
        AwbcRuntimeTypeShape::Variant {
            owner,
            arguments,
            cases,
        },
    );
    let error = program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("flat Option payload must not be admitted");
    assert!(matches!(
        error,
        AwbcVerifyError::InvalidInvariant { message, .. }
            if message.as_str() == super::type_projection::AwbcTypeProjectionError::InvalidBuiltinVariant { index: 2 }.to_string().as_str()
    ));
}

#[test]
fn project_call_verifier_rejects_a_group_that_disagrees_with_its_callable_state() {
    let mut initial = project_call_invoke_program();
    project_call_mut(&mut initial).completed_group = 1;
    expect_project_call_rejection(
        initial,
        "project-call group disagrees with its checked state position",
    );

    let mut retained = project_call_retained_program();
    project_call_at_mut(&mut retained, 2).completed_group = 0;
    expect_project_call_rejection(
        retained,
        "project-call group disagrees with its checked state position",
    );
}

#[test]
fn project_call_verifier_applies_the_call_row_budget_before_register_walk() {
    let mut program = project_call_retained_program();
    project_call_mut(&mut program).operands[0].value = AwbcRegisterId(99);
    let mut budget = AwbcVerifyBudget::default();
    budget.args_per_call = 0;
    assert_eq!(
        program.verify(budget, AwbcVerifyContext::default()),
        Err(AwbcVerifyError::BudgetExceeded {
            budget: "args_per_call"
        })
    );
}

#[test]
fn project_call_verifier_rejects_invalid_callable_state_shapes() {
    let mut non_function = project_call_invoke_program();
    non_function.callable_states[0].function_type = AwbcTypeId(0);
    expect_project_call_rejection(
        non_function,
        "callable state function type is absent or not a function",
    );

    let mut dynamic_abi = project_call_fixed_program();
    let dynamic_type = AwbcTypeId(u32::try_from(dynamic_abi.runtime_types.len()).unwrap());
    dynamic_abi.runtime_types.push(AwbcRuntimeType::dynamic());
    dynamic_abi.runtime_types[1] = runtime_type(
        2,
        AwbcRuntimeTypeShape::Function {
            contract: RuntimeFunctionTypeContract::default(),
            parameters: vec![dynamic_type],
            result: AwbcTypeId(0),
        },
    );
    dynamic_abi.signatures[1].params = vec![dynamic_type];
    dynamic_abi.frame_layouts[1].slots[0].ty = dynamic_type;
    dynamic_abi.callable_states[0].parameters[0].abi_ty = dynamic_type;
    dynamic_abi.callable_states[0].parameters[0].binding_ty = dynamic_type;
    expect_project_call_rejection(dynamic_abi, "project-call ABI cannot use Dynamic types");
}

#[test]
fn project_call_verifier_rejects_noncanonical_logical_and_source_rows() {
    let mut sparse_parameter = project_call_two_fixed_parameters_program();
    if let Some(AwbcProjectCallOrdinaryMaterialization::Fixed { parameter, .. }) =
        project_call_mut(&mut sparse_parameter).ordinary.first_mut()
    {
        *parameter = 1;
    }
    expect_project_call_rejection(
        sparse_parameter,
        "project-call logical parameter rows are not dense",
    );

    let mut fixed_spread = project_call_fixed_program();
    project_call_mut(&mut fixed_spread).operands[0].mode = AwbcProjectCallOperandMode::Spread;
    expect_project_call_rejection(
        fixed_spread,
        "fixed project-call source must be a value operand",
    );

    let mut repeated_source = project_call_default_program();
    // One physical source cannot bind both an ordinary parameter and the attached value.
    repeated_source.runtime_types.push(runtime_type(
        5,
        AwbcRuntimeTypeShape::Function {
            contract: RuntimeFunctionTypeContract::default(),
            parameters: vec![AwbcTypeId(0)],
            result: AwbcTypeId(0),
        },
    ));
    repeated_source.callable_states[0].function_type = AwbcTypeId(4);
    repeated_source.frame_layouts[0].slots[0].ty = AwbcTypeId(4);
    repeated_source.callable_states[0].parameters = vec![RuntimeCallableParameterInput {
        coordinate: RuntimeCallableParameterCoordinate {
            group: 0,
            parameter: 0,
        },
        kind: RuntimeCallableParameterKind::Fixed,
        abi_ty: AwbcTypeId(0),
        binding_ty: AwbcTypeId(0),
    }]
    .into_boxed_slice();
    repeated_source.callable_states[0].transition = RuntimeCallableTransition::Invoke {
        function: AwbcFunctionId(2),
        captures: Box::new([]),
        arguments: vec![
            RuntimeCallableInputSource::Argument { position: 0 },
            RuntimeCallableInputSource::Attached,
        ]
        .into_boxed_slice(),
    };
    repeated_source.signatures[2].params = vec![AwbcTypeId(0), AwbcTypeId(0)];
    repeated_source.frame_layouts[2].slots[0].ty = AwbcTypeId(0);
    repeated_source.frame_layouts[2].slots[1].role = AwbcFrameSlotRole::Parameter;
    repeated_source.frame_layouts[2].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(0),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    repeated_source.blocks[3].terminator = AwbcTerminator::Return {
        value: Some(AwbcRegisterId(2)),
    };
    repeated_source.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(0),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    repeated_source.instructions.insert(
        1,
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(0),
        },
    );
    repeated_source.instructions[3] = AwbcInstruction::LoadConst {
        dst: AwbcRegisterId(2),
        constant: AwbcConstantId(0),
    };
    repeated_source.blocks[0].instructions = AwbcTableRange::new(0, 2);
    repeated_source.blocks[2].instructions = AwbcTableRange::new(2, 1);
    repeated_source.blocks[3].instructions = AwbcTableRange::new(3, 1);
    let call = project_call_mut(&mut repeated_source);
    call.operands = vec![AwbcProjectCallOperand {
        value: AwbcRegisterId(1),
        mode: AwbcProjectCallOperandMode::Value,
    }];
    call.ordinary = vec![AwbcProjectCallOrdinaryMaterialization::Fixed {
        parameter: 0,
        source_index: 0,
    }];
    call.attached = Some(AwbcProjectCallAttachedMaterialization {
        source_index: Some(0),
        presence: AwbcProjectCallAttachedPresence::DefaultedPresent,
    });
    expect_project_call_rejection(repeated_source, "project-call source operand is repeated");

    let mut unconsumed_source = project_call_fixed_program();
    project_call_mut(&mut unconsumed_source)
        .operands
        .push(AwbcProjectCallOperand {
            value: AwbcRegisterId(99),
            mode: AwbcProjectCallOperandMode::Value,
        });
    expect_project_call_rejection(
        unconsumed_source,
        "project-call source operand is not consumed",
    );
}

#[test]
fn project_call_verifier_rejects_rest_shape_and_spread_type_mismatches() {
    let mut non_sequence_binding = project_call_fixed_program();
    non_sequence_binding.callable_states[0].parameters[0].kind = RuntimeCallableParameterKind::Rest;
    expect_project_call_rejection(
        non_sequence_binding,
        "callable parameter ABI/binding shape is invalid",
    );

    for kind in [
        crate::plan::RuntimePlanSequenceKind::Seq,
        crate::plan::RuntimePlanSequenceKind::Slice,
        crate::plan::RuntimePlanSequenceKind::Array,
    ] {
        let mut non_vec_binding = project_call_fixed_program();
        let binding_type = AwbcTypeId(u32::try_from(non_vec_binding.runtime_types.len()).unwrap());
        non_vec_binding.runtime_types.push(runtime_type(
            3,
            AwbcRuntimeTypeShape::Sequence {
                kind,
                item: AwbcTypeId(0),
            },
        ));
        non_vec_binding.callable_states[0].parameters[0].kind = RuntimeCallableParameterKind::Rest;
        non_vec_binding.callable_states[0].parameters[0].binding_ty = binding_type;
        non_vec_binding.signatures[1].params = vec![binding_type];
        non_vec_binding.frame_layouts[1].slots[0].ty = binding_type;
        let error = non_vec_binding
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .expect_err("rest binding must preserve its Vec family");
        assert!(
            error
                .to_string()
                .contains("callable parameter ABI/binding shape is invalid"),
            "{kind:?}: {error:?}"
        );
    }

    let mut wrong_spread_item = project_call_fixed_program();
    wrong_spread_item.runtime_types.push(runtime_type(
        3,
        AwbcRuntimeTypeShape::Sequence {
            kind: crate::plan::RuntimePlanSequenceKind::Vec,
            item: AwbcTypeId(0),
        },
    ));
    wrong_spread_item.callable_states[0].parameters[0].kind = RuntimeCallableParameterKind::Rest;
    wrong_spread_item.callable_states[0].parameters[0].binding_ty = AwbcTypeId(2);
    wrong_spread_item.signatures[1].params = vec![AwbcTypeId(2)];
    wrong_spread_item.frame_layouts[1].slots[0].ty = AwbcTypeId(2);
    let call = project_call_mut(&mut wrong_spread_item);
    call.operands[0].mode = AwbcProjectCallOperandMode::Spread;
    call.ordinary = vec![AwbcProjectCallOrdinaryMaterialization::Rest {
        parameter: 0,
        source_indices: vec![0],
    }];
    expect_project_call_rejection(
        wrong_spread_item,
        "rest spread source has the wrong item type",
    );
}

#[test]
fn awbc_sequence_kinds_roundtrip_as_distinct_runtime_types() {
    let kinds = [
        crate::plan::RuntimePlanSequenceKind::Vec,
        crate::plan::RuntimePlanSequenceKind::Array,
        crate::plan::RuntimePlanSequenceKind::Slice,
        crate::plan::RuntimePlanSequenceKind::Seq,
    ];
    let mut program = minimal_program();
    program.runtime_types = vec![runtime_type(1, AwbcRuntimeTypeShape::Bool)];
    program
        .runtime_types
        .extend(kinds.iter().enumerate().map(|(index, kind)| {
            runtime_type(
                u8::try_from(index + 2).unwrap(),
                AwbcRuntimeTypeShape::Sequence {
                    kind: *kind,
                    item: AwbcTypeId(0),
                },
            )
        }));

    let encoded = program
        .encode_canonical()
        .expect("encode all sequence kinds");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode all sequence kinds");
    assert_eq!(decoded.runtime_types, program.runtime_types);
    assert_eq!(decoded.encode_canonical().unwrap(), encoded);
    for (index, expected) in kinds.iter().enumerate() {
        assert!(matches!(
            decoded.runtime_types[index + 1].shape(),
            AwbcRuntimeTypeShape::Sequence { kind, item: AwbcTypeId(0) } if kind == expected
        ));
    }
}

#[test]
fn project_call_verifier_rejects_attached_presence_and_source_mismatches() {
    let mut parity = project_call_default_program();
    project_call_mut(&mut parity)
        .attached
        .as_mut()
        .expect("default fixture has attached row")
        .source_index = Some(0);
    expect_project_call_rejection(parity, "project-call attached source parity is invalid");

    let mut optional_wrong_type = project_call_default_program();
    optional_wrong_type.callable_states[0].attached = RuntimeCallableAttachedContract::Optional {
        value: AwbcTypeId(0),
        binding: AwbcTypeId(0),
    };
    expect_project_call_rejection(
        optional_wrong_type,
        "optional attached binding must be Option of its value type",
    );

    let mut required_spread = project_call_default_program();
    required_spread.callable_states[0].attached =
        RuntimeCallableAttachedContract::Required { ty: AwbcTypeId(0) };
    let call = project_call_mut(&mut required_spread);
    call.operands.push(AwbcProjectCallOperand {
        value: AwbcRegisterId(0),
        mode: AwbcProjectCallOperandMode::Spread,
    });
    let attached = call
        .attached
        .as_mut()
        .expect("default fixture has attached row");
    attached.source_index = Some(0);
    attached.presence = AwbcProjectCallAttachedPresence::RequiredPresent;
    expect_project_call_rejection(required_spread, "attached source must be a value operand");

    let mut omitted_with_source = project_call_default_program();
    let attached = project_call_mut(&mut omitted_with_source)
        .attached
        .as_mut()
        .expect("default fixture has attached row");
    attached.presence = AwbcProjectCallAttachedPresence::DefaultedOmitted;
    attached.source_index = Some(0);
    expect_project_call_rejection(
        omitted_with_source,
        "project-call attached source parity is invalid",
    );
}

#[test]
fn project_call_verifier_rejects_invalid_retained_state_projections() {
    let valid = project_call_retained_program();
    valid
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("the next group is sealed by the retained state definition");

    let mut dropped_prefix = project_call_retained_program();
    dropped_prefix.callable_states[1].retained = Box::new([]);
    expect_project_call_rejection(
        dropped_prefix,
        "callable retained transition has an incompatible target",
    );

    let mut non_function = project_call_retained_program();
    non_function.callable_states[1].function_type = AwbcTypeId(0);
    expect_project_call_rejection(
        non_function,
        "callable retained transition has an incompatible target",
    );

    let mut changed_parameters = project_call_retained_program();
    changed_parameters
        .runtime_types
        .push(runtime_type(4, AwbcRuntimeTypeShape::Bool));
    changed_parameters.runtime_types[2] = runtime_type(
        3,
        AwbcRuntimeTypeShape::Function {
            contract: RuntimeFunctionTypeContract::default(),
            parameters: vec![AwbcTypeId(3)],
            result: AwbcTypeId(0),
        },
    );
    expect_project_call_rejection(
        changed_parameters,
        "callable function arrow disagrees with its state layout",
    );

    let mut noncanonical_prefix = project_call_retained_program();
    noncanonical_prefix.callable_states[1].parameters[0]
        .coordinate
        .group = 2;
    expect_project_call_rejection(
        noncanonical_prefix,
        "callable parameter coordinate disagrees with its position",
    );

    let mut changed_binding = project_call_retained_program();
    changed_binding.callable_states[1].retained[0].ty = AwbcTypeId(2);
    expect_project_call_rejection(
        changed_binding,
        "callable retained transition changes a typed role",
    );

    let mut attached_continue = project_call_default_program();
    attached_continue.callable_states[0].transition = RuntimeCallableTransition::Retain {
        state: callable_state_id(0),
        values: Box::new([]),
    };
    expect_project_call_rejection(
        attached_continue,
        "callable retained transition has an incompatible target",
    );
}

#[test]
fn project_call_verifier_rejects_invoke_target_identity_and_result_mismatches() {
    let mut missing_target = project_call_invoke_program();
    let RuntimeCallableTransition::Invoke { function, .. } =
        &mut missing_target.callable_states[0].transition
    else {
        unreachable!();
    };
    *function = AwbcFunctionId(99);
    expect_project_call_rejection(missing_target, "functions` index 99 is out of bounds");

    let mut missing_signature = project_call_invoke_program();
    missing_signature.functions[1].signature = AwbcSignatureId(99);
    let error = missing_signature
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("out-of-range invocation signature must reject");
    assert!(
        matches!(
            error,
            AwbcVerifyError::IndexOutOfBounds {
                table: "signatures",
                index: 99,
                ..
            }
        ),
        "{error:?}"
    );

    let mut result_mismatch = project_call_invoke_program();
    result_mismatch.signatures[1].result = Some(AwbcTypeId(1));
    expect_project_call_rejection(
        result_mismatch,
        "callable transition target ABI disagrees with typed projections",
    );

    let mut target_kind = project_call_invoke_program();
    target_kind.functions[1].kind = AwbcFunctionKind::Flow;
    expect_project_call_rejection(
        target_kind,
        "callable transition target must be an executable function",
    );

    let mut default_noninvoke = project_call_default_program();
    default_noninvoke.callable_states[0].transition = RuntimeCallableTransition::Retain {
        state: callable_state_id(0),
        values: Box::new([]),
    };
    expect_project_call_rejection(
        default_noninvoke,
        "callable retained transition has an incompatible target",
    );
}

#[test]
fn project_call_verifier_rejects_default_capture_domain_and_effect_mismatches() {
    let mut direct_prefix = project_call_default_program();
    let RuntimeCallableAttachedContract::Defaulted { default, .. } =
        &mut direct_prefix.callable_states[0].attached
    else {
        unreachable!();
    };
    let RuntimeCallableDefault::Body { captures, .. } = default else {
        unreachable!();
    };
    *captures = vec![RuntimeCallableInputSource::Attached].into_boxed_slice();
    expect_project_call_rejection(direct_prefix, "default callable capture source is invalid");

    let mut out_of_range = project_call_default_program();
    let RuntimeCallableAttachedContract::Defaulted { default, .. } =
        &mut out_of_range.callable_states[0].attached
    else {
        unreachable!();
    };
    let RuntimeCallableDefault::Body { captures, .. } = default else {
        unreachable!();
    };
    *captures = vec![RuntimeCallableInputSource::Argument { position: 0 }].into_boxed_slice();
    expect_project_call_rejection(out_of_range, "default callable capture source is invalid");

    let mut default_effects = project_call_default_program();
    default_effects.signatures[1].effects = add_effect_set(&mut default_effects, &["fs.read"]);
    default_effects.canonicalize_string_table();
    expect_project_call_rejection(default_effects, "callable default");

    let mut target_effects = project_call_invoke_program();
    target_effects.signatures[1].effects = add_effect_set(&mut target_effects, &["fs.read"]);
    target_effects.canonicalize_string_table();
    expect_project_call_rejection(target_effects, "callable invocation");
}

#[test]
fn project_call_verifier_applies_each_project_call_argument_budget() {
    let mut operand_budget = project_call_retained_program();
    project_call_mut(&mut operand_budget).operands[0].value = AwbcRegisterId(99);
    let mut budget = AwbcVerifyBudget::default();
    budget.args_per_call = 0;
    expect_project_call_rejection_with_budget(operand_budget, budget, "args_per_call");

    let mut row_budget = project_call_retained_program();
    project_call_mut(&mut row_budget).operands.clear();
    let mut budget = AwbcVerifyBudget::default();
    budget.args_per_call = 0;
    expect_project_call_rejection_with_budget(row_budget, budget, "args_per_call");
}

fn expect_project_call_rejection_with_budget(
    program: AwbcProgram,
    budget: AwbcVerifyBudget,
    message: &str,
) {
    let error = program
        .verify(budget, AwbcVerifyContext::default())
        .expect_err("malformed ProjectCall must be rejected");
    assert!(
        error.to_string().contains(message),
        "expected ProjectCall rejection containing {message:?}, got {error:?}"
    );
}

#[test]
fn project_call_default_and_target_stages_snapshot_with_verified_rejoin() {
    let program = std::sync::Arc::new(project_call_default_program());
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("defaulted ProjectCall program verifies");

    let mut default_fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).expect("fiber");
    let output = step_with_callable_context(
        &program,
        &mut default_fiber,
        super::vm::VmStepOptions {
            max_instructions: 2,
        },
    )
    .expect("enter default function");
    assert_eq!(output.exit, super::vm::VmExit::Running);
    assert_eq!(default_fiber.frames.len(), 2);
    default_fiber
        .validate_for_program(&program)
        .expect("default stage validates");
    let default_snapshot = AwbcFiberStateSnapshot::from_live(&default_fiber).expect("snapshot");
    let default_restored = default_snapshot
        .into_live_for_program(&RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&program)))
        .expect("restore snapshot");
    default_restored
        .validate_for_program(&program)
        .expect("restored default stage validates");

    let mut target_fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).expect("fiber");
    let output = step_with_callable_context(
        &program,
        &mut target_fiber,
        super::vm::VmStepOptions {
            max_instructions: 4,
        },
    )
    .expect("enter target function after default");
    assert_eq!(output.exit, super::vm::VmExit::Running);
    assert_eq!(target_fiber.frames.len(), 2);
    assert_eq!(target_fiber.cursor.function, AwbcFunctionId(2));
    target_fiber
        .validate_for_program(&program)
        .expect("target stage validates");
    let target_snapshot = AwbcFiberStateSnapshot::from_live(&target_fiber).expect("snapshot");
    let mut target_restored = target_snapshot
        .into_live_for_program(&RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&program)))
        .expect("restore snapshot");
    target_restored
        .validate_for_program(&program)
        .expect("restored target stage validates");
    let return_to = target_restored.frames[1]
        .return_to
        .as_mut()
        .expect("target return point");
    if let FiberReturnContinuation::ProjectCallTarget { site } = &mut return_to.continuation {
        site.caller_function = AwbcFunctionId(99);
    } else {
        panic!("target frame must carry a ProjectCall continuation");
    }
    assert!(matches!(
        target_restored.validate_for_program(&program),
        Err(super::fiber::FiberStateError::InvalidFrame)
    ));
}

#[test]
fn goto_static_and_dynamic_unwind_every_call_frame_without_project_call_return() {
    for dynamic in [false, true] {
        let mut program = goto_unwind_program(dynamic);
        let effects = add_effect_set(&mut program, &["fs.read"]);
        let signature = AwbcSignatureId(u32::try_from(program.signatures.len()).unwrap());
        program.signatures.push(AwbcSignature {
            params: Vec::new(),
            result: None,
            effects,
        });
        program.functions[2].signature = signature;
        program.canonicalize_string_table();
        program
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .expect("goto unwind program verifies");
        let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).expect("fiber");
        fiber.frames[0].root_cleanups.push(FiberScopeCleanup {
            key: "caller-cleanup".to_owned(),
            effect: AwbcEffectPlanId(0),
            args: Vec::new(),
        });
        fiber
            .push_call_frame_at(
                &program,
                AwbcFunctionId(1),
                super::fiber::FiberReturnPoint::ordinary(
                    super::fiber::FiberCursor {
                        function: AwbcFunctionId(0),
                        block: AwbcBlockId(0),
                        instruction_offset: 0,
                    },
                    None,
                ),
                &[],
            )
            .expect("push nested goto frame");
        fiber.frames[1].root_cleanups.push(FiberScopeCleanup {
            key: "callee-cleanup".to_owned(),
            effect: AwbcEffectPlanId(0),
            args: Vec::new(),
        });
        fiber
            .validate_for_program(&program)
            .expect("nested goto frames validate before transfer");
        let output = super::vm::step(
            &program,
            &mut fiber,
            super::vm::VmStepOptions {
                max_instructions: 16,
            },
        )
        .expect("execute nonlocal goto");
        assert_eq!(output.exit, super::vm::VmExit::Returned(None));
        assert_eq!(fiber.frames.len(), 1);
        assert_eq!(
            fiber.active_frame().expect("new root frame").function,
            AwbcFunctionId(2)
        );
        assert!(
            fiber
                .active_frame()
                .expect("new root frame")
                .return_to
                .is_none()
        );
        let cleanup_effects = output
            .observations
            .iter()
            .filter_map(|observation| match observation {
                super::vm::VmObservation::Effect { effect, .. } => Some(*effect),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            cleanup_effects,
            vec![AwbcEffectPlanId(0), AwbcEffectPlanId(0)]
        );
    }
}

#[test]
fn flow_transfer_keeps_target_effect_and_capability_validation() {
    for dynamic in [false, true] {
        let mut program = goto_unwind_program(dynamic);
        let effects = add_effect_set(&mut program, &["fs.read"]);
        let signature = AwbcSignatureId(u32::try_from(program.signatures.len()).unwrap());
        program.signatures.push(AwbcSignature {
            params: Vec::new(),
            result: None,
            effects,
        });
        program.functions[2].signature = signature;
        program.signatures[1].effects = effects;
        program.instructions.push(AwbcInstruction::EmitEffect {
            effect: AwbcEffectPlanId(0),
            args: Vec::new(),
        });
        program.blocks[2].instructions = AwbcTableRange::new(1, 1);
        program.canonicalize_string_table();
        program
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .expect("transfer changes the active Flow effect scope");
        let denied = std::collections::BTreeSet::new();
        assert!(matches!(
            program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext {
                allowed_effects: Some(&denied), ..AwbcVerifyContext::default()
            }),
            Err(AwbcVerifyError::EffectDenied { effect }) if effect == "fs.read"
        ));

        // The destination's own effect must still fit the destination scope.
        program.signatures[signature.index()].effects = AwbcEffectSetId(0);
        assert!(matches!(
            program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
            Err(AwbcVerifyError::EffectSetMismatch { caller: 2, .. })
        ));
    }
}

#[test]
fn static_flow_transfer_still_validates_argument_arity_and_types() {
    let mut program = goto_unwind_program(false);
    program.blocks[1].terminator = AwbcTerminator::GotoStatic {
        function: AwbcFunctionId(2),
        args: vec![AwbcRegisterId(0)],
    };
    assert!(matches!(
        program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::ArgumentCountMismatch { .. })
    ));
    program.signatures.push(AwbcSignature {
        params: vec![AwbcTypeId(0)],
        result: None,
        effects: AwbcEffectSetId(0),
    });
    program.functions[2].signature = AwbcSignatureId(2);
    program.frame_layouts[2].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(0),
        role: AwbcFrameSlotRole::Parameter,
        scope_depth: 0,
    });
    assert!(matches!(
        program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::TypeMismatch { .. })
    ));
}

#[test]
fn returning_calls_use_the_canonical_scoped_effect_coverage_rule() {
    for project_call in [false, true] {
        for (permitted, required, accepted) in [
            (vec![], "fs.read", false),
            (vec!["fs.read(save)"], "fs.read", true),
            (vec!["fs.read"], "fs.read(save)", true),
            (vec!["fs.read(save)"], "fs.read(other)", false),
            (
                vec!["fs.read(other)", "fs.read(save)", "fs.write"],
                "fs.read",
                true,
            ),
            (
                vec!["fs.read(other)", "fs.read(save)", "fs.write"],
                "fs.read(save)",
                true,
            ),
            (vec!["fs.read.deep", "fs.reader"], "fs.read", false),
        ] {
            let mut program = if project_call {
                project_call_invoke_program()
            } else {
                ordinary_returning_call_program()
            };
            program.signatures[0].effects = add_effect_set(&mut program, &permitted);
            program.signatures[1].effects = add_effect_set(&mut program, &[required]);
            program.canonicalize_string_table();
            let result = program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default());
            if accepted {
                result.expect("scoped effect coverage agrees with semantic analysis");
            } else {
                assert!(
                    matches!(result, Err(AwbcVerifyError::EffectSetMismatch { .. })),
                    "{result:?}"
                );
            }
        }
    }
}

#[test]
fn verifier_rejects_noncanonical_effect_identities_at_wire_admission() {
    for invalid in ["invalid", "fs.read()", "fs.read (save)"] {
        let mut program = minimal_program();
        program.signatures[0].effects = add_effect_set(&mut program, &[invalid]);
        program.canonicalize_string_table();
        assert!(matches!(
            program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
            Err(AwbcVerifyError::InvalidInvariant { at, .. }) if at == "effect set 1"
        ));
    }
}

#[test]
fn project_call_target_goto_abandons_target_continuation_and_unwinds_all_frames() {
    for dynamic in [false, true] {
        let program = std::sync::Arc::new(project_call_target_goto_program(dynamic));
        program
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .expect("ProjectCall target-goto program verifies");
        let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).expect("fiber");
        let output = step_with_callable_context(
            &program,
            &mut fiber,
            super::vm::VmStepOptions {
                max_instructions: 2,
            },
        )
        .expect("enter ProjectCall target");
        assert_eq!(output.exit, super::vm::VmExit::Running);
        assert_eq!(fiber.frames.len(), 2);
        assert!(matches!(
            fiber.frames[1]
                .return_to
                .as_ref()
                .map(|point| &point.continuation),
            Some(FiberReturnContinuation::ProjectCallTarget { .. })
        ));
        fiber.frames[0].root_cleanups.push(FiberScopeCleanup {
            key: "caller-cleanup".to_owned(),
            effect: AwbcEffectPlanId(0),
            args: Vec::new(),
        });
        fiber.frames[1].root_cleanups.push(FiberScopeCleanup {
            key: "target-cleanup".to_owned(),
            effect: AwbcEffectPlanId(0),
            args: Vec::new(),
        });
        let output = step_with_callable_context(
            &program,
            &mut fiber,
            super::vm::VmStepOptions {
                max_instructions: 16,
            },
        )
        .expect("execute target nonlocal goto");
        assert_eq!(output.exit, super::vm::VmExit::Returned(None));
        assert_eq!(fiber.frames.len(), 1);
        assert_eq!(
            fiber.active_frame().expect("new root frame").function,
            AwbcFunctionId(2)
        );
        assert!(
            fiber
                .active_frame()
                .expect("new root frame")
                .return_to
                .is_none()
        );
        let cleanup_effects = output
            .observations
            .iter()
            .filter_map(|observation| match observation {
                super::vm::VmObservation::Effect { effect, .. } => Some(*effect),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            cleanup_effects,
            vec![AwbcEffectPlanId(0), AwbcEffectPlanId(0)]
        );
    }
}

#[test]
fn project_call_default_goto_abandons_default_continuation_and_unwinds_all_frames() {
    let program = std::sync::Arc::new(project_call_default_goto_program());
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("ProjectCall default-goto program verifies");
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).expect("fiber");
    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 2,
        },
    )
    .expect("enter ProjectCall default");
    assert_eq!(output.exit, super::vm::VmExit::Running);
    assert_eq!(fiber.frames.len(), 2);
    assert!(matches!(
        fiber.frames[1]
            .return_to
            .as_ref()
            .map(|point| &point.continuation),
        Some(FiberReturnContinuation::ProjectCallDefault { .. })
    ));
    fiber.frames[0].root_cleanups.push(FiberScopeCleanup {
        key: "caller-cleanup".to_owned(),
        effect: AwbcEffectPlanId(0),
        args: Vec::new(),
    });
    fiber.frames[1].root_cleanups.push(FiberScopeCleanup {
        key: "default-cleanup".to_owned(),
        effect: AwbcEffectPlanId(0),
        args: Vec::new(),
    });
    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("execute default nonlocal goto");
    assert_eq!(output.exit, super::vm::VmExit::Returned(None));
    assert_eq!(fiber.frames.len(), 1);
    assert_eq!(
        fiber.active_frame().expect("new root frame").function,
        AwbcFunctionId(3)
    );
    assert!(
        fiber
            .active_frame()
            .expect("new root frame")
            .return_to
            .is_none()
    );
    let cleanup_effects = output
        .observations
        .iter()
        .filter_map(|observation| match observation {
            super::vm::VmObservation::Effect { effect, .. } => Some(*effect),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        cleanup_effects,
        vec![AwbcEffectPlanId(0), AwbcEffectPlanId(0)]
    );
}

#[test]
fn project_call_target_snapshot_rejoins_verified_site_and_rejects_tampering() {
    let program = std::sync::Arc::new(project_call_invoke_program());
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("invoking ProjectCall program verifies");
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).expect("fiber");
    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 2,
        },
    )
    .expect("enter target function");
    assert_eq!(output.exit, super::vm::VmExit::Running);
    assert_eq!(fiber.frames.len(), 2);
    fiber
        .validate_for_program(&program)
        .expect("live target continuation validates");

    let encoded = serde_json::to_vec(&AwbcFiberStateSnapshot::from_live(&fiber).expect("snapshot"))
        .expect("snapshot codec");
    let snapshot: AwbcFiberStateSnapshot =
        serde_json::from_slice(&encoded).expect("snapshot decode");
    let mut restored = snapshot
        .into_live_for_program(&RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&program)))
        .expect("restore snapshot");
    restored
        .validate_for_program(&program)
        .expect("restored target continuation validates");

    let return_to = restored.frames[1]
        .return_to
        .as_mut()
        .expect("target return point");
    if let FiberReturnContinuation::ProjectCallTarget { site } = &mut return_to.continuation {
        site.block = AwbcBlockId(99);
    } else {
        panic!("target frame must carry a ProjectCall continuation");
    }
    assert!(matches!(
        restored.validate_for_program(&program),
        Err(super::fiber::FiberStateError::InvalidFrame)
    ));
}

#[test]
fn function_kind_numeric_inventory_is_total_and_closed() {
    let expected = [
        AwbcFunctionKind::Flow,
        AwbcFunctionKind::Ordinary,
        AwbcFunctionKind::PureHelper,
        AwbcFunctionKind::TraitMethod,
        AwbcFunctionKind::Synthetic,
        AwbcFunctionKind::GeneratorProducer,
        AwbcFunctionKind::StreamTransform,
        AwbcFunctionKind::LineActivation,
        AwbcFunctionKind::LineTask,
        AwbcFunctionKind::LineCancellationHandler,
    ];
    assert_eq!(AwbcFunctionKind::ALL, expected);
    for (encoded, kind) in expected.into_iter().enumerate() {
        let encoded = u8::try_from(encoded).expect("bounded function kind inventory");
        assert_eq!(kind.encoded(), encoded);
        assert_eq!(AwbcFunctionKind::from_encoded(encoded), Some(kind));
        assert_eq!(
            serde_json::to_value(kind).expect("function kind serializes numerically"),
            serde_json::json!(encoded)
        );
    }
    for encoded in 10..=u8::MAX {
        assert_eq!(AwbcFunctionKind::from_encoded(encoded), None);
    }
}

#[test]
fn awbc_owned_unit_enum_tags_are_total_and_numeric() {
    macro_rules! assert_owner {
        ($ty:ty) => {{
            for value in <$ty>::ALL.iter().copied() {
                let encoded = value.encoded();
                assert_eq!(<$ty>::from_encoded(encoded), Some(value));
                assert_eq!(
                    serde_json::to_value(value).expect("AWBC enum serializes numerically"),
                    serde_json::json!(encoded)
                );
                assert_eq!(
                    serde_json::from_value::<$ty>(serde_json::json!(encoded))
                        .expect("AWBC enum numeric Serde round trip"),
                    value
                );
            }
            for encoded in u8::MIN..=u8::MAX {
                let expected = <$ty>::ALL
                    .iter()
                    .copied()
                    .find(|value| value.encoded() == encoded);
                assert_eq!(<$ty>::from_encoded(encoded), expected);
            }
        }};
    }

    assert_owner!(AwbcSignedIntKind);
    assert_owner!(AwbcUnsignedIntKind);
    assert_owner!(AwbcFrameSlotRole);
    assert_owner!(AwbcDialogueValueRole);
    assert_owner!(AwbcTraitReceiverMode);
    assert_owner!(AwbcBindMode);
    assert_owner!(AwbcUnaryOp);
    assert_owner!(AwbcBinaryOp);
    assert_owner!(AwbcSafePointKind);
    assert_owner!(AwbcTrapCode);
    assert_owner!(AwbcHostCallMode);
    assert_owner!(AwbcTaskClass);
    assert_owner!(AwbcTaskPolicy);
    assert_owner!(AwbcEffectKind);
    assert_owner!(AwbcResourceAccessMode);
    assert_owner!(AwbcReduceOp);
    assert_owner!(AwbcChildCleanup);
    assert_owner!(AwbcPresentationCleanup);
    assert_owner!(AwbcAudioCleanup);
    assert_owner!(AwbcParallelPolicy);
    assert_owner!(AwbcChildJoinPolicy);
    assert_owner!(AwbcChildCancelPolicy);
    assert_owner!(AwbcPureHelperOrigin);
    assert_owner!(AwbcResourceResidency);
}

#[test]
fn function_kind_and_producer_role_matrix_is_closed() {
    assert_eq!(
        AwbcFunctionFlags::empty().validate_for_kind(AwbcFunctionKind::Synthetic),
        Ok(())
    );
    let need = AwbcFunctionFlags::empty()
        .with(AwbcFunctionFlag::Deterministic)
        .with(AwbcFunctionFlag::MayAllocate)
        .with(AwbcFunctionFlag::NeedProducer);
    assert_eq!(need.validate_for_kind(AwbcFunctionKind::Synthetic), Ok(()));
    assert!(matches!(
        need.validate_for_kind(AwbcFunctionKind::Ordinary),
        Err(AwbcFunctionRoleError::NeedProducerKind {
            actual: AwbcFunctionKind::Ordinary
        })
    ));
    assert_eq!(
        need.with(AwbcFunctionFlag::MaySuspend)
            .validate_for_kind(AwbcFunctionKind::Synthetic),
        Err(AwbcFunctionRoleError::NeedProducerFlags)
    );
    assert_eq!(
        need.with(AwbcFunctionFlag::HasDynamicTarget)
            .validate_for_kind(AwbcFunctionKind::Synthetic),
        Err(AwbcFunctionRoleError::NeedProducerFlags)
    );

    let stream = AwbcFunctionFlags::empty()
        .with(AwbcFunctionFlag::MaySuspend)
        .with(AwbcFunctionFlag::OwnsStreamProducer);
    assert_eq!(
        stream.validate_for_kind(AwbcFunctionKind::GeneratorProducer),
        Ok(())
    );
    assert!(matches!(
        stream.validate_for_kind(AwbcFunctionKind::StreamTransform),
        Err(AwbcFunctionRoleError::StreamProducerKind {
            actual: AwbcFunctionKind::StreamTransform
        })
    ));
    assert_eq!(
        AwbcFunctionFlags::empty().validate_for_kind(AwbcFunctionKind::GeneratorProducer),
        Err(AwbcFunctionRoleError::StreamProducerFlags)
    );
    assert_eq!(
        need.with(AwbcFunctionFlag::OwnsStreamProducer)
            .validate_for_kind(AwbcFunctionKind::Synthetic),
        Err(AwbcFunctionRoleError::ConflictingProducerRoles)
    );
    assert!(AwbcFunctionFlags::try_from_bits(AwbcFunctionFlags::KNOWN_MASK + 1).is_err());

    let mut observed_mask = 0_u32;
    for (index, flag) in AwbcFunctionFlag::ALL.iter().copied().enumerate() {
        let expected = 1_u32 << u32::try_from(index).expect("bounded function flag inventory");
        assert_eq!(flag.mask(), expected);
        assert_eq!(observed_mask & flag.mask(), 0);
        observed_mask |= flag.mask();
    }
    assert_eq!(observed_mask, AwbcFunctionFlags::KNOWN_MASK);

    for bits in 0..=AwbcFunctionFlags::KNOWN_MASK {
        let flags = AwbcFunctionFlags::try_from_bits(bits).expect("known flag subset");
        for kind in AwbcFunctionKind::ALL.iter().copied() {
            let need = flags.contains(AwbcFunctionFlag::NeedProducer);
            let stream = flags.contains(AwbcFunctionFlag::OwnsStreamProducer);
            let expected = if need && stream {
                Err(AwbcFunctionRoleError::ConflictingProducerRoles)
            } else if need && kind != AwbcFunctionKind::Synthetic {
                Err(AwbcFunctionRoleError::NeedProducerKind { actual: kind })
            } else if need
                && (!flags.contains(AwbcFunctionFlag::Deterministic)
                    || !flags.contains(AwbcFunctionFlag::MayAllocate)
                    || flags.contains(AwbcFunctionFlag::MaySuspend)
                    || flags.contains(AwbcFunctionFlag::HasDynamicTarget))
            {
                Err(AwbcFunctionRoleError::NeedProducerFlags)
            } else if kind == AwbcFunctionKind::GeneratorProducer
                && (!stream || !flags.contains(AwbcFunctionFlag::MaySuspend))
            {
                Err(AwbcFunctionRoleError::StreamProducerFlags)
            } else if stream && kind != AwbcFunctionKind::GeneratorProducer {
                Err(AwbcFunctionRoleError::StreamProducerKind { actual: kind })
            } else {
                Ok(())
            };
            assert_eq!(
                flags.validate_for_kind(kind),
                expected,
                "{kind:?} {bits:#x}"
            );
        }
    }
}

#[test]
fn verifier_rejects_function_kind_role_mismatch_at_the_function_owner() {
    let mut program = minimal_program();
    program.functions[0].flags = AwbcFunctionFlags::empty()
        .with(AwbcFunctionFlag::Deterministic)
        .with(AwbcFunctionFlag::MayAllocate)
        .with(AwbcFunctionFlag::NeedProducer);

    assert!(matches!(
        program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::InvalidFunctionRoles {
            function: 0,
            source: AwbcFunctionRoleError::NeedProducerKind {
                actual: AwbcFunctionKind::Flow
            }
        })
    ));
}

#[test]
fn removed_register_handle_effect_row_cannot_reenter_through_dense_tag_zero() {
    let mut program = minimal_program();
    program.constants = vec![AwbcConstant::Unit, AwbcConstant::Unit];
    program.effect_plans = vec![AwbcEffectPlan {
        kind: AwbcEffectKind::Wait,
        signature: AwbcSignatureId(0),
        capability: None,
        audio: None,
        static_args: vec![AwbcConstantId(0), AwbcConstantId(1)],
        resources: Vec::new(),
    }];

    let encoded = program
        .encode_canonical()
        .expect("encode complete former RegisterHandle row shape");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("dense v1 tag zero decodes only as Wait");
    assert_eq!(decoded.effect_plans[0].kind, AwbcEffectKind::Wait);
    assert!(matches!(
        decoded.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::MalformedEffectPayload { effect: 0, .. })
    ));
}

#[test]
fn removed_drop_handle_effect_row_cannot_reenter_through_dense_tag_one() {
    let mut program = minimal_program();
    program.constants = vec![AwbcConstant::Unit];
    program.effect_plans = vec![AwbcEffectPlan {
        kind: AwbcEffectKind::Audio,
        signature: AwbcSignatureId(0),
        capability: None,
        audio: None,
        static_args: vec![AwbcConstantId(0)],
        resources: Vec::new(),
    }];

    let encoded = program
        .encode_canonical()
        .expect("encode complete former DropHandle row shape");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("dense v1 tag one decodes only as Audio");
    assert_eq!(decoded.effect_plans[0].kind, AwbcEffectKind::Audio);
    assert!(matches!(
        decoded.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::MalformedAudioPayload { effect: 0, .. })
    ));
}

#[test]
fn typed_drop_is_an_exact_vm_transaction_boundary() {
    let mut program = minimal_program();
    program.runtime_types = vec![runtime_type(1, AwbcRuntimeTypeShape::Unit)];
    program.constants = vec![AwbcConstant::Unit];
    program.frame_layouts[0] = AwbcFrameLayout {
        scopes: Vec::new(),
        slots: vec![
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Local,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Local,
                scope_depth: 0,
            },
        ],
        max_scope_depth: 0,
    };
    program.instructions = vec![
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::Drop {
            register: AwbcRegisterId(0),
            policy: AwbcDropPolicy::Cancel,
        },
        AwbcInstruction::Drop {
            register: AwbcRegisterId(1),
            policy: AwbcDropPolicy::Finish,
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 4);
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("typed drop program verifies");

    let mut fiber =
        FiberState::for_entry(&program, AwbcEntryId(0), 0, 64).expect("typed drop fiber");
    let first = super::vm::step(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 64,
        },
    )
    .expect("first drop boundary");
    assert_eq!(first.executed, 3);
    assert_eq!(fiber.cursor.instruction_offset, 3);
    assert_eq!(
        first
            .observations
            .iter()
            .filter_map(|observation| match observation {
                super::vm::VmObservation::Drop { policy } => Some(*policy),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![crate::effect::RuntimeDropPolicy::Cancel]
    );

    let second = super::vm::step(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 64,
        },
    )
    .expect("second drop boundary");
    assert_eq!(second.executed, 1);
    assert_eq!(fiber.cursor.instruction_offset, 4);
    assert_eq!(
        second
            .observations
            .iter()
            .filter_map(|observation| match observation {
                super::vm::VmObservation::Drop { policy } => Some(*policy),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![crate::effect::RuntimeDropPolicy::Finish]
    );
}

#[test]
fn execute_line_operation_codec_row_rejects_a_missing_typed_operation() {
    let mut program = minimal_program();
    program.instructions = vec![AwbcInstruction::ExecuteLineOperation {
        dst: AwbcRegisterId(0),
        operation: AwbcLineOperationId(0),
        args: Vec::new(),
    }];
    program.blocks[0].instructions = AwbcTableRange::new(0, 1);

    let encoded = program
        .encode_canonical()
        .expect("encode ExecuteLineOperation row");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode typed ExecuteLineOperation row");
    assert!(matches!(
        decoded.instructions.as_slice(),
        [AwbcInstruction::ExecuteLineOperation { .. }]
    ));
    assert!(matches!(
        decoded.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::IndexOutOfBounds {
            table: "line_operations",
            index: 0,
            ..
        })
    ));
}

#[test]
fn commit_dialogue_result_codec_row_rejects_a_non_activation_owner() {
    let mut program = minimal_program();
    program.instructions = vec![AwbcInstruction::CommitDialogueResult {
        source: AwbcRegisterId(0),
    }];
    program.blocks[0].instructions = AwbcTableRange::new(0, 1);

    let encoded = program
        .encode_canonical()
        .expect("encode CommitDialogueResult row");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode typed CommitDialogueResult row");
    assert!(matches!(
        decoded.instructions.as_slice(),
        [AwbcInstruction::CommitDialogueResult { .. }]
    ));
    assert!(matches!(
        decoded.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::InvalidInvariant { .. })
    ));
}

#[test]
fn typed_drop_stop_codec_row_rejects_a_non_duration_fade() {
    let mut program = minimal_program();
    program.runtime_types = vec![runtime_type(1, AwbcRuntimeTypeShape::Unit)];
    program.constants = vec![AwbcConstant::Unit];
    program.frame_layouts[0] = AwbcFrameLayout {
        scopes: Vec::new(),
        slots: vec![AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(0),
            role: AwbcFrameSlotRole::Local,
            scope_depth: 0,
        }],
        max_scope_depth: 0,
    };
    program.instructions = vec![
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::Drop {
            register: AwbcRegisterId(0),
            policy: AwbcDropPolicy::Stop {
                fade: AwbcRegisterId(0),
            },
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 2);

    let encoded = program.encode_canonical().expect("encode typed Drop row");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode typed Drop row");
    assert!(matches!(
        decoded.instructions.as_slice(),
        [
            _,
            AwbcInstruction::Drop {
                policy: AwbcDropPolicy::Stop { .. },
                ..
            }
        ]
    ));
    assert!(matches!(
        decoded.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::InvalidInvariant { .. })
    ));
}

#[test]
fn pattern_rest_modes_roundtrip_in_the_schema_one_codec() {
    let patterns = vec![
        AwbcPattern::Record {
            ty: None,
            fields: Vec::new(),
            rest: AwbcPatternRest::Exact,
        },
        AwbcPattern::Record {
            ty: None,
            fields: Vec::new(),
            rest: AwbcPatternRest::Ignore,
        },
        AwbcPattern::Record {
            ty: None,
            fields: Vec::new(),
            rest: AwbcPatternRest::Bind(AwbcRegisterId(3)),
        },
        AwbcPattern::Sequence {
            items: Vec::new(),
            rest: AwbcPatternRest::Exact,
        },
        AwbcPattern::Sequence {
            items: Vec::new(),
            rest: AwbcPatternRest::Ignore,
        },
        AwbcPattern::Sequence {
            items: Vec::new(),
            rest: AwbcPatternRest::Bind(AwbcRegisterId(5)),
        },
    ];
    let program = AwbcProgram {
        patterns: patterns.clone(),
        ..AwbcProgram::default()
    };

    let encoded = program.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default()).unwrap();

    assert_eq!(AWBC_CODEC_VERSION, 1);
    assert_eq!(decoded.patterns, patterns);
}

#[test]
fn line_cancel_input_action_roundtrips_in_schema_one_codec() {
    let group = AwbcLineTaskGroup {
        captures: Vec::new(),
        activation: AwbcFunctionId(0),
        result_type: AwbcTypeId(0),
        handle_sites: Vec::new(),
        root: AwbcLineTaskNodeId(0),
        nodes: AwbcTableRange::new(0, 0),
        cancel_handlers: vec![AwbcLineCancelHandler {
            trigger: arcweft_interaction_model::input::InputActionId::new("dialogue.cancel")
                .expect("valid input action"),
            function: AwbcFunctionId(0),
        }],
        cleanup_completed: None,
        cleanup_cancelled: None,
        cleanup_failed: None,
        cleanup: AwbcLineCleanupPolicy {
            child_tasks: AwbcChildCleanup::Finish,
            presentation: AwbcPresentationCleanup::KeepRegistered,
            audio: AwbcAudioCleanup::KeepRegistered,
        },
    };
    let program = AwbcProgram {
        line_task_groups: vec![group.clone()],
        ..AwbcProgram::default()
    };

    let encoded = program.encode_canonical().expect("encode cancel action");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode cancel action");

    assert_eq!(AWBC_CODEC_VERSION, 1);
    assert_eq!(decoded.line_task_groups, [group]);
}

#[test]
fn dynamic_defer_registration_roundtrips_in_the_schema_one_codec() {
    let instruction = AwbcInstruction::RegisterDefer {
        site: crate::runtime_id::RuntimeDeferSiteId::from_zero_based(0)
            .expect("first defer-site identity"),
        outcome: crate::line_task::RuntimeDeferOutcomeFilter::Cancelled,
        owner: super::schema::AwbcDeferOwner::LineRoot,
        captures: vec![AwbcRegisterId(1), AwbcRegisterId(2)],
    };
    let program = AwbcProgram {
        defer_sites: vec![AwbcFunctionId(3)],
        instructions: vec![instruction.clone()],
        ..AwbcProgram::default()
    };

    let encoded = program
        .encode_canonical()
        .expect("encode defer registration");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode defer registration");

    assert_eq!(AWBC_CODEC_VERSION, 1);
    assert_eq!(decoded.defer_sites, [AwbcFunctionId(3)]);
    assert_eq!(decoded.instructions, [instruction]);
}

#[test]
fn optional_agent_record_types_roundtrip_in_the_schema_one_codec() {
    let runtime_types = [
        RuntimeAgentOperationalType::SourcePosition,
        RuntimeAgentOperationalType::ProjectFlowControlSummary,
        RuntimeAgentOperationalType::ProjectGraphSummary,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, operational)| {
        runtime_type(
            u8::try_from(index + 1).expect("bounded Agent type fixture"),
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(operational)),
        )
    })
    .collect::<Vec<_>>();
    let program = AwbcProgram {
        runtime_types: runtime_types.clone(),
        ..AwbcProgram::default()
    };

    let encoded = program
        .encode_canonical()
        .expect("encode Agent record types");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode Agent record types");

    assert_eq!(decoded.runtime_types, runtime_types);
}

#[test]
fn verifier_rejects_duplicate_binding_targets_across_pattern_rest() {
    let mut program = minimal_program();
    program.runtime_types = vec![
        runtime_type(1, AwbcRuntimeTypeShape::Dynamic),
        runtime_type(2, AwbcRuntimeTypeShape::Bool),
    ];
    program.signatures[0].params = vec![AwbcTypeId(0)];
    program.frame_layouts[0] = AwbcFrameLayout {
        scopes: Vec::new(),
        slots: vec![
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Local,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(1),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
        ],
        max_scope_depth: 0,
    };
    program.patterns = vec![
        AwbcPattern::Bind {
            target: AwbcRegisterId(1),
            mutable: false,
            expected: None,
        },
        AwbcPattern::Sequence {
            items: vec![AwbcPatternId(0)],
            rest: AwbcPatternRest::Bind(AwbcRegisterId(1)),
        },
    ];
    program.instructions = vec![AwbcInstruction::TestPattern {
        dst: AwbcRegisterId(2),
        pattern: AwbcPatternId(1),
        value: AwbcRegisterId(0),
    }];
    program.blocks[0].instructions = AwbcTableRange::new(0, 1);

    assert_eq!(
        program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::DuplicatePatternBindingTarget {
            pattern: 1,
            register: 1,
        })
    );
}

#[test]
fn verifier_tracks_dynamic_record_children_before_the_rest_binding() {
    let mut program = minimal_program();
    program.runtime_types = vec![runtime_type(1, AwbcRuntimeTypeShape::Dynamic)];
    program.signatures[0].params = vec![AwbcTypeId(0)];
    program.frame_layouts[0] = AwbcFrameLayout {
        scopes: Vec::new(),
        slots: vec![
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Local,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Local,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
        ],
        max_scope_depth: 0,
    };
    program.patterns = vec![
        AwbcPattern::Bind {
            target: AwbcRegisterId(1),
            mutable: false,
            expected: None,
        },
        AwbcPattern::Record {
            ty: None,
            fields: vec![AwbcRecordPatternField {
                field: 0,
                pattern: AwbcPatternId(0),
            }],
            rest: AwbcPatternRest::Bind(AwbcRegisterId(2)),
        },
    ];
    program.instructions = vec![
        AwbcInstruction::BindPattern {
            pattern: AwbcPatternId(1),
            value: AwbcRegisterId(0),
            mode: AwbcBindMode::Declare,
        },
        AwbcInstruction::Move {
            dst: AwbcRegisterId(3),
            src: AwbcRegisterId(1),
        },
        AwbcInstruction::Move {
            dst: AwbcRegisterId(3),
            src: AwbcRegisterId(2),
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 3);

    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
}

#[test]
fn verifier_rejects_incorrect_agent_field_destination_type() {
    let mut program = minimal_program();
    program.strings.push("enabled".to_owned());
    program.runtime_types = vec![
        runtime_type(
            1,
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(
                RuntimeAgentOperationalType::ActionTarget,
            )),
        ),
        runtime_type(2, AwbcRuntimeTypeShape::String),
    ];
    program.signatures[0].params = vec![AwbcTypeId(0)];
    program.frame_layouts[0] = AwbcFrameLayout {
        scopes: Vec::new(),
        slots: vec![
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(1),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
        ],
        max_scope_depth: 0,
    };
    program.instructions = vec![AwbcInstruction::ProjectField {
        dst: AwbcRegisterId(1),
        target: AwbcRegisterId(0),
        field: AwbcFieldProjection::Named(AwbcStringId(1)),
    }];
    program.blocks[0].instructions = AwbcTableRange::new(0, 1);
    program.canonicalize_string_table();

    let error = program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("Agent enabled field must not project into a string register");
    assert!(
        matches!(&error, AwbcVerifyError::InvalidInvariant { message, .. }
            if message == "Agent field projection destination"),
        "{error:?}"
    );
}

fn optional_string_field_program(owner: AwbcRuntimeTypeShape, label: &str) -> AwbcProgram {
    let mut program = minimal_program();
    program
        .strings
        .extend([label.to_owned(), "Some".to_owned(), "None".to_owned()]);
    program.runtime_types = vec![
        runtime_type(1, owner),
        runtime_type(2, AwbcRuntimeTypeShape::String),
        runtime_type(
            3,
            AwbcRuntimeTypeShape::Variant {
                owner: AwbcVariantIdentity::Builtin(
                    crate::pattern::RuntimeBuiltinVariantIdentity::Option,
                ),
                arguments: Vec::new(),
                cases: vec![
                    AwbcVariantCase {
                        name: AwbcStringId(2),
                        payload: Some(AwbcTypeId(4)),
                    },
                    AwbcVariantCase {
                        name: AwbcStringId(3),
                        payload: None,
                    },
                ],
            },
        ),
        runtime_type(4, AwbcRuntimeTypeShape::Bool),
        runtime_type(5, AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(1)])),
    ];
    program.signatures[0].params = vec![AwbcTypeId(0)];
    program.frame_layouts[0] = AwbcFrameLayout {
        scopes: Vec::new(),
        slots: vec![
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(2),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
        ],
        max_scope_depth: 0,
    };
    program.instructions = vec![AwbcInstruction::ProjectField {
        dst: AwbcRegisterId(1),
        target: AwbcRegisterId(0),
        field: AwbcFieldProjection::Named(AwbcStringId(1)),
    }];
    program.blocks[0].instructions = AwbcTableRange::new(0, 1);
    program.canonicalize_string_table();
    program
}

#[test]
fn verifier_requires_option_destination_for_optional_agent_fields() {
    let mut program = optional_string_field_program(
        AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(
            RuntimeAgentOperationalType::ObservedObject,
        )),
        "parent_id",
    );
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("optional parent identity projects into Option<String>");

    program.frame_layouts[0].slots[1].ty = AwbcTypeId(3);
    let error = program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("optional parent identity must not project into bool");
    assert!(matches!(
        error,
        AwbcVerifyError::InvalidInvariant { ref message, .. }
            if message == "Agent field projection destination"
    ));
}

#[test]
fn progress_label_projects_through_the_registered_option_payload() {
    let program = optional_string_field_program(AwbcRuntimeTypeShape::Progress, "label");
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    for label in [None, Some("working".to_owned())] {
        let progress = crate::value::Progress::new(0.5).unwrap();
        let progress = match &label {
            Some(label) => progress.with_label(label),
            None => progress,
        };
        let value = RuntimeValue::Progress(progress);
        let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).unwrap();
        fiber
            .bind_function_argument_values(&program, &[value])
            .unwrap();
        super::vm::step(
            &program,
            &mut fiber,
            super::vm::VmStepOptions {
                max_instructions: 1,
            },
        )
        .unwrap();
        let expected = label.map_or_else(RuntimeValue::option_none, |label| {
            RuntimeValue::option_some(RuntimeValue::String(label))
        });
        assert_eq!(
            fiber
                .active_frame()
                .unwrap()
                .register(AwbcRegisterId(1))
                .unwrap(),
            &expected
        );
    }
}

#[test]
fn verifier_rejects_agent_operands_that_can_only_fail_at_runtime() {
    let mut viewport = minimal_program();
    viewport.runtime_types = vec![
        runtime_type(1, AwbcRuntimeTypeShape::Int(AwbcSignedIntKind::I64)),
        runtime_type(
            2,
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(
                RuntimeAgentOperationalType::ViewportPoint,
            )),
        ),
    ];
    viewport.signatures[0].params = vec![AwbcTypeId(0), AwbcTypeId(0)];
    viewport.frame_layouts[0] = AwbcFrameLayout {
        scopes: Vec::new(),
        slots: vec![
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(1),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
        ],
        max_scope_depth: 0,
    };
    viewport.instructions = vec![AwbcInstruction::MakeAgent {
        dst: AwbcRegisterId(2),
        constructor: crate::value::RuntimeAgentConstructor::ViewportPoint,
        operands: vec![AwbcRegisterId(0), AwbcRegisterId(1)],
    }];
    viewport.blocks[0].instructions = AwbcTableRange::new(0, 1);
    assert!(matches!(
        viewport
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .expect_err("signed viewport coordinates must reject"),
        AwbcVerifyError::InvalidInvariant { message, .. }
            if message.contains("ViewportPoint rejects operand")
    ));

    let mut all = minimal_program();
    all.runtime_types = vec![
        runtime_type(1, AwbcRuntimeTypeShape::String),
        runtime_type(
            2,
            AwbcRuntimeTypeShape::Sequence {
                kind: crate::plan::RuntimePlanSequenceKind::Vec,
                item: AwbcTypeId(0),
            },
        ),
        runtime_type(
            3,
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(
                RuntimeAgentOperationalType::Predicate,
            )),
        ),
    ];
    all.signatures[0].params = vec![AwbcTypeId(1)];
    all.frame_layouts[0] = AwbcFrameLayout {
        scopes: Vec::new(),
        slots: vec![
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(1),
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(2),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
        ],
        max_scope_depth: 0,
    };
    all.instructions = vec![AwbcInstruction::MakeAgent {
        dst: AwbcRegisterId(1),
        constructor: crate::value::RuntimeAgentConstructor::PredicateAll,
        operands: vec![AwbcRegisterId(0)],
    }];
    all.blocks[0].instructions = AwbcTableRange::new(0, 1);
    assert!(matches!(
        all.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .expect_err("sequence<string> must not construct an Agent predicate list"),
        AwbcVerifyError::InvalidInvariant { message, .. }
            if message.contains("PredicateAll rejects operand")
    ));
}

#[test]
fn vm_pretests_before_writes_and_binds_record_and_sequence_rests_last() {
    let mut program = minimal_program();
    program.runtime_types = vec![runtime_type(1, AwbcRuntimeTypeShape::Dynamic)];
    program.frame_layouts[0] = AwbcFrameLayout {
        scopes: Vec::new(),
        slots: (0..3)
            .map(|_| AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Local,
                scope_depth: 0,
            })
            .collect(),
        max_scope_depth: 0,
    };
    program.constants = vec![AwbcConstant::Bool(true)];
    program.patterns = vec![
        AwbcPattern::Bind {
            target: AwbcRegisterId(0),
            mutable: false,
            expected: None,
        },
        AwbcPattern::Literal(AwbcConstantId(0)),
        AwbcPattern::Tuple(vec![AwbcPatternId(0), AwbcPatternId(1)]),
        AwbcPattern::Sequence {
            items: vec![AwbcPatternId(0)],
            rest: AwbcPatternRest::Bind(AwbcRegisterId(1)),
        },
        AwbcPattern::Record {
            ty: None,
            fields: vec![AwbcRecordPatternField {
                field: 0,
                pattern: AwbcPatternId(0),
            }],
            rest: AwbcPatternRest::Bind(AwbcRegisterId(2)),
        },
        AwbcPattern::Sequence {
            items: Vec::new(),
            rest: AwbcPatternRest::Exact,
        },
        AwbcPattern::Sequence {
            items: Vec::new(),
            rest: AwbcPatternRest::Ignore,
        },
        AwbcPattern::Record {
            ty: None,
            fields: Vec::new(),
            rest: AwbcPatternRest::Exact,
        },
        AwbcPattern::Record {
            ty: None,
            fields: Vec::new(),
            rest: AwbcPatternRest::Ignore,
        },
    ];
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64).unwrap();
    fiber
        .active_frame_mut()
        .unwrap()
        .set_register(AwbcRegisterId(0), RuntimeValue::String("old".to_owned()))
        .unwrap();

    let mismatch = RuntimeValue::Tuple(vec![RuntimeValue::i64(1), RuntimeValue::Bool(false)]);
    super::vm::bind_pattern(&program, &mut fiber, AwbcPatternId(2), &mismatch)
        .expect_err("a later literal mismatch rejects the complete pattern");
    assert_eq!(
        fiber
            .active_frame()
            .unwrap()
            .register(AwbcRegisterId(0))
            .unwrap(),
        &RuntimeValue::String("old".to_owned())
    );

    let sequence = runtime_sequence_values(vec![RuntimeValue::i64(1), RuntimeValue::i64(2)]);
    super::vm::bind_pattern(&program, &mut fiber, AwbcPatternId(3), &sequence).unwrap();
    assert_eq!(
        fiber
            .active_frame()
            .unwrap()
            .register(AwbcRegisterId(1))
            .unwrap(),
        &runtime_sequence_values(vec![RuntimeValue::i64(2)])
    );

    let record = RuntimeValue::try_record(vec![
        ("first".to_owned(), RuntimeValue::i64(3)),
        ("second".to_owned(), RuntimeValue::i64(4)),
    ])
    .unwrap();
    super::vm::bind_pattern(&program, &mut fiber, AwbcPatternId(4), &record).unwrap();
    assert_eq!(
        fiber
            .active_frame()
            .unwrap()
            .register(AwbcRegisterId(2))
            .unwrap(),
        &record
    );

    assert!(!super::vm::test_pattern(&program, AwbcPatternId(5), &sequence).unwrap());
    assert!(super::vm::test_pattern(&program, AwbcPatternId(6), &sequence).unwrap());
    assert!(!super::vm::test_pattern(&program, AwbcPatternId(7), &record).unwrap());
    assert!(super::vm::test_pattern(&program, AwbcPatternId(8), &record).unwrap());
}

#[test]
fn nominal_record_bytes_and_never_types_roundtrip_and_project_exactly() {
    let mut program = AwbcProgram {
        strings: vec![
            "alpha".to_owned(),
            "game.Pair".to_owned(),
            "zeta".to_owned(),
        ],
        runtime_types: vec![
            runtime_type(1, AwbcRuntimeTypeShape::Bool),
            runtime_type(2, AwbcRuntimeTypeShape::Bytes),
            runtime_type(3, AwbcRuntimeTypeShape::Never),
            runtime_type(
                31,
                AwbcRuntimeTypeShape::NominalRecord {
                    public_id: AwbcStringId(1),
                    layout: [32; 32],
                    arguments: Vec::new(),
                    shape: crate::entry::RuntimeNominalRecordShape::Record,
                    fields: vec![
                        AwbcRecordField {
                            field: crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(
                                0,
                            )
                            .unwrap(),
                            name: Some(AwbcStringId(0)),
                            ty: AwbcTypeId(1),
                        },
                        AwbcRecordField {
                            field: crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(
                                1,
                            )
                            .unwrap(),
                            name: Some(AwbcStringId(2)),
                            ty: AwbcTypeId(2),
                        },
                    ],
                },
            ),
        ],
        ..AwbcProgram::default()
    };
    program.canonicalize_string_table();
    let encoded = program.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default()).unwrap();
    let layout = decoded
        .nominal_record_layout(AwbcTypeId(3))
        .unwrap()
        .unwrap();

    assert_eq!(
        layout.shape(),
        crate::entry::RuntimeNominalRecordShape::Record
    );
    for (ordinal, name) in ["alpha", "zeta"].into_iter().enumerate() {
        let field = &layout.fields()[ordinal];
        assert_eq!(field.name(), Some(name));
        assert_eq!(
            field.field(),
            crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap()
        );
    }
    assert_eq!(
        layout.fields()[0].checked_type(),
        &RuntimeCheckedType::Bytes
    );
    assert_eq!(
        layout.fields()[1].checked_type(),
        &RuntimeCheckedType::Never
    );
}

#[test]
fn verifier_rejects_duplicate_nominal_record_descriptor_authority() {
    let descriptor = runtime_type(
        41,
        AwbcRuntimeTypeShape::NominalRecord {
            public_id: AwbcStringId(0),
            layout: [42; 32],
            arguments: Vec::new(),
            shape: crate::entry::RuntimeNominalRecordShape::Record,
            fields: Vec::new(),
        },
    );
    let program = AwbcProgram {
        strings: vec!["game.Empty".to_owned()],
        runtime_types: vec![descriptor.clone(), descriptor],
        ..AwbcProgram::default()
    };

    assert!(matches!(
        program.verify(
            AwbcVerifyBudget::default(),
            AwbcVerifyContext {
                require_entrypoint: false,
                ..AwbcVerifyContext::default()
            }
        ),
        Err(AwbcVerifyError::InvalidInvariant { message, .. })
            if message.contains("semantic type identity is duplicated")
    ));
}

#[test]
fn verifier_rejects_a_callable_state_with_a_stale_body_reference() {
    let mut program = project_call_invoke_program();
    let RuntimeCallableTransition::Invoke { function, .. } =
        &mut program.callable_states[0].transition
    else {
        unreachable!();
    };
    *function = AwbcFunctionId(u32::MAX);
    assert!(matches!(
        program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::IndexOutOfBounds {
            table: "functions",
            index: u32::MAX,
            ..
        })
    ));
}

fn expression_apply_frame_layouts() -> Vec<AwbcFrameLayout> {
    vec![
        AwbcFrameLayout {
            scopes: Vec::new(),
            slots: vec![
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(1),
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(0),
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
            ],
            max_scope_depth: 0,
        },
        AwbcFrameLayout {
            scopes: Vec::new(),
            slots: vec![
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(0),
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(0),
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
            ],
            max_scope_depth: 0,
        },
    ]
}

fn expression_apply_functions(synthetic_len: u32) -> Vec<AwbcFunction> {
    vec![
        AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 1),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        },
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Synthetic,
            signature: AwbcSignatureId(1),
            frame_layout: AwbcFrameLayoutId(1),
            blocks: AwbcTableRange::new(1, synthetic_len),
            entry_block: AwbcBlockId(1),
            flags: AwbcFunctionFlags::empty()
                .with(AwbcFunctionFlag::Deterministic)
                .with(AwbcFunctionFlag::MaySuspend),
        },
    ]
}

fn expression_apply_program(
    synthetic_entry_instructions: Vec<AwbcInstruction>,
    synthetic_blocks: Vec<(AwbcTerminator, AwbcSafePointKind)>,
    resume_points: Vec<AwbcResumePoint>,
) -> AwbcProgram {
    let synthetic_len =
        u32::try_from(synthetic_blocks.len()).expect("test block count fits in AWBC range");
    let synthetic_instruction_len = u32::try_from(synthetic_entry_instructions.len())
        .expect("test instruction count fits in AWBC range");
    let synthetic_code_len = synthetic_instruction_len + 1;
    let mut blocks = Vec::with_capacity(synthetic_blocks.len() + 1);
    blocks.push(AwbcBlock {
        owner: AwbcFunctionId(0),
        instructions: AwbcTableRange::new(0, 2),
        terminator: AwbcTerminator::Return {
            value: Some(AwbcRegisterId(1)),
        },
        safe_point: AwbcSafePointKind::FlowEntry,
        source_map: None,
    });
    blocks.extend(synthetic_blocks.into_iter().enumerate().map(
        |(index, (terminator, safe_point))| AwbcBlock {
            owner: AwbcFunctionId(1),
            instructions: if index == 0 {
                AwbcTableRange::new(2, synthetic_code_len)
            } else {
                AwbcTableRange::new(2 + synthetic_code_len, 0)
            },
            terminator: match terminator {
                AwbcTerminator::Return { value: None } => AwbcTerminator::Return {
                    value: Some(AwbcRegisterId(1)),
                },
                terminator => terminator,
            },
            safe_point,
            source_map: None,
        },
    ));

    AwbcProgram {
        strings: vec!["main".to_owned()],
        constants: vec![AwbcConstant::Unit],
        signatures: vec![
            AwbcSignature {
                params: Vec::new(),
                result: Some(AwbcTypeId(0)),
                effects: AwbcEffectSetId(0),
            },
            AwbcSignature {
                params: Vec::new(),
                result: Some(AwbcTypeId(0)),
                effects: AwbcEffectSetId(0),
            },
        ],
        runtime_types: vec![
            runtime_type(1, AwbcRuntimeTypeShape::Unit),
            runtime_type(
                2,
                AwbcRuntimeTypeShape::Function {
                    contract: RuntimeFunctionTypeContract::default(),
                    parameters: Vec::new(),
                    result: AwbcTypeId(0),
                },
            ),
        ],
        callable_states: vec![RuntimeCallableStateDefinition {
            function_type: AwbcTypeId(1),
            origin: callable_state_id(0),
            position: RuntimeCallablePosition::Unapplied,
            retained: Box::new([]),
            parameters: Box::new([]),
            result: AwbcTypeId(0),
            attached: RuntimeCallableAttachedContract::None,
            transition: RuntimeCallableTransition::Invoke {
                function: AwbcFunctionId(1),
                captures: Box::new([]),
                arguments: Box::new([]),
            },
            partials: Box::new([]),
        }],
        frame_layouts: expression_apply_frame_layouts(),
        functions: expression_apply_functions(synthetic_len),
        blocks,
        instructions: {
            let mut instructions = vec![
                AwbcInstruction::MakeCallable {
                    dst: AwbcRegisterId(0),
                    state: callable_state_id(0),
                    captures: Vec::new(),
                },
                AwbcInstruction::ApplyGroup {
                    dst: AwbcRegisterId(1),
                    callee: AwbcRegisterId(0),
                    args: Vec::new(),
                },
                AwbcInstruction::LoadConst {
                    dst: AwbcRegisterId(1),
                    constant: AwbcConstantId(0),
                },
            ];
            instructions.extend(synthetic_entry_instructions);
            instructions
        },
        resume_points,
        flow_bindings: vec![test_flow_binding("main", 0)],
        flow_executables: vec![test_flow_executable("main", 0)],
        entries: vec![AwbcEntry {
            runtime_id: crate::plan::EntryRuntimeId::canonical("main")
                .expect("test entry runtime ID is valid"),
            binding: crate::entry::EntryBindingIdentity::from_bytes([1; 32]),
            public_id: AwbcStringId(0),
            kind: AwbcEntryKind::Cli,
            target: AwbcEntryTarget::Function {
                function: AwbcFunctionId(0),
            },
            roles: crate::entry::RuntimeEntryRoles::None,
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
    let payload_len = u64::from_le_bytes(first[12..20].try_into().expect("fixed envelope width"));
    assert_eq!(
        payload_len,
        u64::try_from(first.len() - 20).expect("encoded payload length fits u64")
    );
    let decoded = AwbcProgram::decode_canonical(&first, AwbcDecodeBudget::default())
        .expect("decode canonical AWBC");
    assert_eq!(decoded, program);
    assert_eq!(
        decoded
            .encode_canonical()
            .expect("re-encode canonical AWBC"),
        first
    );
    decoded
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify decoded AWBC");
}

#[test]
fn dialogue_content_effect_manifest_and_instruction_codec_round_trip() {
    let mut program = minimal_program();
    let template = crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
        .expect("template identity");
    let site = crate::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(0)
        .expect("effect site identity");
    let delayed_site = crate::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(1)
        .expect("second effect site identity");
    program.content_templates.push(AwbcDialogueContentTemplate {
        id: template,
        digest: crate::entry::RuntimeDialogueContentTemplateDigest::from_bytes([0x71; 32]),
        slots: Vec::new(),
        effects: vec![
            AwbcDialogueContentEffectSlot {
                site,
                trigger: crate::plan::RuntimeDialogueContentEffectTrigger::Content,
                capture_types: Vec::new(),
            },
            AwbcDialogueContentEffectSlot {
                site: delayed_site,
                trigger: crate::plan::RuntimeDialogueContentEffectTrigger::Delay {
                    duration: crate::time::LogicalDuration::from_nanos(17),
                },
                capture_types: Vec::new(),
            },
        ],
    });
    program
        .instructions
        .push(AwbcInstruction::MakeDialogueContent {
            destination: AwbcRegisterId(0),
            template,
            values: Vec::new(),
            effects: vec![
                AwbcDialogueContentEffectBinding {
                    site,
                    state: callable_state_id(0),
                    captures: Vec::new(),
                },
                AwbcDialogueContentEffectBinding {
                    site: delayed_site,
                    state: callable_state_id(0),
                    captures: Vec::new(),
                },
            ],
        });

    let encoded = program
        .encode_canonical()
        .expect("encode dialogue content effect ABI");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode dialogue content effect ABI");
    assert_eq!(decoded, program);
}

#[test]
fn canonical_codec_round_trips_checked_and_controller_flow_identities_and_labels() {
    let checked = FlowRuntimeId::from_checked_declaration_digest([0xa5; 32], "flow.opening")
        .expect("accepted Flow public label");
    let controller = FlowRuntimeId::for_agent_controller_callable(
        &crate::entry::RuntimeCallableId::from_checked_digest([0x5a; 32]),
    );
    assert!(FlowRuntimeId::from_source_entity_body(controller.public_label().as_str()).is_err());
    for flow in [checked, controller] {
        let public_label = flow.public_label();
        assert!(FlowRuntimeId::canonical(&flow.canonical_label()).is_err());
        let mut program = minimal_program();
        program.flow_bindings[0].flow = flow.clone();
        program.flow_executables[0].metadata.flow = flow.clone();

        let encoded = program.encode_canonical().expect("encode runtime Flow ID");
        let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
            .expect("decode runtime Flow ID independently of its public label");

        assert_eq!(decoded.flow_executables[0].metadata.flow, flow);
        assert_eq!(
            decoded.flow_executables[0].metadata.flow.public_label(),
            public_label
        );
        assert_eq!(decoded, program);
    }
}

#[test]
fn canonical_flow_bindings_preserve_same_label_declarations_and_reject_ambiguous_targets() {
    let mut program = minimal_program();
    let first = FlowRuntimeId::from_checked_declaration_digest([0x11; 32], "flow.opening")
        .expect("first checked Flow identity");
    let second = FlowRuntimeId::from_checked_declaration_digest([0x22; 32], "flow.opening")
        .expect("second checked Flow identity");
    program.flow_bindings[0].flow = first.clone();
    program.flow_executables[0].metadata.flow = first.clone();
    let mut second_function = program.functions[0].clone();
    second_function.blocks = AwbcTableRange::new(1, 1);
    second_function.entry_block = AwbcBlockId(1);
    program.functions.push(second_function);
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(1),
        instructions: AwbcTableRange::new(0, 0),
        terminator: AwbcTerminator::Return { value: None },
        safe_point: AwbcSafePointKind::FlowEntry,
        source_map: None,
    });
    program.flow_bindings.push(AwbcFlowBinding {
        flow: second.clone(),
        function: AwbcFunctionId(1),
    });

    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("same public label remains a valid typed Flow inventory");
    let encoded = program
        .encode_canonical()
        .expect("encode typed Flow bindings");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode typed Flow bindings");
    assert_eq!(decoded.flow_function(&first), Some(AwbcFunctionId(0)));
    assert_eq!(decoded.flow_function(&second), Some(AwbcFunctionId(1)));
    assert_eq!(decoded.flow_identity(AwbcFunctionId(0)), Some(&first));
    assert_eq!(decoded.flow_identity(AwbcFunctionId(1)), Some(&second));
    assert!(matches!(
        decoded.resolve_flow_target_value("flow.opening"),
        Err(RuntimeFlowTargetError::Ambiguous { matches: 2, .. })
    ));
    assert!(matches!(
        decoded.resolve_flow_target_value("flow.missing"),
        Err(RuntimeFlowTargetError::Missing { .. })
    ));
    assert!(matches!(
        decoded.resolve_flow_target_value(&first.canonical_label()),
        Err(RuntimeFlowTargetError::Invalid(_))
    ));

    program.flow_bindings.pop();
    program.functions.pop();
    program.blocks.pop();
    assert_eq!(
        program
            .resolve_flow_target_value("flow.opening")
            .map(|(flow, function)| (flow.clone(), function)),
        Ok((first, AwbcFunctionId(0)))
    );
}

#[test]
fn canonical_awbc_assertion_payload_round_trips_as_typed_identity() {
    let mut program = minimal_program();
    let guard =
        RuntimeAssertionGuardId::try_from_bytes([0xa7; 16]).expect("non-zero assertion guard");
    program.strings = vec![
        "always".to_owned(),
        "inventory >= 0".to_owned(),
        "inventory must stay non-negative".to_owned(),
        "main".to_owned(),
    ];
    program.entries[0].public_id = AwbcStringId(3);
    program.constants = vec![
        AwbcConstant::Bytes(guard.as_bytes().to_vec()),
        AwbcConstant::String(AwbcStringId(1)),
        AwbcConstant::String(AwbcStringId(2)),
        AwbcConstant::String(AwbcStringId(0)),
    ];
    program.effect_plans.push(AwbcEffectPlan {
        kind: AwbcEffectKind::Assert,
        signature: AwbcSignatureId(0),
        capability: None,
        audio: None,
        static_args: vec![
            AwbcConstantId(0),
            AwbcConstantId(1),
            AwbcConstantId(2),
            AwbcConstantId(3),
        ],
        resources: Vec::new(),
    });

    let encoded = program.encode_canonical().expect("encode assertion AWBC");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode assertion AWBC");
    decoded
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify decoded assertion AWBC");
    assert_eq!(decoded, program);

    let decoded_guard = match &decoded.constants[0] {
        AwbcConstant::Bytes(bytes) => RuntimeAssertionGuardId::try_from_bytes(
            bytes.as_slice().try_into().expect("fixed 16-byte guard"),
        )
        .expect("decoded guard remains non-zero"),
        other => panic!("assertion guard changed constant kind: {other:?}"),
    };
    assert_eq!(decoded_guard, guard);
    assert_eq!(decoded.effect_plans[0].kind, AwbcEffectKind::Assert);
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
fn canonical_codec_round_trips_choice_and_nominal_runtime_types() {
    let mut program = minimal_program();
    let nominal_name = AwbcStringId(
        u32::try_from(program.strings.len()).expect("test string table fits AWBC index"),
    );
    program.strings.push("state.game.State".to_owned());
    let string_type = AwbcTypeId(
        u32::try_from(program.runtime_types.len()).expect("test type table fits AWBC index"),
    );
    program
        .runtime_types
        .push(runtime_type(16, AwbcRuntimeTypeShape::String));
    let nominal_type = AwbcTypeId(
        u32::try_from(program.runtime_types.len()).expect("test type table fits AWBC index"),
    );
    program.runtime_types.push(runtime_type(
        17,
        AwbcRuntimeTypeShape::Nominal {
            public_id: nominal_name,
            layout: [18; 32],
            arguments: Vec::new(),
        },
    ));
    program.runtime_types.push(runtime_type(
        19,
        AwbcRuntimeTypeShape::Choice(vec![string_type, nominal_type]),
    ));
    let progress_type = AwbcTypeId(
        u32::try_from(program.runtime_types.len()).expect("test type table fits AWBC index"),
    );
    program
        .runtime_types
        .push(runtime_type(20, AwbcRuntimeTypeShape::Progress));

    let encoded = program
        .encode_canonical()
        .expect("encode typed runtime type table");
    assert_eq!(
        u16::from_le_bytes([encoded[8], encoded[9]]),
        AWBC_CODEC_VERSION
    );
    assert_eq!(AWBC_CODEC_VERSION, 1);
    let mut unsupported_version = encoded.clone();
    unsupported_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        AwbcProgram::decode_canonical(&unsupported_version, AwbcDecodeBudget::default())
            .expect_err("codec rejects an unsupported version"),
        AwbcCodecError::UnsupportedCodecVersion {
            actual: 2,
            expected: AWBC_CODEC_VERSION,
        }
    );
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode typed runtime type table");
    decoded
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify typed runtime type table");
    assert_eq!(
        decoded
            .checked_type(progress_type)
            .expect("project Progress type"),
        RuntimeCheckedType::Progress
    );

    assert_eq!(decoded, program);
}

fn opaque_program() -> (AwbcProgram, AwbcTypeId, AwbcTypeId) {
    let mut program = minimal_program();
    program.strings.push("producer.dialogue".to_owned());
    let exact = AwbcTypeId(
        u32::try_from(program.runtime_types.len()).expect("test type table fits AWBC index"),
    );
    program.runtime_types.push(runtime_type(
        41,
        AwbcRuntimeTypeShape::Opaque {
            producer: AwbcStringId(1),
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: RuntimeOpaqueValueClass::Plain,
            persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
            arguments: vec![],
        },
    ));
    let wide = AwbcTypeId(
        u32::try_from(program.runtime_types.len()).expect("test type table fits AWBC index"),
    );
    program.runtime_types.push(runtime_type(
        42,
        AwbcRuntimeTypeShape::Opaque {
            producer: AwbcStringId(1),
            admission: RuntimeOpaqueTypeAdmission::ProducerWide,
            value_class: RuntimeOpaqueValueClass::Plain,
            persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
            arguments: vec![],
        },
    ));
    (program, exact, wide)
}

#[test]
fn opaque_codec_owner_compatibility_and_vm_materialization_share_core_authority() {
    let (mut program, exact, wide) = opaque_program();
    program.strings.push("producer.foreign".to_owned());
    let foreign = AwbcTypeId(
        u32::try_from(program.runtime_types.len()).expect("test type table fits AWBC index"),
    );
    program.runtime_types.push(runtime_type(
        43,
        AwbcRuntimeTypeShape::Opaque {
            producer: AwbcStringId(2),
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: RuntimeOpaqueValueClass::Plain,
            persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
            arguments: vec![],
        },
    ));
    let other_identity = AwbcTypeId(
        u32::try_from(program.runtime_types.len()).expect("test type table fits AWBC index"),
    );
    program.runtime_types.push(runtime_type(
        44,
        AwbcRuntimeTypeShape::Opaque {
            producer: AwbcStringId(1),
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: RuntimeOpaqueValueClass::Plain,
            persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
            arguments: vec![],
        },
    ));
    let payload = AwbcConstantId(
        u32::try_from(program.constants.len()).expect("test constant table fits AWBC index"),
    );
    program
        .constants
        .push(AwbcConstant::String(AwbcStringId(0)));
    let opaque = AwbcConstantId(
        u32::try_from(program.constants.len()).expect("test constant table fits AWBC index"),
    );
    program
        .constants
        .push(AwbcConstant::Opaque { ty: exact, payload });
    program.canonicalize_string_table();

    let encoded = program.encode_canonical().expect("encode opaque AWBC rows");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode opaque AWBC rows");
    decoded
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify opaque AWBC rows");
    assert!(super::verify::types_compatible(&decoded, wide, exact));
    assert!(!super::verify::types_compatible(&decoded, exact, wide));
    assert!(!super::verify::types_compatible(&decoded, wide, foreign));
    assert!(!super::verify::types_compatible(
        &decoded,
        exact,
        other_identity
    ));

    let value = super::vm::constant_value(&decoded, opaque).expect("materialize opaque constant");
    assert!(super::fiber::runtime_value_matches_type(
        &decoded, &value, exact, 0
    ));
    assert!(super::fiber::runtime_value_matches_type(
        &decoded, &value, wide, 0
    ));
    let RuntimeValue::Opaque(value) = value else {
        panic!("opaque constant materializes an opaque runtime value");
    };
    assert_eq!(value.payload(), &RuntimeValue::String("main".to_owned()));
}

#[test]
fn reduction_unchanged_instruction_roundtrips_verifies_and_constructs_typed_value() {
    let mut program = minimal_program();
    program.strings.push("std.reduction".to_owned());
    program.runtime_types = vec![
        AwbcRuntimeType::unit(),
        runtime_type(
            93,
            AwbcRuntimeTypeShape::Opaque {
                producer: AwbcStringId(1),
                admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                value_class: RuntimeOpaqueValueClass::Plain,
                persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
                arguments: vec![AwbcTypeId(0)],
            },
        ),
    ];
    program.signatures[0].result = Some(AwbcTypeId(1));
    program.frame_layouts[0] = AwbcFrameLayout {
        scopes: Vec::new(),
        slots: vec![
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(1),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
        ],
        max_scope_depth: 0,
    };
    program.constants.push(AwbcConstant::Unit);
    program.instructions = vec![
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::MakeReductionUnchanged {
            dst: AwbcRegisterId(1),
            ty: AwbcTypeId(1),
            state: AwbcRegisterId(0),
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 2);
    program.blocks[0].terminator = AwbcTerminator::Return {
        value: Some(AwbcRegisterId(1)),
    };

    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("typed Reduction.unchanged program verifies");
    let encoded = program
        .encode_canonical()
        .expect("typed Reduction.unchanged program encodes");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("typed Reduction.unchanged program decodes");
    let mut fiber =
        FiberState::for_entry(&decoded, AwbcEntryId(0), 0, 64).expect("typed fiber initializes");
    let output = super::vm::step(
        &decoded,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 8,
        },
    )
    .expect("typed Reduction.unchanged program executes");
    let super::vm::VmExit::Returned(Some(RuntimeValue::Reduction(value))) = output.exit else {
        panic!("typed Reduction.unchanged must return a reduction value");
    };
    assert_eq!(value.state(), &RuntimeValue::Unit);
    assert_eq!(value.commands(), []);
}

#[test]
fn verifier_rejects_wide_or_cyclic_opaque_constants_and_invalid_producers() {
    let (mut program, _exact, wide) = opaque_program();
    program.constants.push(AwbcConstant::Unit);
    program.constants.push(AwbcConstant::Opaque {
        ty: wide,
        payload: AwbcConstantId(0),
    });
    program.canonicalize_string_table();
    let error = program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("wide opaque constant must reject");
    assert!(
        matches!(&error, AwbcVerifyError::InvalidInvariant { message, .. }
            if message == "opaque constant requires an exact constant-admissible opaque type row"),
        "{error:?}"
    );

    let (mut cyclic, exact, _wide) = opaque_program();
    cyclic.constants.push(AwbcConstant::Opaque {
        ty: exact,
        payload: AwbcConstantId(0),
    });
    cyclic.canonicalize_string_table();
    let error = cyclic
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("cyclic opaque constant must reject");
    assert!(
        matches!(&error, AwbcVerifyError::InvalidInvariant { message, .. }
            if message == "opaque constant payload must precede its owner row"),
        "{error:?}"
    );

    let mut invalid = minimal_program();
    invalid.strings.push(String::new());
    invalid.runtime_types.push(runtime_type(
        43,
        AwbcRuntimeTypeShape::Opaque {
            producer: AwbcStringId(1),
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: RuntimeOpaqueValueClass::Plain,
            persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
            arguments: vec![],
        },
    ));
    invalid.canonicalize_string_table();
    let error = invalid
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("invalid opaque producer must reject");
    assert!(
        matches!(&error, AwbcVerifyError::InvalidInvariant { message, .. }
            if message.contains("identity cannot be empty")),
        "{error:?}"
    );
}

#[test]
fn snapshot_only_affine_opaque_type_roundtrips_but_rejects_constant_materialization() {
    let mut program = minimal_program();
    program.strings.push("std.line.cue_handle".to_owned());
    let handle_ty = AwbcTypeId(
        u32::try_from(program.runtime_types.len()).expect("test type table fits AWBC index"),
    );
    program.runtime_types.push(runtime_type(
        55,
        AwbcRuntimeTypeShape::Opaque {
            producer: AwbcStringId(1),
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue),
            persistence: RuntimeOpaquePersistence::SnapshotOnly,
            arguments: vec![],
        },
    ));
    program.constants.push(AwbcConstant::Unit);
    program.constants.push(AwbcConstant::Opaque {
        ty: handle_ty,
        payload: AwbcConstantId(0),
    });
    program.canonicalize_string_table();

    let encoded = program
        .encode_canonical()
        .expect("snapshot-only handle type encodes");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("snapshot-only handle type decodes");
    let owner = decoded
        .opaque_owner(handle_ty)
        .expect("opaque owner projects")
        .expect("handle row is opaque");
    assert_eq!(
        owner.value_class(),
        RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue)
    );
    assert_eq!(owner.persistence(), RuntimeOpaquePersistence::SnapshotOnly);
    let error = decoded
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("snapshot-only opaque constant must reject");
    assert!(
        matches!(&error, AwbcVerifyError::InvalidInvariant { message, .. }
            if message == "opaque constant requires an exact constant-admissible opaque type row"),
        "{error:?}"
    );
}

#[test]
fn verifier_rejects_non_opaque_and_missing_opaque_constant_references() {
    let mut non_opaque = minimal_program();
    non_opaque.constants.push(AwbcConstant::Unit);
    non_opaque.constants.push(AwbcConstant::Opaque {
        ty: AwbcTypeId(0),
        payload: AwbcConstantId(0),
    });
    let error = non_opaque
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("non-opaque type reference must reject");
    assert!(
        matches!(&error, AwbcVerifyError::InvalidInvariant { message, .. }
            if message == "opaque constant requires an exact constant-admissible opaque type row"),
        "{error:?}"
    );

    let (mut missing, exact, _wide) = opaque_program();
    missing.constants.push(AwbcConstant::Opaque {
        ty: exact,
        payload: AwbcConstantId(99),
    });
    missing.canonicalize_string_table();
    assert!(matches!(
        missing
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .expect_err("missing opaque payload must reject"),
        AwbcVerifyError::IndexOutOfBounds {
            table: "constants",
            index: 99,
            ..
        }
    ));
}

#[test]
fn fiber_snapshot_serde_preserves_opaque_owner_and_rejects_tampering() {
    let (mut program, exact, _wide) = opaque_program();
    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: exact,
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    program.canonicalize_string_table();
    let owner = program
        .opaque_owner(exact)
        .expect("opaque owner projection succeeds")
        .expect("type row is opaque");
    let value = owner
        .try_wrap(RuntimeValue::String("saved".to_owned()))
        .expect("exact owner wraps saved value");
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64)
        .expect("opaque snapshot fiber initializes");
    fiber
        .active_frame_mut()
        .expect("active frame")
        .set_register(AwbcRegisterId(0), value)
        .expect("write opaque register");
    let encoded = serde_json::to_vec(&fiber).expect("fiber snapshot serializes");
    let mut restored: FiberState =
        serde_json::from_slice(&encoded).expect("fiber snapshot deserializes");
    restored
        .validate_for_program(&program)
        .expect("restored opaque register validates");

    let foreign = RuntimeOpaqueTypeOwner::exact(
        RuntimeOpaqueTypeProducerId::try_new("producer.foreign").expect("valid producer"),
        RuntimeSemanticTypeId::from_bytes([41; 32]),
    )
    .try_wrap(RuntimeValue::String("saved".to_owned()))
    .expect("foreign exact owner wraps");
    restored
        .active_frame_mut()
        .expect("active frame")
        .set_register(AwbcRegisterId(0), foreign)
        .expect("tamper opaque register");
    assert!(matches!(
        restored
            .validate_for_program(&program)
            .expect_err("foreign opaque owner must reject on restore validation"),
        super::fiber::FiberStateError::InvalidRuntimeValue { .. }
    ));

    let mut class_tampered: FiberState =
        serde_json::from_slice(&encoded).expect("fiber snapshot deserializes again");
    let affine = RuntimeOpaqueTypeOwner::exact_with(
        RuntimeOpaqueTypeProducerId::try_new("producer.dialogue").expect("valid producer"),
        RuntimeSemanticTypeId::from_bytes([41; 32]),
        RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue),
        RuntimeOpaquePersistence::SnapshotOnly,
    )
    .try_wrap(RuntimeValue::String("saved".to_owned()))
    .expect("tampered exact owner wraps");
    class_tampered
        .active_frame_mut()
        .expect("active frame")
        .set_register(AwbcRegisterId(0), affine)
        .expect("tamper opaque class");
    assert!(matches!(
        class_tampered
            .validate_for_program(&program)
            .expect_err("opaque class/persistence tamper must reject on restore validation"),
        super::fiber::FiberStateError::InvalidRuntimeValue { .. }
    ));
}

#[test]
fn verifier_rejects_non_canonical_builtin_variant_schema() {
    let mut program = minimal_program();
    program
        .strings
        .extend(["None".to_owned(), "Some".to_owned()]);
    program.runtime_types.push(runtime_type(
        56,
        AwbcRuntimeTypeShape::Variant {
            owner: AwbcVariantIdentity::Builtin(
                crate::pattern::RuntimeBuiltinVariantIdentity::Option,
            ),
            arguments: Vec::new(),
            cases: vec![
                AwbcVariantCase {
                    name: AwbcStringId(1),
                    payload: None,
                },
                AwbcVariantCase {
                    name: AwbcStringId(2),
                    payload: Some(AwbcTypeId(0)),
                },
            ],
        },
    ));
    program.canonicalize_string_table();

    let error = program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("non-canonical Option schema must reject");
    assert!(
        matches!(
            &error,
            AwbcVerifyError::InvalidInvariant { message, .. }
                if message.as_str() == super::type_projection::AwbcTypeProjectionError::InvalidBuiltinVariant { index: 2 }.to_string().as_str()
        ),
        "{error:?}"
    );
}

#[test]
fn verifier_rejects_variant_constant_with_obsolete_nominal_type() {
    let mut program = minimal_program();
    program
        .strings
        .extend(["state.Widget".to_owned(), "Ready".to_owned()]);
    program.runtime_types.push(runtime_type(
        23,
        AwbcRuntimeTypeShape::Nominal {
            public_id: AwbcStringId(1),
            layout: [24; 32],
            arguments: Vec::new(),
        },
    ));
    program.constants.push(AwbcConstant::Variant {
        ty: AwbcTypeId(0),
        case: 0,
        payload: None,
    });
    program.canonicalize_string_table();

    let error = program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("variant constant cannot reference a nominal type");
    assert!(
        matches!(
            &error,
            AwbcVerifyError::InvalidInvariant { message, .. }
                if message == "variant constant references a non-variant type"
        ),
        "{error:?}"
    );
}

#[test]
fn fiber_checkpoint_and_serde_preserve_cleanup_stacks() {
    let mut program = minimal_program();
    program.frame_layouts[0].scopes = vec![AwbcScopeDefinition {
        parent: None,
        identity: crate::scope::RuntimeScopeIdentity::Anonymous,
    }];
    program.frame_layouts[0].max_scope_depth = 1;
    program.instructions = vec![
        AwbcInstruction::EnterScope {
            scope: AwbcScopeId(0),
        },
        AwbcInstruction::ExitScope {
            scope: AwbcScopeId(0),
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 2);
    let mut fiber =
        FiberState::for_entry(&program, AwbcEntryId(0), 7, 64).expect("fiber initializes");
    super::vm::step(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 1,
        },
    )
    .expect("declared lexical scope enters");
    fiber
        .active_frame_mut()
        .expect("active frame")
        .root_cleanups
        .push(FiberScopeCleanup {
            key: "handle.root".to_owned(),
            effect: AwbcEffectPlanId(0),
            args: vec![RuntimeValue::String("root".to_owned())],
        });
    fiber.active_frame_mut().expect("active frame").scopes[0]
        .cleanups
        .push(FiberScopeCleanup {
            key: "handle.scope".to_owned(),
            effect: AwbcEffectPlanId(0),
            args: vec![RuntimeValue::String("scope".to_owned())],
        });

    let checkpoint = fiber.checkpoint();
    let encoded_checkpoint =
        serde_json::to_string(&checkpoint).expect("fiber checkpoint serializes");
    let decoded_checkpoint = serde_json::from_str(&encoded_checkpoint)
        .expect("fiber checkpoint deserializes without session identity");
    let encoded = serde_json::to_string(&fiber).expect("fiber state serializes");
    let decoded: FiberState = serde_json::from_str(&encoded).expect("fiber state deserializes");
    assert_eq!(decoded, fiber);

    fiber
        .active_frame_mut()
        .expect("active frame")
        .root_cleanups
        .clear();
    fiber
        .active_frame_mut()
        .expect("active frame")
        .scopes
        .clear();
    fiber.restore(decoded_checkpoint);

    let frame = fiber.active_frame().expect("active frame restored");
    assert_eq!(frame.root_cleanups[0].key, "handle.root");
    assert_eq!(frame.scopes[0].cleanups[0].key, "handle.scope");
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
fn verifier_rejects_effect_static_argument_shape_that_product_mapping_cannot_read() {
    let mut program = minimal_program();
    program.effect_plans.push(AwbcEffectPlan {
        kind: AwbcEffectKind::Log,
        signature: AwbcSignatureId(0),
        capability: None,
        audio: None,
        static_args: Vec::new(),
        resources: Vec::new(),
    });

    let error = program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("zero assert guard must be rejected");
    assert!(
        matches!(
            error,
            AwbcVerifyError::MalformedEffectPayload { effect: 0, .. }
        ),
        "{error:?}"
    );
}

#[test]
fn verifier_rejects_evaluated_effect_signature_with_wrong_arity() {
    let mut program = minimal_program();
    program
        .constants
        .push(AwbcConstant::String(AwbcStringId(0)));
    program.signatures.push(AwbcSignature {
        params: vec![AwbcTypeId(0)],
        result: None,
        effects: AwbcEffectSetId(0),
    });
    program.effect_plans.push(AwbcEffectPlan {
        kind: AwbcEffectKind::SignalWrite,
        signature: AwbcSignatureId(1),
        capability: None,
        audio: None,
        static_args: vec![AwbcConstantId(0), AwbcConstantId(0)],
        resources: Vec::new(),
    });

    assert!(matches!(
        program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::MalformedEffectPayload { effect: 0, .. })
    ));
}

#[test]
fn verifier_rejects_unknown_assert_profile_instead_of_defaulting_it() {
    let mut program = minimal_program();
    program.strings.extend([
        "profile.condition".to_owned(),
        "profile.message".to_owned(),
        "sometimes".to_owned(),
    ]);
    program.constants = vec![
        AwbcConstant::Bytes(vec![7; 16]),
        AwbcConstant::String(AwbcStringId(1)),
        AwbcConstant::String(AwbcStringId(2)),
        AwbcConstant::String(AwbcStringId(3)),
    ];
    program.effect_plans.push(AwbcEffectPlan {
        kind: AwbcEffectKind::Assert,
        signature: AwbcSignatureId(0),
        capability: None,
        audio: None,
        static_args: vec![
            AwbcConstantId(0),
            AwbcConstantId(1),
            AwbcConstantId(2),
            AwbcConstantId(3),
        ],
        resources: Vec::new(),
    });

    let error = program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("unknown assert profile must be rejected");
    assert!(
        matches!(
            error,
            AwbcVerifyError::MalformedEffectPayload { effect: 0, .. }
        ),
        "{error:?}"
    );
}

#[test]
fn verifier_rejects_zero_assert_guard() {
    let mut program = minimal_program();
    program.strings.extend([
        "profile.condition".to_owned(),
        "profile.message".to_owned(),
        "sometimes".to_owned(),
    ]);
    program.constants = vec![
        AwbcConstant::Bytes(vec![0; 16]),
        AwbcConstant::String(AwbcStringId(1)),
        AwbcConstant::String(AwbcStringId(2)),
        AwbcConstant::String(AwbcStringId(3)),
    ];
    program.effect_plans.push(AwbcEffectPlan {
        kind: AwbcEffectKind::Assert,
        signature: AwbcSignatureId(0),
        capability: None,
        audio: None,
        static_args: vec![
            AwbcConstantId(0),
            AwbcConstantId(1),
            AwbcConstantId(2),
            AwbcConstantId(3),
        ],
        resources: Vec::new(),
    });

    let error = program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect_err("zero assert guard must be rejected");
    assert!(
        matches!(
            error,
            AwbcVerifyError::MalformedEffectPayload { effect: 0, .. }
        ),
        "{error:?}"
    );
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
    program
        .runtime_types
        .push(runtime_type(66, AwbcRuntimeTypeShape::Bool));
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
#[allow(
    clippy::too_many_lines,
    reason = "The AWBC callable fixture is intentionally inline so capture, verifier, codec, and VM expectations stay in one place."
)]
fn callable_instructions_capture_and_apply_program_owned_state() {
    let program = AwbcProgram {
        strings: vec!["captured".to_owned(), "main".to_owned(), "x".to_owned()],
        constants: vec![AwbcConstant::String(AwbcStringId(0))],
        signatures: vec![
            AwbcSignature {
                params: Vec::new(),
                result: Some(AwbcTypeId(0)),
                effects: AwbcEffectSetId(0),
            },
            AwbcSignature {
                params: vec![AwbcTypeId(0)],
                result: Some(AwbcTypeId(0)),
                effects: AwbcEffectSetId(0),
            },
        ],
        frame_layouts: vec![
            AwbcFrameLayout {
                scopes: Vec::new(),
                slots: vec![
                    AwbcFrameSlot {
                        name: Some(AwbcStringId(2)),
                        ty: AwbcTypeId(0),
                        role: AwbcFrameSlotRole::Local,
                        scope_depth: 0,
                    },
                    AwbcFrameSlot {
                        name: None,
                        ty: AwbcTypeId(1),
                        role: AwbcFrameSlotRole::Temporary,
                        scope_depth: 0,
                    },
                    AwbcFrameSlot {
                        name: None,
                        ty: AwbcTypeId(0),
                        role: AwbcFrameSlotRole::Temporary,
                        scope_depth: 0,
                    },
                ],
                max_scope_depth: 0,
            },
            AwbcFrameLayout {
                scopes: Vec::new(),
                slots: vec![AwbcFrameSlot {
                    name: Some(AwbcStringId(2)),
                    ty: AwbcTypeId(0),
                    role: AwbcFrameSlotRole::Parameter,
                    scope_depth: 0,
                }],
                max_scope_depth: 0,
            },
        ],
        runtime_types: vec![
            runtime_type(1, AwbcRuntimeTypeShape::String),
            runtime_type(
                2,
                AwbcRuntimeTypeShape::Function {
                    contract: RuntimeFunctionTypeContract::default(),
                    parameters: Vec::new(),
                    result: AwbcTypeId(0),
                },
            ),
        ],
        callable_states: vec![RuntimeCallableStateDefinition {
            function_type: AwbcTypeId(1),
            origin: callable_state_id(0),
            position: RuntimeCallablePosition::Unapplied,
            retained: vec![RuntimeCallableRetainedInput {
                role: RuntimeCallableRetainedRole::Capture { position: 0 },
                ty: AwbcTypeId(0),
            }]
            .into_boxed_slice(),
            parameters: Box::new([]),
            result: AwbcTypeId(0),
            attached: RuntimeCallableAttachedContract::None,
            transition: RuntimeCallableTransition::Invoke {
                function: AwbcFunctionId(1),
                captures: vec![RuntimeCallableInputSource::Retained { position: 0 }]
                    .into_boxed_slice(),
                arguments: Box::new([]),
            },
            partials: Box::new([]),
        }],
        functions: vec![
            AwbcFunction {
                public_id: Some(AwbcStringId(1)),
                kind: AwbcFunctionKind::Flow,
                signature: AwbcSignatureId(0),
                frame_layout: AwbcFrameLayoutId(0),
                blocks: AwbcTableRange::new(0, 1),
                entry_block: AwbcBlockId(0),
                flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
            },
            AwbcFunction {
                public_id: None,
                kind: AwbcFunctionKind::Synthetic,
                signature: AwbcSignatureId(1),
                frame_layout: AwbcFrameLayoutId(1),
                blocks: AwbcTableRange::new(1, 1),
                entry_block: AwbcBlockId(1),
                flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
            },
        ],
        blocks: vec![
            AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::new(0, 3),
                terminator: AwbcTerminator::Return {
                    value: Some(AwbcRegisterId(2)),
                },
                safe_point: AwbcSafePointKind::FlowEntry,
                source_map: None,
            },
            AwbcBlock {
                owner: AwbcFunctionId(1),
                instructions: AwbcTableRange::new(3, 0),
                terminator: AwbcTerminator::Return {
                    value: Some(AwbcRegisterId(0)),
                },
                safe_point: AwbcSafePointKind::CallableBoundary,
                source_map: None,
            },
        ],
        instructions: vec![
            AwbcInstruction::LoadConst {
                dst: AwbcRegisterId(0),
                constant: AwbcConstantId(0),
            },
            AwbcInstruction::MakeCallable {
                dst: AwbcRegisterId(1),
                state: callable_state_id(0),
                captures: vec![AwbcRegisterId(0)],
            },
            AwbcInstruction::ApplyGroup {
                dst: AwbcRegisterId(2),
                callee: AwbcRegisterId(1),
                args: Vec::new(),
            },
        ],
        flow_bindings: vec![test_flow_binding("main", 0)],
        flow_executables: vec![test_flow_executable("main", 0)],
        entries: vec![AwbcEntry {
            runtime_id: crate::plan::EntryRuntimeId::canonical("main")
                .expect("test entry runtime ID is valid"),
            binding: crate::entry::EntryBindingIdentity::from_bytes([1; 32]),
            public_id: AwbcStringId(1),
            kind: AwbcEntryKind::Cli,
            target: AwbcEntryTarget::Function {
                function: AwbcFunctionId(0),
            },
            roles: crate::entry::RuntimeEntryRoles::None,
        }],
        ..AwbcProgram::default()
    };
    let program = std::sync::Arc::new(program);
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("callable program verifies");

    let encoded = program.encode_canonical().expect("encode callable program");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode callable program");
    assert_eq!(decoded, program.as_ref().clone());

    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64).expect("create fiber");
    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("step callable program");
    assert_eq!(
        output.exit,
        super::vm::VmExit::Returned(Some(RuntimeValue::String("captured".to_owned())))
    );
}

#[test]
fn expression_apply_preserves_dynamic_call_frame_across_suspension_and_resume() {
    let program = std::sync::Arc::new(expression_apply_program(
        Vec::new(),
        vec![
            (
                AwbcTerminator::BudgetYield {
                    resume: AwbcResumePointId(0),
                },
                AwbcSafePointKind::CallableBoundary,
            ),
            (
                AwbcTerminator::Return { value: None },
                AwbcSafePointKind::None,
            ),
        ],
        vec![AwbcResumePoint {
            function: AwbcFunctionId(1),
            block: AwbcBlockId(2),
            frame_layout: AwbcFrameLayoutId(1),
            kind: AwbcSafePointKind::BudgetYield,
        }],
    ));
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify expression apply suspension program");

    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64)
        .expect("create expression apply fiber");
    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("suspending expression apply exits normally");

    assert_eq!(
        output.exit,
        super::vm::VmExit::Suspended(FiberSuspensionReason::BudgetYield)
    );
    assert_eq!(fiber.frames.len(), 2);
    assert_eq!(fiber.cursor.function, AwbcFunctionId(1));
    assert_eq!(
        fiber
            .suspension
            .as_ref()
            .and_then(FiberSuspension::declared_resume),
        Some(AwbcResumePointId(0))
    );
    assert_eq!(
        fiber.frames[1]
            .return_to
            .clone()
            .expect("dynamic call continuation")
            .cursor,
        super::fiber::FiberCursor {
            function: AwbcFunctionId(0),
            block: AwbcBlockId(0),
            instruction_offset: 2,
        }
    );
    fiber
        .validate_for_program(&program)
        .expect("suspended dynamic call snapshot validates");
    let mut fiber = fiber.clone();
    fiber
        .validate_for_program(&program)
        .expect("restored dynamic call snapshot validates");

    fiber
        .resume_at(&program, AwbcResumePointId(0))
        .expect("resume dynamic callee");
    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("finish resumed dynamic call");
    assert_eq!(
        output.exit,
        super::vm::VmExit::Returned(Some(RuntimeValue::Unit))
    );
    assert_eq!(fiber.frames.len(), 1);
}

#[test]
fn expression_apply_surfaces_await_from_the_dynamic_callee() {
    let mut await_program = expression_apply_program(
        vec![AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(1),
        }],
        vec![
            (
                AwbcTerminator::Await {
                    handle: AwbcRegisterId(0),
                    binding: None,
                    observer: None,
                    resume: AwbcResumePointId(0),
                },
                AwbcSafePointKind::CallableBoundary,
            ),
            (
                AwbcTerminator::Return { value: None },
                AwbcSafePointKind::None,
            ),
        ],
        vec![AwbcResumePoint {
            function: AwbcFunctionId(1),
            block: AwbcBlockId(2),
            frame_layout: AwbcFrameLayoutId(1),
            kind: AwbcSafePointKind::Await,
        }],
    );
    await_program.strings.push("task.dynamic".to_owned());
    await_program
        .constants
        .push(AwbcConstant::String(AwbcStringId(1)));
    await_program
        .runtime_types
        .push(runtime_type(67, AwbcRuntimeTypeShape::Task(AwbcTypeId(1))));
    await_program.frame_layouts[1].slots[0].ty = AwbcTypeId(2);
    let await_program = std::sync::Arc::new(await_program);

    let mut fiber = FiberState::for_entry(&await_program, AwbcEntryId(0), 0, 64)
        .expect("create await expression apply fiber");
    let output = step_with_callable_context(
        &await_program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("dynamic callee reaches await");
    assert!(matches!(
        output.exit,
        super::vm::VmExit::Suspended(FiberSuspensionReason::Await {
            target: FiberAwaitTarget::Task(RuntimeValue::String(ref task)),
            binding: None,
            observer: None,
        }) if task == "task.dynamic"
    ));
    assert_eq!(
        fiber
            .suspension
            .as_ref()
            .and_then(FiberSuspension::declared_resume),
        Some(AwbcResumePointId(0))
    );
    fiber
        .validate_for_program(&await_program)
        .expect("awaiting dynamic callee snapshot validates");
    fiber
        .resume_at(&await_program, AwbcResumePointId(0))
        .expect("resume await dynamic callee");
    assert_eq!(
        step_with_callable_context(
            &await_program,
            &mut fiber,
            super::vm::VmStepOptions {
                max_instructions: 16,
            },
        )
        .expect("finish await dynamic callee")
        .exit,
        super::vm::VmExit::Returned(Some(RuntimeValue::Unit))
    );
}

#[test]
fn expression_apply_surfaces_host_call_from_the_dynamic_callee() {
    let mut host_program = expression_apply_program(
        Vec::new(),
        vec![
            (
                AwbcTerminator::HostCall {
                    call: AwbcHostCallId(0),
                    args: Vec::new(),
                    dst: None,
                    resume: AwbcResumePointId(0),
                },
                AwbcSafePointKind::CallableBoundary,
            ),
            (
                AwbcTerminator::Return { value: None },
                AwbcSafePointKind::None,
            ),
        ],
        vec![AwbcResumePoint {
            function: AwbcFunctionId(1),
            block: AwbcBlockId(2),
            frame_layout: AwbcFrameLayoutId(1),
            kind: AwbcSafePointKind::HostCall,
        }],
    );
    host_program.signatures.push(AwbcSignature {
        params: Vec::new(),
        result: None,
        effects: AwbcEffectSetId(0),
    });
    host_program.host_calls.push(AwbcHostCall {
        public_id: AwbcStringId(0),
        capability: AwbcStringId(0),
        operation: AwbcStringId(0),
        contract: None,
        signature: AwbcSignatureId(2),
        mode: AwbcHostCallMode::Suspend,
        deterministic: true,
        arguments: Vec::new(),
    });
    let host_program = std::sync::Arc::new(host_program);

    let mut fiber = FiberState::for_entry(&host_program, AwbcEntryId(0), 0, 64)
        .expect("create host-call expression apply fiber");
    let output = step_with_callable_context(
        &host_program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("dynamic callee reaches host call");
    assert_eq!(
        output.exit,
        super::vm::VmExit::Suspended(FiberSuspensionReason::HostCall {
            call: AwbcHostCallId(0),
            args: Vec::new(),
            destination: None,
        })
    );
    assert_eq!(
        fiber
            .suspension
            .as_ref()
            .and_then(FiberSuspension::declared_resume),
        Some(AwbcResumePointId(0))
    );
    fiber
        .validate_for_program(&host_program)
        .expect("host-call dynamic callee snapshot validates");
    fiber
        .resume_at(&host_program, AwbcResumePointId(0))
        .expect("resume host-call dynamic callee");
    assert_eq!(
        step_with_callable_context(
            &host_program,
            &mut fiber,
            super::vm::VmStepOptions {
                max_instructions: 16,
            },
        )
        .expect("finish host-call dynamic callee")
        .exit,
        super::vm::VmExit::Returned(Some(RuntimeValue::Unit))
    );
}

#[test]
fn expression_apply_uses_the_callers_budget_without_a_hidden_inner_limit() {
    let synthetic_instructions = vec![
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(0),
        };
        4_097
    ];
    let program = std::sync::Arc::new(expression_apply_program(
        synthetic_instructions,
        vec![(
            AwbcTerminator::Return { value: None },
            AwbcSafePointKind::CallableBoundary,
        )],
        Vec::new(),
    ));
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify expression apply budget program");

    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 8_192)
        .expect("create expression apply fiber");
    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 8_192,
        },
    )
    .expect("long dynamic call uses caller budget");

    assert_eq!(
        output.exit,
        super::vm::VmExit::Returned(Some(RuntimeValue::Unit))
    );
    assert!(output.executed > 4_096);
}

#[test]
fn expression_apply_keeps_partial_application_as_a_value_operation() {
    let mut program = expression_apply_program(
        Vec::new(),
        vec![(
            AwbcTerminator::Return { value: None },
            AwbcSafePointKind::CallableBoundary,
        )],
        Vec::new(),
    );
    program.runtime_types.extend([
        runtime_type(
            3,
            AwbcRuntimeTypeShape::Function {
                contract: RuntimeFunctionTypeContract::default(),
                parameters: vec![AwbcTypeId(0), AwbcTypeId(0)],
                result: AwbcTypeId(0),
            },
        ),
        runtime_type(
            4,
            AwbcRuntimeTypeShape::Function {
                contract: RuntimeFunctionTypeContract::default(),
                parameters: vec![AwbcTypeId(0)],
                result: AwbcTypeId(0),
            },
        ),
    ]);
    let first = RuntimeCallableParameterCoordinate {
        group: 0,
        parameter: 0,
    };
    let second = RuntimeCallableParameterCoordinate {
        group: 0,
        parameter: 1,
    };
    program.callable_states = vec![
        RuntimeCallableStateDefinition {
            function_type: AwbcTypeId(2),
            origin: callable_state_id(0),
            position: RuntimeCallablePosition::Unapplied,
            retained: Box::new([]),
            parameters: vec![
                RuntimeCallableParameterInput {
                    coordinate: first,
                    kind: RuntimeCallableParameterKind::Fixed,
                    abi_ty: AwbcTypeId(0),
                    binding_ty: AwbcTypeId(0),
                },
                RuntimeCallableParameterInput {
                    coordinate: second,
                    kind: RuntimeCallableParameterKind::Fixed,
                    abi_ty: AwbcTypeId(0),
                    binding_ty: AwbcTypeId(0),
                },
            ]
            .into_boxed_slice(),
            result: AwbcTypeId(0),
            attached: RuntimeCallableAttachedContract::None,
            transition: RuntimeCallableTransition::Invoke {
                function: AwbcFunctionId(1),
                captures: Box::new([]),
                arguments: vec![
                    RuntimeCallableInputSource::Argument { position: 0 },
                    RuntimeCallableInputSource::Argument { position: 1 },
                ]
                .into_boxed_slice(),
            },
            partials: vec![crate::plan::RuntimeCallablePartialTransition {
                parameters: vec![first].into_boxed_slice(),
                state: callable_state_id(1),
                values: vec![RuntimeCallableInputSource::Argument { position: 0 }]
                    .into_boxed_slice(),
            }]
            .into_boxed_slice(),
        },
        RuntimeCallableStateDefinition {
            function_type: AwbcTypeId(3),
            origin: callable_state_id(0),
            position: RuntimeCallablePosition::WithinGroup {
                group: 0,
                bound: vec![first].into_boxed_slice(),
            },
            retained: vec![RuntimeCallableRetainedInput {
                role: RuntimeCallableRetainedRole::Parameter(first),
                ty: AwbcTypeId(0),
            }]
            .into_boxed_slice(),
            parameters: vec![RuntimeCallableParameterInput {
                coordinate: second,
                kind: RuntimeCallableParameterKind::Fixed,
                abi_ty: AwbcTypeId(0),
                binding_ty: AwbcTypeId(0),
            }]
            .into_boxed_slice(),
            result: AwbcTypeId(0),
            attached: RuntimeCallableAttachedContract::None,
            transition: RuntimeCallableTransition::Invoke {
                function: AwbcFunctionId(1),
                captures: Box::new([]),
                arguments: vec![
                    RuntimeCallableInputSource::Retained { position: 0 },
                    RuntimeCallableInputSource::Argument { position: 0 },
                ]
                .into_boxed_slice(),
            },
            partials: Box::new([]),
        },
    ];
    program.signatures[0].result = Some(AwbcTypeId(3));
    program.signatures[1].params = vec![AwbcTypeId(0), AwbcTypeId(0)];
    program.frame_layouts[0].slots[0].ty = AwbcTypeId(2);
    program.frame_layouts[0].slots[1].ty = AwbcTypeId(3);
    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(0),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    program.frame_layouts[1].slots = vec![
        AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(0),
            role: AwbcFrameSlotRole::Parameter,
            scope_depth: 0,
        },
        AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(0),
            role: AwbcFrameSlotRole::Parameter,
            scope_depth: 0,
        },
    ];
    program.instructions.insert(
        1,
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(2),
            constant: AwbcConstantId(0),
        },
    );
    let AwbcInstruction::ApplyGroup { args, .. } = &mut program.instructions[2] else {
        panic!("expression apply fixture uses ApplyGroup");
    };
    args.push(AwbcRegisterId(2));
    program.blocks[0].instructions = AwbcTableRange::new(0, 3);
    program.blocks[1].instructions = AwbcTableRange::new(3, 1);
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify callable partial application program");

    let program = std::sync::Arc::new(program);
    let mut fiber =
        FiberState::for_entry(&program, AwbcEntryId(0), 0, 16).expect("create partial fiber");
    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("partially apply callable state");
    let super::vm::VmExit::Returned(Some(RuntimeValue::Callable(callable))) = output.exit else {
        panic!("partial application must return a callable state");
    };
    assert_eq!(callable.state(), callable_state_id(1));
    assert_eq!(callable.retained(), [RuntimeValue::Unit]);
    assert_eq!(fiber.frames.len(), 1);
}

#[test]
fn verifier_rejects_callable_body_abi_mismatch() {
    let mut program = expression_apply_program(
        Vec::new(),
        vec![(
            AwbcTerminator::Return { value: None },
            AwbcSafePointKind::CallableBoundary,
        )],
        Vec::new(),
    );
    program.signatures[1].params.push(AwbcTypeId(0));
    program.frame_layouts[1].slots[0].role = AwbcFrameSlotRole::Parameter;

    assert!(matches!(
        program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::InvalidInvariant { message, .. })
            if message.contains("callable transition target ABI disagrees with typed projections")
    ));
}

#[test]
fn expression_apply_verifier_rejects_more_arguments_than_the_callable_group() {
    let mut program = expression_apply_program(
        Vec::new(),
        vec![(
            AwbcTerminator::Return { value: None },
            AwbcSafePointKind::CallableBoundary,
        )],
        Vec::new(),
    );
    let AwbcInstruction::ApplyGroup { args, .. } = &mut program.instructions[1] else {
        panic!("expression apply fixture ends with ApplyGroup");
    };
    args.push(AwbcRegisterId(0));
    assert!(matches!(
        program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::InvalidInvariant { message, .. })
            if message.contains("group application supplies more values than its function arrow")
    ));
}

#[test]
fn budget_preemption_inside_dynamic_callee_resumes_at_the_exact_cursor() {
    let program = std::sync::Arc::new(expression_apply_program(
        Vec::new(),
        vec![(
            AwbcTerminator::Return { value: None },
            AwbcSafePointKind::CallableBoundary,
        )],
        Vec::new(),
    ));
    let mut fiber =
        FiberState::for_entry(&program, AwbcEntryId(0), 0, 2).expect("create budgeted fiber");
    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("preempt dynamic callee");

    let exact_callee_entry = super::fiber::FiberCursor {
        function: AwbcFunctionId(1),
        block: AwbcBlockId(1),
        instruction_offset: 0,
    };
    assert!(matches!(
        output.exit,
        super::vm::VmExit::BudgetYield(point) if point.cursor == exact_callee_entry
    ));
    assert_eq!(
        fiber
            .suspension
            .as_ref()
            .map(|suspension| suspension.resume),
        Some(FiberResumeTarget::Exact(exact_callee_entry))
    );
    fiber
        .validate_for_program(&program)
        .expect("preempted dynamic call snapshot validates");
    let mut fiber = fiber.clone();
    fiber
        .resume_budget_yield(&program)
        .expect("resume exact budget target");
    assert_eq!(fiber.cursor, exact_callee_entry);
    fiber.replenish_budget();

    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("return from the exact-resumed dynamic callee");
    let caller_result_load = super::fiber::FiberCursor {
        function: AwbcFunctionId(0),
        block: AwbcBlockId(0),
        instruction_offset: 2,
    };
    assert!(matches!(
        output.exit,
        super::vm::VmExit::BudgetYield(point) if point.cursor == caller_result_load
    ));
    fiber
        .resume_budget_yield(&program)
        .expect("resume at the caller result load");
    fiber.replenish_budget();
    let output = step_with_callable_context(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("finish the exact-resumed dynamic call");
    assert_eq!(
        output.exit,
        super::vm::VmExit::Returned(Some(RuntimeValue::Unit))
    );
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
            resume: FiberResumeTarget::Declared(AwbcResumePointId(0)),
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
        scopes: Vec::new(),
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
        flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
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
