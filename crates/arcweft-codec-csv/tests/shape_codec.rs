use std::collections::BTreeMap;

use arcweft_codec_csv::CsvCodec;
use arcweft_data::{
    Bytes, BytesFormat, Codec, DataErrorKind, DecodeLimits, DecodeOptions, EncodeOptions,
    FieldShape, Number, RecordPolicy, TypeShape, Value,
};

fn row_shape() -> TypeShape {
    TypeShape::seq(TypeShape::Record {
        name: "AssetRow".to_owned(),
        fields: vec![
            FieldShape::new("active", "active", TypeShape::Bool),
            FieldShape::new("score", "score", TypeShape::U8),
            FieldShape::new("name", "name", TypeShape::String),
            FieldShape::new(
                "hash",
                "hash",
                TypeShape::Bytes {
                    format: BytesFormat::Binary,
                },
            )
            .with_bytes_format(BytesFormat::Hex),
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

#[test]
fn csv_codec_uses_shape_headers_and_cell_types() {
    let value = Value::Seq(vec![
        record([
            ("active", Value::Bool(true)),
            ("score", Value::Number(Number::U(7))),
            ("name", Value::String("hero".to_owned())),
            ("hash", Value::Bytes(Bytes::from([1_u8, 2, 255].as_slice()))),
            ("nickname", Value::String("ace".to_owned())),
        ]),
        record([
            ("active", Value::Bool(false)),
            ("score", Value::Number(Number::U(0))),
            ("name", Value::String("sidekick".to_owned())),
            ("hash", Value::Bytes(Bytes::from([0_u8].as_slice()))),
            ("nickname", Value::Unit),
        ]),
    ]);

    let csv = CsvCodec
        .encode_value(&value, &row_shape(), &EncodeOptions::default())
        .expect("encode");
    let text = std::str::from_utf8(&csv).expect("csv is utf8");
    assert_eq!(
        text.lines().next().expect("header"),
        "active,score,name,hash,nickname"
    );

    let decoded = CsvCodec
        .decode_value(&csv, &row_shape(), &DecodeOptions::default())
        .expect("decode");
    assert_eq!(decoded, value);
}

#[test]
fn csv_codec_rejects_unknown_columns_when_policy_denies_them() {
    let error = CsvCodec
        .decode_value(
            b"active,score,name,hash,nickname,extra\ntrue,1,hero,00,,ignored\n",
            &row_shape(),
            &DecodeOptions::default(),
        )
        .expect_err("unknown column");
    assert_eq!(error.kind(), &DataErrorKind::UnknownField);
}

#[test]
fn csv_codec_rejects_missing_required_columns() {
    let error = CsvCodec
        .decode_value(
            b"active,score,name,nickname\ntrue,1,hero,\n",
            &row_shape(),
            &DecodeOptions::default(),
        )
        .expect_err("missing column");
    assert_eq!(error.kind(), &DataErrorKind::MissingField);
}

#[test]
fn csv_codec_rejects_unknown_encode_fields_when_policy_denies_them() {
    let value = Value::Seq(vec![Value::Record(BTreeMap::from([
        ("active".to_owned(), Value::Bool(true)),
        ("score".to_owned(), Value::Number(Number::U(1))),
        ("name".to_owned(), Value::String("hero".to_owned())),
        (
            "hash".to_owned(),
            Value::Bytes(Bytes::from([0_u8].as_slice())),
        ),
        ("nickname".to_owned(), Value::Unit),
        ("extra".to_owned(), Value::String("ignored".to_owned())),
    ]))]);

    let error = CsvCodec
        .encode_value(&value, &row_shape(), &EncodeOptions::default())
        .expect_err("unknown field");
    assert_eq!(error.kind(), &DataErrorKind::UnknownField);
}

#[test]
fn csv_codec_rejects_nested_cell_shapes() {
    let shape = TypeShape::seq(TypeShape::record(
        "Nested",
        [FieldShape::new(
            "tags",
            "tags",
            TypeShape::seq(TypeShape::String),
        )],
    ));
    let value = Value::Seq(vec![record([(
        "tags",
        Value::Seq(vec![Value::String("one".to_owned())]),
    )])]);

    let error = CsvCodec
        .encode_value(&value, &shape, &EncodeOptions::default())
        .expect_err("nested shapes are unsupported");
    assert_eq!(error.kind(), &DataErrorKind::UnsupportedFormat);
}

#[test]
fn csv_codec_enforces_numeric_edge_policy() {
    let int_shape = TypeShape::seq(TypeShape::record(
        "Numeric",
        [FieldShape::new("count", "count", TypeShape::U8)],
    ));
    let float_shape = TypeShape::seq(TypeShape::record(
        "Floaty",
        [FieldShape::new("ratio", "ratio", TypeShape::F64)],
    ));

    let value = Value::Seq(vec![record([(
        "ratio",
        Value::Number(Number::F64(f64::INFINITY)),
    )])]);
    let error = CsvCodec
        .encode_value(&value, &float_shape, &EncodeOptions::default())
        .expect_err("infinity rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    let error = CsvCodec
        .decode_value(b"count\n1.5\n", &int_shape, &DecodeOptions::default())
        .expect_err("float to integer rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    let error = CsvCodec
        .decode_value(b"count\n-1\n", &int_shape, &DecodeOptions::default())
        .expect_err("negative unsigned rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    let error = CsvCodec
        .decode_value(b"ratio\ninf\n", &float_shape, &DecodeOptions::default())
        .expect_err("non-finite float rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);
}

#[test]
fn csv_decode_consumes_row_budget_during_reader_iteration() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_sequence_len: 2,
            ..DecodeLimits::default()
        },
    };

    let error = CsvCodec
        .decode_value(
            b"active,score,name,hash,nickname\ntrue,1,a,00,\ntrue,2,b,00,\ntrue,3,c,00,\n",
            &row_shape(),
            &options,
        )
        .expect_err("row budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn csv_decode_consumes_cell_string_budget_during_reader_iteration() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_string_len: 3,
            ..DecodeLimits::default()
        },
    };

    let error = CsvCodec
        .decode_value(
            b"active,score,name,hash,nickname\ntrue,1,hero,00,\n",
            &row_shape(),
            &options,
        )
        .expect_err("string budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn csv_decode_checks_hex_bytes_budget_before_decode_allocation() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_bytes_len: 1,
            ..DecodeLimits::default()
        },
    };

    let error = CsvCodec
        .decode_value(
            b"active,score,name,hash,nickname\ntrue,1,hero,0000,\n",
            &row_shape(),
            &options,
        )
        .expect_err("bytes budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}
