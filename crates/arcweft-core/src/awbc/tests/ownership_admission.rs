use super::*;
use crate::awbc::fiber::FiberCursor;
use crate::awbc::schema::AwbcPlaceReadMode;
use crate::entry::RuntimeSchemaLimits;
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::value::ownership::RuntimeValueOwnershipAdmissionError;
use crate::value::{AwbcRuntimeValueSnapshot, RuntimePlaceStorage, RuntimeSeq};
use std::sync::Arc;

fn applied_group_retained_copy_program() -> AwbcProgram {
    let mut program = project_call_retained_copy_program();
    program.instructions.insert(
        2,
        AwbcInstruction::ApplyGroup {
            dst: AwbcRegisterId(1),
            callee: AwbcRegisterId(0),
            args: vec![AwbcRegisterId(2)],
        },
    );
    program.blocks[0].instructions.len += 1;
    program.blocks[0].terminator = AwbcTerminator::Jump {
        target: AwbcBlockId(1),
    };
    for block in program.blocks.iter_mut().skip(1) {
        block.instructions.start += 1;
    }
    program
}

#[test]
fn group_application_retained_copy_requires_the_actual_input_proof() {
    let program = applied_group_retained_copy_program();
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("the sealed retaining transition preserves its unrestricted input");
    let encoded = program.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default()).unwrap();
    assert_eq!(decoded, program);
    decoded
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    let mut affine = program;
    make_retained_operand_affine_capable(&mut affine);
    expect_project_call_rejection(affine, "producer-proven unrestricted CopyValue source");
}

#[test]
fn group_application_copy_projection_spends_the_existing_dataflow_budget() {
    let program = applied_group_retained_copy_program();
    assert!(matches!(
        program.verify(
            AwbcVerifyBudget {
                dataflow_steps: 4,
                ..AwbcVerifyBudget::default()
            },
            AwbcVerifyContext::default()
        ),
        Err(AwbcVerifyError::BudgetExceeded {
            budget: "dataflow_steps"
        })
    ));
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
}

fn guarded_copy_program() -> AwbcProgram {
    let mut program = minimal_program();
    program.runtime_types = vec![
        runtime_type(71, AwbcRuntimeTypeShape::Unit),
        runtime_type(72, AwbcRuntimeTypeShape::Need(AwbcTypeId(0))),
        runtime_type(
            73,
            AwbcRuntimeTypeShape::Sequence {
                kind: crate::plan::RuntimePlanSequenceKind::Seq,
                item: AwbcTypeId(1),
            },
        ),
    ];
    program.signatures[0].params = vec![AwbcTypeId(2)];
    program.functions[0].input_ownership = vec![AwbcFunctionInputOwnership::parameter(
        manual_awbc_formal_identity("awbc.ownership_admission.input"),
        0,
        crate::plan::RuntimeFunctionParameterPassing::Affine,
    )];
    program.frame_layouts[0].slots = [AwbcFrameSlotRole::Parameter, AwbcFrameSlotRole::Temporary]
        .into_iter()
        .map(|role| AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(2),
            role,
            scope_depth: 0,
        })
        .collect();
    program.instructions = vec![
        AwbcInstruction::RequireUnrestricted {
            value: AwbcRegisterId(0),
        },
        AwbcInstruction::CopyValue {
            dst: AwbcRegisterId(1),
            src: AwbcRegisterId(0),
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 2);
    program
}

fn owned_sequence(program: &AwbcProgram, affine: bool) -> FiberState {
    let items = if affine {
        vec![RuntimeValue::NeedHandle(
            crate::tests::reusable_need_with_outcome(
                "guarded.need",
                crate::task::TaskOutcomeContract::program(RuntimeSemanticTypeId::from_bytes(
                    [71; 32],
                )),
            ),
        )]
    } else {
        Vec::new()
    };
    let mut fiber = FiberState::for_entry(program, AwbcEntryId(0), 1, 64).unwrap();
    fiber
        .bind_function_argument_values_owned(
            program,
            vec![RuntimeValue::Seq(RuntimeSeq::values(items))],
        )
        .unwrap();
    fiber
}

