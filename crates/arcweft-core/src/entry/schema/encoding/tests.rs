use super::super::{
    RuntimeBytesFormat, RuntimeEnumRepr, RuntimeEnumTagStyle, RuntimeSchemaError,
    RuntimeSchemaField, RuntimeSchemaVariant, RuntimeTypeSchema, canonical_schema_bytes,
    canonical_schema_layout_hash,
};

fn document(body: &[u8]) -> Vec<u8> {
    let mut bytes = b"arcweft.nominal-schema\0".to_vec();
    bytes.push(1);
    bytes.extend_from_slice(body);
    bytes
}

#[test]
fn scalar_schema_tags_preserve_the_version_one_transcript() {
    let scalars = [
        RuntimeTypeSchema::Unit,
        RuntimeTypeSchema::Bool,
        RuntimeTypeSchema::I8,
        RuntimeTypeSchema::I16,
        RuntimeTypeSchema::I32,
        RuntimeTypeSchema::I64,
        RuntimeTypeSchema::I128,
        RuntimeTypeSchema::ISize,
        RuntimeTypeSchema::U8,
        RuntimeTypeSchema::U16,
        RuntimeTypeSchema::U32,
        RuntimeTypeSchema::U64,
        RuntimeTypeSchema::U128,
        RuntimeTypeSchema::USize,
        RuntimeTypeSchema::F32,
        RuntimeTypeSchema::F64,
        RuntimeTypeSchema::String,
        RuntimeTypeSchema::Char,
    ];
    for (ordinal, schema) in scalars.iter().enumerate() {
        let tag = u8::try_from(ordinal + 1).unwrap();
        let expected = document(&[tag]);
        assert_eq!(
            canonical_schema_bytes(schema, expected.len()).unwrap(),
            expected
        );
        assert_eq!(
            schema.try_layout_hash().unwrap().as_bytes(),
            blake3::hash(&expected).as_bytes()
        );
    }
}

#[test]
fn nested_field_and_variant_metadata_keep_their_exact_wire_positions() {
    let schema = RuntimeTypeSchema::Record {
        name: "R".to_owned(),
        deny_unknown_fields: true,
        fields: vec![
            RuntimeSchemaField {
                rust_name: "a".to_owned(),
                wire_name: "z".to_owned(),
                has_default: true,
                skip: false,
                bytes_format: None,
                schema: RuntimeTypeSchema::Enum {
                    name: "E".to_owned(),
                    tag: RuntimeEnumTagStyle::Internal {
                        tag: "t".to_owned(),
                    },
                    repr: Some(RuntimeEnumRepr::I8),
                    variants: vec![
                        RuntimeSchemaVariant {
                            rust_name: "U".to_owned(),
                            wire_name: "u".to_owned(),
                            payload: None,
                            discriminant: None,
                        },
                        RuntimeSchemaVariant {
                            rust_name: "V".to_owned(),
                            wire_name: "v".to_owned(),
                            payload: Some(RuntimeTypeSchema::Map {
                                key: Box::new(RuntimeTypeSchema::String),
                                value: Box::new(RuntimeTypeSchema::Option(Box::new(
                                    RuntimeTypeSchema::I64,
                                ))),
                            }),
                            discriminant: Some(-1),
                        },
                    ],
                },
            },
            RuntimeSchemaField {
                rust_name: "b".to_owned(),
                wire_name: "a".to_owned(),
                schema: RuntimeTypeSchema::Bytes {
                    format: RuntimeBytesFormat::Hex,
                },
                has_default: false,
                skip: true,
                bytes_format: Some(RuntimeBytesFormat::Array),
            },
        ],
    };
    let mut expected = document(&[
        23, 1, b'R', 1, 2, 1, b'a', 1, b'z', 24, 1, b'E', 2, 1, b't', 1, 1, 2, 1, b'U', 1, b'u', 0,
        0, 1, b'V', 1, b'v', 1, 22, 17, 20, 6, 1,
    ]);
    expected.extend_from_slice(&[0xff; 16]);
    expected.extend_from_slice(&[1, 0, 0, 1, b'b', 1, b'a', 19, 3, 0, 1, 1, 4]);
    assert_eq!(
        canonical_schema_bytes(&schema, expected.len()).unwrap(),
        expected
    );
    assert_eq!(
        canonical_schema_layout_hash(&schema, expected.len())
            .unwrap()
            .as_bytes(),
        blake3::hash(&expected).as_bytes()
    );
    for limit in 0..expected.len() {
        let error = RuntimeSchemaError::BudgetExceeded {
            budget: "encoded_bytes",
        };
        assert_eq!(canonical_schema_bytes(&schema, limit), Err(error.clone()));
        assert_eq!(canonical_schema_layout_hash(&schema, limit), Err(error));
    }
}

#[test]
fn deep_schema_encoding_and_layout_hash_do_not_use_recursive_calls() {
    let depth = 20_000;
    let mut schema = RuntimeTypeSchema::Unit;
    for _ in 0..depth {
        schema = RuntimeTypeSchema::Option(Box::new(schema));
    }
    let mut expected = document(&[]);
    expected.extend(std::iter::repeat_n(20, depth));
    expected.push(1);
    let bytes = canonical_schema_bytes(&schema, expected.len());
    let layout = schema.try_layout_hash();
    let limited = canonical_schema_layout_hash(&schema, expected.len() - 1);
    // The input's ordinary Box drop is independent of the encoder. Peel it
    // iteratively so this test measures schema traversal, not Rust drop glue.
    while let RuntimeTypeSchema::Option(inner) = schema {
        schema = *inner;
    }
    assert_eq!(bytes.unwrap(), expected);
    assert_eq!(
        layout.unwrap().as_bytes(),
        blake3::hash(&expected).as_bytes()
    );
    assert_eq!(
        limited,
        Err(RuntimeSchemaError::BudgetExceeded {
            budget: "encoded_bytes"
        })
    );
}
