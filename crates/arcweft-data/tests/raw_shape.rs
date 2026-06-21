use std::collections::BTreeMap;

use arcweft_data::{
    Bytes, BytesFormat, DataErrorKind, EnumRepr, EnumTagStyle, FieldShape, Number, RawValue,
    RecordPolicy, TypeShape, Value, VariantShape, decode_with_shape, encode_with_shape,
};

#[test]
fn raw_shape_roundtrips_bytes_and_records() {
    let shape = TypeShape::record(
        "Asset",
        [
            FieldShape::new("name", "name", TypeShape::String),
            FieldShape::new(
                "blob",
                "blob",
                TypeShape::Bytes {
                    format: BytesFormat::Binary,
                },
            ),
        ],
    );
    let value = Value::Record(BTreeMap::from([
        ("name".to_owned(), Value::String("hero".to_owned())),
        (
            "blob".to_owned(),
            Value::Bytes(Bytes::new(vec![0, 1, 2, 255])),
        ),
    ]));

    let raw = encode_with_shape(&value, &shape).expect("record encodes");
    assert_eq!(
        raw,
        RawValue::Map(vec![
            (
                RawValue::String("name".to_owned()),
                RawValue::String("hero".to_owned()),
            ),
            (
                RawValue::String("blob".to_owned()),
                RawValue::Bytes(vec![0, 1, 2, 255]),
            ),
        ])
    );
    assert_eq!(
        decode_with_shape(&raw, &shape).expect("record decodes"),
        value
    );
}

#[test]
fn raw_shape_rejects_unknown_record_fields() {
    let shape = TypeShape::record(
        "Config",
        [FieldShape::new("name", "name", TypeShape::String)],
    );
    let raw = RawValue::Map(vec![
        (
            RawValue::String("name".to_owned()),
            RawValue::String("ok".to_owned()),
        ),
        (RawValue::String("extra".to_owned()), RawValue::Bool(true)),
    ]);

    let error = decode_with_shape(&raw, &shape).expect_err("unknown field rejected");
    assert_eq!(error.kind(), &DataErrorKind::UnknownField);
    assert_eq!(error.path().to_string(), "$.extra");
}

#[test]
fn raw_shape_decodes_missing_optional_record_field_as_unit() {
    let shape = TypeShape::record(
        "Config",
        [
            FieldShape::new("name", "name", TypeShape::String),
            FieldShape::new("tag", "tag", TypeShape::option(TypeShape::String)),
        ],
    );
    let raw = RawValue::Map(vec![(
        RawValue::String("name".to_owned()),
        RawValue::String("ok".to_owned()),
    )]);

    let decoded = decode_with_shape(&raw, &shape).expect("missing optional field decodes");
    assert_eq!(
        decoded,
        Value::Record(BTreeMap::from([
            ("name".to_owned(), Value::String("ok".to_owned())),
            ("tag".to_owned(), Value::Unit),
        ]))
    );
}

#[test]
fn raw_shape_rejects_number_overflow() {
    let value = Value::Number(Number::U(300));
    let error = encode_with_shape(&value, &TypeShape::U8).expect_err("u8 overflow rejected");

    assert_eq!(error.kind(), &DataErrorKind::NumberOutOfRange);
}

#[test]
fn raw_shape_rejects_float_to_integer_recovery() {
    let value = Value::Number(Number::F64(1.0));
    let error = encode_with_shape(&value, &TypeShape::U8).expect_err("float cannot encode as u8");
    assert_eq!(error.kind(), &DataErrorKind::InvalidType);

    let error = decode_with_shape(&RawValue::F64(1.0), &TypeShape::U8)
        .expect_err("float cannot decode as u8");
    assert_eq!(error.kind(), &DataErrorKind::InvalidType);
}

