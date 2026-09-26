use super::*;
use crate::awbc::schema::{AwbcEffectSet, AwbcSignature};
use crate::pattern::RuntimeSemanticTypeId;

fn test_type(marker: u8, shape: AwbcRuntimeTypeShape) -> AwbcRuntimeType {
    AwbcRuntimeType::new(RuntimeSemanticTypeId::from_bytes([marker; 32]), shape)
}

#[test]
fn capacity_intrinsic_verifier_rejects_forged_family_arity_and_hint_type() {
    let mut program = AwbcProgram::default();
    program.runtime_types = vec![
        test_type(1, AwbcRuntimeTypeShape::UInt(AwbcUnsignedIntKind::USize)),
        test_type(2, AwbcRuntimeTypeShape::Unit),
        test_type(3, AwbcRuntimeTypeShape::String),
        test_type(4, AwbcRuntimeTypeShape::Bytes),
        test_type(
            5,
            AwbcRuntimeTypeShape::Sequence {
                kind: RuntimePlanSequenceKind::Vec,
                item: AwbcTypeId(0),
            },
        ),
    ];
    program.effect_sets = vec![AwbcEffectSet {
        effects: Vec::new(),
    }];
    program.signatures = vec![AwbcSignature {
        params: vec![AwbcTypeId(0)],
        result: Some(AwbcTypeId(3)),
        effects: AwbcEffectSetId(0),
    }];
    assert!(
        verify_capacity_intrinsic_signature(
            &program,
            RuntimeIntrinsic::BytesWithCapacity,
            AwbcSignatureId(0),
            "test",
        )
        .is_ok()
    );
    assert!(
        verify_capacity_intrinsic_signature(
            &program,
            RuntimeIntrinsic::VecWithCapacity,
            AwbcSignatureId(0),
            "test",
        )
        .is_err()
    );

    program.signatures[0].params = vec![AwbcTypeId(3), AwbcTypeId(0)];
    program.signatures[0].result = Some(AwbcTypeId(1));
    assert!(
        verify_capacity_intrinsic_signature(
            &program,
            RuntimeIntrinsic::BytesReserve,
            AwbcSignatureId(0),
            "test",
        )
        .is_ok()
    );
    program.signatures[0].params[1] = AwbcTypeId(3);
    assert!(
        verify_capacity_intrinsic_signature(
            &program,
            RuntimeIntrinsic::BytesReserve,
            AwbcSignatureId(0),
            "test",
        )
        .is_err()
    );
    program.signatures[0].params[1] = AwbcTypeId(0);
    program.signatures[0].params[0] = AwbcTypeId(3);
    assert!(
        verify_capacity_intrinsic_signature(
            &program,
            RuntimeIntrinsic::VecReserve,
            AwbcSignatureId(0),
            "test",
        )
        .is_err()
    );
    program.signatures[0].params.pop();
    assert!(
        verify_capacity_intrinsic_signature(
            &program,
            RuntimeIntrinsic::BytesReserve,
            AwbcSignatureId(0),
            "test",
        )
        .is_err()
    );
}
