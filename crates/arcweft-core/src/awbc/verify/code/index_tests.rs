use super::*;
use crate::awbc::schema::{AwbcEffectSet, AwbcSignature};
use crate::pattern::RuntimeSemanticTypeId;

fn test_type(marker: u8, shape: AwbcRuntimeTypeShape) -> AwbcRuntimeType {
    AwbcRuntimeType::new(RuntimeSemanticTypeId::from_bytes([marker; 32]), shape)
}

#[test]
fn core_index_verifier_rejects_an_affine_item_signature() {
    let mut program = AwbcProgram::default();
    program.runtime_types = vec![
        test_type(1, AwbcRuntimeTypeShape::Unit),
        test_type(2, AwbcRuntimeTypeShape::Need(AwbcTypeId(0))),
        test_type(
            3,
            AwbcRuntimeTypeShape::Sequence {
                kind: RuntimePlanSequenceKind::Vec,
                item: AwbcTypeId(0),
            },
        ),
        test_type(
            4,
            AwbcRuntimeTypeShape::Sequence {
                kind: RuntimePlanSequenceKind::Vec,
                item: AwbcTypeId(1),
            },
        ),
        test_type(5, AwbcRuntimeTypeShape::UInt(AwbcUnsignedIntKind::USize)),
        test_type(6, AwbcRuntimeTypeShape::String),
        test_type(7, AwbcRuntimeTypeShape::Char),
    ];
    program.effect_sets = vec![AwbcEffectSet {
        effects: Vec::new(),
    }];
    program.signatures = vec![AwbcSignature {
        params: vec![AwbcTypeId(2), AwbcTypeId(4)],
        result: Some(AwbcTypeId(0)),
        effects: AwbcEffectSetId(0),
    }];

    assert!(
        verify_index_intrinsic_signature(
            &program,
            RuntimeIntrinsic::CoreIndex,
            AwbcSignatureId(0),
            "test",
        )
        .is_ok()
    );

    program.signatures[0].params[0] = AwbcTypeId(3);
    program.signatures[0].result = Some(AwbcTypeId(1));
    assert!(
        verify_index_intrinsic_signature(
            &program,
            RuntimeIntrinsic::CoreIndex,
            AwbcSignatureId(0),
            "test",
        )
        .is_err()
    );

    program.signatures[0].params = vec![AwbcTypeId(5), AwbcTypeId(4)];
    program.signatures[0].result = Some(AwbcTypeId(6));
    assert!(
        verify_index_intrinsic_signature(
            &program,
            RuntimeIntrinsic::CoreIndex,
            AwbcSignatureId(0),
            "test",
        )
        .is_ok()
    );
}
