use std::collections::BTreeMap;

use arcweft_codec_avro::codec::AvroCodec;
use arcweft_data::{
    Bytes, BytesFormat, Codec, DataErrorKind, DecodeLimits, DecodeOptions, EncodeOptions,
    FieldShape, Number, RecordPolicy, TypeShape, Value, VariantShape,
};

const ROW_SCHEMA: &str = r#"
{
  "type": "record",
  "name": "AssetRow",
  "fields": [
    {"name": "active", "type": "boolean"},
    {"name": "score", "type": "int"},
    {"name": "ratio", "type": "double"},
    {"name": "name", "type": "string"},
    {"name": "initial", "type": "string"},
    {"name": "blob", "type": "bytes"},
    {"name": "nickname", "type": ["null", "string"], "default": null},
    {"name": "tags", "type": {"type": "array", "items": "string"}},
    {"name": "meta", "type": {"type": "map", "values": "long"}},
    {"name": "kind", "type": {"type": "enum", "name": "AssetKind", "symbols": ["full", "empty"]}}
  ]
}
"#;

fn row_shape() -> TypeShape {
    TypeShape::seq(TypeShape::Record {
        name: "AssetRow".to_owned(),
        fields: vec![
            FieldShape::new("active", "active", TypeShape::Bool),
            FieldShape::new("score", "score", TypeShape::U8),
            FieldShape::new("ratio", "ratio", TypeShape::F64),
            FieldShape::new("name", "name", TypeShape::String),
            FieldShape::new("initial", "initial", TypeShape::Char),
            FieldShape::new(
                "blob",
                "blob",
                TypeShape::Bytes {
                    format: BytesFormat::default(),
                },
            ),
            FieldShape::new("nickname", "nickname", TypeShape::option(TypeShape::String)),
            FieldShape::new("tags", "tags", TypeShape::seq(TypeShape::String)),
            FieldShape::new(
                "meta",
                "meta",
                TypeShape::map(TypeShape::String, TypeShape::I64),
            ),
            FieldShape::new(
                "kind",
                "kind",
                TypeShape::enumeration(
                    "AssetKind",
                    [
                        VariantShape::unit("Full", "full"),
                        VariantShape::unit("Empty", "empty"),
                    ],
                ),
            ),
        ],
        policy: RecordPolicy {
            deny_unknown_fields: true,
        },
    })
}

