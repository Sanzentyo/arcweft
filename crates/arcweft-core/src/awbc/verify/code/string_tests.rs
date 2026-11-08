use super::*;
use crate::awbc::schema::{AwbcEffectSet, AwbcSignature, AwbcStringId};
use crate::pattern::RuntimeSemanticTypeId;
use crate::value::{RuntimeStringOperation, RuntimeStringResultKind};

fn fixture(operation: RuntimeStringOperation) -> AwbcProgram {
    AwbcProgram {
        runtime_types: [
            AwbcRuntimeTypeShape::String,
            AwbcRuntimeTypeShape::UInt(AwbcUnsignedIntKind::USize),
            AwbcRuntimeTypeShape::UInt(AwbcUnsignedIntKind::U64),
            AwbcRuntimeTypeShape::Bool,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, shape)| {
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([u8::try_from(index + 1).unwrap(); 32]),
                shape,
            )
        })
        .collect(),
        strings: vec!["test.effect".into()],
        effect_sets: vec![
            AwbcEffectSet {
                effects: Vec::new(),
            },
            AwbcEffectSet {
                effects: vec![AwbcStringId(0)],
            },
        ],
        signatures: vec![AwbcSignature {
            params: vec![AwbcTypeId(0)],
            result: Some(AwbcTypeId(match operation.result_kind() {
                RuntimeStringResultKind::String => 0,
                RuntimeStringResultKind::USize => 1,
            })),
            effects: AwbcEffectSetId(0),
        }],
        ..AwbcProgram::default()
    }
}

#[test]
fn string_intrinsic_signatures_require_exact_receiver_result_and_purity() {
    for operation in RuntimeStringOperation::ALL {
        let check = |program: &AwbcProgram| {
            verify_string_intrinsic_signature(
                program,
                RuntimeIntrinsic::String(operation),
                AwbcSignatureId(0),
                "String ABI",
            )
        };
        assert_eq!(check(&fixture(operation)), Ok(()));
        for wrong_result in [None, Some(AwbcTypeId(2)), Some(AwbcTypeId(3))] {
            let mut program = fixture(operation);
            program.signatures[0].result = wrong_result;
            assert!(check(&program).is_err(), "{operation:?} {wrong_result:?}");
        }
        let mut program = fixture(operation);
        program.signatures[0].params = vec![AwbcTypeId(3)];
        assert!(check(&program).is_err());
        let mut program = fixture(operation);
        program.signatures[0].effects = AwbcEffectSetId(1);
        assert!(check(&program).is_err());
        for params in [Vec::new(), vec![AwbcTypeId(0), AwbcTypeId(0)]] {
            let mut program = fixture(operation);
            program.signatures[0].params = params;
            assert!(check(&program).is_err());
        }
    }
}
