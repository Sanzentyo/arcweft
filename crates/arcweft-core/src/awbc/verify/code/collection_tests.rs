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

#[test]
fn traversal_destructuring_copy_proofs_use_field_types_and_actual_callback_custody() {
    use crate::awbc::schema::AwbcVariantCase;
    use crate::pattern::RuntimeBuiltinVariantIdentity;
    let mut program = collection_program();
    program.strings.extend(["Ok".into(), "Err".into()]);
    for (marker, shape) in [
        (8, AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(0)])),
        (
            9,
            AwbcRuntimeTypeShape::Variant {
                owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Result),
                arguments: vec![],
                cases: vec![
                    AwbcVariantCase {
                        name: AwbcStringId(1),
                        payload: Some(AwbcTypeId(7)),
                    },
                    AwbcVariantCase {
                        name: AwbcStringId(2),
                        payload: Some(AwbcTypeId(7)),
                    },
                ],
            },
        ),
        (10, AwbcRuntimeTypeShape::Need(AwbcTypeId(8))),
        (
            11,
            AwbcRuntimeTypeShape::Function {
                contract: crate::plan::RuntimeFunctionTypeContract::monomorphic(
                    crate::effect_row::EffectSet::default(),
                ),
                parameters: vec![AwbcTypeId(0)],
                result: AwbcTypeId(9),
            },
        ),
        (
            12,
            AwbcRuntimeTypeShape::Traversal {
                source: AwbcTypeId(4),
                mapping: AwbcTypeId(10),
            },
        ),
    ] {
        program.runtime_types.push(AwbcRuntimeType::new(
            RuntimeSemanticTypeId::from_bytes([marker; 32]),
            shape,
        ));
    }
    program.patterns = vec![
        AwbcPattern::Bind {
            target: AwbcRegisterId(0),
            mutable: false,
            expected: Some(AwbcTypeId(4)),
        },
        AwbcPattern::Bind {
            target: AwbcRegisterId(1),
            mutable: false,
            expected: Some(AwbcTypeId(10)),
        },
        AwbcPattern::Tuple(vec![AwbcPatternId(0), AwbcPatternId(1)]),
    ];
    let verifier = Verifier {
        program: &program,
        budget: crate::awbc::verify::AwbcVerifyBudget::default(),
        context: crate::awbc::verify::AwbcVerifyContext::default(),
        block_owner: vec![],
    };
    for (proof, expected) in [
        (CopyProof::Affine, [CopyProof::Copyable, CopyProof::Affine]),
        (
            CopyProof::Tuple(vec![CopyProof::Copyable, CopyProof::Function(vec![])]),
            [CopyProof::Copyable, CopyProof::Function(vec![])],
        ),
        (
            CopyProof::Tuple(vec![
                CopyProof::Copyable,
                CopyProof::Function(vec![CopyProof::Affine]),
            ]),
            [
                CopyProof::Copyable,
                CopyProof::Function(vec![CopyProof::Affine]),
            ],
        ),
    ] {
        let mut state = FlowState {
            initialized: vec![RuntimePlaceInitialization::Initialized; 2],
            moved_fields: BTreeMap::new(),
            copy_proofs: vec![CopyProof::Affine; 2],
            scopes: vec![],
            format_attempts: vec![],
        };
        assign_pattern_copy_proofs(
            &verifier,
            0,
            0,
            AwbcPatternId(2),
            AwbcTypeId(11),
            &proof,
            &mut state,
            0,
        )
        .unwrap();
        assert_eq!(state.copy_proofs, expected);
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

#[test]
fn ownership_guard_abi_preserves_exact_function_identity_and_purity() {
    let mut program = collection_program();
    for marker in [31, 32] {
        program.runtime_types.push(AwbcRuntimeType::new(
            RuntimeSemanticTypeId::from_bytes([marker; 32]),
            AwbcRuntimeTypeShape::Function {
                contract: crate::plan::RuntimeFunctionTypeContract::monomorphic(
                    crate::effect_row::EffectSet::default(),
                ),
                parameters: vec![AwbcTypeId(0)],
                result: AwbcTypeId(1),
            },
        ));
    }
    program.signatures[0].params = vec![AwbcTypeId(7)];
    program.signatures[0].result = Some(AwbcTypeId(7));
    let check = |program: &AwbcProgram| {
        verify_ownership_intrinsic_signature(
            program,
            RuntimeIntrinsic::CoreRequireUnrestricted,
            AwbcSignatureId(0),
            "ownership ABI",
        )
    };
    assert_eq!(check(&program), Ok(()));
    program.signatures[0].result = Some(AwbcTypeId(8));
    assert!(check(&program).is_err());
    program.signatures[0].result = Some(AwbcTypeId(7));
    program.signatures[0].effects = AwbcEffectSetId(1);
    assert!(check(&program).is_err());
}

#[test]
fn fanout_inputs_abi_preserves_collection_identity_and_exact_limit_width() {
    let mut program = collection_program();
    program.runtime_types.push(AwbcRuntimeType::new(
        RuntimeSemanticTypeId::from_bytes([33; 32]),
        AwbcRuntimeTypeShape::UInt(AwbcUnsignedIntKind::U32),
    ));
    program.runtime_types.push(AwbcRuntimeType::new(
        RuntimeSemanticTypeId::from_bytes([34; 32]),
        AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(4), AwbcTypeId(2), AwbcTypeId(2)]),
    ));
    program.signatures[0].params = vec![AwbcTypeId(4), AwbcTypeId(7)];
    program.signatures[0].result = Some(AwbcTypeId(8));
    assert_eq!(verify(&program, RuntimeIntrinsic::CoreFanoutInputs), Ok(()));
    program.signatures[0].params[0] = AwbcTypeId(5);
    assert!(verify(&program, RuntimeIntrinsic::CoreFanoutInputs).is_err());
    program.signatures[0].params[0] = AwbcTypeId(4);
    program.signatures[0].params[1] = AwbcTypeId(2);
    assert!(verify(&program, RuntimeIntrinsic::CoreFanoutInputs).is_err());
    program.signatures[0].params[1] = AwbcTypeId(7);
    program.signatures[0].effects = AwbcEffectSetId(1);
    assert!(verify(&program, RuntimeIntrinsic::CoreFanoutInputs).is_err());
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
