use std::collections::BTreeMap;

use arcweft_codec_json::JsonCodec;
use arcweft_data::{
    Bytes, BytesFormat, Codec, DataErrorKind, DecodeLimits, DecodeOptions, EncodeOptions,
    FieldShape, RecordPolicy, TypeShape, Value,
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