#[test]
fn ownership_guard_refines_only_the_checked_value_at_the_exact_cursor() {
    let program = guarded_copy_program();
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    let encoded = program.encode_canonical().unwrap();
    assert_eq!(
        AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default()).unwrap(),
        program
    );
    for (offset, expected) in [
        (0, vec![]),
        (1, vec![AwbcRegisterId(0)]),
        (2, vec![AwbcRegisterId(0), AwbcRegisterId(1)]),
    ] {
        assert_eq!(
            program
                .copy_proofs_at(FiberCursor {
                    function: AwbcFunctionId(0),
                    block: AwbcBlockId(0),
                    instruction_offset: offset,
                })
                .unwrap()
                .into_iter()
                .map(|(register, _)| register)
                .collect::<Vec<_>>(),
            expected
        );
    }
    let mut unchecked = program.clone();
    unchecked.instructions[0] = AwbcInstruction::Nop;
    expect_project_call_rejection(unchecked, "producer-proven unrestricted CopyValue source");
    let mut wrong_source = program;
    wrong_source.instructions[0] = AwbcInstruction::RequireUnrestricted {
        value: AwbcRegisterId(1),
    };
    assert!(matches!(
        wrong_source.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::UninitializedRegister { register: 1, .. })
    ));
}

#[test]
fn ownership_guard_rejects_an_affine_descendant_without_consuming_the_source() {
    let program = Arc::new(guarded_copy_program());
    let owner = RuntimeProgramOwner::Awbc(Arc::clone(&program));
    let mut fiber = owned_sequence(&program, true);
    let before = AwbcFiberStateSnapshot::from_live(&fiber, &owner).unwrap();
    let output = super::super::vm::step(
        &program,
        &mut fiber,
        super::super::vm::VmStepOptions {
            max_instructions: 1,
        },
    )
    .unwrap();
    assert!(matches!(output.exit, super::super::vm::VmExit::Trapped(_)));
    assert_eq!(fiber.cursor.instruction_offset, 0);
    let after = AwbcFiberStateSnapshot::from_live(&fiber, &owner).unwrap();
    assert_eq!(after.frames[0].registers, before.frames[0].registers);
    assert!(after.frames[0].registers[1].as_ref().is_none());
}

#[test]
fn restore_rechecks_original_cursor_ownership_before_atomic_replacement() {
    let program = Arc::new(guarded_copy_program());
    let owner = RuntimeProgramOwner::Awbc(Arc::clone(&program));
    let affine = owned_sequence(&program, true);
    let affine_image = AwbcFiberStateSnapshot::from_live(&affine, &owner).unwrap();
    affine_image.clone().into_live_for_program(&owner).unwrap();
    let mut valid = owned_sequence(&program, false);
    super::super::vm::step(
        &program,
        &mut valid,
        super::super::vm::VmStepOptions {
            max_instructions: 1,
        },
    )
    .unwrap();
    assert_eq!(valid.cursor.instruction_offset, 1);
    let saved = AwbcFiberStateSnapshot::from_live(&valid, &owner).unwrap();
    let encoded = serde_json::to_vec(&saved).unwrap();
    let decoded: AwbcFiberStateSnapshot = serde_json::from_slice(&encoded).unwrap();
    let mut restored = decoded.into_live_for_program(&owner).unwrap();
    let mut tampered = saved.clone();
    tampered.frames[0].registers[0] = affine_image.frames[0].registers[0].clone();
    let error = restored
        .replace_from_snapshot(tampered, &owner)
        .unwrap_err();
    assert!(error.to_string().contains("unrestricted"), "{error:?}");
    assert_eq!(
        AwbcFiberStateSnapshot::from_live(&restored, &owner).unwrap(),
        saved
    );
    super::super::vm::step(
        &program,
        &mut restored,
        super::super::vm::VmStepOptions {
            max_instructions: 2,
        },
    )
    .unwrap();
    assert_eq!(restored.status, FiberStatus::Returned);
}

#[test]
fn ownership_and_resources_share_one_cumulative_live_and_inert_traversal() {
    let live = RuntimeValue::Tuple(vec![RuntimeValue::Unit, RuntimeValue::Unit]);
    let inert = AwbcRuntimeValueSnapshot::Tuple(vec![
        AwbcRuntimeValueSnapshot::Unit,
        AwbcRuntimeValueSnapshot::Unit,
    ]);
    for value in [live.view(), inert.view()] {
        let mut exact = ValueResourceAdmission::live(RuntimeSchemaLimits {
            max_nodes: 3,
            ..RuntimeSchemaLimits::engine_default()
        });
        RuntimeValueOwnershipRequirement::Unrestricted
            .admit(value, &mut exact, 0)
            .unwrap();
        assert!(matches!(
            RuntimeValueOwnershipRequirement::Owned.admit(RuntimeValue::Unit.view(), &mut exact, 0),
            Err(RuntimeValueOwnershipAdmissionError::Resource(_))
        ));
        let mut short = ValueResourceAdmission::live(RuntimeSchemaLimits {
            max_nodes: 2,
            ..RuntimeSchemaLimits::engine_default()
        });
        assert!(matches!(
            RuntimeValueOwnershipRequirement::Unrestricted.admit(value, &mut short, 0),
            Err(RuntimeValueOwnershipAdmissionError::Resource(_))
        ));
    }
}

