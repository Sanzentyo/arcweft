use super::codec::{AwbcCodecError, AwbcDecodeBudget};
use super::fiber::{
    FiberAwaitTarget, FiberResumeTarget, FiberScope, FiberScopeCleanup, FiberState, FiberStatus,
    FiberSuspension, FiberSuspensionReason, FiberTrap,
};
use super::schema::*;
use super::verify::{AwbcVerifyBudget, AwbcVerifyContext, AwbcVerifyError};
use crate::effect::RuntimeAssertionGuardId;
use crate::entry::{FlowContractHash, RuntimeFlowExecutable};
use crate::pattern::{
    RuntimeCheckedType, RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner,
    RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId,
};
use crate::plan::{FlowRuntimeId, RuntimeAgentOperationalType, RuntimeFlowTargetError};
use crate::value::{
    RuntimeFunctionValue, RuntimeHandleKind, RuntimeOpaquePersistence, RuntimeOpaqueValueClass,
    RuntimeValue, runtime_sequence_values,
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
fn typed_drop_is_an_exact_vm_transaction_boundary() {
    let mut program = minimal_program();
    program.runtime_types = vec![runtime_type(1, AwbcRuntimeTypeShape::Unit)];
    program.constants = vec![AwbcConstant::Unit];
    program.frame_layouts[0] = AwbcFrameLayout {
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

#[test]
fn verifier_requires_option_destination_for_optional_agent_fields() {
    let mut program = minimal_program();
    program
        .strings
        .extend(["parent_id".to_owned(), "Some".to_owned(), "None".to_owned()]);
    program.runtime_types = vec![
        runtime_type(
            1,
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(
                RuntimeAgentOperationalType::ObservedObject,
            )),
        ),
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
                        payload: Some(AwbcTypeId(1)),
                    },
                    AwbcVariantCase {
                        name: AwbcStringId(3),
                        payload: None,
                    },
                ],
            },
        ),
        runtime_type(4, AwbcRuntimeTypeShape::Bool),
    ];
    program.signatures[0].params = vec![AwbcTypeId(0)];
    program.frame_layouts[0] = AwbcFrameLayout {
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
        runtime_type(2, AwbcRuntimeTypeShape::Sequence(AwbcTypeId(0))),
        runtime_type(
            3,
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(
                RuntimeAgentOperationalType::Predicate,
            )),
        ),
    ];
    all.signatures[0].params = vec![AwbcTypeId(1)];
    all.frame_layouts[0] = AwbcFrameLayout {
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
                    fields: vec![
                        AwbcRecordField {
                            name: AwbcStringId(0),
                            ty: AwbcTypeId(1),
                        },
                        AwbcRecordField {
                            name: AwbcStringId(2),
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
fn fiber_snapshot_rejects_stale_functions_in_cleanup_arguments() {
    let mut program = minimal_program();
    program
        .strings
        .extend(["zz.debug".to_owned(), "zz.message".to_owned()]);
    program.constants.extend([
        AwbcConstant::String(AwbcStringId(1)),
        AwbcConstant::String(AwbcStringId(2)),
    ]);
    let dynamic_type = AwbcTypeId(1);
    program.signatures.push(AwbcSignature {
        params: vec![dynamic_type],
        result: None,
        effects: AwbcEffectSetId(0),
    });
    program.effect_plans.push(AwbcEffectPlan {
        kind: AwbcEffectKind::Log,
        signature: AwbcSignatureId(1),
        capability: None,
        audio: None,
        static_args: vec![AwbcConstantId(0), AwbcConstantId(1)],
        resources: Vec::new(),
    });
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64)
        .expect("create snapshot validation fiber");
    fiber
        .active_frame_mut()
        .expect("active frame")
        .root_cleanups
        .push(FiberScopeCleanup {
            key: "cleanup.stale-function".to_owned(),
            effect: AwbcEffectPlanId(0),
            args: vec![RuntimeValue::Function(RuntimeFunctionValue::new_awbc(
                Vec::new(),
                AwbcFunctionId(u32::MAX),
                Vec::new(),
            ))],
        });

    let error = fiber
        .validate_for_program(&program)
        .expect_err("stale cleanup function must reject");
    assert!(
        matches!(
        &error,
        super::fiber::FiberStateError::InvalidRuntimeValue { path, reason }
            if path == "frames[0].root_cleanups[0].args[0]"
                && reason.contains("does not exist")
        ),
        "{error:?}"
    );
}

fn expression_apply_frame_layouts() -> Vec<AwbcFrameLayout> {
    vec![
        AwbcFrameLayout {
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
            slots: vec![AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            }],
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
            instructions: AwbcTableRange::new(
                2,
                if index == 0 {
                    synthetic_instruction_len
                } else {
                    0
                },
            ),
            terminator,
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
                result: None,
                effects: AwbcEffectSetId(0),
            },
        ],
        frame_layouts: expression_apply_frame_layouts(),
        functions: expression_apply_functions(synthetic_len),
        blocks,
        instructions: {
            let mut instructions = vec![
                AwbcInstruction::MakeFunction {
                    dst: AwbcRegisterId(0),
                    function: AwbcFunctionId(1),
                    params: Vec::new(),
                    capture_names: Vec::new(),
                    captures: Vec::new(),
                },
                AwbcInstruction::ApplyFunction {
                    dst: AwbcRegisterId(1),
                    callee: AwbcRegisterId(0),
                    args: Vec::new(),
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
    let decoded = AwbcProgram::decode_canonical(&first, AwbcDecodeBudget::default())
        .expect("decode canonical AWBC");
    assert_eq!(decoded, program);
    decoded
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify decoded AWBC");
}

#[test]
fn canonical_codec_round_trips_checked_flow_identity_and_public_label() {
    let mut program = minimal_program();
    let flow = FlowRuntimeId::from_checked_declaration_digest([0xa5; 32], "flow.opening")
        .expect("accepted Flow public label");
    program.flow_bindings[0].flow = flow.clone();
    program.flow_executables[0].metadata.flow = flow.clone();

    let encoded = program.encode_canonical().expect("encode checked Flow ID");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode checked Flow ID");

    assert_eq!(decoded.flow_executables[0].metadata.flow, flow);
    assert_eq!(
        decoded.flow_executables[0]
            .metadata
            .flow
            .public_label()
            .as_str(),
        "flow.opening"
    );
    assert_eq!(decoded, program);
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
                if message == "builtin variant owner has a non-canonical case schema"
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
        case_name: AwbcStringId(2),
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
    let program = minimal_program();
    let mut fiber =
        FiberState::for_entry(&program, AwbcEntryId(0), 7, 64).expect("fiber initializes");
    fiber
        .active_frame_mut()
        .expect("active frame")
        .root_cleanups
        .push(FiberScopeCleanup {
            key: "handle.root".to_owned(),
            effect: AwbcEffectPlanId(0),
            args: vec![RuntimeValue::String("root".to_owned())],
        });
    fiber
        .active_frame_mut()
        .expect("active frame")
        .scopes
        .push(FiberScope {
            id: AwbcScopeId(0),
            depth: 1,
            cleanups: vec![FiberScopeCleanup {
                key: "handle.scope".to_owned(),
                effect: AwbcEffectPlanId(0),
                args: vec![RuntimeValue::String("scope".to_owned())],
            }],
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
    reason = "The AWBC closure fixture is intentionally inline so opcode, verifier, codec, and VM expectations stay in one place."
)]
fn closure_instructions_capture_and_apply_awbc_function_value() {
    let program = AwbcProgram {
        strings: vec!["captured".to_owned(), "main".to_owned(), "x".to_owned()],
        constants: vec![AwbcConstant::String(AwbcStringId(0))],
        signatures: vec![
            AwbcSignature {
                params: Vec::new(),
                result: Some(AwbcTypeId(1)),
                effects: AwbcEffectSetId(0),
            },
            AwbcSignature {
                params: vec![AwbcTypeId(1)],
                result: Some(AwbcTypeId(1)),
                effects: AwbcEffectSetId(0),
            },
        ],
        frame_layouts: vec![
            AwbcFrameLayout {
                slots: vec![
                    AwbcFrameSlot {
                        name: Some(AwbcStringId(2)),
                        ty: AwbcTypeId(1),
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
                        ty: AwbcTypeId(1),
                        role: AwbcFrameSlotRole::Temporary,
                        scope_depth: 0,
                    },
                ],
                max_scope_depth: 0,
            },
            AwbcFrameLayout {
                slots: vec![AwbcFrameSlot {
                    name: Some(AwbcStringId(2)),
                    ty: AwbcTypeId(1),
                    role: AwbcFrameSlotRole::Parameter,
                    scope_depth: 0,
                }],
                max_scope_depth: 0,
            },
        ],
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
            AwbcInstruction::MakeFunction {
                dst: AwbcRegisterId(1),
                function: AwbcFunctionId(1),
                params: Vec::new(),
                capture_names: vec![AwbcStringId(2)],
                captures: vec![AwbcRegisterId(0)],
            },
            AwbcInstruction::ApplyFunction {
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
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("closure program verifies");

    let encoded = program.encode_canonical().expect("encode closure program");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("decode closure program");
    assert_eq!(decoded, program);

    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64).expect("create fiber");
    let output = super::vm::step(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("step closure program");
    assert_eq!(
        output.exit,
        super::vm::VmExit::Returned(Some(RuntimeValue::String("captured".to_owned())))
    );
}

#[test]
fn expression_apply_preserves_dynamic_call_frame_across_suspension_and_resume() {
    let program = expression_apply_program(
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
    );
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify expression apply suspension program");

    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64)
        .expect("create expression apply fiber");
    let output = super::vm::step(
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
    let output = super::vm::step(
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

    let mut fiber = FiberState::for_entry(&await_program, AwbcEntryId(0), 0, 64)
        .expect("create await expression apply fiber");
    let output = super::vm::step(
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
        super::vm::step(
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
    host_program.host_calls.push(AwbcHostCall {
        public_id: AwbcStringId(0),
        capability: AwbcStringId(0),
        operation: AwbcStringId(0),
        contract: None,
        signature: AwbcSignatureId(1),
        mode: AwbcHostCallMode::Suspend,
        deterministic: true,
        arguments: Vec::new(),
    });

    let mut fiber = FiberState::for_entry(&host_program, AwbcEntryId(0), 0, 64)
        .expect("create host-call expression apply fiber");
    let output = super::vm::step(
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
        super::vm::step(
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
    let program = expression_apply_program(
        synthetic_instructions,
        vec![(
            AwbcTerminator::Return { value: None },
            AwbcSafePointKind::CallableBoundary,
        )],
        Vec::new(),
    );
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify expression apply budget program");

    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 8_192)
        .expect("create expression apply fiber");
    let output = super::vm::step(
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
    program.signatures[0].result = Some(AwbcTypeId(1));
    program.signatures[1].params.push(AwbcTypeId(1));
    program.frame_layouts[0].slots[1].ty = AwbcTypeId(1);
    program.frame_layouts[1].slots[0].ty = AwbcTypeId(1);
    program.frame_layouts[1].slots[0].role = AwbcFrameSlotRole::Parameter;
    program.frame_layouts[1].slots[0].name = Some(AwbcStringId(0));
    let AwbcInstruction::MakeFunction { params, .. } = &mut program.instructions[0] else {
        panic!("expression apply fixture starts with MakeFunction");
    };
    params.push(AwbcStringId(0));
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify partial expression apply program");

    let mut fiber =
        FiberState::for_entry(&program, AwbcEntryId(0), 0, 16).expect("create partial fiber");
    let output = super::vm::step(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("partially apply function");
    let super::vm::VmExit::Returned(Some(RuntimeValue::Function(function))) = output.exit else {
        panic!("partial application must return a function value");
    };
    assert_eq!(function.remaining_arity(), Ok(1));
    let crate::value::RuntimeFunctionBody::Awbc(closure) = function.body() else {
        panic!("AWBC execution must return an AWBC closure");
    };
    assert!(closure.captures().is_empty());
    assert_eq!(fiber.frames.len(), 1);
}

#[test]
fn verifier_rejects_make_function_binding_name_mismatch() {
    let mut program = expression_apply_program(
        Vec::new(),
        vec![(
            AwbcTerminator::Return { value: None },
            AwbcSafePointKind::CallableBoundary,
        )],
        Vec::new(),
    );
    program.signatures[1].params.push(AwbcTypeId(1));
    program.frame_layouts[1].slots[0].ty = AwbcTypeId(1);
    program.frame_layouts[1].slots[0].role = AwbcFrameSlotRole::Parameter;
    program.frame_layouts[1].slots[0].name = Some(AwbcStringId(0));
    let AwbcInstruction::MakeFunction { params, .. } = &mut program.instructions[0] else {
        panic!("expression apply fixture starts with MakeFunction");
    };
    params.push(AwbcStringId(0));
    program.frame_layouts[1].slots[0].name = None;

    assert!(matches!(
        program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::InvalidInvariant { message, .. })
            if message.contains("name matching its closure binding")
    ));
}

#[test]
fn expression_apply_rejects_over_application_before_entering_the_callee() {
    let mut program = expression_apply_program(
        Vec::new(),
        vec![(
            AwbcTerminator::Return { value: None },
            AwbcSafePointKind::CallableBoundary,
        )],
        Vec::new(),
    );
    let AwbcInstruction::ApplyFunction { args, .. } = &mut program.instructions[1] else {
        panic!("expression apply fixture ends with ApplyFunction");
    };
    args.push(AwbcRegisterId(0));
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("verify over-application program");

    let mut fiber =
        FiberState::for_entry(&program, AwbcEntryId(0), 0, 16).expect("create over-apply fiber");
    let output = super::vm::step(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("over-application becomes a typed trap");
    assert!(matches!(
        output.exit,
        super::vm::VmExit::Trapped(FiberTrap {
            code: AwbcTrapCode::TypeMismatch,
            message: Some(ref message),
            ..
        }) if message == "function application expected 0 arguments, received 1"
    ));
    assert_eq!(fiber.frames.len(), 1);
}

#[test]
fn budget_preemption_inside_dynamic_callee_resumes_at_the_exact_cursor() {
    let program = expression_apply_program(
        Vec::new(),
        vec![(
            AwbcTerminator::Return { value: None },
            AwbcSafePointKind::CallableBoundary,
        )],
        Vec::new(),
    );
    let mut fiber =
        FiberState::for_entry(&program, AwbcEntryId(0), 0, 2).expect("create budgeted fiber");
    let output = super::vm::step(
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

    let output = super::vm::step(
        &program,
        &mut fiber,
        super::vm::VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("finish exact-resumed dynamic call");
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
