use std::collections::BTreeMap;

use arcweft_codec_arrow::{ArrowIpcCodec, ParquetCodec};
use arcweft_data::{
    Bytes, BytesFormat, Codec, DataErrorKind, DecodeLimits, DecodeOptions, EncodeOptions,
    FieldShape, Number, RecordPolicy, TypeShape, Value, VariantShape,
};

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
        ]),
        record([
            ("active", Value::Bool(false)),
            ("score", Value::Number(Number::U(8))),
            ("ratio", Value::Number(Number::F64(1.25))),
            ("name", Value::String("sidekick".to_owned())),
            ("initial", Value::Char('s')),
            ("blob", Value::Bytes(Bytes::from([4_u8].as_slice()))),
            ("nickname", Value::Unit),
        ]),
    ])
}

#[test]
fn arrow_ipc_codec_roundtrips_shape_driven_rows() {
    roundtrip_shape_driven_rows(&ArrowIpcCodec);
}

#[test]
fn parquet_codec_roundtrips_shape_driven_rows() {
    roundtrip_shape_driven_rows(&ParquetCodec);
}

fn roundtrip_shape_driven_rows(codec: &impl Codec) {
    let encoded = codec
        .encode_value(&sample_rows(), &row_shape(), &EncodeOptions::default())
        .expect("encode");
    let decoded = codec
        .decode_value(&encoded, &row_shape(), &DecodeOptions::default())
        .expect("decode");
    assert_eq!(decoded, sample_rows());
}

#[test]
fn arrow_ipc_codec_rejects_unknown_and_missing_fields() {
    rejects_unknown_and_missing_fields(&ArrowIpcCodec);
}

#[test]
fn parquet_codec_rejects_unknown_and_missing_fields() {
    rejects_unknown_and_missing_fields(&ParquetCodec);
}

fn rejects_unknown_and_missing_fields(codec: &impl Codec) {
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
fn arrow_ipc_codec_enforces_numeric_edge_policy() {
    enforces_numeric_edge_policy(&ArrowIpcCodec);
}

#[test]
fn parquet_codec_enforces_numeric_edge_policy() {
    enforces_numeric_edge_policy(&ParquetCodec);
}

fn enforces_numeric_edge_policy(codec: &impl Codec) {
    let value = Value::Seq(vec![record([
        ("active", Value::Bool(true)),
        ("score", Value::Number(Number::U(1))),
        ("ratio", Value::Number(Number::F64(f64::NAN))),
        ("name", Value::String("bad".to_owned())),
        ("initial", Value::Char('b')),
        ("blob", Value::Bytes(Bytes::from([0_u8].as_slice()))),
        ("nickname", Value::Unit),
    ])]);
    let error = codec
        .encode_value(&value, &row_shape(), &EncodeOptions::default())
        .expect_err("nan rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    let wide_shape = TypeShape::seq(TypeShape::record(
        "Wide",
        [FieldShape::new("score", "score", TypeShape::U64)],
    ));
    let narrow_shape = TypeShape::seq(TypeShape::record(
        "Narrow",
        [FieldShape::new("score", "score", TypeShape::U8)],
    ));
    let encoded = codec
        .encode_value(
            &Value::Seq(vec![record([("score", Value::Number(Number::U(300)))])]),
            &wide_shape,
            &EncodeOptions::default(),
        )
        .expect("wide encode");
    let error = codec
        .decode_value(&encoded, &narrow_shape, &DecodeOptions::default())
        .expect_err("u8 overflow rejected");
    assert_eq!(error.kind(), &DataErrorKind::NumberOutOfRange);

    let float_shape = TypeShape::seq(TypeShape::record(
        "Floaty",
        [FieldShape::new("score", "score", TypeShape::F64)],
    ));
    let encoded = codec
        .encode_value(
            &Value::Seq(vec![record([("score", Value::Number(Number::F64(1.5)))])]),
            &float_shape,
            &EncodeOptions::default(),
        )
        .expect("float encode");
    let error = codec
        .decode_value(&encoded, &narrow_shape, &DecodeOptions::default())
        .expect_err("float to integer rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidType);
}

#[test]
fn arrow_ipc_codec_checks_input_limit_before_parse() {
    checks_input_limit_before_parse(&ArrowIpcCodec);
}

#[test]
fn parquet_codec_checks_input_limit_before_parse() {
    checks_input_limit_before_parse(&ParquetCodec);
}

fn checks_input_limit_before_parse(codec: &impl Codec) {
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
fn arrow_ipc_codec_consumes_row_budget_during_decode() {
    consumes_row_budget_during_decode(&ArrowIpcCodec);
}

#[test]
fn parquet_codec_consumes_row_budget_during_decode() {
    consumes_row_budget_during_decode(&ParquetCodec);
}

fn consumes_row_budget_during_decode(codec: &impl Codec) {
    let encoded = codec
        .encode_value(&sample_rows(), &row_shape(), &EncodeOptions::default())
        .expect("encode");
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_sequence_len: 1,
            ..DecodeLimits::default()
        },
    };
    let error = codec
        .decode_value(&encoded, &row_shape(), &options)
        .expect_err("row budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn arrow_ipc_codec_consumes_record_field_budget_during_decode() {
    consumes_record_field_budget_during_decode(&ArrowIpcCodec);
}

#[test]
fn parquet_codec_consumes_record_field_budget_during_decode() {
    consumes_record_field_budget_during_decode(&ParquetCodec);
}

fn consumes_record_field_budget_during_decode(codec: &impl Codec) {
    let encoded = codec
        .encode_value(&sample_rows(), &row_shape(), &EncodeOptions::default())
        .expect("encode");
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_map_len: 2,
            ..DecodeLimits::default()
        },
    };
    let error = codec
        .decode_value(&encoded, &row_shape(), &options)
        .expect_err("record field budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn arrow_ipc_codec_consumes_string_budget_before_string_copy() {
    consumes_string_budget_before_string_copy(&ArrowIpcCodec);
}

