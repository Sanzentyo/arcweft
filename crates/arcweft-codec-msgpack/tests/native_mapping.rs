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

use arcweft_codec_msgpack::MessagePackCodec;
use arcweft_data::{
    Bytes, BytesFormat, DataErrorKind, DecodeLimits, DecodeOptions, EncodeOptions, FieldShape,
    MapKind, Number, RecordPolicy, TypeShape, Value,
};
use rmpv::Value as MessagePackValue;

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
fn msgpack_codec_preserves_native_binary_bytes() {
    let value = Value::Record(BTreeMap::from([
        (
            "blob".to_owned(),
            Value::Bytes(Bytes::from([0_u8, 1, 2, 255].as_slice())),
        ),
        ("name".to_owned(), Value::String("hero".to_owned())),
    ]));

    let encoded = MessagePackCodec
        .encode_value(&value, &asset_shape(), &EncodeOptions::default())
        .expect("encode");
    let native = rmpv::decode::read_value(&mut Cursor::new(&encoded)).expect("native decode");
    let MessagePackValue::Map(entries) = native else {
        panic!("expected map");
    };
    assert!(entries.iter().any(|(key, value)| {
        matches!(key.as_str(), Some("blob"))
            && matches!(value, MessagePackValue::Binary(bytes) if bytes == &[0, 1, 2, 255])
    }));

    let decoded = MessagePackCodec
        .decode_value(&encoded, &asset_shape(), &DecodeOptions::default())
        .expect("decode");
    assert_eq!(decoded, value);
}

#[test]
fn msgpack_codec_roundtrips_typed_options_tuples_and_ordered_nonstring_maps() {
    let option_shape = TypeShape::option(TypeShape::Unit);
    for value in [
        Value::Option(None),
        Value::Option(Some(Box::new(Value::Unit))),
    ] {
        let encoded = MessagePackCodec
            .encode_value(&value, &option_shape, &EncodeOptions::default())
            .expect("encode option");
        assert_eq!(
            MessagePackCodec
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
        let encoded = MessagePackCodec
            .encode_value(&value, &shape, &EncodeOptions::default())
            .expect("encode tuple");
        assert_eq!(
            MessagePackCodec
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
    let encoded = MessagePackCodec
        .encode_value(&value, &shape, &EncodeOptions::default())
        .expect("encode map");
    let MessagePackValue::Map(entries) =
        rmpv::decode::read_value(&mut Cursor::new(&encoded)).expect("native")
    else {
        panic!("expected native map");
    };
    assert_eq!(entries[0].0.as_u64(), Some(2));
    assert_eq!(entries[1].0.as_u64(), Some(1));
    assert_eq!(
        MessagePackCodec
            .decode_value(&encoded, &shape, &DecodeOptions::default())
            .expect("decode map"),
        value
    );
}

#[test]
fn msgpack_codec_rejects_trailing_bytes() {
    let mut encoded = MessagePackCodec
        .encode_value(
            &Value::Bool(true),
            &TypeShape::Bool,
            &EncodeOptions::default(),
        )
        .expect("encode");
    encoded.push(0xc0);

    let error = MessagePackCodec
        .decode_value(&encoded, &TypeShape::Bool, &DecodeOptions::default())
        .expect_err("trailing bytes");
    assert_eq!(error.kind(), &DataErrorKind::TrailingData);
}

#[test]
fn msgpack_codec_enforces_numeric_edge_policy() {
    let error = MessagePackCodec
        .encode_value(
            &Value::Number(Number::F64(f64::INFINITY)),
            &TypeShape::F64,
            &EncodeOptions::default(),
        )
        .expect_err("infinity rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    let mut encoded = Vec::new();
    rmpv::encode::write_value(&mut encoded, &MessagePackValue::F64(1.5)).expect("encode float");
    let error = MessagePackCodec
        .decode_value(&encoded, &TypeShape::U8, &DecodeOptions::default())
        .expect_err("float to integer rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidType);

    let mut encoded = Vec::new();
    rmpv::encode::write_value(&mut encoded, &MessagePackValue::from(-1)).expect("encode signed");
    let error = MessagePackCodec
        .decode_value(&encoded, &TypeShape::U8, &DecodeOptions::default())
        .expect_err("negative unsigned rejected");
    assert_eq!(error.kind(), &DataErrorKind::NumberOutOfRange);
}

#[test]
fn msgpack_decode_checks_declared_string_len_before_reading_payload() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_string_len: 4,
            ..DecodeLimits::default()
        },
    };
    let input = [0xdb, 0x00, 0x00, 0x00, 0x05];

    let error = MessagePackCodec
        .decode_value(&input, &TypeShape::String, &options)
        .expect_err("string budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn msgpack_decode_checks_declared_array_len_before_allocating_items() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_sequence_len: 2,
            ..DecodeLimits::default()
        },
    };
    let input = [0xdd, 0x00, 0x00, 0x00, 0x03];

    let error = MessagePackCodec
        .decode_value(&input, &TypeShape::Seq(Box::new(TypeShape::Unit)), &options)
        .expect_err("array budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn msgpack_decode_consumes_node_budget_during_parse() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_nodes: 2,
            ..DecodeLimits::default()
        },
    };
    let input = [0x92, 0xc0, 0xc0];

    let error = MessagePackCodec
        .decode_value(&input, &TypeShape::Seq(Box::new(TypeShape::Unit)), &options)
        .expect_err("node budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}
