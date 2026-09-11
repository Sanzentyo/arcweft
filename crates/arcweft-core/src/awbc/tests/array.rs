use super::*;
use crate::plan::{RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed};
use crate::value::RuntimeSeq;

#[test]
fn nested_array_lengths_are_checked_after_awbc_reification() {
    let mut program = minimal_program();
    program.runtime_types = vec![
        runtime_type(1, AwbcRuntimeTypeShape::Bool),
        runtime_type(
            2,
            AwbcRuntimeTypeShape::Array {
                item: AwbcTypeId(0),
                length: 2,
            },
        ),
        runtime_type(3, AwbcRuntimeTypeShape::Sequence(AwbcTypeId(1))),
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
            .admit_semantic_batch(
                [
                    RuntimePlanTypeSeed::new(item_identity, RuntimePlanTypeProjection::Bool),
                    RuntimePlanTypeSeed::new(
                        array_identity,
                        RuntimePlanTypeProjection::Array {
                            item: item_identity,
                            length,
                        },
                    ),
                ],
                [],
                [],
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
                    length,
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
