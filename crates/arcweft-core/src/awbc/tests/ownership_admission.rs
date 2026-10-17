use super::*;
use crate::awbc::fiber::FiberCursor;
use crate::entry::RuntimeSchemaLimits;
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::value::ownership::RuntimeValueOwnershipAdmissionError;
use crate::value::{AwbcRuntimeValueSnapshot, RuntimeSeq};
use std::sync::Arc;

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
                .unrestricted_registers_at(FiberCursor {
                    function: AwbcFunctionId(0),
                    block: AwbcBlockId(0),
                    instruction_offset: offset,
                })
                .unwrap(),
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