#[test]
fn inert_frame_counts_precede_original_ownership_dataflow() {
    let mut program = guarded_copy_program();
    let original = RuntimeProgramOwner::Awbc(Arc::new(program.clone()));
    let saved =
        AwbcFiberStateSnapshot::from_live(&owned_sequence(&program, false), &original).unwrap();
    program.instructions[0] = AwbcInstruction::Nop;
    let owner = RuntimeProgramOwner::Awbc(Arc::new(program));
    for (maximum, resource_first) in [(1, true), (2, false)] {
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            RuntimeSchemaLimits {
                max_sequence_items: maximum,
                ..RuntimeSchemaLimits::engine_default()
            },
        );
        let error = saved.admit_retained(&mut admission).unwrap_err();
        if resource_first {
            assert!(
                matches!(
                    error,
                    crate::value::AwbcRuntimeValueSnapshotError::Resource(_)
                ),
                "{error:?}"
            );
        } else {
            assert!(
                matches!(
                    error,
                    crate::value::AwbcRuntimeValueSnapshotError::ResumeOwnership(_)
                ),
                "{error:?}"
            );
        }
    }
}

fn mixed_copy_program() -> AwbcProgram {
    let mut program = guarded_copy_program();
    program.runtime_types.push(runtime_type(
        74,
        AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(2), AwbcTypeId(1)]),
    ));
    program.signatures[0].params = vec![AwbcTypeId(1)];
    program.frame_layouts[0].slots = [1, 2, 3, 2]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(ty),
            role: if index == 0 {
                AwbcFrameSlotRole::Parameter
            } else {
                AwbcFrameSlotRole::Temporary
            },
            scope_depth: 0,
        })
        .collect();
    program.instructions = vec![
        AwbcInstruction::MakeSequence {
            dst: AwbcRegisterId(1),
            items: Vec::new(),
        },
        AwbcInstruction::MakeTuple {
            dst: AwbcRegisterId(2),
            items: vec![AwbcRegisterId(1), AwbcRegisterId(0)],
        },
        AwbcInstruction::ProjectTuple {
            dst: AwbcRegisterId(3),
            target: AwbcRegisterId(2),
            ordinal: 0,
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 3);
    program
}

fn mixed_copy_fiber(program: &AwbcProgram, identity: &str) -> FiberState {
    let mut fiber = FiberState::for_entry(program, AwbcEntryId(0), 1, 64).unwrap();
    fiber
        .bind_function_argument_values_owned(
            program,
            vec![RuntimeValue::NeedHandle(
                crate::tests::reusable_need_with_outcome(
                    identity,
                    crate::task::TaskOutcomeContract::program(RuntimeSemanticTypeId::from_bytes(
                        [71; 32],
                    )),
                ),
            )],
        )
        .unwrap();
    fiber
}

#[test]
fn restore_checks_copy_subtrees_without_rejecting_affine_siblings() {
    let program = Arc::new(mixed_copy_program());
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    let owner = RuntimeProgramOwner::Awbc(Arc::clone(&program));
    let mut distinct = mixed_copy_fiber(&program, "distinct.forged.child");
    let forged = RuntimeValue::Seq(RuntimeSeq::values(vec![
        distinct.frames[0].registers[0].take().unwrap(),
    ]));
    let forged = AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&forged, &owner).unwrap();
    let mut fiber = mixed_copy_fiber(&program, "original.affine.sibling");
    super::super::vm::step(
        &program,
        &mut fiber,
        super::super::vm::VmStepOptions {
            max_instructions: 2,
        },
    )
    .unwrap();
    assert_eq!(fiber.cursor.instruction_offset, 2);
    let saved = AwbcFiberStateSnapshot::from_live(&fiber, &owner).unwrap();
    let mut restored = saved.clone().into_live_for_program(&owner).unwrap();
    let mut tampered = saved.clone();
    let Some(AwbcRuntimeValueSnapshot::Tuple(items)) = tampered.frames[0].registers[2].as_mut()
    else {
        panic!("the mixed aggregate remains a tuple");
    };
    items[0] = forged;
    let error = restored
        .replace_from_snapshot(tampered, &owner)
        .expect_err("a distinct affine descendant cannot replace a producer-proven Copy subtree");
    assert!(
        error
            .to_string()
            .contains("original Copy-proof value shape"),
        "{error:?}"
    );
    assert_eq!(
        AwbcFiberStateSnapshot::from_live(&restored, &owner).unwrap(),
        saved
    );
    super::super::vm::step(
        &program,
        &mut restored,
        super::super::vm::VmStepOptions {
            max_instructions: 2,
        },
    )
    .unwrap();
    assert_eq!(restored.status, FiberStatus::Returned);

    let mut distinct = mixed_copy_fiber(&program, "distinct.live.child");
    let Some(RuntimeValue::Tuple(items)) = fiber.frames[0].registers[2].as_mut() else {
        panic!("the live mixed aggregate remains a tuple");
    };
    items[0] = RuntimeValue::Seq(RuntimeSeq::values(vec![
        distinct.frames[0].registers[0].take().unwrap(),
    ]));
    let error = AwbcFiberStateSnapshot::from_live(&fiber, &owner)
        .expect_err("live capture must check the same original subtree proof before copying");
    assert!(
        error
            .to_string()
            .contains("original Copy-proof value shape"),
        "{error:?}"
    );
}

