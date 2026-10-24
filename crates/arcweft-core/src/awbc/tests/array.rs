use super::*;
use crate::plan::{RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed};
use crate::value::RuntimeSeq;

fn array_from_vec_program(length: u64, items: usize) -> AwbcProgram {
    let mut program = minimal_program();
    program.runtime_types = vec![
        runtime_type(1, AwbcRuntimeTypeShape::Bool),
        runtime_type(
            2,
            AwbcRuntimeTypeShape::Sequence {
                kind: crate::plan::RuntimePlanSequenceKind::Vec,
                item: AwbcTypeId(0),
            },
        ),
        runtime_type(
            3,
            AwbcRuntimeTypeShape::Array {
                item: AwbcTypeId(0),
                length: length.into(),
            },
        ),
    ];
    program.signatures[0].result = Some(AwbcTypeId(2));
    program.frame_layouts[0].slots = (0..3)
        .map(|index| AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(index),
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        })
        .collect();
    program.constants = vec![AwbcConstant::Bool(true)];
    program.instructions = vec![
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::MakeSequence {
            dst: AwbcRegisterId(1),
            items: if items == 0 {
                vec![]
            } else {
                vec![AwbcRegisterId(0)]
            },
        },
        AwbcInstruction::ArrayFromVec {
            dst: AwbcRegisterId(2),
            value: AwbcRegisterId(1),
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 3);
    program.blocks[0].terminator = AwbcTerminator::Return {
        value: Some(AwbcRegisterId(2)),
    };
    program
}

#[test]
fn array_from_vec_awbc_codec_and_execution_check_length_before_consuming_storage() {
    for (length, items) in [(0, 0), (1, 1), (2, 1), (0, 1)] {
        let program = array_from_vec_program(length, items);
        let bytes = program.encode_canonical().unwrap();
        let program = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
        assert_eq!(program.encode_canonical().unwrap(), bytes);
        program
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .unwrap();
        let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).unwrap();
        for _ in 0..2 {
            super::super::vm::step(
                &program,
                &mut fiber,
                super::super::vm::VmStepOptions {
                    max_instructions: 1,
                },
            )
            .unwrap();
        }
        let result = super::super::vm::step(
            &program,
            &mut fiber,
            super::super::vm::VmStepOptions {
                max_instructions: 1,
            },
        );
        if length == items as u64 {
            result.unwrap();
            assert_eq!(
                fiber
                    .active_frame()
                    .unwrap()
                    .register(AwbcRegisterId(2))
                    .unwrap(),
                &RuntimeValue::Seq(RuntimeSeq::values(vec![RuntimeValue::Bool(true); items]))
            );
            assert!(
                fiber
                    .active_frame()
                    .unwrap()
                    .register(AwbcRegisterId(1))
                    .is_err()
            );
        } else {
            assert!(matches!(
                result.unwrap().exit,
                super::super::vm::VmExit::Trapped(_)
            ));
            assert!(
                fiber
                    .active_frame()
                    .unwrap()
                    .register(AwbcRegisterId(1))
                    .is_ok()
            );
            assert!(
                fiber
                    .active_frame()
                    .unwrap()
                    .register(AwbcRegisterId(2))
                    .is_err()
            );
        }
    }
}

#[test]
fn array_from_vec_awbc_verifier_rejects_non_vec_source() {
    let mut program = array_from_vec_program(1, 1);
    program.runtime_types[1] = runtime_type(
        2,
        AwbcRuntimeTypeShape::Sequence {
            kind: crate::plan::RuntimePlanSequenceKind::Seq,
            item: AwbcTypeId(0),
        },
    );
    assert!(
        program
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .is_err()
    );
}

#[test]
fn nested_array_lengths_are_checked_after_awbc_reification() {
    let mut program = minimal_program();
    program.runtime_types = vec![
        runtime_type(1, AwbcRuntimeTypeShape::Bool),
        runtime_type(
            2,
            AwbcRuntimeTypeShape::Array {
                item: AwbcTypeId(0),
                length: 2.into(),
            },
        ),
        runtime_type(
            3,
            AwbcRuntimeTypeShape::Sequence {
                kind: crate::plan::RuntimePlanSequenceKind::Vec,
                item: AwbcTypeId(1),
            },
        ),
    ];
    let restored = AwbcProgram::decode_canonical(
        &program.encode_canonical().unwrap(),
        AwbcDecodeBudget::default(),
    )
    .unwrap();
    let checked = restored.checked_type(AwbcTypeId(2)).unwrap();
    let row =
        |length| RuntimeValue::Seq(RuntimeSeq::values(vec![RuntimeValue::Bool(true); length]));
    assert!(checked.accepts_value(&RuntimeValue::Seq(RuntimeSeq::values(vec![row(2), row(2)]))));
    assert!(!checked.accepts_value(&RuntimeValue::Seq(RuntimeSeq::values(vec![row(2), row(1)]))));
    assert!(!checked.accepts_value(&RuntimeValue::Seq(RuntimeSeq::values(vec![row(3)]))));
}

#[test]
fn array_length_survives_plan_projection_awbc_codec_and_checked_value_admission() {
    let item_identity = RuntimeSemanticTypeId::from_bytes([1; 32]);
    let array_identity = RuntimeSemanticTypeId::from_bytes([2; 32]);
    for length in [0, 2, u64::MAX] {
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [
                    RuntimePlanTypeSeed::new(item_identity, RuntimePlanTypeProjection::Bool),
                    RuntimePlanTypeSeed::new(
                        array_identity,
                        RuntimePlanTypeProjection::Array {
                            item: item_identity,
                            length: length.into(),
                        },
                    ),
                ],
                [],
            )
            .unwrap();
        let plan = builder.finish().unwrap();
        let ty = plan.type_table().id_for_semantic(array_identity).unwrap();
        let expected = RuntimeCheckedType::Array {
            item: Box::new(RuntimeCheckedType::Bool),
            length,
        };
        assert_ne!(
            expected.semantic_identity_digest(),
            RuntimeCheckedType::Sequence(Box::new(RuntimeCheckedType::Bool))
                .semantic_identity_digest()
        );
        assert_eq!(plan.checked_type(ty).unwrap(), Some(expected.clone()));

        let mut program = minimal_program();
        program.runtime_types = vec![
            runtime_type(1, AwbcRuntimeTypeShape::Bool),
            runtime_type(
                2,
                AwbcRuntimeTypeShape::Array {
                    item: AwbcTypeId(0),
                    length: length.into(),
                },
            ),
        ];
        let bytes = program.encode_canonical().unwrap();
        let restored = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
        let actual = restored.checked_type(AwbcTypeId(1)).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(restored.encode_canonical().unwrap(), bytes);
        for actual_length in 0..=3 {
            let value = RuntimeValue::Seq(RuntimeSeq::values(vec![
                RuntimeValue::Bool(true);
                actual_length
            ]));
            assert_eq!(
                actual.accepts_value(&value),
                u64::try_from(actual_length).unwrap() == length
            );
        }
        if length == 2 {
            assert!(
                !actual.accepts_value(&RuntimeValue::Seq(RuntimeSeq::values(vec![
                    RuntimeValue::Bool(true),
                    RuntimeValue::Unit
                ])))
            );
        }
    }
}
