use std::collections::BTreeMap;

use arcweft_codec_yaml::YamlCodec;
use arcweft_data::{
    Bytes, BytesFormat, Codec, DataErrorKind, DecodeLimits, DecodeOptions, EncodeOptions,
    FieldShape, Number, RecordPolicy, TypeShape, Value,
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
            FieldShape::new("tag", "tag", TypeShape::option(TypeShape::String)),
        ],
        policy: RecordPolicy {
            deny_unknown_fields: true,
        },
    }
}

#[test]
fn yaml_codec_uses_shape_bytes_policy_for_records() {
    let value = Value::Record(BTreeMap::from([
        (
            "hash".to_owned(),
            Value::Bytes(Bytes::from([1_u8, 2, 255].as_slice())),
        ),
        ("name".to_owned(), Value::String("hero".to_owned())),
        ("tag".to_owned(), Value::String("npc".to_owned())),
    ]));

    let yaml = YamlCodec
        .encode_value(&value, &asset_shape(), &EncodeOptions::default())
        .expect("encode");
    let text = std::str::from_utf8(&yaml).expect("utf8");
    assert!(text.contains("0102ff"));
    assert!(text.contains("hero"));

    let decoded = YamlCodec
        .decode_value(&yaml, &asset_shape(), &DecodeOptions::default())
        .expect("decode");
    assert_eq!(decoded, value);
}

#[test]
fn yaml_codec_roundtrips_record_option_none_as_null() {
    let value = Value::Record(BTreeMap::from([
        (
            "hash".to_owned(),
            Value::Bytes(Bytes::from([0_u8, 255].as_slice())),
        ),
        ("name".to_owned(), Value::String("hero".to_owned())),
        ("tag".to_owned(), Value::Unit),
    ]));

    let yaml = YamlCodec
        .encode_value(&value, &asset_shape(), &EncodeOptions::default())
        .expect("encode");
    let text = std::str::from_utf8(&yaml).expect("utf8");
    assert!(text.contains("tag"));

    let decoded = YamlCodec
        .decode_value(&yaml, &asset_shape(), &DecodeOptions::default())
        .expect("decode");
    assert_eq!(decoded, value);
}

#[test]
fn yaml_codec_rejects_unknown_record_fields_through_shape() {
    let error = YamlCodec
        .decode_value(
            b"hash: '00'\nname: hero\nextra: true\n",
            &asset_shape(),
            &DecodeOptions::default(),
        )
        .expect_err("unknown field");
    assert_eq!(error.kind(), &DataErrorKind::UnknownField);
}

#[test]
fn yaml_codec_checks_input_limit_before_parse() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_input_len: 2,
            ..DecodeLimits::default()
        },
    };
    let error = YamlCodec
        .decode_value(b"name: hero\n", &asset_shape(), &options)
        .expect_err("input cap");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn yaml_codec_rejects_multiple_documents() {
    let error = YamlCodec
        .decode_value(
            b"hash: '00'\nname: hero\n---\nhash: '01'\nname: villain\n",
            &asset_shape(),
            &DecodeOptions::default(),
        )
        .expect_err("second document");
    assert_eq!(error.kind(), &DataErrorKind::TrailingData);
}

#[test]
fn yaml_codec_enforces_numeric_edge_policy() {
    let error = YamlCodec
        .encode_value(
            &Value::Number(Number::F32(f32::NAN)),
            &TypeShape::F32,
            &EncodeOptions::default(),
        )
        .expect_err("nan rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    let shape = TypeShape::record(
        "Numeric",
        [FieldShape::new("count", "count", TypeShape::U8)],
    );
    let error = YamlCodec
        .decode_value(b"count: 1.5\n", &shape, &DecodeOptions::default())
        .expect_err("float to integer rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidType);

    let error = YamlCodec
        .decode_value(b"count: -1\n", &shape, &DecodeOptions::default())
        .expect_err("negative unsigned rejected");
    assert_eq!(error.kind(), &DataErrorKind::NumberOutOfRange);
}