#[test]
fn parquet_codec_consumes_string_budget_before_string_copy() {
    consumes_string_budget_before_string_copy(&ParquetCodec);
}

fn consumes_string_budget_before_string_copy(codec: &impl Codec) {
    let encoded = codec
        .encode_value(&sample_rows(), &row_shape(), &EncodeOptions::default())
        .expect("encode");
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_string_len: 3,
            ..DecodeLimits::default()
        },
    };
    let error = codec
        .decode_value(&encoded, &row_shape(), &options)
        .expect_err("string budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn arrow_ipc_codec_consumes_bytes_budget_before_bytes_copy() {
    consumes_bytes_budget_before_bytes_copy(&ArrowIpcCodec);
}

#[test]
fn parquet_codec_consumes_bytes_budget_before_bytes_copy() {
    consumes_bytes_budget_before_bytes_copy(&ParquetCodec);
}

fn consumes_bytes_budget_before_bytes_copy(codec: &impl Codec) {
    let encoded = codec
        .encode_value(&sample_rows(), &row_shape(), &EncodeOptions::default())
        .expect("encode");
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_bytes_len: 2,
            ..DecodeLimits::default()
        },
    };
    let error = codec
        .decode_value(&encoded, &row_shape(), &options)
        .expect_err("bytes budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn arrow_ipc_codec_rejects_nested_or_enum_shapes_explicitly() {
    rejects_nested_or_enum_shapes_explicitly(&ArrowIpcCodec);
}

#[test]
fn parquet_codec_rejects_nested_or_enum_shapes_explicitly() {
    rejects_nested_or_enum_shapes_explicitly(&ParquetCodec);
}

fn rejects_nested_or_enum_shapes_explicitly(codec: &impl Codec) {
    let enum_shape = TypeShape::seq(TypeShape::record(
        "EnumRow",
        [FieldShape::new(
            "kind",
            "kind",
            TypeShape::enumeration("Kind", [VariantShape::unit("Full", "full")]),
        )],
    ));
    let error = codec
        .encode_value(
            &Value::Seq(vec![record([(
                "kind",
                Value::Enum {
                    variant: "full".to_owned(),
                    payload: None,
                },
            )])]),
            &enum_shape,
            &EncodeOptions::default(),
        )
        .expect_err("enum unsupported");
    assert_eq!(error.kind(), &DataErrorKind::UnsupportedFormat);

    let nested_shape = TypeShape::seq(TypeShape::record(
        "NestedRow",
        [FieldShape::new(
            "tags",
            "tags",
            TypeShape::seq(TypeShape::String),
        )],
    ));
    let error = codec
        .encode_value(
            &Value::Seq(vec![record([(
                "tags",
                Value::Seq(vec![Value::String("one".to_owned())]),
            )])]),
            &nested_shape,
            &EncodeOptions::default(),
        )
        .expect_err("nested unsupported");
    assert_eq!(error.kind(), &DataErrorKind::UnsupportedFormat);
}
