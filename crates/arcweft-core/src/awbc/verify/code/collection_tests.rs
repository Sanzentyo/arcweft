use super::*;
use crate::awbc::schema::{AwbcEffectSet, AwbcSignature, AwbcSignedIntKind, AwbcStringId};
use crate::pattern::RuntimeSemanticTypeId;

fn collection_program() -> AwbcProgram {
    AwbcProgram {
        strings: vec!["fs.read".into()],
        runtime_types: vec![
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([1; 32]),
                AwbcRuntimeTypeShape::Int(AwbcSignedIntKind::I32),
            ),
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([2; 32]),
                AwbcRuntimeTypeShape::Int(AwbcSignedIntKind::I64),
            ),
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([3; 32]),
                AwbcRuntimeTypeShape::UInt(AwbcUnsignedIntKind::USize),
            ),
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([4; 32]),
                AwbcRuntimeTypeShape::Bool,
            ),
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([5; 32]),
                AwbcRuntimeTypeShape::Sequence {
                    kind: RuntimePlanSequenceKind::Vec,
                    item: AwbcTypeId(0),
                },
            ),
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([6; 32]),
                AwbcRuntimeTypeShape::Sequence {
                    kind: RuntimePlanSequenceKind::Vec,
                    item: AwbcTypeId(3),
                },
            ),
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([7; 32]),
                AwbcRuntimeTypeShape::Array {
                    item: AwbcTypeId(0),
                    length: 3.into(),
                },
            ),
        ],
        effect_sets: vec![
            AwbcEffectSet {
                effects: Vec::new(),
            },
            AwbcEffectSet {
                effects: vec![AwbcStringId(0)],
            },
        ],
        signatures: vec![AwbcSignature {
            params: vec![AwbcTypeId(4)],
            result: Some(AwbcTypeId(1)),
            effects: AwbcEffectSetId(0),
        }],
        ..AwbcProgram::default()
    }
}

fn verify(program: &AwbcProgram, intrinsic: RuntimeIntrinsic) -> Result<(), AwbcVerifyError> {
    verify_collection_intrinsic_signature(
        program,
        intrinsic,
        AwbcSignatureId(0),
        "collection signature",
    )
}

fn assert_rejected(program: &AwbcProgram, intrinsic: RuntimeIntrinsic, expected: &str) {
    assert_eq!(
        verify(program, intrinsic),
        Err(AwbcVerifyError::InvalidInvariant {
            at: "collection signature".into(),
            message: expected.into(),
        })
    );
}

#[test]
fn collection_intrinsic_verifier_accepts_exact_sequence_and_array_signatures() {
    let mut program = collection_program();
    for receiver in [AwbcTypeId(4), AwbcTypeId(6)] {
        program.signatures[0].params = vec![receiver];
        program.signatures[0].result = Some(AwbcTypeId(1));
        assert_eq!(verify(&program, RuntimeIntrinsic::CoreSeqSum), Ok(()));
        program.signatures[0].result = Some(AwbcTypeId(2));
        assert_eq!(verify(&program, RuntimeIntrinsic::CoreSeqLen), Ok(()));
    }
}

#[test]
fn collection_intrinsic_verifier_rejects_forged_receiver_and_result_at_the_signature() {
    let mut program = collection_program();
    program.signatures[0].params = vec![AwbcTypeId(0)];
    assert_rejected(
        &program,
        RuntimeIntrinsic::CoreSeqSum,
        "collection intrinsic requires a sequence receiver",
    );
    program.signatures[0].params = vec![AwbcTypeId(5)];
    assert_rejected(
        &program,
        RuntimeIntrinsic::CoreSeqSum,
        "collection intrinsic has an invalid typed signature",
    );
    program.signatures[0].params = vec![AwbcTypeId(4)];
    program.signatures[0].result = Some(AwbcTypeId(0));
    assert_rejected(
        &program,
        RuntimeIntrinsic::CoreSeqSum,
        "collection intrinsic has an invalid typed signature",
    );
    program.signatures[0].result = Some(AwbcTypeId(1));
    assert_rejected(
        &program,
        RuntimeIntrinsic::CoreSeqLen,
        "collection intrinsic has an invalid typed signature",
    );
    program.signatures[0].result = None;
    assert_rejected(
        &program,
        RuntimeIntrinsic::CoreSeqSum,
        "collection intrinsic requires a result",
    );
}

#[test]
fn collection_intrinsic_verifier_rejects_wrong_arity_before_receiver_admission() {
    let mut program = collection_program();
    for params in [Vec::new(), vec![AwbcTypeId(4), AwbcTypeId(4)]] {
        program.signatures[0].params = params;
        assert_rejected(
            &program,
            RuntimeIntrinsic::CoreSeqSum,
            "collection intrinsic requires exactly one receiver",
        );
    }
}

#[test]
fn collection_intrinsic_verifier_rejects_an_actual_nonempty_effect_row() {
    let mut program = collection_program();
    assert_eq!(program.effect_sets[1].effects, [AwbcStringId(0)]);
    assert_eq!(program.strings[0], "fs.read");
    program.signatures[0].effects = AwbcEffectSetId(1);
    assert_rejected(
        &program,
        RuntimeIntrinsic::CoreSeqSum,
        "collection intrinsic has an invalid typed signature",
    );
    program.signatures[0].result = Some(AwbcTypeId(2));
    assert_rejected(
        &program,
        RuntimeIntrinsic::CoreSeqLen,
        "collection intrinsic has an invalid typed signature",
    );
}
