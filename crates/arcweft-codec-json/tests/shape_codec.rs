use std::collections::BTreeMap;

use arcweft_codec_json::JsonCodec;
use arcweft_data::{
    Bytes, BytesFormat, Codec, DataErrorKind, DecodeLimits, DecodeOptions, EncodeOptions, EnumRepr,
    EnumTagStyle, FieldShape, Number, RecordPolicy, TypeShape, Value, VariantShape,
};

fn asset_shape() -> TypeShape {
    TypeShape::Record {
        name: "Asset".to_owned(),
        fields: vec![
            FieldShape::new(
                "hash",
                "hash",
                TypeShape::Bytes {
                    format: BytesFormat::Binary,
                },
            )
            .with_bytes_format(BytesFormat::Hex),
            FieldShape::new("name", "name", TypeShape::String),
        ],
        policy: RecordPolicy {
            deny_unknown_fields: true,
        },
    }
}

#[test]
fn json_codec_uses_shape_bytes_policy_for_records() {
    let value = Value::Record(BTreeMap::from([
        (
            "hash".to_owned(),
            Value::Bytes(Bytes::from([1_u8, 2, 255].as_slice())),
        ),
        ("name".to_owned(), Value::String("hero".to_owned())),
    ]));
    let json = JsonCodec
        .encode_value(&value, &asset_shape(), &EncodeOptions::default())
        .expect("encode");
    assert_eq!(
        std::str::from_utf8(&json).expect("utf8"),
        r#"{"hash":"0102ff","name":"hero"}"#
    );

    let decoded = JsonCodec
        .decode_value(&json, &asset_shape(), &DecodeOptions::default())
        .expect("decode");
    assert_eq!(decoded, value);
}

#[test]
fn json_codec_rejects_unknown_record_fields_through_shape() {
    let error = JsonCodec
        .decode_value(
            br#"{"hash":"00","name":"hero","extra":true}"#,
            &asset_shape(),
            &DecodeOptions::default(),
        )
        .expect_err("unknown field");
    assert_eq!(error.kind(), &DataErrorKind::UnknownField);
}

#[test]
fn json_codec_checks_input_limit_before_parse() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_input_len: 2,
            ..DecodeLimits::default()
        },
    };
    let error = JsonCodec
        .decode_value(br"null", &TypeShape::Unit, &options)
        .expect_err("input cap");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn json_codec_rejects_trailing_non_whitespace() {
    let error = JsonCodec
        .decode_value(br"null true", &TypeShape::Unit, &DecodeOptions::default())
        .expect_err("trailing");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);
}

#[test]
fn json_codec_uses_adjacent_enum_tag_shape() {
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

    let json = JsonCodec
        .encode_value(&value, &shape, &EncodeOptions::default())
        .expect("encode");
    assert_eq!(
        std::str::from_utf8(&json).expect("utf8"),
        r#"{"kind":"score","value":42}"#
    );
    assert_eq!(
        JsonCodec
            .decode_value(&json, &shape, &DecodeOptions::default())
            .expect("decode"),
        value
    );
}

#[test]
fn json_codec_roundtrips_repr_enum_as_number() {
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
        variant: "quick".to_owned(),
        payload: None,
    };

    let json = JsonCodec
        .encode_value(&value, &shape, &EncodeOptions::default())
        .expect("encode");
    assert_eq!(std::str::from_utf8(&json).expect("utf8"), "2");
    assert_eq!(
        JsonCodec
            .decode_value(&json, &shape, &DecodeOptions::default())
            .expect("decode"),
        value
    );
}

#[test]
fn json_codec_enforces_numeric_edge_policy() {
    let error = JsonCodec
        .encode_value(
            &Value::Number(Number::F64(f64::NAN)),
            &TypeShape::F64,
            &EncodeOptions::default(),
        )
        .expect_err("nan rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    let error = JsonCodec
        .decode_value(b"1.5", &TypeShape::U8, &DecodeOptions::default())
        .expect_err("float to integer rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    let error = JsonCodec
        .decode_value(b"-1", &TypeShape::U8, &DecodeOptions::default())
        .expect_err("negative unsigned rejected");
    assert_eq!(error.kind(), &DataErrorKind::NumberOutOfRange);

    let error = JsonCodec
        .encode_value(
            &Value::Number(Number::U(u128::MAX)),
            &TypeShape::U128,
            &EncodeOptions::default(),
        )
        .expect_err("u128 beyond JSON number rejected");
    assert_eq!(error.kind(), &DataErrorKind::NumberOutOfRange);
}

#[test]
fn json_decode_consumes_string_budget_during_parse() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_string_len: 3,
            ..DecodeLimits::default()
        },
    };

    let error = JsonCodec
        .decode_value(br#""hero""#, &TypeShape::String, &options)
        .expect_err("string budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn json_decode_consumes_collection_budget_during_parse() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_sequence_len: 2,
            ..DecodeLimits::default()
        },
    };

    let error = JsonCodec
        .decode_value(
            br"[null,null,null]",
            &TypeShape::Seq(Box::new(TypeShape::Unit)),
            &options,
        )
        .expect_err("array budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn json_decode_consumes_node_budget_during_parse() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_nodes: 2,
            ..DecodeLimits::default()
        },
    };

    let error = JsonCodec
        .decode_value(
            br"[null,null]",
            &TypeShape::Seq(Box::new(TypeShape::Unit)),
            &options,
        )
        .expect_err("node budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}
