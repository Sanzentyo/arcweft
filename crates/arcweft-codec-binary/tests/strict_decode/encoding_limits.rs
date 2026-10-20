use arcweft_data::{
    Codec, DataErrorKind, DecodeOptions, EmptyShapeAccess, EncodeOptions, FieldShape, ShapeRef,
    TypeShape, Value,
};
use std::collections::BTreeMap;

fn assert_bounded(codec: &dyn Codec, value: &Value, shape: &TypeShape) {
    for pretty in [false, true] {
        let options = EncodeOptions {
            pretty,
            ..EncodeOptions::default()
        };
        let bytes = options
            .encode_value(codec, value, ShapeRef::inline(shape), &EmptyShapeAccess)
            .unwrap();
        assert!(!bytes.is_empty());
        for maximum in [0, bytes.len() - 1] {
            let short = EncodeOptions {
                max_output_len: maximum,
                ..options.clone()
            };
            let error = short
                .encode_value(codec, value, ShapeRef::inline(shape), &EmptyShapeAccess)
                .unwrap_err();
            assert_eq!(
                error.kind(),
                &DataErrorKind::LimitExceeded,
                "{} limit {maximum}: {error}",
                codec.id().as_str()
            );
        }
        let exact = EncodeOptions {
            max_output_len: bytes.len(),
            ..options
        };
        let accepted = exact
            .encode_value(codec, value, ShapeRef::inline(shape), &EmptyShapeAccess)
            .unwrap();
        assert_eq!(accepted.len(), bytes.len());
        let decoded = codec
            .decode_value(
                &accepted,
                ShapeRef::inline(shape),
                &EmptyShapeAccess,
                &DecodeOptions::default(),
            )
            .unwrap();
        assert_eq!(&decoded, value);
    }
}

#[test]
fn encoded_output_limits_reject_truncation_and_accept_the_exact_boundary() {
    let shape = TypeShape::record(
        "Bounded",
        [FieldShape::new("text", "text", TypeShape::String)],
    );
    let value = Value::Record(BTreeMap::from([(
        "text".to_owned(),
        Value::String("\0\"\\\n蟹".to_owned()),
    )]));

    assert_bounded(&arcweft_codec_binary::ArcweftBinaryCodec, &value, &shape);
}

#[cfg(feature = "bincode-interop")]
#[test]
fn bincode_interop_bounds_its_own_bytes_without_a_json_byte_round_trip() {
    let value = Value::String("\n".repeat(8));
    let shape = TypeShape::String;
    let json = serde_json::Value::String("\n".repeat(8));
    let expected = bincode::serde::encode_to_vec(&json, bincode::config::standard()).unwrap();
    assert!(serde_json::to_vec(&json).unwrap().len() > expected.len());
    let options = EncodeOptions {
        max_output_len: expected.len(),
        ..EncodeOptions::default()
    };
    let actual = options
        .encode_value(
            &arcweft_codec_binary::BincodeInteropCodec,
            &value,
            ShapeRef::inline(&shape),
            &EmptyShapeAccess,
        )
        .unwrap();
    assert_eq!(actual, expected);
    let short = EncodeOptions {
        max_output_len: expected.len() - 1,
        ..options
    };
    assert_eq!(
        short
            .encode_value(
                &arcweft_codec_binary::BincodeInteropCodec,
                &value,
                ShapeRef::inline(&shape),
                &EmptyShapeAccess
            )
            .unwrap_err()
            .kind(),
        &DataErrorKind::LimitExceeded
    );
}
