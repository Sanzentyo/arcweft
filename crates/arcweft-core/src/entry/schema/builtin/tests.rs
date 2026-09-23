use super::super::canonical_schema_bytes;
use super::*;
use crate::entry::{RuntimeSchemaError, RuntimeSchemaLimits};
use crate::pattern::RuntimeVariantIdentity;
use crate::value::RuntimeValue;

fn owners() -> [RuntimeBuiltinVariantIdentity; 7] {
    use RuntimeBuiltinVariantIdentity as Owner;
    [
        Owner::Option,
        Owner::Result,
        Owner::AgentResourceBody,
        Owner::AgentBinaryEncoding,
        Owner::CaptureFormat,
        Owner::CaptureKind,
        Owner::PointerButton,
    ]
}

#[test]
fn builtin_schema_and_value_constructors_agree_on_every_case_and_wrapper() {
    for owner in owners() {
        // These are structural schema fixtures. Standard-library semantic
        // payload signatures are supplied and correlated by their producers.
        let schema =
            RuntimeTypeSchema::builtin(owner, vec![RuntimeTypeSchema::Bool; owner.payload_count()])
                .unwrap();
        let RuntimeTypeSchema::Builtin(builtin) = &schema else {
            unreachable!();
        };
        let limits = RuntimeSchemaLimits::engine_default();
        for (ordinal, case) in owner.cases().iter().enumerate() {
            let value = RuntimeValue::try_builtin_variant(
                case.identity(),
                case.has_payload().then_some(RuntimeValue::Bool(true)),
            )
            .unwrap();
            assert_eq!(
                schema.validate_value(&value, limits).unwrap(),
                value.try_digest(limits.platform_encoded_bytes()).unwrap()
            );
            assert_eq!(builtin.case(ordinal).unwrap().0, *case);

            let RuntimeValue::Variant {
                owner,
                ordinal,
                name,
                ..
            } = value
            else {
                unreachable!();
            };
            for payload in [
                Some(RuntimeValue::Bool(true)),
                Some(RuntimeValue::Tuple(vec![])),
                Some(RuntimeValue::Tuple(vec![RuntimeValue::Bool(true); 2])),
                Some(RuntimeValue::Tuple(vec![RuntimeValue::Unit])),
            ] {
                let malformed = RuntimeValue::Variant {
                    owner: owner.clone(),
                    ordinal,
                    name: name.clone(),
                    payload: payload.map(Box::new),
                };
                assert!(schema.validate_value(&malformed, limits).is_err());
            }
            if case.has_payload() {
                let missing = RuntimeValue::Variant {
                    owner,
                    ordinal,
                    name,
                    payload: None,
                };
                assert!(schema.validate_value(&missing, limits).is_err());
            }
        }
        assert!(builtin.case(owner.cases().len()).is_none());
        assert!(builtin.case(usize::MAX).is_none());
        assert_eq!(
            serde_json::from_slice::<RuntimeTypeSchema>(&serde_json::to_vec(&schema).unwrap())
                .unwrap(),
            schema
        );
    }
}

#[test]
fn builtin_schema_rejects_wrong_owner_case_and_name_before_payload_admission() {
    let schema = RuntimeTypeSchema::option(RuntimeTypeSchema::Bool);
    let limits = RuntimeSchemaLimits::engine_default();
    let other = RuntimeValue::result_ok(RuntimeValue::Bool(true));
    assert!(matches!(
        schema.validate_value(&other, limits),
        Err(RuntimeSchemaError::BuiltinVariantOwner {
            expected: RuntimeBuiltinVariantIdentity::Option,
            actual: RuntimeVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Result),
            ..
        })
    ));
    for (ordinal, name) in [(0, "None"), (1, "Some"), (u32::MAX, "Some")] {
        let value = RuntimeValue::Variant {
            owner: RuntimeVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Option),
            ordinal,
            name: name.to_owned(),
            payload: Some(Box::new(RuntimeValue::Tuple(vec![RuntimeValue::Bool(
                true,
            )]))),
        };
        assert!(matches!(
            schema.validate_value(&value, limits),
            Err(RuntimeSchemaError::UnknownVariant { .. })
        ));
    }
}

