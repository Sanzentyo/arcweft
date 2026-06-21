use std::collections::BTreeMap;

use arcweft_codec_toml::TomlCodec;
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
fn toml_codec_uses_shape_bytes_policy_for_records() {
    let value = Value::Record(BTreeMap::from([
        (
            "hash".to_owned(),
            Value::Bytes(Bytes::from([1_u8, 2, 255].as_slice())),
        ),
        ("name".to_owned(), Value::String("hero".to_owned())),
        ("tag".to_owned(), Value::String("npc".to_owned())),
    ]));

    let toml = TomlCodec
        .encode_value(&value, &asset_shape(), &EncodeOptions::default())
        .expect("encode");
    let text = std::str::from_utf8(&toml).expect("utf8");
    assert!(text.contains("hash = \"0102ff\""));
    assert!(text.contains("name = \"hero\""));

    let decoded = TomlCodec
        .decode_value(&toml, &asset_shape(), &DecodeOptions::default())
        .expect("decode");
    assert_eq!(decoded, value);
}

#[test]
fn toml_codec_omits_record_option_none_and_decodes_missing_as_unit() {
    let value = Value::Record(BTreeMap::from([
        (
            "hash".to_owned(),
            Value::Bytes(Bytes::from([0_u8, 255].as_slice())),
        ),
        ("name".to_owned(), Value::String("hero".to_owned())),
        ("tag".to_owned(), Value::Unit),
    ]));

    let toml = TomlCodec
        .encode_value(&value, &asset_shape(), &EncodeOptions::default())
        .expect("encode");
    let text = std::str::from_utf8(&toml).expect("utf8");
    assert!(!text.contains("tag"));

    let decoded = TomlCodec
        .decode_value(&toml, &asset_shape(), &DecodeOptions::default())
        .expect("decode");
    assert_eq!(decoded, value);
}

#[test]
fn toml_codec_rejects_top_level_option_none() {
    let error = TomlCodec
        .encode_value(
            &Value::Unit,
            &TypeShape::option(TypeShape::String),
            &EncodeOptions::default(),
        )
        .expect_err("top-level none unsupported");
    assert_eq!(error.kind(), &DataErrorKind::UnsupportedFormat);
}

#[test]
fn toml_codec_rejects_unknown_record_fields_through_shape() {
    let error = TomlCodec
        .decode_value(
            b"hash = \"00\"\nname = \"hero\"\nextra = true\n",
            &asset_shape(),
            &DecodeOptions::default(),
        )
        .expect_err("unknown field");
    assert_eq!(error.kind(), &DataErrorKind::UnknownField);
}

#[test]
fn toml_codec_checks_input_limit_before_parse() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_input_len: 2,
            ..DecodeLimits::default()
        },
    };
    let error = TomlCodec
        .decode_value(b"name = \"hero\"\n", &asset_shape(), &options)
        .expect_err("input cap");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn toml_codec_enforces_numeric_edge_policy() {
    let error = TomlCodec
        .encode_value(
            &Value::Number(Number::F64(f64::INFINITY)),
            &TypeShape::F64,
            &EncodeOptions::default(),
        )
        .expect_err("infinity rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    let shape = TypeShape::record(
        "Numeric",
        [FieldShape::new("count", "count", TypeShape::U8)],
    );
    let error = TomlCodec
        .decode_value(b"count = 1.5\n", &shape, &DecodeOptions::default())
        .expect_err("float to integer rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidType);

    let error = TomlCodec
        .decode_value(b"count = -1\n", &shape, &DecodeOptions::default())
        .expect_err("negative unsigned rejected");
    assert_eq!(error.kind(), &DataErrorKind::NumberOutOfRange);
}

#[test]
fn toml_decode_consumes_string_budget_before_value_projection() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_string_len: 3,
            ..DecodeLimits::default()
        },
    };

    let error = TomlCodec
        .decode_value(
            b"hash = \"00\"\nname = \"hero\"\n",
            &asset_shape(),
            &options,
        )
        .expect_err("string budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn toml_decode_preflights_string_budget_before_detable_parse() {
    let mut input = b"hash = \"00\"\nname = \"".to_vec();
    input.extend(std::iter::repeat_n(b'a', 16 * 1024));
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_string_len: 8,
            ..DecodeLimits::default()
        },
    };

    let error = TomlCodec
        .decode_value(&input, &asset_shape(), &options)
        .expect_err("source string budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn toml_decode_preflights_bare_key_budget_before_detable_parse() {
    let mut input = Vec::new();
    input.extend(std::iter::repeat_n(b'a', 16 * 1024));
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_string_len: 8,
            ..DecodeLimits::default()
        },
    };

    let error = TomlCodec
        .decode_value(&input, &asset_shape(), &options)
        .expect_err("source bare key budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn toml_decode_consumes_array_budget_before_value_projection() {
    let shape = TypeShape::record(
        "Tags",
        [FieldShape::new(
            "tags",
            "tags",
            TypeShape::seq(TypeShape::String),
        )],
    );
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_sequence_len: 2,
            ..DecodeLimits::default()
        },
    };

    let error = TomlCodec
        .decode_value(b"tags = [\"a\", \"b\", \"c\"]\n", &shape, &options)
        .expect_err("array budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn toml_decode_preflights_array_budget_before_detable_parse() {
    let shape = TypeShape::record(
        "Tags",
        [FieldShape::new(
            "tags",
            "tags",
            TypeShape::seq(TypeShape::String),
        )],
    );
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_sequence_len: 2,
            ..DecodeLimits::default()
        },
    };

    let error = TomlCodec
        .decode_value(b"tags = [\"a\", \"b\", \"c\"\n", &shape, &options)
        .expect_err("source array budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn toml_decode_consumes_node_budget_before_value_projection() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_nodes: 2,
            ..DecodeLimits::default()
        },
    };

    let error = TomlCodec
        .decode_value(
            b"hash = \"00\"\nname = \"hero\"\n",
            &asset_shape(),
            &options,
        )
        .expect_err("node budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}
