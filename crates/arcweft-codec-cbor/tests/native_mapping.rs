use std::collections::BTreeMap;
use std::io::Cursor;

use arcweft_codec_cbor::CborCodec;
use arcweft_data::{
    Bytes, BytesFormat, Codec, DataErrorKind, DecodeLimits, DecodeOptions, EncodeOptions,
    FieldShape, Number, RecordPolicy, TypeShape, Value,
};
use ciborium::Value as CborValue;

fn asset_shape() -> TypeShape {
    TypeShape::Record {
        name: "Asset".to_owned(),
        fields: vec![
            FieldShape::new(
                "blob",
                "blob",
                TypeShape::Bytes {
                    format: BytesFormat::Binary,
                },
            ),
            FieldShape::new("name", "name", TypeShape::String),
        ],
        policy: RecordPolicy {
            deny_unknown_fields: true,
        },
    }
}

#[test]
fn cbor_codec_preserves_native_bytes() {
    let value = Value::Record(BTreeMap::from([
        (
            "blob".to_owned(),
            Value::Bytes(Bytes::from([0_u8, 1, 2, 255].as_slice())),
        ),
        ("name".to_owned(), Value::String("hero".to_owned())),
    ]));

    let encoded = CborCodec
        .encode_value(&value, &asset_shape(), &EncodeOptions::default())
        .expect("encode");
    let native =
        ciborium::from_reader::<CborValue, _>(Cursor::new(&encoded)).expect("native decode");
    let CborValue::Map(entries) = native else {
        panic!("expected map");
    };
    assert!(entries.iter().any(|(key, value)| {
        matches!(key, CborValue::Text(key) if key == "blob")
            && matches!(value, CborValue::Bytes(bytes) if bytes == &[0, 1, 2, 255])
    }));

    let decoded = CborCodec
        .decode_value(&encoded, &asset_shape(), &DecodeOptions::default())
        .expect("decode");
    assert_eq!(decoded, value);
}

#[test]
fn cbor_codec_rejects_trailing_bytes() {
    let mut encoded = CborCodec
        .encode_value(
            &Value::Bool(true),
            &TypeShape::Bool,
            &EncodeOptions::default(),
        )
        .expect("encode");
    encoded.push(0xf6);

    let error = CborCodec
        .decode_value(&encoded, &TypeShape::Bool, &DecodeOptions::default())
        .expect_err("trailing bytes");
    assert_eq!(error.kind(), &DataErrorKind::TrailingData);
}

#[test]
fn cbor_codec_enforces_numeric_edge_policy() {
    let error = CborCodec
        .encode_value(
            &Value::Number(Number::F64(f64::INFINITY)),
            &TypeShape::F64,
            &EncodeOptions::default(),
        )
        .expect_err("infinity rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    let mut encoded = Vec::new();
    ciborium::into_writer(&CborValue::Float(1.5), &mut encoded).expect("encode float");
    let error = CborCodec
        .decode_value(&encoded, &TypeShape::U8, &DecodeOptions::default())
        .expect_err("float to integer rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidType);

    let mut encoded = Vec::new();
    ciborium::into_writer(&CborValue::Integer((-1).into()), &mut encoded).expect("encode signed");
    let error = CborCodec
        .decode_value(&encoded, &TypeShape::U8, &DecodeOptions::default())
        .expect_err("negative unsigned rejected");
    assert_eq!(error.kind(), &DataErrorKind::NumberOutOfRange);
}

#[test]
fn cbor_decode_checks_declared_bytes_len_before_reading_payload() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_bytes_len: 4,
            ..DecodeLimits::default()
        },
    };
    let input = [0x45];

    let error = CborCodec
        .decode_value(
            &input,
            &TypeShape::Bytes {
                format: BytesFormat::Binary,
            },
            &options,
        )
        .expect_err("bytes budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn cbor_decode_checks_declared_array_len_before_allocating_items() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_sequence_len: 2,
            ..DecodeLimits::default()
        },
    };
    let input = [0x83];

    let error = CborCodec
        .decode_value(&input, &TypeShape::Seq(Box::new(TypeShape::Unit)), &options)
        .expect_err("array budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn cbor_decode_consumes_indefinite_array_budget_during_parse() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_sequence_len: 2,
            ..DecodeLimits::default()
        },
    };
    let input = [0x9f, 0xf6, 0xf6, 0xf6, 0xff];

    let error = CborCodec
        .decode_value(&input, &TypeShape::Seq(Box::new(TypeShape::Unit)), &options)
        .expect_err("indefinite array budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}