#[test]
fn builtin_payloads_keep_case_order_and_count_the_physical_tuple() {
    let schema = RuntimeTypeSchema::result(RuntimeTypeSchema::Bool, RuntimeTypeSchema::String);
    let limits = RuntimeSchemaLimits {
        max_validation_work: 3,
        max_nodes: 3,
        max_depth: 2,
        ..RuntimeSchemaLimits::engine_default()
    };
    for value in [
        RuntimeValue::result_ok(RuntimeValue::Bool(true)),
        RuntimeValue::result_err(RuntimeValue::String("failure".to_owned())),
    ] {
        assert!(schema.validate_value(&value, limits).is_ok());
        assert!(
            schema
                .validate_value(
                    &value,
                    RuntimeSchemaLimits {
                        max_nodes: 2,
                        ..limits
                    }
                )
                .is_err()
        );
        assert!(
            schema
                .validate_value(
                    &value,
                    RuntimeSchemaLimits {
                        max_validation_work: 2,
                        ..limits
                    }
                )
                .is_err()
        );
        assert!(
            schema
                .validate_value(
                    &value,
                    RuntimeSchemaLimits {
                        max_depth: 1,
                        ..limits
                    }
                )
                .is_err()
        );
    }
    for value in [
        RuntimeValue::result_ok(RuntimeValue::String("failure".to_owned())),
        RuntimeValue::result_err(RuntimeValue::Bool(true)),
    ] {
        assert!(schema.validate_value(&value, limits).is_err());
    }
}

#[test]
fn builtin_schema_count_is_checked_by_construction_and_deserialization() {
    for owner in owners() {
        let invalid = vec![RuntimeTypeSchema::Bool; owner.payload_count() + 1];
        assert_eq!(
            RuntimeBuiltinSchema::try_new(owner, invalid.clone()).unwrap_err(),
            RuntimeBuiltinSchemaError {
                owner,
                expected: owner.payload_count(),
                actual: invalid.len(),
            }
        );
        let invalid = serde_json::json!({ "owner": owner, "payloads": invalid });
        assert!(serde_json::from_value::<RuntimeBuiltinSchema>(invalid).is_err());
    }
    let mut deep = RuntimeTypeSchema::Bool;
    for _ in 0..20_000 {
        deep = RuntimeTypeSchema::Seq(Box::new(deep));
    }
    assert!(
        RuntimeBuiltinSchema::try_new(RuntimeBuiltinVariantIdentity::CaptureFormat, vec![deep])
            .is_err()
    );
}

#[test]
fn builtin_layout_contains_canonical_unit_cases_and_ordered_payload_schemas() {
    let mut expected = b"arcweft.nominal-schema\0\x01".to_vec();
    expected.extend_from_slice(&[
        20, 1, 2, 0, 2, b'O', b'k', 1, 2, 1, 3, b'E', b'r', b'r', 1, 17,
    ]);
    let schema = RuntimeTypeSchema::result(RuntimeTypeSchema::Bool, RuntimeTypeSchema::String);
    assert_eq!(
        canonical_schema_bytes(&schema, expected.len()).unwrap(),
        expected
    );
    assert_eq!(
        schema.try_layout_hash().unwrap().as_bytes(),
        blake3::hash(&expected).as_bytes()
    );
    assert!(canonical_schema_bytes(&schema, expected.len() - 1).is_err());
    let swapped = RuntimeTypeSchema::result(RuntimeTypeSchema::String, RuntimeTypeSchema::Bool);
    assert_ne!(
        schema.try_layout_hash().unwrap(),
        swapped.try_layout_hash().unwrap()
    );

    let schema =
        RuntimeTypeSchema::builtin(RuntimeBuiltinVariantIdentity::CaptureFormat, vec![]).unwrap();
    let mut expected = b"arcweft.nominal-schema\0\x01".to_vec();
    expected.extend_from_slice(&[
        20, 4, 2, 0, 3, b'p', b'n', b'g', 0, 1, 8, b'r', b'a', b'w', b'_', b'r', b'g', b'b', b'a',
        0,
    ]);
    assert_eq!(
        canonical_schema_bytes(&schema, expected.len()).unwrap(),
        expected
    );
}
