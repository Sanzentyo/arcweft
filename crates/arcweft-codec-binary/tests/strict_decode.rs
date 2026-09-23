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
use arcweft_codec_binary::ArcweftBinaryCodec;
use arcweft_data::{
    DataErrorKind, DecodeLimits, DecodeOptions, EncodeOptions, FieldShape, MapKind, Number,
    TypeShape, Value,
};

fn decode(input: &[u8], limits: DecodeLimits) -> arcweft_data::Result<Value> {
    ArcweftBinaryCodec.decode_value(input, &TypeShape::Unit, &DecodeOptions { limits })
}

fn payload(tag: u8) -> Vec<u8> {
    let mut out = b"AWBN1".to_vec();
    out.push(tag);
    out
}

fn push_len(out: &mut Vec<u8>, len: u64) {
    out.extend_from_slice(&len.to_le_bytes());
}

fn push_string(out: &mut Vec<u8>, value: &str) {
    push_len(out, value.len() as u64);
    out.extend_from_slice(value.as_bytes());
}

fn push_bool(out: &mut Vec<u8>, value: bool) {
    out.push(if value { 2 } else { 1 });
}

#[test]
fn binary_decode_rejects_duplicate_map_keys() {
    let mut input = payload(12);
    push_len(&mut input, 2);
    push_string(&mut input, "same");
    push_bool(&mut input, false);
    push_string(&mut input, "same");
    push_bool(&mut input, true);

    let error = decode(&input, DecodeLimits::default()).expect_err("duplicate key");
    assert_eq!(error.kind(), &DataErrorKind::DuplicateField);
}

#[test]
fn binary_roundtrips_typed_options_tuples_ordered_maps_and_marker_records() {
    let option_shape = TypeShape::option(TypeShape::Unit);
    for value in [
        Value::Option(None),
        Value::Option(Some(Box::new(Value::Unit))),
    ] {
        let encoded = ArcweftBinaryCodec
            .encode_value(&value, &option_shape, &EncodeOptions::default())
            .expect("encode option");
        assert_eq!(
            ArcweftBinaryCodec
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
        let encoded = ArcweftBinaryCodec
            .encode_value(&value, &shape, &EncodeOptions::default())
            .expect("encode tuple");
        assert_eq!(
            ArcweftBinaryCodec
                .decode_value(&encoded, &shape, &DecodeOptions::default())
                .expect("decode tuple"),
            value
        );
    }

    let map_shape = TypeShape::map(TypeShape::U8, TypeShape::String, MapKind::Ordered);
    let map = Value::map(
        MapKind::Ordered,
        [
            (Value::Number(Number::U(2)), Value::String("two".to_owned())),
            (Value::Number(Number::U(1)), Value::String("one".to_owned())),
        ],
    );
    let encoded = ArcweftBinaryCodec
        .encode_value(&map, &map_shape, &EncodeOptions::default())
        .expect("encode map");
    assert_eq!(
        ArcweftBinaryCodec
            .decode_value(&encoded, &map_shape, &DecodeOptions::default())
            .expect("decode map"),
        map
    );

    let marker_shape = TypeShape::record(
        "MarkerLookingRecord",
        [
            FieldShape::new("$arcweft", "$arcweft", TypeShape::String),
            FieldShape::new("present", "present", TypeShape::Bool),
            FieldShape::new("value", "value", TypeShape::String),
        ],
    );
    let marker_record = Value::Record(std::collections::BTreeMap::from([
        ("$arcweft".to_owned(), Value::String("option".to_owned())),
        ("present".to_owned(), Value::Bool(true)),
        ("value".to_owned(), Value::String("literal".to_owned())),
    ]));
    let encoded = ArcweftBinaryCodec
        .encode_value(&marker_record, &marker_shape, &EncodeOptions::default())
        .expect("encode marker record");
    assert_eq!(
        ArcweftBinaryCodec
            .decode_value(&encoded, &marker_shape, &DecodeOptions::default())
            .expect("decode marker record"),
        marker_record
    );
}

#[test]
fn binary_decode_rejects_invalid_enum_payload_flag() {
    let mut input = payload(13);
    push_string(&mut input, "done");
    input.push(2);

    let error = decode(&input, DecodeLimits::default()).expect_err("invalid flag");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);
}

#[test]
fn binary_decode_checks_sequence_length_before_allocation() {
    let mut input = payload(10);
    push_len(&mut input, 2);
    let limits = DecodeLimits {
        max_sequence_len: 1,
        ..DecodeLimits::default()
    };

    let error = decode(&input, limits).expect_err("sequence length limit");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn binary_decode_checks_collection_item_budget_before_allocation() {
    let mut input = payload(10);
    push_len(&mut input, 2);
    let limits = DecodeLimits {
        max_collection_items: 1,
        ..DecodeLimits::default()
    };

    let error = decode(&input, limits).expect_err("collection item limit");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn binary_decode_checks_depth_during_parse() {
    let mut input = payload(10);
    push_len(&mut input, 1);
    input.push(10);
    push_len(&mut input, 1);
    input.push(0);
    let limits = DecodeLimits {
        max_depth: 0,
        ..DecodeLimits::default()
    };

    let error = decode(&input, limits).expect_err("depth limit");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn binary_decode_checks_string_length_before_reading_payload() {
    let mut input = payload(7);
    push_len(&mut input, 8);
    let limits = DecodeLimits {
        max_string_len: 4,
        ..DecodeLimits::default()
    };

    let error = decode(&input, limits).expect_err("string length limit");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn binary_decode_checks_input_and_node_budgets_during_parse() {
    let input = payload(0);
    let input_error = decode(
        &input,
        DecodeLimits {
            max_input_len: 4,
            ..DecodeLimits::default()
        },
    )
    .expect_err("input length limit");
    assert_eq!(input_error.kind(), &DataErrorKind::LimitExceeded);

    let node_error = decode(
        &input,
        DecodeLimits {
            max_nodes: 0,
            ..DecodeLimits::default()
        },
    )
    .expect_err("node budget");
    assert_eq!(node_error.kind(), &DataErrorKind::LimitExceeded);
}