#[test]
fn pushing_to_an_unknown_copy_sequence_does_not_invent_a_copy_prefix() {
    let mut program = guarded_copy_program();
    program.runtime_types[2] = runtime_type(
        73,
        AwbcRuntimeTypeShape::Sequence {
            kind: crate::plan::RuntimePlanSequenceKind::Vec,
            item: AwbcTypeId(1),
        },
    );
    program.signatures[0].params = vec![AwbcTypeId(2), AwbcTypeId(1)];
    program.functions[0].input_ownership = [
        crate::plan::RuntimeFunctionParameterPassing::Value,
        crate::plan::RuntimeFunctionParameterPassing::Affine,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, passing)| {
        AwbcFunctionInputOwnership::parameter(
            manual_awbc_formal_identity(&format!("awbc.ownership_admission.push.{index}")),
            u32::try_from(index).unwrap(),
            passing,
        )
    })
    .collect();
    program.functions[0].input_ownership[0].requirement =
        RuntimeValueOwnershipRequirement::Unrestricted;
    program.frame_layouts[0].slots = [2, 1, 1, 1]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(ty),
            role: if index < 2 {
                AwbcFrameSlotRole::Parameter
            } else {
                AwbcFrameSlotRole::Temporary
            },
            scope_depth: 0,
        })
        .collect();
    program.instructions = vec![
        AwbcInstruction::SequencePush {
            sequence: AwbcRegisterId(0),
            value: AwbcRegisterId(1),
        },
        AwbcInstruction::CopyValue {
            dst: AwbcRegisterId(3),
            src: AwbcRegisterId(2),
        },
    ];
    let mut some = program.blocks[0].clone();
    some.instructions = AwbcTableRange::new(1, 1);
    some.safe_point = AwbcSafePointKind::None;
    let mut none = some.clone();
    none.instructions = AwbcTableRange::new(2, 0);
    program.blocks[0].instructions = AwbcTableRange::new(0, 1);
    program.blocks[0].terminator = AwbcTerminator::SequenceNext {
        sequence: AwbcRegisterId(0),
        item: AwbcRegisterId(2),
        some_block: AwbcBlockId(1),
        none_block: AwbcBlockId(2),
    };
    program.blocks.extend([some, none]);
    program.functions[0].blocks = AwbcTableRange::new(0, 3);
    expect_project_call_rejection(program, "producer-proven unrestricted CopyValue source");
}

