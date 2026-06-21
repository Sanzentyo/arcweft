use arcweft_codec_binary::ArcweftBinaryCodec;
use arcweft_data::{Codec, DataErrorKind, DecodeLimits, DecodeOptions, TypeShape, Value};

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
