// Inline-shape test adapter. Production Codec callers supply ShapeRef and ShapeAccess directly.
use self::InlineCodecTestExt as Codec;

trait InlineCodecTestExt {
    fn encode_value(
        &self,
        value: &arcweft_data::Value,
        shape: &arcweft_data::TypeShape,
        options: &arcweft_data::EncodeOptions,
    ) -> arcweft_data::Result<Vec<u8>>;

    fn decode_value(
        &self,
        input: &[u8],
        shape: &arcweft_data::TypeShape,
        options: &arcweft_data::DecodeOptions,
    ) -> arcweft_data::Result<arcweft_data::Value>;
}

impl<C: arcweft_data::Codec + ?Sized> InlineCodecTestExt for C {
    fn encode_value(
        &self,
        value: &arcweft_data::Value,
        shape: &arcweft_data::TypeShape,
        options: &arcweft_data::EncodeOptions,
    ) -> arcweft_data::Result<Vec<u8>> {
        <C as arcweft_data::Codec>::encode_value(
            self,
            value,
            arcweft_data::ShapeRef::Inline(shape),
            &arcweft_data::EmptyShapeAccess,
            options,
        )
    }

    fn decode_value(
        &self,
        input: &[u8],
        shape: &arcweft_data::TypeShape,
        options: &arcweft_data::DecodeOptions,
    ) -> arcweft_data::Result<arcweft_data::Value> {
        <C as arcweft_data::Codec>::decode_value(
            self,
            input,
            arcweft_data::ShapeRef::Inline(shape),
            &arcweft_data::EmptyShapeAccess,
            options,
        )
    }
}
use std::collections::BTreeMap;
use std::io::Cursor;

use arcweft_codec_cbor::CborCodec;
use arcweft_data::{
    Bytes, BytesFormat, DataErrorKind, DecodeLimits, DecodeOptions, EncodeOptions, FieldShape,
    MapKind, Number, RecordPolicy, TypeShape, Value,
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
fn cbor_codec_roundtrips_typed_options_tuples_and_ordered_nonstring_maps() {
    let option_shape = TypeShape::option(TypeShape::Unit);
    for value in [
        Value::Option(None),
        Value::Option(Some(Box::new(Value::Unit))),
    ] {
        let encoded = CborCodec
            .encode_value(&value, &option_shape, &EncodeOptions::default())
            .expect("encode option");
        assert_eq!(
            CborCodec
                .decode_value(&encoded, &option_shape, &DecodeOptions::default())
                .expect("decode option"),
            value
        );
    }

    for (shape, value) in [
        (TypeShape::Tuple(vec![]), Value::Tuple(vec![])),
        (
            TypeShape::Tuple(vec![TypeShape::Bool]),
            Value::Tuple(vec![Value::Bool(true)]),
        ),
    ] {
        let encoded = CborCodec
            .encode_value(&value, &shape, &EncodeOptions::default())
            .expect("encode tuple");
        assert_eq!(
            CborCodec
                .decode_value(&encoded, &shape, &DecodeOptions::default())
                .expect("decode tuple"),
            value
        );
    }

    let shape = TypeShape::map(TypeShape::U8, TypeShape::String, MapKind::Ordered);
    let value = Value::map(
        MapKind::Ordered,
        [
            (Value::Number(Number::U(2)), Value::String("two".to_owned())),
            (Value::Number(Number::U(1)), Value::String("one".to_owned())),
        ],
    );
    let encoded = CborCodec
        .encode_value(&value, &shape, &EncodeOptions::default())
        .expect("encode map");
    let CborValue::Map(entries) = ciborium::from_reader(Cursor::new(&encoded)).expect("native")
    else {
        panic!("expected native map");
    };
    assert_eq!(&entries[0].0, &CborValue::Integer(2.into()));
    assert_eq!(&entries[1].0, &CborValue::Integer(1.into()));
    assert_eq!(
        CborCodec
            .decode_value(&encoded, &shape, &DecodeOptions::default())
            .expect("decode map"),
        value
    );
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