#[test]
fn raw_shape_rejects_non_finite_floats() {
    let error = encode_with_shape(&Value::Number(Number::F32(f32::NAN)), &TypeShape::F32)
        .expect_err("nan encode rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    let error = decode_with_shape(&RawValue::F64(f64::INFINITY), &TypeShape::F64)
        .expect_err("infinity decode rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);
}

#[test]
fn raw_shape_roundtrips_enum_payload() {
    let shape = TypeShape::enumeration(
        "Route",
        [VariantShape::unit("Done", "done").with_payload(TypeShape::String)],
    );
    let value = Value::Enum {
        variant: "done".to_owned(),
        payload: Some(Box::new(Value::String("opening".to_owned()))),
    };

    let raw = encode_with_shape(&value, &shape).expect("enum encodes");
    assert_eq!(
        decode_with_shape(&raw, &shape).expect("enum decodes"),
        value
    );
}

#[test]
fn raw_shape_encodes_adjacent_enum_tag_and_content() {
    let shape = TypeShape::Enum {
        name: "Event".to_owned(),
        variants: vec![VariantShape::unit("Score", "score").with_payload(TypeShape::U32)],
        tag: EnumTagStyle::Adjacent {
            tag: "kind".to_owned(),
            content: "value".to_owned(),
        },
        repr: None,
    };
    let value = Value::Enum {
        variant: "score".to_owned(),
        payload: Some(Box::new(Value::Number(Number::U(42)))),
    };

    let raw = encode_with_shape(&value, &shape).expect("adjacent enum encodes");
    assert_eq!(
        raw,
        RawValue::Map(vec![
            (
                RawValue::String("kind".to_owned()),
                RawValue::String("score".to_owned()),
            ),
            (RawValue::String("value".to_owned()), RawValue::Unsigned(42),),
        ])
    );
    assert_eq!(
        decode_with_shape(&raw, &shape).expect("adjacent enum decodes"),
        value
    );
}

#[test]
fn raw_shape_encodes_internal_enum_payload_record() {
    let payload_shape = TypeShape::Record {
        name: "LineShown".to_owned(),
        fields: vec![FieldShape::new("line_id", "line_id", TypeShape::String)],
        policy: RecordPolicy {
            deny_unknown_fields: true,
        },
    };
    let shape = TypeShape::Enum {
        name: "Event".to_owned(),
        variants: vec![VariantShape::unit("LineShown", "line_shown").with_payload(payload_shape)],
        tag: EnumTagStyle::Internal {
            tag: "kind".to_owned(),
        },
        repr: None,
    };
    let value = Value::Enum {
        variant: "line_shown".to_owned(),
        payload: Some(Box::new(Value::Record(BTreeMap::from([(
            "line_id".to_owned(),
            Value::String("l001".to_owned()),
        )])))),
    };

    let raw = encode_with_shape(&value, &shape).expect("internal enum encodes");
    assert_eq!(
        raw,
        RawValue::Map(vec![
            (
                RawValue::String("kind".to_owned()),
                RawValue::String("line_shown".to_owned()),
            ),
            (
                RawValue::String("line_id".to_owned()),
                RawValue::String("l001".to_owned()),
            ),
        ])
    );
    assert_eq!(
        decode_with_shape(&raw, &shape).expect("internal enum decodes"),
        value
    );
}

#[test]
fn raw_shape_roundtrips_repr_enum_discriminants() {
    let shape = TypeShape::Enum {
        name: "SaveKind".to_owned(),
        variants: vec![
            VariantShape::unit("Full", "full").with_discriminant(1),
            VariantShape::unit("Quick", "quick").with_discriminant(2),
        ],
        tag: EnumTagStyle::External,
        repr: Some(EnumRepr::U8),
    };
    let value = Value::Enum {
        variant: "full".to_owned(),
        payload: None,
    };

    assert_eq!(
        encode_with_shape(&value, &shape).expect("repr enum encodes"),
        RawValue::Unsigned(1)
    );
    assert_eq!(
        decode_with_shape(&RawValue::Unsigned(2), &shape).expect("repr enum decodes"),
        Value::Enum {
            variant: "quick".to_owned(),
            payload: None,
        }
    );
}
