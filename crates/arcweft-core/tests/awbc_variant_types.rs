use arcweft_core::awbc::{
    codec::AwbcDecodeBudget,
    schema::{
        AwbcProgram, AwbcRuntimeType, AwbcRuntimeTypeShape as Type, AwbcStringId, AwbcTypeId,
        AwbcVariantCase, AwbcVariantIdentity,
    },
    verify::{AwbcVerifyBudget, AwbcVerifyContext},
};
use arcweft_core::pattern::{RuntimeBuiltinVariantIdentity, RuntimeSemanticTypeId};
use arcweft_core::value::RuntimeValue;

fn builtin_program(owner: RuntimeBuiltinVariantIdentity, payload: Type) -> AwbcProgram {
    let mut program = AwbcProgram {
        strings: owner
            .cases()
            .iter()
            .map(|case| case.name().to_owned())
            .collect(),
        runtime_types: vec![
            AwbcRuntimeType::new(RuntimeSemanticTypeId::from_bytes([1; 32]), Type::Bool),
            AwbcRuntimeType::new(RuntimeSemanticTypeId::from_bytes([2; 32]), payload),
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([3; 32]),
                Type::Variant {
                    owner: AwbcVariantIdentity::Builtin(owner),
                    arguments: vec![],
                    cases: owner
                        .cases()
                        .iter()
                        .enumerate()
                        .map(|(ordinal, case)| AwbcVariantCase {
                            name: AwbcStringId(u32::try_from(ordinal).unwrap()),
                            payload: case.has_payload().then_some(AwbcTypeId(1)),
                        })
                        .collect(),
                },
            ),
        ],
        ..AwbcProgram::default()
    };
    program.canonicalize_string_table();
    program
}

fn verifies(program: &AwbcProgram) -> bool {
    program
        .verify(
            AwbcVerifyBudget::default(),
            AwbcVerifyContext {
                require_entrypoint: false,
                ..AwbcVerifyContext::default()
            },
        )
        .is_ok()
}

#[test]
fn every_builtin_case_round_trips_and_uses_its_registered_payload_container() {
    for owner in (0..=u8::MAX).filter_map(RuntimeBuiltinVariantIdentity::from_wire_tag) {
        let program = builtin_program(owner, Type::Tuple(vec![AwbcTypeId(0)]));
        let bytes = program.encode_canonical().unwrap();
        let decoded = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
        assert!(verifies(&decoded), "{owner:?}");
        let predicate = decoded.checked_type(AwbcTypeId(2)).unwrap();
        for case in owner.cases() {
            let value = RuntimeValue::try_builtin_variant(
                case.identity(),
                case.has_payload().then_some(RuntimeValue::Bool(true)),
            )
            .unwrap();
            assert!(predicate.accepts_value(&value), "{owner:?} {}", case.name());
        }
    }
}

#[test]
fn payload_bearing_builtins_reject_bare_empty_multi_item_and_dangling_payload_types() {
    for owner in (0..=u8::MAX)
        .filter_map(RuntimeBuiltinVariantIdentity::from_wire_tag)
        .filter(|owner| owner.payload_count() > 0)
    {
        for payload in [
            Type::Bool,
            Type::Tuple(vec![]),
            Type::Tuple(vec![AwbcTypeId(0), AwbcTypeId(0)]),
            Type::Tuple(vec![AwbcTypeId(99)]),
        ] {
            let program = builtin_program(owner, payload.clone());
            assert!(!verifies(&program), "{owner:?}: {payload:?}");
            assert!(
                program.checked_type(AwbcTypeId(2)).is_err(),
                "{owner:?}: {payload:?}"
            );
        }
    }
}

#[test]
fn case_names_and_references_are_validated_by_both_consumers() {
    let program = builtin_program(
        RuntimeBuiltinVariantIdentity::Option,
        Type::Tuple(vec![AwbcTypeId(0)]),
    );
    let Type::Variant { cases, .. } = program.runtime_types[2].shape().clone() else {
        unreachable!()
    };
    for invalid in 0..4 {
        let mut candidate = program.clone();
        let mut cases = cases.clone();
        match invalid {
            0 => {
                cases[1].name = cases[0].name;
            }
            1 => {
                cases[0].name = AwbcStringId(u32::try_from(candidate.strings.len()).unwrap());
                candidate.strings.push(String::new());
            }
            2 => {
                cases[0].name = AwbcStringId(99);
            }
            3 => {
                cases[0].payload = Some(AwbcTypeId(99));
            }
            _ => unreachable!(),
        }
        candidate.runtime_types[2] = AwbcRuntimeType::new(
            RuntimeSemanticTypeId::from_bytes([3; 32]),
            Type::Variant {
                owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Option),
                arguments: vec![],
                cases,
            },
        );
        candidate.canonicalize_string_table();
        assert!(!verifies(&candidate));
        assert!(candidate.checked_type(AwbcTypeId(2)).is_err());
    }
}
