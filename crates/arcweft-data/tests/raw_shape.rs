use std::collections::BTreeMap;

use arcweft_data::{
    Bytes, BytesFormat, DataErrorKind, FieldShape, Number, RawValue, TypeShape, Value,
    VariantShape, decode_with_shape, encode_with_shape,
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