fn record(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Record(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn map(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Map(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn sample_rows() -> Value {
    Value::Seq(vec![
        record([
            ("active", Value::Bool(true)),
            ("score", Value::Number(Number::U(7))),
            ("ratio", Value::Number(Number::F64(0.5))),
            ("name", Value::String("hero".to_owned())),
            ("initial", Value::Char('h')),
            ("blob", Value::Bytes(Bytes::from([1_u8, 2, 3].as_slice()))),
            ("nickname", Value::String("ace".to_owned())),
            (
                "tags",
                Value::Seq(vec![
                    Value::String("front".to_owned()),
                    Value::String("rare".to_owned()),
                ]),
            ),
            ("meta", map([("hp", Value::Number(Number::I(10)))])),
            (
                "kind",
                Value::Enum {
                    variant: "full".to_owned(),
                    payload: None,
                },
            ),
        ]),
        record([
            ("active", Value::Bool(false)),
            ("score", Value::Number(Number::U(8))),
            ("ratio", Value::Number(Number::F64(1.25))),
            ("name", Value::String("sidekick".to_owned())),
            ("initial", Value::Char('s')),
            ("blob", Value::Bytes(Bytes::from([4_u8].as_slice()))),
            ("nickname", Value::Unit),
            ("tags", Value::Seq(Vec::new())),
            ("meta", map([("hp", Value::Number(Number::I(5)))])),
            (
                "kind",
                Value::Enum {
                    variant: "empty".to_owned(),
                    payload: None,
                },
            ),
        ]),
    ])
}

#[test]
fn avro_codec_roundtrips_shape_driven_rows() {
    let codec = AvroCodec::new(ROW_SCHEMA).expect("schema");
    let encoded = codec
        .encode_value(&sample_rows(), &row_shape(), &EncodeOptions::default())
        .expect("encode");
    let decoded = codec
        .decode_value(&encoded, &row_shape(), &DecodeOptions::default())
        .expect("decode");
    assert_eq!(decoded, sample_rows());
}

#[test]
fn avro_codec_rejects_unknown_and_missing_fields() {
    let codec = AvroCodec::new(ROW_SCHEMA).expect("schema");
    let mut with_extra = sample_rows();
    let Value::Seq(rows) = &mut with_extra else {
        panic!("expected seq");
    };
    let Value::Record(first) = &mut rows[0] else {
        panic!("expected record");
    };
    first.insert("extra".to_owned(), Value::String("ignored".to_owned()));
    let error = codec
        .encode_value(&with_extra, &row_shape(), &EncodeOptions::default())
        .expect_err("unknown field");
    assert_eq!(error.kind(), &DataErrorKind::UnknownField);

    let value = Value::Seq(vec![Value::Record(BTreeMap::from([(
        "active".to_owned(),
        Value::Bool(true),
    )]))]);
    let error = codec
        .encode_value(&value, &row_shape(), &EncodeOptions::default())
        .expect_err("missing required field");
    assert_eq!(error.kind(), &DataErrorKind::MissingField);
}

#[test]
fn avro_codec_rejects_schema_mismatch_before_value_conversion() {
    let schema = r#"
    {
      "type": "record",
      "name": "WrongRow",
      "fields": [
        {"name": "active", "type": "boolean"},
        {"name": "score", "type": "string"}
      ]
    }
    "#;
    let shape = TypeShape::seq(TypeShape::record(
        "WrongRow",
        [
            FieldShape::new("active", "active", TypeShape::Bool),
            FieldShape::new("score", "score", TypeShape::U8),
        ],
    ));
    let value = Value::Seq(vec![record([
        ("active", Value::Bool(true)),
        ("score", Value::Number(Number::U(1))),
    ])]);
    let codec = AvroCodec::new(schema).expect("schema");
    let error = codec
        .encode_value(&value, &shape, &EncodeOptions::default())
        .expect_err("schema mismatch");
    assert_eq!(error.kind(), &DataErrorKind::InvalidType);
}

#[test]
fn avro_codec_enforces_numeric_edge_policy() {
    let codec = AvroCodec::new(ROW_SCHEMA).expect("schema");
    let value = Value::Seq(vec![record([
        ("active", Value::Bool(true)),
        ("score", Value::Number(Number::U(1))),
        ("ratio", Value::Number(Number::F64(f64::NAN))),
        ("name", Value::String("bad".to_owned())),
        ("initial", Value::Char('b')),
        ("blob", Value::Bytes(Bytes::from([0_u8].as_slice()))),
        ("nickname", Value::Unit),
        ("tags", Value::Seq(Vec::new())),
        ("meta", map([("hp", Value::Number(Number::I(1)))])),
        (
            "kind",
            Value::Enum {
                variant: "full".to_owned(),
                payload: None,
            },
        ),
    ])]);
    let error = codec
        .encode_value(&value, &row_shape(), &EncodeOptions::default())
        .expect_err("nan rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    let wide_schema =
        r#"{"type":"record","name":"Wide","fields":[{"name":"score","type":"long"}]}"#;
    let wide_shape = TypeShape::seq(TypeShape::record(
        "Wide",
        [FieldShape::new("score", "score", TypeShape::U32)],
    ));
    let narrow_shape = TypeShape::seq(TypeShape::record(
        "Wide",
        [FieldShape::new("score", "score", TypeShape::U8)],
    ));
    let wide_codec = AvroCodec::new(wide_schema).expect("schema");
    let encoded = wide_codec
        .encode_value(
            &Value::Seq(vec![record([("score", Value::Number(Number::U(300)))])]),
            &wide_shape,
            &EncodeOptions::default(),
        )
        .expect("wide encode");
    let error = wide_codec
        .decode_value(&encoded, &narrow_shape, &DecodeOptions::default())
        .expect_err("u8 overflow rejected");
    assert_eq!(error.kind(), &DataErrorKind::NumberOutOfRange);

    let error = wide_codec
        .encode_value(
            &Value::Seq(vec![record([("score", Value::Number(Number::F64(1.5)))])]),
            &wide_shape,
            &EncodeOptions::default(),
        )
        .expect_err("float to integer rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidType);
}

#[test]
fn avro_codec_checks_input_limit_before_parse() {
    let codec = AvroCodec::new(ROW_SCHEMA).expect("schema");
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_input_len: 2,
            ..DecodeLimits::default()
        },
    };
    let error = codec
        .decode_value(b"not a real file", &row_shape(), &options)
        .expect_err("input limit");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn avro_codec_rejects_payload_enums_and_symbol_mismatch_explicitly() {
    let payload_shape = TypeShape::seq(TypeShape::record(
        "PayloadRow",
        [FieldShape::new(
            "kind",
            "kind",
            TypeShape::enumeration(
                "Kind",
                [VariantShape::unit("Full", "full").with_payload(TypeShape::String)],
            ),
        )],
    ));
    let payload_schema = r#"
    {
      "type": "record",
      "name": "PayloadRow",
      "fields": [
        {"name": "kind", "type": {"type": "enum", "name": "Kind", "symbols": ["full"]}}
      ]
    }
    "#;
    let codec = AvroCodec::new(payload_schema).expect("schema");
    let error = codec
        .encode_value(
            &Value::Seq(Vec::new()),
            &payload_shape,
            &EncodeOptions::default(),
        )
        .expect_err("payload enum unsupported");
    assert_eq!(error.kind(), &DataErrorKind::UnsupportedFormat);

    let reversed_schema = r#"
    {
      "type": "record",
      "name": "AssetRow",
      "fields": [
        {"name": "kind", "type": {"type": "enum", "name": "AssetKind", "symbols": ["empty", "full"]}}
      ]
    }
    "#;
    let reversed_shape = TypeShape::seq(TypeShape::record(
        "AssetRow",
        [FieldShape::new(
            "kind",
            "kind",
            TypeShape::enumeration(
                "AssetKind",
                [
                    VariantShape::unit("Full", "full"),
                    VariantShape::unit("Empty", "empty"),
                ],
            ),
        )],
    ));
    let codec = AvroCodec::new(reversed_schema).expect("schema");
    let error = codec
        .encode_value(
            &Value::Seq(Vec::new()),
            &reversed_shape,
            &EncodeOptions::default(),
        )
        .expect_err("symbol order mismatch");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEnumTag);
}

#[test]
fn avro_codec_supports_top_level_scalar_policy() {
    let codec = AvroCodec::new(r#""boolean""#).expect("schema");
    let encoded = codec
        .encode_value(
            &Value::Bool(true),
            &TypeShape::Bool,
            &EncodeOptions::default(),
        )
        .expect("encode");
    let decoded = codec
        .decode_value(&encoded, &TypeShape::Bool, &DecodeOptions::default())
        .expect("decode");
    assert_eq!(decoded, Value::Bool(true));

    let seq_error = codec
        .decode_value(
            &encoded,
            &TypeShape::seq(TypeShape::Bool),
            &DecodeOptions::default(),
        )
        .expect("sequence decode uses container datum stream");
    assert_eq!(seq_error, Value::Seq(vec![Value::Bool(true)]));
}