#[test]
fn partial_copy_relations_share_the_admitted_node_and_work_allowance() {
    let live = RuntimeValue::Tuple(vec![RuntimeValue::Unit, RuntimeValue::Unit]);
    let inert = AwbcRuntimeValueSnapshot::Tuple(vec![
        AwbcRuntimeValueSnapshot::Unit,
        AwbcRuntimeValueSnapshot::Unit,
    ]);
    let proof = crate::awbc::verify::CopyProof::Tuple(vec![
        crate::awbc::verify::CopyProof::Copyable,
        crate::awbc::verify::CopyProof::Affine,
    ]);
    for value in [live.view(), inert.view()] {
        let storage = RuntimePlaceStorage::from(value);
        let mut resources = ValueResourceAdmission::live(RuntimeSchemaLimits {
            max_nodes: 3,
            ..RuntimeSchemaLimits::engine_default()
        });
        RuntimeValueOwnershipRequirement::Owned
            .admit(value, &mut resources, 0)
            .unwrap();
        proof
            .validate_storage(&storage, &resources.work(), &|value| *value, 0)
            .unwrap();
        assert!(matches!(
            RuntimeValueOwnershipRequirement::Owned.admit(
                RuntimeValue::Unit.view(),
                &mut resources,
                0
            ),
            Err(RuntimeValueOwnershipAdmissionError::Resource(_))
        ));

        let mut limited = ValueResourceAdmission::live(RuntimeSchemaLimits {
            max_validation_work: 3,
            ..RuntimeSchemaLimits::engine_default()
        });
        RuntimeValueOwnershipRequirement::Owned
            .admit(value, &mut limited, 0)
            .unwrap();
        assert!(matches!(
            proof.validate_storage(&storage, &limited.work(), &|value| *value, 0),
            Err(crate::entry::RuntimeSchemaError::ValidationWork { .. })
        ));
    }
}

fn partially_moved_record_program() -> AwbcProgram {
    let mut program = mixed_copy_program();
    let fields = [0, 1].map(|ordinal| {
        crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap()
    });
    program
        .strings
        .extend(["kept".to_owned(), "moved".to_owned()]);
    program.runtime_types[3] = runtime_type(
        74,
        AwbcRuntimeTypeShape::Record {
            public_id: None,
            fields: vec![
                AwbcRecordField {
                    field: fields[0],
                    name: Some(AwbcStringId(1)),
                    ty: AwbcTypeId(2),
                },
                AwbcRecordField {
                    field: fields[1],
                    name: Some(AwbcStringId(2)),
                    ty: AwbcTypeId(1),
                },
            ],
        },
    );
    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(1),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    program.instructions.truncate(1);
    program.instructions.extend([
        AwbcInstruction::MakeRecord {
            dst: AwbcRegisterId(2),
            ty: AwbcTypeId(3),
            fields: vec![AwbcRegisterId(1), AwbcRegisterId(0)],
        },
        AwbcInstruction::ReadPlace {
            dst: AwbcRegisterId(4),
            root: AwbcRegisterId(2),
            fields: vec![fields[1]],
            mode: AwbcPlaceReadMode::Move,
        },
        AwbcInstruction::ReadPlace {
            dst: AwbcRegisterId(3),
            root: AwbcRegisterId(2),
            fields: vec![fields[0]],
            mode: AwbcPlaceReadMode::Copy,
        },
    ]);
    program.blocks[0].instructions = AwbcTableRange::new(0, 4);
    program.canonicalize_string_table();
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    program
}

#[test]
fn restore_checks_surviving_copy_children_in_a_partially_moved_record() {
    let program = Arc::new(partially_moved_record_program());
    let owner = RuntimeProgramOwner::Awbc(Arc::clone(&program));
    let mut fiber = mixed_copy_fiber(&program, "partial.original.sibling");
    super::super::vm::step(
        &program,
        &mut fiber,
        super::super::vm::VmStepOptions {
            max_instructions: 3,
        },
    )
    .unwrap();
    assert_eq!(fiber.cursor.instruction_offset, 3);
    assert!(fiber.frames[0].registers[2].record_parts().unwrap().1[1].is_vacant());
    let saved = AwbcFiberStateSnapshot::from_live(&fiber, &owner).unwrap();
    let mut restored = saved.clone().into_live_for_program(&owner).unwrap();
    let mut distinct = mixed_copy_fiber(&program, "partial.distinct.child");
    let forged = RuntimeValue::Seq(RuntimeSeq::values(vec![
        distinct.frames[0].registers[0].take().unwrap(),
    ]));
    let forged = AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&forged, &owner).unwrap();
    let mut tampered = saved.clone();
    let mut replaced = 0;
    tampered.frames[0].registers[2] = std::mem::take(&mut tampered.frames[0].registers[2])
        .try_map(&mut |_| {
            replaced += 1;
            Ok::<_, std::convert::Infallible>(forged.clone())
        })
        .unwrap();
    assert_eq!(replaced, 1);
    let error = restored
        .replace_from_snapshot(tampered, &owner)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("original Copy-proof value shape"),
        "{error:?}"
    );
    assert_eq!(
        AwbcFiberStateSnapshot::from_live(&restored, &owner).unwrap(),
        saved
    );
    super::super::vm::step(
        &program,
        &mut restored,
        super::super::vm::VmStepOptions {
            max_instructions: 2,
        },
    )
    .unwrap();
    assert_eq!(restored.status, FiberStatus::Returned);
}
