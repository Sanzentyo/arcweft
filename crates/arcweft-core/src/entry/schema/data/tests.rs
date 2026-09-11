use arcweft_data::{
    BytesFormat, EnumRepr, EnumTagStyle, FieldShape, RecordPolicy, TypeShape, VariantShape,
};

use crate::entry::{RuntimeSchemaError, RuntimeSchemaLimits, RuntimeTypeSchema};
use crate::value::{RuntimeSeq, RuntimeValue};

#[test]
fn reflected_record_schema_validates_runtime_widths_and_sequences() {
    let shape = TypeShape::Record {
        name: "Packet".to_owned(),
        fields: vec![
            FieldShape::new("code", "status", TypeShape::U16),
            FieldShape::new("data", "body", TypeShape::Seq(Box::new(TypeShape::U8))),
        ],
        policy: RecordPolicy {
            deny_unknown_fields: true,
        },
    };
    let schema = RuntimeTypeSchema::from(&shape);
    let limits = RuntimeSchemaLimits::engine_default();
    let packet = |code| {
        RuntimeValue::try_record(vec![
            ("code".to_owned(), code),
            (
                "data".to_owned(),
                RuntimeValue::Seq(RuntimeSeq::Values(vec![RuntimeValue::u8(7)])),
            ),
        ])
        .unwrap()
    };
    let value = packet(RuntimeValue::u16(200));
    assert_eq!(
        schema.validate_value(&value, limits).unwrap(),
        value.try_digest(limits.platform_encoded_bytes()).unwrap()
    );
    assert!(
        matches!(schema.validate_value(&packet(RuntimeValue::u32(200)), limits), Err(RuntimeSchemaError::Type { path, .. }) if path == "$.code")
    );
    let encoded = serde_json::to_vec(&schema).unwrap();
    let decoded: RuntimeTypeSchema = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded, schema);
    assert_eq!(
        decoded.try_layout_hash().unwrap(),
        schema.try_layout_hash().unwrap()
    );
}

#[test]
fn reflected_wire_metadata_and_recursive_names_remain_in_canonical_layout() {
    let shape = TypeShape::Enum {
        name: "Chain".to_owned(),
        variants: vec![
            VariantShape::unit("End", "end").with_discriminant(-1),
            VariantShape::unit("Next", "next").with_payload(TypeShape::Record {
                name: "Link".to_owned(),
                fields: vec![
                    FieldShape::new(
                        "bytes",
                        "b",
                        TypeShape::Bytes {
                            format: BytesFormat::Binary,
                        },
                    )
                    .with_bytes_format(BytesFormat::Hex)
                    .with_default(),
                    FieldShape::new(
                        "parent",
                        "p",
                        TypeShape::Option(Box::new(TypeShape::Named("Chain".to_owned()))),
                    ),
                    FieldShape::new("cache", "c", TypeShape::Unit).skipped(),
                ],
                policy: RecordPolicy {
                    deny_unknown_fields: true,
                },
            }),
        ],
        tag: EnumTagStyle::Adjacent {
            tag: "kind".to_owned(),
            content: "content".to_owned(),
        },
        repr: Some(EnumRepr::I8),
    };
    let schema = RuntimeTypeSchema::from(&shape);
    let original = schema.try_layout_hash().unwrap();
    let RuntimeTypeSchema::Enum {
        variants,
        tag,
        repr,
        ..
    } = &schema
    else {
        panic!("enum projection")
    };
    assert_eq!(
        *tag,
        super::RuntimeEnumTagStyle::Adjacent {
            tag: "kind".to_owned(),
            content: "content".to_owned()
        }
    );
    assert_eq!(*repr, Some(super::RuntimeEnumRepr::I8));
    assert_eq!(variants[0].discriminant, Some(-1));
    let Some(RuntimeTypeSchema::Record {
        fields,
        deny_unknown_fields,
        ..
    }) = &variants[1].payload
    else {
        panic!("record payload")
    };
    assert!(*deny_unknown_fields);
    assert!(fields[0].has_default);
    assert_eq!(fields[0].bytes_format, Some(super::RuntimeBytesFormat::Hex));
    assert!(fields[2].skip);
    assert_eq!(
        fields[1].schema,
        RuntimeTypeSchema::Option(Box::new(RuntimeTypeSchema::Named("Chain".to_owned())))
    );

    for change in 0..5 {
        let mut changed = shape.clone();
        let TypeShape::Enum {
            variants,
            tag,
            repr,
            ..
        } = &mut changed
        else {
            unreachable!()
        };
        match change {
            0 => variants[0].wire_name = "done".to_owned(),
            1 => variants[0].discriminant = Some(0),
            2 => *tag = EnumTagStyle::External,
            3 => *repr = Some(EnumRepr::U8),
            4 => {
                let Some(TypeShape::Record { fields, .. }) = &mut variants[1].payload else {
                    unreachable!()
                };
                fields[0].bytes_format = Some(BytesFormat::Base64);
            }
            _ => unreachable!(),
        }
        assert_ne!(
            original,
            RuntimeTypeSchema::from(&changed).try_layout_hash().unwrap()
        );
    }
}
